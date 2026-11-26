// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
#include "../cpp/scope_tree.h"
#include <cassert>
#include <cstdint>
#include <limits>
#include <vector>

struct Scope { Scope *child = nullptr; Scope *next = nullptr; };
struct Edge { Scope *node; std::uint32_t id; std::uint32_t parent; };

static bool walk(Scope *root, std::size_t limit, std::vector<Edge> &edges)
{
  edges.clear();
  return edg_bridge::walk_scope_tree(root, limit,
      [](Scope *s) { return s->child; }, [](Scope *s) { return s->next; },
      [&](Scope *s, std::uint32_t id, std::uint32_t parent) {
        edges.push_back({s, id, parent});
        return true;
      });
}

int main()
{
  const auto none = std::numeric_limits<std::uint32_t>::max();
  std::vector<Edge> edges;
  assert(walk(nullptr, 0, edges) && edges.empty());
  Scope root, a, b, c, external;
  root.child = &a; root.next = &external;
  a.child = &c; a.next = &b;
  assert(walk(&root, 4, edges) && edges.size() == 4);
  assert(edges[0].node == &root && edges[0].parent == none);
  assert(edges[1].node == &a && edges[1].parent == 0);
  assert(edges[2].node == &c && edges[2].parent == 1);
  assert(edges[3].node == &b && edges[3].parent == 0);
  for (std::size_t i = 0; i < edges.size(); ++i) assert(edges[i].id == i);
  assert(!walk(&root, 3, edges) && edges.size() == 3);
  assert(!walk(&root, 0, edges) && edges.empty());
  b.child = &c; // shared child
  assert(!walk(&root, 10, edges));
  b.child = nullptr; b.next = &a; // sibling cycle
  assert(!walk(&root, 10, edges));
  b.next = nullptr; c.child = &root; // ancestor cycle
  assert(!walk(&root, 10, edges));
  c.child = nullptr;
  assert(walk(&root, 4, edges));
  std::vector<Scope> chain(10001);
  for (std::size_t i = 1; i < chain.size(); ++i) chain[i-1].child = &chain[i];
  assert(!walk(&chain[0], 10000, edges) && edges.size() == 10000);
  chain[9999].child = nullptr;
  assert(walk(&chain[0], 10000, edges) && edges.size() == 10000);
  for (std::size_t i = 1; i < edges.size(); ++i) {
    assert(edges[i].node == &chain[i] && edges[i].id == i && edges[i].parent == i-1);
  }
  // A wide sibling chain uses the same global node budget.
  for (auto &s : chain) { s.child = nullptr; s.next = nullptr; }
  root.child = &chain[0];
  for (std::size_t i = 1; i < chain.size(); ++i) chain[i-1].next = &chain[i];
  assert(!walk(&root, 10000, edges) && edges.size() == 10000);
  chain[9998].next = nullptr;
  assert(walk(&root, 10000, edges) && edges.size() == 10000);
  for (std::size_t i = 1; i < edges.size(); ++i) assert(edges[i].parent == 0);
  unsigned visits = 0;
  assert(!edg_bridge::walk_scope_tree(&root, 10000,
      [](Scope *s) { return s->child; }, [](Scope *s) { return s->next; },
      [&](Scope *, std::uint32_t, std::uint32_t) { return ++visits < 2; }));
  assert(visits == 2);
}
