// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
#include "../cpp/expression_tree.h"
#include <cassert>
#include <string>
#include <vector>

struct Node { Node *child = nullptr; Node *next = nullptr; bool linked = true; };
struct Children { Node *first; bool linked_list; };
struct Entry { Node *node; std::string path; std::string parent; };
static bool walk(Node *root, std::vector<Entry> &entries)
{
  entries.clear();
  return edg_bridge::walk_expression_tree(root, "0", "",
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
  assert(walk(&root, entries) && entries.size() == 5);
  assert(entries[0].path == "0" && entries[0].parent.empty());
  assert(entries[1].path == "0.0" && entries[1].parent == "0");
  assert(entries[2].node == &leaf && entries[2].path == "0.0.0");
  assert(entries[3].node == &b && entries[3].path == "0.1");
  assert(entries[4].node == &leaf && entries[4].path == "0.1.0");
  root.linked = false;
  assert(walk(&root, entries) && entries.size() == 3);
  root.linked = true;
  leaf.child = &root;
  assert(!walk(&root, entries));
  leaf.child = nullptr; b.next = &a;
  assert(!walk(&root, entries));
  b.next = nullptr;
  assert(walk(nullptr, entries) && entries.empty());
  std::vector<Node> chain(10001);
  for (std::size_t i=1;i<chain.size();++i) chain[i-1].child = &chain[i];
  unsigned count=0;
  auto deep = [&]() {
    count=0;
    return edg_bridge::walk_expression_tree(&chain[0], "r", "p",
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
  assert(deep() && count==chain.size());
  // Wide valid lists also exceed the former artificial limit.
  for (auto &n:chain) {n.child=nullptr;n.next=nullptr;}
  root.child=&chain[0];
  for (std::size_t i=1;i<chain.size();++i) chain[i-1].next=&chain[i];
  assert(walk(&root, entries) && entries.size()==chain.size()+1);
  chain.back().next=&chain[0];
  assert(!walk(&root, entries));
}
