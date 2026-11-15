//! Terminal platform for Lathe (owner: role 09).
//!
//! Ports the process side of `src/vs/platform/terminal/` and the pure helpers it uses:
//! - [`environment`]: environment merge rules (`terminalEnvironment.ts`, `objects.ts`);
//! - [`launch`]: shell launch configuration and the checks run before spawning
//!   (`terminalProcess.ts` `_validateCwd` / `_validateExecutable`, `base/node/processes.ts`
//!   `findExecutable`);
//! - [`process`]: the PTY process host (`TerminalProcess`), using `portable-pty`.
//!
//! Following ADR 0005, [`TerminalProcess::spawn`] starts I/O threads and reports through a channel
//! of [`TerminalEvent`]s, which the caller forwards to the UI thread.

pub mod environment;
pub mod launch;
pub mod process;

pub use environment::{EnvironmentPatch, ProcessEnvironment, merge_environments};
pub use launch::{LaunchError, ShellLaunchConfig, validate_cwd, validate_executable};
pub use process::{TerminalEvent, TerminalProcess};
