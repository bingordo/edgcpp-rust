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

## C++ inheritance (prototype)

The native bridge captures direct base-class edges for C++ class and struct
types. `Compilation::il.base_classes` contains `BaseClass` records with
`derived` and `base` compilation-local `TypeId` values, declaration-order
`position`, and `is_virtual`. The graph validates the references and
provides `Graph::bases(derived_type_id)` in declaration order.

```rust
let graph = edgcpp::graph::Graph::from_il(&result.il)?;
for edge in graph.bases(derived_type_id) {
    println!("base {:?}, virtual: {}", edge.base, edge.is_virtual);
}
```

Direct base access specifiers are available through
`Compilation::il.base_class_access` and `Graph::base_access(derived, position)`.
Their `access` value is a native EDG enum, or `None` when no direct
derivation path is retained; `pack_expansion` indicates a variadic base
specifier. Only **direct** inheritance edges are exported. Transitive
inheritance, base-subobject layout and complete C++ access checking are not
yet modeled.

## Declared symbol types

The native bridge exports compilation-local `SymbolType` edges linking
routine and scope variable `SymbolId` values to EDG `TypeId` values.
`Graph::symbol_type(symbol_id)` resolves each edge; `Graph::signature(type_id)`
and `Graph::parameters(type_id)` then expose the function prototype when
the type is a routine. Type metadata is not guaranteed for symbols that
were captured only as references from an expression; a missing edge is
represented by `None`.

## Class member functions

The bridge traverses EDG class/struct/union scopes, including nested
classes, rather than only file and namespace scopes. Member routines are
included in the routine snapshot and connected to their declaring class
through `Compilation::il.member_functions`. In the graph:

```rust
for member in graph.member_functions(class_type_id) {
    let routine = graph.symbol(member.routine);
    println!("{routine:?}");
}
let owner = graph.member_function_owner(routine_symbol_id);
```

Member-function ownership includes static and nonstatic member routines.
EDG routine classification and dispatch flags are also available through
`Graph::routine_attributes(routine_symbol_id)`: native `special_kind`,
virtual/pure-virtual/override/final flags, override relationships, inline and
constexpr flags, explicit-constructor and definition flags, and whether an
implicit `this` parameter is present. These describe EDG's processed IL and
do not reconstruct the original source spelling. Access control and overload
grouping are not yet exported.

## Constructor and destructor subobject initialization

For defined C++ constructors and destructors, the native bridge reads EDG's
function-scope `constructor_inits` list. Each record is owned by the
compilation-local routine `SymbolId` and contains a dense execution-order
position, EDG initializer kind, target base `TypeId` or field `SymbolId`,
and flags for compiler-generated, braced, default member and pack-expansion
initialization.

```rust
let graph = edgcpp::graph::Graph::from_il(&result.il)?;
for init in graph.constructor_initializers(constructor_symbol_id) {
    println!("kind={}, base={:?}, field={:?}",
             init.kind, init.target_type, init.target_field);
}
```

When EDG represents an initializer as a dynamic expression (or supplies
a source-expression fallback), the bridge now captures a separate owned
`ConstructorInitializerExpression` tree. Paths are local to the
`(routine SymbolId, initializer position)` pair. Use
`Graph::constructor_initializer_root(routine, position)`,
`Graph::constructor_initializer_expression(routine, position, path)`, and
`Graph::constructor_initializer_operands(node)` for navigation. Expression
trees are optional: not every EDG initializer form exposes an expression.
The graph checks parent references, type/symbol references, root presence
and dense operand positions.

Constructor/destructor classification is retained as native EDG kinds, and
no source spelling is reconstructed. Records are available only for function
scopes traversed by the native bridge. As with the rest of the native IL,
IDs are not stable across compilations.

## Source locations and runtime object lifetimes

When EDG retains a source position, the bridge resolves its compilation
sequence number through EDG's own source-file mapping, preserving the
diagnostic filename, logical line (including `#line`), column and original
sequence number. No source-code tokenization or line-number guessing is
involved. Synthetic entities and operations without mapped positions simply
have no source-location record.

- `Graph::symbol_source(symbol_id)` identifies the declaration location for
  a routine, variable or field.
- `Graph::statement_source(function, statement_path)` identifies the
  source position attached to a statement.
- `Graph::expression_source(function, statement_path, role, expression_path)`
  identifies an expression position (role 0 is the primary tree; roles 1–4
  are the supplemental control/initializer trees).

Source positions may identify macro invocation locations rather than macro
body spelling; the bridge does not yet export macro expansion stacks or
complete end-of-range source spans. EDG's generated statements often have
unknown positions.

**Runtime object lifetimes are distinct from lexical scopes.** The bridge
preserves EDG's lifetime-region hierarchy as `ObjectLifetime` records with
dense IDs per routine, native kind, and parent ID. `ScopeLifetime` records
link an EDG lexical scope to its runtime region if both exist. Dynamic
initialization statements may additionally carry `InitializationLifetime`
records with an associated lifetime and/or destructor routine.
`LifetimeDestruction` records preserve the order of cleanup entries in
the EDG destruction list (the first entry is destroyed first).

```rust
if let Some(region) = graph.scope_lifetime(routine_id, scope_id) {
    for cleanup in graph.lifetime_destructions(routine_id, region.id) {
        println!("destruction position {}", cleanup.position);
    }
}
```

The queries `object_lifetime(routine, id)`,
`initialization_lifetime(function, statement_path)` and
`lifetime_destructions(routine, id)` allow passes to reason about object
cleanup regions without exposing EDG's native pointers. The lifetime
hierarchy is **not** a computed exact instruction-level live interval:
constructs eliminated by lowering, objects with no required destruction,
explicit `new`/`delete`, and independent memory regions may not appear.
Missing metadata must not be interpreted as proof that no destruction occurs.

## Declaration sites and initializer execution

EDG's lexical scopes identify which function or block *owns* a variable;
they do not necessarily identify the statement where it is declared or
initialized. The bridge now captures `stmk_decl` variable lists and VLA
declarations as ordered `StatementDeclaration` records, each with the
function, statement path, list position, `SymbolId`, `TypeId`, and VLA
marker. Non-variable declarations (types, static assertions, etc.) are
not included.

```rust
for declaration in graph.declarations_at("f", "0.block.1") {
    println!("{:?}: {:?}", declaration.symbol, declaration.ty);
}
for site in graph.declaration_sites(variable_symbol_id) {
    println!("declared at {}", site.statement_path);
}
```

Dynamic initializations in `stmk_init`, return-value initialization, and
GNU statement-expression result initialization are represented separately
as `StatementInitialization` records. They preserve the variable symbol
if present, the native EDG dynamic-init kind, and two initialization flags.
The role identifies the statement variant: 0 for `stmk_init`, 1 for
return, and 2 for statement-expression result. For
`dik_expression`/`dik_class_result_via_ctor`, the initializer expression
tree is available through `Graph::statement_initializer_expression` and
`Graph::control_operands`, using supplemental expression role 4.

`Graph::statement_initialization(function, path)` resolves an execution
site. `Graph::variable_initialization_sites(symbol)` enumerates its
captured executions; these are not assumed to be the same as declaration
sites. The graph validates symbols, types, statement references, duplicate
positions, dense declaration indices, and expression-tree ownership.
Lowering may eliminate an explicit `stmk_decl`, synthesize an initialization
or represent scalar initialization directly as operations in the function
body. An absent record therefore does **not** imply that a C++ source
variable is uninitialized.

## Routine identity and statement-body ownership

The bridge emits an authoritative `RoutineIdentity { routine, function }`
link for each exported routine. `function` is the key used by statement
and expression paths, and can differ from the symbol's display name when
EDG first encounters a reference using a full overloaded signature or a
destructor variant. Use `Graph::routine_function(SymbolId)` and
`Graph::routine_for_function(&str)` rather than comparing
`Symbol.name` to `Statement.function`. Graph validation also uses this
mapping for local scope and runtime lifetime associations.

## Function parameters and lexical scope trees

For function definitions, the native bridge snapshots the EDG function
scope, its explicit parameter-variable list and its implicit C++ `this`
variable. `Compilation::il.function_parameters` links each variable
`SymbolId` to its owning routine and `TypeId`. Explicit parameters have
zero-based positions; an implicit `this` has `position = None`.
This is separate from the existing routine **type signature**:
`Graph::parameters(type_id)` describes declared parameter types, whereas
`Graph::function_parameters(routine_symbol_id)` describes actual parameter
variables in the function body. `Graph::implicit_this(routine_symbol_id)`
returns the implicit object parameter when present.

Function and nested block scopes are captured as `LocalScope` records,
using dense scope IDs starting at zero per routine; each has a parent
scope ID, EDG scope kind and optional associated statement path. Declarations
on the scope's nonstatic and static variable lists become `LocalVariable`
records, with the variable symbol and its type, storage class, and flags
for compiler generation, static storage, structured bindings and catch
handler parameters. Scope IDs are compilation-local and do not escape
as EDG pointers. Repeated names (including shadowed locals) have distinct
`SymbolId` values.

```rust
let graph = edgcpp::graph::Graph::from_il(&result.il)?;
for parameter in graph.function_parameters(routine_id) {
    println!("parameter {:?}: {:?}", parameter.symbol, parameter.ty);
}
if let Some(object) = graph.implicit_this(routine_id) {
    println!("implicit this: {:?}", object.symbol);
}
for scope in graph.local_scopes(routine_id) {
    for local in graph.locals_in_scope(routine_id, scope.id) {
        println!("local {:?}: {:?}", local.symbol, local.ty);
    }
}
```

`Graph::local_scope(routine, id)` and `Graph::local_variable_scope(symbol)`
provide parent/declaration lookups. As with other snapshots, EDG may have
lowered away some source declarations and generated temporary variables.
Scopes without an associated statement are retained. Lexical scopes alone
do not define exact instruction-level live intervals. Source locations and
runtime object-lifetime regions are available through the separate source
and lifetime graph APIs described above.

**Hoisted static locals:** EDG can allocate function-local static and
thread-storage variables in the file-scope memory region. Their
`source_corresp.enclosing_routine` still identifies the owning function, but
their exact lexical block is not retained as a safe scope pointer. These
are therefore exported separately in `Compilation::il.static_local_owners`
and `Graph::static_locals(routine)` / `Graph::static_local_owner(symbol)`.
No scope ID is invented for such declarations. They may also appear in
`local_variables` when EDG does retain their scope-list entry.

## Structured control flow and loop expressions

The native bridge now follows more of EDG's structured statement tree:
`if`/`else`, `if constexpr`, C++23 consteval-if, while, do/while,
`for` initializer and body, range-based `for` initializer and body,
`switch` bodies, and C++ `try` plus individual catch-clause bodies.
These remain **EDG IL** statements: constructs erased by lowering or
rewritten into other kinds are represented in their lowered form.

Child statement paths retain the parent path and a named edge, such as
`0.block.1.body.0`, `0.block.2.init.0`, or
`0.block.3.catch1.0`. A dependent statement is traversed as a single
node rather than accidentally following its enclosing sibling list.

`Compilation::il.statement_attributes` retains flags for
compiler-generated statements, initialization guards, fallthrough,
`[[likely]]`/`[[unlikely]]`, and lowering-generated/boilerplate statements.
`Graph::statement_attributes(function, statement_path)` finds the
metadata for a particular statement.

`goto` and label statements receive `StatementLabel` records whose
`SymbolId` refers to a native label symbol (kind 4). The graph exposes
`statement_label(statement_id)`, `label_target(label_symbol_id)`, and
`goto_target(goto_statement_id)`. A target may be absent when the matching
label definition was not captured; no raw EDG pointer escapes the bridge.

The primary expression in `a_statement::expr` remains in
`Compilation::il.expressions`. Additional control-flow expressions
are independently owned in `Compilation::il.control_expressions`,
keyed by function, statement path and EDG role:

- role 1: `for` loop increment;
- role 2: range-`for` iterator comparison;
- role 3: range-`for` iterator increment.

Use `Graph::control_expression_root(function, statement, role)`,
`Graph::control_expression(function, statement, role, path)`, and
`Graph::control_operands(node)`. The graph validates parent links,
dense operand positions and type/symbol identities. These expressions
are optional because EDG may lower a source construct or represent its
control expressions elsewhere.

## Function statements (prototype)

`Compilation::il.statements` now contains a bounded snapshot of function-body
statements. Each record has a qualified function name, an EDG statement-kind
discriminant, a stable statement path, and an optional parent path. The bridge
recurses into blocks, if/else branches, while, for and range-for loops,
switch bodies, constexpr-if and try/catch handlers.
Unknown statement kinds are retained as opaque records.

`Compilation::il.expressions` snapshots the primary expression on a statement,
including ordered operands of EDG operation nodes. Records retain function,
statement path, expression path, parent expression path, EDG expression kind,
operator kind when applicable, the basic result type kind, and optional
variable/routine/field references. Other expression
kinds remain opaque. Some semantic type relationships and non-operation expression children remain
partial, and CFG edges are not yet represented. No EDG pointers escape the callback.

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
- The declaration snapshot includes namespaces and supported class member
  routines, including out-of-line definitions; access control, overload
  grouping, and portions of function-body IL remain incomplete.
- The custom back-end symbol relies on static archive resolution; link tests
  against the exact archive version are necessary.
- Exceptions do not cross the C ABI.

License: Apache-2.0 WITH LLVM-exception, as in upstream EDG.

## Type layout and constants

Expressions expose optional EDG target type size/alignment (bytes) and the
constant representation kind for constant expressions. Layout is absent
for incomplete or zero-sized types. Constant values and semantic type graph
edges are not yet serialized; consumers must not confuse kind tags with values.

Signed integer constants representable as i64 are copied as `Expression::constant_integer`; out-of-range integer values are left absent. This is not yet a general arbitrary-precision constant representation.

## Direct type relationships

Expression records include optional one-hop kind summaries for pointees,
array elements and function return types. These summaries are accompanied
by identity-preserving `TypeId` references in the owned `Il::types` table.
Use the type table for traversal instead of treating kind values as identities.

## Identity-preserving type snapshots

`Il::types` now contains `TypeDef` records keyed by `TypeId`. EDG type
addresses are mapped to per-compilation numeric IDs; the callback exports
only those IDs. Pointer, array and routine types link to pointee, element,
and return types respectively, allowing recursive type graphs to be represented.
Each expression carries an optional `type_id` referring into the same table.
Routine parameter types, typeref/typedef chains, record members and native
qualifier masks are exported where EDG exposes them. Full semantic C++
qualification and template parameter instantiation are still incomplete.
IDs are valid only within the containing compilation.

The indexed `Graph::from_il()` validates unique, nonzero type IDs, checks that direct type edges resolve, and rejects expressions referring to missing types. `Graph::type_def(TypeId)` resolves an owned type node. Cycles are permitted (recursive types).

## Function parameter edges

Function types now export their parameter types in declaration order as
`TypeParameter { owner, position, ty }` records. The indexed graph validates
both referenced type IDs and rejects duplicate parameter positions. Query
`Graph::parameters(TypeId)` for ordered parameter edges. Fixed array bounds
and native qualifier masks are exported, but dependent/VLA dimensions and
fully interpreted C++ qualifiers are not yet represented.

Fixed-size array types now export element counts as `ArrayBound` records. Bounds for variable-length, template-dependent, and incomplete arrays remain absent. `Graph::array_bound(TypeId)` resolves a known fixed bound.

Record fields include byte offsets, bit offsets within the byte, and bit-field widths from the target EDG layout. Zero bit width denotes an ordinary field in this snapshot.

Typeref types now have an underlying type edge (relation role 4). The bridge also exports EDG pointer reference/rvalue-reference flags and the raw qualifier mask for typeref nodes as `TypeDetail`. The qualifier mask is the native EDG encoding, not a portable `const`/`volatile` abstraction.

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

The wire contract includes typed events for symbols, types, statements,
source locations, lifetimes and cleanup records. It supports arbitrary
UTF-8 in string fields, including tabs and embedded NUL bytes. This
protocol is private to the in-process bridge and is not a stable file
format or public extension ABI.

Variable declarations preserve EDG's native `init_kind`, explicit-initializer
and uninitialized flags, attached to their `SymbolId`. These flags classify
the declaration and should not be mistaken for a complete initializer tree.

Dynamic variable initializers report their native EDG `dik_*` category and,
when supported, a copied scalar expression tree. Constructor and aggregate
initializer variants may still be represented only partially.

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


## Graph expression analysis

The owned `graph::Graph` API supports compiler passes that need to inspect
function-body expressions, supplemental control expressions, constructor
initializers, and variable initializers without retaining EDG native pointers.

```rust
use edgcpp::graph::Graph;
use edgcpp::SymbolId;

fn inspect(graph: &Graph, function: &str, routine: SymbolId, variable: SymbolId) {
    let stats = graph.routine_expression_stats(function, routine);
    println!("routine nodes: {}, unique symbols: {}",
        stats.total(), stats.distinct_symbols);

    for use_count in graph.routine_symbol_use_counts(function, routine) {
        println!("{:?}: {} uses", use_count.symbol, use_count.count);
    }

    let initializer = graph.variable_expression_stats(variable);
    println!("initializer nodes: {}", initializer.total());
}
```

`AnyExpressionRef` provides common `symbol_id()`, `type_id()`,
`kind()`, and `operator()` accessors. Traversal callbacks can stop early
by returning `false`. `walk_routine_expressions` visits primary and
supplemental control expressions in statement order, followed by constructor
initializers in position/tree-path order. `walk_variable_expressions`
visits initializer expressions in numeric tree-path order.

The statistics count **captured IL nodes**, not source-language expressions
or runtime operations. `symbol_uses` includes repeated references;
`distinct_symbols` counts each symbol once. Per-symbol use counts are
ordered by first occurrence. Function names and symbol IDs must refer to
the same compilation snapshot; IDs are not stable between compilations.

For bottom-up passes, use `walk_expression_postorder`,
`walk_statements_postorder`, or `walk_function_expressions_postorder`.


## Borrowed type and declaration views

`Graph::type_view(type_id)` combines a captured type definition with optional
type detail, function signature, and array element count. It returns `None`
when the type definition was not captured. `symbol_type_view(symbol_id)`
resolves a symbol's declared type and returns the same borrowed view.

For declaration-level processing, use `walk_record_fields`,
`walk_base_classes`, `walk_type_parameters`, and
`walk_member_functions`. These visitors return borrowed IL records and stop
when their callback returns `false`. They follow the captured IL order;
callers requiring a different ordering should sort their own collected IDs.
Missing owners simply produce no callbacks.

The type view is deliberately non-recursive: it does not infer semantic type
relationships that the EDG snapshot did not explicitly capture. References
are valid only while the owning `Graph` is alive.


## Declaration and record-layout API

`Graph::routine_view(symbol_id)` and `Graph::variable_view(symbol_id)`
provide borrowed declaration summaries. Both require a captured symbol;
uncaptured type metadata is represented by `None`, rather than inventing a
type or failing the entire query. Routine views expose the canonical captured
function name and optional implicit C++ `this` parameter. Variable views
expose optional initialization classification.

`walk_routine_parameters` visits explicit parameters, excluding implicit
`this`. `routine_parameter_types` returns their type IDs in captured order.

`walk_record_layout` visits direct base-class edges before nonstatic fields,
using `RecordLayoutItem::Base` and `RecordLayoutItem::Field`. It is a
declaration stream, **not** an ABI layout calculator: base offsets and
padding must not be inferred from this order. For direct dependencies,
`direct_base_types` and `record_field_types` return type IDs in captured
order. All visitors stop when the callback returns `false`.

These APIs retain the compilation-local ID and borrowing constraints of
`Graph`. They do not promise cross-compilation identity or complete
C++ semantic reconstruction.


## Type dependency graph

The graph API exposes direct captured type relationships through
`type_dependencies(type_id)` and `walk_type_dependencies(type_id, visitor)`.
Each edge identifies its target and kind: `Related`, `Base`, `Field`,
or `Parameter`. Direct edges are emitted in that category order, retaining
duplicate edges where the IL has multiple relationships.

`reachable_types(type_id)` and `walk_reachable_types(type_id, visitor)`
perform iterative breadth-first traversal with cycle detection. Results
include the root at depth zero and report each type ID once with its
shortest discovered edge distance. `symbol_reachable_types(symbol_id)`
starts from the declared type of a symbol, returning `None` if no
symbol-to-type edge was captured.

References to type IDs without a captured definition are included as
leaves, not silently discarded. These are structural references in the
captured snapshot, not proof of complete semantic dependency or ABI layout.
Traversal callbacks may return `false` to stop early.


## Reverse type dependencies and cycles

`direct_type_dependents(type_id)` reports captured types with an outgoing
dependency to the requested type. `type_dependents(type_id)` and
`walk_type_dependents(type_id, visitor)` traverse the reverse graph using
cycle-safe breadth-first search, including the root at depth zero. They can
support conservative invalidation when a type changes.

`type_strongly_connected_components()` partitions captured type definitions
into maximal mutually reachable groups using an iterative two-pass DFS.
Edges to uncaptured type IDs do not create component members. Component
ordering is an implementation detail; callers should not use it as a stable
serialization order. These relationships reflect only the captured EDG
snapshot, not every possible C++ semantic dependency.


## Incremental type invalidation

`affected_types(&changed_type_ids)` computes a conservative reverse
dependency closure for one or more changed type IDs. It deduplicates seeds,
includes each seed at depth zero, and reports the shortest reverse-edge
distance from any seed. `affected_symbols(&changed_type_ids)` returns
captured symbols whose declared type lies in that closure, in symbol order.

`affected_type_groups(&changed_type_ids)` groups affected captured types
by strongly connected components, so mutually dependent types can be
reprocessed together. Uncaptured IDs remain singleton groups. The groups
are not a topological build schedule; callers must order them according to
their pass dependencies. This analysis reflects captured structural type
edges only, and does not substitute for complete C++ semantic invalidation.
