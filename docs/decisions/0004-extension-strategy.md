# ADR 0004: Extension strategy

Status: accepted (Phase 0); revisit at the start of M5

## Context

VS Code extensions run in a Node.js extension host (`src/vs/workbench/api/node/`,
`extHost.api.impl.ts`) that talks to the window over an RPC protocol
(`src/vs/workbench/services/extensions/common/rpcProtocol.ts`) with about 120 `MainThread*` /
`ExtHost*` shape pairs declared in `src/vs/workbench/api/common/extHost.protocol.ts`.
M5 requires one sample extension to run.

Options:

- **(a) Drop Node extensions in v1.** Cheapest; no extension ecosystem at all.
- **(b) Run an extension host as a Node subprocess speaking VS Code's RPC protocol.** Reuses the
  upstream extension host JavaScript unchanged (MIT), so real extensions run against the real
  `vscode` API. Lathe implements the `MainThread*` side in Rust, one shape at a time. Costs:
  Lathe needs a Node runtime, and the protocol is internal and unversioned, so it is pinned to the
  reference commit (ADR 0001).
- **(c) A new WASM plugin API.** Sandboxed and portable, but compatible with nothing; every
  extension would have to be rewritten.

## Decision

**(b)**, in increments:

1. `extensions` (role 10) builds the extension host JS from the reference tree, spawns it with
   Node, and implements the RPC framing (`ipc.net.ts` protocol plus the `rpcProtocol.ts` message
   format).
2. Implement the `MainThread*` shapes in this order: `Commands`, `Window` / `MessageService`,
   `Documents` + `DocumentsAndEditors`, `Configuration`, `Languages` (feature providers),
   `Diagnostics`, `StatusBar`, `OutputService`, `Terminal`, `Debug`. The M5 sample extension uses
   only the first three.
3. Unsupported shapes answer with a typed "not supported" error rather than hanging.

Fallback: if the protocol proves unworkable, (a) for v1 with the same `extensions` crate
boundary, decided by the Architect in an ADR.

## Consequences

- `vscode-api` must model commands, documents and configuration closely enough to serve the
  extension host. That favors VS Code's own shapes (string command IDs, JSON arguments,
  dotted configuration keys), which `vscode-api` adopts.
- Node is a runtime dependency only when extensions are enabled.
