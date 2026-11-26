// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
#pragma once
#include <cstddef>
#include <cstdint>
#include <limits>
#include <unordered_set>
#include <vector>

namespace edg_bridge {
// Preorder, source sibling order, dense IDs, and a global admission budget.
// The stack holds only the next sibling of each active parent. A repeated
// identity is malformed (cycle or shared child), not an omitted subtree.
template<class Scope, class Children, class Next, class Visit>
bool walk_scope_tree(Scope root, std::size_t limit, Children children,
                     Next next, Visit visit)
{
  if (root == nullptr) return true;
  const auto no_parent = std::numeric_limits<std::uint32_t>::max();
  struct Frame { Scope node; std::uint32_t parent; };
  std::vector<Frame> stack;
  std::unordered_set<Scope> seen;
  Scope node = root;
  std::uint32_t parent = no_parent;
  while (node != nullptr) {
    if (seen.size() >= limit || seen.size() >= no_parent ||
        !seen.insert(node).second) return false;
    const auto id = static_cast<std::uint32_t>(seen.size() - 1);
    if (!visit(node, id, parent)) return false;
    // The root's next link belongs to an external list, not this tree.
    Scope sibling = parent == no_parent ? nullptr : next(node);
    Scope child = children(node);
    if (child != nullptr) {
      if (sibling != nullptr) stack.push_back({sibling, parent});
      node = child;
      parent = id;
    } else if (sibling != nullptr) {
      node = sibling;
    } else if (!stack.empty()) {
      node = stack.back().node;
      parent = stack.back().parent;
      stack.pop_back();
    } else {
      node = nullptr;
    }
  }
  return true;
}
}
