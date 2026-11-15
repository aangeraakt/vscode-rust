//! Shared contracts for Lathe, a Rust port of the behavior of microsoft/vscode.
//!
//! This crate is owned by the Architect (role 01). Every other crate may depend on it, and it
//! depends on no other workspace crate. Changes need Architect approval (see `COORDINATION.md`);
//! prefer adding items over changing existing ones.
//!
//! Each module cites the TypeScript source it was ported from. Paths are relative to the
//! repository root at the reference commit pinned in `docs/decisions/0001-project-name-and-reference.md`.

pub mod event;
pub mod ids;
pub mod lifecycle;
pub mod services;

pub use event::{Emitter, Event};
pub use ids::{CommandId, IdGenerator, LanguageId};
pub use lifecycle::{Disposable, DisposableStore, LifecyclePhase, ShutdownReason};
pub use services::ServiceRegistry;
