# Handoff

Updated at the end of every session. Start here, then read `COORDINATION.md`.

## Session 1 — 2026-11-15 (Architect, Phase 0)

### Merged into `rust-port`

| PR | What |
|---|---|
| #1 | ADRs 0001–0006 (name **Lathe** and reference pin, UI toolkit, piece tree + UTF-16 columns, extension host strategy (b), single process + tokio, review policy), `docs/ARCHITECTURE.md`, `COORDINATION.md`, M1 e2e scenario `tests/e2e/m1-editor.md` |
| #2 | `docs/PORT_MAP.md` (survey of `src/vs`, `cli/src`, `extensions/`) |
| #3 (+ #4 squashed in) | Rust workspace in `rust/`; `vscode-api`: `ids`, `lifecycle`, `event`, `services`, `uri` |
| #5 | `vscode-api::commands` and PORT_MAP bookkeeping |
| #6 | `vscode-api::configuration`, plus review fixes for #5 (handler dropped outside the lock, real port of "register/register/dispose") |
| #7 | `vscode-api::files` provider contract |
| #8 | `vscode-api::editor`: position, range, selection, edits, `TextModel` trait |

The repository owner merged #1–#5 before any non-author review. #5 was reviewed afterwards
(APPROVE). #6–#8 merged after an `APPROVE` review comment from a non-author role (ADR 0006:
Agent 10 for #6, Agent 08 for #7, Agent 03 for #8) and green local gates (build, test, clippy
`-D warnings`, fmt; 56 tests).

**Phase 0 is complete:** the `vscode-api` crate is merged and approved. Phase 1 (feature work)
can start.

### Open

| PR | Branch | Contents |
|---|---|---|
| this PR | `agent/01-handoff-session1` | HANDOFF.md, COORDINATION.md status and interface items |

Lesson for stacked PRs: after a squash merge, rebase the next branch with
`git rebase --onto origin/rust-port <old tip of the merged branch>`. A plain
`git rebase origin/rust-port` replays the pre-squash commits and conflicts.

### Blocked

Nothing in the code is blocked. Process gaps:
- **No CI yet.** The `rust-port` protection requires PRs but no status checks, because no workflow exists. Until role 10 lands CI, every author runs the four gates locally and pastes the result in the PR.
- **Shared GitHub account.** Approvals are review comments (ADR 0006), so merges rely on discipline. Owner merges that skip review should still get a review comment afterwards, as happened for #5.

### Open interface items

The deferred `vscode-api` items are listed in `COORDINATION.md` under "Open interface requests".

### Exact next tasks (Phase 1)

Each agent opens a claim PR for one PORT_MAP row first, then works it.

1. **Role 10:** `.github/workflows/rust-ci.yml` running build, test, `clippy -D warnings` and `fmt --check` on Ubuntu and Windows for PRs into `rust-port` (paths `rust/**`). Then ask the Architect to make it a required status check. **Do this first.** Then the row "Keybinding model and key codes".
2. **Role 03:** row "Piece tree text buffer": port `pieceTreeBase.ts`, `pieceTreeTextBuffer.ts`, the builder, and the upstream test suite `src/vs/editor/test/common/model/pieceTreeTextBuffer/`.
3. **Role 02:** row "Window, native host": winit window and event loop, plus a draw-layer API proposal (ADR 0002) as a COORDINATION request.
4. **Role 08:** row "Files: disk provider": a `file`-scheme `FileSystemProvider` on tokio and `notify`.
5. **Role 04:** row "Editor options" (`src/vs/editor/common/config/`): pure logic; cursor work waits on role 03.
6. **Role 05:** row "Line layout, scroll model" (`src/vs/editor/common/viewLayout/`): pure logic.
7. **Role 06:** row "Tokenization and language configuration": pure logic.
8. **Role 07:** row "UI widgets: split view, grid, sash": layout model first, without rendering.
9. **Role 09:** row "Terminal platform (PTY host)": PTY spawn and resize with `portable-pty`.
10. **Role 01:** rows "Resource and path helpers", "Async primitives", then the `lathe` binary skeleton; write the M2 scenario in `tests/e2e/` before M2 starts.
