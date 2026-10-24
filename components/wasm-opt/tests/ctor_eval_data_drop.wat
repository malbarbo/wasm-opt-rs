;; A ctor that drops a passive segment. Code that outlives evaluation still names
;; each one, so the module keeps them, and what is kept must be what the drop
;; left: an empty segment, from which nothing but nothing can be copied.
(module
  (type $bytes (array (mut i8)))
  (type $funcs (array (mut funcref)))
  (import "env" "outside" (func $outside))
  (memory $m 1 1)
  (data $active (i32.const 10) "waka waka")
  (data $passive "hello")
  (data $kept "kept")
  (elem $elems func $read)
  (elem $kept-elems func $read)
  (global $read (mut i32) (i32.const 0))
  (export "test1" (func $test1))
  (export "drop" (func $drop))
  (export "fail" (func $fail))
  (export "bytes" (func $bytes))
  (export "funcs" (func $funcs))
  (export "memory" (memory $m))
  (export "read" (global $read))
  (func $test1
    (global.set $read (i32.load8_u (i32.const 10)))
  )
  (func $drop
    (data.drop $passive)
    (elem.drop $elems)
  )
  ;; The second item drops and then fails, so none of it is evalled.
  (func $fail
    (drop (i32.const 0))
    (block
      (data.drop $kept)
      (elem.drop $kept-elems)
      (call $outside)
    )
  )
  (func $bytes (param $s i32) (result (ref $bytes))
    (drop (array.new_data $bytes $kept (i32.const 0) (local.get $s)))
    (array.new_data $bytes $passive (i32.const 0) (local.get $s))
  )
  (func $funcs (param $s i32) (result (ref $funcs))
    (drop (array.new_elem $funcs $kept-elems (i32.const 0) (local.get $s)))
    (array.new_elem $funcs $elems (i32.const 0) (local.get $s))
  )
  (func $read (result i32)
    (global.get $read)
  )
)
