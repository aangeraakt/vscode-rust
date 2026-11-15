# M2 Workbench: end-to-end scenario

Written by the Architect before M2 work starts. The harness (role 10) runs `lathe` with the
test-driver hook from M1 (synthetic input, frame-timing probe) plus two M2 probes: a **layout
probe** that returns the pixel rectangle of every visible part, group, tab, sash and tree row,
and the **accessibility tree** (accesskit) that names each of them. Steps check results through
those probes and on disk. The milestone is complete only when every step passes on `rust-port`
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
   and the status bar. The window title is `proj`. The layout probe reports the side bar at
   its default width (VS Code default) and the status bar at 22 px tall along the bottom edge.
2. **File tree order.** The Explorer's root `proj` is expanded. Its rows, top to bottom, are
   `.hidden`, `docs`, `src`, `a.txt`, `B.txt`: folders first, then files, names compared
   case-insensitively (`explorer.sortOrder: default`). Each row is exposed in the
   accessibility tree with role tree item, its name, its level, and expanded/collapsed state.
3. **Expand and navigate.** Focus the Explorer, select `src`, press `Right`. `src` expands
   and its children are `nested`, `file2.rs`, `file10.rs`, `main.rs` (numeric-aware order:
   `file2` before `file10`). `Down` moves the selection one row; `Left` on a child moves to
   its parent; `Left` on `src` collapses it. Each key is reflected within one frame.
4. **Preview tab.** Single-click `a.txt` (or select it and press `Space`, which opens it and
   keeps focus in the Explorer). One tab `a.txt` opens in preview mode (the accessibility tree
   marks it as preview, and the tab label is italic). Opening `B.txt` the same way replaces the
   preview tab, so there is still one tab. Double-clicking `B.txt` in the Explorer, or typing
   in it, pins the tab.
5. **Tabs.** Open `a.txt`, `B.txt` and `docs/readme.md` pinned. The group has three tabs in
   that order, `readme.md` active. `Ctrl+Tab` (macOS `Ctrl+Tab`) switches to the most recently
   used other editor, `B.txt`. `Ctrl+PageDown` / `Ctrl+PageUp` move to the next / previous tab
   by position, wrapping around. `Ctrl+W` (macOS `Cmd+W`) on `B.txt` closes it and activates
   the most recently used remaining editor (`workbench.editor.focusRecentEditorAfterClose`).
   A modified editor shows the dirty indicator on its tab; closing it shows the save prompt
   from M1.
6. **Split groups.** With `a.txt` active, `Ctrl+\` (macOS `Cmd+\`) splits the editor to the
   right: two groups side by side, each 50% of the editor area width (±1 px), both showing
   `a.txt`, focus in the new group. Typing in one group is visible in the other within one
   frame (same text model). `Ctrl+1` / `Ctrl+2` focus group 1 / 2.
7. **Sash.** Drag the sash between the groups 200 px to the left. The layout probe shows the
   groups' widths changed by 200 px and summing to the editor area width; no group goes below
   the minimum editor width (VS Code's `DEFAULT_EDITOR_MIN_DIMENSIONS`). Double-clicking the
   sash restores equal widths. Resizing the window keeps the proportions.
8. **Close a group.** Close every editor in group 2. The group is removed and group 1 takes the
   full width (`workbench.editor.closeEmptyGroups`).
9. **Quick open.** `Ctrl+P` (macOS `Cmd+P`) opens quick open with recently opened editors
   first. Type `mainrs`. The first result is `main.rs` with description `src`, and the matched
   characters are highlighted (exposed as highlight ranges through the driver). `Enter` opens
   it in the active group. `Escape` closes quick open without opening anything. Typing
   `nested/deep` matches `src/nested/deep.rs` (path matching).
10. **Command palette.** `Ctrl+Shift+P` (macOS `Cmd+Shift+P`), equivalently `F1`, opens quick
    open with the `>` prefix. Type `split editor`. The list contains `View: Split Editor` with
    its keybinding `Ctrl+\` shown on the right. `Enter` runs it, with the same result as step 6.
    Commands whose `when` clause is false (for example editor-only commands with no editor open)
    are not listed. The most recently run command is listed first the next time.
11. **Large folder.** `lathe <tmp>/big`. The Explorer's first painted frame with rows appears
    within 1 s. Expanding a folder with 100 children paints within 100 ms. Quick open over the
    10,000 files shows the results for a three-character query within 200 ms of the last key.
12. **Status bar.** Open `src/main.rs` and place the cursor at line 2, column 5. The status
    bar shows, right-aligned in this order: `Ln 2, Col 5`, `Spaces: 4`, `UTF-8`, `LF`, and the
    language name (`Plain Text` until M3 brings language detection). Moving the cursor updates
    the `Ln, Col` item within one frame; selecting two lines shows `(N selected)`. Clicking `LF`
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
