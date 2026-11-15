# ADR 0003: Text buffer structure and position semantics

Status: accepted (Phase 0)

## Context

VS Code's text model is a piece tree: a red-black tree of pieces over append-only string buffers
with cached line starts (`src/vs/editor/common/model/pieceTreeTextBuffer/`). Its public semantics
(`src/vs/editor/common/model.ts`, `core/position.ts`, `core/range.ts`) are:

- lines and columns are **1-based**;
- columns and offsets count **UTF-16 code units**, as JavaScript strings do and as LSP does by
  default;
- line terminators are `\n`, `\r\n` or `\r` on input; a model has one EOL (`LF` or `CRLF`)
  and normalizes on load according to `files.eol` / content detection;
- a BOM is stripped and remembered;
- every edit bumps `versionId`; undo/redo restore an `alternativeVersionId`.

A rope (ropey, crop) is the common Rust choice, but its line semantics treat `\r` and Unicode
line separators differently from VS Code, and it indexes by chars or bytes.

## Decision

- Port the **piece tree** from `pieceTreeBase.ts` / `pieceTreeTextBuffer.ts` and
  `pieceTreeTextBufferBuilder.ts`, storing chunks as UTF-8 `String`s with a per-chunk UTF-16
  line-start index, so lookups by (line, UTF-16 column) stay O(log n).
- The public coordinate system in `vscode-api` is VS Code's: `Position { line_number, column }`,
  1-based, columns in UTF-16 code units. Conversion to byte offsets is internal to `text-buffer`.
- The upstream unit tests (`src/vs/editor/test/common/model/pieceTreeTextBuffer/`) are ported as
  the parity suite, including the randomized fuzz tests.

## Consequences

- Parity tests port nearly verbatim, and LSP positions need no conversion.
- Every column-to-byte conversion costs a scan inside one line; lines are short in practice.
  Very long lines (minified files) are covered by a dedicated benchmark in M1.
