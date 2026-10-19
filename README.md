# edgcpp-rust

A lightweight Rust wrapper for the EDG C/C++ front end on **Linux x86-64**.
It invokes the callable front-end entry point from `libedgcpfe.a` and captures
a lifetime-safe subset of EDG's intermediate language (IL) before EDG frees it.

**Status:** initial integration. The current IL API snapshots namespace-qualified routine names, definition flags, variables, named types,
and namespaces.
It is not yet a complete EDG IL binding, AST, or stable serialized IL format.

## Build prerequisites

- A matching checkout of [bingundu/edgcpp](https://github.com/bingundu/edgcpp)
  on branch `ci/x86_64-callable-release`, with its callable-library CMake option.
- Its generated **PIC-enabled** `libedgcpfe.a` release archive and the corresponding CMake
  `compile_commands.json`.
- Rust/Cargo, Python 3, g++, GNU ar, and the same C++ toolchain configuration
  used for the EDG archive.

Configure and build the EDG library (from the EDG checkout):

```sh
cmake --preset linux-gcc-release \
  -DEDG_BUILD_LIB=OFF \
  -DEDG_BUILD_CALLABLE_LIBRARY=ON \
  -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
cmake --build build/gcc-release --target edgcpfe --parallel 2
```

Then, from this repository:

```sh
export EDGCPP_SOURCE_DIR=/absolute/path/to/edgcpp
export EDGCPP_COMPILE_COMMANDS="$EDGCPP_SOURCE_DIR/build/gcc-release/compile_commands.json"
export EDGCPP_LIB_DIR="$EDGCPP_SOURCE_DIR/build/gcc-release/lib"
cargo build --release
cargo run --example parse -- -c --c++20 /absolute/path/to/example.cpp
```

The published library archive alone is **not enough** to compile the C++
bridge: EDG's IL structs vary according to compile-time macro definitions.
The bridge builds with the exact compiler command recorded for `edgcpfe`
in CMake's compilation database, guaranteeing the same IL struct layout.
This avoids hardcoding potentially incorrect C++ struct definitions in Rust.

## API

```rust
let result = edgcpp::compile(&["edgcpfe", "--c++20", "example.cpp"])?;
assert_eq!(result.status, 0);
for entity in result.il.entities {
    println!("{entity:?}");
}
```

You can also inspect an owned namespace hierarchy with:

```rust
let result = edgcpp::compile(&["edgcpfe", "--c++20", "example.cpp"])?;
let namespaces = result.il.scope_tree();
if let Some(scope) = namespaces.namespace(&["outer", "inner"]) {
    for declaration in &scope.declarations {
        println!("{declaration:?}");
    }
}
```

## Function statements (prototype)

`Compilation::il.statements` now contains a bounded snapshot of function-body
statements. Each record has a qualified function name, an EDG statement-kind
discriminant, a stable statement path, and an optional parent path. The bridge
recurses into blocks, if/else branches, while loops, and for-loop bodies.
Unknown statement kinds are retained as opaque records.

`Compilation::il.expressions` snapshots the primary expression on a statement,
including ordered operands of EDG operation nodes. Records retain function,
statement path, expression path, parent expression path, EDG expression kind,
operator kind when applicable, and the basic result type kind. Other expression
kinds remain opaque. Full type graphs, some non-operation expression children,
and CFG edges are not yet represented. No EDG pointers escape the callback.

The bridge defines `edg::back_end()` in place of the archive's C-code-generating
back end. EDG invokes it after front-end processing, while IL is still alive.
It copies the relevant data through a C callback. Rust keeps only owned data.

This initial interface does **not** expose a complete C++ AST or full function-body IL.
That requires extending the IL visitor within the C++ bridge. The underlying
EDG archive may change implementation and macro configurations; build the
bridge and archive from the **same commit and configuration**.

## Known limitations

- The callable EDG library is not thread-safe; `compile()` serializes calls
  within this process.
- EDG manages its own diagnostics and source-file options.
- The declaration snapshot recurses into named namespaces, but not class members;
  function bodies have a partial statement/expression snapshot.
- The custom back-end symbol relies on static archive resolution; link tests
  against the exact archive version are necessary.
- Exceptions do not cross the C ABI.

License: Apache-2.0 WITH LLVM-exception, as in upstream EDG.
