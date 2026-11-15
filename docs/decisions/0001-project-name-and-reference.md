# ADR 0001: Project name, license and reference source

Status: accepted (Phase 0)

## Context

The port reproduces the behavior of microsoft/vscode in Rust. The result must not use the
"Visual Studio Code" or "VS Code" name or logo, and must keep the upstream MIT license notice.
The TypeScript source is the behavioral reference.

## Decision

- The project is called **Lathe**. The binary is `lathe`. User-facing strings, window titles,
  product metadata and docs use "Lathe".
- Internal crate names follow the coordination prompt (`vscode-api`, `editor`, ...). They are
  identifiers, not branding, and never appear in the UI.
- The upstream `LICENSE.txt` (MIT, Copyright (c) Microsoft Corporation) stays at the repo root.
  Every Rust crate declares `license = "MIT"` through the workspace, and `rust/README.md` states
  that Lathe is derived from microsoft/vscode under that license.
- **Reference source:** the fork already contains the full upstream tree. Rather than cloning a
  second copy into `reference/`, the in-tree TypeScript (`src/`, `extensions/`, `cli/`) on
  `rust-port` is the reference. It is read-only: no PR may modify files outside `rust/`, `docs/`,
  `tests/e2e/`, `.github/workflows/rust-*.yml`, `COORDINATION.md` and `.gitignore`.
- The reference is pinned to upstream commit `d03edc4b18d687c45af067195bad9544b5b07475`
  (the `main` head when `rust-port` was created). Updating the pin is an Architect PR that
  merges `main` into `rust-port` and records the new commit here.

## Consequences

- Module doc comments cite paths such as `src/vs/base/common/event.ts`; those paths resolve on
  `rust-port` itself, so reviewers can check them without another checkout.
- No duplicate 100+ MB tree in the repo and no submodule to keep in sync.
