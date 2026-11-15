//! Editor core for Lathe, a Rust port of the behavior of microsoft/vscode (MIT).
//!
//! This crate is owned by role 04 (Editor Core). It depends only on `vscode-api`.
//!
//! Ported from `src/vs/editor/common/config/`:
//! - `editorOptions.ts`: the option validators (`boolean`, `clampedInt`, `clampedFloat`,
//!   `stringSet`) in [`value`].
//! - `fontInfo.ts`: the line height and zoom rules of `BareFontInfo._create`, the
//!   `GOLDEN_LINE_HEIGHT_RATIO` and `MINIMUM_LINE_HEIGHT` constants, in [`font`].
//!
//! Model indentation defaults come from `src/vs/editor/common/core/misc/textModelDefaults.ts`
//! (see [`model`]).

pub mod environment;
pub mod font;
pub mod model;
pub mod value;

pub use environment::{Environment, Platform};
pub use font::{FontMetrics, bare_font_metrics, default_font_size, golden_line_height_ratio};
pub use model::ModelOptions;
