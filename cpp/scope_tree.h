// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
#pragma once
#include <cstddef>
#include <cstdint>
#include <limits>
#include <unordered_set>
#include <vector>

namespace edg_bridge {
// Check native next-linked lists in linear time and constant extra space,
// independently of the caller's semantic predicates. A repeated identity
// is malformed; there is no artificial declaration-count ceiling.
template<class Ptr>
bool acyclic_next_list(Ptr first)
{
  auto slow=first, fast=first;
  while (fast!=nullptr && fast->next!=nullptr) {
    slow=slow->next;
    fast=fast->next->next;
    if (slow==fast) return false;
  }
  return true;
}

// Preorder, source sibling order, dense IDs, and representable wire IDs.
// The stack holds only the next sibling of each active parent. A repeated
// identity is malformed (cycle or shared child), not an omitted subtree.
template<class Scope, class Children, class Next, class Visit>
bool walk_scope_tree(Scope root, Children children,
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
    if (seen.size() >= no_parent ||
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

// Admit another root into a previously indexed forest. Repeated roots are
// valid discovery aliases, but repeated nodes reached by child edges are not.
// Local preorder IDs are translated into the forest's global ID space.
template<class Scope, class IdMap, class Children, class Next, class Visit>
bool walk_scope_forest_root(Scope root,
                            std::uint32_t parent, IdMap &ids,
                            Children children, Next next, Visit visit)
{
  if (root == nullptr || ids.find(root) != ids.end()) return true;
  const auto none = std::numeric_limits<std::uint32_t>::max();
  if (ids.size() >= none) return false;
  const auto base = static_cast<std::uint32_t>(ids.size());
  return walk_scope_tree(root, children, next,
      [&](Scope node, std::uint32_t local_id, std::uint32_t local_parent) {
        const bool child_edge = local_parent != none;
        if (local_id >= none - base) return false;
        const auto id = base + local_id;
        if (!ids.emplace(node, id).second) return false;
        return visit(node, id, child_edge ? base + local_parent : parent,
                     child_edge);
      });
}
}
