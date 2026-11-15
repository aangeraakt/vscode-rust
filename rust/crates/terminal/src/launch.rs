//! Shell launch configuration and the checks that run before a shell is spawned.
//!
//! Ported from:
//! - `src/vs/platform/terminal/node/terminalProcess.ts`: `_validateCwd` and `_validateExecutable`
//!   (the same error kinds, with the English text of the `localize` calls);
//! - `src/vs/base/node/processes.ts`: `findExecutable`.
//!
//! Matched behavior: a missing cwd or a cwd that is not a directory is an error; other `stat`
//! failures on the cwd are ignored, as in TypeScript. An executable must resolve to a regular file
//! (directories do not count). A permission error on the resolved executable is ignored.
//!
//! Divergence: when a permission error is ignored, TypeScript leaves the bare executable name for
//! node-pty to search on `PATH`. Here the resolved path is always returned.

use crate::environment::{EnvironmentPatch, ProcessEnvironment, get_case_insensitive};
use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// What to start in the terminal. Port of the parts of `IShellLaunchConfig` the process host uses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellLaunchConfig {
    /// The shell executable: a name looked up on `PATH`, or a path.
    pub executable: Option<String>,
    /// Arguments passed to the executable.
    pub args: Vec<String>,
    /// Base directory for resolving a relative executable path. The shell's own working
    /// directory is given to the spawn call, not here.
    pub cwd: Option<PathBuf>,
    /// Changes applied over the inherited environment for the shell. `PATH` in this patch is
    /// searched when resolving the executable.
    pub env: EnvironmentPatch,
}

/// Why a launch was refused before or during spawning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    /// No executable was configured (TypeScript throws `IShellLaunchConfig.executable not set`).
    ExecutableNotSet,
    /// The starting directory does not exist.
    CwdDoesNotExist(PathBuf),
    /// The starting directory exists but is not a directory.
    CwdNotDirectory(PathBuf),
    /// The executable could not be found.
    ExecutableDoesNotExist(String),
    /// The executable resolved to something other than a regular file.
    ExecutableIsNotFileOrSymlink(String),
    /// An unexpected I/O failure while checking the executable.
    Io(String),
    /// The PTY could not be created or the process could not be started.
    Spawn(String),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExecutableNotSet => write!(f, "IShellLaunchConfig.executable not set"),
            Self::CwdDoesNotExist(cwd) => write!(
                f,
                "Starting directory (cwd) \"{}\" does not exist",
                cwd.display()
            ),
            Self::CwdNotDirectory(cwd) => write!(
                f,
                "Starting directory (cwd) \"{}\" is not a directory",
                cwd.display()
            ),
            Self::ExecutableDoesNotExist(exe) => {
                write!(f, "Path to shell executable \"{exe}\" does not exist")
            }
            Self::ExecutableIsNotFileOrSymlink(exe) => {
                write!(
                    f,
                    "Path to shell executable \"{exe}\" is not a file or a symlink"
                )
            }
            Self::Io(message) => write!(f, "{message}"),
            Self::Spawn(message) => {
                write!(f, "A native exception occurred during launch ({message})")
            }
        }
    }
}

impl std::error::Error for LaunchError {}

/// Checks that `cwd` is an existing directory (`_validateCwd`).
pub fn validate_cwd(cwd: &Path) -> Result<(), LaunchError> {
    match fs::metadata(cwd) {
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(LaunchError::CwdNotDirectory(cwd.to_path_buf())),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            Err(LaunchError::CwdDoesNotExist(cwd.to_path_buf()))
        }
        // Other stat failures are ignored, as in `_validateCwd`.
        Err(_) => Ok(()),
    }
}

/// Resolves the configured executable and checks that it is a file (`_validateExecutable`).
///
/// `env` is the environment used for the `PATH` and `PATHEXT` lookup (TypeScript
/// `_executableEnv`). `is_windows` selects the Windows lookup rules.
/// Returns the resolved path, which the spawn call should use directly.
pub fn validate_executable(
    config: &ShellLaunchConfig,
    env: &ProcessEnvironment,
    is_windows: bool,
) -> Result<PathBuf, LaunchError> {
    let executable = config
        .executable
        .as_deref()
        .filter(|e| !e.is_empty())
        .ok_or(LaunchError::ExecutableNotSet)?;
    let config_paths: Option<Vec<PathBuf>> = config
        .env
        .get("PATH")
        .and_then(|v| v.as_deref())
        .map(|p| env::split_paths(p).collect());
    let found = find_executable(
        executable,
        config.cwd.as_deref(),
        config_paths.as_deref(),
        env,
        is_windows,
    )
    .ok_or_else(|| LaunchError::ExecutableDoesNotExist(executable.to_owned()))?;

    match fs::metadata(&found) {
        Ok(meta) if meta.is_file() => Ok(found),
        Ok(_) => Err(LaunchError::ExecutableIsNotFileOrSymlink(
            executable.to_owned(),
        )),
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => Ok(found),
        Err(err) => Err(LaunchError::Io(err.to_string())),
    }
}

/// Port of `findExecutable`. Returns the first existing regular file that `command` resolves to.
///
/// - An absolute `command` is taken as is.
/// - A `command` with a directory part is joined to `cwd` (default: the process working
///   directory).
/// - A bare name is searched in `paths`, or in `PATH` from `env` when `paths` is `None`. On
///   Windows each entry is also tried with each `PATHEXT` extension first.
/// - Otherwise the name is tried in `cwd`.
pub fn find_executable(
    command: &str,
    cwd: Option<&Path>,
    paths: Option<&[PathBuf]>,
    env: &ProcessEnvironment,
    is_windows: bool,
) -> Option<PathBuf> {
    let command_path = Path::new(command);
    if command_path.is_absolute() {
        return is_regular_file(command_path).then(|| command_path.to_path_buf());
    }

    let cwd = cwd
        .map(Path::to_path_buf)
        .unwrap_or_else(|| env::current_dir().unwrap_or_default());
    let has_directory_part = command_path
        .parent()
        .is_some_and(|dir| !dir.as_os_str().is_empty() && dir != Path::new("."));
    if has_directory_part {
        let full = cwd.join(command_path);
        return is_regular_file(&full).then_some(full);
    }

    let env_paths: Vec<PathBuf>;
    let search: &[PathBuf] = match paths {
        Some(paths) => paths,
        None => {
            env_paths = get_case_insensitive(env, "PATH")
                .map(|p| env::split_paths(p).collect())
                .unwrap_or_default();
            &env_paths
        }
    };

    if search.is_empty() {
        let full = cwd.join(command);
        return is_regular_file(&full).then_some(full);
    }

    let pathext = if is_windows {
        get_case_insensitive(env, "PATHEXT")
            .filter(|s| !s.is_empty())
            .map(String::as_str)
            .unwrap_or(".COM;.EXE;.BAT;.CMD")
    } else {
        ""
    };
    for entry in search {
        let dir = if entry.is_absolute() {
            entry.clone()
        } else {
            cwd.join(entry)
        };
        let full = dir.join(command);
        if is_windows {
            for ext in pathext.split(';') {
                let mut with_ext = full.clone().into_os_string();
                with_ext.push(ext);
                let with_ext = PathBuf::from(with_ext);
                if is_regular_file(&with_ext) {
                    return Some(with_ext);
                }
            }
        }
        if is_regular_file(&full) {
            return Some(full);
        }
    }
    let full = cwd.join(command);
    is_regular_file(&full).then_some(full)
}

/// `fileExistsDefault`: exists and is not a directory. Symbolic links are followed.
fn is_regular_file(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|meta| !meta.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env() -> ProcessEnvironment {
        ProcessEnvironment::new()
    }

    fn touch(path: &Path) {
        fs::write(path, b"").unwrap();
    }

    fn path_list(dirs: &[&Path]) -> String {
        env::join_paths(dirs).unwrap().into_string().unwrap()
    }

    #[test]
    fn cwd_validation_reports_missing_and_non_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(validate_cwd(dir.path()), Ok(()));

        let missing = dir.path().join("missing");
        assert_eq!(
            validate_cwd(&missing),
            Err(LaunchError::CwdDoesNotExist(missing.clone()))
        );

        let file = dir.path().join("file.txt");
        touch(&file);
        assert_eq!(
            validate_cwd(&file),
            Err(LaunchError::CwdNotDirectory(file.clone()))
        );
        assert_eq!(
            LaunchError::CwdDoesNotExist(missing).to_string(),
            format!(
                "Starting directory (cwd) \"{}\" does not exist",
                dir.path().join("missing").display()
            )
        );
    }

    #[test]
    fn executable_must_be_set_and_be_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = ShellLaunchConfig::default();
        assert_eq!(
            validate_executable(&config, &no_env(), false),
            Err(LaunchError::ExecutableNotSet)
        );

        let config = ShellLaunchConfig {
            executable: Some("no-such-shell-xyz".into()),
            cwd: Some(dir.path().to_path_buf()),
            ..Default::default()
        };
        assert_eq!(
            validate_executable(&config, &no_env(), false),
            Err(LaunchError::ExecutableDoesNotExist(
                "no-such-shell-xyz".into()
            ))
        );

        // A directory does not count as an executable.
        let config = ShellLaunchConfig {
            executable: Some(dir.path().display().to_string()),
            ..Default::default()
        };
        assert_eq!(
            validate_executable(&config, &no_env(), false),
            Err(LaunchError::ExecutableDoesNotExist(
                dir.path().display().to_string()
            ))
        );
    }

    #[test]
    fn executable_resolves_absolute_and_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        let shell = dir.path().join("myshell");
        touch(&shell);

        let absolute = ShellLaunchConfig {
            executable: Some(shell.display().to_string()),
            ..Default::default()
        };
        assert_eq!(
            validate_executable(&absolute, &no_env(), false),
            Ok(shell.clone())
        );

        let relative = ShellLaunchConfig {
            executable: Some("myshell".into()),
            cwd: Some(dir.path().to_path_buf()),
            ..Default::default()
        };
        assert_eq!(validate_executable(&relative, &no_env(), false), Ok(shell));
    }

    #[test]
    fn bare_name_is_searched_in_the_patch_path() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let tool = second.path().join("tool");
        touch(&tool);

        let mut patch = EnvironmentPatch::new();
        patch.insert(
            "PATH".into(),
            Some(path_list(&[first.path(), second.path()])),
        );
        let config = ShellLaunchConfig {
            executable: Some("tool".into()),
            env: patch,
            ..Default::default()
        };
        assert_eq!(validate_executable(&config, &no_env(), false), Ok(tool));
    }

    #[test]
    fn windows_lookup_tries_pathext_extensions() {
        let dir = tempfile::tempdir().unwrap();
        let tool = dir.path().join("tool.CMD");
        touch(&tool);
        let dirs = [dir.path().to_path_buf()];
        assert_eq!(
            find_executable("tool", None, Some(&dirs), &no_env(), true),
            Some(tool)
        );
        // Without the Windows rules the extension is not added.
        assert_eq!(
            find_executable("tool", None, Some(&dirs), &no_env(), false),
            None
        );
    }

    #[test]
    fn find_executable_reads_path_from_env_when_no_paths_given() {
        let dir = tempfile::tempdir().unwrap();
        let tool = dir.path().join("tool");
        touch(&tool);
        let mut env = ProcessEnvironment::new();
        env.insert("Path".into(), path_list(&[dir.path()]));
        assert_eq!(find_executable("tool", None, None, &env, false), Some(tool));
    }

    #[test]
    fn launch_error_messages_match_terminal_process() {
        assert_eq!(
            LaunchError::ExecutableIsNotFileOrSymlink("sh".into()).to_string(),
            "Path to shell executable \"sh\" is not a file or a symlink"
        );
        assert_eq!(
            LaunchError::Spawn("boom".into()).to_string(),
            "A native exception occurred during launch (boom)"
        );
    }
}
