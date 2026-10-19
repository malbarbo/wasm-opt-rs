(module
  (import "lib" "add" (func $add (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "run") (result i32)
    (call $add (i32.const 2) (i32.const 3))))
