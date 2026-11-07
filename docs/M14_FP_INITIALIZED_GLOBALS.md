# M14 — byte-exact IEEE F32/F64 file-scope global definitions

Status: implementation and pinned native CI under review. This is a bounded
storage step, **not** general C initializer or FP lvalue support.

## Source contract

The EDG bridge emits event 60 only for an **explicit, native static
constant initializer** of a file-scope standard C `float`/`double`
variable. The initializer must be an EDG `ck_float` constant, type
`tk_float`, layout 4/4 or 8/8 on the pinned Linux x86-64 target with
native IEEE binary32/binary64. The event contains the variable symbol,
exact type identity, 32/64-bit image width and raw IEEE image. These
are real bytes from the EDG constant, not parsing C source spelling or
evaluating unverified constant expressions in Rust.

The owned Rust IL stores and validates the event with exact width and
bounds. The FP reference checks the native declared source type,
linkage/storage class, absence of TLS and local-static provenance, and
uniqueness of the image. A visible explicit image becomes a real
aligned ELF `.data` definition with its exact bytes; ordinary
tentative/uninitialized definitions remain aligned `.bss`. A
source `extern` declaration with an initializer is a definition,
not an imported undefined symbol. Externally visible source
definitions are included in the ELF even if no local function reads
them. Global loads and stores remain true ordinary memory effects.

## Validation

Pure Rust checks validate the wire event, reject bad widths and record
length, and require exact data image sizes, defined ownership and
unique ELF symbols. The native differential fixture tests signed
zero, binary32 ULP values, file-static storage, externally visible
globals without local references, independently linked GCC reads and
writes, genuine BSS zeros and observable generated stores. GCC and
Cranelift objects run with the *same* separate GCC driver.

Negative C fixtures must fail without partial ELF for qualified
volatile globals, thread-local initialized data, and unmodeled
integer global access in the FP source path. Existing
`m14_fp_zero_owned_globals_bad_init.c` becomes a positive
supported example by design, rather than a false failure gate.

## Limitations

Only file-scope explicit standard IEEE scalar constant initializers
are accepted. Dynamic initializers, aggregates, relocations, TLS,
volatile/atomic, pointer and address-taken local memory, and the full
source storage/lifetime model still fail closed. This does not
introduce a backend-neutral semantic projection or support arbitrary
C expressions and the floating environment; these remain on
[M14 issue #60](https://github.com/bingordo/edgcpp-rust/issues/60).
