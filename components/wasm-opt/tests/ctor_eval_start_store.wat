;; A module whose memory cannot be flattened, as in ctor_eval_unflattened.wat,
;; and whose start function writes it, as a C module's `_initialize` does.
(module
  (type $bytes (array (mut i8)))
  (memory $m 1 1)
  (data $active (i32.const 10) "waka waka")
  (data $passive "hello")
  (global $read (mut i32) (i32.const 0))
  (global $arr (mut (ref null $bytes)) (ref.null $bytes))
  (export "test1" (func $test1))
  (export "memory" (memory $m))
  (export "read" (global $read))
  (export "arr" (global $arr))
  (start $start)
  ;; 110 is "n": "wana waka" for as long as the module runs.
  (func $start
    (i32.store8 (i32.const 12) (i32.const 110))
    (global.set $arr (array.new_data $bytes $passive (i32.const 0) (i32.const 5)))
  )
  (func $test1
    (global.set $read (i32.load8_u (i32.const 12)))
  )
)
