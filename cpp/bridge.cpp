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

extern "C" {
typedef void (*edgcpp_il_callback)(void *user_data, uint32_t kind,
                                    const char *name, uint32_t defined);
}

namespace {
thread_local edgcpp_il_callback current_callback = nullptr;
thread_local void *current_user_data = nullptr;
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
  std::string record = function + "\t" + stmt_path + "\t" + path + "\t" +
                       parent + "\t" + std::to_string(op) + "\t" +
                       std::to_string(ty) + "\t" + referenced + "\t" + type_size + "\t" + type_align + "\t" + constant_kind + "\t" + integer_value + "\t" +
                       std::to_string(relationship) + "\t" + related_kind;
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
      current_callback(current_user_data, 2, name.c_str(), 0);
    }
  }
  for (a_type_ptr t = scope->types; t != nullptr; t = t->next) {
    if (source_name(t->source_corresp) != nullptr) {
      std::string name = declaration_name(t->source_corresp, false);
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
  current_callback = callback;
  current_user_data = user_data;
  // EDG is built with -fno-exceptions; its callable path returns a status.
  // Do not introduce an exception handler compiled with incompatible flags.
  int status = edg::edg_main(argc, argv);
  current_callback = nullptr;
  current_user_data = nullptr;
  return status;
}
