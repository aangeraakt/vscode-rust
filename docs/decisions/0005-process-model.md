# ADR 0005: Process model

Status: accepted (Phase 0)

## Context

VS Code runs a main process, one renderer per window, a shared process, utility processes
(file watcher, PTY host, search) and extension hosts. The split exists largely because of Electron
and the single-threaded JavaScript event loop. Rust has threads and memory safety, so most of
these boundaries are unnecessary.

## Decision

**One process, multi-threaded:**

- The **UI thread** runs the winit event loop and owns all UI and model state: text models,
  view models, workbench layout. Models are accessed only from this thread and need no locks.
- A **tokio multi-thread runtime** runs I/O: the file system, the watcher (`notify`), search
  (`ignore` + `grep-searcher`), git, the LSP and DAP transports, and PTY I/O. Results return to
  the UI thread through a channel, as an event-loop user event.
- **Child processes** only where an external program is the point: language servers, debug
  adapters, shells (PTY), task commands, and the Node extension host (ADR 0004).
- Multiple windows live in the same process.

Contract consequences in `vscode-api`:
- Shared contract types are `Send + Sync` where they cross to the I/O runtime: the file system
  provider, the command registry and the event emitter.
- Async APIs return boxed `Send` futures, so they can run on tokio without tying the contract
  crate to tokio.

## Consequences

- No IPC serialization layer between UI and services, which means less code to port.
- A panic in a service can take the window down. Services run on tokio tasks with panics caught
  at the task boundary and reported as errors.
