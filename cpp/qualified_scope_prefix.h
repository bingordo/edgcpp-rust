// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
#pragma once
#include <string>
#include <unordered_set>
#include <vector>

namespace edg_bridge {
// Return false on a cyclic parent chain; never publish a partial prefix.
// Scope and name are borrowed only for the duration of the walk.
template<class Scope, class Parent, class Name>
bool qualified_scope_prefix(Scope scope, Parent parent, Name name,
                            std::string &result)
{
  result.clear();
  std::unordered_set<Scope> visited;
  std::vector<const char *> components;
  for (; scope != nullptr; scope = parent(scope)) {
    if (!visited.insert(scope).second) return false;
    if (const char *component = name(scope)) components.push_back(component);
  }
  for (auto it = components.rbegin(); it != components.rend(); ++it) {
    result += *it;
    result += "::";
  }
  return true;
}
}
