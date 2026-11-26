// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
#include "../cpp/qualified_scope_prefix.h"
#include <cassert>
#include <string>
#include <vector>

struct Scope {
  Scope *parent;
  const char *name;
};

static bool prefix(Scope *scope, std::string &result)
{
  return edg_bridge::qualified_scope_prefix(
      scope, [](Scope *s) { return s->parent; },
      [](Scope *s) { return s->name; }, result);
}

int main()
{
  std::string result = "stale";
  assert(prefix(nullptr, result) && result.empty());
  Scope root{nullptr, "outer"}, middle{&root, nullptr}, leaf{&middle, "inner"};
  assert(prefix(&leaf, result) && result == "outer::inner::");
  std::vector<Scope> deep(4096);
  std::string expected;
  for (std::size_t i = 0; i < deep.size(); ++i) {
    deep[i] = {i ? &deep[i - 1] : nullptr, "n"};
    expected += "n::";
  }
  assert(prefix(&deep.back(), result) && result == expected);
  root.parent = &root;
  assert(!prefix(&root, result) && result.empty());
  assert(!prefix(&leaf, result) && result.empty());
  root.parent = &leaf;
  assert(!prefix(&leaf, result) && result.empty());
  root.parent = nullptr;
  assert(prefix(&leaf, result) && result == "outer::inner::");
}
