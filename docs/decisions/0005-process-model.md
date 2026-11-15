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

## Amendment (2026-11-15)

Two services found that the tokio rule above does not fit every OS API. The file watcher (PR #54,
`notify`, one batching thread per watch) and the PTY host (PR #47, `portable-pty` reader and waiter
threads) use dedicated `std::thread`s. Both libraries expose blocking handles and no async API.
`FileSystemProvider::watch` is synchronous and can be called without a runtime, so it cannot assume
tokio is running. This amendment settles the rule:

- **tokio is for async I/O**: sockets, files, and child-process pipes that have async APIs.
- **Blocking OS handles** with no async API (PTY reads and waits, native file-watcher callbacks)
  may run on **dedicated threads owned by the service that created them**. A service that
  creates such a thread is responsible for stopping it.
- Such a thread **never touches UI-thread state**. It delivers results as messages, through a
  channel or a `Send` `Emitter`. The UI side forwards them to the event loop as user events, the
  same way it forwards tokio results.
- Each such thread **must have a documented shutdown path**: dropping the handle or killing the
  child stops the thread. The service's docs name the path.
- Such a thread **must not block the UI thread**. Nothing on the UI thread waits on it.

Sources: PR #47 (PTY host, `portable-pty` reader and waiter threads) and PR #54 (file watcher,
`notify` batching thread per watch). Both were raised in review.
