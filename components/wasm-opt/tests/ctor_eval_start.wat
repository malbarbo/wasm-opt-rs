(module
  (import "env" "external-input" (func $external-input))
  (memory $m 256 256)
  (data (i32.const 10) "waka waka")
  (export "test1" (func $test1))
  (export "memory" (memory $m))
  (start $start)
  (func $start
    (call $external-input) ;; not safe to call, so there is no instance to eval in
  )
  (func $test1
    (i32.store8 (i32.const 10) (i32.const 110)) ;; a safe store, never tried
  )
)
