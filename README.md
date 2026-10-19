# edgcpp-rust

A lightweight Rust wrapper for the EDG C/C++ front end on **Linux x86-64**.
It invokes the callable front-end entry point from `libedgcpfe.a` and captures
a lifetime-safe subset of EDG's intermediate language (IL) before EDG frees it.

**Status:** initial integration. The current IL API snapshots file-scope routine
names, whether a routine has a definition, and file-scope variable names.
It is not yet a complete EDG IL binding, AST, or stable serialized IL format.

## Build prerequisites

- A matching checkout of [bingundu/edgcpp](https://github.com/bingundu/edgcpp)
  on branch `ci/x86_64-callable-release`, with its callable-library CMake option.
- Its generated `libedgcpfe.a` release archive and the corresponding CMake
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

The bridge defines `edg::back_end()` in place of the archive's C-code-generating
back end. EDG invokes it after front-end processing, while IL is still alive.
It copies the relevant data through a C callback. Rust keeps only owned data.

This initial interface does **not** expose a full C++ AST or function-body IL.
That requires extending the IL visitor within the C++ bridge. The underlying
EDG archive may change implementation and macro configurations; build the
bridge and archive from the **same commit and configuration**.

## Known limitations

- The callable EDG library is not thread-safe; `compile()` serializes calls
  within this process.
- EDG manages its own diagnostics and source-file options.
- File-scope snapshots do not recurse into namespaces/classes or function
  bodies yet.
- The custom back-end symbol relies on static archive resolution; link tests
  against the exact archive version are necessary.
- Exceptions do not cross the C ABI.

License: Apache-2.0 WITH LLVM-exception, as in upstream EDG.
