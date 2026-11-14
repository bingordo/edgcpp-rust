/* SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
 * A narrow, synchronous C ABI over EDG's native C++ IL.
 *
 * This file must be compiled with the exact configuration of libedgcpfe.a.
 * Its back_end() replaces the C-generating back end when the archive is linked.
 */
#include "fe_common.h"
#include "const_ints.h"
#include "lower_name.h"
#include <cstdint>
#include <cstddef>
#include <cstring>
#include <limits>
#include <string>
#include <cxxabi.h>
#include <cstdlib>
#include <cstdio>
#include <unordered_map>
#include <unordered_set>
#include <vector>
#include <initializer_list>

extern "C" {
struct edgcpp_field { const char *data; size_t length; uint64_t value; uint32_t tag; };
static_assert(sizeof(edgcpp_field) == 32, "Unexpected native wire-field layout");
static_assert(alignof(edgcpp_field) == 8, "Unexpected native wire-field alignment");
typedef void (*edgcpp_il_callback)(void *, uint32_t, const edgcpp_field *, size_t, uint32_t);
}

namespace {
thread_local edgcpp_il_callback current_callback = nullptr;
thread_local void *current_user_data = nullptr;
thread_local std::unordered_map<const void *, uint32_t> type_ids;
thread_local std::unordered_map<const void *, uint32_t> symbol_ids;
thread_local std::unordered_set<const void *> visited_scopes;
thread_local std::unordered_set<const void *> emitted_symbol_types;
thread_local std::unordered_map<const void *, uint32_t> current_lifetime_ids;
thread_local std::unordered_map<const void *, std::string> scope_statement_paths;
thread_local std::unordered_set<const void *> emitted_routines;
thread_local std::unordered_set<const void *> emitted_member_edges;
struct PendingStaticLocal {
  const void *variable;
  const void *routine;
};
thread_local std::vector<PendingStaticLocal> pending_static_locals;
struct PendingNamedStatic {
  const void *variable;
  std::string qualified_name;
};
thread_local std::vector<PendingNamedStatic> pending_named_statics;
thread_local std::unordered_map<const void *, std::string> routine_names;
thread_local std::unordered_set<std::string> used_routine_names;
struct PendingMember {
  const void *routine;
  uint32_t symbol;
  std::string name;
};
thread_local std::vector<PendingMember> unresolved_members;
thread_local std::unordered_map<std::string, uint32_t> named_class_types;

// A per-compilation, pre-lowering witness of EDG's original virtual call.
// The two node-address carriers are observed only while EDG owns the IL.
// Copying/inlining may lose them; a surviving carrier is diagnostic, never a
// one-to-one completeness or executable Itanium vtable ABI certificate.
struct PrelowerVirtualCall {
  const void *routine;
  const void *original_receiver;
  const void *receiver_type;
  unsigned original_operator;
  const void *original_routine_operand;
  bool ambiguous;
};
thread_local std::unordered_map<const void *, PrelowerVirtualCall>
    prelower_virtual_calls;

// Source-owned native IA64_ABI vptr + index expression. All three exact
// node identities and the parent->operand links must survive into the
// final callee subtree; a detached index is insufficient evidence.
// This still does not prove a live vtable address point or machine ABI.
struct NativeVirtualEntrySelection {
  const void *entry_expression;
  const void *vptr_expression;
  const void *index_expression;
  uint64_t source_index;
  uint64_t entry_bytes;
  bool ambiguous;
};
thread_local std::unordered_map<const void *, NativeVirtualEntrySelection>
    native_virtual_entry_selections;

// Source-observed object used in vptr lookup and the reusable receiver
// which EDG will pass to the same rewritten indirect virtual call.
// Pointer identity is diagnostic: this does not prove class subobject
// location, object lifetime, ABI adjustments or execution safety.
struct NativeVirtualReceiverPair {
  const void *vptr_lookup_object;
  const void *reusable_receiver;
  bool ambiguous;
};
thread_local std::unordered_map<const void *, NativeVirtualReceiverPair>
    native_virtual_receiver_pairs;

// Recorded ONLY by a second EDG source hook after the actual call rewrite.
// This is stronger than old raw-pointer-only guesses: the original call and
// new eok_call must be associated by the single source transformation, and
// the final emitted call must still own exactly the same callee node.
// No post-hook, ambiguous map, or copied/replaced callee -> no wire event.
struct PostlowerVirtualCall {
  PrelowerVirtualCall source;
  const void *rewritten_callee;
  // EDG source get_virtual_function_address explicitly returned this
  // make_reusable_copy receiver. This is only a node provenance witness,
  // not a machine this-adjustment or proof of live object storage.
  const void *receiver_reused_for_call;
  NativeVirtualEntrySelection native_entry;
  bool native_entry_seen;
  NativeVirtualReceiverPair receiver_pair;
  bool receiver_pair_seen;
  bool ambiguous;
};
thread_local std::unordered_map<const void *, PostlowerVirtualCall>
    postlower_virtual_calls;
}

namespace edg {
// Wire tags and selected record IDs share schema/wire_schema.json with Rust.
// Deliberately invalid RECORD_INCOMPLETE_SNAPSHOT (kind 0) is reserved.
#include "wire_schema_generated.inc"
static edgcpp_field txt(const std::string &s) { return {s.data(), s.size(), 0, WIRE_TEXT}; }
static edgcpp_field num(uint64_t n) { return {nullptr, 0, n, WIRE_UNSIGNED}; }
static edgcpp_field signed_num(int64_t n) { return {nullptr, 0, static_cast<uint64_t>(n), WIRE_SIGNED}; }
static edgcpp_field absent() { return {nullptr, 0, 0, WIRE_ABSENT}; }
static void emit_record(uint32_t kind, std::initializer_list<edgcpp_field> fields, uint32_t aux = 0)
{
  if (!wire_record_field_tags_valid(kind, fields)) {
    // Report a malformed snapshot instead of sending a mis-tagged record
    // that might be misinterpreted by a different native/managed version.
    current_callback(current_user_data, RECORD_INCOMPLETE_SNAPSHOT, nullptr, 0, 0);
    return;
  }
  current_callback(current_user_data, kind, fields.begin(), fields.size(), aux);
}
static void report_incomplete_snapshot()
{
  if (current_callback != nullptr)
    emit_record(RECORD_INCOMPLETE_SNAPSHOT, {});
}

// Native linked lists have an explicit defensive upper bound. Never treat
// reaching it as a clean end-of-list: report invalid IL to the Rust collector.
// Full object images are intentionally limited to 4 KiB until larger
// layouts and relocation storage receive separate resource validation.
static constexpr std::size_t MAX_NATIVE_AGGREGATE_IMAGE_BYTES = 4096;
static constexpr unsigned MAX_NATIVE_AGGREGATE_INITIALIZER_VISITS = 4096;
static constexpr std::size_t MAX_NATIVE_LIST_ELEMENTS = 10000;
static bool within_native_list_limit(std::size_t index)
{
  if (index < MAX_NATIVE_LIST_ELEMENTS) return true;
  report_incomplete_snapshot();
  return false;
}
// EDG sequence numbers are compilation-global and may incorporate includes
// and #line remapping. Resolve while the source-file lookup table is live.
// Omit synthetic/unknown locations rather than inventing a source line.
struct ResolvedSource {
  uint32_t seq;
  uint32_t column;
  std::string file;
  uint32_t line;
};
static bool resolve_source(const a_source_position &position,
                           ResolvedSource &resolved)
{
  if (position.seq == 0) return false;
  a_const_char *file = nullptr;
  a_const_char *full = nullptr;
  a_line_number line = 0;
  a_boolean at_end = FALSE;
  a_source_file_ptr source = conv_seq_to_file_and_line(
      position.seq, &file, &full, &line, &at_end);
  if (source == nullptr || line == 0 || at_end) return false;
  const char *name = file != nullptr ? file : full;
  if (name == nullptr) return false;
  resolved = {position.seq, position.column, name, line};
  return true;
}
static void emit_statement_source(const std::string &function,
                                  const std::string &path,
                                  const a_source_position &pos)
{
  ResolvedSource location{};
  if (resolve_source(pos, location)) {
    emit_record(RECORD_STATEMENT_SOURCE, {txt(function), txt(path), num(location.seq),
                     num(location.column), txt(location.file), num(location.line)});
  }
}
static void emit_expression_source(const std::string &function,
                                   const std::string &statement,
                                   uint32_t role,
                                   const std::string &path,
                                   const a_source_position &pos)
{
  ResolvedSource location{};
  if (resolve_source(pos, location)) {
    emit_record(RECORD_EXPRESSION_SOURCE, {txt(function), txt(statement), num(role), txt(path),
                     num(location.seq), num(location.column),
                     txt(location.file), num(location.line)});
  }
}

static const char *source_name(const a_source_correspondence &sc)
{
#if NEED_NAME_MANGLING
  if (sc.name_has_been_mangled &&
      sc.unmangled_name_or_mangled_encoding != nullptr) {
    return sc.unmangled_name_or_mangled_encoding;
  }
#endif
  return sc.name;
}

static std::string namespace_prefix(a_scope_ptr parent)
{
  // File-scope IL lists can include entities declared in nested namespaces.
  // Use each declaration's parent scope, not the containing list, to qualify it.
  std::string result;
  // A fixed depth cutoff silently truncated valid C++ qualified names.
  // Track native scope identities instead, so even malformed cyclic parent
  // chains terminate without modifying a valid deep namespace hierarchy.
  std::unordered_set<a_scope_ptr> visited;
  for (a_scope_ptr scope = parent; scope != nullptr;
       scope = scope->parent) {
    if (!visited.insert(scope).second) break;
    if (scope->kind != sck_namespace) continue;
    a_namespace_ptr ns = scope->variant.assoc_namespace;
    if (ns == nullptr) continue;
    const char *name = source_name(ns->source_corresp);
    if (name != nullptr) {
      result = std::string(name) + "::" + result;
    }
  }
  return result;
}

// Conservative fallback when the system C++ demangler refuses a very deep
// Itanium ABI object name. Decode only _ZN<length><identifier>...E with no
// substitutions, templates, qualifiers or type suffixes. A partial parse
// must never become a plausible but incorrect C++ source name.
static std::string plain_nested_object_name(const char *raw)
{
  if (raw == nullptr || std::strncmp(raw, "_ZN", 3) != 0) return {};
  const char *end = raw + std::strlen(raw);
  const char *cursor = raw + 3;
  std::string name;
  while (cursor < end && *cursor != 'E') {
    if (*cursor < '0' || *cursor > '9') return {};
    std::size_t width = 0;
    do {
      const unsigned digit = static_cast<unsigned>(*cursor - '0');
      if (width > (std::numeric_limits<std::size_t>::max() - digit) / 10)
        return {};
      width = width * 10 + digit;
      ++cursor;
    } while (cursor < end && *cursor >= '0' && *cursor <= '9');
    if (width == 0 || width > static_cast<std::size_t>(end - cursor))
      return {};
    if (!name.empty()) name += "::";
    name.append(cursor, width);
    cursor += width;
  }
  if (name.empty() || cursor >= end || *cursor != 'E' || cursor + 1 != end)
    return {};
  return name;
}

static std::string declaration_name(const a_source_correspondence &sc,
                                    bool is_routine, bool keep_parameters = false)
{
#if NEED_NAME_MANGLING
  if (sc.name_has_been_mangled && sc.name != nullptr) {
    int status = 0;
    char *decoded = abi::__cxa_demangle(sc.name, nullptr, nullptr, &status);
    if (status == 0 && decoded != nullptr) {
      std::string qualified(decoded);
      std::free(decoded);
      if (is_routine && !keep_parameters) {
        const auto param = qualified.find('(');
        if (param != std::string::npos) qualified.resize(param);
      }
      return qualified;
    }
    std::free(decoded);
    if (!is_routine) {
      auto plain = plain_nested_object_name(sc.name);
      if (!plain.empty()) return plain;
    }
  }
#endif
  return namespace_prefix(sc.parent_scope) +
         (source_name(sc) != nullptr ? source_name(sc) : "");
}

// Native addresses are used only as lookup keys while EDG owns the IL.
static uint32_t emit_symbol(const void *ptr, unsigned kind,
                            const std::string &name)
{
  if (ptr == nullptr || current_callback == nullptr) return 0;
  auto found = symbol_ids.find(ptr);
  if (found != symbol_ids.end()) return found->second;
  uint32_t id = static_cast<uint32_t>(symbol_ids.size() + 1);
  symbol_ids.emplace(ptr, id);
  emit_record(RECORD_SYMBOL, {num(id), num(kind), txt(name)});
  if (kind >= 1 && kind <= 3) {
    // EDG routines, variables and fields all place source_corresp first.
    const auto *correspondence =
        reinterpret_cast<const a_source_correspondence *>(ptr);
    // C++5: emit EDG's *actual* stored routine OR global-data spelling separately from
    // any demangled display/source key. A C++ overload or global data may have the same
    // display base as other routines; only the source correspondence
    // carries the name EDG has already mangled. Do not re-mangle it here.
    // For a C++ external data declaration (e.g. std::cout), EDG may
    // retain only its unmangled source spelling until name lowering.
    // Ask EDG's own target name mangler for the exact linker spelling;
    // never invent an Itanium encoding in our bridge or Rust frontend.
    // This API is read-only and does not change source_corresp.name.
    bool emitted_native_link = false;
#if TEMPLATE_LOOKUP_NEEDED || MODULE_ID_NEEDED
    if (kind == 2) {
      const auto *variable = reinterpret_cast<const a_variable *>(ptr);
      if (variable->storage_class == sc_extern &&
          !variable->has_explicit_initializer &&
          !variable->is_thread_local &&
          !variable->source_corresp.is_class_member &&
          !variable->is_template_variable &&
          !variable->is_prototype_instantiation &&
          variable->source_corresp.name != nullptr &&
          variable_name_mangling_needed(const_cast<a_variable_ptr>(variable))) {
        const char *native = get_mangled_variable_name(
            const_cast<a_variable_ptr>(variable));
        if (native != nullptr && native[0] == '_' && native[1] == 'Z') {
          emit_record(61, {num(id), txt(std::string(native)), num(1)});
          emitted_native_link = true;
        }
      }
    }
#endif
    if (!emitted_native_link &&
        (kind == 1 || kind == 2) && correspondence->name != nullptr &&
        correspondence->name[0] != '\0') {
      unsigned already_mangled = 0;
#if NEED_NAME_MANGLING
      already_mangled = correspondence->name_has_been_mangled ? 1 : 0;
#endif
      emit_record(61, {num(id), txt(std::string(correspondence->name)),
                       num(already_mangled)});
    }
    ResolvedSource position{};
    if (resolve_source(correspondence->decl_position, position)) {
      emit_record(36, {num(id), num(position.seq), num(position.column),
                       txt(position.file), num(position.line)});
    }
  }
  return id;
}

// Source-authoritative C++1 class-object gate for trivial record storage and
// ordinary nonvirtual methods. Only an EDG-certified trivial object can be
// passed as an implicit this pointer; no constructor/destructor ABI is inferred.
// EDG's complete member scope includes methods which may be inlined away
// before the native frontend sees an expression. Never infer triviality
// merely from the absence of output routine bodies or record-field shapes.
// EDG's is_pod_class() recurses through class symbol supplements; that
// semantic routine is not safe for every notional/template-prototype type
// exposed by full GNU C++ library headers during the *output* snapshot.
// Native C++0/1/2 do not admit template class object layouts yet. Reject
// these before invoking any EDG lifetime or POD introspection, rather than
// treating an unsupported template as ordinary/POD or dereferencing a
// missing supplement in the compiler's foreign-language bridge.
static bool supported_concrete_class_proof_root(a_type_ptr type) {
  if (type == nullptr ||
      (type->kind != tk_class && type->kind != tk_struct) ||
      type->incomplete ||
      type->variant.class_struct_union.is_nonreal_class) return false;
  a_symbol_ptr sym = symbol_for(type);
  if (sym == nullptr || sym->kind != sk_class_or_struct_tag) return false;
  const auto *supplement = class_symbol_supp(sym);
  return supplement != nullptr && supplement->class_template == nullptr;
}

static bool plain_cpp_record(a_type_ptr type) {
  if (!supported_concrete_class_proof_root(type)) return false;
  const auto *extra = type->variant.class_struct_union.extra_info;
  if (extra == nullptr || extra->assoc_scope == nullptr ||
      extra->direct_base_classes != nullptr ||
      extra->has_field_initializer || extra->is_lambda_closure_class ||
      type->variant.class_struct_union.any_virtual_functions ||
      type->variant.class_struct_union.any_virtual_base_classes ||
      extra->assoc_scope->variables != nullptr) return false;
  // The runtime routine list and source declaration list can both be
  // pruned by EDG. Use its authoritative *semantic* C++ class property:
  // a POD/trivial class cannot have a user-provided nontrivial constructor,
  // destructor or initializer. This survives constructor inlining.
  // A source member-list entry, when retained, can additionally reject
  // any explicit routine without relying on mutable field pointer identity.
  if (symbol_for(type) == nullptr || !is_pod_class(type) ||
      !is_trivial_class(type) || type_has_nontrivial_destructor(type)) {
    return false;
  }
#if MAINTAIN_CLASS_MEMBER_LIST
  unsigned declarations = 0;
  for (an_il_entity_list_entry_ptr entry = extra->member_declarations;
       entry != nullptr; entry = entry->next) {
    if (++declarations > 512 ||
        (entry->entity.kind != iek_field &&
         entry->entity.kind != iek_routine)) return false;
  }
#endif
  unsigned count = 0;
  for (a_routine_ptr method = extra->assoc_scope->routines;
       method != nullptr; method = method->next) {
    if (++count > 256 || method->is_virtual) return false;
    switch (method->special_kind) {
      case sfk_constructor:
        if (special_member_is_user_provided(method) ||
            (!method->is_trivial_default_constructor &&
             !method->is_trivial_copy_function)) return false;
        break;
      case sfk_destructor:
        if (special_member_is_user_provided(method) ||
            !method->is_trivial_destructor) return false;
        break;
      case sfk_operator:
        if (special_member_is_user_provided(method) ||
            !method->is_trivial_copy_function) return false;
        break;
      case sfk_none:
        // No virtual dispatch or special object lifetime is introduced by
        // an ordinary nonvirtual member's own declaration. Its body and
        // this argument remain subject to the compiler's separate P1 gate.
        break;
      default:
        return false; // Conversion/literal operators, etc.
    }
  }
  return true;
}

// C++2 P2.1a: separate *nontrivial* class-scope evidence. Do not
// relabel a user-defined constructor/destructor as POD or share P1's
// plain_cpp_record gate. This only certifies a narrow candidate for a
// later path-sensitive native lifetime verifier, NOT executable admission.
static bool bounded_cpp_scalar_lifetime(a_type_ptr type) {
  if (!supported_concrete_class_proof_root(type) ||
      type->size != 4 || type->alignment != 4) return false;
  const auto *extra = type->variant.class_struct_union.extra_info;
  if (extra == nullptr || extra->assoc_scope == nullptr ||
      extra->direct_base_classes != nullptr ||
      extra->has_field_initializer || extra->is_lambda_closure_class ||
      type->variant.class_struct_union.any_virtual_functions ||
      type->variant.class_struct_union.any_virtual_base_classes ||
      extra->assoc_scope->variables != nullptr ||
      !type_has_nontrivial_destructor(type)) return false;
  // One ordinary 32-bit scalar, at byte offset zero; no bitfields,
  // subobjects or hidden layout that our first native P2 caller could
  // initialize or destroy accidentally.
  a_field_ptr field = type->variant.class_struct_union.field_list;
  if (field == nullptr || field->next != nullptr ||
      field->type == nullptr || field->type->kind != tk_integer ||
      field->type->size != 4 || field->type->alignment != 4 ||
      field->offset != 0 || field->offset_bit_remainder != 0 ||
      field->is_bit_field) return false;
#if MAINTAIN_CLASS_MEMBER_LIST
  unsigned declarations = 0;
  for (an_il_entity_list_entry_ptr entry = extra->member_declarations;
       entry != nullptr; entry = entry->next) {
    if (++declarations > 128 ||
        (entry->entity.kind != iek_field &&
         entry->entity.kind != iek_routine)) return false;
  }
#endif
  // EDG may prune the class routine list before this type is serialized:
  // an empty list is NOT evidence that the user constructor/destructor
  // or any other special members are absent. The native TypeId caller
  // must independently verify exact routine ownership, special_kind,
  // ABI and executable call/cleanup paths from retained IL metadata.
  // Thus this event is only a structural *candidate*, never authority
  // to emit a nontrivial object by itself.
  return true;
}

// Assign dense IDs during compilation; never export EDG type addresses.
static uint32_t emit_type(a_type_ptr type, unsigned depth)
{
  if (type == nullptr || current_callback == nullptr) return 0;
  // Already registered recursive types are safe regardless of encounter depth.
  auto existing = type_ids.find(type);
  if (existing != type_ids.end()) return existing->second;
  // Missing a type relationship must invalidate the whole native snapshot,
  // not silently degrade the relationship to "no target".
  if (depth > 64) { report_incomplete_snapshot(); return 0; }
  uint32_t id = static_cast<uint32_t>(type_ids.size() + 1);
  type_ids.emplace(type, id); // Register before recursion, including recursive types.
  a_type_ptr related = nullptr;
  unsigned relation = 0;
  if (type->kind == tk_pointer) {
    related = type->variant.pointer.type;
    relation = 1;
  } else if (type->kind == tk_array) {
    related = type->variant.array.element_type;
    relation = 2;
  } else if (type->kind == tk_routine) {
    related = type->variant.routine.return_type;
    relation = 3;
  } else if (type->kind == tk_typeref) {
    related = type->variant.typeref.type;
    relation = 4; // Underlying typedef / qualified type.
  }
  uint32_t child = emit_type(related, depth + 1);
  if (child == 0) relation = 0; // Never export a relationship without a target.
  // Event 63 is an immutable EDG class-scope proof. A bare C record has
  // no C++ class ABI; event 64 separately identifies the language.
  // In C++ mode a missing or false proof must fail closed even if a
  // constructor was optimized into ordinary scalar stores.
  if (type->kind == tk_class || type->kind == tk_struct) {
    emit_record(63, {num(id), num(plain_cpp_record(type) ? 1 : 0)});
    emit_record(65, {num(id), num(bounded_cpp_scalar_lifetime(type) ? 1 : 0)});
  }
  // Native EDG object layout, never reconstructed from field order.
  // An alignment of zero is not a usable layout.
  if (!type->incomplete && type->size > 0 && type->alignment > 0) {
    emit_record(49, {num(id), num(type->size), num(type->alignment)});
  }
  if (type->kind == tk_class || type->kind == tk_struct || type->kind == tk_union) {
    // Native count proof is independent of event 10 and events 19/24/69.
    // A lost last field or base must not appear as a shorter valid layout.
    uint32_t field_count = 0;
    for (a_field_ptr field = type->variant.class_struct_union.field_list;
         field != nullptr && within_native_list_limit(field_count);
         field = field->next, ++field_count) {}
    uint32_t direct_base_count = 0;
    if ((type->kind == tk_class || type->kind == tk_struct) &&
        type->variant.class_struct_union.extra_info != nullptr) {
      for (a_base_class_ptr base =
               type->variant.class_struct_union.extra_info->direct_base_classes;
           base != nullptr && within_native_list_limit(direct_base_count);
           base = base->next_direct, ++direct_base_count) {}
    }
    emit_record(RECORD_NATIVE_RECORD_SHAPE,
                {num(id), num(field_count), num(direct_base_count)});
    unsigned index = 0;
    for (a_field_ptr field = type->variant.class_struct_union.field_list;
         field != nullptr && within_native_list_limit(index); field = field->next, ++index) {
      uint32_t field_type = emit_type(field->type, depth + 1);
      const char *field_name = source_name(field->source_corresp);
      std::string field_text = field_name == nullptr ? "" : field_name;
      // Preserve the field's native identity. Matching by name or
      // declaration position alone is ambiguous for anonymous members.
      const uint32_t field_symbol = emit_symbol(field, 3, field_text);
      emit_record(10, {num(id), num(index), num(field_type), txt(field_text),
                       num(field->offset), num(field->offset_bit_remainder),
                       num(field->is_bit_field ? field->bit_size : 0),
                       num(field_symbol)});
    }
  }
  // Direct base classes are linked in source declaration order, unlike
  // the transitive postorder base_classes list.
  if ((type->kind == tk_class || type->kind == tk_struct) &&
      type->variant.class_struct_union.extra_info != nullptr) {
    unsigned position = 0;
    for (a_base_class_ptr base =
             type->variant.class_struct_union.extra_info->direct_base_classes;
         base != nullptr && within_native_list_limit(position);
         base = base->next_direct, ++position) {
      const uint32_t base_id = emit_type(base->type, depth + 1);
      if (base_id != 0) {
        emit_record(19, {num(id), num(position), num(base_id),
                         num(base->is_virtual ? 1 : 0)});
        // EDG's target object layout owns this byte offset. Do NOT
        // interpret an unset offset as the valid offset zero, and never
        // use a static byte adjustment for a virtual base.
        bool offset_proven = !base->is_virtual && !type->incomplete;
#if IA64_ABI
        offset_proven = offset_proven && base->offset_is_set;
#else
        // The classic ABI lacks EDG's explicit offset_is_set proof.
        offset_proven = false;
#endif
        emit_record(69, {num(id), num(position),
                         offset_proven ? num(base->offset) : absent()});
        // For virtual bases there may be several derivation paths.
        // Choose the direct derivation when reporting the access written
        // on this base specifier, not an indirect access path.
        a_base_class_derivation_ptr direct_path = base->derivation;
        while (direct_path != nullptr && !direct_path->direct) {
          direct_path = direct_path->next;
        }
        emit_record(24, {num(id), num(position),
                         direct_path != nullptr ?
                             num(static_cast<unsigned>(direct_path->access)) : absent(),
                         num(base->is_pack_expansion ? 1 : 0)});

      }
    }
  }
  // Event 71 independently certifies the actual EDG parameter-list length.
  // Do not derive this from the event 8 records: a missing final event 8
  // would otherwise silently shorten a routine's native call signature.
  if (type->kind == tk_routine) {
    const auto *info = type->variant.routine.extra_info;
    uint32_t native_parameter_count = 0;
    if (info != nullptr) {
      for (a_param_type_ptr p = info->param_type_list;
           p != nullptr && within_native_list_limit(native_parameter_count);
           p = p->next, ++native_parameter_count) {}
    }
    emit_record(RECORD_ROUTINE_SUPPLEMENT,
                {num(id), num(info != nullptr ? 1 : 0),
                 num(native_parameter_count)});
  }
  // Preserve the calling interface shape, including variadic functions.
  if (type->kind == tk_routine && type->variant.routine.extra_info != nullptr) {
    const auto *info = type->variant.routine.extra_info;
    emit_record(12, {num(id), num(info->prototyped ? 1 : 0),
                     num(info->has_ellipsis ? 1 : 0)});
  }
  // Send additional parameter type edges in declaration order.
  if (type->kind == tk_routine && type->variant.routine.extra_info != nullptr) {
    unsigned position = 0;
    for (a_param_type_ptr p = type->variant.routine.extra_info->param_type_list;
         p != nullptr && within_native_list_limit(position); p = p->next, ++position) {
      uint32_t parameter = emit_type(p->type, depth + 1);
      emit_record(8, {num(id), num(position), num(parameter)});
    }
  }
  emit_record(7, {num(id), num(static_cast<unsigned>(type->kind)),
                  num(relation), num(child)});
  // This reference backend only promises the signed i64 SysV C ABI.
  // Do not guess from EDG enum numbers, typenames or argument expressions.
  // Typerefs are resolved by Rust through the type graph.
  if (type->kind == tk_integer && !type->incomplete && type->size == 8 &&
      !type->variant.integer.enum_type) {
    const auto integer_kind = type->variant.integer.int_kind;
    bool signed_64 = integer_kind == ik_long;
#if LONG_LONG_ALLOWED
    signed_64 = signed_64 || integer_kind == ik_long_long;
#endif
    if (signed_64) emit_record(43, {num(id)});
  }
  // Capture scalar C ABI width/sign from EDG, rather than assuming host
  // C++ integer layout or manually copying EDG private discriminants.
  // Restrict to ordinary complete 8/16/32/64-bit integer types (no enums,
  // bit-precise types, or unsupported extended integer sizes).
  if (type->kind == tk_integer && !type->incomplete &&
      !type->variant.integer.enum_type &&
      (type->size == 1 || type->size == 2 ||
       type->size == 4 || type->size == 8) &&
      type->variant.integer.int_kind != ik_bit_precise &&
      type->variant.integer.int_kind != ik_unsigned_bit_precise) {
    const auto kind = type->variant.integer.int_kind;
    if (kind < ik_last) {
      emit_record(46, {num(id), num(type->size * 8),
                       num(type->variant.integer.bool_type ? 0 :
                           int_kind_is_signed[kind] ? 1 : 0),
                       num(type->variant.integer.bool_type ? 1 : 0)});
    }
  }
  // M14: export *only* the two standard binary FP source types. Width is
  // confirmed by EDG's native target layout and the exact float_kind,
  // never inferred from the spelling of an unrelated extended FP type.
  if (type->kind == tk_float && !type->incomplete &&
      ((type->variant.float_kind == fk_float && type->size == 4) ||
       (type->variant.float_kind == fk_double && type->size == 8))) {
    emit_record(57, {num(id), num(type->size * 8)});
  }
  // Certify whether native records 43/46/57 must be present. These EDG
  // predicates are independently evaluated from source type properties;
  // never infer presence from the records that reached the Rust callback.
  bool native_signed_i64 = false;
  if (type->kind == tk_integer && !type->incomplete && type->size == 8 &&
      !type->variant.integer.enum_type) {
    const auto kind = type->variant.integer.int_kind;
    native_signed_i64 = kind == ik_long;
#if LONG_LONG_ALLOWED
    native_signed_i64 = native_signed_i64 || kind == ik_long_long;
#endif
  }
  bool native_integer_abi = false;
  if (type->kind == tk_integer && !type->incomplete &&
      !type->variant.integer.enum_type &&
      (type->size == 1 || type->size == 2 ||
       type->size == 4 || type->size == 8) &&
      type->variant.integer.int_kind != ik_bit_precise &&
      type->variant.integer.int_kind != ik_unsigned_bit_precise) {
    native_integer_abi = type->variant.integer.int_kind < ik_last;
  }
  const bool native_float_abi =
      type->kind == tk_float && !type->incomplete &&
      ((type->variant.float_kind == fk_float && type->size == 4) ||
       (type->variant.float_kind == fk_double && type->size == 8));
  emit_record(RECORD_NATIVE_SCALAR_METADATA_PROOF,
              {num(id), num((native_integer_abi ? 1 : 0) |
                            (native_float_abi ? 2 : 0) |
                            (native_signed_i64 ? 4 : 0))});
  // Type qualifiers and reference distinctions are native EDG properties,
  // rather than inferred from the printed type name.
  if (type->kind == tk_pointer || type->kind == tk_typeref) {
    unsigned flags = 0;
    if (type->kind == tk_pointer) {
      if (type->variant.pointer.is_reference) flags |= 1;
      if (type->variant.pointer.is_rvalue_reference) flags |= 2;
    }
    emit_record(11, {num(id), num(flags),
                     type->kind == tk_typeref ?
                       num(static_cast<unsigned>(type->variant.typeref.qualifiers)) :
                       absent()});
  }
  if (type->kind == tk_array &&
      !type->variant.array.is_variable_size_array &&
      !type->variant.array.is_template_dependent_size_array &&
      !type->incomplete) {
    emit_record(9, {num(id), num(type->variant.array.variant.number_of_elements)});
  }
  // Independently certify whether event 49 and event 9 MUST be present.
  // Absence otherwise looks like an incomplete type or variable array.
  // Use the EDG source state, never the number of wire records emitted.
  const bool native_has_layout =
      !type->incomplete && type->size > 0 && type->alignment > 0;
  const bool native_has_fixed_array_bound =
      type->kind == tk_array && !type->incomplete &&
      !type->variant.array.is_variable_size_array &&
      !type->variant.array.is_template_dependent_size_array;
  emit_record(RECORD_NATIVE_TYPE_LAYOUT_PROOF,
              {num(id), num(native_has_layout ? 1 : 0),
               num(native_has_fixed_array_bound ? 1 : 0)});
  return id;
}

// Keep function-body and dynamic-initializer traversal in sync. EDG operation
// operands are linked lists; wrapper expressions own a single child.
// Other variants remain deliberately opaque until their native layout is verified.
struct expression_children {
  an_expr_node_ptr first;
  bool linked_list;
};

static expression_children children_of(an_expr_node_ptr node)
{
  if (node == nullptr) return {nullptr, false};
  if (node->kind == enk_operation)
    return {node->variant.operation.operands, true};
  // EDG il_def.h: these variants directly own linked expression lists.
  // Function calls and constructor invocations are enk_operation nodes,
  // already handled by the branch above.
  if (node->kind == enk_braced_init_list)
    return {node->variant.braced_init_list, true};
  if (node->kind == enk_typeid)
    return {node->variant.typeid_info.type_with_opt_expr, true};
  if (node->kind == enk_c11_generic)
    return {node->variant.c11_generic.operands, true};
  if (node->kind == enk_object_lifetime)
    return {node->variant.object_lifetime.expr, false};
  if ((node->kind == enk_sizeof || node->kind == enk_alignof ||
       node->kind == enk_datasizeof) && !node->variant.sizeof_info.is_type)
    return {node->variant.sizeof_info.variant.expr, false};
  return {nullptr, false};
}

// Fail-closed bounded walk of the final indirect *callee only*: require
// exact EDG source-built eok_padd, its vptr operand and its index operand.
// Merely finding the numeric index node somewhere in the callee must not
// certify that it remains part of the original vtable lookup. Pointer
// copies, unexpected cycles, aliasing and alternate shapes suppress proof.
static bool callee_contains_exact_native_padd(an_expr_node_ptr root,
                                              const void *entry,
                                              const void *vptr,
                                              const void *index)
{
  if (root == nullptr || entry == nullptr || vptr == nullptr ||
      index == nullptr || entry == vptr || entry == index || vptr == index)
    return false;
  std::vector<an_expr_node_ptr> pending{root};
  std::unordered_set<const void *> visited;
  unsigned entry_matches = 0, vptr_matches = 0, index_matches = 0;
  while (!pending.empty()) {
    if (visited.size() >= 512 || pending.size() > 512) return false;
    an_expr_node_ptr candidate = pending.back();
    pending.pop_back();
    if (candidate == nullptr || !visited.insert(candidate).second)
      return false;
    if (candidate == entry) {
      ++entry_matches;
      if (candidate->kind != enk_operation ||
          candidate->variant.operation.kind != eok_padd)
        return false;
      an_expr_node_ptr first = candidate->variant.operation.operands;
      if (first == nullptr || first != vptr ||
          first->next != index || first->next->next != nullptr)
        return false;
    }
    if (candidate == vptr) ++vptr_matches;
    if (candidate == index) ++index_matches;
    const expression_children children = children_of(candidate);
    unsigned count = 0;
    an_expr_node_ptr child = children.first;
    for (; child != nullptr && count < 64;
         child = children.linked_list ? child->next : nullptr, ++count) {
      pending.push_back(child);
    }
    if (child != nullptr || entry_matches > 1 ||
        vptr_matches > 1 || index_matches > 1) return false;
  }
  return entry_matches == 1 && vptr_matches == 1 && index_matches == 1;
}

// A bounded, cycle-safe lookup of a single exact EDG source node in a
// vptr operand subtree. It must not accidentally traverse a sibling
// expression outside that subtree (notably the adjacent table index).
static bool subtree_contains_unique_native_node(an_expr_node_ptr root,
                                                 const void *target)
{
  if (root == nullptr || target == nullptr) return false;
  std::vector<an_expr_node_ptr> pending{root};
  std::unordered_set<const void *> visited;
  unsigned matches = 0;
  while (!pending.empty()) {
    if (visited.size() >= 512 || pending.size() > 512) return false;
    an_expr_node_ptr node = pending.back();
    pending.pop_back();
    if (node == nullptr || !visited.insert(node).second) return false;
    if (node == target) ++matches;
    const expression_children children = children_of(node);
    unsigned count = 0;
    an_expr_node_ptr child = children.first;
    for (; child != nullptr && count < 64;
         child = children.linked_list ? child->next : nullptr, ++count) {
      pending.push_back(child);
    }
    if (child != nullptr || matches > 1) return false;
  }
  return matches == 1;
}

// EDG's call operators encode the callee as operand zero. Member calls
// additionally encode the object as operand one. Keep the classification
// symbolic here: numeric enum values depend on EDG's build configuration.
static unsigned call_argument_offset(an_expr_node_ptr node)
{
  if (node == nullptr || node->kind != enk_operation) return 0;
  switch (node->variant.operation.kind) {
    case eok_call: return 1;
    case eok_dot_member_call:
    case eok_points_to_member_call:
    case eok_dot_pm_call:
    case eok_points_to_pm_call: return 2;
    default: return 0;
  }
}

// M8: turn native constant aggregate trees into a fully sized, x86-64
// object image. Walk typed subobjects, not source-text initializer spelling.
// Incomplete, dynamic, bitfield or unrecognized entries are rejected rather
// than silently treating an unknown initializer as zero.
static a_type_ptr object_initializer_type(a_type_ptr type) {
  for (unsigned n = 0; n < 64 && type != nullptr &&
       type->kind == tk_typeref; ++n) {
    type = type->variant.typeref.type;
  }
  return type;
}
// Store linker references separately from integer object bytes.
struct NativeAggregateReloc {
  size_t byte_offset;
  a_variable_ptr target;
  int64_t addend;
};
static bool constant_initializer_image(
    a_constant_ptr value, a_type_ptr original_type, size_t offset,
    std::vector<uint8_t> &bytes, unsigned depth,
    std::vector<NativeAggregateReloc> *relocations);
// The existing object image is zero-filled. Apply only the bits of an EDG
// constant bitfield; adjacent members and padding are never overwritten.
// This uses native *byte* offset and offset_bit_remainder on little-endian
// x86-64, without assuming the declared int is an addressable storage unit.
static bool pack_constant_bitfield(
    a_constant_ptr value, a_field_ptr field, size_t object_base,
    std::vector<uint8_t> &bytes) {
  if (value == nullptr || value->kind != ck_integer || field == nullptr ||
      !field->is_bit_field || field->bit_size == 0 ||
      field->bit_size > 64 || field->offset_bit_remainder >= 8 ||
      field->offset_bit_remainder + field->bit_size > 64 ||
      object_base > bytes.size()) return false;
  a_type_ptr type = object_initializer_type(field->type);
  if (type == nullptr || type->kind != tk_integer ||
      type->size == 0 || type->size > 8 ||
      field->bit_size > type->size * 8) return false;
  const size_t byte_offset=object_base + field->offset;
  const size_t count=(field->offset_bit_remainder + field->bit_size + 7)/8;
  if (byte_offset<object_base || byte_offset>bytes.size() ||
      count>bytes.size()-byte_offset) return false;
  a_boolean overflow=FALSE;
  uint64_t bits=0;
  if (int_constant_is_signed(value)) {
    const a_host_large_integer signed_value =
        value_of_integer_constant(value,&overflow);
    if (overflow || sizeof(signed_value)>sizeof(int64_t)) return false;
    bits=static_cast<uint64_t>(static_cast<int64_t>(signed_value));
  } else {
    const a_host_large_unsigned unsigned_value =
        unsigned_value_of_integer_constant(value,&overflow);
    if (overflow || unsigned_value>std::numeric_limits<uint64_t>::max())
      return false;
    bits=static_cast<uint64_t>(unsigned_value);
  }
  for (unsigned i=0;i<field->bit_size;++i) {
    const size_t bit=static_cast<size_t>(field->offset_bit_remainder)+i;
    const size_t byte_index=byte_offset+bit/8;
    const uint8_t bit_mask=static_cast<uint8_t>(1u<<(bit%8));
    if ((bits>>i)&1u) bytes[byte_index]|=bit_mask;
    else bytes[byte_index]&=static_cast<uint8_t>(~bit_mask);
  }
  return true;
}
// Checked subobject-relative offsets. Nested designated initializers can
// walk multiple EDG field/array levels before reaching the leaf-size check;
// validate each step so malformed native offsets cannot wrap or underflow.
static constexpr bool aggregate_subobject_fits(
    size_t total, size_t base, size_t relative)
{
  return base <= total && relative <= total - base;
}
static_assert(aggregate_subobject_fits(MAX_NATIVE_AGGREGATE_IMAGE_BYTES,
                                       MAX_NATIVE_AGGREGATE_IMAGE_BYTES - 1, 1));
static_assert(!aggregate_subobject_fits(MAX_NATIVE_AGGREGATE_IMAGE_BYTES,
                                        MAX_NATIVE_AGGREGATE_IMAGE_BYTES - 1, 2));
static_assert(!aggregate_subobject_fits(MAX_NATIVE_AGGREGATE_IMAGE_BYTES,
                                        std::numeric_limits<size_t>::max(), 1));

static bool constant_designated_value(
    a_constant_ptr &current, a_type_ptr original_type, size_t offset,
    std::vector<uint8_t> &bytes, unsigned depth,
    std::vector<NativeAggregateReloc> *relocations) {
  if (current == nullptr || depth > 32 || offset > bytes.size()) return false;
  if (current->kind != ck_designator) {
    if (!constant_initializer_image(current, original_type, offset,
                                    bytes, depth + 1, relocations)) return false;
    current = current->next;
    return true;
  }
  a_type_ptr type = object_initializer_type(original_type);
  if (type == nullptr || current->variant.designator.is_generic)
    return false;
  a_type_ptr element = nullptr;
  size_t child_offset = offset;
  if (current->variant.designator.is_field_designator) {
    if (type->kind != tk_struct && type->kind != tk_union) return false;
    a_field_ptr selected = current->variant.designator.variant.field;
    bool found = false;
    for (a_field_ptr f = type->variant.class_struct_union.field_list;
         f != nullptr; f = f->next) {
      if (f == selected) { found = true; break; }
    }
    if (!found) return false;
    if (selected->is_bit_field) {
      current=current->next;
      if (!pack_constant_bitfield(current,selected,offset,bytes))
        return false;
      current=current->next;
      return true;
    }
    if (selected->offset_bit_remainder != 0 ||
        !aggregate_subobject_fits(bytes.size(), offset, selected->offset))
      return false;
    element = selected->type;
    child_offset = offset + static_cast<size_t>(selected->offset);
  } else {
    if (type->kind != tk_array || type->incomplete ||
        type->variant.array.is_variable_size_array ||
        type->variant.array.is_template_dependent_size_array) return false;
    const uint64_t index = current->variant.designator.variant.array_element;
    if (index >= type->variant.array.variant.number_of_elements) return false;
    element = type->variant.array.element_type;
    if (element == nullptr || element->size == 0 ||
        index > (bytes.size() - offset) / element->size) return false;
    child_offset += static_cast<size_t>(index * element->size);
  }
  current = current->next;
  return constant_designated_value(current, element, child_offset, bytes,
                                   depth + 1, relocations);
}
static bool constant_initializer_image(
    a_constant_ptr value, a_type_ptr original_type, size_t offset,
    std::vector<uint8_t> &bytes, unsigned depth,
    std::vector<NativeAggregateReloc> *relocations) {
  a_type_ptr type = object_initializer_type(original_type);
  if (value == nullptr || type == nullptr || type->incomplete ||
      type->size == 0 || depth > 32 || offset > bytes.size() ||
      type->size > bytes.size() - offset) return false;
  // Resolve data variable addresses only through ELF relocations.
  if (value->kind == ck_address && type->kind == tk_pointer) {
    const auto &address = value->variant.address;
    if (type->size != 8 || relocations == nullptr ||
        address.kind != abk_variable || address.variant.variable == nullptr ||
        address.is_object_reflection)
      return false;
    // The 4 KiB image bound and disjoint eight-byte slots already limit
    // relocations to 512; an independent 256-entry cap rejected valid data.
    static_assert(MAX_NATIVE_AGGREGATE_IMAGE_BYTES / 8 == 512);
    const auto delta = address.offset;
    if (delta < static_cast<decltype(delta)>(
                    std::numeric_limits<int64_t>::min()) ||
        delta > static_cast<decltype(delta)>(
                    std::numeric_limits<int64_t>::max()))
      return false;
    // Check entire relocation ranges rather than only equal start offsets.
    // Both start positions are inside the already validated 4 KiB image.
    for (const auto &prior : *relocations) {
      if (!aggregate_subobject_fits(bytes.size(), prior.byte_offset, 8))
        return false;
      if (offset < prior.byte_offset + 8 && prior.byte_offset < offset + 8)
        return false;
    }
    relocations->push_back({offset, address.variant.variable,
                            static_cast<int64_t>(delta)});
    return true;
  }
  if (value->kind == ck_integer &&
      type->kind == tk_integer && type->size <= 8) {
    a_boolean overflow = FALSE;
    uint64_t bits = 0;
    if (int_constant_is_signed(value)) {
      a_host_large_integer v = value_of_integer_constant(value, &overflow);
      if (overflow || sizeof(v) > sizeof(int64_t)) return false;
      bits = static_cast<uint64_t>(static_cast<int64_t>(v));
    } else {
      a_host_large_unsigned v =
          unsigned_value_of_integer_constant(value, &overflow);
      if (overflow || v > std::numeric_limits<uint64_t>::max()) return false;
      bits = static_cast<uint64_t>(v);
    }
    for (size_t i = 0; i < type->size; ++i)
      bytes[offset + i] = static_cast<uint8_t>(bits >> (8 * i));
    return true;
  }
  if (value->kind == ck_string && type->kind == tk_array &&
      type->variant.array.element_type != nullptr &&
      type->variant.array.element_type->size == 1) {
    const size_t n = value->variant.string.length;
    if (n > type->size || value->variant.string.value == nullptr) return false;
    std::memcpy(bytes.data() + offset, value->variant.string.value, n);
    return true;
  }
  if (value->kind != ck_aggregate) return false;
  a_constant_ptr current = value->variant.aggregate.first_constant;
  if (type->kind == tk_array) {
    if (type->variant.array.is_variable_size_array ||
        type->variant.array.is_template_dependent_size_array) return false;
    a_type_ptr element = type->variant.array.element_type;
    if (element == nullptr || element->size == 0) return false;
    uint64_t index = 0;
    const uint64_t count = type->variant.array.variant.number_of_elements;
    unsigned visits = 0;
    while (current != nullptr && visits++ < MAX_NATIVE_AGGREGATE_INITIALIZER_VISITS) {
      if (current->kind == ck_init_repeat) {
        // EDG compresses repeated array constants (often implicit zeros).
        // Do not implement multidimensional designated range tails by guess.
        const auto &repeat=current->variant.init_repeat;
        if (repeat.count==0 || repeat.count>count-index ||
            repeat.multidimensional_aggr_tail_not_repeated ||
            repeat.constant==nullptr) return false;
        for (uint64_t r=0; r<repeat.count; ++r) {
          const uint64_t element_index=index+r;
          if (element_index >= count ||
              element_index > (bytes.size()-offset)/element->size) return false;
          if (!constant_initializer_image(repeat.constant,element,
                 offset+static_cast<size_t>(element_index*element->size),
                 bytes,depth+1,relocations)) return false;
        }
        index+=repeat.count;
        current=current->next;
        continue;
      }
      if (current->kind == ck_designator) {
        if (current->variant.designator.is_generic ||
            current->variant.designator.is_field_designator) return false;
        index = current->variant.designator.variant.array_element;
        if (index >= count) return false;
        if (!constant_designated_value(current, type, offset, bytes,
                                       depth + 1, relocations)) return false;
      } else {
        if (index >= count || index > (bytes.size() - offset) / element->size)
          return false;
        if (!constant_designated_value(current, element,
                         offset + static_cast<size_t>(index * element->size),
                         bytes, depth + 1, relocations)) return false;
      }
      ++index;
    }
    return current == nullptr;
  }
  if (type->kind == tk_struct || type->kind == tk_union) {
    a_field_ptr field = type->variant.class_struct_union.field_list;
    unsigned visits = 0;
    while (current != nullptr && visits++ < MAX_NATIVE_AGGREGATE_INITIALIZER_VISITS) {
      if (current->kind == ck_designator) {
        if (current->variant.designator.is_generic ||
            !current->variant.designator.is_field_designator) return false;
        field = current->variant.designator.variant.field;
        bool valid = false;
        for (a_field_ptr f = type->variant.class_struct_union.field_list;
             f != nullptr; f = f->next)
          if (field == f) {valid = true; break;}
        if (!valid) return false;
        if (!constant_designated_value(current, type, offset, bytes,
                                       depth + 1, relocations)) return false;
      } else {
        if (field == nullptr) return false;
        if (field->is_bit_field) {
          if (!pack_constant_bitfield(current,field,offset,bytes))
            return false;
          current=current->next;
        } else {
          if (field->offset_bit_remainder != 0) return false;
          if (!aggregate_subobject_fits(bytes.size(), offset, field->offset) ||
              !constant_designated_value(current, field->type,
                                         offset + static_cast<size_t>(field->offset),
                                         bytes, depth + 1, relocations))
            return false;
        }
      }
      if (type->kind == tk_union) {
        // One active member, except explicit designators with their own
        // native subobject identity. Do not infer union member overlap.
        if (current != nullptr && current->kind != ck_designator) return false;
      }
      field = field != nullptr ? field->next : nullptr;
    }
    return current == nullptr;
  }
  return false;
}
static bool aggregate_initializer_image(
    a_constant_ptr root, a_type_ptr type, std::string &encoded,
    std::vector<NativeAggregateReloc> *relocations = nullptr) {
  type = object_initializer_type(type);
  if (root == nullptr || type == nullptr || type->incomplete ||
      (type->kind != tk_array && type->kind != tk_struct &&
       type->kind != tk_union) || type->size == 0 ||
      type->size > MAX_NATIVE_AGGREGATE_IMAGE_BYTES) return false;
  std::vector<uint8_t> bytes(static_cast<size_t>(type->size), 0);
  if (!constant_initializer_image(root, type, 0, bytes, 0,
                                  relocations)) return false;
  static constexpr char digits[] = "0123456789abcdef";
  encoded.clear();
  encoded.reserve(bytes.size() * 2);
  for (uint8_t b : bytes) {
    encoded.push_back(digits[b >> 4]);
    encoded.push_back(digits[b & 15]);
  }
  return true;
}

// Variable references need their declared type even if the symbol is
// reached before the file-scope declaration list.
static uint32_t emit_variable_type(a_variable_ptr var);
static uint32_t emit_routine_type(a_routine_ptr routine);
static void emit_statements(a_statement_ptr head, const std::string &function,
                            const std::string &parent, const std::string &edge,
                            unsigned depth, bool single = false);

// A bounded structural snapshot of function statement trees. The path is
// sequence-indexed, so parents can be recovered without exporting IL pointers.
// Each expression receives a local path and parent path. Operation operands
// are traversed in EDG list order. Non-operation kinds remain opaque.
// Copy the exact EDG-host IEEE image, not the approximate value returned by
// floating conversion routines. EDG's float_pt.c stores standard binary32
// and binary64 values in the first bytes of an_internal_float_value.
static bool standard_float_image(a_constant_ptr c, a_type_ptr ty,
                                 uint32_t &width, uint64_t &bits)
{
  if (c == nullptr || ty == nullptr || c->kind != ck_float ||
      ty->kind != tk_float || !TARG_HAS_IEEE_FLOATING_POINT ||
      !std::numeric_limits<float>::is_iec559 ||
      !std::numeric_limits<double>::is_iec559) return false;
  if (ty->variant.float_kind == fk_float && ty->size == 4 &&
      sizeof(float) == 4) {
    uint32_t raw = 0;
    std::memcpy(&raw, &c->variant.float_value, sizeof(raw));
    width = 32;
    bits = raw;
    return true;
  }
  if (ty->variant.float_kind == fk_double && ty->size == 8 &&
      sizeof(double) == 8) {
    uint64_t raw = 0;
    std::memcpy(&raw, &c->variant.float_value, sizeof(raw));
    width = 64;
    bits = raw;
    return true;
  }
  return false;
}
static void emit_expr(an_expr_node_ptr node, const std::string &function,
                      const std::string &stmt_path, const std::string &path,
                      const std::string &parent, unsigned depth)
{
  if (node == nullptr || current_callback == nullptr) return;
  if (depth > 64) { report_incomplete_snapshot(); return; }
  emit_expression_source(function, stmt_path, 0, path, node->position);
  const unsigned op = node->kind == enk_operation ?
      static_cast<unsigned>(node->variant.operation.kind) : 0;
  const unsigned ty = node->type == nullptr ? 0 :
      static_cast<unsigned>(node->type->kind);
  std::string referenced;
  uint32_t symbol = 0;
  if (node->kind == enk_variable && node->variant.variable.ptr != nullptr) {
    referenced = declaration_name(node->variant.variable.ptr->source_corresp, false);
    symbol = emit_symbol(node->variant.variable.ptr, 2, referenced);
    emit_variable_type(node->variant.variable.ptr);
  } else if (node->kind == enk_routine && node->variant.routine.ptr != nullptr) {
    referenced = declaration_name(node->variant.routine.ptr->source_corresp, true, true);
    symbol = emit_symbol(node->variant.routine.ptr, 1, referenced);
    emit_routine_type(node->variant.routine.ptr);
  } else if (node->kind == enk_field && node->variant.field.ptr != nullptr) {
    const char *field = source_name(node->variant.field.ptr->source_corresp);
    if (field != nullptr) referenced = field;
    symbol = emit_symbol(node->variant.field.ptr, 3, referenced);
  }
  // Report target type layout only when EDG has a complete nonzero-sized type.
  const bool layout_known = node->type != nullptr &&
                             !node->type->incomplete && node->type->size != 0;
  const bool has_constant = node->kind == enk_constant &&
      node->variant.constant.ptr != nullptr;
  bool has_integer = false;
  int64_t integer_value = 0;
  if (node->kind == enk_constant && node->variant.constant.ptr != nullptr &&
      node->variant.constant.ptr->kind == ck_integer) {
    a_constant_ptr constant = node->variant.constant.ptr;
    a_boolean overflow = FALSE;
    if (int_constant_is_signed(constant)) {
      a_host_large_integer value = value_of_integer_constant(constant, &overflow);
      if (!overflow && sizeof(value) <= sizeof(int64_t)) {
        integer_value = static_cast<int64_t>(value);
        has_integer = true;
      }
    } else {
      a_host_large_unsigned value =
          unsigned_value_of_integer_constant(constant, &overflow);
      if (!overflow && sizeof(value) <= sizeof(uint64_t)) {
        // Carry the full 64-bit bit pattern of unsigned C constants.
        // The Rust wire value is a signed i64 carrier, not the C type.
        uint64_t bits = static_cast<uint64_t>(value);
        std::memcpy(&integer_value, &bits, sizeof(bits));
        has_integer = true;
      }
    }
  }
  a_type_ptr related_type = nullptr;
  unsigned relationship = 0; // 1=pointee, 2=element, 3=return
  if (node->type != nullptr) {
    if (node->type->kind == tk_pointer) {
      related_type = node->type->variant.pointer.type;
      relationship = 1;
    } else if (node->type->kind == tk_array) {
      related_type = node->type->variant.array.element_type;
      relationship = 2;
    } else if (node->type->kind == tk_routine) {
      related_type = node->type->variant.routine.return_type;
      relationship = 3;
    }
  }
  const uint32_t type_id = emit_type(node->type, 0);
  // Native narrow C string literal. EDG can expose ck_string either as
  // the complete array or as an already-decayed *pointer-typed* constant.
  // Preserve the EDG-owned bytes at the exact original expression path;
  // never assume that a ck_address or an arbitrary pointer is a literal.
  if (has_constant && type_id != 0 && node->type != nullptr &&
      node->variant.constant.ptr->kind == ck_string &&
      !node->type->incomplete &&
      (node->type->kind == tk_array || node->type->kind == tk_pointer)) {
    const bool array = node->type->kind == tk_array;
    a_type_ptr element = object_initializer_type(
        array ? node->type->variant.array.element_type
              : node->type->variant.pointer.type);
    if (element != nullptr && element->size == 1) {
      const a_constant_ptr constant = node->variant.constant.ptr;
      const size_t n = constant->variant.string.length;
      const size_t size = array ? static_cast<size_t>(node->type->size) : n;
      if (size > 0 && size <= MAX_NATIVE_AGGREGATE_IMAGE_BYTES && n <= size &&
          constant->variant.string.value != nullptr) {
        std::vector<uint8_t> image(size, 0);
        std::memcpy(image.data(), constant->variant.string.value, n);
        // The literal object includes its C NUL sentinel. For arrays,
        // exact EDG array extent authorizes any terminal zero fill; for
        // already-decayed pointers require the sentinel in the image.
        if (image.back() == 0) {
          static constexpr char digits[] = "0123456789abcdef";
          std::string hex;
          hex.reserve(image.size() * 2);
          for (uint8_t byte : image) {
            hex.push_back(digits[byte >> 4]);
            hex.push_back(digits[byte & 15]);
          }
          emit_record(62, {txt(function), txt(stmt_path), txt(path),
                           num(type_id), txt(hex)});
        }
      }
    }
  }
  // The ordinary C expression puts("literal") is represented by EDG
  // as ck_address/abk_constant of an underlying ck_string array. EDG
  // explicitly assigns stable identity to that constant in memory.
  // Capture only this exact direct address with zero displacement;
  // arbitrary constant pointers and temporary/global addresses remain
  // unsupported. This intentionally excludes string address comparisons
  // until source identity is modeled across all occurrences.
  if (has_constant && type_id != 0 && node->type != nullptr &&
      node->type->kind == tk_pointer &&
      node->variant.constant.ptr->kind == ck_address) {
    const auto &address=node->variant.constant.ptr->variant.address;
    if (address.kind == abk_constant && address.offset == 0 &&
        !address.one_past_the_end && !address.is_object_reflection &&
        address.subobject_path == nullptr &&
        address.variant.constant != nullptr &&
        address.variant.constant->kind == ck_string) {
      a_constant_ptr original=address.variant.constant;
      a_type_ptr array=original->type;
      a_type_ptr pointee=object_initializer_type(
          node->type->variant.pointer.type);
      a_type_ptr element=array!=nullptr && array->kind==tk_array
          ? object_initializer_type(array->variant.array.element_type)
          : nullptr;
      if (array != nullptr && array->kind == tk_array &&
          !array->incomplete && pointee != nullptr &&
          pointee->size == 1 && element != nullptr &&
          element->size == 1 &&
          array->size>0 && array->size<=MAX_NATIVE_AGGREGATE_IMAGE_BYTES) {
        const size_t size=static_cast<size_t>(array->size);
        const size_t n=original->variant.string.length;
        if (n<=size && original->variant.string.value!=nullptr) {
          std::vector<uint8_t> image(size,0);
          std::memcpy(image.data(),original->variant.string.value,n);
          if (image.back()==0) {
            static constexpr char digits[]="0123456789abcdef";
            std::string hex;
            hex.reserve(size*2);
            for (uint8_t byte:image) {
              hex.push_back(digits[byte>>4]);
              hex.push_back(digits[byte&15]);
            }
            emit_record(62,{txt(function),txt(stmt_path),txt(path),
                            num(type_id),txt(hex)});
          }
        }
      }
    }
  }
  // M14: path-keyed constant-expression IEEE image (wire event 58).
  uint32_t fp_width = 0;
  uint64_t fp_bits = 0;
  if (has_constant && type_id != 0 &&
      standard_float_image(node->variant.constant.ptr, node->type,
                           fp_width, fp_bits)) {
    emit_record(58, {txt(function), txt(stmt_path), txt(path),
                     num(type_id), num(fp_width), num(fp_bits)});
  }
  if (has_constant &&
      node->variant.constant.ptr->kind == ck_address &&
      node->variant.constant.ptr->variant.address.kind == abk_variable) {
    a_constant_ptr constant = node->variant.constant.ptr;
    a_variable_ptr variable = constant->variant.address.variant.variable;
    if (variable != nullptr && !constant->variant.address.is_object_reflection) {
      const uint32_t target = emit_symbol(variable, 2,
          declaration_name(variable->source_corresp, false));
      emit_variable_type(variable);
      // EDG gives a signed *byte* displacement, not an element index.
      const auto offset = constant->variant.address.offset;
      if (offset >= static_cast<decltype(offset)>(
                        std::numeric_limits<int64_t>::min()) &&
          offset <= static_cast<decltype(offset)>(
                        std::numeric_limits<int64_t>::max())) {
        emit_record(48, {txt(function), txt(stmt_path), txt(path),
                         num(target), signed_num(static_cast<int64_t>(offset))});
      }
    }
  }
  // Some routine designators are already folded to ck_address/abk_routine
  // by EDG (for example, both arms of a function-pointer conditional).
  // Record the exact routine symbol, never treat a function as data bytes.
  if (has_constant && node->variant.constant.ptr->kind == ck_address) {
    const auto &address = node->variant.constant.ptr->variant.address;
    if (address.kind == abk_routine && address.variant.routine != nullptr &&
        address.offset == 0 && !address.is_object_reflection) {
      a_routine_ptr target = address.variant.routine;
      const uint32_t id = emit_symbol(target, 1,
          declaration_name(target->source_corresp, true, true));
      emit_record(55, {txt(function), txt(stmt_path), txt(path), num(id)});
    }
  }
  emit_record(6, {txt(function), txt(stmt_path), txt(path),
                  parent.empty() ? absent() : txt(parent),
                  node->kind == enk_operation ? num(op) : absent(),
                  node->type != nullptr ? num(ty) : absent(),
                  referenced.empty() ? absent() : txt(referenced),
                  layout_known ? num(node->type->size) : absent(),
                  layout_known ? num(node->type->alignment) : absent(),
                  has_constant ? num(node->variant.constant.ptr->kind) : absent(),
                  has_integer ? signed_num(integer_value) : absent(),
                  relationship ? num(relationship) : absent(),
                  related_type ? num(related_type->kind) : absent(),
                  type_id ? num(type_id) : absent(),
                  symbol ? num(symbol) : absent()},
              static_cast<uint32_t>(node->kind));
  // EDG may wrap an illegal implicit C pointer conversion in eok_cast,
  // leaving only the converted result type visible to the call operand
  // validator (e.g. int** -> const int**). Preserve the original C pointer
  // TypeId, but ONLY for genuinely compiler-generated casts. User-written
  // explicit casts must retain their separate C semantics.
  if (node->kind == enk_operation &&
      node->variant.operation.kind == eok_cast &&
      node->compiler_generated && node->type != nullptr) {
    an_expr_node_ptr source = node->variant.operation.operands;
    if (source != nullptr && source->type != nullptr &&
        object_initializer_type(node->type) != nullptr &&
        object_initializer_type(source->type) != nullptr &&
        object_initializer_type(node->type)->kind == tk_pointer &&
        object_initializer_type(source->type)->kind == tk_pointer) {
      const uint32_t original = emit_type(source->type, depth + 1);
      if (original != 0) {
        emit_record(RECORD_IMPLICIT_POINTER_CAST,
                    {txt(function), txt(stmt_path), txt(path), num(original)});
      }
    }
  }
  const unsigned call_offset = call_argument_offset(node);
  if (call_offset != 0) {
    // Event 18: call site (function, statement, expression path,
    // first argument operand index). The callee is always operand zero.
    emit_record(18, {txt(function), txt(stmt_path), txt(path), num(call_offset)});
    // Record 78 exports EDG's *original* member-call dispatch bit,
    // not a reconstructed Itanium virtual slot. After lowering, eok_call
    // may itself be the output of virtual dispatch, so kind=0 is
    // "ordinary or already lowered" rather than proof of nonvirtual.
    // 0=ordinary or lowered eok_call; 1=direct member-call node;
    // 2=pointer-to-member call (its runtime target may be virtual).
    uint32_t dispatch_kind = 0;
    uint32_t virtual_dispatch = 0;
    const an_expr_operator_kind original_op=node->variant.operation.kind;
    switch (original_op) {
      case eok_dot_member_call:
      case eok_points_to_member_call:
        dispatch_kind = 1;
        virtual_dispatch = node->variant.operation.is_virtual_call ? 1 : 0;
        break;
      case eok_dot_pm_call:
      case eok_points_to_pm_call:
        dispatch_kind = 2;
        break;
      default: break;
    }
    emit_record(RECORD_NATIVE_CALL_DISPATCH, {
        txt(function),txt(stmt_path),txt(path),
        num(static_cast<unsigned>(original_op)),
        num(dispatch_kind),num(virtual_dispatch)});
    // Event 77 is an *additional native-IL source ownership witness* for
    // the callee expression at this exact callsite. Classification:
    // 1 = direct EDG routine, 2 = exact ck_address/abk_routine constant,
    // 3 = otherwise indirect. Importantly 3 is NOT proof of virtual
    // dispatch or any vtable slot; standard C function pointers also
    // use this form. Keep exact type and source symbol where present.
    // A missing operand is an incomplete native snapshot, not a call.
    an_expr_node_ptr callee = node->variant.operation.operands;
    if (callee == nullptr) {
      report_incomplete_snapshot();
    } else {
      uint32_t form = 3, callee_symbol = 0;
      if (callee->kind == enk_routine &&
          callee->variant.routine.ptr != nullptr) {
        a_routine_ptr routine = callee->variant.routine.ptr;
        callee_symbol = emit_symbol(routine, 1,
            declaration_name(routine->source_corresp, true, true));
        form = 1;
      } else if (callee->kind == enk_constant &&
                 callee->variant.constant.ptr != nullptr &&
                 callee->variant.constant.ptr->kind == ck_address) {
        const auto &address = callee->variant.constant.ptr->variant.address;
        if (address.kind == abk_routine &&
            address.variant.routine != nullptr &&
            address.offset == 0 && !address.is_object_reflection) {
          a_routine_ptr routine = address.variant.routine;
          callee_symbol = emit_symbol(routine, 1,
              declaration_name(routine->source_corresp, true, true));
          form = 2;
        }
      }
      const uint32_t callee_type = emit_type(callee->type, depth + 1);
      emit_record(RECORD_CALL_TARGET_PROVENANCE, {
          txt(function), txt(stmt_path), txt(path), num(form),
          callee_type ? num(callee_type) : absent(),
          callee_symbol ? num(callee_symbol) : absent()});
      // This optional native witness pairs source hook callbacks *before*
      // and *after* EDG's same lower_virtual_function_call rewrite. The
      // post-hook names the exact rewritten eok_call and its callee node:
      // any missing/ambiguous/mismatched post-rewrite pointer yields NO event.
      // It still does not establish virtual slot encoding, thunks, receiver
      // adjustment, object lifetime, or permission to call.
      if (form == 3 && original_op == eok_call) {
        auto observed = postlower_virtual_calls.find(node);
        if (observed != postlower_virtual_calls.end() &&
            !observed->second.ambiguous &&
            observed->second.rewritten_callee == callee &&
            !observed->second.source.ambiguous &&
            observed->second.source.routine != nullptr) {
          const PrelowerVirtualCall &origin = observed->second.source;
          auto *routine = static_cast<a_routine_ptr>(
              const_cast<void *>(origin.routine));
          auto *receiver_type = static_cast<a_type_ptr>(
              const_cast<void *>(origin.receiver_type));
          const uint32_t original_symbol = emit_symbol(routine, 1,
              declaration_name(routine->source_corresp, true, true));
          const uint32_t original_receiver_type = receiver_type != nullptr ?
              emit_type(receiver_type, depth + 1) : 0;
          if (original_symbol == 0) {
            report_incomplete_snapshot();
          } else {
            emit_record(RECORD_PRELOWER_VIRTUAL_CALL_ORIGIN, {
                txt(function),txt(stmt_path),txt(path),
                num(original_symbol),
                original_receiver_type ? num(original_receiver_type) : absent(),
                num(origin.original_operator),
                num(callee->next == origin.original_receiver ? 1 : 0)});

            // EDG's get_virtual_function_address produced an exact reusable
            // receiver expression for this virtual call. The new source hook
            // supplies that node; export event 81 only when it is still a
            // *unique direct call argument* of the already proven eok_call.
            // This records expression reuse, not class subobject identity,
            // ABI vtable slot, thunk or adjusted machine `this` value.
            auto *reuse = static_cast<an_expr_node_ptr>(
                const_cast<void *>(observed->second.receiver_reused_for_call));
            if (reuse != nullptr) {
              unsigned index = 0;
              unsigned matches = 0;
              unsigned matched_index = 0;
              for (an_expr_node_ptr argument = callee->next;
                   argument != nullptr && index < 16;
                   argument = argument->next, ++index) {
                if (argument == reuse) {
                  ++matches;
                  matched_index = index;
                }
              }
              if (matches == 1 && index < 16) {
                uint32_t receiver_type_id = emit_type(reuse->type, depth + 1);
                if (receiver_type_id == 0) {
                  report_incomplete_snapshot();
                } else {
                  emit_record(RECORD_VIRTUAL_RECEIVER_REUSE, {
                      txt(function),txt(stmt_path),txt(path),
                      num(receiver_type_id),num(matched_index),
                      num(reuse == origin.original_receiver ? 1 : 0)});
                }
              }
            }

            // Event 82 requires the exact EDG-generated eok_padd parent,
            // original vptr operand and index operand to remain linked in
            // this call's final *callee subtree*. A surviving detached
            // index alone is not evidence of the lookup. Event 80 already
            // binds the original virtual routine to the rewritten call.
            // Still no physical machine slot, thunk, this adjustment or
            // live object's address point is proven.
            if (observed->second.native_entry_seen &&
                observed->second.native_entry.entry_bytes != 0 &&
                callee_contains_exact_native_padd(
                    callee,
                    observed->second.native_entry.entry_expression,
                    observed->second.native_entry.vptr_expression,
                    observed->second.native_entry.index_expression)) {
              emit_record(RECORD_VIRTUAL_ENTRY_INDEX_SELECTION, {
                  txt(function),txt(stmt_path),txt(path),
                  num(observed->second.native_entry.source_index),
                  num(observed->second.native_entry.entry_bytes)});
              // Event 85 is strictly optional source-evaluation linkage:
              // the object passed by EDG into the vptr lookup is still
              // uniquely reachable under that original vptr subtree,
              // and its independently sourced reusable copy remains a
              // unique *direct argument* of this exact lowered call.
              // It does not establish machine subobject layout or thunks.
              if (observed->second.receiver_pair_seen &&
                  observed->second.receiver_pair.reusable_receiver ==
                      observed->second.receiver_reused_for_call &&
                  subtree_contains_unique_native_node(
                      static_cast<an_expr_node_ptr>(const_cast<void *>(
                          observed->second.native_entry.vptr_expression)),
                      observed->second.receiver_pair.vptr_lookup_object)) {
                auto *lookup = static_cast<an_expr_node_ptr>(
                    const_cast<void *>(
                        observed->second.receiver_pair.vptr_lookup_object));
                auto *paired_receiver = static_cast<an_expr_node_ptr>(
                    const_cast<void *>(
                        observed->second.receiver_pair.reusable_receiver));
                unsigned pos = 0, count = 0, matched_pos = 0;
                for (an_expr_node_ptr argument = callee->next;
                     argument != nullptr && pos < 16;
                     argument = argument->next, ++pos) {
                  if (argument == paired_receiver) {
                    ++count;
                    matched_pos = pos;
                  }
                }
                if (count == 1 && pos < 16) {
                  const uint32_t lookup_type =
                      emit_type(lookup->type, depth + 1);
                  if (lookup_type == 0) {
                    report_incomplete_snapshot();
                  } else {
                    emit_record(RECORD_VIRTUAL_VPTR_RECEIVER_PAIR, {
                        txt(function),txt(stmt_path),txt(path),
                        num(lookup_type),num(matched_pos)});
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  // GNU ({...}) owns a real EDG statement subtree. Give each expression
  // occurrence a unique child edge under its enclosing statement so the
  // existing graph can validate and lower declarations in execution order.
  // Never turn a statement expression into a fake constant/value.
  if (node->kind == enk_statement) {
    std::string edge = "gnu" + path;
    for (char &c : edge) if (c == '.') c = '_';
    emit_statements(node->variant.statement, function, stmt_path, edge,
                    depth + 1, true);
  }
  const auto children = children_of(node);
  // Count independently of emitted operands so a missing tail fails closed.
  uint32_t native_child_count = 0;
  for (an_expr_node_ptr operand = children.first;
       operand != nullptr && within_native_list_limit(native_child_count);
       operand = children.linked_list ? operand->next : nullptr,
       ++native_child_count) {}
  emit_record(RECORD_NATIVE_EXPRESSION_CHILD_COUNT,
              {txt(function), txt(stmt_path), txt(path), num(native_child_count)});
  unsigned index = 0;
  for (an_expr_node_ptr operand = children.first;
       operand != nullptr && within_native_list_limit(index);
       operand = children.linked_list ? operand->next : nullptr, ++index) {
    emit_expr(operand, function, stmt_path,
              path + "." + std::to_string(index), path, depth + 1);
  }
}

// EDG stores several control expressions outside a_statement::expr.
// Preserve their own operand trees with keys (function, statement, role, path).
// Roles: 1=for increment, 2=range test, 3=range increment.
static void emit_control_expr(an_expr_node_ptr node, const std::string &function,
                              const std::string &statement, unsigned role,
                              const std::string &path, const std::string &parent,
                              unsigned depth)
{
  if (node == nullptr || current_callback == nullptr) return;
  if (depth > 64) { report_incomplete_snapshot(); return; }
  uint32_t symbol = 0;
  if (node->kind == enk_variable && node->variant.variable.ptr != nullptr) {
    a_variable_ptr var = node->variant.variable.ptr;
    symbol = emit_symbol(var, 2, declaration_name(var->source_corresp, false));
    emit_variable_type(var);
  } else if (node->kind == enk_routine && node->variant.routine.ptr != nullptr) {
    a_routine_ptr routine = node->variant.routine.ptr;
    symbol = emit_symbol(routine, 1, declaration_name(routine->source_corresp, true, true));
    emit_routine_type(routine);
  } else if (node->kind == enk_field && node->variant.field.ptr != nullptr) {
    a_field_ptr field = node->variant.field.ptr;
    const char *name = source_name(field->source_corresp);
    symbol = emit_symbol(field, 3, name == nullptr ? "" : name);
  }
  const uint32_t type_id = emit_type(node->type, 0);
  emit_expression_source(function, statement, role, path, node->position);
  emit_record(27, {txt(function), txt(statement), num(role), txt(path),
                   parent.empty() ? absent() : txt(parent),
                   num(static_cast<unsigned>(node->kind)),
                   node->kind == enk_operation ?
                       num(static_cast<unsigned>(node->variant.operation.kind)) : absent(),
                   type_id ? num(type_id) : absent(),
                   symbol ? num(symbol) : absent()});
  const auto children = children_of(node);
  uint32_t native_child_count = 0;
  for (an_expr_node_ptr child = children.first;
       child != nullptr && within_native_list_limit(native_child_count);
       child = children.linked_list ? child->next : nullptr,
       ++native_child_count) {}
  emit_record(RECORD_NATIVE_CONTROL_CHILD_COUNT,
              {txt(function),txt(statement),num(role),txt(path),num(native_child_count)});
  unsigned index = 0;
  for (an_expr_node_ptr operand = children.first;
       operand != nullptr && within_native_list_limit(index);
       operand = children.linked_list ? operand->next : nullptr, ++index) {
    emit_control_expr(operand, function, statement, role,
                      path + "." + std::to_string(index), path, depth + 1);
  }
}

// A dependent statement is a *single* IL node, whereas block.statements is
// a linked sequence. Never follow dependent->next into the enclosing list.
// Emit each symbol's declared type once, even if it is visible from
// multiple EDG scope lists or was referenced earlier by an expression.
// A compiler-inserted runtime helper may be referenced without appearing in
// the source namespace's routine list. Export its real EDG prototype on the
// first reference, and deduplicate with later scope traversal.
static uint32_t emit_routine_type(a_routine_ptr routine)
{
  if (routine == nullptr) return 0;
  const uint32_t type_id = emit_type(routine->type, 0);
  if (type_id != 0 && emitted_symbol_types.insert(routine).second) {
    const auto found = symbol_ids.find(routine);
    if (found != symbol_ids.end()) {
      emit_record(25, {num(found->second), num(type_id)});
    }
  }
  return type_id;
}

static uint32_t emit_variable_type(a_variable_ptr var)
{
  if (var == nullptr) return 0;
  const uint32_t type_id = emit_type(var->type, 0);
  if (type_id != 0 && emitted_symbol_types.insert(var).second) {
    // The caller has already registered this variable symbol.
    const auto found = symbol_ids.find(var);
    if (found != symbol_ids.end()) {
      emit_record(25, {num(found->second), num(type_id)});
    }
  }
  return type_id;
}

// The IL owns a function-scope tree independently of the statement tree.
// Walk only a routine's own scope region and retain parent relationships.
// The lifetime of all pointers is limited to the synchronous EDG callback.
// Runtime object lifetimes are not equivalent to lexical scopes.
// ID assignment and traversal are limited to one function's live IL region.
static void emit_lifetime_tree(an_object_lifetime_ptr lifetime,
                               uint32_t routine, unsigned parent,
                               unsigned depth)
{
  if (lifetime == nullptr || current_callback == nullptr) return;
  if (current_lifetime_ids.find(lifetime) != current_lifetime_ids.end()) return;
  // Neither an over-deep lifetime tree nor an exhausted ID budget can be
  // represented by silently dropping destructors or scope relationships.
  if (depth > 64 || current_lifetime_ids.size() >= MAX_NATIVE_LIST_ELEMENTS) {
    report_incomplete_snapshot();
    return;
  }
  const uint32_t id = static_cast<uint32_t>(current_lifetime_ids.size());
  current_lifetime_ids.emplace(lifetime, id);
  emit_record(38, {num(routine), num(id),
                   parent == std::numeric_limits<uint32_t>::max() ?
                       absent() : num(parent),
                   num(static_cast<unsigned>(lifetime->kind))});

  // EDG orders this list in destruction order; preserve that order.
  unsigned position = 0;
  std::unordered_set<a_dynamic_init_ptr> seen;
  for (a_dynamic_init_ptr d = lifetime->destructions;
       d != nullptr && within_native_list_limit(position) && seen.insert(d).second;
       d = d->next_in_destruction_list) {
    uint32_t variable = 0, destructor = 0;
    if (d->variable != nullptr) {
      a_variable_ptr v = d->variable;
      const char *raw = source_name(v->source_corresp);
      variable = emit_symbol(v, 2, raw == nullptr ? "<destructible>" :
                             declaration_name(v->source_corresp, false));
      emit_variable_type(v);
    }
    if (d->destructor != nullptr) {
      a_routine_ptr r = d->destructor;
      destructor = emit_symbol(r, 1,
          declaration_name(r->source_corresp, true, true));
    }
    emit_record(41, {num(routine), num(id), num(position++),
                     variable ? num(variable) : absent(),
                     destructor ? num(destructor) : absent()});
  }
  unsigned children = 0;
  for (an_object_lifetime_ptr child = lifetime->child_lifetime;
       child != nullptr && within_native_list_limit(children++); child = child->next) {
    emit_lifetime_tree(child, routine, id, depth + 1);
  }
}

static void prepare_lifetimes(a_scope_ptr body, uint32_t routine)
{
  current_lifetime_ids.clear();
  if (body == nullptr || body->kind != sck_function) return;
  emit_lifetime_tree(body->lifetime, routine,
                     std::numeric_limits<uint32_t>::max(), 0);
  emit_lifetime_tree(body->variant.routine.lifetime_of_local_static_vars,
                     routine, std::numeric_limits<uint32_t>::max(), 0);
  // A scope's lifetime can be distinct from its parent's lifetime
  // (e.g., after labels, temporaries and special static regions).
  std::vector<a_scope_ptr> pending{body};
  std::unordered_set<a_scope_ptr> visited;
  while (!pending.empty() && within_native_list_limit(visited.size())) {
    a_scope_ptr scope = pending.back();
    pending.pop_back();
    if (scope == nullptr || !visited.insert(scope).second) continue;
    if (scope->lifetime != nullptr &&
        current_lifetime_ids.find(scope->lifetime) == current_lifetime_ids.end()) {
      unsigned parent = std::numeric_limits<uint32_t>::max();
      auto prior = current_lifetime_ids.find(scope->lifetime->parent_lifetime);
      if (prior != current_lifetime_ids.end()) parent = prior->second;
      emit_lifetime_tree(scope->lifetime, routine, parent, 0);
    }
    unsigned children = 0;
    for (a_scope_ptr child = scope->scopes;
         child != nullptr && within_native_list_limit(children++); child = child->next) {
      pending.push_back(child);
    }
  }
}

static void emit_local_scope(a_scope_ptr scope, uint32_t routine,
                             a_routine_ptr source_owner,
                             const std::string &function, uint32_t parent,
                             unsigned &next_id,
                             std::unordered_set<a_scope_ptr> &seen_scopes,
                             std::unordered_set<a_variable_ptr> &seen_locals,
                             unsigned depth)
{
  if (scope == nullptr || current_callback == nullptr) return;
  if (seen_scopes.find(scope) != seen_scopes.end()) return;
  if (depth > 64 || next_id >= MAX_NATIVE_LIST_ELEMENTS) {
    report_incomplete_snapshot();
    return;
  }
  seen_scopes.insert(scope);
  const uint32_t id = next_id++;
  const auto known_path = scope_statement_paths.find(scope);
  emit_record(30, {num(routine), num(id),
                   parent == std::numeric_limits<uint32_t>::max() ?
                       absent() : num(parent),
                   num(static_cast<unsigned>(scope->kind)),
                   known_path == scope_statement_paths.end() ?
                       absent() : txt(known_path->second)});
  const auto region = current_lifetime_ids.find(scope->lifetime);
  if (region != current_lifetime_ids.end()) {
    emit_record(39, {num(routine), num(id), num(region->second)});
  }

  for (unsigned group = 0; group < 2; ++group) {
    a_variable_ptr first = group == 0 ?
        scope->nonstatic_variables : scope->variables;
    unsigned count = 0;
    for (a_variable_ptr var = first; var != nullptr && within_native_list_limit(count++);
         var = var->next) {
      if (var->is_parameter || var->is_this_parameter ||
          !seen_locals.insert(var).second) continue;
      const char *raw_name = source_name(var->source_corresp);
      const std::string name = function + "::" +
          (raw_name == nullptr ? "<local>" : raw_name);
      const uint32_t symbol = emit_symbol(var, 2, name);
      const uint32_t ty = emit_variable_type(var);
      if (symbol == 0 || ty == 0) continue;
      // Diagnostic ONLY, enabled by the exclusive compiler-ci oracle.
      // EDG's a_source_correspondence explicitly owns local-to-function
      // and enclosing_routine metadata; never infer routine ownership
      // from a symbol spelling, memory address, or a generic scope list.
      // The ordinary wire and C++ admission checks are unchanged.
      if (std::getenv("EDGCPP_DIAG_SOURCE_LOCALS") != nullptr &&
          raw_name == nullptr && var->type != nullptr &&
          (var->type->kind == tk_class || var->type->kind == tk_struct) &&
          var->type->size >= 128) {
        ResolvedSource source_position{};
        const bool has_position =
            resolve_source(var->source_corresp.decl_position, source_position);
        ResolvedSource type_source_position{};
        const bool has_type_position = resolve_source(
            var->type->source_corresp.decl_position, type_source_position);
        const char *source_type_name =
            source_name(var->type->source_corresp);
        // Distinguish the precise EDG *structural* reason this type
        // cannot enter the existing concrete-class semantic API. The
        // template/nonreal cases must never invoke is_pod_class().
        a_symbol_ptr class_tag = symbol_for(var->type);
        const bool class_tag_valid = class_tag != nullptr &&
            class_tag->kind == sk_class_or_struct_tag;
        const auto *class_supplement = class_tag_valid ?
            class_symbol_supp(class_tag) : nullptr;
        std::fprintf(stderr,
            "EDG_NATIVE_SCOPE_OWNER function=%s local_symbol=%u class_bytes=%lu "
            "scope_kind=%u source_local=%u owner_present=%u owner_matches=%u "
            "parent_present=%u parent_matches=%u position_present=%u "
            "type_position_present=%u source_list=%u init_kind=%u "
            "explicit_init=%u type_name=%s nonreal_class=%u "
            "concrete_root=%u generated=%u storage_class=%u "
            "class_tag_present=%u class_tag_kind=%u class_supplement=%u "
            "class_template=%u class_incomplete=%u local_used=%u "
            "address_taken=%u variable_nonreal=%u prototype_instance=%u "
            "diagnostic_only=true\n",
            function.c_str(), symbol,
            static_cast<unsigned long>(var->type->size),
            static_cast<unsigned>(scope->kind),
            var->source_corresp.is_local_to_function ? 1u : 0u,
            var->source_corresp.enclosing_routine != nullptr ? 1u : 0u,
            var->source_corresp.enclosing_routine == source_owner ? 1u : 0u,
            var->source_corresp.parent_scope != nullptr ? 1u : 0u,
            var->source_corresp.parent_scope == scope ? 1u : 0u,
            has_position ? 1u : 0u,
            has_type_position ? 1u : 0u,
            group, // 0: nonstatic_variables, 1: variables.
            static_cast<unsigned>(var->init_kind),
            var->has_explicit_initializer ? 1u : 0u,
            source_type_name != nullptr ? source_type_name : "<none>",
            var->type->variant.class_struct_union.is_nonreal_class ? 1u : 0u,
            supported_concrete_class_proof_root(var->type) ? 1u : 0u,
            var->compiler_generated ? 1u : 0u,
            static_cast<unsigned>(var->storage_class),
            class_tag != nullptr ? 1u : 0u,
            class_tag != nullptr ? static_cast<unsigned>(class_tag->kind) : 0u,
            class_supplement != nullptr ? 1u : 0u,
            class_supplement != nullptr &&
                class_supplement->class_template != nullptr ? 1u : 0u,
            var->type->incomplete ? 1u : 0u,
            var->used ? 1u : 0u,
            var->address_taken ? 1u : 0u,
            var->is_nonreal ? 1u : 0u,
            var->is_prototype_instantiation ? 1u : 0u);
      }
      const uint32_t flags =
          (var->compiler_generated ? 1u : 0u) |
          // scope->variables can also contain lowered automatic temporaries.
          // Classify storage from the declaration, not list membership.
          ((var->storage_class == sc_static || var->is_thread_local) ? 2u : 0u) |
          (var->is_struct_binding ? 4u : 0u) |
          (var->is_handler_param ? 8u : 0u);
      emit_record(31, {num(routine), num(id), num(symbol), num(ty),
                       num(static_cast<unsigned>(var->storage_class)),
                       num(flags)});
      if (group == 1 &&
          (var->storage_class == sc_static || var->is_thread_local) &&
          raw_name != nullptr && var->source_corresp.is_local_to_function &&
          var->source_corresp.enclosing_routine != nullptr) {
        pending_static_locals.push_back(
            {var, var->source_corresp.enclosing_routine});
      }
    }
  }
  unsigned child_count = 0;
  for (a_scope_ptr child = scope->scopes;
       child != nullptr && within_native_list_limit(child_count++); child = child->next) {
    emit_local_scope(child, routine, source_owner, function, id, next_id,
                     seen_scopes, seen_locals, depth + 1);
  }
}

static void emit_function_locals(a_scope_ptr body, uint32_t routine,
                                 a_routine_ptr source_owner,
                                 const std::string &function)
{
  if (body == nullptr || body->kind != sck_function) return;
  unsigned position = 0;
  unsigned count = 0;
  std::unordered_set<a_variable_ptr> seen_parameters;
  for (a_variable_ptr var = body->variant.routine.parameters;
       var != nullptr && within_native_list_limit(count++); var = var->next) {
    if (var->is_this_parameter || !seen_parameters.insert(var).second) continue;
    const char *raw_name = source_name(var->source_corresp);
    const std::string name = function + "::" +
        (raw_name == nullptr ? "<parameter>" : raw_name);
    const uint32_t symbol = emit_symbol(var, 2, name);
    const uint32_t ty = emit_variable_type(var);
    if (symbol == 0 || ty == 0) continue;
    emit_record(29, {num(routine), num(position++), num(symbol), num(ty),
                     num(var->compiler_generated ? 2u : 0u)});
  }
  // The implicit C++ object parameter is separate from the explicit list.
  a_variable_ptr this_var = body->variant.routine.this_param_variable;
  if (this_var != nullptr && seen_parameters.insert(this_var).second) {
    const uint32_t symbol = emit_symbol(this_var, 2, function + "::<this>");
    const uint32_t ty = emit_variable_type(this_var);
    if (symbol != 0 && ty != 0) {
      emit_record(29, {num(routine), absent(), num(symbol), num(ty),
                       num(1u | (this_var->compiler_generated ? 2u : 0u))});
    }
  }
  unsigned next_id = 0;
  std::unordered_set<a_scope_ptr> seen_scopes;
  std::unordered_set<a_variable_ptr> seen_locals;
  emit_local_scope(body, routine, source_owner, function,
                   std::numeric_limits<uint32_t>::max(), next_id,
                   seen_scopes, seen_locals, 0);
}

static void emit_statements(a_statement_ptr head, const std::string &function,
                            const std::string &parent, const std::string &edge,
                            unsigned depth, bool single)
{
  if (current_callback == nullptr) return;
  if (depth > 64) { report_incomplete_snapshot(); return; }
  uint32_t sibling_count = 0;
  for (a_statement_ptr stmt = head;
       stmt != nullptr && within_native_list_limit(sibling_count);
       stmt = single ? nullptr : stmt->next, ++sibling_count) {}
  emit_record(RECORD_NATIVE_STATEMENT_SIBLING_COUNT,
              {txt(function), parent.empty() ? absent() : txt(parent),
               txt(edge), num(sibling_count)});
  unsigned index = 0;
  for (a_statement_ptr stmt = head; stmt != nullptr && within_native_list_limit(index);
       stmt = single ? nullptr : stmt->next, ++index) {
    std::string path = parent.empty() ? std::to_string(index) :
                         parent + "." + edge + "." + std::to_string(index);
    emit_record(5, {txt(function), txt(path), parent.empty() ? absent() : txt(parent)},
                static_cast<uint32_t>(stmt->kind));
    emit_statement_source(function, path, stmt->position);
    uint32_t flags = (stmt->compiler_generated ? 1u : 0u) |
                     (stmt->is_initialization_guard ? 2u : 0u) |
                     (stmt->is_fallthrough_statement ? 4u : 0u) |
                     (stmt->is_likely ? 8u : 0u) |
                     (stmt->is_unlikely ? 16u : 0u);
#if DO_IL_LOWERING
    flags |= (stmt->lowering_generated ? 32u : 0u) |
             (stmt->is_lowering_boilerplate ? 64u : 0u);
#endif
    emit_record(26, {txt(function), txt(path), num(flags)});
    // Preserve where declarations occur, not only which scope owns them.
    // An stmk_decl can declare multiple entities, some of which are types
    // or static assertions. Only variable entries have a variable identity.
    if (stmt->kind == stmk_decl) {
      unsigned position = 0;
      unsigned traversed = 0;
      for (an_il_entity_list_entry_ptr entity = stmt->variant.decl.entities;
           entity != nullptr && within_native_list_limit(traversed++); entity = entity->next) {
        if (entity->entity.kind != iek_variable || entity->entity.ptr == nullptr)
          continue;
        a_variable_ptr variable =
            reinterpret_cast<a_variable_ptr>(entity->entity.ptr);
        const char *raw = source_name(variable->source_corresp);
        const uint32_t symbol = emit_symbol(
            variable, 2, raw == nullptr ? "<declared>" :
            declaration_name(variable->source_corresp, false));
        const uint32_t ty = emit_variable_type(variable);
        if (symbol != 0 && ty != 0) {
          emit_record(33, {txt(function), txt(path), num(position++),
                           num(symbol), num(ty), num(0)});
        }
      }
    } else if (stmt->kind == stmk_set_vla_size &&
               stmt->variant.vla_dimension != nullptr) {
      // EDG stores a runtime VLA bound in the function scope, not in the
      // array's type. The set-size statement is its execution/evaluation
      // point, including side effects and re-entry through a loop.
      a_vla_dimension_ptr dim = stmt->variant.vla_dimension;
      uint32_t array_type = emit_type(dim->type, 0);
      const bool has_expr = dim->dimension_expr != nullptr;
      if (array_type != 0) {
        emit_record(56, {txt(function), txt(path), num(array_type),
                         num(has_expr ? 1 : 0),
                         num(dim->original_dimension != nullptr ? 1 : 0)});
      }
      if (has_expr && stmt->expr == nullptr) {
        emit_expr(dim->dimension_expr, function, path, "0", "", 0);
      }
    } else if (stmt->kind == stmk_vla_decl &&
               !stmt->variant.vla.is_typedef_decl &&
               stmt->variant.vla.variant.variable != nullptr) {
      a_variable_ptr variable = stmt->variant.vla.variant.variable;
      const char *raw = source_name(variable->source_corresp);
      const uint32_t symbol = emit_symbol(
          variable, 2, raw == nullptr ? "<vla>" :
          declaration_name(variable->source_corresp, false));
      const uint32_t ty = emit_variable_type(variable);
      if (symbol != 0 && ty != 0) {
        // Variably modified pointer declarations are not VLA allocations.
        emit_record(33, {txt(function), txt(path), num(0),
                         num(symbol), num(ty), num(variable->is_vla ? 1 : 0)});
      }
    }
    // Initialization is an execution point, which can differ from the
    // declaration. Preserve the variable link and any representable
    // expression without conflating this with stmk_decl.
    a_dynamic_init_ptr init = nullptr;
    unsigned init_role = 0;
    if (stmt->kind == stmk_init) {
      init = stmt->variant.dynamic_init;
    } else if (stmt->kind == stmk_return) {
      init = stmt->variant.return_dynamic_init;
      init_role = 1;
    } else if (stmt->kind == stmk_stmt_expr_result) {
      init = stmt->variant.stmt_expr_result.dynamic_init;
      init_role = 2;
    }
    if (init != nullptr) {
      uint32_t variable_symbol = 0;
      if (init->variable != nullptr) {
        a_variable_ptr variable = init->variable;
        const char *raw = source_name(variable->source_corresp);
        variable_symbol = emit_symbol(
            variable, 2, raw == nullptr ? "<initialized>" :
            declaration_name(variable->source_corresp, false));
        emit_variable_type(variable);
      }
      const unsigned init_flags =
          (init->follows_an_exec_statement ? 1u : 0u) |
          (init->is_braced_initializer ? 2u : 0u);
      emit_record(34, {txt(function), txt(path),
                       variable_symbol ? num(variable_symbol) : absent(),
                       num(static_cast<unsigned>(init->kind)),
                       num(init_flags), num(init_role)});
      const auto region = current_lifetime_ids.find(init->lifetime);
      uint32_t destructor = 0;
      if (init->destructor != nullptr) {
        a_routine_ptr r = init->destructor;
        destructor = emit_symbol(r, 1,
            declaration_name(r->source_corresp, true, true));
      }
      if (region != current_lifetime_ids.end() || destructor != 0) {
        emit_record(40, {txt(function), txt(path),
                         region == current_lifetime_ids.end() ?
                             absent() : num(region->second),
                         destructor ? num(destructor) : absent()});
      }

      if (stmt->kind == stmk_init) {
        // M14: constant-initialized FP locals have no expression tree.
        // Preserve their exact IEEE image at the native execution site,
        // keyed by the initialized symbol and its authoritative EDG type.
        if (variable_symbol != 0 && init->variable != nullptr &&
            init->kind == dik_constant &&
            init->variant.constant.ptr != nullptr) {
          uint32_t width = 0;
          uint64_t bits = 0;
          a_type_ptr ty = init->variable->type;
          const uint32_t type_id = emit_type(ty, 0);
          if (type_id != 0 &&
              standard_float_image(init->variant.constant.ptr, ty,
                                   width, bits)) {
            emit_record(59, {txt(function), txt(path),
                             num(variable_symbol), num(type_id),
                             num(width), num(bits)});
          }
        }
        // Constant scalar initializers do not own an expression tree.
        // Export their exact i64 value at this execution site; never guess
        // zero when a constant cannot be represented.
        if (init->kind == dik_zero) {
          emit_record(45, {txt(function), txt(path), signed_num(0)});
        } else if (init->kind == dik_constant &&
                   init->variant.constant.ptr != nullptr &&
                   init->variant.constant.ptr->kind == ck_integer) {
          a_constant_ptr constant = init->variant.constant.ptr;
          a_boolean overflow = FALSE;
          bool fits = false;
          int64_t value = 0;
          if (int_constant_is_signed(constant)) {
            a_host_large_integer signed_value =
                value_of_integer_constant(constant, &overflow);
            if (!overflow && sizeof(signed_value) <= sizeof(int64_t)) {
              value = static_cast<int64_t>(signed_value);
              fits = true;
            }
          } else {
            a_host_large_unsigned unsigned_value =
                unsigned_value_of_integer_constant(constant, &overflow);
            // The wire field is an i64 *bit carrier*. C unsigned 64-bit
            // constants above INT64_MAX are valid and must retain all bits.
            if (!overflow && unsigned_value <=
                static_cast<a_host_large_unsigned>(
                    std::numeric_limits<uint64_t>::max())) {
              const uint64_t bits = static_cast<uint64_t>(unsigned_value);
              std::memcpy(&value, &bits, sizeof(bits));
              fits = true;
            }
          }
          if (fits) emit_record(45, {txt(function), txt(path), signed_num(value)});
        }
        // A C function-pointer initializer may be folded into a native
        // ck_address/abk_routine constant. Preserve the routine identity,
        // never cast that pointer to an integer literal or infer it by name.
        if (variable_symbol != 0 && init->variable != nullptr &&
            init->kind == dik_constant && init->variant.constant.ptr != nullptr &&
            init->variant.constant.ptr->kind == ck_address) {
          const auto &address = init->variant.constant.ptr->variant.address;
          if (address.kind == abk_routine && address.variant.routine != nullptr &&
              address.offset == 0 && !address.is_object_reflection) {
            a_routine_ptr target = address.variant.routine;
            const uint32_t id = emit_symbol(target, 1,
                declaration_name(target->source_corresp, true, true));
            emit_record(54, {txt(function), txt(path), num(id)});
          }
        }
        // Preserve exact constant aggregate values with their native type,
        // even for partially initialized or designated subobjects.
        if (variable_symbol != 0 && init->variable != nullptr &&
            init->kind == dik_constant && init->variant.constant.ptr != nullptr) {
          std::string hex;
          a_type_ptr ty = init->variable->type;
          if (aggregate_initializer_image(init->variant.constant.ptr, ty, hex)) {
            emit_record(50, {txt(function), txt(path), num(variable_symbol),
                             num(emit_type(ty, 0)), txt(hex)});
          }
        }
      }
      if ((init->kind == dik_expression ||
           init->kind == dik_class_result_via_ctor) &&
          init->variant.expression != nullptr) {
        // Role 4 identifies the initializer tree of this statement.
        emit_control_expr(init->variant.expression, function, path, 4,
                          "0", "", 0);
        // The existing control snapshot does not include constant payloads.
        // Also retain a primary expression for plain initializer statements,
        // enabling checked i64 local initialization in the reference backend.
        if ((stmt->kind == stmk_init ||
             stmt->kind == stmk_stmt_expr_result) && stmt->expr == nullptr)
          emit_expr(init->variant.expression, function, path, "0", "", 0);
      }
    }
    if ((stmt->kind == stmk_goto || stmt->kind == stmk_label) &&
        stmt->variant.label.ptr != nullptr) {
      a_label_ptr label = stmt->variant.label.ptr;
      const char *label_name = source_name(label->source_corresp);
      const uint32_t label_id = emit_symbol(label, 4,
                                             label_name == nullptr ? "" : label_name);
      emit_record(28, {txt(function), txt(path), num(label_id),
                       num(stmt->kind == stmk_label ? 1 : 0)});
      // EDG represents source break/continue as gotos to compiler labels.
      // Native label flags distinguish those from user gotos and switch breaks.
      unsigned loop_kind = 0;
      if (label->continue_label)
        loop_kind = stmt->kind == stmk_goto ? 2 : 4;
      else if (label->switch_break_label)
        loop_kind = stmt->kind == stmk_goto ? 5 : 6;
      else if (label->break_label)
        loop_kind = stmt->kind == stmk_goto ? 1 : 3;
      if (loop_kind != 0)
        emit_record(44, {txt(function), txt(path), num(loop_kind)});
    }
    if (stmt->kind == stmk_switch_case) {
      // A case/default has no executable expression of its own. Preserve
      // native integer values as an i64 bit carrier, including unsigned
      // 64-bit values. Status: 0=default, 1=integer, 2=unsupported.
      a_switch_case_entry_ptr entry = stmt->variant.switch_case.extra_info;
      uint32_t status = 2;
      int64_t bits = 0;
      if (entry != nullptr) {
        if (entry->case_value == nullptr) {
          status = 0;
        } else if (entry->case_value->kind == ck_integer) {
          a_boolean overflow = FALSE;
          if (int_constant_is_signed(entry->case_value)) {
            a_host_large_integer value =
                value_of_integer_constant(entry->case_value, &overflow);
            if (!overflow && sizeof(value) <= sizeof(int64_t)) {
              bits = static_cast<int64_t>(value);
              status = 1;
            }
          } else {
            a_host_large_unsigned value =
                unsigned_value_of_integer_constant(entry->case_value, &overflow);
            if (!overflow &&
                value <= static_cast<a_host_large_unsigned>(
                    std::numeric_limits<uint64_t>::max())) {
              uint64_t raw = static_cast<uint64_t>(value);
              std::memcpy(&bits, &raw, sizeof(bits));
              status = 1;
            }
          }
        }
#if GNU_EXTENSIONS_ALLOWED
        if (entry->range_end != nullptr) status = 2;
#endif
      }
      emit_record(53, {txt(function), txt(path), num(status),
                       status == 1 ? signed_num(bits) : absent()});
    }
    if (stmt->expr != nullptr) {
      emit_expr(stmt->expr, function, path, "0", "", 0);
    }
    if (stmt->kind == stmk_block) {
      if (stmt->variant.block.extra_info != nullptr &&
          stmt->variant.block.extra_info->assoc_scope != nullptr) {
        scope_statement_paths.emplace(
            stmt->variant.block.extra_info->assoc_scope, path);
      }
      emit_statements(stmt->variant.block.statements, function, path, "block", depth + 1);
    } else if (stmt->kind == stmk_if ||
               stmt->kind == stmk_if_consteval ||
               stmt->kind == stmk_if_not_consteval) {
      emit_statements(stmt->variant.if_stmt.then_statement,
                      function, path, "then", depth + 1, true);
      emit_statements(stmt->variant.if_stmt.else_statement,
                      function, path, "else", depth + 1, true);
    } else if (stmt->kind == stmk_constexpr_if) {
      a_constexpr_if_ptr info = stmt->variant.constexpr_if;
      // Even a missing supplement has both empty native branch-list
      // witnesses: otherwise a dropped last event-84 is undetectable.
      emit_statements(info != nullptr ? info->then_statement : nullptr,
                      function, path, "then", depth + 1, true);
      emit_statements(info != nullptr ? info->else_statement : nullptr,
                      function, path, "else", depth + 1, true);
    } else if (stmt->kind == stmk_while ||
               stmt->kind == stmk_end_test_while) {
      emit_statements(stmt->variant.loop_statement,
                      function, path, "body", depth + 1, true);
    } else if (stmt->kind == stmk_for) {
      a_for_loop_ptr info = stmt->variant.for_loop.extra_info;
      if (info != nullptr) {
        if (info->for_init_scope != nullptr) {
          scope_statement_paths.emplace(info->for_init_scope, path);
        }
        emit_statements(info->initialization, function, path, "init", depth + 1, true);
        emit_control_expr(info->increment, function, path, 1, "0", "", 0);
        // The role-1 control snapshot contains no constant values. Retain a
        // parallel executable expression tree on a synthetic statement,
        // preserving the original role-1 snapshot and loop edge identity.
        if (info->increment != nullptr) {
          const std::string increment_path = path + ".increment.0";
          emit_record(5, {txt(function), txt(increment_path), txt(path)},
                      static_cast<uint32_t>(stmk_expr));
          // Synthetic increments have no source flags but still require
          // the mandatory native statement-attributes witness.
          emit_record(26, {txt(function), txt(increment_path), num(0)});
          emit_expr(info->increment, function, increment_path, "0", "", 0);
        }
      }
      emit_statements(stmt->variant.for_loop.statement,
                      function, path, "body", depth + 1, true);
    } else if (stmt->kind == stmk_range_based_for) {
      a_range_based_for_loop_ptr info = stmt->variant.range_based_for_loop.extra_info;
      if (info != nullptr) {
        if (info->range_based_for_scope != nullptr) {
          scope_statement_paths.emplace(info->range_based_for_scope, path);
        }
        if (info->iterator_scope != nullptr) {
          scope_statement_paths.emplace(info->iterator_scope, path);
        }
        emit_control_expr(info->ne_call_expr, function, path, 2, "0", "", 0);
        emit_control_expr(info->incr_call_expr, function, path, 3, "0", "", 0);
      }
      emit_statements(info != nullptr ? info->initialization : nullptr,
                      function, path, "init", depth + 1, true);
      emit_statements(stmt->variant.range_based_for_loop.statement,
                      function, path, "body", depth + 1, true);
    } else if (stmt->kind == stmk_switch) {
      emit_statements(stmt->variant.switch_stmt.body_statement,
                      function, path, "body", depth + 1, true);
    } else if (stmt->kind == stmk_try_block) {
      a_try_supplement_ptr info = stmt->variant.try_block;
      // Independent EDG handler-list cardinality witness. Without it,
      // losing a final catchN list (including its event-84 proof) would
      // make a truncated try/catch tree appear complete.
      uint32_t native_handler_count = 0;
      for (a_handler_ptr handler = info != nullptr ? info->handlers : nullptr;
           handler != nullptr && within_native_list_limit(native_handler_count);
           handler = handler->next, ++native_handler_count) {}
      emit_record(RECORD_NATIVE_TRY_HANDLER_COUNT,
                  {txt(function), txt(path), num(native_handler_count)});
      emit_statements(info != nullptr ? info->statement : nullptr,
                      function, path, "try", depth + 1, true);
      if (info != nullptr) {
        unsigned handler_index = 0;
        for (a_handler_ptr handler = info->handlers;
             handler != nullptr && within_native_list_limit(handler_index);
             handler = handler->next, ++handler_index) {
          emit_statements(handler->statement, function, path,
                          "catch" + std::to_string(handler_index), depth + 1, true);
        }
      }
    }
  }
}

// Independently owned expression nodes for dynamic variable initializers.
static void emit_initializer_expr(an_expr_node_ptr expr, uint32_t owner,
                                  const std::string &path,
                                  const std::string &parent, unsigned depth,
                                  bool constructor = false, uint32_t position = 0)
{
  if (expr == nullptr) return;
  if (depth > 64) { report_incomplete_snapshot(); return; }
  uint32_t symbol = 0;
  if (expr->kind == enk_variable && expr->variant.variable.ptr != nullptr) {
    auto ptr = expr->variant.variable.ptr;
    symbol = emit_symbol(ptr, 2, declaration_name(ptr->source_corresp, false));
  } else if (expr->kind == enk_routine && expr->variant.routine.ptr != nullptr) {
    auto ptr = expr->variant.routine.ptr;
    symbol = emit_symbol(ptr, 1, declaration_name(ptr->source_corresp, true, true));
  } else if (expr->kind == enk_field && expr->variant.field.ptr != nullptr) {
    auto ptr = expr->variant.field.ptr;
    const char *name = source_name(ptr->source_corresp);
    symbol = emit_symbol(ptr, 3, name == nullptr ? "" : name);
  }
  uint32_t type_id = emit_type(expr->type, 0);
  if (constructor) {
    emit_record(23, {num(owner), num(position), txt(path),
                     parent.empty() ? absent() : txt(parent),
                     num(static_cast<uint32_t>(expr->kind)),
                     expr->kind == enk_operation ?
                         num(static_cast<uint32_t>(expr->variant.operation.kind)) : absent(),
                     type_id ? num(type_id) : absent(),
                     symbol ? num(symbol) : absent()});
  } else {
    emit_record(17, {num(owner), txt(path), parent.empty() ? absent() : txt(parent),
                     num(static_cast<uint32_t>(expr->kind)),
                     expr->kind == enk_operation ?
                         num(static_cast<uint32_t>(expr->variant.operation.kind)) : absent(),
                     type_id ? num(type_id) : absent(),
                     symbol ? num(symbol) : absent()});
  }
  const auto children = children_of(expr);
  uint32_t native_child_count = 0;
  for (an_expr_node_ptr child = children.first;
       child != nullptr && within_native_list_limit(native_child_count);
       child = children.linked_list ? child->next : nullptr,
       ++native_child_count) {}
  if (constructor) {
    emit_record(RECORD_NATIVE_CONSTRUCTOR_CHILD_COUNT,
                {num(owner),num(position),txt(path),num(native_child_count)});
  } else {
    emit_record(RECORD_NATIVE_INITIALIZER_CHILD_COUNT,
                {num(owner),txt(path),num(native_child_count)});
  }
  unsigned index = 0;
  for (an_expr_node_ptr operand = children.first;
       operand != nullptr && within_native_list_limit(index);
       operand = children.linked_list ? operand->next : nullptr, ++index) {
    emit_initializer_expr(operand, owner, path + "." + std::to_string(index),
                          path, depth + 1, constructor, position);
  }
}

static void collect_scope(a_scope_ptr scope, const std::string &prefix,
                          unsigned depth)
{
  if (scope == nullptr || current_callback == nullptr) return;
  // Class scopes can be reached through more than one enclosing declaration.
  if (visited_scopes.find(scope) != visited_scopes.end()) return;
  // C++20 accepts deeply nested namespace chains (the bridge CI captures
  // 136 levels). Retain a larger defensive recursion cap, but never return
  // a seemingly complete snapshot when it is exceeded.
  if (depth > 256) { report_incomplete_snapshot(); return; }
  visited_scopes.insert(scope);
  // In the standard EDG configuration this scope-owned list, not the
  // optional separate-routine scheduler, holds the actual initialization
  // entries. Each file or namespace list is explicitly ordered by EDG.
  // Keep scopes distinct; this is *not* a cross-namespace startup order.
  if (scope->kind == sck_file || scope->kind == sck_namespace) {
    const std::string scope_key =
        std::to_string(visited_scopes.size()) + ":" + prefix;
    std::size_t position = 0;
    for (a_dynamic_init_ptr init = scope->dynamic_inits;
         init != nullptr && within_native_list_limit(position);
         init = init->next, ++position) {
      uint32_t variable_id = 0;
      uint32_t destructor_id = 0;
      if (init->variable != nullptr) {
        a_variable_ptr v = init->variable;
        const char *raw = source_name(v->source_corresp);
        variable_id = emit_symbol(v, 2, raw != nullptr ?
            declaration_name(v->source_corresp, false) :
            "<EDG-global-init-variable>");
      }
      if (init->destructor != nullptr) {
        a_routine_ptr r = init->destructor;
        const char *raw = source_name(r->source_corresp);
        destructor_id = emit_symbol(r, 1, raw != nullptr ?
            declaration_name(r->source_corresp, true, true) :
            "<EDG-global-init-destructor>");
      }
      emit_record(68, {txt(scope_key),num(static_cast<unsigned>(position)),
                       variable_id ? num(variable_id) : absent(),
                       destructor_id ? num(destructor_id) : absent(),
                       num(static_cast<unsigned>(init->kind))});
    }
  }

  // Overloaded functions cannot be keyed solely by qualified base name:
  // preserve demangled parameter signatures only when the base is ambiguous.
  std::unordered_map<std::string, unsigned> routine_counts;
  for (a_routine_ptr r = scope->routines; r != nullptr; r = r->next) {
    if (source_name(r->source_corresp) != nullptr) {
      ++routine_counts[declaration_name(r->source_corresp, true)];
    }
  }
  for (a_routine_ptr r = scope->routines; r != nullptr; r = r->next) {
    if (source_name(r->source_corresp) != nullptr) {
      std::string name;
      auto previously_named = routine_names.find(r);
      if (previously_named != routine_names.end()) {
        name = previously_named->second;
      } else {
        std::string base = declaration_name(r->source_corresp, true);
        std::string preferred = routine_counts[base] > 1 ?
            declaration_name(r->source_corresp, true, true) : base;
        name = preferred;
        // Several distinct EDG entries can share a demangled signature,
        // notably ABI-specific destructor variants. Statement identities
        // must remain distinct even when the source spelling is identical.
        unsigned suffix = 2;
        while (!used_routine_names.insert(name).second) {
          name = preferred + " [EDG " + std::to_string(suffix++) + "]";
        }
        routine_names.emplace(r, name);
      }
      const uint32_t routine_type_id = emit_type(r->type, 0);
      uint32_t routine_symbol = emit_symbol(r, 1, name);
      // EDG's lowered IL may put out-of-line member definitions in a
      // file/namespace routine list. Their declaration scope, rather than
      // this traversal scope, identifies the owning class. A nonstatic
      // method's this_class is an additional fallback after lowering.
      a_type_ptr owning_class = nullptr;
      a_scope_ptr declared_in = r->source_corresp.parent_scope;
      if (r->source_corresp.is_class_member && declared_in != nullptr &&
          (declared_in->kind == sck_class_struct_union ||
           declared_in->kind == sck_class_reactivation)) {
        owning_class = declared_in->variant.assoc_type;
      }
      if (owning_class == nullptr && scope->kind == sck_class_struct_union) {
        owning_class = scope->variant.assoc_type;
      }
      if (owning_class == nullptr && r->type != nullptr &&
          r->type->kind == tk_routine &&
          r->type->variant.routine.extra_info != nullptr) {
        owning_class = r->type->variant.routine.extra_info->this_class;
      }
      if (owning_class != nullptr &&
          emitted_member_edges.insert(r).second) {
        const uint32_t owner_type = emit_type(owning_class, 0);
        if (owner_type != 0) {
          emit_record(21, {num(owner_type), num(routine_symbol)});
        }
      }
      // Out-of-line definitions may be visible through multiple IL scope
      // lists. Export their body and entity only once per routine pointer.
      if (owning_class == nullptr) {
        // Static member functions have no this_class. Resolve these from
        // the class declaration names after collecting all nested scopes.
        unresolved_members.push_back({r, routine_symbol, name});
      }
      if (!emitted_routines.insert(r).second) continue;
      // Do not reconstruct the body owner from an arbitrary first-encounter
      // symbol display name: overload and destructor names can differ.
      emit_record(42, {num(routine_symbol), txt(name)});
      if (routine_type_id != 0) {
        emit_routine_type(r);
      }
      // Routine classification and dispatch properties are EDG-native.
      // Bit 9 reflects the implicit this parameter, not a source keyword.
      const bool has_this = r->type != nullptr &&
          r->type->kind == tk_routine &&
          r->type->variant.routine.extra_info != nullptr &&
          r->type->variant.routine.extra_info->has_this_param;
      const uint32_t attributes =
          (r->is_virtual ? 1u : 0u) |
          (r->pure_virtual ? 2u : 0u) |
          (r->override ? 4u : 0u) |
          (r->final ? 8u : 0u) |
          (r->overrides_base_member ? 16u : 0u) |
          (r->is_inline ? 32u : 0u) |
          (r->is_constexpr ? 64u : 0u) |
          (r->defined ? 128u : 0u) |
          (r->is_explicit_constructor ? 256u : 0u) |
          (has_this ? 512u : 0u) |
          (r->compiler_generated ? 1024u : 0u);
      emit_record(22, {num(routine_symbol),
                       num(static_cast<unsigned>(r->special_kind)),
                       num(attributes)});
      // Record 79: EDG-authored *logical* virtual member numbering and
      // class virtual-info location. IA64_ABI in EDG means an Itanium-style
      // class ABI, but an EDG virtual function number is NOT by itself a
      // machine vtable slot, vtable address point or this-adjustment.
      // This source witness remains diagnostic-only: never authorize an
      // indirect call or a live class-object dereference from it.
      if (!C_mode() && r->is_virtual) {
        if (owning_class == nullptr ||
            (owning_class->kind != tk_class &&
             owning_class->kind != tk_struct)) {
          report_incomplete_snapshot();
        } else {
          const uint32_t owner_id = emit_type(owning_class, 0);
          const bool numbered = !r->is_consteval &&
              !r->is_constexpr_intrinsic &&
              r->number.virtual_function != VIRTUAL_FUNCTION_NUMBER_NONE;
          const auto *class_info =
              owning_class->variant.class_struct_union.extra_info;
          const bool has_virtual_info =
              !owning_class->incomplete &&
              owning_class->variant.class_struct_union.any_virtual_functions &&
              class_info != nullptr;
          uint32_t shared_base = 0;
          if (has_virtual_info &&
              class_info->virtual_function_info_base_class != nullptr) {
            a_type_ptr base =
                class_info->virtual_function_info_base_class->type;
            if (base == nullptr) {
              report_incomplete_snapshot();
            } else {
              shared_base = emit_type(base, 0);
            }
          }
          if (owner_id == 0) {
            report_incomplete_snapshot();
          } else {
            emit_record(RECORD_NATIVE_VIRTUAL_MEMBER_INFO, {
                num(routine_symbol), num(owner_id),
                numbered ? num(static_cast<uint64_t>(
                    r->number.virtual_function)) : absent(),
                has_virtual_info ? num(static_cast<uint64_t>(
                    class_info->virtual_function_info_offset)) : absent(),
                shared_base ? num(shared_base) : absent()});
          }
        }
      }
      // EDG's active (non-patch) mode exposes the generated startup body
      // as a compiler-created routine, not as an __link data object. EDG's
      // C-generating backend identifies these functions through the raw
      // IL_LOWERING_INIT_ROUTINE_PREFIX ("__sti__") name convention. Require
      // the *native compiler-generated flag* and a definition; never use
      // demangled user names or treat this as a complete .init_array plan.
#if DO_IL_LOWERING && !SEPARATE_ROUTINES_FOR_FILE_SCOPE_DYNAMIC_INITS
      if (!C_mode() && r->compiler_generated &&
          r->function_def_number != NULL_function_def_number &&
          r->source_corresp.name != nullptr &&
          std::string(r->source_corresp.name).rfind("__sti__", 0) == 0) {
        uint32_t priority = 0;
#if GNU_INIT_PRIORITY_ATTRIBUTE_ALLOWED
        priority = static_cast<uint32_t>(r->init_priority);
#endif
        emit_record(73, {num(routine_symbol), num(priority)});
      }
#endif
      emit_record(1, {txt(name)}, r->function_def_number != NULL_function_def_number);
      if (r->function_def_number != NULL_function_def_number) {
        a_scope_ptr body = scope_for_routine_or_null(r);
        if (body != nullptr) {
          // The function scope owns constructor/destructor subobject
          // initialization entries, ordered by actual execution order.
          unsigned init_position = 0;
          for (a_constructor_init_ptr init = body->variant.routine.constructor_inits;
               init != nullptr && within_native_list_limit(init_position);
               init = init->next, ++init_position) {
            uint32_t target_type = 0;
            uint32_t target_symbol = 0;
            if ((init->kind == cik_virtual_base_class ||
                 init->kind == cik_direct_base_class) &&
                init->variant.base_class != nullptr) {
              target_type = emit_type(init->variant.base_class->type, 0);
            } else if (init->kind == cik_field && init->variant.field != nullptr) {
              a_field_ptr field = init->variant.field;
              const char *field_name = source_name(field->source_corresp);
              target_symbol = emit_symbol(field, 3,
                                          field_name == nullptr ? "" : field_name);
            }
            unsigned flags = (init->compiler_generated ? 1u : 0u) |
                             (init->is_braced ? 2u : 0u) |
                             (init->use_field_initializer ? 4u : 0u) |
                             (init->is_pack_expansion ? 8u : 0u);
            emit_record(20, {num(emit_symbol(r, 1, name)), num(init_position),
                             num(static_cast<unsigned>(init->kind)),
                             target_type ? num(target_type) : absent(),
                             target_symbol ? num(target_symbol) : absent(),
                             num(flags)});
            // Capture initializer expression trees separately from function
            // body statements. Only dynamic-init variants documented to own
            // an expression are safe to dereference here.
            an_expr_node_ptr init_expr = nullptr;
            if (init->initializer != nullptr &&
                (init->initializer->kind == dik_expression ||
                 init->initializer->kind == dik_class_result_via_ctor)) {
              init_expr = init->initializer->variant.expression;
            }
            if (init_expr == nullptr) init_expr = init->source_expr;
            if (init_expr != nullptr) {
              emit_initializer_expr(init_expr, routine_symbol, "0", "",
                                    0, true, init_position);
            }

          }
          // Function memory regions may be recycled by EDG. Do not carry
          // raw scope-pointer-to-statement links across function bodies.
          scope_statement_paths.clear();
          if (body->assoc_block != nullptr) {
            scope_statement_paths.emplace(body, "0");
          }
          prepare_lifetimes(body, routine_symbol);
          emit_statements(body->assoc_block, name, "", "", 0);
          emit_function_locals(body, routine_symbol, r, name);
          current_lifetime_ids.clear();
        }
      }
    }
  }
  for (a_variable_ptr v = scope->variables; v != nullptr; v = v->next) {
    if (source_name(v->source_corresp) != nullptr) {
      std::string name = declaration_name(v->source_corresp, false);
      uint32_t symbol_id = emit_symbol(v, 2, name);
      emit_variable_type(v);
      // 47: file/global storage metadata; keep native classification out
      // of the Rust codegen. A declaration with an explicit initializer is
      // a definition even when its storage class says extern.
      const unsigned linkage = v->storage_class == sc_extern ? 1u :
                               v->storage_class == sc_static ? 2u : 3u;
      const bool is_local = v->source_corresp.is_local_to_function ||
                            name.find(")::") != std::string::npos;
      emit_record(47, {num(symbol_id), num(linkage),
                       num(v->has_explicit_initializer ? 1u : 0u),
                       num(v->is_thread_local ? 1u : 0u),
                       num(is_local ? 1u : 0u)});
      if ((v->storage_class == sc_static || v->is_thread_local) &&
          v->source_corresp.is_local_to_function) {
        if (v->source_corresp.enclosing_routine != nullptr &&
            source_name(v->source_corresp) != nullptr) {
          pending_static_locals.push_back(
              {v, v->source_corresp.enclosing_routine});
        } else if (name.find(")::") != std::string::npos) {
          pending_named_statics.push_back({v, name});
        }
      } else if ((v->storage_class == sc_static || v->is_thread_local) &&
                 name.find(")::") != std::string::npos) {
        // Some EDG lowering modes clear is_local_to_function and the
        // cross-region enclosing_routine pointer on hoisted statics.
        pending_named_statics.push_back({v, name});
      }
      emit_record(14, {num(symbol_id), num(v->init_kind),
                       num(v->has_explicit_initializer ? 1 : 0),
                       num(v->uninitialized ? 1 : 0)});
      // In the optional legacy patch-startup mode EDG records its root
      // as a *routine address* in its compiler-generated "__link" static
      // aggregate. Preserve the exact native function pointer, rather than
      // inferring __sti__ spellings. This records a startup root only; the
      // runtime destruction-registration path is still unproven.
#if DO_IL_LOWERING && USE_PATCH_INIT_STARTUP
      if (!C_mode() && v->lowering_generated &&
          v->storage_class == sc_static &&
          std::string(source_name(v->source_corresp)) == "__link") {
        a_constant_ptr aggregate = v->init_kind == initk_static ?
                                   v->initializer.constant : nullptr;
        if (aggregate == nullptr || aggregate->kind != ck_aggregate ||
            aggregate->variant.aggregate.has_dynamic_init_component) {
          report_incomplete_snapshot();
        } else {
          a_constant_ptr first = aggregate->variant.aggregate.first_constant;
          a_constant_ptr middle = first != nullptr ? first->next : nullptr;
          a_constant_ptr last = middle != nullptr ? middle->next : nullptr;
          if (first == nullptr || middle == nullptr || last == nullptr ||
              last->next != nullptr || middle->kind != ck_address ||
              middle->variant.address.kind != abk_routine ||
              middle->variant.address.variant.routine == nullptr ||
              middle->variant.address.offset != 0 ||
              middle->variant.address.one_past_the_end ||
              middle->variant.address.is_object_reflection) {
            report_incomplete_snapshot();
          } else {
            a_routine_ptr root = middle->variant.address.variant.routine;
            const char *raw = source_name(root->source_corresp);
            const uint32_t startup = emit_symbol(root, 1, raw != nullptr ?
                declaration_name(root->source_corresp, true, true) :
                "<EDG-lowered-startup-routine>");
            if (startup != 0) {
              emit_record(72, {num(symbol_id), num(startup)});
            } else {
              report_incomplete_snapshot();
            }
          }
        }
      }
#endif
      // Only scalar static integer constants representable in i64 are
      // exported as values. Aggregate and dynamic initializers stay opaque.
      if (v->init_kind == initk_static && v->initializer.constant != nullptr &&
          v->initializer.constant->kind == ck_integer) {
        a_constant_ptr c = v->initializer.constant;
        a_boolean overflow = FALSE;
        bool have_value = false;
        int64_t value = 0;
        if (int_constant_is_signed(c)) {
          a_host_large_integer n = value_of_integer_constant(c, &overflow);
          if (!overflow && sizeof(n) <= sizeof(int64_t)) {
            value = static_cast<int64_t>(n);
            have_value = true;
          }
        } else {
          a_host_large_unsigned n = unsigned_value_of_integer_constant(c, &overflow);
          if (!overflow && n <= static_cast<a_host_large_unsigned>(
              std::numeric_limits<int64_t>::max())) {
            value = static_cast<int64_t>(n);
            have_value = true;
          }
        }
        if (have_value) emit_record(15, {num(symbol_id), signed_num(value)});
      }
      // M14: native IEEE scalar file-scope initializer image, not source
      // spelling or an inferred zero. This never authorizes dynamic init.
      if (v->init_kind == initk_static &&
          v->has_explicit_initializer && v->initializer.constant != nullptr) {
        uint32_t width = 0;
        uint64_t bits = 0;
        const uint32_t type_id = emit_type(v->type, 0);
        if (type_id != 0 &&
            standard_float_image(v->initializer.constant, v->type,
                                 width, bits)) {
          emit_record(60, {num(symbol_id), num(type_id),
                           num(width), num(bits)});
        }
      }
      if (v->init_kind == initk_static && v->initializer.constant != nullptr) {
        std::string hex;
        std::vector<NativeAggregateReloc> relocations;
        if (aggregate_initializer_image(v->initializer.constant, v->type,
                                        hex, &relocations)) {
          emit_record(51, {num(symbol_id), num(emit_type(v->type, 0)), txt(hex)});
          for (const auto &reloc : relocations) {
            if (reloc.byte_offset > std::numeric_limits<uint32_t>::max())
              continue;
            const auto target = emit_symbol(
                reloc.target, 2,
                declaration_name(reloc.target->source_corresp, false));
            emit_variable_type(reloc.target);
            emit_record(52, {num(symbol_id), num(reloc.byte_offset),
                             num(target), signed_num(reloc.addend)});
          }
        }
      }
      if (v->init_kind == initk_dynamic && v->initializer.dynamic != nullptr) {
        a_dynamic_init_ptr dyn = v->initializer.dynamic;
        an_expr_node_ptr expr = nullptr;
        if (dyn->kind == dik_expression || dyn->kind == dik_class_result_via_ctor) {
          expr = dyn->variant.expression;
        }
        emit_record(16, {num(symbol_id), num(static_cast<unsigned>(dyn->kind)),
                         expr != nullptr ? num(static_cast<unsigned>(expr->kind)) : absent(),
                         expr != nullptr && expr->kind == enk_operation ?
                             num(static_cast<unsigned>(expr->variant.operation.kind)) : absent()});
        if (expr != nullptr) emit_initializer_expr(expr, symbol_id, "0", "", 0);
      }
      emit_record(2, {txt(name)}, 0);
    }
  }
  for (a_type_ptr t = scope->types; t != nullptr; t = t->next) {
    if (source_name(t->source_corresp) != nullptr) {
      std::string name = declaration_name(t->source_corresp, false);
      const uint32_t named_type_id = emit_type(t, 0);
      if ((t->kind == tk_class || t->kind == tk_struct ||
           t->kind == tk_union) && named_type_id != 0) {
        named_class_types.emplace(name, named_type_id);
      }
      emit_record(3, {txt(name)}, 0);
      if ((t->kind == tk_class || t->kind == tk_struct ||
           t->kind == tk_union) &&
          t->variant.class_struct_union.extra_info != nullptr) {
        a_scope_ptr members = t->variant.class_struct_union.extra_info->assoc_scope;
        if (members != nullptr && members != scope) {
          collect_scope(members, prefix, depth + 1);
        }
      }
    }
  }
  for (a_namespace_ptr ns = scope->namespaces; ns != nullptr; ns = ns->next) {
    if (ns->is_namespace_alias) continue;
    const char *name = ns->source_corresp.name;
    std::string qualified = prefix + (name != nullptr ? name : "<anonymous>");
    emit_record(4, {txt(qualified)});
    collect_scope(ns->variant.assoc_scope, qualified + "::", depth + 1);
  }
}

void back_end(void)
{
  // EDG's actual chosen language dialect, not a filename, flag heuristic
  // or the presence of an already-mangled C++ symbol.
  emit_record(64, {num(C_mode() ? 0 : 1)});
  // Snapshot data while EDG owns the IL. Names are copied by Rust callback.
  collect_scope(il_header.primary_scope, "", 0);

  // EDG explicitly owns this ordered runtime initialization schedule when
  // the separate-file-scope-init configuration is enabled. Preserve the
  // actual routine pointers and order, never reconstruct names/calls from
  // source spelling. These records are diagnostic only: they are not a
  // complete C++ startup/shutdown or ELF admission certificate.
#if SEPARATE_ROUTINES_FOR_FILE_SCOPE_DYNAMIC_INITS
  a_routine_list_entry_ptr init_entry =
      il_header.file_scope_dynamic_init_routines;
  std::size_t init_position = 0;
  for (; init_entry != nullptr && within_native_list_limit(init_position);
       init_entry = init_entry->next, ++init_position) {
    a_routine_ptr routine = init_entry->routine;
    if (routine == nullptr) {
      report_incomplete_snapshot();
      break;
    }
    const char *raw = source_name(routine->source_corresp);
    const std::string name = raw != nullptr ?
        declaration_name(routine->source_corresp, true, true) :
        "<EDG-file-scope-initializer>";
    const uint32_t routine_symbol = emit_symbol(routine, 1, name);
    emit_record(67, {num(static_cast<unsigned>(init_position)),
                     num(routine_symbol)});
  }
  emit_record(66, {num(1), num(init_entry == nullptr ? 1 : 0)});
#else
  emit_record(66, {num(0), num(0)});
#endif

  // After every routine has been registered, recover static locals that
  // EDG moved into the file-scope memory region. Their source correspondence
  // still points to the enclosing routine, but deliberately not to a
  // function/block scope across memory regions. Do not invent a scope ID.
  std::unordered_set<const void *> seen_static;
  for (const PendingStaticLocal &entry : pending_static_locals) {
    if (!seen_static.insert(entry.variable).second ||
        emitted_routines.find(entry.routine) == emitted_routines.end()) {
      continue;
    }
    const auto routine = symbol_ids.find(entry.routine);
    const auto variable = symbol_ids.find(entry.variable);
    if (routine != symbol_ids.end() && variable != symbol_ids.end()) {
      emit_record(32, {num(routine->second), num(variable->second)});
    }
  }
  // Hoisted static locals can lose even their enclosing_routine pointer.
  // A demangled full function signature followed by "::local" is specific
  // enough to restore ownership, but only for a unique routine match.
  for (const PendingNamedStatic &entry : pending_named_statics) {
    if (seen_static.find(entry.variable) != seen_static.end()) continue;
    const void *owner = nullptr;
    bool ambiguous = false;
    for (const auto &candidate : routine_names) {
      if (emitted_routines.find(candidate.first) == emitted_routines.end())
        continue;
      a_routine_ptr routine = static_cast<a_routine_ptr>(
          const_cast<void *>(candidate.first));
      const std::string signature =
          declaration_name(routine->source_corresp, true, true) + "::";
      if (entry.qualified_name.compare(0, signature.size(), signature) == 0) {
        if (owner != nullptr && owner != candidate.first) {
          ambiguous = true;
          break;
        }
        owner = candidate.first;
      }
    }
    if (owner == nullptr || ambiguous) continue;
    const auto routine = symbol_ids.find(owner);
    const auto variable = symbol_ids.find(entry.variable);
    if (routine != symbol_ids.end() && variable != symbol_ids.end()) {
      seen_static.insert(entry.variable);
      emit_record(32, {num(routine->second), num(variable->second)});
    }
  }
  for (const PendingMember &candidate : unresolved_members) {
    if (emitted_member_edges.find(candidate.routine) != emitted_member_edges.end()) {
      continue;
    }
    // Require an exact class-name prefix plus "::", not an arbitrary
    // namespace prefix. The longest match handles nested classes.
    uint32_t owner = 0;
    size_t best_length = 0;
    for (const auto &named_type : named_class_types) {
      const std::string prefix = named_type.first + "::";
      if (prefix.size() > best_length &&
          candidate.name.compare(0, prefix.size(), prefix) == 0) {
        best_length = prefix.size();
        owner = named_type.second;
      }
    }
    if (owner != 0) {
      emitted_member_edges.insert(candidate.routine);
      emit_record(21, {num(owner), num(candidate.symbol)});
    }
  }
}
} // namespace edg

// Optional source hooks at the entrance and exit of EDG's *same*
// lower_virtual_function_call invocation. The first observes the original
// virtual routine/receiver; the second proves exactly which eok_call EDG
// created. Older or partially instrumented archives cannot emit event 80.
// Neither hook changes EDG's IL, proves an ABI slot, nor permits execution.
extern "C" void edgcpp_capture_pre_lower_virtual_call(
    void *call, void *original_routine, void *original_receiver)
{
  using namespace edg;
  if (current_callback == nullptr || call == nullptr ||
      original_routine == nullptr || original_receiver == nullptr) return;
  auto *expr = static_cast<an_expr_node_ptr>(call);
  auto *callee = static_cast<an_expr_node_ptr>(original_routine);
  auto *receiver = static_cast<an_expr_node_ptr>(original_receiver);
  if (expr->kind != enk_operation ||
      !expr->variant.operation.is_virtual_call ||
      expr->variant.operation.operands != callee ||
      callee->next != receiver ||
      callee->kind != enk_routine ||
      callee->variant.routine.ptr == nullptr) return;
  const PrelowerVirtualCall proof = {
      callee->variant.routine.ptr, receiver, receiver->type,
      static_cast<unsigned>(expr->variant.operation.kind),
      original_routine, false};
  for (const void *carrier : {call, original_routine}) {
    auto inserted = prelower_virtual_calls.emplace(carrier, proof);
    if (!inserted.second) inserted.first->second.ambiguous = true;
  }
}

// Observed inside EDG make_vtbl_entry_node when the native
// eok_padd(__vptr, index) is built. Require exact parent and operand
// identities up front; reconfirm their links in the *final* callee.
// Only accept the genuine pre-lowering source virtual-call identity.
// Distinct v2 C symbol: old EDG source archives may still reference
// edgcpp_capture_virtual_entry_index with the original four-argument ABI.
// Leave that old weak symbol undefined rather than calling an incompatible
// function or claiming stronger provenance from an old producer.
extern "C" void edgcpp_capture_virtual_entry_shape(
    void *original_routine_operand, void *entry_expression,
    void *vptr_expression, void *index_expression,
    unsigned long long source_index, unsigned long long entry_bytes)
{
  using namespace edg;
  if (current_callback == nullptr || original_routine_operand == nullptr ||
      entry_expression == nullptr || vptr_expression == nullptr ||
      index_expression == nullptr || entry_bytes == 0 || entry_bytes > 32)
    return;
  auto origin = prelower_virtual_calls.find(original_routine_operand);
  if (origin == prelower_virtual_calls.end() || origin->second.ambiguous ||
      origin->second.original_routine_operand != original_routine_operand ||
      origin->second.routine == nullptr) return;
  auto *entry_node = static_cast<an_expr_node_ptr>(entry_expression);
  auto *vptr_node = static_cast<an_expr_node_ptr>(vptr_expression);
  auto *index_node = static_cast<an_expr_node_ptr>(index_expression);
  auto *routine = static_cast<a_routine_ptr>(
      const_cast<void *>(origin->second.routine));
  if (entry_node->kind != enk_operation ||
      entry_node->variant.operation.kind != eok_padd ||
      entry_node->variant.operation.operands != vptr_node ||
      vptr_node->next != index_node || index_node->next != nullptr ||
      index_node->kind != enk_constant ||
      index_node->variant.constant.ptr == nullptr ||
      routine->number.virtual_function != source_index) return;
  auto added = native_virtual_entry_selections.emplace(
      original_routine_operand, NativeVirtualEntrySelection {
          entry_expression, vptr_expression, index_expression,
          static_cast<uint64_t>(source_index),
          static_cast<uint64_t>(entry_bytes), false});
  if (!added.second) added.first->second.ambiguous = true;
}

// This observer pairs the *same* source virtual routine with EDG's
// lookup-object node and the reusable receiver which is later inserted
// into the rewritten call. A copied expression has a different pointer;
// the final vptr subtree and direct argument are checked again at emission.
extern "C" void edgcpp_capture_virtual_receiver_pair(
    void *original_routine_operand, void *vptr_lookup_object,
    void *reusable_call_receiver)
{
  using namespace edg;
  if (current_callback == nullptr || original_routine_operand == nullptr ||
      vptr_lookup_object == nullptr || reusable_call_receiver == nullptr)
    return;
  const auto origin = prelower_virtual_calls.find(original_routine_operand);
  if (origin == prelower_virtual_calls.end() || origin->second.ambiguous ||
      origin->second.original_routine_operand != original_routine_operand ||
      origin->second.routine == nullptr) return;
  auto added = native_virtual_receiver_pairs.emplace(
      original_routine_operand, NativeVirtualReceiverPair {
          vptr_lookup_object, reusable_call_receiver, false});
  if (!added.second) added.first->second.ambiguous = true;
}

// EDG invokes this only after converting the original member-call node
// into an ordinary indirect eok_call. In the comma-operator case the call
// is the old routine operand, *not* the original enclosing expression.
// Bind the exact rewritten node and its immediate callee pointer while
// both are still live. Matching the pointer later guards against some
// subsequent copies/replacements; it cannot establish complete provenance
// across arbitrary EDG inlining or prove receiver adjustments.
extern "C" void edgcpp_capture_post_lower_virtual_call(
    void *original_call, void *rewritten_call,
    void *receiver_reused_for_call)
{
  using namespace edg;
  if (current_callback == nullptr || original_call == nullptr ||
      rewritten_call == nullptr ||
      receiver_reused_for_call == nullptr) return;
  const auto original = prelower_virtual_calls.find(original_call);
  if (original == prelower_virtual_calls.end() ||
      original->second.ambiguous || original->second.routine == nullptr) return;
  auto *parent = static_cast<an_expr_node_ptr>(original_call);
  auto *lowered = static_cast<an_expr_node_ptr>(rewritten_call);
  if (parent->kind != enk_operation ||
      lowered->kind != enk_operation ||
      lowered->variant.operation.kind != eok_call ||
      lowered->variant.operation.operands == nullptr ||
      lowered->variant.operation.operands->kind == enk_routine) return;
  // EDG either overwrites the original node with the new eok_call, or
  // retains it as a comma expression with the new call as a direct child.
  bool source_rewrite_is_structurally_visible = false;
  if (parent == lowered) {
    source_rewrite_is_structurally_visible =
        parent->variant.operation.kind == eok_call;
  } else if (parent->variant.operation.kind == eok_comma) {
    unsigned examined = 0;
    for (an_expr_node_ptr child = parent->variant.operation.operands;
         child != nullptr && examined++ < 16; child = child->next) {
      if (child == lowered) {
        source_rewrite_is_structurally_visible = true;
        break;
      }
    }
  }
  if (!source_rewrite_is_structurally_visible) return;
  NativeVirtualEntrySelection native_entry = {nullptr, nullptr, nullptr,
                                              0, 0, true};
  bool entry_seen = false;
  const auto native = native_virtual_entry_selections.find(
      original->second.original_routine_operand);
  if (native != native_virtual_entry_selections.end() &&
      !native->second.ambiguous) {
    native_entry = native->second;
    entry_seen = true;
  }
  NativeVirtualReceiverPair receiver_pair = {nullptr, nullptr, true};
  bool receiver_pair_seen = false;
  const auto pair = native_virtual_receiver_pairs.find(
      original->second.original_routine_operand);
  if (pair != native_virtual_receiver_pairs.end() &&
      !pair->second.ambiguous) {
    receiver_pair = pair->second;
    receiver_pair_seen = true;
  }
  auto added = postlower_virtual_calls.emplace(
      rewritten_call,
      PostlowerVirtualCall {
          original->second, lowered->variant.operation.operands,
          receiver_reused_for_call, native_entry, entry_seen,
          receiver_pair, receiver_pair_seen, false});
  if (!added.second) {
    // Even two apparently equal entries are ambiguous across EDG
    // transformations. Never silently reuse a physical node address.
    added.first->second.ambiguous = true;
  }
}

extern "C" int edgcpp_compile(int argc, char **argv,
                               edgcpp_il_callback callback, void *user_data)
{
  if (current_callback != nullptr) {
    return -1; // Recursive invocation not supported.
  }
  type_ids.clear();
  symbol_ids.clear();
  visited_scopes.clear();
  emitted_symbol_types.clear();
  current_lifetime_ids.clear();
  scope_statement_paths.clear();
  emitted_routines.clear();
  emitted_member_edges.clear();
  routine_names.clear();
  used_routine_names.clear();
  unresolved_members.clear();
  named_class_types.clear();
  pending_static_locals.clear();
  pending_named_statics.clear();
  prelower_virtual_calls.clear();
  native_virtual_entry_selections.clear();
  native_virtual_receiver_pairs.clear();
  postlower_virtual_calls.clear();
  current_callback = callback;
  current_user_data = user_data;
  // EDG is built with -fno-exceptions; its callable path returns a status.
  // Do not introduce an exception handler compiled with incompatible flags.
  int status = edg::edg_main(argc, argv);
  current_callback = nullptr;
  current_user_data = nullptr;
  type_ids.clear();
  symbol_ids.clear();
  visited_scopes.clear();
  emitted_symbol_types.clear();
  current_lifetime_ids.clear();
  scope_statement_paths.clear();
  emitted_routines.clear();
  emitted_member_edges.clear();
  routine_names.clear();
  used_routine_names.clear();
  unresolved_members.clear();
  named_class_types.clear();
  pending_static_locals.clear();
  pending_named_statics.clear();
  prelower_virtual_calls.clear();
  native_virtual_entry_selections.clear();
  native_virtual_receiver_pairs.clear();
  postlower_virtual_calls.clear();
  return status;
}

/* Stable semantic discriminants for the optional Rust reference backend.
 * Never duplicate EDG's private numeric enum layout in Rust. */
extern "C" unsigned edgcpp_codegen_kind(unsigned category, unsigned value)
{
  using namespace edg;
  if (category == 1) {
    if (value == static_cast<unsigned>(stmk_return)) return 1;
    if (value == static_cast<unsigned>(stmk_block)) return 2;
    if (value == static_cast<unsigned>(stmk_if)) return 3;
    if (value == static_cast<unsigned>(stmk_decl)) return 4;
    if (value == static_cast<unsigned>(stmk_init)) return 5;
    if (value == static_cast<unsigned>(stmk_expr)) return 6;
    if (value == static_cast<unsigned>(stmk_while)) return 7;
    if (value == static_cast<unsigned>(stmk_end_test_while)) return 8;
    if (value == static_cast<unsigned>(stmk_for)) return 9;
    if (value == static_cast<unsigned>(stmk_goto)) return 10;
    if (value == static_cast<unsigned>(stmk_label)) return 11;
    if (value == static_cast<unsigned>(stmk_stmt_expr_result)) return 12;
    if (value == static_cast<unsigned>(stmk_switch)) return 13;
    if (value == static_cast<unsigned>(stmk_switch_case)) return 14;
    if (value == static_cast<unsigned>(stmk_set_vla_size)) return 15;
    if (value == static_cast<unsigned>(stmk_vla_decl)) return 16;
    if (value == static_cast<unsigned>(stmk_constexpr_if)) return 17;
    if (value == static_cast<unsigned>(stmk_range_based_for)) return 18;
    if (value == static_cast<unsigned>(stmk_try_block)) return 19;
    if (value == static_cast<unsigned>(stmk_if_consteval)) return 20;
    if (value == static_cast<unsigned>(stmk_if_not_consteval)) return 21;
  } else if (category == 2) {
    if (value == static_cast<unsigned>(enk_constant)) return 1;
    if (value == static_cast<unsigned>(enk_variable)) return 2;
    if (value == static_cast<unsigned>(enk_operation)) return 3;
    // GNU statement expressions own embedded blocks, not just operands.
    if (value == static_cast<unsigned>(enk_statement)) return 4;
    if (value == static_cast<unsigned>(enk_field)) return 5;
    // Named C routine designators are values (function addresses), not data.
    if (value == static_cast<unsigned>(enk_routine)) return 6;
  } else if (category == 3) {
    if (value == static_cast<unsigned>(eok_add)) return 1;
    if (value == static_cast<unsigned>(eok_subtract)) return 2;
    if (value == static_cast<unsigned>(eok_multiply)) return 3;
    if (value == static_cast<unsigned>(eok_negate)) return 4;
    if (value == static_cast<unsigned>(eok_eq)) return 5;
    if (value == static_cast<unsigned>(eok_ne)) return 6;
    if (value == static_cast<unsigned>(eok_lt)) return 7;
    if (value == static_cast<unsigned>(eok_le)) return 8;
    if (value == static_cast<unsigned>(eok_gt)) return 9;
    if (value == static_cast<unsigned>(eok_ge)) return 10;
    if (value == static_cast<unsigned>(eok_parens)) return 11;
    if (value == static_cast<unsigned>(eok_unary_plus)) return 12;
    if (value == static_cast<unsigned>(eok_and)) return 13;
    if (value == static_cast<unsigned>(eok_or)) return 14;
    if (value == static_cast<unsigned>(eok_xor)) return 15;
    if (value == static_cast<unsigned>(eok_cast)) return 16;
    if (value == static_cast<unsigned>(eok_bool_cast)) return 17;
    if (value == static_cast<unsigned>(eok_assign)) return 18;
    if (value == static_cast<unsigned>(eok_divide)) return 19;
    if (value == static_cast<unsigned>(eok_remainder)) return 20;
    if (value == static_cast<unsigned>(eok_shiftl)) return 21;
    if (value == static_cast<unsigned>(eok_shiftr)) return 22;
    if (value == static_cast<unsigned>(eok_land)) return 23;
    if (value == static_cast<unsigned>(eok_lor)) return 24;
    if (value == static_cast<unsigned>(eok_question)) return 25;
    if (value == static_cast<unsigned>(eok_comma)) return 26;
    if (value == static_cast<unsigned>(eok_complement)) return 27;
    if (value == static_cast<unsigned>(eok_not)) return 28;
    if (value == static_cast<unsigned>(eok_post_incr)) return 29;
    if (value == static_cast<unsigned>(eok_post_decr)) return 30;
    if (value == static_cast<unsigned>(eok_add_assign)) return 31;
    if (value == static_cast<unsigned>(eok_subtract_assign)) return 32;
    if (value == static_cast<unsigned>(eok_multiply_assign)) return 33;
    if (value == static_cast<unsigned>(eok_divide_assign)) return 34;
    if (value == static_cast<unsigned>(eok_remainder_assign)) return 35;
    if (value == static_cast<unsigned>(eok_shiftl_assign)) return 36;
    if (value == static_cast<unsigned>(eok_shiftr_assign)) return 37;
    if (value == static_cast<unsigned>(eok_and_assign)) return 38;
    if (value == static_cast<unsigned>(eok_or_assign)) return 39;
    if (value == static_cast<unsigned>(eok_xor_assign)) return 40;
    if (value == static_cast<unsigned>(eok_pre_incr)) return 41;
    if (value == static_cast<unsigned>(eok_pre_decr)) return 42;
    // Memory/lvalue operators share stable semantic IDs; do not bake EDG
    // private enum discriminants into Rust.
    if (value == static_cast<unsigned>(eok_address_of)) return 43;
    if (value == static_cast<unsigned>(eok_indirect)) return 44;
    if (value == static_cast<unsigned>(eok_subscript)) return 45;
    if (value == static_cast<unsigned>(eok_array_to_pointer)) return 46;
    if (value == static_cast<unsigned>(eok_padd)) return 47;
    if (value == static_cast<unsigned>(eok_psubtract)) return 48;
    if (value == static_cast<unsigned>(eok_pdiff)) return 49;
    if (value == static_cast<unsigned>(eok_padd_assign)) return 50;
    if (value == static_cast<unsigned>(eok_psubtract_assign)) return 51;
    // The right operand of either field operation is an enk_field
    // carrying an identity exported in the record-field table.
    if (value == static_cast<unsigned>(eok_dot_field)) return 52;
    if (value == static_cast<unsigned>(eok_points_to_field)) return 53;
    // Record 18's first_argument must agree with the actual native call
    // operator, not just with how many operands were copied into the graph.
    if (value == static_cast<unsigned>(eok_call)) return 54;
    if (value == static_cast<unsigned>(eok_dot_member_call) ||
        value == static_cast<unsigned>(eok_points_to_member_call) ||
        value == static_cast<unsigned>(eok_dot_pm_call) ||
        value == static_cast<unsigned>(eok_points_to_pm_call)) return 55;
  } else if (category == 4) {
    if (value == static_cast<unsigned>(tk_void)) return 4;
    if (value == static_cast<unsigned>(tk_integer)) return 1;
    if (value == static_cast<unsigned>(tk_routine)) return 2;
    if (value == static_cast<unsigned>(tk_typeref)) return 3;
    if (value == static_cast<unsigned>(tk_pointer)) return 5;
    if (value == static_cast<unsigned>(tk_array)) return 6;
    // Keep records distinct from unions and C++ classes; neither may be
    // accepted by the conservative C-struct storage path by accident.
    if (value == static_cast<unsigned>(tk_struct)) return 7;
    if (value == static_cast<unsigned>(tk_class)) return 8;
    if (value == static_cast<unsigned>(tk_union)) return 9;
  } else if (category == 5) {
    // Dynamic-initializer kinds, normalized for the reference backend.
    // Only native dik_zero promises full-object zero initialization.
    if (value == static_cast<unsigned>(dik_zero)) return 1;
  }
  return 0;
}
