# M1 Editor: end-to-end scenario

Written by the Architect before M1 work starts. The harness (role 10) runs `lathe` with a
test-driver hook (role 02 exposes synthetic input and a frame-timing probe) and checks the
results on disk and through the driver. The milestone is complete only when every step passes on
`rust-port` in CI.

Fixtures are created by the harness in a temp directory.

## Steps

1. **Open from the command line.** `lathe <tmp>/hello.txt` where the file contains
   `hello\nworld\n` (LF). The window opens with the cursor at 1:1 and the file content visible.
2. **Type.** Send keys `End`, then type ` there`. Line 1 is `hello there`.
3. **Undo / redo.** `Ctrl+Z` (macOS `Cmd+Z`) restores `hello`. `Ctrl+Y` / `Ctrl+Shift+Z`
   restores `hello there`. Consecutive typed characters undo as one group, as in VS Code.
4. **Save.** `Ctrl+S`. The file on disk is `hello there\nworld\n`; the EOL and lack of BOM are
   preserved.
5. **Multi-cursor.** Place the cursor at 1:1, press `Ctrl+Alt+Down` (add cursor below), type
   `> `. Both lines start with `> `. `Escape` collapses to one cursor.
6. **Find / replace.** `Ctrl+H`, find `o`, replace with `0`, Replace All. The buffer is
   `> hell0 there\n> w0rld\n`. One `Ctrl+Z` undoes the whole replace.
7. **CRLF preservation.** Open `crlf.txt` containing `a\r\nb\r\n`, type `x` at 1:2, save. The
   file is `ax\r\nb\r\n`.
8. **Large file.** Open a generated file with 1,000,000 lines of 80 ASCII characters. Measured
   through the frame-timing probe:
   - time from launch to first painted frame with content is at most 2 s on CI hardware;
   - `Ctrl+End` reaches line 1,000,000 and paints within 100 ms;
   - scrolling 5,000 lines with 100 wheel events never takes a frame longer than 33 ms (no stutter);
   - typing one character at line 500,000 is visible within one frame (16 ms target, 33 ms limit).
9. **Exit.** Closing the window with unsaved changes shows the save prompt; choosing
   "Don't Save" exits with code 0.

## Out of scope for M1

Workbench chrome (tabs, explorer), syntax highlighting, settings files. Default keybindings are
hard-wired until M5.
