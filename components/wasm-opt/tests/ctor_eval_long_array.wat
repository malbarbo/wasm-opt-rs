(module
  (type $B (array (mut i8)))
  (type $S (struct (field i32)))
  (type $R (array (mut (ref null $S))))
  ;; An exported memory, which therefore stays, and which will not flatten
  ;; once evaluation adds a passive segment: a GC program's will not.
  (memory (export "memory") 1 1)
  (global $b (mut (ref null $B)) (ref.null $B))
  (global $r (mut (ref null $R)) (ref.null $R))
  ;; Two arrays of 10 001 values, one more than an array.new_fixed may hold.
  (func $make (export "make")
    (local $a (ref $B))
    (local.set $a (array.new_default $B (i32.const 10001)))
    (array.set $B (local.get $a) (i32.const 10000) (i32.const 200))
    (global.set $b (local.get $a))
    (global.set $r
      (array.new $R (struct.new $S (i32.const 7)) (i32.const 10001)))
  )
  (global $sum (mut i32) (i32.const 0))
  (func $again (export "again")
    (global.set $sum (call $read))
  )
  (func $sum (export "sum") (result i32)
    (global.get $sum)
  )
  (func $read (export "read") (result i32)
    (i32.add
      (array.get_u $B (global.get $b) (i32.const 10000))
      (struct.get $S 0
        (array.get $R (global.get $r) (i32.const 10000))))
  )
)
