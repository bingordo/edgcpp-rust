// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
#include "../cpp/expression_tree.h"
#include <cassert>
#include <string>
#include <vector>

struct Node { Node *child = nullptr; Node *next = nullptr; bool linked = true; };
struct Children { Node *first; bool linked_list; };
struct Entry { Node *node; std::string path; std::string parent; };
static bool walk(Node *root, std::size_t limit, std::vector<Entry> &entries)
{
  entries.clear();
  return edg_bridge::walk_expression_tree(root, "0", "", limit,
      [](Node *n) -> Children { return {n->child, n->linked}; },
      [](Node *n) { return n->next; },
      [&](Node *n, const std::string &p, const std::string &parent) {
        entries.push_back({n,p,parent});
        return true;
      });
}
int main()
{
  std::vector<Entry> entries;
  Node root, a, b, leaf, external;
  root.child = &a; root.next = &external;
  a.next = &b; a.child = &leaf; b.child = &leaf;
  assert(walk(&root, 4, entries) && entries.size() == 5);
  assert(entries[0].path == "0" && entries[0].parent.empty());
  assert(entries[1].path == "0.0" && entries[1].parent == "0");
  assert(entries[2].node == &leaf && entries[2].path == "0.0.0");
  assert(entries[3].node == &b && entries[3].path == "0.1");
  assert(entries[4].node == &leaf && entries[4].path == "0.1.0");
  root.linked = false;
  assert(walk(&root, 4, entries) && entries.size() == 3);
  root.linked = true;
  leaf.child = &root;
  assert(!walk(&root, 4, entries));
  leaf.child = nullptr; b.next = &a;
  assert(!walk(&root, 4, entries));
  b.next = nullptr;
  assert(!walk(&root, 2, entries));
  assert(walk(nullptr, 0, entries) && entries.empty());
  assert(!walk(&root, 0, entries));
  std::vector<Node> chain(10001);
  for (std::size_t i=1;i<chain.size();++i) chain[i-1].child = &chain[i];
  unsigned count=0;
  auto deep = [&](std::size_t limit) {
    count=0;
    return edg_bridge::walk_expression_tree(&chain[0], "r", "p", limit,
        [](Node *n) -> Children { return {n->child,n->linked}; },
        [](Node *n) { return n->next; },
        [&](Node *n, const std::string &path, const std::string &parent) {
          assert(n == &chain[count]);
          assert(path.size() == 1 + 2*count);
          assert(parent.size() == (count ? path.size()-2 : 1));
          ++count;
          return true;
        });
  };
  assert(!deep(10000) && count==10000);
  chain[9999].child=nullptr;
  assert(deep(10000) && count==10000);
}
