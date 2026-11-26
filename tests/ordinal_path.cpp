// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
#include "../cpp/ordinal_path.h"
#include <cassert>
#include <string>

int main() {
  using Arena = edg_bridge::ordinal_path_arena;
  Arena paths(10003);
  const auto root = paths.child(Arena::none,0);
  const auto first = paths.child(root,17);
  const auto second = paths.child(root,4);
  assert(paths.text(Arena::none).empty());
  assert(paths.text(root)=="0");
  assert(paths.text(first)=="0.17" && paths.parent(first)==root);
  assert(paths.text(second)=="0.4");
  assert(paths.child(100000,1)==Arena::none);
  auto deep=first;
  for (unsigned i=0;i<10000;++i) deep=paths.child(deep,0);
  assert(paths.text(deep).size()==4+20000);
  assert(paths.text(second)=="0.4");
  assert(paths.child(deep,0)==Arena::none);
  Arena empty(0);
  assert(empty.child(Arena::none,0)==Arena::none);
}
