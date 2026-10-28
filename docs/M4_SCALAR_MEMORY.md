# M4 implementation checkpoint: scalar memory and external data

Status: **test inputs committed; backend support not yet implemented**.

## Acceptance test

Compile `tests/fixtures/m4_external_globals.c` with the EDG/Cranelift
reference code generator, compile `m4_external_driver.c` with GCC,
link the resulting objects via GCC, and compare its output to a wholly
GCC-built control. The fixtures exercise four external scalar types,
read/modify/write, narrowing, boolean canonicalization, and both initial
data and subsequent writes. No call lowering is required because the
external driver calls the generated function.

Reference commands:

```sh
gcc -std=c11 -O2 -c tests/fixtures/m4_external_driver.c -o /tmp/m4_driver.o
gcc -std=c11 -O2 -c tests/fixtures/m4_external_globals.c -o /tmp/m4_gcc.o
gcc /tmp/m4_driver.o /tmp/m4_gcc.o -o /tmp/m4_gcc
/tmp/m4_gcc
# Once supported, generate /tmp/m4_edg.o using examples/codegen:
gcc /tmp/m4_driver.o /tmp/m4_edg.o -o /tmp/m4_edg
cmp <(/tmp/m4_gcc) <(/tmp/m4_edg)
```

## Implementation sequence

1. Export stable native metadata for global scalar declarations: symbol,
   C type/qualifiers, storage/linkage and definition versus external reference.
   Do not invent external names from source spelling for internal-linkage objects.
2. Add a **typed lvalue** to the shared codegen representation. Reads must
   lower to loads; writes and compound expressions must lower to stores.
   Preserve side-effect sequencing introduced in M3.
3. Declare imported data with `ObjectModule::declare_data` and
   `Linkage::Import`; obtain a function-relative global value using the
   module data-reference API and emit appropriately typed loads and stores.
   Let the object backend own ELF relocation generation.
4. Handle actual definitions separately: zero-initialized BSS versus
   initialized data, correct declared linkage/visibility and width.
5. Address-taken locals require stack slots with explicit escape checks;
   plain non-address-taken locals should remain SSA values.
6. Extend GCC differential checks for signed/unsigned narrow globals,
   `_Bool`, multiple writes, internal versus external linkage, zero
   initialization, and negative unsupported cases.

## Safety gates

Never treat global reads/writes as SSA locals. Reject volatile/atomic objects
until required ordering and access semantics are modeled. Reject address-taken,
unknown-width, TLS, aggregate, tentative/common-definition and aliased lvalues
unless implemented explicitly. Never produce an object file on a rejected
translation unit. M4 is only complete when the generated ELF links and
executes with a GCC-owned global-data driver.

## Backend relocation checkpoint (unverified)

The optional codegen module now includes `src/codegen/memory.rs` with a
standalone ELF object builder that imports one writable `u32` global and emits
an increment using a Cranelift global-value address, typed load, and store.
`examples/m4_data_probe.rs` writes the object; `tools/m4_data_probe.sh`
links it with a GCC-built owner of `m4_counter` and checks wraparound and
subsequent mutation. This validates the relocation strategy separately from
EDG IL decoding *once executed*. Neither the new Rust module nor this shell
probe has been verified by CI in this checkpoint; do not count it as passing.

The main M4 fixture `m4_external_globals.c` remains **unsupported** by
`emit_edg_object()` until variable references are resolved to typed global
lvalues and imported data are registered in the same `ObjectModule` that
emits the routines. At that point, run the full differential harness and
negative tests before marking M4 complete.
