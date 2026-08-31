;; `data.drop` is the one memory effect the external interface never sees -- the
;; interpreter records it in a set of its own -- so it cannot be failed one ctor
;; at a time, and a module whose memory will not flatten is refused for it.
(module
  (memory $m 1 1)
  (data $active (i32.const 10) "waka waka")
  (data $passive "hello")
  (global $read (mut i32) (i32.const 0))
  (export "test1" (func $test1))
  (export "drop" (func $drop))
  (export "memory" (memory $m))
  (export "read" (global $read))
  (func $test1
    (global.set $read (i32.load8_u (i32.const 10)))
  )
  (func $drop
    (data.drop $passive)
  )
)
