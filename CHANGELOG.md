## 0.132.0

- Updated Binaryen to [version 132](https://github.com/WebAssembly/binaryen/blob/main/CHANGELOG.md#v132),
  from version 116.
- **Building now requires a C++20 compiler**, as required by Binaryen.
- New wasm features: `stack-switching`, `shared-everything`, `fp16`,
  `bulk-memory-opt`, `call-indirect-overlong`, `custom-descriptors`,
  `acquire-release-atomics`, `custom-page-sizes`, `multibyte`,
  `wide-arithmetic`, `compact-imports` and `relaxed-atomics`.
- `Feature::MultiMemory` is now spelled `multimemory` (was `multi-memory`),
  following Binaryen.
- Added the passes registered by Binaryen 132, and removed the passes it no
  longer registers: `generate-stack-ir`, `optimize-stack-ir`, `print-stack-ir`,
  `legalize-js-interface-minimally`, `jspi`, `mod-asyncify-never-unwind` and
  `mod-asyncify-always-and-only-unwind`.

  StackIR is no longer produced by passes; it is generated while writing the
  module, controlled by the new `PassOptions::allow_stack_ir` option
  (`--no-stack-ir` on the command line), and enabled by default at optimize
  level 2 or shrink level 1, as in the `wasm-opt` command line tool.
- Added `InliningOptions::max_combined_binary_size`
  (`--inline-max-combined-binary-size`).
- Added `ReaderOptions::preserve_type_order` (`--preserve-type-order`).
  As `wasm-opt` does, the type order of the input module is now discarded
  unless this is set.
- A pass argument named after a pass, e.g. `--pass-arg extract-function@NAME`,
  is now attached to that pass instance, as Binaryen requires.
- Added `CtorEvalOptions`, an API for Binaryen's `wasm-ctor-eval` tool, which
  executes exported functions at compile time and applies their effects to the
  module. It supports the same options as the command line tool, which is not
  itself installed by this crate.
- Fixed an abort when optimizing any module containing an exception. The C++ is
  now built with `-fno-rtti` and `-fno-omit-frame-pointer`, which is how
  Binaryen's own CMake builds it: at `-O3` with RTTI, gcc 14 leaves the
  `NonconstantException` that `Precompute` raises and catches with no handler,
  and the process aborts.
- **Building now requires `patch`.** `components/wasm-opt-sys/patches/` holds
  changes to Binaryen that are not upstream, and the build script applies them
  to copies of the sources under `OUT_DIR`. The submodule stays pointed at
  upstream and is never written to.
- Ctor evaluation no longer gives up on a module whose memory cannot be
  flattened. Binaryen refuses to flatten a module with a passive data segment,
  or one where an expression names a segment -- `array.new_data`, which is how
  a GC language materialises a constant array -- so for those it used to
  evaluate nothing at all. Now only a ctor that *writes* linear memory fails,
  and the rest are evaluated. A module holding a `data.drop` is still refused:
  that effect never reaches the external interface, so it cannot be caught one
  ctor at a time.
- Added `CtorEvalOptions::max_steps`, which bounds the instructions an
  evaluation may execute. Binaryen's evaluator cannot time out, so without this
  a ctor that does not terminate does not either. Running over it fails that
  ctor, the way reading an import does.

## 0.116.1

- [Fixed build on wasm32-wasmi](https://github.com/brson/wasm-opt-rs/pull/165).

## 0.116.0

- The "dwarf" feature is enabled by default.
  This feature activates DWARF-related passes in Binaryen.
  It builds C++ code from the LLVM project.
  Disable it to avoid linkage errors with LLVM.
- [Binaryen changelog for 116](https://github.com/WebAssembly/binaryen/blob/main/CHANGELOG.md#v116).
- [Binaryen changelog for 115](https://github.com/WebAssembly/binaryen/blob/main/CHANGELOG.md#v115).

## 0.114.2

- Added the "dwarf" cargo feature, disabled by default.
- [Fixed link-time regression in 0.114.1](https://github.com/brson/wasm-opt-rs/issues/154)

  0.114.1 added missing DWARF passes. Unfortunately these passes, taken from
  LLVM code, cause duplicate symbol linker errors when linked into a program
  that links to LLVM. For now we have put the compilation of these passes under
  the "dwarf" flag and made them non-default. In a future release "dwarf" will
  be a default feature. Version 0.114.1 has been yanked.

## 0.114.1

- [Compiled missing DWARF passes](https://github.com/brson/wasm-opt-rs/pull/151).

## 0.114.0

- Upgraded to Binaryen 114.

## 0.113.0

- Upgraded to Binaryen 113.

## 0.112.0

- Upgraded to Binaryen 112.
- [Fixed the displayed version number in `wasm-opt` bin](https://github.com/brson/wasm-opt-rs/pull/133)

## 0.111.0

- Upgraded to Binaryen 111.
- [Fixed bugs in feature selection via the API](https://github.com/brson/wasm-opt-rs/issues/123).
- Binaryen now enables the `SignExt` and `MutableGlobals` features by default,
  which are also enabled in the LLVM backend.
  In the future Binaryen will align its default feature selection with the LLVM backend.
  To get the same feature selection as Binaryen 110, call

  ```rust
      opts.mvp_features_only()
  ```
- The `TypedFunctionReferences` feature has been removed. The CLI still accepts
  `--enable-typed-function-references` and `--disabled-type-function-references`
  as no-ops. The `integration` module does not accept these command line arguments.

## 0.110.2

- [Backported Binaryen patch to remove empty memories sections from output](https://github.com/brson/wasm-opt-rs/pull/111).

## 0.110.1

- [Removed Binaryen test suite from published source](https://github.com/brson/wasm-opt-rs/issues/98).
- [Removed duplicate Binaryen source from `wasm-opt-cxx-sys`](https://github.com/brson/wasm-opt-rs/pull/90).
- [Fixed exception handling in bin](https://github.com/brson/wasm-opt-rs/issues/89).

## 0.110.0

- Initial release
