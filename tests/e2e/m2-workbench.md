# M2 Workbench: end-to-end scenario

Written by the Architect before M2 work starts. The harness (role 10) runs `lathe` with the
test-driver hook from M1 (synthetic input, window resize, frame-timing probe) plus three M2
probes:

- the **layout probe** returns the pixel rectangle of every visible part, group, tab, sash,
  tree row, quick open row and status bar item;
- the **widget state probe** returns, per tab: label, preview flag, dirty flag, active flag;
  per quick open row: label, description, keybinding label, and the highlight ranges (UTF-16
  offsets) of label and description; per status bar item: id, text, alignment;
- the **accessibility tree** (accesskit) names each of them, with roles and states.

Steps check results through those probes and on disk. A timing window ends at the first
presented frame whose layout and widget state probes show the expected result. The milestone is complete only when every step passes on `rust-port`
in CI, with the M1 scenario still green.

Defaults are VS Code's own defaults unless a step says otherwise. The window is 1280×800 logical
pixels at scale factor 1.

## Fixture

The harness creates `<tmp>/proj`:

```
proj/
  .hidden/keep.txt
  B.txt                 "b\n"
  a.txt                 "alpha\nbeta\ngamma\n"
  docs/readme.md        "# readme\n"
  src/file10.rs         "fn ten() {}\n"
  src/file2.rs          "fn two() {}\n"
  src/main.rs           "fn main() {\n    let x = 1;\n}\n"
  src/nested/deep.rs    "\n"
```

and, for step 11, `<tmp>/big` with 10,000 files `d<0..99>/f<0..99>.txt`.

## Steps

1. **Open folder.** `lathe <tmp>/proj`. Within 1 s of launch the window shows the workbench:
   activity bar, side bar with the Explorer view, an empty editor area (one group, no tabs),
   and the status bar. The window title (the accessibility tree's window node name) is `proj - Lathe` on Windows and Linux and `proj` on
   macOS (`window.title` default, `windowTitle.ts`). The layout probe reports the side bar
   300 px wide, the status bar 22 px tall along the bottom edge, and the editor area filling
   the rest.
2. **File tree order.** A single-folder workspace shows the folder name `proj` as the Explorer
   pane title, not as a row. The rows, top to bottom, are `.hidden`, `docs`, `src`, `a.txt`,
   `B.txt`, all at level 1: folders first, then files, names compared case-insensitively
   (`explorer.sortOrder: default`). Each row is exposed in the accessibility tree with role
   tree item, its name, its level, and expanded/collapsed state.
3. **Expand and navigate.** Focus the Explorer, select `src`, press `Right`. `src` expands
   and its children, at level 2, are `nested`, `file2.rs`, `file10.rs`, `main.rs`
   (numeric-aware order: `file2` before `file10`). `Down` moves the selection one row; `Left` on a child moves to
   its parent; `Left` on `src` collapses it. Each key is reflected within one frame.
4. **Preview tab.** Single-click `a.txt` (or select it and press `Space`, which opens it and
   keeps focus in the Explorer). One tab `a.txt` opens in preview mode (the widget state probe
   reports the preview flag, which also selects the italic label style). Opening `B.txt` the same way replaces the
   preview tab, so there is still one tab. Double-clicking `B.txt` in the Explorer, or typing
   in it, pins the tab.
5. **Tabs.** Open `a.txt`, `B.txt` and `docs/readme.md` pinned, in that order. The group has
   three tabs in that order, `readme.md` active.
   - `Ctrl+Tab`, then release `Ctrl` (on every platform): the editor picker opens on key-down
     and switches on release, to the most recently used other editor, `B.txt`.
   - `Ctrl+PageDown` (macOS `Cmd+Alt+Right`) from `B.txt` activates `readme.md`; again wraps
     to `a.txt`. `Ctrl+PageUp` (macOS `Cmd+Alt+Left`) from `a.txt` wraps to `readme.md`.
     The most recently used order is now `readme.md`, `a.txt`, `B.txt`.
   - `Ctrl+PageUp` once more activates `B.txt`. `Ctrl+W` (macOS `Cmd+W`) closes it and
     activates `readme.md`, the most recently used remaining editor
     (`workbench.editor.focusRecentEditorAfterClose`), leaving tabs `a.txt`, `readme.md`.
   - Typing in `a.txt` sets its dirty flag; `Ctrl+W` on it shows the save prompt from M1;
     choose "Cancel" and the tab stays. `Ctrl+Z` (macOS `Cmd+Z`) undoes the typing, which
     returns `a.txt` to its saved version and clears the dirty flag.
6. **Split groups.** With `a.txt` active, `Ctrl+\` (macOS `Cmd+\`) splits the editor to the
   right: two groups side by side, each 50% of the editor area width (±1 px), both showing
   `a.txt`, focus in the new group. Typing in one group is visible in the other within one
   frame (same text model); `Ctrl+Z` then undoes it in both, and `a.txt` is not dirty.
   `Ctrl+1` / `Ctrl+2` focus group 1 / 2.
7. **Sash.** Drag the sash between the groups 200 px to the left. The layout probe shows the
   groups' widths changed by 200 px and summing to the editor area width; no group goes below
   the minimum editor width of 220 px (`DEFAULT_EDITOR_MIN_DIMENSIONS`). Dragging further left
   stops at 220 px. Double-clicking the sash restores equal widths. Resizing the window to
   1600×800 keeps the groups' width ratio within ±1 px per group.
8. **Close a group.** Close every editor in group 2. The group is removed and group 1 takes the
   full width (`workbench.editor.closeEmptyGroups`).
9. **Quick open.** `Ctrl+P` (macOS `Cmd+P`) opens quick open with recently opened editors
   first. Type `mainrs`. The first result is `main.rs` with description `src`, and the matched
   characters are highlighted (exposed as highlight ranges through the driver). `Enter` opens
   it in the active group as a pinned tab. Reopening quick open and pressing `Escape` closes it
   without opening anything. Typing `nested/deep` lists `deep.rs` with description
   `src/nested` (`src
ested` on Windows) first (path matching).
10. **Command palette.** Close all editors (`Ctrl+K Ctrl+W`, macOS `Cmd+K Cmd+W`); no editor is dirty, so no prompt appears. `Ctrl+Shift+P` (macOS `Cmd+Shift+P`),
    equivalently `F1`, opens quick open with the `>` prefix. Type `transform to uppercase`: no
    row is listed, because editor actions are listed only for the active text editor and
    only when their precondition (`editorReadonly` false) holds (`commandsQuickAccess.ts`).
    Open `a.txt` and repeat with `split editor`: the list contains `View: Split Editor` with
    keybinding label `Ctrl+\` (macOS `⌘\`). `Enter` runs it: two groups, each showing
    `a.txt`, focus in group 2. Reopening the palette lists `View: Split Editor` first, under
    "recently used".
11. **Large folder.** `lathe <tmp>/big`. The Explorer's first painted frame with rows appears
    within 1 s. Expanding a folder with 100 children paints within 100 ms. Quick open over the
    10,000 files shows the results for a three-character query within 200 ms of the last key.
12. **Status bar.** Open `src/main.rs` and place the cursor at line 2, column 5. Among the
    right-aligned status bar items, these appear from left to right in this relative order:
    `Ln 2, Col 5`, `Spaces: 4`, `UTF-8`, `LF`, and the language name (`Plain Text` until M3
    brings language detection). Moving the cursor updates the `Ln, Col` item within one frame.
    Selecting from 1:1 to 3:1 shows `Ln 3, Col 1 (27 selected)`. Clicking `LF`
    opens a quick pick with `LF` and `CRLF`; choosing `CRLF` and saving writes CRLF line endings
    to disk.
13. **Accessibility.** Every step's focus target is the accessibility tree's focused node:
    tree items (step 3), tabs with role tab and selected state (step 5), the quick open input
    and its active list item (steps 9 and 10), and status bar items with their labels (step 12).
14. **Exit.** Closing the window with no dirty editors exits with code 0.

## Out of scope for M2

Restoring the open editors and layout after a restart (needs the storage service, M5),
drag-and-drop of tabs and files, file operations in the Explorer (new, rename, delete), search
across files (M4), themes other than the default dark theme (M5), and syntax highlighting (M3).
Keybindings stay hard-wired until M5.
