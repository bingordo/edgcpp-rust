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
#include <limits>
#include <string>
#include <cxxabi.h>
#include <cstdlib>
#include <unordered_map>
#include <unordered_set>
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
thread_local std::unordered_set<const void *> emitted_routines;
thread_local std::unordered_set<const void *> emitted_member_edges;
thread_local std::unordered_map<const void *, std::string> routine_names;
thread_local std::unordered_set<std::string> used_routine_names;
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
  if (type->kind == tk_class || type->kind == tk_struct || type->kind == tk_union) {
    unsigned index = 0;
    for (a_field_ptr field = type->variant.class_struct_union.field_list;
         field != nullptr && index < 10000; field = field->next, ++index) {
      uint32_t field_type = emit_type(field->type, depth + 1);
      const char *field_name = source_name(field->source_corresp);
      std::string field_text = field_name == nullptr ? "" : field_name;
      emit_record(10, {num(id), num(index), num(field_type), txt(field_text),
                       num(field->offset), num(field->offset_bit_remainder),
                       num(field->is_bit_field ? field->bit_size : 0)});
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

// A bounded structural snapshot of function statement trees. The path is
// sequence-indexed, so parents can be recovered without exporting IL pointers.
// Each expression receives a local path and parent path. Operation operands
// are traversed in EDG list order. Non-operation kinds remain opaque.
static void emit_expr(an_expr_node_ptr node, const std::string &function,
                      const std::string &stmt_path, const std::string &path,
                      const std::string &parent, unsigned depth)
{
  if (node == nullptr || depth > 64 || current_callback == nullptr) return;
  const unsigned op = node->kind == enk_operation ?
      static_cast<unsigned>(node->variant.operation.kind) : 0;
  const unsigned ty = node->type == nullptr ? 0 :
      static_cast<unsigned>(node->type->kind);
  std::string referenced;
  uint32_t symbol = 0;
  if (node->kind == enk_variable && node->variant.variable.ptr != nullptr) {
    referenced = declaration_name(node->variant.variable.ptr->source_corresp, false);
    symbol = emit_symbol(node->variant.variable.ptr, 2, referenced);
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
      if (!overflow && value <=
          static_cast<a_host_large_unsigned>(std::numeric_limits<int64_t>::max())) {
        integer_value = static_cast<int64_t>(value);
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
  const auto children = children_of(node);
  unsigned index = 0;
  for (an_expr_node_ptr operand = children.first;
       operand != nullptr && index < 10000;
       operand = children.linked_list ? operand->next : nullptr, ++index) {
    emit_expr(operand, function, stmt_path,
              path + "." + std::to_string(index), path, depth + 1);
  }
}

static void emit_statements(a_statement_ptr head, const std::string &function,
                            const std::string &parent, const std::string &edge, unsigned depth)
{
  if (head == nullptr || depth > 64 || current_callback == nullptr) return;
  unsigned index = 0;
  for (a_statement_ptr stmt = head; stmt != nullptr && index < 10000;
       stmt = stmt->next, ++index) {
    std::string path = parent.empty() ? std::to_string(index) :
                         parent + "." + edge + "." + std::to_string(index);
    emit_record(5, {txt(function), txt(path), parent.empty() ? absent() : txt(parent)},
                static_cast<uint32_t>(stmt->kind));
    if (stmt->expr != nullptr) {
      emit_expr(stmt->expr, function, path, "0", "", 0);
    }
    if (stmt->kind == stmk_block) {
      emit_statements(stmt->variant.block.statements, function, path, "block", depth + 1);
    } else if (stmt->kind == stmk_if) {
      emit_statements(stmt->variant.if_stmt.then_statement,
                      function, path, "then", depth + 1);
      emit_statements(stmt->variant.if_stmt.else_statement,
                      function, path, "else", depth + 1);
    } else if (stmt->kind == stmk_while ||
               stmt->kind == stmk_end_test_while) {
      emit_statements(stmt->variant.loop_statement,
                      function, path, "body", depth + 1);
    } else if (stmt->kind == stmk_for) {
      emit_statements(stmt->variant.for_loop.statement,
                      function, path, "body", depth + 1);
    }
  }
}

// Independently owned expression nodes for dynamic variable initializers.
static void emit_initializer_expr(an_expr_node_ptr expr, uint32_t owner,
                                  const std::string &path,
                                  const std::string &parent, unsigned depth)
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
  emit_record(17, {num(owner), txt(path), parent.empty() ? absent() : txt(parent),
                   num(static_cast<uint32_t>(expr->kind)),
                   expr->kind == enk_operation ?
                       num(static_cast<uint32_t>(expr->variant.operation.kind)) : absent(),
                   type_id ? num(type_id) : absent(),
                   symbol ? num(symbol) : absent()});
  const auto children = children_of(expr);
  unsigned index = 0;
  for (an_expr_node_ptr operand = children.first;
       operand != nullptr && index < 10000;
       operand = children.linked_list ? operand->next : nullptr, ++index) {
    emit_initializer_expr(operand, owner, path + "." + std::to_string(index),
                          path, depth + 1);
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
      emit_type(r->type, 0);
      uint32_t routine_symbol = emit_symbol(r, 1, name);
      if (scope->kind == sck_class_struct_union &&
          scope->variant.assoc_type != nullptr &&
          emitted_member_edges.insert(r).second) {
        uint32_t owner_type = emit_type(scope->variant.assoc_type, 0);
        emit_record(21, {num(owner_type), num(routine_symbol)});
      }
      // Out-of-line definitions may be visible through multiple IL scope
      // lists. Export their body and entity only once per routine pointer.
      if (!emitted_routines.insert(r).second) continue;
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
          }
          emit_statements(body->assoc_block, name, "", "", 0);
        }
      }
    }
  }
  for (a_variable_ptr v = scope->variables; v != nullptr; v = v->next) {
    if (source_name(v->source_corresp) != nullptr) {
      std::string name = declaration_name(v->source_corresp, false);
      emit_type(v->type, 0);
      uint32_t symbol_id = emit_symbol(v, 2, name);
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
      emit_type(t, 0);
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
  emitted_routines.clear();
  emitted_member_edges.clear();
  routine_names.clear();
  used_routine_names.clear();
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
  emitted_routines.clear();
  emitted_member_edges.clear();
  routine_names.clear();
  used_routine_names.clear();
  return status;
}
