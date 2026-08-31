;; A module whose memory cannot be flattened: it has a passive segment, and
;; `array.new_data` names one, which is how a GC language materialises a
;; constant array. Binaryen's own tool gives up on the whole module for this.
(module
  (type $bytes (array (mut i8)))
  (memory $m 1 1)
  (data $active (i32.const 10) "waka waka")
  (data $passive "hello")
  (global $read (mut i32) (i32.const 0))
  (global $arr (mut (ref null $bytes)) (ref.null $bytes))
  (export "test1" (func $test1))
  (export "test2" (func $test2))
  (export "test3" (func $test3))
  (export "memory" (memory $m))
  ;; Exported so that what the ctors computed survives the cleanup passes.
  (export "read" (global $read))
  (export "arr" (global $arr))
  ;; Reading memory needs no flattening: the interpreter seeds each active
  ;; segment at its own offset. 119 is "w".
  (func $test1
    (global.set $read (i32.load8_u (i32.const 10)))
  )
  ;; Neither does reading a data segment as an array.
  (func $test2
    (global.set $arr (array.new_data $bytes $passive (i32.const 0) (i32.const 5)))
  )
  ;; Writing memory does. This one, and only this one, must fail.
  (func $test3
    (i32.store8 (i32.const 12) (i32.const 110))
  )
)
