//! The `wasm-merge` API.
//!
//! `wasm-merge.cpp`'s `main` reads each input in turn, merges every one after
//! the first into the first, fuses the imports that name another input's
//! exports, and writes the result. [`MergeOptions::run`] does the same, with
//! the merge itself the tool's own code, reached through the bridge
//! `wasm-opt-sys`'s build script appends to it.

use crate::api::{Features, FileType, WriterOptions};
use crate::base::{
    merge_add, merge_begin, merge_end, validate_wasm, Module, ModuleReader, ModuleWriter,
};
use crate::run::convert_feature_sets;

use std::path::Path;
use thiserror::Error;

/// An error from [`MergeOptions::run`].
#[derive(Error, Debug)]
pub enum MergeError {
    #[error("Failed to validate wasm: error validating input {name}")]
    ValidateWasmInput { name: String },
    #[error("Failed to validate wasm: error after merging")]
    ValidateWasmOutput,
    #[error("Failed to read module")]
    Read {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    #[error("Failed to merge {name}")]
    Merge {
        name: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    #[error("Failed to write module")]
    Write {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    #[error("Nothing to merge")]
    NoInputs,
}

/// What a later input's export does when the merged module has one of that
/// name already: the tool's `--rename-export-conflicts` and
/// `--skip-export-conflicts`, or neither.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ExportConflicts {
    /// The merge fails.
    #[default]
    Error,
    /// The later export gets a suffix.
    Rename,
    /// The later export is dropped.
    Skip,
}

/// Options for `wasm-merge`.
#[derive(Clone, Debug, Default)]
pub struct MergeOptions {
    /// Options for writing the merged module.
    pub writer: WriterOptions,
    /// The features each input is read with.
    pub features: Features,
    /// What a conflicting export does.
    pub export_conflicts: ExportConflicts,
    /// Whether the name section is written.
    pub debug_info: bool,
    /// What the merged module exports, if not every export the merge kept.
    ///
    /// An input that exports a function only so that another can import it
    /// leaves it exported after the merge, which keeps it, and everything it
    /// calls, alive. Naming the exports the result is meant to have removes
    /// the rest, and what only they kept.
    pub exports: Option<Vec<String>>,
}

impl MergeOptions {
    /// New options: no features beyond the default, and a conflicting export
    /// fails the merge.
    pub fn new() -> Self {
        Self::default()
    }

    /// Merges `inputs`, each a file and the module name its imports are
    /// resolved by, into the first of them, and writes the result to
    /// `outfile`.
    ///
    /// Every input is validated, and so is the result.
    pub fn run(
        &self,
        inputs: &[(&Path, &str)],
        outfile: impl AsRef<Path>,
    ) -> Result<(), MergeError> {
        let ((first, first_name), rest) = inputs.split_first().ok_or(MergeError::NoInputs)?;

        let mut merged = self.read_module(first, first_name)?;
        merge_begin(&mut merged, first_name, self.mode());
        for (path, name) in rest {
            let mut input = self.read_module(path, name)?;
            merge_add(&mut input, name).map_err(|e| MergeError::Merge {
                name: name.to_string(),
                source: Box::from(e),
            })?;
        }
        merge_end(true, self.exports.as_deref()).map_err(|e| MergeError::Merge {
            name: first_name.to_string(),
            source: Box::from(e),
        })?;

        if !validate_wasm(&mut merged) {
            return Err(MergeError::ValidateWasmOutput);
        }

        let mut writer = ModuleWriter::new();
        writer.set_debug_info(self.debug_info);
        match self.writer.file_type {
            FileType::Wat => writer.write_text(&mut merged, outfile.as_ref()),
            FileType::Wasm | FileType::Any => writer.write_binary(&mut merged, outfile.as_ref()),
        }
        .map_err(|e| MergeError::Write {
            source: Box::from(e),
        })
    }

    fn mode(&self) -> u8 {
        match self.export_conflicts {
            ExportConflicts::Error => 0,
            ExportConflicts::Rename => 1,
            ExportConflicts::Skip => 2,
        }
    }

    fn read_module(&self, infile: &Path, name: &str) -> Result<Module, MergeError> {
        let mut m = Module::new();

        let (enabled_features, disabled_features) = convert_feature_sets(&self.features);
        m.apply_features(enabled_features, disabled_features);

        ModuleReader::new()
            .read(infile, &mut m, None)
            .map_err(|e| MergeError::Read {
                source: Box::from(e),
            })?;

        // `wasm-merge` validates each input before merging it, as here.
        if !validate_wasm(&mut m) {
            return Err(MergeError::ValidateWasmInput {
                name: name.to_string(),
            });
        }

        Ok(m)
    }
}
