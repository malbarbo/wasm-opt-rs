;; Two memories, neither of which can be flattened: Binaryen flattens only a
;; module of one, and this one has a passive segment besides. It is the shape of
;; a GC program merged with a C module, each with a memory of its own.
(module
  (type $bytes (array (mut i8)))
  (import "env" "outside" (func $outside))
  (memory $a 1 1)
  (memory $b 1 1)
  (data $active (memory $a) (i32.const 10) "waka waka")
  (data $other (memory $b) (i32.const 0) "abc")
  (data $passive "hello")
  (global $read (mut i32) (i32.const 0))
  (global $arr (mut (ref null $bytes)) (ref.null $bytes))
  (export "test1" (func $test1))
  (export "fail" (func $fail))
  (export "test2" (func $test2))
  (export "a" (memory $a))
  (export "b" (memory $b))
  (export "read" (global $read))
  (export "arr" (global $arr))
  (start $start)
  ;; 88 is "X": "aXc" in $b.
  (func $start
    (i32.store8 $b (i32.const 1) (i32.const 88))
    (global.set $arr (array.new_data $bytes $passive (i32.const 0) (i32.const 5)))
  )
  ;; Past every segment.
  (func $test1
    (i32.store8 $a (i32.const 100) (i32.const 7))
  )
  ;; The second item stores and then fails, so none of it is evalled.
  (func $fail
    (drop (i32.const 0))
    (block
      (i32.store8 $a (i32.const 200) (i32.const 9))
      (call $outside)
    )
  )
  ;; 7 if `fail`'s store is gone, 16 if it leaked.
  (func $test2
    (global.set $read
      (i32.add (i32.load8_u $a (i32.const 100)) (i32.load8_u $a (i32.const 200))))
  )
)
