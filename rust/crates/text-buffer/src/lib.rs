//! The text buffer: a piece tree with VS Code's line and offset semantics (ADR 0003).
//!
//! Ported from `src/vs/editor/common/model/pieceTreeTextBuffer/` (`pieceTreeBase.ts`,
//! `rbTreeBase.ts`, `pieceTreeTextBufferBuilder.ts`). Lines and columns are 1-based, columns and
//! offsets count UTF-16 code units, and `\r`, `\n` and `\r\n` each end a line.
//!
//! This is the first layer of the stack: the arena red-black tree and the pieces it stores.
//! The builder and the piece tree itself follow in later PRs.

// The tree and pieces have no caller until the piece tree lands in the next PR of this stack.
// That PR removes this allow.
#![allow(dead_code)]

mod piece;
mod rbtree;
