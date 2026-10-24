(module
  (type $B (array (mut i8)))
  (type $I (array (mut i32)))
  (global $b (mut (ref null $B)) (ref.null $B))
  (global $i (mut (ref null $I)) (ref.null $I))
  ;; A string's bytes, which are smaller as data than as one i32.const each,
  ;; and three small numbers, which are not.
  (func $make (export "make")
    (global.set $b (array.new $B (i32.const 104) (i32.const 64)))
    (global.set $i
      (array.new_fixed $I 3 (i32.const 1) (i32.const 2) (i32.const 3)))
  )
  (func $read (export "read") (result i32)
    (i32.add
      (array.get_u $B (global.get $b) (i32.const 63))
      (array.get $I (global.get $i) (i32.const 2)))
  )
)
