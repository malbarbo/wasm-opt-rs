;; `spin` does not terminate. Nothing in the evaluator can time out, so only a
;; step limit stops it.
(module
  (memory $m 1 1)
  (data (i32.const 10) "waka waka")
  (export "test1" (func $test1))
  (export "spin" (func $spin))
  (export "memory" (memory $m))
  (func $test1
    (i32.store8 (i32.const 10) (i32.const 110))
  )
  (func $spin
    (loop $l (br $l))
  )
)
