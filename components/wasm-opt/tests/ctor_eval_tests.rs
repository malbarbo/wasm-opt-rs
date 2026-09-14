use std::fs;
use tempfile::Builder;
use wasm_opt::{CtorEvalError, CtorEvalOptions, CtorEvalOutcome, FileType};

static CTOR_EVAL_WAT: &str = "tests/ctor_eval.wat";
static CTOR_EVAL_STOP_WAT: &str = "tests/ctor_eval_stop.wat";
static CTOR_EVAL_UNFLATTENED_WAT: &str = "tests/ctor_eval_unflattened.wat";
static CTOR_EVAL_LOOP_WAT: &str = "tests/ctor_eval_loop.wat";
static CTOR_EVAL_DATA_DROP_WAT: &str = "tests/ctor_eval_data_drop.wat";
static CTOR_EVAL_START_WAT: &str = "tests/ctor_eval_start.wat";

fn eval_file(
    options: &mut CtorEvalOptions,
    infile: &str,
) -> anyhow::Result<(CtorEvalOutcome, String)> {
    let temp_dir = Builder::new().prefix("wasm_ctor_eval_tests").tempdir()?;
    let outfile = temp_dir.path().join("outfile.wat");

    let outcome = options
        .writer_file_type(FileType::Wat)
        .run(infile, &outfile)?;

    Ok((outcome, fs::read_to_string(&outfile)?))
}

fn eval(options: &mut CtorEvalOptions) -> anyhow::Result<(CtorEvalOutcome, String)> {
    eval_file(options, CTOR_EVAL_WAT)
}

#[test]
fn eval_ctors_works() -> anyhow::Result<()> {
    let (outcome, out) = eval(
        CtorEvalOptions::new()
            .add_ctor("test1")
            .add_ctor("test2")
            .add_ctor("test3"),
    )?;

    assert_eq!(outcome, CtorEvalOutcome::Finished);

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
    let (outcome, out) = eval(&mut CtorEvalOptions::new())?;

    assert_eq!(outcome, CtorEvalOutcome::Finished);

    assert!(out.contains("test1"));
    assert!(out.contains("test2"));
    assert!(out.contains("test3"));
    assert!(out.contains(r#""waka waka waka waka waka"#));

    Ok(())
}

#[test]
fn kept_exports_works() -> anyhow::Result<()> {
    let (outcome, out) = eval(
        CtorEvalOptions::new()
            .add_ctor("test1")
            .add_ctor("test3")
            .add_kept_export("test3"),
    )?;

    assert_eq!(outcome, CtorEvalOutcome::Finished);

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
    let outcome = CtorEvalOptions::new()
        .add_ctor("test1")
        .add_ctor("test2")
        .writer_file_type(FileType::Wat)
        .run(CTOR_EVAL_STOP_WAT, &outfile)?;

    // And it says so, by name.
    assert_eq!(outcome, CtorEvalOutcome::StoppedAt("test2".to_string()));

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

/// A module whose memory cannot be flattened is still evalled.
///
/// Reading memory does not need a flat one -- the interpreter seeds each active
/// segment at its own offset -- and neither does reading a segment as an array.
/// Writing does, so the ctor that writes fails, and only it.
#[test]
fn a_module_that_cannot_be_flattened_is_still_evalled() -> anyhow::Result<()> {
    let (outcome, out) = eval_file(
        CtorEvalOptions::new()
            .all_features()
            .add_ctor("test1")
            .add_ctor("test2")
            .add_ctor("test3"),
        CTOR_EVAL_UNFLATTENED_WAT,
    )?;

    assert_eq!(outcome, CtorEvalOutcome::StoppedAt("test3".to_string()));

    // `test1` read the active segment: 119 is "w", the first byte of "waka".
    assert!(!out.contains(r#"(export "test1""#), "{}", out);
    assert!(
        out.contains("(global $read (mut i32) (i32.const 119))"),
        "{}",
        out
    );

    // `test2` read the passive segment as an array, which is now a constant.
    // The segment itself is gone with it, nothing else naming it.
    assert!(!out.contains(r#"(export "test2""#), "{}", out);
    assert!(out.contains("array.new_fixed"), "{}", out);
    assert!(!out.contains(r#""hello""#), "{}", out);

    // `test3` wrote memory, and nothing could carry that back into a module
    // whose memory is not one segment at offset 0. It was left alone, and so
    // was what it would have written to.
    assert!(out.contains(r#"(export "test3""#), "{}", out);
    assert!(
        out.contains(r#"(data $active (i32.const 10) "waka waka")"#),
        "{}",
        out
    );

    Ok(())
}

/// Without a step limit the evaluation of a ctor that does not terminate does
/// not terminate either, so the limit is what this asserts by finishing.
#[test]
fn the_step_limit_stops_a_ctor_that_does_not_end() -> anyhow::Result<()> {
    let (outcome, out) = eval_file(
        CtorEvalOptions::new()
            .add_ctor("test1")
            .add_ctor("spin")
            .max_steps(10_000),
        CTOR_EVAL_LOOP_WAT,
    )?;

    assert_eq!(outcome, CtorEvalOutcome::StoppedAt("spin".to_string()));

    // `test1` was evalled and keeps what it wrote; `spin` ran over the limit
    // and was left alone.
    assert!(!out.contains(r#"(export "test1""#), "{}", out);
    assert!(out.contains(r#"(export "spin""#), "{}", out);
    assert!(out.contains(r#""naka waka""#), "{}", out);

    Ok(())
}

/// A `data.drop` anywhere in a module whose memory will not flatten gives the
/// whole module up, as every such module was given up before.
#[test]
fn a_data_drop_is_still_refused() -> anyhow::Result<()> {
    let (outcome, out) = eval_file(
        CtorEvalOptions::new().all_features().add_ctor("test1"),
        CTOR_EVAL_DATA_DROP_WAT,
    )?;

    assert_eq!(outcome, CtorEvalOutcome::NothingEvaluated);

    // `test1` does nothing a flat memory is needed for, and was still not
    // evalled: the module was never entered.
    assert!(out.contains(r#"(export "test1""#), "{}", out);
    assert!(
        out.contains("(global $read (mut i32) (i32.const 0))"),
        "{}",
        out
    );

    Ok(())
}

/// A start function runs when the module is instantiated, so one that cannot be
/// evalled leaves no instance to eval a ctor in. Evaluation stops before trying
/// any, and names none: setting a ctor aside would stop the same way.
#[test]
fn a_start_function_that_cannot_be_evalled_stops_before_any_ctor() -> anyhow::Result<()> {
    let (outcome, out) = eval_file(
        CtorEvalOptions::new().add_ctor("test1"),
        CTOR_EVAL_START_WAT,
    )?;

    assert_eq!(outcome, CtorEvalOutcome::NothingEvaluated);

    // `test1` is a store that is safe to eval, and it was not tried.
    assert!(out.contains(r#"(export "test1""#), "{}", out);
    assert!(out.contains(r#""waka waka""#), "{}", out);

    Ok(())
}
