/* SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
 * A narrow, synchronous C ABI over EDG's native C++ IL.
 *
 * This file must be compiled with the exact configuration of libedgcpfe.a.
 * Its back_end() replaces the C-generating back end when the archive is linked.
 */
#include "fe_common.h"
#include "const_ints.h"
#include <cstdint>
#include <string>
#include <cxxabi.h>
#include <cstdlib>
#include <unordered_map>

extern "C" {
typedef void (*edgcpp_il_callback)(void *user_data, uint32_t kind,
                                    const char *name, uint32_t defined);
}

namespace {
thread_local edgcpp_il_callback current_callback = nullptr;
thread_local void *current_user_data = nullptr;
thread_local std::unordered_map<const void *, uint32_t> type_ids;
}

namespace edg {
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
                                    bool is_routine)
{
#if NEED_NAME_MANGLING
  if (sc.name_has_been_mangled && sc.name != nullptr) {
    int status = 0;
    char *decoded = abi::__cxa_demangle(sc.name, nullptr, nullptr, &status);
    if (status == 0 && decoded != nullptr) {
      std::string qualified(decoded);
      std::free(decoded);
      if (is_routine) {
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
  if (type->kind == tk_class || type->kind == tk_struct || type->kind == tk_union) {
    unsigned index = 0;
    for (a_field_ptr field = type->variant.class_struct_union.field_list;
         field != nullptr && index < 10000; field = field->next, ++index) {
      uint32_t field_type = emit_type(field->type, depth + 1);
      const char *field_name = source_name(field->source_corresp);
      std::string field_record = std::to_string(id) + "\t" +
          std::to_string(index) + "\t" + std::to_string(field_type) +
          "\t" + (field_name == nullptr ? "" : field_name) + "\t" +
          std::to_string(field->offset) + "\t" +
          std::to_string(field->offset_bit_remainder) + "\t" +
          std::to_string(field->is_bit_field ? field->bit_size : 0);
      current_callback(current_user_data, 10, field_record.c_str(), 0);
    }
  }
  // Preserve the calling interface shape, including variadic functions.
  if (type->kind == tk_routine && type->variant.routine.extra_info != nullptr) {
    const auto *info = type->variant.routine.extra_info;
    std::string signature = std::to_string(id) + "\\t" +
        std::to_string(info->prototyped ? 1 : 0) + "\\t" +
        std::to_string(info->has_ellipsis ? 1 : 0);
    current_callback(current_user_data, 12, signature.c_str(), 0);
  }
  // Send additional parameter type edges in declaration order.
  if (type->kind == tk_routine && type->variant.routine.extra_info != nullptr) {
    unsigned position = 0;
    for (a_param_type_ptr p = type->variant.routine.extra_info->param_type_list;
         p != nullptr && position < 10000; p = p->next, ++position) {
      uint32_t parameter = emit_type(p->type, depth + 1);
      std::string edge = std::to_string(id) + "\t" +
          std::to_string(position) + "\t" + std::to_string(parameter);
      current_callback(current_user_data, 8, edge.c_str(), 0);
    }
  }
  std::string record = std::to_string(id) + "\t" +
      std::to_string(static_cast<unsigned>(type->kind)) + "\t" +
      std::to_string(relation) + "\t" + std::to_string(child);
  current_callback(current_user_data, 7, record.c_str(), 0);
  // Type qualifiers and reference distinctions are native EDG properties,
  // rather than inferred from the printed type name.
  if (type->kind == tk_pointer || type->kind == tk_typeref) {
    unsigned flags = 0;
    if (type->kind == tk_pointer) {
      if (type->variant.pointer.is_reference) flags |= 1;
      if (type->variant.pointer.is_rvalue_reference) flags |= 2;
    }
    std::string qualifiers = type->kind == tk_typeref ?
        std::to_string(static_cast<unsigned>(type->variant.typeref.qualifiers)) : "";
    std::string detail = std::to_string(id) + "\t" +
        std::to_string(flags) + "\t" + qualifiers;
    current_callback(current_user_data, 11, detail.c_str(), 0);
  }
  if (type->kind == tk_array &&
      !type->variant.array.is_variable_size_array &&
      !type->variant.array.is_template_dependent_size_array &&
      !type->incomplete) {
    std::string count = std::to_string(type->variant.array.variant.number_of_elements);
    std::string bound = std::to_string(id) + "\t" + count;
    current_callback(current_user_data, 9, bound.c_str(), 0);
  }
  return id;
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
  if (node->kind == enk_variable && node->variant.variable.ptr != nullptr) {
    referenced = declaration_name(node->variant.variable.ptr->source_corresp, false);
  } else if (node->kind == enk_routine && node->variant.routine.ptr != nullptr) {
    referenced = declaration_name(node->variant.routine.ptr->source_corresp, true);
  } else if (node->kind == enk_field && node->variant.field.ptr != nullptr) {
    const char *field = source_name(node->variant.field.ptr->source_corresp);
    if (field != nullptr) referenced = field;
  }
  // Report target type layout only when EDG has a complete nonzero-sized type.
  const bool layout_known = node->type != nullptr &&
                             !node->type->incomplete && node->type->size != 0;
  const std::string type_size = layout_known ?
      std::to_string(node->type->size) : "";
  const std::string type_align = layout_known ?
      std::to_string(node->type->alignment) : "";
  const std::string constant_kind = node->kind == enk_constant &&
      node->variant.constant.ptr != nullptr ?
      std::to_string(static_cast<unsigned>(node->variant.constant.ptr->kind)) : "";
  std::string integer_value;
  if (node->kind == enk_constant && node->variant.constant.ptr != nullptr &&
      node->variant.constant.ptr->kind == ck_integer) {
    a_boolean overflow = FALSE;
    a_host_large_integer value =
        value_of_integer_constant(node->variant.constant.ptr, &overflow);
    if (!overflow && sizeof(value) <= sizeof(int64_t)) {
      integer_value = std::to_string(static_cast<int64_t>(value));
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
  const std::string related_kind = related_type == nullptr ? "" :
      std::to_string(static_cast<unsigned>(related_type->kind));
  const uint32_t type_id = emit_type(node->type, 0);
  std::string record = function + "\t" + stmt_path + "\t" + path + "\t" +
                       parent + "\t" + std::to_string(op) + "\t" +
                       std::to_string(ty) + "\t" + referenced + "\t" + type_size + "\t" + type_align + "\t" + constant_kind + "\t" + integer_value + "\t" +
                       std::to_string(relationship) + "\t" + related_kind + "\t" + std::to_string(type_id);
  current_callback(current_user_data, 6, record.c_str(),
                   static_cast<uint32_t>(node->kind));
  if (node->kind == enk_operation) {
    unsigned index = 0;
    for (an_expr_node_ptr operand = node->variant.operation.operands;
         operand != nullptr && index < 10000;
         operand = operand->next, ++index) {
      emit_expr(operand, function, stmt_path,
                path + "." + std::to_string(index), path, depth + 1);
    }
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
    std::string record = function + "\t" + path + "\t" + parent;
    current_callback(current_user_data, 5, record.c_str(),
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

static void collect_scope(a_scope_ptr scope, const std::string &prefix,
                          unsigned depth)
{
  if (scope == nullptr || current_callback == nullptr || depth > 128) return;
  for (a_routine_ptr r = scope->routines; r != nullptr; r = r->next) {
    if (source_name(r->source_corresp) != nullptr) {
      std::string name = declaration_name(r->source_corresp, true);
      emit_type(r->type, 0);
      current_callback(current_user_data, 1, name.c_str(),
                       r->function_def_number != NULL_function_def_number);
      if (r->function_def_number != NULL_function_def_number) {
        a_scope_ptr body = scope_for_routine_or_null(r);
        if (body != nullptr) {
          emit_statements(body->assoc_block, name, "", "", 0);
        }
      }
    }
  }
  for (a_variable_ptr v = scope->variables; v != nullptr; v = v->next) {
    if (source_name(v->source_corresp) != nullptr) {
      std::string name = declaration_name(v->source_corresp, false);
      emit_type(v->type, 0);
      current_callback(current_user_data, 2, name.c_str(), 0);
    }
  }
  for (a_type_ptr t = scope->types; t != nullptr; t = t->next) {
    if (source_name(t->source_corresp) != nullptr) {
      std::string name = declaration_name(t->source_corresp, false);
      emit_type(t, 0);
      current_callback(current_user_data, 3, name.c_str(), 0);
    }
  }
  for (a_namespace_ptr ns = scope->namespaces; ns != nullptr; ns = ns->next) {
    if (ns->is_namespace_alias) continue;
    const char *name = ns->source_corresp.name;
    std::string qualified = prefix + (name != nullptr ? name : "<anonymous>");
    current_callback(current_user_data, 4, qualified.c_str(), 0);
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
  current_callback = callback;
  current_user_data = user_data;
  // EDG is built with -fno-exceptions; its callable path returns a status.
  // Do not introduce an exception handler compiled with incompatible flags.
  int status = edg::edg_main(argc, argv);
  current_callback = nullptr;
  current_user_data = nullptr;
  type_ids.clear();
  return status;
}
