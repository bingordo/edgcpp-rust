// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
#pragma once
#include <cstddef>
#include <string>
#include <unordered_set>
#include <vector>

namespace edg_bridge {
// Occurrence paths, not node identity, identify expression operands. A node
// shared by disjoint paths is legal here; only an active ancestor is a cycle.
// Keep a single mutable path plus a dynamic stack, not one full path per frame.
template<class Node, class Children, class Next, class Visit>
bool walk_expression_tree(Node root, const std::string &root_path,
                          const std::string &root_parent,
                          Children children, Next next, Visit visit)
{
  if (root == nullptr) return true;
  struct Frame {
    Node node;
    Node operand;
    bool linked;
    std::size_t index;
    std::size_t path_size;
  };
  std::string path = root_path;
  std::vector<Frame> stack;
  std::unordered_set<Node> active;
  auto enter = [&](Node node, const std::string &owner_path) {
    if (!active.insert(node).second) return false;
    if (!visit(node, path, owner_path)) return false;
    const auto operands = children(node);
    // Detect sibling cycles independently of ancestor cycles; do not
    // reject long valid lists or count shared occurrences as cycles.
    if (operands.linked_list) {
      Node slow = operands.first, fast = operands.first;
      while (fast != nullptr && next(fast) != nullptr) {
        slow = next(slow);
        fast = next(next(fast));
        if (slow == fast) return false;
      }
    }
    stack.push_back({node, operands.first, operands.linked_list, 0, path.size()});
    return true;
  };
  if (!enter(root, root_parent)) return false;
  while (!stack.empty()) {
    auto &frame = stack.back();
    if (frame.operand == nullptr) {
      active.erase(frame.node);
      stack.pop_back();
      if (!stack.empty()) path.resize(stack.back().path_size);
      continue;
    }
    Node operand = frame.operand;
    frame.operand = frame.linked ? next(operand) : nullptr;
    const std::string owner_path = path;
    path += ".";
    path += std::to_string(frame.index++);
    if (!enter(operand, owner_path)) return false;
  }
  return true;
}
}
