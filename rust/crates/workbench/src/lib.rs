//! Widget layout models for the Lathe workbench: split view and sash state.
//!
//! Each module cites the TypeScript source it ports from `src/vs/base/browser/ui/`. These are
//! layout models only: they compute sizes, visibility and sash states, and never touch a
//! renderer (ADR 0002 gives rendering to the widget layer built on top of them).

pub mod sash;
pub mod splitview;
