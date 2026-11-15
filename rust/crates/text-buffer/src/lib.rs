//! The text buffer: a piece tree with VS Code's line and offset semantics (ADR 0003).
//!
//! Ported from `src/vs/editor/common/model/pieceTreeTextBuffer/` (`pieceTreeBase.ts`,
//! `rbTreeBase.ts`, `pieceTreeTextBufferBuilder.ts`). Lines and columns are 1-based, columns and
//! offsets count UTF-16 code units, and `\r`, `\n` and `\r\n` each end a line.
//!
//! This layer adds the builder and the read-only piece tree. Edits and the `PieceTreeTextBuffer`
//! follow in later PRs of the stack.

// Edits (`insert`, `delete` and their helpers) have no caller yet. The next PR of this stack
// adds them and removes this allow.
#![allow(dead_code)]

mod builder;
mod piece;
mod piece_tree;
mod rbtree;

#[cfg(test)]
mod tests;

pub use builder::{TextBufferBuilder, TextBufferFactory};
pub use piece_tree::PieceTree;
