(module
  (memory (export "memory") 2)
  (func (export "add") (param i32 i32) (result i32)
    (i32.store (i32.const 0) (local.get 0))
    (i32.add (i32.load (i32.const 0)) (local.get 1))))
