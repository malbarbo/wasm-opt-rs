(module
  (type $T (struct (field i32)))
  (import "env" "import" (func $import))
  (global $g (mut i32) (i32.const 0))
  ;; Evalled up to the import: the object in $l is serialized for the rest.
  (func $f (export "f")
    (local $l (ref null $T))
    (local.set $l (struct.new $T (i32.const 42)))
    (call $import)
    (global.set $g (struct.get $T 0 (local.get $l)))
  )
)
