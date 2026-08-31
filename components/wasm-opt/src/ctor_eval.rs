//! An API for `wasm-ctor-eval`, Binaryen's compile-time evaluator.
//!
//! This re-implements the logic of `wasm-ctor-eval.cpp` on top of
//! [`CtorEvalOptions`], the same way [`crate::run`] re-implements
//! `wasm-opt.cpp` on top of [`crate::OptimizationOptions`].

use crate::api::{Features, FileType, ReaderOptions, WriterOptions};
use crate::base::{
    ctor_eval_can_eval, ctor_eval_run, validate_wasm, Module, ModuleReader, ModuleWriter,
    PassRunner,
};
use crate::passes::Pass;
use crate::run::convert_feature_sets;

use std::path::Path;
use thiserror::Error;

/// The passes `wasm-ctor-eval` runs after evalling.
const CLEANUP_PASSES: &[Pass] = &[
    // The memory may have been flattened for evalling, so pack it again.
    Pass::MemoryPacking,
    Pass::RemoveUnusedNames,
    Pass::Dce,
    Pass::MergeBlocks,
    Pass::Vacuum,
    Pass::RemoveUnusedModuleElements,
];

/// An error resulting from the [`CtorEvalOptions::run`] method.
#[derive(Error, Debug)]
pub enum CtorEvalError {
    /// The input module did not validate.
    #[error("Failed to validate wasm: error validating input")]
    ValidateWasmInput,
    /// The module did not validate after evalling.
    #[error("Failed to validate wasm: error after evalling")]
    ValidateWasmOutput,
    /// An error occurred while reading the input module.
    #[error("Failed to read module")]
    Read {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    /// An error occurred while evaluating the ctors.
    ///
    /// This is what a name in [`CtorEvalOptions::ctors`] that is not an
    /// exported function of the module produces.
    #[error("Failed to evaluate ctors")]
    Eval {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    /// An error occurred while writing the output module.
    #[error("Failed to write module")]
    Write {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    /// The input file path represents stdin to Binaryen,
    /// but the API does not support reading stdin.
    #[error("Refusing to read from stdin")]
    InvalidStdinPath,
}

/// Options for evaluating global constructors at compile time.
///
/// This type declares all the options supported by the `wasm-ctor-eval`
/// command line tool. It can be modified directly or by its [builder-pattern]
/// methods.
///
/// Call [`CtorEvalOptions::run`] to perform the evaluation.
///
/// [builder-pattern]: https://rust-unofficial.github.io/patterns/patterns/creational/builder.html
///
/// # Examples
///
/// ```no_run
/// use wasm_opt::CtorEvalOptions;
///
/// CtorEvalOptions::new()
///     .add_ctor("start")
///     .run("hello_world.wasm", "hello_world_evalled.wasm")?;
///
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct CtorEvalOptions {
    /// Options for reading the input wasm module.
    pub reader: ReaderOptions,
    /// Options for writing the evalled wasm module.
    pub writer: WriterOptions,
    /// The set of wasm-features.
    pub features: Features,
    /// The exported functions to evaluate, in order.
    ///
    /// This corresponds to the `--ctors` argument to `wasm-ctor-eval`.
    ///
    /// Evaluation stops at the first one that cannot be evaluated; the
    /// remaining ctors are left alone.
    pub ctors: Vec<String>,
    /// The ctors whose exports are kept even after they are evaluated.
    ///
    /// This corresponds to the `--kept-exports` argument to `wasm-ctor-eval`.
    ///
    /// The exports of the other evaluated ctors are removed from the module.
    /// A kept export is replaced by a function that just returns the value the
    /// original returned.
    pub kept_exports: Vec<String>,
    /// Assume the program reads no external input as it runs.
    ///
    /// This corresponds to the `--ignore-external-input` argument to
    /// `wasm-ctor-eval`.
    ///
    /// When true, environment variables are assumed to be unset, stdin is
    /// assumed to be empty, and `main` is assumed to receive no arguments.
    /// Otherwise reading any of them stops the evaluation.
    ///
    /// Default: `false`.
    pub ignore_external_input: bool,
    /// Emit the names section and debug info.
    ///
    /// This corresponds to the `--debuginfo` argument to `wasm-ctor-eval`.
    ///
    /// Default: `false`.
    pub debug_info: bool,
    /// Do not log the progress of the evaluation to stdout.
    ///
    /// This corresponds to the `--quiet` argument to `wasm-ctor-eval`.
    ///
    /// Unlike the command line tool, this defaults to `true`: a library has no
    /// business writing to stdout unless asked to.
    pub quiet: bool,
    /// How many instructions the evaluation may execute before giving up.
    ///
    /// This has no counterpart in `wasm-ctor-eval`, which can only be stopped
    /// by killing the process. A ctor that runs over the limit fails, the way
    /// one that reads an import does: the ctors evaluated before it keep what
    /// they evaluated, and the ones after it are not attempted.
    ///
    /// The count is over the whole call, not per ctor, so it bounds the time
    /// [`CtorEvalOptions::run`] can take.
    ///
    /// Default: `0`, which is no limit.
    pub max_steps: u32,
}

impl CtorEvalOptions {
    /// Create a new `CtorEvalOptions` with no ctors to evaluate.
    ///
    /// Add ctors with [`CtorEvalOptions::add_ctor`].
    pub fn new() -> Self {
        CtorEvalOptions {
            reader: ReaderOptions::default(),
            writer: WriterOptions::default(),
            features: Features::default(),
            ctors: Vec::new(),
            kept_exports: Vec::new(),
            ignore_external_input: false,
            debug_info: false,
            quiet: true,
            max_steps: 0,
        }
    }
}

impl Default for CtorEvalOptions {
    fn default() -> Self {
        CtorEvalOptions::new()
    }
}

/// Execution.
impl CtorEvalOptions {
    /// Run the Binaryen ctor evaluator.
    ///
    /// This loads a module from a file, evaluates the ctors named by
    /// [`CtorEvalOptions::ctors`], and writes the module back to a file.
    ///
    /// A ctor that cannot be evaluated is not an error: evaluation just stops
    /// there, and the module is written out with whatever was evalled before
    /// it. Set [`CtorEvalOptions::quiet`] to `false` to see why it stopped.
    ///
    /// # Errors
    ///
    /// Returns error on I/O failure, if the input fails to parse, if a name in
    /// `ctors` is not an exported function, or if either the input module or
    /// the evalled module fails to validate.
    ///
    /// The Rust API does not support reading a module on stdin, as the CLI
    /// does. If `infile` is empty or "-",
    /// [`CtorEvalError::InvalidStdinPath`] is returned.
    pub fn run(
        &self,
        infile: impl AsRef<Path>,
        outfile: impl AsRef<Path>,
    ) -> Result<(), CtorEvalError> {
        let infile: &Path = infile.as_ref();
        let outfile: &Path = outfile.as_ref();

        if infile.as_os_str().is_empty() || infile == Path::new("-") {
            return Err(CtorEvalError::InvalidStdinPath);
        }

        let mut m = self.read_module(infile)?;

        if !self.reader.preserve_type_order {
            m.clear_type_indices();
        }

        if !validate_wasm(&mut m) {
            return Err(CtorEvalError::ValidateWasmInput);
        }

        // A module the evaluator will not touch at all is written back out
        // unchanged.
        if ctor_eval_can_eval(&mut m) {
            let ctors = self.ctors.join(",");
            let kept_exports = self.kept_exports.join(",");

            let valid_state = ctor_eval_run(
                &mut m,
                &ctors,
                &kept_exports,
                self.ignore_external_input,
                self.quiet,
                self.max_steps,
            )
            .map_err(|e| CtorEvalError::Eval {
                source: Box::from(e),
            })?;

            if !valid_state {
                // Evalling ran into something it could not handle partway
                // through, leaving the module in a state that cannot be
                // written out. Forget all of it by reading the input again,
                // with nothing at all evalled.
                //
                // As `wasm-ctor-eval` does, the type order is not discarded
                // on this path.
                m = self.read_module(infile)?;
            }

            if !validate_wasm(&mut m) {
                return Err(CtorEvalError::ValidateWasmOutput);
            }

            let mut pass_runner = PassRunner::new(&mut m);

            CLEANUP_PASSES
                .iter()
                .for_each(|pass| pass_runner.add(pass.name()));

            pass_runner.run();
        }

        self.write_module(&mut m, outfile)
    }

    fn read_module(&self, infile: &Path) -> Result<Module, CtorEvalError> {
        let mut m = Module::new();

        let (enabled_features, disabled_features) = convert_feature_sets(&self.features);
        m.apply_features(enabled_features, disabled_features);

        let mut reader = ModuleReader::new();

        match self.reader.file_type {
            FileType::Wasm => reader.read_binary(infile, &mut m, None),
            FileType::Wat => reader.read_text(infile, &mut m),
            FileType::Any => reader.read(infile, &mut m, None),
        }
        .map_err(|e| CtorEvalError::Read {
            source: Box::from(e),
        })?;

        Ok(m)
    }

    fn write_module(&self, m: &mut Module, outfile: &Path) -> Result<(), CtorEvalError> {
        let mut writer = ModuleWriter::new();
        writer.set_debug_info(self.debug_info);

        match self.writer.file_type {
            FileType::Wasm => writer.write_binary(m, outfile),
            FileType::Wat => writer.write_text(m, outfile),
            FileType::Any => match self.reader.file_type {
                FileType::Any | FileType::Wasm => writer.write_binary(m, outfile),
                FileType::Wat => writer.write_text(m, outfile),
            },
        }
        .map_err(|e| CtorEvalError::Write {
            source: Box::from(e),
        })
    }
}
