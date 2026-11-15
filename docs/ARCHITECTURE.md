# Lathe architecture

Lathe is a native Rust port of the behavior of microsoft/vscode (MIT). The TypeScript tree on
this branch is the read-only reference (ADR 0001).

## Crate graph

All crates live in `rust/crates/`. Arrows point at dependencies.

```
                       vscode-api   (contracts only: no UI, no I/O runtime)
                           ^
   +-----------+-----------+-----------+-----------+-----------+
   |           |           |           |           |           |
platform  text-buffer  workspace    language     scm       settings
   ^           ^           ^           ^           ^           ^
   |        editor --------+-----------+           |           |
   |           ^                                   |           |
   +----- editor-view                              |           |
   |           ^                                   |           |
   +------ workbench <----- terminal, tasks, debug, extensions-+
               ^
             lathe  (binary: wires services, owns the event loop)
```

Rules:
- `vscode-api` depends on nothing in the workspace. Every crate may depend on it.
- Feature crates depend only on `vscode-api` and the crates shown below them. Any other edge
  needs an Architect-approved COORDINATION.md entry.
- The `lathe` binary crate (Architect) is the only place where concrete services are
  constructed and registered.

## Contracts in `vscode-api`

| Module | Ported from | Purpose |
|---|---|---|
| `ids` | `base/common/uuid.ts`, `platform/instantiation` service ids | typed IDs and handle generators |
| `lifecycle` | `base/common/lifecycle.ts`, `platform/lifecycle/common/lifecycle.ts` | `Disposable`, `DisposableStore`, lifecycle phases |
| `event` | `base/common/event.ts` | `Emitter<T>` / `Event<T>` (`Send` listeners); `LocalEmitter<T>` / `LocalEvent<T>` for UI-thread listeners (ADR 0007) |
| `uri` | `base/common/uri.ts` | `Uri` with VS Code parse/format and `fs_path` semantics |
| `services` | `platform/instantiation/common/` | type-keyed service registry |
| `commands` | `platform/commands/common/commands.ts` | command registry with stacking registration |
| `configuration` | `platform/configuration/common/configurationModels.ts` | configuration model, merge, language overrides |
| `files` | `platform/files/common/files.ts` | `FileSystemProvider` trait, stat, change events, errors |
| `editor` | `editor/common/core/{position,range}.ts`, `editor/common/model.ts` | positions, ranges, edits, `TextModel` trait |

## Runtime model (ADR 0005)

One process. The UI thread owns the models; I/O runs on tokio; results come back as event-loop
user events. External programs (LSP servers, debug adapters, shells, Node extension host) are
child processes.

## Rendering (ADR 0002)

winit + wgpu + cosmic-text/glyphon, with a small retained widget layer that mirrors VS Code's
`base/browser/ui`.

## Text (ADR 0003)

Piece tree; 1-based lines and columns; columns in UTF-16 code units.

## Extensions (ADR 0004)

Node extension host subprocess speaking VS Code's RPC protocol; the `MainThread*` shapes are
implemented incrementally in Rust.

## Testing

- Unit tests live next to the code. Parity claims cite the upstream test they port.
- `tests/e2e/` holds one scenario per milestone, written by the Architect before the work starts.
  The harness belongs to role 10.
