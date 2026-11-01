/* SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
 * A narrow, synchronous C ABI over EDG's native C++ IL.
 *
 * This file must be compiled with the exact configuration of libedgcpfe.a.
 * Its back_end() replaces the C-generating back end when the archive is linked.
 */
#include "fe_common.h"
#include "const_ints.h"
#include <cstdint>
#include <cstddef>
#include <cstring>
#include <limits>
#include <string>
#include <cxxabi.h>
#include <cstdlib>
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
}

namespace edg {
static edgcpp_field txt(const std::string &s) { return {s.data(), s.size(), 0, 1}; }
static edgcpp_field num(uint64_t n) { return {nullptr, 0, n, 2}; }
static edgcpp_field signed_num(int64_t n) { return {nullptr, 0, static_cast<uint64_t>(n), 3}; }
static edgcpp_field absent() { return {nullptr, 0, 0, 0}; }
static void emit_record(uint32_t kind, std::initializer_list<edgcpp_field> fields, uint32_t aux = 0)
{
  current_callback(current_user_data, kind, fields.begin(), fields.size(), aux);
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
    emit_record(35, {txt(function), txt(path), num(location.seq),
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
    emit_record(37, {txt(function), txt(statement), num(role), txt(path),
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
  unsigned depth = 0;
  for (a_scope_ptr scope = parent; scope != nullptr && depth++ < 128;
       scope = scope->parent) {
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
  emit_record(13, {num(id), num(kind), txt(name)});
  if (kind >= 1 && kind <= 3) {
    // EDG routines, variables and fields all place source_corresp first.
    const auto *correspondence =
        reinterpret_cast<const a_source_correspondence *>(ptr);
    ResolvedSource position{};
    if (resolve_source(correspondence->decl_position, position)) {
      emit_record(36, {num(id), num(position.seq), num(position.column),
                       txt(position.file), num(position.line)});
    }
  }
  return id;
}

// Assign dense IDs during compilation; never export EDG type addresses.
static uint32_t emit_type(a_type_ptr type, unsigned depth)
{
  if (type == nullptr || depth > 64 || current_callback == nullptr) return 0;
  auto existing = type_ids.find(type);
  if (existing != type_ids.end()) return existing->second;
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
  // Native EDG object layout, never reconstructed from field order.
  // An alignment of zero is not a usable layout.
  if (!type->incomplete && type->size > 0 && type->alignment > 0) {
    emit_record(49, {num(id), num(type->size), num(type->alignment)});
  }
  if (type->kind == tk_class || type->kind == tk_struct || type->kind == tk_union) {
    unsigned index = 0;
    for (a_field_ptr field = type->variant.class_struct_union.field_list;
         field != nullptr && index < 10000; field = field->next, ++index) {
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
         base != nullptr && position < 10000;
         base = base->next_direct, ++position) {
      const uint32_t base_id = emit_type(base->type, depth + 1);
      if (base_id != 0) {
        emit_record(19, {num(id), num(position), num(base_id),
                         num(base->is_virtual ? 1 : 0)});
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
         p != nullptr && position < 10000; p = p->next, ++position) {
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
static bool constant_designated_value(
    a_constant_ptr &current, a_type_ptr original_type, size_t offset,
    std::vector<uint8_t> &bytes, unsigned depth,
    std::vector<NativeAggregateReloc> *relocations) {
  if (current == nullptr || depth > 32) return false;
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
    if (selected->offset_bit_remainder != 0) return false;
    element = selected->type;
    child_offset += selected->offset;
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
        address.is_object_reflection || relocations->size() >= 256)
      return false;
    const auto delta = address.offset;
    if (delta < static_cast<decltype(delta)>(
                    std::numeric_limits<int64_t>::min()) ||
        delta > static_cast<decltype(delta)>(
                    std::numeric_limits<int64_t>::max()))
      return false;
    for (const auto &prior : *relocations) {
      if (prior.byte_offset == offset) return false;
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
    while (current != nullptr && visits++ < 4096) {
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
    while (current != nullptr && visits++ < 4096) {
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
          if (!constant_designated_value(current, field->type,
                                         offset + field->offset,
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
      type->size > 4096) return false;
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
static void emit_statements(a_statement_ptr head, const std::string &function,
                            const std::string &parent, const std::string &edge,
                            unsigned depth, bool single = false);

// A bounded structural snapshot of function statement trees. The path is
// sequence-indexed, so parents can be recovered without exporting IL pointers.
// Each expression receives a local path and parent path. Operation operands
// are traversed in EDG list order. Non-operation kinds remain opaque.
static void emit_expr(an_expr_node_ptr node, const std::string &function,
                      const std::string &stmt_path, const std::string &path,
                      const std::string &parent, unsigned depth)
{
  if (node == nullptr || depth > 64 || current_callback == nullptr) return;
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
  const unsigned call_offset = call_argument_offset(node);
  if (call_offset != 0) {
    // Event 18: call site (function, statement, expression path,
    // first argument operand index). The callee is always operand zero.
    emit_record(18, {txt(function), txt(stmt_path), txt(path), num(call_offset)});
  }
  // GNU ({...}) owns a real EDG statement subtree. Give each expression
  // occurrence a unique child edge under its enclosing statement so the
  // existing graph can validate and lower declarations in execution order.
  // Never turn a statement expression into a fake constant/value.
  if (node->kind == enk_statement && node->variant.statement != nullptr) {
    std::string edge = "gnu" + path;
    for (char &c : edge) if (c == '.') c = '_';
    emit_statements(node->variant.statement, function, stmt_path, edge,
                    depth + 1, true);
  }
  const auto children = children_of(node);
  unsigned index = 0;
  for (an_expr_node_ptr operand = children.first;
       operand != nullptr && index < 10000;
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
  if (node == nullptr || current_callback == nullptr || depth > 64) return;
  uint32_t symbol = 0;
  if (node->kind == enk_variable && node->variant.variable.ptr != nullptr) {
    a_variable_ptr var = node->variant.variable.ptr;
    symbol = emit_symbol(var, 2, declaration_name(var->source_corresp, false));
    emit_variable_type(var);
  } else if (node->kind == enk_routine && node->variant.routine.ptr != nullptr) {
    a_routine_ptr routine = node->variant.routine.ptr;
    symbol = emit_symbol(routine, 1, declaration_name(routine->source_corresp, true, true));
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
  unsigned index = 0;
  for (an_expr_node_ptr operand = children.first;
       operand != nullptr && index < 10000;
       operand = children.linked_list ? operand->next : nullptr, ++index) {
    emit_control_expr(operand, function, statement, role,
                      path + "." + std::to_string(index), path, depth + 1);
  }
}

// A dependent statement is a *single* IL node, whereas block.statements is
// a linked sequence. Never follow dependent->next into the enclosing list.
// Emit each symbol's declared type once, even if it is visible from
// multiple EDG scope lists or was referenced earlier by an expression.
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
  if (lifetime == nullptr || depth > 64 || current_callback == nullptr ||
      current_lifetime_ids.size() >= 10000) return;
  if (current_lifetime_ids.find(lifetime) != current_lifetime_ids.end()) return;
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
       d != nullptr && position < 10000 && seen.insert(d).second;
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
       child != nullptr && children++ < 10000; child = child->next) {
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
  while (!pending.empty() && visited.size() < 10000) {
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
         child != nullptr && children++ < 10000; child = child->next) {
      pending.push_back(child);
    }
  }
}

static void emit_local_scope(a_scope_ptr scope, uint32_t routine,
                             const std::string &function, uint32_t parent,
                             unsigned &next_id,
                             std::unordered_set<a_scope_ptr> &seen_scopes,
                             std::unordered_set<a_variable_ptr> &seen_locals,
                             unsigned depth)
{
  if (scope == nullptr || depth > 64 || next_id >= 10000 ||
      !seen_scopes.insert(scope).second) return;
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
    for (a_variable_ptr var = first; var != nullptr && count++ < 10000;
         var = var->next) {
      if (var->is_parameter || var->is_this_parameter ||
          !seen_locals.insert(var).second) continue;
      const char *raw_name = source_name(var->source_corresp);
      const std::string name = function + "::" +
          (raw_name == nullptr ? "<local>" : raw_name);
      const uint32_t symbol = emit_symbol(var, 2, name);
      const uint32_t ty = emit_variable_type(var);
      if (symbol == 0 || ty == 0) continue;
      const uint32_t flags =
          (var->compiler_generated ? 1u : 0u) |
          (group == 1 ? 2u : 0u) |
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
       child != nullptr && child_count++ < 10000; child = child->next) {
    emit_local_scope(child, routine, function, id, next_id,
                     seen_scopes, seen_locals, depth + 1);
  }
}

static void emit_function_locals(a_scope_ptr body, uint32_t routine,
                                 const std::string &function)
{
  if (body == nullptr || body->kind != sck_function) return;
  unsigned position = 0;
  unsigned count = 0;
  std::unordered_set<a_variable_ptr> seen_parameters;
  for (a_variable_ptr var = body->variant.routine.parameters;
       var != nullptr && count++ < 10000; var = var->next) {
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
  emit_local_scope(body, routine, function,
                   std::numeric_limits<uint32_t>::max(), next_id,
                   seen_scopes, seen_locals, 0);
}

static void emit_statements(a_statement_ptr head, const std::string &function,
                            const std::string &parent, const std::string &edge,
                            unsigned depth, bool single)
{
  if (head == nullptr || depth > 64 || current_callback == nullptr) return;
  unsigned index = 0;
  for (a_statement_ptr stmt = head; stmt != nullptr && index < 10000;
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
           entity != nullptr && traversed++ < 10000; entity = entity->next) {
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
        emit_record(33, {txt(function), txt(path), num(0),
                         num(symbol), num(ty), num(1)});
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
      else if (label->break_label && !label->switch_break_label)
        loop_kind = stmt->kind == stmk_goto ? 1 : 3;
      if (loop_kind != 0)
        emit_record(44, {txt(function), txt(path), num(loop_kind)});
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
      if (info != nullptr) {
        emit_statements(info->then_statement, function, path, "then", depth + 1, true);
        emit_statements(info->else_statement, function, path, "else", depth + 1, true);
      }
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
        emit_statements(info->initialization, function, path, "init", depth + 1, true);
        emit_control_expr(info->ne_call_expr, function, path, 2, "0", "", 0);
        emit_control_expr(info->incr_call_expr, function, path, 3, "0", "", 0);
      }
      emit_statements(stmt->variant.range_based_for_loop.statement,
                      function, path, "body", depth + 1, true);
    } else if (stmt->kind == stmk_switch) {
      emit_statements(stmt->variant.switch_stmt.body_statement,
                      function, path, "body", depth + 1, true);
    } else if (stmt->kind == stmk_try_block) {
      a_try_supplement_ptr info = stmt->variant.try_block;
      if (info != nullptr) {
        emit_statements(info->statement, function, path, "try", depth + 1, true);
        unsigned handler_index = 0;
        for (a_handler_ptr handler = info->handlers;
             handler != nullptr && handler_index < 10000;
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
  if (expr == nullptr || depth > 64) return;
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
  unsigned index = 0;
  for (an_expr_node_ptr operand = children.first;
       operand != nullptr && index < 10000;
       operand = children.linked_list ? operand->next : nullptr, ++index) {
    emit_initializer_expr(operand, owner, path + "." + std::to_string(index),
                          path, depth + 1, constructor, position);
  }
}

static void collect_scope(a_scope_ptr scope, const std::string &prefix,
                          unsigned depth)
{
  if (scope == nullptr || current_callback == nullptr || depth > 128) return;
  // Class scopes can be reached through more than one enclosing declaration.
  if (!visited_scopes.insert(scope).second) return;
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
        emit_record(25, {num(routine_symbol), num(routine_type_id)});
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
          (has_this ? 512u : 0u);
      emit_record(22, {num(routine_symbol),
                       num(static_cast<unsigned>(r->special_kind)),
                       num(attributes)});
      emit_record(1, {txt(name)}, r->function_def_number != NULL_function_def_number);
      if (r->function_def_number != NULL_function_def_number) {
        a_scope_ptr body = scope_for_routine_or_null(r);
        if (body != nullptr) {
          // The function scope owns constructor/destructor subobject
          // initialization entries, ordered by actual execution order.
          unsigned init_position = 0;
          for (a_constructor_init_ptr init = body->variant.routine.constructor_inits;
               init != nullptr && init_position < 10000;
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
          emit_function_locals(body, routine_symbol, name);
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
  // Snapshot data while EDG owns the IL. Names are copied by Rust callback.
  collect_scope(il_header.primary_scope, "", 0);
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
  } else if (category == 2) {
    if (value == static_cast<unsigned>(enk_constant)) return 1;
    if (value == static_cast<unsigned>(enk_variable)) return 2;
    if (value == static_cast<unsigned>(enk_operation)) return 3;
    // GNU statement expressions own embedded blocks, not just operands.
    if (value == static_cast<unsigned>(enk_statement)) return 4;
    if (value == static_cast<unsigned>(enk_field)) return 5;
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
