//! The text buffer: a piece tree with VS Code's line and offset semantics (ADR 0003).
//!
//! Ported from `src/vs/editor/common/model/pieceTreeTextBuffer/` (`pieceTreeBase.ts`,
//! `rbTreeBase.ts`, `pieceTreeTextBufferBuilder.ts`). Lines and columns are 1-based, columns and
//! offsets count UTF-16 code units, and `\r`, `\n` and `\r\n` each end a line.
//!
//! Storage: an arena red-black tree (`rbtree`) of pieces over UTF-8 buffers (`piece`). The
//! public API is safe Rust with no `unsafe`.
//!
//! Not in this crate yet: the search methods, snapshots, and the `TextModel` implementation
//! (versioning, undo, decorations). Those belong to the "Text model" and "Decorations" rows.

mod builder;
mod piece;
mod piece_tree;
mod rbtree;
mod text_buffer;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod text_buffer_tests;

pub use builder::{TextBufferBuilder, TextBufferFactory};
pub use piece_tree::PieceTree;
pub use text_buffer::{ApplyEditsResult, PieceTreeTextBuffer};
