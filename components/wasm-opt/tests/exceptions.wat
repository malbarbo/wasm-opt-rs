;; A module with an exception in it, which `Precompute` tries to evaluate: it
;; runs the interpreter over `$thrower`'s body, `visitThrow` raises a
;; `NonconstantException`, and the pass catches it and moves on. Built with the
;; wrong flags that exception finds no handler and the process aborts, so
;; optimising this at all is the assertion.
(module
  (tag $e (param i32))
  (func $thrower (export "thrower") (result i32)
    (block $b (result i32)
      (throw $e (i32.const 1))
    )
  )
)
