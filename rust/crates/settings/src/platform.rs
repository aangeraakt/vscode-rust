//! Operating systems, for code that depends on the platform. Upstream: `OperatingSystem` in
//! `src/vs/base/common/platform.ts`.
//!
//! Platform-dependent behavior takes an `OperatingSystem` argument rather than `cfg`, so every
//! platform can be tested on every host.

/// The operating system whose keybinding conventions apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatingSystem {
    /// Windows.
    Windows,
    /// macOS.
    Macintosh,
    /// Linux.
    Linux,
}
