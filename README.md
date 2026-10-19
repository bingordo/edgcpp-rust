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
operator kind when applicable, the basic result type kind, and optional\nvariable/routine/field references. Other expression
kinds remain opaque. Full type graphs, some non-operation expression children,
and CFG edges are not yet represented. No EDG pointers escape the callback.

## Indexed IL graph

`edgcpp::graph::Graph::from_il(&compilation.il)` converts the owned
snapshot into dense IDs, explicit statement-child edges, statement-to-root-
expression edges and ordered expression operands. `StatementId` and
`ExpressionId` identify nodes within one graph. Invalid references and
duplicate paths return `GraphError`, rather than silently dropping nodes.
The legacy `Il` records remain available unchanged.

```rust
let graph = edgcpp::graph::Graph::from_il(&result.il)?;
for statement in &graph.statements {
    if let Some(root) = statement.expression {
        println!("{:?}", graph.expression(root));
    }
}
```

The graph is a structurally indexed **partial EDG snapshot**, not yet a
complete typed compiler IR. EDG enum values are still native discriminants.

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
\n## Type layout and constants\n\nExpressions expose optional EDG target type size/alignment (bytes) and the\nconstant representation kind for constant expressions. Layout is absent\nfor incomplete or zero-sized types. Constant values and semantic type graph\nedges are not yet serialized; consumers must not confuse kind tags with values.\n\nSigned integer constants representable as i64 are copied as `Expression::constant_integer`; out-of-range integer values are left absent. This is not yet a general arbitrary-precision constant representation.\n
## Direct type relationships (prototype)

Expression records now include `related_type_role` and `related_type_kind`
for pointer pointee (1), array element (2), or function return (3) types.
These are **one-hop kind summaries**, not identity-preserving type graph edges;
the actual referenced type nodes, parameters, qualifiers and typedef chains
will require a separate owned type table.

## Identity-preserving type snapshots

`Il::types` now contains `TypeDef` records keyed by `TypeId`. EDG type
addresses are mapped to per-compilation numeric IDs; the callback exports
only those IDs. Pointer, array and routine types link to pointee, element,
and return types respectively, allowing recursive type graphs to be represented.
Each expression carries an optional `type_id` referring into the same table.
Type parameters, typedef chains, cv qualifiers and record members are not yet
modeled. IDs are valid only within the containing compilation.

The indexed `Graph::from_il()` validates unique, nonzero type IDs, checks that direct type edges resolve, and rejects expressions referring to missing types. `Graph::type_def(TypeId)` resolves an owned type node. Cycles are permitted (recursive types).\n
## Function parameter edges

Function types now export their parameter types in declaration order as
`TypeParameter { owner, position, ty }` records. The indexed graph validates
both referenced type IDs and rejects duplicate parameter positions. Query
`Graph::parameters(TypeId)` for ordered parameter edges. Array dimensions and
full qualifiers are not yet represented.

Fixed-size array types now export element counts as `ArrayBound` records. Bounds for variable-length, template-dependent, and incomplete arrays remain absent. `Graph::array_bound(TypeId)` resolves a known fixed bound.\n\nRecord fields include byte offsets, bit offsets within the byte, and bit-field widths from the target EDG layout. Zero bit width denotes an ordinary field in this snapshot.\n\nTyperef types now have an underlying type edge (relation role 4). The bridge also exports EDG pointer reference/rvalue-reference flags and the raw qualifier mask for typeref nodes as `TypeDetail`. The qualifier mask is the native EDG encoding, not a portable `const`/`volatile` abstraction.\n
Function routine types additionally export prototype and ellipsis flags in `FunctionSignature`. The graph validates unique signature records and exposes `signature(TypeId)`. The C++ fixture includes a variadic declaration for integration coverage.

## Symbol identities (prototype)

The bridge now assigns compilation-local `SymbolId` numbers using EDG routine,
variable and field node identity. `Il::symbols` owns the symbol names and kinds;
`Expression::symbol_id` identifies the referenced native declaration without
relying on display-name equality. `Graph::symbol(SymbolId)` resolves these
references; graph construction rejects duplicate or dangling symbol IDs.
These IDs are **not** stable across compilations. Type/namespace declarations
and expression forms outside the current visitor still need symbol capture.

## Typed in-memory callback ABI

The native bridge now calls Rust with a borrowed `edgcpp_field[]` for each
event, rather than formatting a tab-separated string. Each field carries
either a pointer and byte length (UTF-8), an unsigned 64-bit integer, a
signed 64-bit integer, or an explicit absent marker. The callback copies
strings into the owned Rust IL before returning. Numeric fields never pass
through decimal formatting or parsing.

The C and Rust structures have matching field order and native C layout.
The pointer and length are valid **only during the synchronous callback**.
No EDG pointers are stored in the returned `Il` or `Graph`.

The wire contract currently includes 13 event kinds and supports arbitrary
UTF-8 in string fields, including tabs and embedded NUL bytes. This
protocol is private to the in-process bridge and is not a stable file
format or public extension ABI.

Variable declarations now preserve EDG's native `init_kind`, explicit-initializer flag and uninitialized flag, attached to the declaration's `SymbolId`. This is initial classification metadata, not yet a full initializer expression or aggregate value tree.
\nDynamic variable initializers now also report their native EDG `dik_*` category, and where applicable the root expression kind and operator. This is not yet a copied full expression tree.\n
Dynamic scalar expression initializers now also export operation operand trees
through typed event 17, retaining path/parent, EDG expression kind, operator,
type ID and referenced symbol ID. The graph checks ancestry and dangling
references. Non-operation expression subtrees and constructor/aggregate
initializers are still incomplete.

Expression traversal now follows `enk_object_lifetime` wrapper nodes and
expression-form `enk_sizeof`, `enk_alignof` and `enk_datasizeof` child
expressions, in addition to ordinary operation operand lists. Type-form
size/alignment operations have no expression child. Both function-body and
dynamic-initializer visitors use this rule. The overall snapshot remains
partial for other EDG expression variants.
