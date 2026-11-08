(module
  (type $B (array (mut i8)))
  (global $a (mut (ref null $B)) (ref.null $B))
  (global $b (mut (ref null $B)) (ref.null $B))
  ;; Two strings' bytes, each smaller as data than as constants.
  (func $make (export "make")
    (global.set $a (array.new $B (i32.const 97) (i32.const 40)))
    (global.set $b (array.new $B (i32.const 98) (i32.const 50)))
    (array.set $B (global.get $b) (i32.const 49) (i32.const 99))
  )
  (func $read (export "read") (result i32)
    (i32.add
      (i32.add
        (array.get_u $B (global.get $a) (i32.const 39))
        (i32.mul (array.get_u $B (global.get $b) (i32.const 0)) (i32.const 1000)))
      (i32.mul (array.get_u $B (global.get $b) (i32.const 49)) (i32.const 1000000)))
  )
)
