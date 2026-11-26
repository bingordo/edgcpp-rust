// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
#pragma once
#include <cstddef>
#include <limits>
#include <string>
#include <vector>

namespace edg_bridge {
// Compact stable indices for ordinal occurrence paths. Pending work needs
// only an index; the materialized wire spelling is temporary.
class ordinal_path_arena {
 public:
  using id = std::size_t;
  static constexpr id none = std::numeric_limits<id>::max();

  id child(id parent_id, std::size_t ordinal) {
    if (entries_.size() == none ||
        (parent_id != none && parent_id >= entries_.size())) return none;
    entries_.push_back({parent_id, ordinal});
    return entries_.size() - 1;
  }
  id parent(id item) const { return entries_[item].parent; }
  std::string text(id item) const {
    std::vector<std::size_t> ordinals;
    while (item != none) {
      ordinals.push_back(entries_[item].ordinal);
      item = entries_[item].parent;
    }
    std::string result;
    for (auto it = ordinals.rbegin(); it != ordinals.rend(); ++it) {
      if (!result.empty()) result += ".";
      result += std::to_string(*it);
    }
    return result;
  }

 private:
  struct entry { id parent; std::size_t ordinal; };
  std::vector<entry> entries_;
};
}
