//! Compares the `CtorEvalOptions` API against binaryen's `wasm-ctor-eval`.
//!
//! This runs both over binaryen's own `test/ctor-eval` corpus, the same way
//! binaryen's `check.py` does, and requires the outputs to be identical.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use wasm_opt::{CtorEvalOptions, FileType};

fn get_workspace() -> Result<PathBuf> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let manifest_dir = PathBuf::from(manifest_dir);

    Ok(manifest_dir.join("../.."))
}

fn get_binaryen_wasm_ctor_eval() -> Result<PathBuf> {
    let out_dir = std::env::var("OUT_DIR")?;
    let out_dir = PathBuf::from(out_dir);

    Ok(out_dir.join("binaryen-test-build/bin/wasm-ctor-eval"))
}

/// The corpus, as `check.py` collects it: every module that has a `.ctors`
/// file next to it.
fn get_test_modules() -> Result<Vec<PathBuf>> {
    let test_dir = get_workspace()?.join("binaryen/test/ctor-eval");

    let mut modules = Vec::new();

    for entry in fs::read_dir(&test_dir)? {
        let path = entry?.path();

        let is_module = matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("wast") | Some("wasm")
        );

        if is_module && get_ctors_path(&path).exists() {
            modules.push(path);
        }
    }

    if modules.is_empty() {
        bail!("no ctor-eval test modules found in {}", test_dir.display());
    }

    modules.sort();

    Ok(modules)
}

fn get_ctors_path(module: &Path) -> PathBuf {
    let mut path = module.as_os_str().to_owned();
    path.push(".ctors");

    PathBuf::from(path)
}

/// The extra arguments `check.py` passes based on the test's name.
fn kept_exports_of(module: &Path) -> &'static [&'static str] {
    let name = module.to_string_lossy();

    if name.contains("results") {
        &["test1", "test3"]
    } else {
        &[]
    }
}

fn ignore_external_input(module: &Path) -> bool {
    module.to_string_lossy().contains("ignore-external-input")
}

fn run_binaryen(module: &Path, ctors: &str, outfile: &Path) -> Result<String> {
    let mut cmd = Command::new(get_binaryen_wasm_ctor_eval()?);

    cmd.arg(module)
        .args(["-all", "-S", "-o"])
        .arg(outfile)
        .args(["--ctors", ctors]);

    if ignore_external_input(module) {
        cmd.arg("--ignore-external-input");
    }

    let kept_exports = kept_exports_of(module);
    if !kept_exports.is_empty() {
        cmd.args(["--kept-exports", &kept_exports.join(",")]);
    }

    let output = cmd.output()?;

    if !output.status.success() {
        bail!(
            "wasm-ctor-eval failed on {}: {}",
            module.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(fs::read_to_string(outfile)?)
}

fn run_api(module: &Path, ctors: &str, outfile: &Path) -> Result<String> {
    let mut options = CtorEvalOptions::new();

    options
        .all_features()
        .writer_file_type(FileType::Wat)
        .ignore_external_input(ignore_external_input(module));

    for ctor in ctors.split(',').filter(|c| !c.is_empty()) {
        options.add_ctor(ctor);
    }

    for kept_export in kept_exports_of(module) {
        options.add_kept_export(*kept_export);
    }

    options.run(module, outfile)?;

    Ok(fs::read_to_string(outfile)?)
}

#[test]
fn ctor_eval_matches_binaryen() -> Result<()> {
    let tempdir = tempfile::tempdir()?;
    let modules = get_test_modules()?;

    println!("comparing {} ctor-eval test modules", modules.len());

    for module in modules {
        let ctors = fs::read_to_string(get_ctors_path(&module))
            .with_context(|| format!("reading ctors of {}", module.display()))?;
        let ctors = ctors.trim();

        let binaryen_out = run_binaryen(&module, ctors, &tempdir.path().join("binaryen.wat"))
            .with_context(|| format!("binaryen on {}", module.display()))?;
        let api_out = run_api(&module, ctors, &tempdir.path().join("api.wat"))
            .with_context(|| format!("api on {}", module.display()))?;

        if binaryen_out != api_out {
            bail!(
                "output differs on {}\n--- binaryen ---\n{}\n--- api ---\n{}",
                module.display(),
                binaryen_out,
                api_out
            );
        }
    }

    Ok(())
}
