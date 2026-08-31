use cxx_build::CFG;
use std::fmt::Write as FmtWrite;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> anyhow::Result<()> {
    check_cxx20_support()?;

    let output_dir = std::env::var("OUT_DIR")?;
    let output_dir = Path::new(&output_dir);

    let binaryen_dir = get_binaryen_dir()?;

    let src_dir = binaryen_dir.join("src");
    let src_files = get_src_files(&src_dir)?;

    // Our changes to binaryen, applied to copies under `OUT_DIR`.
    let patched_src_dir = apply_patches(&src_dir)?;

    // Binaryen's `support/suffix_tree` uses LLVM headers even when the DWARF
    // support is not built, so the include path is always needed.
    let llvm_dir = binaryen_dir.join("third_party/llvm-project");
    let llvm_include = llvm_dir.join("include");
    #[cfg(feature = "dwarf")]
    let llvm_files = get_llvm_files(&llvm_dir)?;
    #[cfg(not(feature = "dwarf"))]
    let llvm_files = get_llvm_support_files(&llvm_dir)?;

    // `wasm-interpreter.h` includes `fp16.h`.
    let fp16_include = binaryen_dir.join("third_party/FP16/include");

    let tools_dir = src_dir.join("tools");
    let wasm_opt_src = tools_dir.join("wasm-opt.cpp");
    let wasm_opt_src = get_converted_wasm_opt_cpp(&wasm_opt_src)?;

    let wasm_ctor_eval_src = patched_src_dir.join("tools/wasm-ctor-eval.cpp");
    let wasm_ctor_eval_src = get_converted_wasm_ctor_eval_cpp(&wasm_ctor_eval_src)?;

    let wasm_intrinsics_src = get_converted_wasm_intrinsics_cpp(&src_dir)?;

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let manifest_dir = Path::new(&manifest_dir);
    let wasm_opt_main_shim = manifest_dir.join("src/wasm-opt-main-shim.cpp");

    create_config_header()?;

    // Set up cxx's include path so that wasm-opt-cxx-sys's C++ header can
    // include from these same dirs.
    // The patched headers come first, so that they shadow the originals.
    CFG.exported_header_dirs.push(&patched_src_dir);
    CFG.exported_header_dirs.push(&src_dir);
    CFG.exported_header_dirs.push(&tools_dir);
    CFG.exported_header_dirs.push(&output_dir);
    CFG.exported_header_dirs.push(&llvm_include);
    CFG.exported_header_dirs.push(&fp16_include);

    let mut builder = cxx_build::bridge("src/lib.rs");

    {
        let target_env = std::env::var("CARGO_CFG_TARGET_ENV")?;

        let flags: &[_] = if target_env != "msvc" {
            &[
                "-std=c++20",
                "-w",
                "-Wno-unused-parameter",
                // Binaryen's own CMakeLists sets these two for every non-MSVC
                // build, and this one has to match it: at `-O3` with RTTI on,
                // gcc 14 miscompiles `Precompute`, and the `NonconstantException`
                // that `visitThrow` raises finds no handler at all -- the
                // process aborts on any module with an exception in it. `-O2`
                // with RTTI, or `-O3` without, are both fine, which is what
                // makes it the compiler's bug and not binaryen's; matching the
                // flags binaryen is built and tested with is the fix that is
                // not a guess. See `tests/exceptions.wat`.
                "-fno-rtti",
                "-fno-omit-frame-pointer",
                "-DTHROW_ON_FATAL",
                #[cfg(feature = "dwarf")]
                "-DBUILD_LLVM_DWARF",
                "-DNDEBUG",
            ]
        } else {
            &[
                "/std:c++20",
                "/w",
                "/DTHROW_ON_FATAL",
                #[cfg(feature = "dwarf")]
                "/DBUILD_LLVM_DWARF",
                "/DNDEBUG",
            ]
        };

        for flag in flags {
            builder.flag(flag);
        }
    }

    builder
        .file(wasm_opt_main_shim)
        .files(src_files)
        .file(wasm_opt_src)
        .file(wasm_ctor_eval_src)
        .file(wasm_intrinsics_src);

    builder.files(&llvm_files);

    builder.compile("wasm-opt-cc");

    Ok(())
}

/// Applies `patches/` to copies of the binaryen sources, and returns the
/// directory holding them.
///
/// The copies mirror binaryen's `src/`, so that `patch -p1` finds its targets
/// by the paths the patch names, and so that the directory can go on the
/// include path ahead of the original and shadow a patched header. They are
/// made fresh on every run: a patch applies to a pristine file or the build
/// fails, and there is no state to get out of step.
///
/// This needs `patch` at build time, the way the build already needs a C++
/// compiler.
fn apply_patches(src_dir: &Path) -> anyhow::Result<PathBuf> {
    let output_dir = std::env::var("OUT_DIR")?;
    let patched_dir = Path::new(&output_dir).join("patched-src");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let patches_dir = Path::new(&manifest_dir).join("patches");

    let mut patches: Vec<PathBuf> = fs::read_dir(&patches_dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, std::io::Error>>()?;
    // Applied in name order: `0002` is written against the file `0001` leaves.
    patches.sort();

    let mut copied: Vec<PathBuf> = Vec::new();
    for patch in &patches {
        println!("cargo::rerun-if-changed={}", patch.display());

        // Every file the patch names, copied over pristine before the first
        // patch that touches it -- otherwise `patch` writes outside the copy.
        for file in patched_paths(patch)? {
            if copied.contains(&file) {
                continue;
            }
            let dst = patched_dir.join(&file);
            fs::create_dir_all(dst.parent().expect("joined onto a directory"))?;
            fs::copy(src_dir.join(file.strip_prefix("src")?), &dst)?;
            copied.push(file);
        }

        let status = Command::new("patch")
            .current_dir(&patched_dir)
            .arg("-p1")
            .arg("--no-backup-if-mismatch")
            .arg("-i")
            .arg(patch)
            .status();

        match status {
            Ok(status) if status.success() => {}
            Ok(status) => anyhow::bail!("{} did not apply: {}", patch.display(), status),
            Err(e) => anyhow::bail!("could not run `patch` for {}: {}", patch.display(), e),
        }
    }

    Ok(patched_dir.join("src"))
}

/// The paths a patch changes, as `patch -p1` resolves them.
fn patched_paths(patch: &Path) -> anyhow::Result<Vec<PathBuf>> {
    Ok(fs::read_to_string(patch)?
        .lines()
        .filter_map(|line| line.strip_prefix("+++ b/"))
        .map(|path| PathBuf::from(path.split('\t').next().expect("split yields one")))
        .collect())
}

/// Finds the binaryen source directory.
///
/// During development this will be at the workspace level submodule,
/// but as packaged, will be a subdirectory of the manifest directory.
///
/// The packaged subdirectories are put in place by `publish.sh`.
///
/// The packaged source is pre-processed to remove Binaryen's large test suite.
fn get_binaryen_dir() -> anyhow::Result<PathBuf> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let manifest_dir = Path::new(&manifest_dir);
    let binaryen_packaged_dir = manifest_dir.join("binaryen");
    let binaryen_submodule_dir = manifest_dir.join("../../binaryen");

    match (
        binaryen_packaged_dir.is_dir(),
        binaryen_submodule_dir.is_dir(),
    ) {
        (true, _) => Ok(binaryen_packaged_dir),
        (_, true) => Ok(binaryen_submodule_dir),
        (false, false) => anyhow::bail!(
            "binaryen source directory doesn't exist (maybe `git submodule update --init`?)"
        ),
    }
}

/// Replaces the `main` declaration with a C ABI and a different name.
///
/// It can be called from Rust and doesn't clash with Rust's `main`.
fn get_converted_wasm_opt_cpp(src_dir: &Path) -> anyhow::Result<PathBuf> {
    let wasm_opt_file = File::open(src_dir)?;
    let reader = BufReader::new(wasm_opt_file);

    let output_dir = std::env::var("OUT_DIR")?;
    let output_dir = Path::new(&output_dir);

    let temp_file_dir = output_dir.join("wasm_opt.cpp.temp");
    let temp_file = File::create(&temp_file_dir)?;

    let mut writer = BufWriter::new(temp_file);
    for line in reader.lines() {
        let mut line = line?;

        if line.contains("int main") {
            line = line.replace("int main", "extern \"C\" int wasm_opt_main_actual");
        }

        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
    }

    let output_wasm_opt_file = output_dir.join("wasm-opt.cpp");
    fs::rename(&temp_file_dir, &output_wasm_opt_file)?;

    Ok(output_wasm_opt_file)
}

/// Renames `main` and appends a bridge that the bindings can link against.
///
/// Everything `wasm-ctor-eval.cpp` does interesting lives in an anonymous
/// namespace, and so has internal linkage. Code appended to the same
/// translation unit can still see those names though, so instead of patching
/// the namespace we add a few functions with external linkage at the end.
fn get_converted_wasm_ctor_eval_cpp(src_dir: &Path) -> anyhow::Result<PathBuf> {
    let wasm_ctor_eval_file = File::open(src_dir)?;
    let reader = BufReader::new(wasm_ctor_eval_file);

    let output_dir = std::env::var("OUT_DIR")?;
    let output_dir = Path::new(&output_dir);

    let temp_file_dir = output_dir.join("wasm_ctor_eval.cpp.temp");
    let temp_file = File::create(&temp_file_dir)?;

    let mut writer = BufWriter::new(temp_file);
    for line in reader.lines() {
        let mut line = line?;

        // The bindings do not call it, but leaving it out of the way keeps it
        // from clashing with Rust's `main`.
        if line.contains("int main") {
            line = line.replace("int main", "extern \"C\" int wasm_ctor_eval_main_actual");
        }

        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
    }

    writer.write_all(WASM_CTOR_EVAL_BRIDGE.as_bytes())?;
    writer.flush()?;
    drop(writer);

    let output_wasm_ctor_eval_file = output_dir.join("wasm-ctor-eval.cpp");
    fs::rename(&temp_file_dir, &output_wasm_ctor_eval_file)?;

    Ok(output_wasm_ctor_eval_file)
}

/// The bridge appended to `wasm-ctor-eval.cpp`.
///
/// It is declared by `wasm-opt-cxx-sys`'s `shims.h`.
const WASM_CTOR_EVAL_BRIDGE: &str = r#"
//
// Appended by wasm-opt-sys's build script.
//

namespace wasm_opt_rs {

bool ctorEvalCanEval(wasm::Module& wasm) { return canEval(wasm); }

bool ctorEvalRun(wasm::Module& wasm,
                 const std::string& ctors,
                 const std::string& keptExports,
                 bool ignoreExternalInputArg,
                 bool quietArg,
                 uint32_t maxStepsArg) {
  // These are statics of this translation unit, and `main` only ever sets
  // them. Assign all of them so that a call cannot observe the state left
  // behind by a previous one.
  ignoreExternalInput = ignoreExternalInputArg;
  quiet = quietArg;
  maxSteps = maxStepsArg;
  invalidState = false;

  // `wasm-ctor-eval` splits these the same way, so that a name containing a
  // comma inside brackets is not split.
  wasm::String::Split ctorList, keptExportList;
  if (!ctors.empty()) {
    ctorList = wasm::String::Split(ctors, ",");
  }
  if (!keptExports.empty()) {
    keptExportList = wasm::String::Split(keptExports, ",");
  }

  evalCtors(wasm, ctorList, keptExportList);

  // Evalling can leave the module in a state that cannot be written out.
  return !invalidState;
}

} // namespace wasm_opt_rs
"#;

fn get_src_files(src_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let analysis_dir = src_dir.join("analysis");
    let analysis_files = ["cfg.cpp"];
    let analysis_files = analysis_files.iter().map(|f| analysis_dir.join(f));

    let wasm_dir = src_dir.join("wasm");
    let wasm_files = [
        "literal.cpp",
        "parsing.cpp",
        "source-map.cpp",
        "wasm-binary.cpp",
        "wasm-debug.cpp",
        "wasm-emscripten.cpp",
        "wasm-io.cpp",
        "wasm-ir-builder.cpp",
        "wasm-stack.cpp",
        "wasm-stack-opts.cpp",
        "wasm-type.cpp",
        "wasm-type-shape.cpp",
        "wasm-validator.cpp",
        "wasm.cpp",
    ];
    let wasm_files = wasm_files.iter().map(|f| wasm_dir.join(f));

    let parser_dir = src_dir.join("parser");
    let parser_files = [
        "context-decls.cpp",
        "context-defs.cpp",
        "parse-1-decls.cpp",
        "parse-2-typedefs.cpp",
        "parse-3-implicit-types.cpp",
        "parse-4-module-types.cpp",
        "parse-5-defs.cpp",
        "wast-parser.cpp",
        "wat-parser.cpp",
    ];
    let parser_files = parser_files.iter().map(|f| parser_dir.join(f));

    let support_dir = src_dir.join("support");
    let support_files = [
        "archive.cpp",
        "bits.cpp",
        "colors.cpp",
        "command-line.cpp",
        "debug.cpp",
        "dfa_minimization.cpp",
        "file.cpp",
        "int128.cpp",
        "intervals.cpp",
        "istring.cpp",
        "json.cpp",
        "name.cpp",
        "path.cpp",
        "safe_integer.cpp",
        "string.cpp",
        "suffix_tree.cpp",
        "suffix_tree_node.cpp",
        "threads.cpp",
        "utilities.cpp",
    ];
    let support_files = support_files.iter().map(|f| support_dir.join(f));

    let ir_dir = src_dir.join("ir");
    let ir_files = [
        "abstract.cpp",
        "constraint.cpp",
        "drop.cpp",
        "effects.cpp",
        "eh-utils.cpp",
        "ExpressionManipulator.cpp",
        "ExpressionAnalyzer.cpp",
        "export-utils.cpp",
        "LocalGraph.cpp",
        "LocalStructuralDominance.cpp",
        "lubs.cpp",
        "memory-utils.cpp",
        "metadata.cpp",
        "module-splitting.cpp",
        "module-utils.cpp",
        "names.cpp",
        "possible-contents.cpp",
        "principal-type.cpp",
        "properties.cpp",
        "public-type-validator.cpp",
        "ReFinalize.cpp",
        "return-utils.cpp",
        "runtime-global.cpp",
        "runtime-table.cpp",
        "stack-utils.cpp",
        "table-utils.cpp",
        "type-updating.cpp",
    ];
    let ir_files = ir_files.iter().map(|f| ir_dir.join(f));

    let passes_dir = src_dir.join("passes");
    let passes_files = get_files_from_dir(&passes_dir)?;

    let fuzzing_dir = src_dir.join("tools/fuzzing");
    let fuzzing_files = [
        "fuzzing.cpp",
        "heap-types.cpp",
        "parameters.cpp",
        "random.cpp",
    ];
    let fuzzing_files = fuzzing_files.iter().map(|f| fuzzing_dir.join(f));

    let asmjs_dir = src_dir.join("asmjs");
    let asmjs_files = ["asm_v_wasm.cpp", "asmangle.cpp", "shared-constants.cpp"];
    let asmjs_files = asmjs_files.iter().map(|f| asmjs_dir.join(f));

    let cfg_dir = src_dir.join("cfg");
    let cfg_files = ["Relooper.cpp"];
    let cfg_files = cfg_files.iter().map(|f| cfg_dir.join(f));

    let file_intrinsics = disambiguate_file(&ir_dir.join("intrinsics.cpp"), "intrinsics-ir.cpp")?;

    let src_files: Vec<_> = None
        .into_iter()
        .chain(analysis_files)
        .chain(wasm_files)
        .chain(parser_files)
        .chain(support_files)
        .chain(ir_files)
        .chain(passes_files)
        .chain(fuzzing_files)
        .chain(asmjs_files)
        .chain(cfg_files)
        .chain(Some(file_intrinsics).into_iter())
        .collect();

    Ok(src_files)
}

fn get_files_from_dir(src_dir: &Path) -> anyhow::Result<impl Iterator<Item = PathBuf> + '_> {
    let files = fs::read_dir(src_dir)?
        .map(|f| f.expect("error reading dir"))
        .filter(|f| f.file_name().into_string().expect("UTF8").ends_with(".cpp"))
        .map(move |f| src_dir.join(f.path()));

    Ok(files)
}

fn disambiguate_file(input_file: &Path, new_file_name: &str) -> anyhow::Result<PathBuf> {
    let output_dir = std::env::var("OUT_DIR")?;
    let output_dir = Path::new(&output_dir);
    let output_file = output_dir.join(new_file_name);

    fs::copy(input_file, &output_file)?;

    Ok(output_file)
}

/// Pre-process the WasmIntrinsics.cpp.in file and return a path to the processed file.
///
/// This file needs to be injected with the contents of wasm-intrinsics.wat,
/// replacing `@WASM_INTRINSICS_SIZE@` with the size of the wat + 1,
/// and `@WASM_INTRINSICS_EMBED@` with the hex-encoded contents of the wat,
/// appended with `0x00`.
///
/// The extra byte is presumably a null terminator.
fn get_converted_wasm_intrinsics_cpp(src_dir: &Path) -> anyhow::Result<PathBuf> {
    let src_passes_dir = src_dir.join("passes");

    let output_dir = std::env::var("OUT_DIR")?;
    let output_dir = Path::new(&output_dir);

    let wasm_intrinsics_cpp_in_file = src_passes_dir.join("WasmIntrinsics.cpp.in");
    let wasm_intrinsics_cpp_out_file = output_dir.join("WasmIntrinsics.cpp");

    let (wasm_intrinsics_wat_hex, wasm_intrinsics_wat_bytes) =
        load_wasm_intrinsics_wat(&src_passes_dir)?;

    configure_file(
        &wasm_intrinsics_cpp_in_file,
        &wasm_intrinsics_cpp_out_file,
        &[
            (
                "WASM_INTRINSICS_SIZE",
                format!("{}", wasm_intrinsics_wat_bytes),
            ),
            ("WASM_INTRINSICS_EMBED", wasm_intrinsics_wat_hex),
        ],
    )?;

    Ok(wasm_intrinsics_cpp_out_file)
}

fn load_wasm_intrinsics_wat(passes_dir: &Path) -> anyhow::Result<(String, usize)> {
    let wasm_intrinsics_wat = passes_dir.join("wasm-intrinsics.wat");
    let wat_contents = std::fs::read_to_string(&wasm_intrinsics_wat)?;

    let mut buffer = String::with_capacity(wat_contents.len() * 5 /* 0xNN, */ + 4 /* null */);

    for byte in wat_contents.bytes() {
        write!(buffer, "0x{:02x},", byte)?;
    }
    write!(buffer, "0x00")?;

    Ok((buffer, wat_contents.len() + 1))
}

/// A rough implementation of CMake's `configure_file` directive.
///
/// Consume `src_file` and output `dst_file`.
///
/// `replacements` is a list of key-value pairs from variable name
/// to a textual substitute for that variable.
///
/// Any variables in the source file, surrounded by `@`, e.g.
/// `@WASM_INTRINSICS_SIZE@`, will be replaced with the specified value. The
/// variable as specified in the `replacements` list does not include the `@`
/// symbols.
///
/// re: <https://cmake.org/cmake/help/latest/command/configure_file.html>
fn configure_file(
    src_file: &Path,
    dst_file: &Path,
    replacements: &[(&str, String)],
) -> anyhow::Result<()> {
    let mut src = std::fs::read_to_string(src_file)?;

    for (var, txt) in replacements {
        let var = format!("@{}@", var);
        src = src.replace(&var, txt);
    }

    std::fs::write(dst_file, src)?;

    Ok(())
}

fn create_config_header() -> anyhow::Result<()> {
    let output_dir = std::env::var("OUT_DIR")?;
    let output_dir = Path::new(&output_dir);
    let config_file = output_dir.join("config.h");

    let config_text = "#define PROJECT_VERSION \"132 (version_132)\"";

    fs::write(&config_file, config_text)?;

    Ok(())
}

fn check_cxx20_support() -> anyhow::Result<()> {
    let mut builder = cc::Build::new();
    builder.cpp(true);

    let target_env = std::env::var("CARGO_CFG_TARGET_ENV")?;
    let cxx20_flag = if target_env != "msvc" {
        "-std=c++20"
    } else {
        "/std:c++20"
    };

    if !builder.is_flag_supported(cxx20_flag)? {
        return Err(anyhow::anyhow!(
            "C++ compiler does not support `{}` flag",
            cxx20_flag
        ));
    }

    Ok(())
}

/// The subset of the LLVM sources needed even when DWARF support is not built.
///
/// Binaryen's `support/suffix_tree`, used by the `outlining` pass, uses LLVM's
/// containers regardless of whether DWARF support is enabled.
#[cfg(not(feature = "dwarf"))]
fn get_llvm_support_files(llvm_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    Ok(vec![
        llvm_dir.join("ErrorHandling.cpp"),
        llvm_dir.join("SmallVector.cpp"),
    ])
}

#[cfg(feature = "dwarf")]
fn get_llvm_files(llvm_dir: &Path) -> anyhow::Result<[PathBuf; 63]> {
    let llvm_dwarf = disambiguate_file(&llvm_dir.join("Dwarf.cpp"), "LLVMDwarf.cpp")?;
    let llvm_debug = disambiguate_file(&llvm_dir.join("Debug.cpp"), "LLVMDebug.cpp")?;
    // Array taken from
    // https://github.com/WebAssembly/binaryen/blob/616c08bc1ec604a822d354cbb0353b7994cec72d/third_party/llvm-project/CMakeLists.txt
    Ok([
        llvm_dir.join("Binary.cpp"),
        llvm_dir.join("ConvertUTF.cpp"),
        llvm_dir.join("DataExtractor.cpp"),
        llvm_dir.join("DJB.cpp"),
        llvm_debug,
        llvm_dir.join("dwarf2yaml.cpp"),
        llvm_dir.join("DWARFAbbreviationDeclaration.cpp"),
        llvm_dir.join("DWARFAcceleratorTable.cpp"),
        llvm_dir.join("DWARFAddressRange.cpp"),
        llvm_dir.join("DWARFCompileUnit.cpp"),
        llvm_dir.join("DWARFContext.cpp"),
        llvm_dir.join("DWARFDataExtractor.cpp"),
        llvm_dir.join("DWARFDebugAbbrev.cpp"),
        llvm_dir.join("DWARFDebugAddr.cpp"),
        llvm_dir.join("DWARFDebugAranges.cpp"),
        llvm_dir.join("DWARFDebugArangeSet.cpp"),
        llvm_dir.join("DWARFDebugFrame.cpp"),
        llvm_dir.join("DWARFDebugInfoEntry.cpp"),
        llvm_dir.join("DWARFDebugLine.cpp"),
        llvm_dir.join("DWARFDebugLoc.cpp"),
        llvm_dir.join("DWARFDebugMacro.cpp"),
        llvm_dir.join("DWARFDebugPubTable.cpp"),
        llvm_dir.join("DWARFDebugRangeList.cpp"),
        llvm_dir.join("DWARFDebugRnglists.cpp"),
        llvm_dir.join("DWARFDie.cpp"),
        llvm_dir.join("DWARFEmitter.cpp"),
        llvm_dir.join("DWARFExpression.cpp"),
        llvm_dir.join("DWARFFormValue.cpp"),
        llvm_dir.join("DWARFGdbIndex.cpp"),
        llvm_dir.join("DWARFListTable.cpp"),
        llvm_dir.join("DWARFTypeUnit.cpp"),
        llvm_dir.join("DWARFUnit.cpp"),
        llvm_dir.join("DWARFUnitIndex.cpp"),
        llvm_dir.join("DWARFVerifier.cpp"),
        llvm_dir.join("DWARFVisitor.cpp"),
        llvm_dir.join("DWARFYAML.cpp"),
        llvm_dir.join("Error.cpp"),
        llvm_dir.join("ErrorHandling.cpp"),
        llvm_dir.join("FormatVariadic.cpp"),
        llvm_dir.join("Hashing.cpp"),
        llvm_dir.join("LEB128.cpp"),
        llvm_dir.join("LineIterator.cpp"),
        llvm_dir.join("MCRegisterInfo.cpp"),
        llvm_dir.join("MD5.cpp"),
        llvm_dir.join("MemoryBuffer.cpp"),
        llvm_dir.join("NativeFormatting.cpp"),
        llvm_dir.join("ObjectFile.cpp"),
        llvm_dir.join("obj2yaml_Error.cpp"),
        llvm_dir.join("Optional.cpp"),
        llvm_dir.join("Path.cpp"),
        llvm_dir.join("raw_ostream.cpp"),
        llvm_dir.join("ScopedPrinter.cpp"),
        llvm_dir.join("SmallVector.cpp"),
        llvm_dir.join("SourceMgr.cpp"),
        llvm_dir.join("StringMap.cpp"),
        llvm_dir.join("StringRef.cpp"),
        llvm_dir.join("SymbolicFile.cpp"),
        llvm_dir.join("Twine.cpp"),
        llvm_dir.join("UnicodeCaseFold.cpp"),
        llvm_dir.join("WithColor.cpp"),
        llvm_dir.join("YAMLParser.cpp"),
        llvm_dir.join("YAMLTraits.cpp"),
        llvm_dwarf,
    ])
}
