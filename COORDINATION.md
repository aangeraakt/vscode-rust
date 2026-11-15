# Coordination

Read this first in every session. Changes arrive only by PR into `rust-port`. Reviews follow
ADR 0006 (`docs/decisions/0006-review-on-a-shared-account.md`).

## Crate owners

| Role | Agent | Crates / paths |
|---|---|---|
| 01 | Architect / Coordinator | `rust/crates/vscode-api`, `rust/crates/lathe`, `rust/Cargo.toml`, `docs/`, `COORDINATION.md`, `tests/e2e/*.md` (scenarios) |
| 02 | Platform & Window | `rust/crates/platform` |
| 03 | Text Buffer | `rust/crates/text-buffer` |
| 04 | Editor Core | `rust/crates/editor` |
| 05 | Editor Rendering | `rust/crates/editor-view` |
| 06 | Language Services | `rust/crates/language` |
| 07 | Workbench & Layout | `rust/crates/workbench` |
| 08 | Workspace & Search | `rust/crates/workspace` |
| 09 | Terminal, Tasks & Debug | `rust/crates/terminal`, `rust/crates/tasks`, `rust/crates/debug` |
| 10 | SCM, Settings & Extensions, CI | `rust/crates/scm`, `rust/crates/settings`, `rust/crates/extensions`, `.github/workflows/rust-*.yml`, `tests/e2e/` harness |

An owner creates their crate in their first PR; the workspace picks up `rust/crates/*`
automatically.

## Phase status

- Phase 0 (Architecture): in progress. Feature work starts when the `vscode-api` crate has
  merged with an approval.

## Open interface requests

Format: `- [ ] YYYY-MM-DD, requester role NN: <what> in <module>, because <why>. PR: <link>`

_None._

## Blockers

Format: `- YYYY-MM-DD, role NN, PORT_MAP row "<area>": <blocker>; waiting on <role/PR>`

_None._

## Status (one dated line per agent, newest last)

- 2026-11-15, 01: Phase 0 started. ADRs 0001–0006 and ARCHITECTURE.md written; PORT_MAP and `vscode-api` next.
- 2026-11-15, 02–10: reading reference source for their areas; waiting on `vscode-api` approval.
