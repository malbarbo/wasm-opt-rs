use std::fs;
use std::path::Path;
use tempfile::Builder;
use wasm_opt::{ExportConflicts, FileType, MergeError, MergeOptions};

static MAIN_WAT: &str = "tests/merge_main.wat";
static LIB_WAT: &str = "tests/merge_lib.wat";

fn merge(options: &mut MergeOptions) -> anyhow::Result<String> {
    let temp_dir = Builder::new().prefix("wasm_merge_tests").tempdir()?;
    let outfile = temp_dir.path().join("outfile.wat");
    options.all_features().writer_file_type(FileType::Wat).run(
        &[(Path::new(MAIN_WAT), "main"), (Path::new(LIB_WAT), "lib")],
        &outfile,
    )?;
    Ok(fs::read_to_string(&outfile)?)
}

#[test]
fn an_import_is_fused_to_the_export_it_names() -> anyhow::Result<()> {
    let out = merge(MergeOptions::new().export_conflicts(ExportConflicts::Skip))?;

    assert!(!out.contains("(import"), "{}", out);
    // Both memories, the first input's first.
    assert!(out.contains("(memory $0 1)"), "{}", out);
    assert!(out.contains("(memory $0_1 2)"), "{}", out);
    // The second `memory` export was dropped; `add` stays, as an export.
    assert_eq!(out.matches("(export \"memory\"").count(), 1, "{}", out);
    assert!(out.contains("(export \"add\""), "{}", out);
    Ok(())
}

#[test]
fn a_conflicting_export_fails_by_default() {
    let r = merge(&mut MergeOptions::new());
    let e = r.unwrap_err();
    assert!(
        matches!(e.downcast_ref(), Some(MergeError::Merge { .. })),
        "{:?}",
        e
    );
}

#[test]
fn a_conflicting_export_can_be_renamed() -> anyhow::Result<()> {
    let out = merge(MergeOptions::new().export_conflicts(ExportConflicts::Rename))?;
    assert!(out.contains("(export \"memory_2\""), "{}", out);
    Ok(())
}

#[test]
fn merges_on_several_threads_do_not_share_state() {
    let threads: Vec<_> = (0..8)
        .map(|_| {
            std::thread::spawn(|| {
                for _ in 0..20 {
                    let out =
                        merge(MergeOptions::new().export_conflicts(ExportConflicts::Skip)).unwrap();
                    assert!(!out.contains("(import"), "{}", out);
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
}

#[test]
fn the_merged_module_exports_only_what_it_is_told() -> anyhow::Result<()> {
    let out = merge(
        MergeOptions::new()
            .export_conflicts(ExportConflicts::Skip)
            .exports(vec!["run".to_string()]),
    )?;
    assert!(out.contains("(export \"run\""), "{}", out);
    assert!(!out.contains("(export \"add\""), "{}", out);
    assert!(!out.contains("(export \"memory\""), "{}", out);
    // `add` is still there, called by `run`.
    assert!(out.contains("i32.add"), "{}", out);
    Ok(())
}
