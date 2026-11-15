//! Terminal platform for Lathe (owner: role 09).
//!
//! Ports the process side of `src/vs/platform/terminal/` and the pure helpers it uses:
//! - [`environment`]: environment merge rules (`terminalEnvironment.ts`, `objects.ts`);
//! - [`launch`]: shell launch configuration and the checks run before spawning
//!   (`terminalProcess.ts` `_validateCwd` / `_validateExecutable`, `base/node/processes.ts`
//!   `findExecutable`).
//!
//! Following ADR 0005, this crate does blocking process and PTY work only when the caller runs
//! it on the I/O runtime. Results return to the UI thread through events.

pub mod environment;
pub mod launch;

pub use environment::{EnvironmentPatch, ProcessEnvironment, merge_environments};
pub use launch::{LaunchError, ShellLaunchConfig, validate_cwd, validate_executable};
