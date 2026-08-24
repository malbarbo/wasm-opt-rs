use std::fs;
use tempfile::Builder;
use wasm_opt::{CtorEvalError, CtorEvalOptions, FileType};

static CTOR_EVAL_WAT: &str = "tests/ctor_eval.wat";
static CTOR_EVAL_STOP_WAT: &str = "tests/ctor_eval_stop.wat";

fn eval(options: &mut CtorEvalOptions) -> anyhow::Result<String> {
    let temp_dir = Builder::new().prefix("wasm_ctor_eval_tests").tempdir()?;
    let outfile = temp_dir.path().join("outfile.wat");

    options
        .writer_file_type(FileType::Wat)
        .run(CTOR_EVAL_WAT, &outfile)?;

    Ok(fs::read_to_string(&outfile)?)
}

#[test]
fn eval_ctors_works() -> anyhow::Result<()> {
    let out = eval(
        CtorEvalOptions::new()
            .add_ctor("test1")
            .add_ctor("test2")
            .add_ctor("test3"),
    )?;

    // The evalled ctors are gone, and their writes were applied to the data
    // segment.
    assert!(!out.contains("test1"));
    assert!(!out.contains("test2"));
    assert!(!out.contains("test3"));
    assert!(out.contains(r#"(data $0 (i32.const 10) "nas\00\00\00aka yzkx waka wakm"#));

    Ok(())
}

#[test]
fn eval_no_ctors_keeps_the_module() -> anyhow::Result<()> {
    let out = eval(&mut CtorEvalOptions::new())?;

    assert!(out.contains("test1"));
    assert!(out.contains("test2"));
    assert!(out.contains("test3"));
    assert!(out.contains(r#""waka waka waka waka waka"#));

    Ok(())
}

#[test]
fn kept_exports_works() -> anyhow::Result<()> {
    let out = eval(
        CtorEvalOptions::new()
            .add_ctor("test1")
            .add_ctor("test3")
            .add_kept_export("test3"),
    )?;

    // The kept export stays, as a function that does nothing.
    assert!(!out.contains("test1"));
    assert!(out.contains(r#"(export "test3""#));

    Ok(())
}

#[test]
fn unknown_ctor_is_an_error() -> anyhow::Result<()> {
    let res = eval(CtorEvalOptions::new().add_ctor("no-such-export"));

    let err = res
        .expect_err("expected an error")
        .downcast::<CtorEvalError>()?;

    match err {
        CtorEvalError::Eval { source } => {
            assert!(source.to_string().contains("export not found"));
        }
        e => panic!("unexpected error: {:?}", e),
    }

    Ok(())
}

#[test]
fn stopping_early_is_not_an_error() -> anyhow::Result<()> {
    let temp_dir = Builder::new().prefix("wasm_ctor_eval_tests").tempdir()?;
    let outfile = temp_dir.path().join("outfile.wat");

    // `test2` calls an import, which cannot be evalled.
    CtorEvalOptions::new()
        .add_ctor("test1")
        .add_ctor("test2")
        .writer_file_type(FileType::Wat)
        .run(CTOR_EVAL_STOP_WAT, &outfile)?;

    let out = fs::read_to_string(&outfile)?;

    // `test1` was evalled and its export removed; `test2` was left alone.
    assert!(!out.contains("test1"));
    assert!(out.contains(r#"(export "test2""#));
    assert!(out.contains(r#""naka waka"#));

    Ok(())
}

#[test]
fn invalid_stdin_path_is_an_error() {
    let res = CtorEvalOptions::new().run("", "outfile.wasm");

    assert!(matches!(res, Err(CtorEvalError::InvalidStdinPath)));

    let res = CtorEvalOptions::new().run("-", "outfile.wasm");

    assert!(matches!(res, Err(CtorEvalError::InvalidStdinPath)));
}
