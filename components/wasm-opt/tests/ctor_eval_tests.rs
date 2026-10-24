use std::fs;
use tempfile::Builder;
use wasm_opt::{CtorEvalError, CtorEvalOptions, CtorEvalOutcome, FileType};

static CTOR_EVAL_WAT: &str = "tests/ctor_eval.wat";
static CTOR_EVAL_STOP_WAT: &str = "tests/ctor_eval_stop.wat";
static CTOR_EVAL_UNFLATTENED_WAT: &str = "tests/ctor_eval_unflattened.wat";
static CTOR_EVAL_LOOP_WAT: &str = "tests/ctor_eval_loop.wat";
static CTOR_EVAL_DATA_DROP_WAT: &str = "tests/ctor_eval_data_drop.wat";
static CTOR_EVAL_START_WAT: &str = "tests/ctor_eval_start.wat";
static CTOR_EVAL_START_STORE_WAT: &str = "tests/ctor_eval_start_store.wat";
static CTOR_EVAL_KEEP_GOING_WAT: &str = "tests/ctor_eval_keep_going.wat";
static CTOR_EVAL_LONG_ARRAY_WAT: &str = "tests/ctor_eval_long_array.wat";
static CTOR_EVAL_BYTE_ARRAY_WAT: &str = "tests/ctor_eval_byte_array.wat";
static CTOR_EVAL_PARTIAL_LOCAL_WAT: &str = "tests/ctor_eval_partial_local.wat";
static CTOR_EVAL_MEMORIES_WAT: &str = "tests/ctor_eval_memories.wat";

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
/// Nor does writing: what changed is carried back as segments of its own.
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

    assert_eq!(outcome, CtorEvalOutcome::Finished);

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

    // `test3` wrote memory, and what changed follows the segment it wrote
    // over, as a segment instantiation applies after it.
    assert!(!out.contains(r#"(export "test3""#), "{}", out);
    assert!(out.contains(r#"(data $active (i32.const 10) "#), "{}", out);
    assert!(out.contains(r#"(i32.const 12) "n")"#), "{}", out);

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

/// A `data.drop` or an `elem.drop` a ctor runs empties the segment it drops, as
/// a drop does, and one a ctor passed over under `keep_going` runs leaves its
/// segment as it was. The interpreter records a drop on the instance, and not
/// through the external interface.
#[test]
fn a_dropped_segment_is_empty_and_a_skipped_drop_leaves_it() -> anyhow::Result<()> {
    let (outcome, out) = eval_file(
        CtorEvalOptions::new()
            .all_features()
            .add_ctor("test1")
            .add_ctor("drop")
            .add_ctor("fail")
            .keep_going(true),
        CTOR_EVAL_DATA_DROP_WAT,
    )?;

    assert_eq!(outcome, CtorEvalOutcome::Skipped(vec!["fail".to_string()]));

    assert!(!out.contains(r#"(export "test1""#), "{}", out);
    assert!(!out.contains(r#"(export "drop""#), "{}", out);
    // 119 is "w".
    assert!(
        out.contains("(global $read (mut i32) (i32.const 119))"),
        "{}",
        out
    );
    assert!(out.contains(r#"(data $passive "")"#), "{}", out);
    assert!(out.contains("(elem $elems func)"), "{}", out);
    assert!(out.contains(r#"(data $kept "kept")"#), "{}", out);
    assert!(out.contains("(elem $kept-elems func $read)"), "{}", out);

    Ok(())
}

/// The fills of an evaluated module drop their segments in the start function,
/// so a second evaluation of what the first wrote runs a `data.drop` before any
/// ctor, and is not given up for it.
#[test]
fn an_evaluated_module_with_fills_is_evaluated_again() -> anyhow::Result<()> {
    let temp_dir = Builder::new().prefix("wasm_ctor_eval_tests").tempdir()?;
    let first = temp_dir.path().join("first.wasm");
    let outcome = CtorEvalOptions::new()
        .all_features()
        .add_ctor("make")
        .run(CTOR_EVAL_LONG_ARRAY_WAT, &first)?;
    assert_eq!(outcome, CtorEvalOutcome::Finished);

    let (outcome, out) = eval_file(
        CtorEvalOptions::new().all_features().add_ctor("again"),
        first.to_str().unwrap(),
    )?;

    assert_eq!(outcome, CtorEvalOutcome::Finished);
    assert!(!out.contains(r#"(export "again""#), "{}", out);
    // 200 + 7.
    assert!(out.contains("(i32.const 207)"), "{}", out);

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

/// A start function that writes a memory which cannot be flattened is evalled
/// like any other: what it wrote is carried into the module as an active
/// segment after the others, which instantiation applies last.
#[test]
fn a_start_function_that_writes_an_unflattened_memory_is_evalled() -> anyhow::Result<()> {
    let (outcome, out) = eval_file(
        CtorEvalOptions::new().all_features().add_ctor("test1"),
        CTOR_EVAL_START_STORE_WAT,
    )?;

    assert_eq!(outcome, CtorEvalOutcome::Finished);

    // The start function is gone, and its store is data. `test1` read it.
    assert!(!out.contains("(start"), "{}", out);
    assert!(!out.contains("i32.store8"), "{}", out);
    assert!(out.contains(r#"(i32.const 12) "n")"#), "{}", out);
    assert!(!out.contains(r#"(export "test1""#), "{}", out);
    assert!(
        out.contains("(global $read (mut i32) (i32.const 110))"),
        "{}",
        out
    );

    // The segment it wrote over is still there, and the new one comes after
    // it. The byte it overwrites is dead, and a cleanup pass zeroes it.
    assert!(
        out.contains(r#"(data $active (i32.const 10) "wa\00a waka")"#),
        "{}",
        out
    );
    assert!(
        out.find("(data $active").unwrap() < out.find("(data $ctor-eval$memory").unwrap(),
        "{}",
        out
    );

    Ok(())
}

/// Each memory is carried back on its own, and a ctor passed over under
/// `keep_going` leaves no byte of what it stored in the item that failed, even
/// one past every segment, where no instantiation writes.
#[test]
fn every_memory_is_carried_back_and_a_skipped_ctor_leaves_none_of_its_stores() -> anyhow::Result<()>
{
    let (outcome, out) = eval_file(
        CtorEvalOptions::new()
            .all_features()
            .add_ctor("test1")
            .add_ctor("fail")
            .add_ctor("test2")
            .keep_going(true),
        CTOR_EVAL_MEMORIES_WAT,
    )?;

    assert_eq!(outcome, CtorEvalOutcome::Skipped(vec!["fail".to_string()]));

    assert!(!out.contains("(start"), "{}", out);
    assert!(out.contains(r#"(memory $b) (i32.const 1) "X")"#), "{}", out);
    // `$a` is the first memory, which the printer leaves unnamed.
    assert!(out.contains(r#"(i32.const 100) "\07")"#), "{}", out);
    // `fail`'s store is still code in what is left of it, and not data.
    assert!(!out.contains(r#"(i32.const 200) "\t")"#), "{}", out);
    assert!(
        out.contains("(global $read (mut i32) (i32.const 7))"),
        "{}",
        out
    );

    Ok(())
}

/// Under `keep_going` a ctor that fails is passed over, and the ctors after it
/// are evaluated from the state it would have left had evaluation stopped
/// there: with the part of it that evaluated, and none of the part that did
/// not.
#[test]
fn keep_going_passes_over_a_ctor_that_fails() -> anyhow::Result<()> {
    let (outcome, out) = eval_file(
        CtorEvalOptions::new()
            .all_features()
            .add_ctor("a")
            .add_ctor("fail")
            .add_ctor("b")
            .keep_going(true),
        CTOR_EVAL_KEEP_GOING_WAT,
    )?;

    assert_eq!(outcome, CtorEvalOutcome::Skipped(vec!["fail".to_string()]));

    // `a` and `b` were evaluated, and their exports removed.
    assert!(!out.contains(r#"(export "a""#), "{}", out);
    assert!(!out.contains(r#"(export "b""#), "{}", out);

    // `fail` keeps its export, for the part of it that was not evaluated.
    assert!(out.contains(r#"(export "fail""#), "{}", out);

    // Its first item was, and `b` read it; its second set `$z` before failing,
    // which neither the module nor `b` saw.
    assert!(
        out.contains("(global $x (mut i32) (i32.const 100))"),
        "{}",
        out
    );
    assert!(
        out.contains("(global $y (mut i32) (i32.const 101))"),
        "{}",
        out
    );

    // `b` read the object `a` allocated, across the new instance.
    assert!(
        out.contains("(global $w (mut i32) (i32.const 42))"),
        "{}",
        out
    );

    Ok(())
}

/// Without `keep_going` the same ctors stop at the one that fails.
#[test]
fn without_keep_going_a_failing_ctor_stops_evaluation() -> anyhow::Result<()> {
    let (outcome, out) = eval_file(
        CtorEvalOptions::new()
            .all_features()
            .add_ctor("a")
            .add_ctor("fail")
            .add_ctor("b"),
        CTOR_EVAL_KEEP_GOING_WAT,
    )?;

    assert_eq!(outcome, CtorEvalOutcome::StoppedAt("fail".to_string()));
    assert!(out.contains(r#"(export "b""#), "{}", out);

    Ok(())
}

/// V8 refuses an `array.new_fixed` of more than 10 000 values, so a longer
/// array is made empty and the start function copies its values in: numbers
/// from a passive data segment, references from a passive element segment.
#[test]
fn a_long_array_is_filled_in_the_start_function() -> anyhow::Result<()> {
    let (outcome, out) = eval_file(
        CtorEvalOptions::new().all_features().add_ctor("make"),
        CTOR_EVAL_LONG_ARRAY_WAT,
    )?;

    assert_eq!(outcome, CtorEvalOutcome::Finished);
    assert!(!out.contains("array.new_fixed"), "{}", out);
    assert!(out.contains("(array.init_data $B"), "{}", out);
    assert!(out.contains("(array.init_elem $R"), "{}", out);
    assert!(out.contains("(data.drop "), "{}", out);
    assert!(out.contains("(elem.drop "), "{}", out);
    assert!(out.contains("(start "), "{}", out);

    // The byte the ctor wrote is the segment's last.
    assert!(out.contains(r#"\c8")"#), "{}", out);

    Ok(())
}

/// An array whose values take fewer bytes as data than as constants, a
/// string's bytes, is filled in the start function too, however short.
#[test]
fn an_array_smaller_as_data_is_filled_in_the_start_function() -> anyhow::Result<()> {
    let (outcome, out) = eval_file(
        CtorEvalOptions::new().all_features().add_ctor("make"),
        CTOR_EVAL_BYTE_ARRAY_WAT,
    )?;

    assert_eq!(outcome, CtorEvalOutcome::Finished);
    assert!(out.contains("(array.init_data $B"), "{}", out);
    assert!(out.contains(r#""hhhhhhhh"#), "{}", out);
    assert!(out.contains("(array.new_fixed $I 3"), "{}", out);

    Ok(())
}

/// A ctor evalled in part resumes with its locals, and an object one holds is
/// serialized into a defining global the code that resumes reads.
#[test]
fn a_partly_evalled_ctor_keeps_the_objects_its_locals_hold() -> anyhow::Result<()> {
    let (outcome, out) = eval_file(
        CtorEvalOptions::new().all_features().add_ctor("f"),
        CTOR_EVAL_PARTIAL_LOCAL_WAT,
    )?;

    assert_eq!(outcome, CtorEvalOutcome::StoppedAt("f".to_string()));
    assert!(out.contains("(global $ctor-eval$global"), "{}", out);

    Ok(())
}
