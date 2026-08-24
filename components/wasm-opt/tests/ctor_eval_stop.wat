(module
  (import "env" "external-input" (func $external-input))
  (memory $m 256 256)
  (data (i32.const 10) "waka waka")
  (export "test1" (func $test1))
  (export "test2" (func $test2))
  (export "memory" (memory $m))
  (func $test1
    (i32.store8 (i32.const 10) (i32.const 110)) ;; a safe store
  )
  (func $test2
    (call $external-input) ;; not safe to call, evalling must stop here
  )
)
