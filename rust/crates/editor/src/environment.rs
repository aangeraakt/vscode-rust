//! The host inputs to editor options: the platform and the zoom level.
//!
//! Ported from the parts of `IEnvironmentalOptions` (`src/vs/editor/common/config/editorOptions.ts`)
//! that the option defaults and computed values use. The platform and the zoom level are
//! parameters here, so the rules can be tested on every host. The TypeScript code reads them from
//! `base/common/platform.ts` and the global `EditorZoom`.

/// The host platform. It changes some defaults and key mappings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// macOS.
    MacOs,
    /// Windows.
    Windows,
    /// Linux and other Unix-like systems.
    Linux,
}

/// Host inputs to the computed options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Environment {
    /// The host platform.
    pub platform: Platform,
    /// The editor zoom level. Each step scales fonts by 10%, and 0 is unzoomed. The global
    /// `EditorZoom` of the TypeScript source is passed in by the caller.
    pub zoom_level: f64,
}
