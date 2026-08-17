#ifndef TREE_SITTER_TREE_CURSOR_H_
#define TREE_SITTER_TREE_CURSOR_H_

#include "./subtree.h"

typedef struct {
  const Subtree *subtree;
  Length position;
  uint32_t child_index;
  uint32_t structural_child_index;
  uint32_t descendant_index;
  // Memoized answer to "which of this subtree's children are visible / named",
  // filled on demand by `ts_tree_cursor_current_status` and described there.
  // Every construction site uses a designated initializer, so these are
  // zero-filled: `false` means "not computed yet", which is why the flag is
  // spelled that way round — a construction site that forgets them can only
  // cost a recomputation, never hand back a stale answer.
  bool sibling_summary_valid;
  // One past the index of the last child that contributes a visible node, and
  // one past the index of the last that contributes a visible *named* one.
  // Zero means there is no such child.
  uint32_t visible_children_end;
  uint32_t named_children_end;
} TreeCursorEntry;

typedef struct {
  const TSTree *tree;
  Array(TreeCursorEntry) stack;
  TSSymbol root_alias_symbol;
} TreeCursor;

typedef enum {
  TreeCursorStepNone,
  TreeCursorStepHidden,
  TreeCursorStepVisible,
} TreeCursorStep;

void ts_tree_cursor_init(TreeCursor *self, TSNode node);
void ts_tree_cursor_current_status(
  const TSTreeCursor *_self,
  TSFieldId *field_id,
  bool *has_later_siblings,
  bool *has_later_named_siblings,
  bool *can_have_later_siblings_with_this_field,
  TSSymbol *supertypes,
  unsigned *supertype_count
);

TreeCursorStep ts_tree_cursor_goto_first_child_internal(TSTreeCursor *_self);
TreeCursorStep ts_tree_cursor_goto_next_sibling_internal(TSTreeCursor *_self);

static inline Subtree ts_tree_cursor_current_subtree(const TSTreeCursor *_self) {
  const TreeCursor *self = (const TreeCursor *)_self;
  TreeCursorEntry *last_entry = array_back(&self->stack);
  return *last_entry->subtree;
}

TSNode ts_tree_cursor_parent_node(const TSTreeCursor *_self);

#endif  // TREE_SITTER_TREE_CURSOR_H_
