use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<()> {
    if cfg!(windows) {
        panic!("this doesn't work on windows yet");
    }

    build_binaryen_wasm_opt()?;
    build_rust_wasm_opt()?;

    Ok(())
}

struct Dirs {
    workspace: PathBuf,
    binaryen_src: PathBuf,
    binaryen_build: PathBuf,
}

fn get_dirs() -> Result<Dirs> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let manifest_dir = PathBuf::from(manifest_dir);
    let workspace = manifest_dir.join("../..");
    let binaryen_src = workspace.join("binaryen");
    let out_dir = std::env::var("OUT_DIR")?;
    let out_dir = PathBuf::from(out_dir);
    let binaryen_build = out_dir.join("binaryen-test-build");

    Ok(Dirs {
        workspace,
        binaryen_src,
        binaryen_build,
    })
}

fn build_binaryen_wasm_opt() -> Result<()> {
    let dirs = get_dirs()?;

    // The tools have to be built from the same sources `wasm-opt-sys` compiles
    // -- patches included -- or this compares our binaryen against binaryen's.
    let binaryen_src = patched_binaryen_src(&dirs)?;

    std::fs::create_dir_all(&dirs.binaryen_build)?;

    let cmake_status = Command::new("cmake")
        .current_dir(&dirs.binaryen_build)
        .arg(&binaryen_src)
        .arg("-DBUILD_TESTS=OFF")
        .status()?;

    if !cmake_status.success() {
        bail!("cmake failed");
    }

    let jobs = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let make_status = Command::new("make")
        .current_dir(&dirs.binaryen_build)
        .arg(format!("-j{}", jobs))
        .status()?;

    if !make_status.success() {
        bail!("make failed");
    }

    Ok(())
}

/// A copy of the binaryen sources with `wasm-opt-sys`'s `patches/` applied.
///
/// The copy is refreshed rather than remade, and a patched file is written only
/// when its content changes, so that the cmake build below stays incremental.
/// `cp -au` skips a file already up to date, which includes the patched ones:
/// they are newer than the sources they were made from.
fn patched_binaryen_src(dirs: &Dirs) -> Result<PathBuf> {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let dst = out_dir.join("binaryen-patched");
    fs::create_dir_all(&dst)?;

    let status = Command::new("cp")
        .arg("-au")
        .arg(format!("{}/.", dirs.binaryen_src.display()))
        .arg(&dst)
        .status()?;
    if !status.success() {
        bail!("copying the binaryen sources failed");
    }

    // Applied to their own pristine copies, in name order: `0002` is written
    // against the file `0001` leaves.
    let patches_dir = dirs.workspace.join("components/wasm-opt-sys/patches");
    let mut patches: Vec<PathBuf> = fs::read_dir(&patches_dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, std::io::Error>>()?;
    patches.sort();

    let work = out_dir.join("patch-work");
    let _ = fs::remove_dir_all(&work);

    let mut patched: Vec<PathBuf> = Vec::new();
    for patch in &patches {
        println!("cargo::rerun-if-changed={}", patch.display());

        for file in patched_paths(patch)? {
            if patched.contains(&file) {
                continue;
            }
            let to = work.join(&file);
            fs::create_dir_all(to.parent().expect("joined onto a directory"))?;
            fs::copy(dirs.binaryen_src.join(&file), &to)?;
            patched.push(file);
        }

        let status = Command::new("patch")
            .current_dir(&work)
            .arg("-p1")
            .arg("--no-backup-if-mismatch")
            .arg("-i")
            .arg(patch)
            .status()?;
        if !status.success() {
            bail!("{} did not apply: {}", patch.display(), status);
        }
    }

    for file in &patched {
        let (from, to) = (work.join(file), dst.join(file));
        if fs::read(&from)? != fs::read(&to).unwrap_or_default() {
            fs::copy(&from, &to)?;
        }
    }

    Ok(dst)
}

/// The paths a patch changes, as `patch -p1` resolves them.
fn patched_paths(patch: &Path) -> Result<Vec<PathBuf>> {
    Ok(fs::read_to_string(patch)?
        .lines()
        .filter_map(|line| line.strip_prefix("+++ b/"))
        .map(|path| PathBuf::from(path.split('\t').next().expect("split yields one")))
        .collect())
}

fn build_rust_wasm_opt() -> Result<()> {
    let dirs = get_dirs()?;

    let mut cmd = Command::new("cargo");
    cmd.current_dir(dirs.workspace)
        .args(["build", "-p", "wasm-opt", "--release"]);

    #[cfg(feature = "dwarf")]
    cmd.args(["--features", "dwarf"]);

    let cargo_status = cmd.status()?;

    if !cargo_status.success() {
        bail!("cargo failed");
    }

    Ok(())
}
