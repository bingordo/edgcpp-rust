/* SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
 * A narrow, synchronous C ABI over EDG's native C++ IL.
 *
 * This file must be compiled with the exact configuration of libedgcpfe.a.
 * Its back_end() replaces the C-generating back end when the archive is linked.
 */
#include "fe_common.h"
#include <cstdint>
#include <exception>

extern "C" {
typedef void (*edgcpp_il_callback)(void *user_data, uint32_t kind,
                                    const char *name, uint32_t defined);
}

namespace {
thread_local edgcpp_il_callback current_callback = nullptr;
thread_local void *current_user_data = nullptr;
}

namespace edg {
void back_end(void)
{
  if (current_callback == nullptr || il_header.primary_scope == nullptr) {
    return;
  }
  // Snapshot only scalar data and names while EDG owns the live IL.
  // No EDG pointers may escape this callback.
  for (a_routine_ptr routine = il_header.primary_scope->routines;
       routine != nullptr; routine = routine->next) {
    const char *name = routine->source_corresp.name;
    if (name != nullptr) {
      current_callback(current_user_data, 1, name,
                       routine->function_def_number != NULL_function_def_number);
    }
  }
  for (a_variable_ptr var = il_header.primary_scope->variables;
       var != nullptr; var = var->next) {
    const char *name = var->source_corresp.name;
    if (name != nullptr) {
      current_callback(current_user_data, 2, name, 0);
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
  current_callback = callback;
  current_user_data = user_data;
  int status = -2;
  try {
    status = edg::edg_main(argc, argv);
  } catch (...) {
    status = -2; // Never allow a C++ exception to cross the C ABI.
  }
  current_callback = nullptr;
  current_user_data = nullptr;
  return status;
}
