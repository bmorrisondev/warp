# Configurable Tab Groups — Tech Spec

## Problem

Tabs in the workspace are stored as a flat `Vec<TabData>` with no grouping concept. The vertical tabs panel renders each tab as its own "group," but there's no way to organize multiple tabs under a named group, rename groups, or drag tabs between groups.

## Current State

- `Workspace.tabs: Vec<TabData>` — flat list indexed by `usize`
- `active_tab_index: usize` — points into the flat list
- Every action, drag/drop, context menu, and persistence path uses flat `tab_index: usize`
- In the vertical tabs panel, each `TabData` is rendered by `render_tab_group()` which treats it as a standalone group
- `PaneGroup.custom_title` provides per-tab rename; group headers only appear when a custom title is set
- Tab drag uses `StartTabDrag` → `DragTab { tab_index }` → `DropTab` with `Draggable` elements
- Tab reorder: `MoveTabLeft(usize)` / `MoveTabRight(usize)` swap adjacent entries in the flat vec
- Persistence: workspace state save/restore serializes the flat tab list

## Design Approach

Add a grouping layer **on top of** the existing flat `tabs: Vec<TabData>` to minimize invasive refactoring. Each tab gets a `group_id`, and a separate `tab_groups` vec holds group metadata.

### Data Model

**New type:** `TabGroupId` — a `Copy + Eq + Hash` newtype around `usize` (auto-incrementing counter on `Workspace`).

**New struct `TabGroupInfo`:**

```rust
struct TabGroupInfo {
    id: TabGroupId,
    name: Option<String>,
    collapsed: bool,
}
```

**Workspace additions:**

- `tab_groups: Vec<TabGroupInfo>` — ordered list of groups; rendering order matches this vec
- `next_group_id: usize` — monotonically increasing counter for minting new `TabGroupId`s
- Helper: `fn tabs_in_group(&self, group_id: TabGroupId) -> Vec<(usize, &TabData)>` — returns `(flat_index, tab)` pairs for a group, preserving order within `self.tabs`

**TabData addition:**

- `group_id: TabGroupId` — which group this tab belongs to

**Invariants:**

- Every `TabData.group_id` must reference a valid entry in `tab_groups`
- Tabs within a group appear contiguously in the flat `tabs` vec (enforced by insert/move helpers)
- There is always at least one group; the default group is created on workspace init

### New Actions (`WorkspaceAction`)

- `CreateTabGroup { name: Option<String> }` — appends a new group; the next new tab goes here
- `RenameTabGroup { group_id: TabGroupId }` — enters rename mode for the group header
- `SetTabGroupName { group_id: TabGroupId, name: String }` — commits the rename
- `DeleteTabGroup { group_id: TabGroupId }` — removes the group; moves its tabs to the previous group (or next if first)
- `CollapseTabGroup { group_id: TabGroupId }` / `ExpandTabGroup { group_id: TabGroupId }`
- `MoveTabToGroup { tab_index: usize, target_group_id: TabGroupId, position: Option<usize> }` — moves a tab to a different group at an optional position within that group
- `DragTabBetweenGroups { tab_index: usize, target_group_id: TabGroupId, insert_position: usize }` — drag-and-drop variant
- `ReorderTabGroup { group_id: TabGroupId, direction: TabMovement }` — reorder groups themselves

### Vertical Tabs Rendering Changes

File: `app/src/workspace/view/vertical_tabs.rs`

**`render_vertical_tabs_groups()`** (currently iterates `visible_tabs` which maps 1:1 to tabs):

- Change to iterate `workspace.tab_groups` as the outer loop
- For each group, render a group header (always visible, not just when custom title is set)
- Below the header, render the tabs belonging to that group using existing `render_tab_group()` / `render_tab_group_internal()` (renamed to `render_tab_item()` for clarity)
- Collapsed groups show only the header

**Group header (extend existing `render_group_header()`):**

- Always rendered for each group (currently only shown when `has_custom_title`)
- Shows group name (or "Group N" placeholder) in 10pt sub-text
- Double-click triggers `RenameTabGroup`; uses the existing `tab_rename_editor` pattern with a `TextInput`
- Right-click opens a group context menu (rename, delete, collapse/expand)
- Collapse chevron icon on the left

**Drop targets:**

- Each group header is a `DropTarget` for cross-group tab moves
- Dropping a tab on a group header moves it to the end of that group
- Existing within-group drop targets (before/after tab) still work
- Visual indicator: highlight the target group header on hover during drag

### Horizontal Tab Bar Changes

File: `app/src/workspace/view.rs`

The horizontal tab bar currently renders tabs as a flat row. Add visual group separators:

- Render a thin vertical divider between groups
- Optionally show a small group name label above/beside the divider
- Drag a tab past a group divider to move it to the adjacent group

### Persistence

- Extend the workspace save/restore serialization to include `tab_groups` metadata and each tab's `group_id`
- Backward compat: if no group data is found on restore, create a single default group containing all tabs

### State Additions to `Workspace`

- `group_rename_editor: ViewHandle<EditorView>` — shared editor for group rename (mirrors `tab_rename_editor`)
- `renaming_group_id: Option<TabGroupId>` — which group is being renamed
- `tab_group_context_menu: ViewHandle<Menu<WorkspaceAction>>` — context menu for group headers
- `show_tab_group_context_menu: Option<(TabGroupId, Vector2F)>`

### Context Menu for Group Headers

Items:

- Rename Group
- Collapse / Expand Group
- Separator
- New Tab in Group
- Separator
- Move Group Up / Move Group Down
- Separator
- Delete Group (moves tabs to adjacent group)

### Key Interactions

**Creating a group:**

- Context menu on vertical tabs panel background → "New Group"
- Command palette → "Create Tab Group"
- New tabs are added to the group of the currently active tab

**Renaming a group:**

- Double-click group header → inline rename (same UX as tab rename)
- Context menu → "Rename Group"

**Dragging tabs between groups:**

- Drag a tab row; while dragging, group headers become highlighted drop targets
- Dropping on a group header appends the tab to that group
- Dropping between tabs in a different group inserts at that position and changes `group_id`
- The flat `tabs` vec is reordered so tabs within a group remain contiguous

**Collapsing a group:**

- Click the chevron on the group header
- Collapsed groups show only the header; their tabs are hidden but still exist
- The active tab's group auto-expands

## Files Changed (estimated)

- `app/src/workspace/view/vertical_tabs.rs` — rendering, drop targets, group headers
- `app/src/workspace/view.rs` — `Workspace` struct, state, horizontal tab bar separators
- `app/src/workspace/action.rs` — new action variants
- `app/src/tab.rs` — `TabData` gets `group_id` field
- `app/src/workspace/mod.rs` — binding registration for new actions
- `app/src/workspace/tab_settings.rs` — potential new settings (e.g., default group behavior)
- `app/src/pane_group/mod.rs` — minor: persistence helpers
- Persistence/serialization files — save/restore group metadata

## Risks

- **Contiguity invariant**: Keeping tabs within a group contiguous in the flat vec is critical for correct `active_tab_index` behavior. Every insert/move/close operation must maintain this.
- **Backward compat**: Existing workspaces with no group data must gracefully default to a single group.
- **Horizontal tab bar**: Group separators in the horizontal bar need careful layout math to avoid breaking the existing overflow/scroll behavior.
- **Cross-window drag**: The existing cross-window tab drag (`DragTabsToWindows` feature flag) needs to carry group context; the tab should land in a matching or default group in the target window.
