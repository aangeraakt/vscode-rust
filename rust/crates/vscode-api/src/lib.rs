//! Shared contracts for Lathe, a Rust port of the behavior of microsoft/vscode.
//!
//! This crate is owned by the Architect (role 01). Every other crate may depend on it, and it
//! depends on no other workspace crate. Changes need Architect approval (see `COORDINATION.md`);
//! prefer adding items over changing existing ones.
//!
//! Each module cites the TypeScript source it was ported from. Paths are relative to the
//! repository root at the reference commit pinned in `docs/decisions/0001-project-name-and-reference.md`.

pub mod commands;
pub mod configuration;
pub mod editor;
pub mod event;
pub mod files;
pub mod ids;
pub mod lifecycle;
pub mod path;
pub mod services;
pub mod uri;

pub use commands::{CommandError, CommandHandler, CommandRegistry, CommandResult};
pub use configuration::{ConfigurationModel, ConfigurationOverride, ConfigurationTarget};
pub use editor::{Position, Range, Selection, SingleEditOperation, TextModel};
pub use event::{Emitter, Event};
pub use files::{FileSystemError, FileSystemErrorCode, FileSystemProvider};
pub use ids::{CommandId, IdGenerator, LanguageId};
pub use lifecycle::{Disposable, DisposableStore, LifecyclePhase, ShutdownReason};
pub use services::ServiceRegistry;
pub use uri::{Uri, UriError};
