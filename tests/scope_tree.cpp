// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
#include "../cpp/scope_tree.h"
#include <cassert>
#include <cstdint>
#include <limits>
#include <vector>
#include <unordered_map>

struct Scope { Scope *child = nullptr; Scope *next = nullptr; };
struct Edge { Scope *node; std::uint32_t id; std::uint32_t parent; };

static bool walk(Scope *root, std::vector<Edge> &edges)
{
  edges.clear();
  return edg_bridge::walk_scope_tree(root,
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
  assert(walk(nullptr, edges) && edges.empty());
  Scope root, a, b, c, external;
  root.child = &a; root.next = &external;
  a.child = &c; a.next = &b;
  assert(walk(&root, edges) && edges.size() == 4);
  assert(edges[0].node == &root && edges[0].parent == none);
  assert(edges[1].node == &a && edges[1].parent == 0);
  assert(edges[2].node == &c && edges[2].parent == 1);
  assert(edges[3].node == &b && edges[3].parent == 0);
  for (std::size_t i = 0; i < edges.size(); ++i) assert(edges[i].id == i);
  b.child = &c; // shared child
  assert(!walk(&root, edges));
  b.child = nullptr; b.next = &a; // sibling cycle
  assert(!walk(&root, edges));
  b.next = nullptr; c.child = &root; // ancestor cycle
  assert(!walk(&root, edges));
  c.child = nullptr;
  assert(walk(&root, edges));
  std::vector<Scope> chain(10001);
  for (std::size_t i = 1; i < chain.size(); ++i) chain[i-1].child = &chain[i];
  assert(walk(&chain[0], edges) && edges.size() == chain.size());
  for (std::size_t i = 1; i < edges.size(); ++i) {
    assert(edges[i].node == &chain[i] && edges[i].id == i && edges[i].parent == i-1);
  }
  // Wide lists also exceed the former cap.
  for (auto &s : chain) { s.child = nullptr; s.next = nullptr; }
  root.child = &chain[0];
  for (std::size_t i = 1; i < chain.size(); ++i) chain[i-1].next = &chain[i];
  assert(walk(&root, edges) && edges.size() == chain.size()+1);
  for (std::size_t i = 1; i < edges.size(); ++i) assert(edges[i].parent == 0);
  unsigned visits = 0;
  assert(!edg_bridge::walk_scope_tree(&root,
      [](Scope *s) { return s->child; }, [](Scope *s) { return s->next; },
      [&](Scope *, std::uint32_t, std::uint32_t) { return ++visits < 2; }));
  assert(visits == 2);

  // Routine lifetime discovery can revisit a root, or attach a fallback
  // root to an already indexed parent without claiming a native child edge.
  std::unordered_map<const void *, std::uint32_t> ids;
  auto forest = [&](Scope *node, std::uint32_t parent) {
    return edg_bridge::walk_scope_forest_root(node, parent, ids,
        [](Scope *n) { return n->child; }, [](Scope *n) { return n->next; },
        [&](Scope *n, std::uint32_t id, std::uint32_t owner, bool child_edge) {
          assert(child_edge == (n == &a || n == &b));
          edges.push_back({n, id, owner});
          return true;
        });
  };
  root.child = &a; a.child = nullptr; a.next = &b; b.child = nullptr;
  b.next = nullptr; c.child = nullptr;
  edges.clear();
  assert(forest(&root, none) && ids.size() == 3);
  assert(forest(&a, none) && edges.size() == 3); // alias
  assert(forest(&c, 1) && ids.size() == 4);
  assert(edges.back().id == 3 && edges.back().parent == 1);
  assert(forest(&c, none) && edges.size() == 4); // alias
  external.child = &a; // cross-root duplicate child edge
  assert(!forest(&external, none) && ids.size() == 5);
  assert(edges.back().id == 4 && edges.back().parent == none);
}
