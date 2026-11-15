//! PTY process host: starts a shell in a pseudo-terminal and carries its I/O and exit.
//!
//! Ported from `src/vs/platform/terminal/node/terminalProcess.ts` (`TerminalProcess`): spawn with
//! a launch config, `resize`, `input`, data and exit reporting, and `shutdown`. The PTY comes from
//! `portable-pty` (ConPTY on Windows, `openpty` elsewhere) in place of node-pty.
//!
//! Threads and events (ADR 0005): [`TerminalProcess::spawn`] starts a reader thread and a waiter
//! thread. Both send [`TerminalEvent`]s on the channel returned from `spawn`. The channel buffers
//! events until the consumer reads them, so output produced before the consumer starts reading is
//! not lost. The consumer forwards them to the UI thread (an event-loop user event).
//!
//! Consumer contract: on Windows, ConPTY asks the terminal for the cursor position (`ESC [ 6 n`)
//! and does not run the shell until the reply is written back with [`TerminalProcess::input`].
//! The host does not answer it, because the cursor belongs to the terminal emulator. The
//! emulator (xterm.js in VS Code) must answer it. The tests do the same.
//!
//! Matched behavior:
//! - the cwd and executable are checked before any PTY is opened (see [`crate::launch`]);
//! - the shell environment is the base environment with the launch config's patch merged in;
//! - `resize` clamps both dimensions to at least 1 and does nothing after exit;
//! - the exit event follows trailing output: the waiter gives the reader up to 250 ms to finish
//!   (`DataFlushTimeout`) before it closes the PTY and reports the exit code;
//! - `shutdown(false)` kills the process if it has not exited within the flush timeout; `kill`
//!   on an exited process is a no-op.
//!
//! Divergences:
//! - The flush wait is a fixed 250 ms after the child exits. TypeScript restarts it on every data
//!   event, and caps a shutdown at 5 s.
//! - Data events carry raw bytes; node-pty decodes UTF-8 for the caller. Sequences split across
//!   reads are left to the consumer.
//! - Not ported: flow control (`FlowControlConstants`), kill/spawn throttling for ConPTY
//!   (microsoft/vscode#71966), shell integration injection, process title and cwd polling, the
//!   Windows ConPTY DLL options, pixel sizes, and `clearBuffer`.
//! - Events after the exit event are possible when output is still arriving after the flush
//!   timeout. TypeScript has the same race.

use crate::environment::{ProcessEnvironment, merge_environments};
use crate::launch::{LaunchError, ShellLaunchConfig, validate_cwd, validate_executable};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::fmt::Display;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

/// How long the waiter gives trailing output to arrive after the child exits.
const DATA_FLUSH_TIMEOUT: Duration = Duration::from_millis(250);
const READ_BUFFER_SIZE: usize = 4096;

/// Something the terminal process reports to its consumer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    /// Bytes the shell wrote to the terminal.
    Data(Vec<u8>),
    /// The shell exited with this code (0 when the code could not be read).
    Exit(u32),
}

/// A shell running in a pseudo-terminal.
///
/// Dropping the handle kills the shell if it is still running, so a shell never outlives the
/// handle that started it.
pub struct TerminalProcess {
    exit_state: Arc<ExitState>,
    master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Arc<Mutex<Box<dyn ChildKiller + Send + Sync>>>,
}

/// Exit code shared between the waiter thread and the methods that check or wait for it.
#[derive(Default)]
struct ExitState {
    code: Mutex<Option<u32>>,
    changed: Condvar,
}

impl ExitState {
    fn code(&self) -> Option<u32> {
        *lock(&self.code)
    }

    fn set(&self, code: u32) {
        *lock(&self.code) = Some(code);
        self.changed.notify_all();
    }

    /// Waits up to `timeout` for the exit code. Returns `None` if the process is still running.
    fn wait(&self, timeout: Duration) -> Option<u32> {
        let guard = lock(&self.code);
        let (guard, _) = self
            .changed
            .wait_timeout_while(guard, timeout, |code| code.is_none())
            .unwrap_or_else(PoisonError::into_inner);
        *guard
    }
}

impl TerminalProcess {
    /// Validates the launch config, opens a PTY of `cols` x `rows` and starts the shell in
    /// `cwd`. The shell environment is `base_env` with `config.env` merged over it.
    ///
    /// `cols` and `rows` are clamped to at least 1. `is_windows` selects the Windows lookup and
    /// environment rules. Returns the process and the channel its events arrive on.
    pub fn spawn(
        config: &ShellLaunchConfig,
        base_env: &ProcessEnvironment,
        cwd: &Path,
        cols: u16,
        rows: u16,
        is_windows: bool,
    ) -> Result<(Self, Receiver<TerminalEvent>), LaunchError> {
        validate_cwd(cwd)?;
        let executable = validate_executable(config, base_env, is_windows)?;
        let mut env = base_env.clone();
        merge_environments(&mut env, &config.env, is_windows);

        let (cols, rows) = clamp_size(cols, rows);
        let pair = native_pty_system()
            .openpty(pty_size(cols, rows))
            .map_err(spawn_error)?;
        let mut command = CommandBuilder::new(&executable);
        command.args(&config.args);
        command.cwd(cwd);
        command.env_clear();
        for (key, value) in &env {
            command.env(key, value);
        }
        let child = pair.slave.spawn_command(command).map_err(spawn_error)?;
        // Closing our copy of the slave lets the reader see end of file when the shell exits.
        drop(pair.slave);

        let reader = pair.master.try_clone_reader().map_err(spawn_error)?;
        let writer = pair.master.take_writer().map_err(spawn_error)?;
        let killer = child.clone_killer();
        let master = Arc::new(Mutex::new(Some(pair.master)));
        let exit_state = Arc::new(ExitState::default());
        let (events_tx, events_rx) = mpsc::channel();
        let (reader_done_tx, reader_done_rx) = mpsc::channel::<()>();

        let output_tx = events_tx.clone();
        thread::spawn(move || {
            pump_output(reader, &output_tx);
            let _ = reader_done_tx.send(());
        });

        let waiter_state = Arc::clone(&exit_state);
        let waiter_master = Arc::clone(&master);
        let mut child = child;
        thread::spawn(move || {
            let code = child.wait().map(|status| status.exit_code()).unwrap_or(0);
            // Let trailing output reach the channel before the PTY is closed.
            let _ = reader_done_rx.recv_timeout(DATA_FLUSH_TIMEOUT);
            drop(lock(&waiter_master).take());
            waiter_state.set(code);
            let _ = events_tx.send(TerminalEvent::Exit(code));
        });

        Ok((
            Self {
                exit_state,
                master,
                writer: Mutex::new(writer),
                killer: Arc::new(Mutex::new(killer)),
            },
            events_rx,
        ))
    }

    /// The exit code, once the process has exited.
    pub fn exit_code(&self) -> Option<u32> {
        self.exit_state.code()
    }

    /// Writes `data` to the shell's input. Port of `input` / `processBinary`: the caller encodes
    /// text as UTF-8 first.
    pub fn input(&self, data: &[u8]) -> io::Result<()> {
        let mut writer = lock(&self.writer);
        writer.write_all(data)?;
        writer.flush()
    }

    /// Resizes the terminal, clamping both dimensions to at least 1. Does nothing after exit.
    pub fn resize(&self, cols: u16, rows: u16) -> io::Result<()> {
        if self.exit_state.code().is_some() {
            return Ok(());
        }
        let (cols, rows) = clamp_size(cols, rows);
        let master = lock(&self.master);
        let Some(master) = master.as_ref() else {
            return Ok(());
        };
        match master.resize(pty_size(cols, rows)) {
            Ok(()) => Ok(()),
            // A pty that exits during the call is not an error, as in TypeScript.
            Err(_) if self.exit_state.code().is_some() => Ok(()),
            Err(err) => Err(io::Error::other(err.to_string())),
        }
    }

    /// Kills the shell now, if it is still running. Best effort: the outcome is reported through
    /// the exit event, as TypeScript's `_kill` swallows kill errors. (portable-pty 0.9 also
    /// reports a successful Windows `TerminateProcess` as an error, so the result could not be
    /// trusted anyway.)
    pub fn kill(&self) {
        if self.exit_state.code().is_none() {
            let _ = lock(&self.killer).kill();
        }
    }

    /// Ends the shell. With `immediate` the kill happens now (except on Windows, where TypeScript
    /// defers it to avoid a ConPTY hang, microsoft/vscode#71966). Otherwise the kill happens if the
    /// shell has not exited within the flush timeout.
    pub fn shutdown(&self, immediate: bool) {
        if immediate && !cfg!(windows) {
            self.kill();
            return;
        }
        let exit_state = Arc::clone(&self.exit_state);
        let killer = Arc::clone(&self.killer);
        thread::spawn(move || {
            if exit_state.wait(DATA_FLUSH_TIMEOUT).is_none() {
                let _ = lock(&killer).kill();
            }
        });
    }

    #[cfg(test)]
    fn pty_size(&self) -> Option<(u16, u16)> {
        lock(&self.master)
            .as_ref()
            .and_then(|master| master.get_size().ok())
            .map(|size| (size.cols, size.rows))
    }
}

impl Drop for TerminalProcess {
    fn drop(&mut self) {
        // A shell must not outlive the handle that started it.
        self.kill();
    }
}

fn pump_output(mut reader: Box<dyn Read + Send>, events: &Sender<TerminalEvent>) {
    let mut buffer = [0u8; READ_BUFFER_SIZE];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                if events
                    .send(TerminalEvent::Data(buffer[..read].to_vec()))
                    .is_err()
                {
                    break;
                }
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

fn clamp_size(cols: u16, rows: u16) -> (u16, u16) {
    (cols.max(1), rows.max(1))
}

fn pty_size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn spawn_error(err: impl Display) -> LaunchError {
    LaunchError::Spawn(err.to_string())
}

/// Locks a mutex, recovering the data after a panic in another thread.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// Upper bound for any wait in these tests, so a hang fails instead of stalling CI.
    const TIMEOUT: Duration = Duration::from_secs(30);

    fn base_env() -> ProcessEnvironment {
        std::env::vars().collect()
    }

    fn launch(executable: &str, args: &[&str]) -> ShellLaunchConfig {
        ShellLaunchConfig {
            executable: Some(executable.into()),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            ..Default::default()
        }
    }

    /// A one-line shell command, written once per platform.
    fn shell_command(windows_cmd: &str, unix_script: &str) -> ShellLaunchConfig {
        if cfg!(windows) {
            launch("cmd", &["/c", windows_cmd])
        } else {
            launch("sh", &["-c", unix_script])
        }
    }

    /// A command that keeps running until it is killed.
    fn long_running() -> ShellLaunchConfig {
        if cfg!(windows) {
            launch("ping", &["-n", "120", "127.0.0.1"])
        } else {
            launch("sleep", &["120"])
        }
    }

    fn start(config: &ShellLaunchConfig) -> (TerminalProcess, Receiver<TerminalEvent>) {
        let cwd = std::env::temp_dir();
        TerminalProcess::spawn(config, &base_env(), &cwd, 80, 24, cfg!(windows))
            .expect("spawn should succeed")
    }

    /// The cursor position request ConPTY sends before the shell runs.
    const CURSOR_POSITION_REQUEST: &[u8] = b"\x1b[6n";
    /// A reply to it, as a terminal emulator sends it (here: the cursor at 1,1).
    const CURSOR_POSITION_REPLY: &[u8] = b"\x1b[1;1R";

    /// Reads events until the exit event, returning the output text and the exit code. Plays the
    /// terminal emulator's part: a cursor position request is answered, which ConPTY waits for.
    fn run_to_exit(process: &TerminalProcess, events: &Receiver<TerminalEvent>) -> (String, u32) {
        let deadline = Instant::now() + TIMEOUT;
        let mut output = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match events.recv_timeout(remaining) {
                Ok(TerminalEvent::Data(bytes)) => {
                    if bytes
                        .windows(CURSOR_POSITION_REQUEST.len())
                        .any(|w| w == CURSOR_POSITION_REQUEST)
                    {
                        process.input(CURSOR_POSITION_REPLY).unwrap();
                    }
                    output.extend(bytes);
                }
                Ok(TerminalEvent::Exit(code)) => {
                    return (String::from_utf8_lossy(&output).into_owned(), code);
                }
                Err(err) => panic!("process did not exit in time: {err}"),
            }
        }
    }

    #[test]
    fn echo_writes_output_and_reports_exit_zero() {
        let (process, events) = start(&shell_command("echo hello", "echo hello"));
        let (output, code) = run_to_exit(&process, &events);
        assert!(output.contains("hello"), "output was {output:?}");
        assert_eq!(code, 0);
        assert_eq!(process.exit_code(), Some(0));
    }

    #[test]
    fn non_zero_exit_code_is_reported() {
        let (process, events) = start(&shell_command("exit 3", "exit 3"));
        let (_, code) = run_to_exit(&process, &events);
        assert_eq!(code, 3);
    }

    #[test]
    fn input_reaches_the_shell() {
        if cfg!(windows) {
            // `set /p` exits only after it reads a line, so an exit code of 0 proves the input
            // arrived.
            let (process, events) = start(&shell_command("set /p X=", ""));
            process.input(b"abc\r").unwrap();
            let (_, code) = run_to_exit(&process, &events);
            assert_eq!(code, 0);
        } else {
            let (process, events) = start(&shell_command("", "read x && echo got-$x"));
            process.input(b"abc\n").unwrap();
            let (output, code) = run_to_exit(&process, &events);
            assert!(output.contains("got-abc"), "output was {output:?}");
            assert_eq!(code, 0);
        }
    }

    #[test]
    fn resize_clamps_dimensions_to_at_least_one() {
        assert_eq!(clamp_size(0, 0), (1, 1));
        assert_eq!(clamp_size(120, 0), (120, 1));

        let (process, _events) = start(&long_running());
        process.resize(100, 30).unwrap();
        assert_eq!(process.pty_size(), Some((100, 30)));
        process.resize(0, 0).unwrap();
        assert_eq!(process.pty_size(), Some((1, 1)));
        process.kill();
    }

    #[test]
    fn resize_is_ignored_after_exit() {
        let (process, events) = start(&shell_command("exit 0", "exit 0"));
        run_to_exit(&process, &events);
        assert_eq!(process.exit_code(), Some(0));
        assert!(process.resize(50, 10).is_ok());
    }

    #[test]
    fn invalid_cwd_is_reported_before_spawning() {
        let missing = std::env::temp_dir().join("lathe-terminal-no-such-dir-xyz");
        let config = shell_command("echo hello", "echo hello");
        let result = TerminalProcess::spawn(&config, &base_env(), &missing, 80, 24, cfg!(windows));
        assert!(matches!(
            result,
            Err(LaunchError::CwdDoesNotExist(path)) if path == missing
        ));
    }

    #[test]
    fn missing_executable_is_reported_before_spawning() {
        let config = launch("lathe-no-such-shell-xyz", &[]);
        let cwd = std::env::temp_dir();
        let result = TerminalProcess::spawn(&config, &base_env(), &cwd, 80, 24, cfg!(windows));
        assert!(matches!(
            result,
            Err(LaunchError::ExecutableDoesNotExist(name)) if name == "lathe-no-such-shell-xyz"
        ));
    }

    #[test]
    fn kill_stops_a_running_process() {
        let (process, events) = start(&long_running());
        process.kill();
        // The exit code of a killed process is platform-specific; only the exit event matters.
        let (_, _code) = run_to_exit(&process, &events);
        assert!(process.exit_code().is_some());
        // Killing an exited process is a no-op.
        process.kill();
    }

    #[test]
    fn shutdown_immediate_ends_a_running_process() {
        let (process, events) = start(&long_running());
        process.shutdown(true);
        let _ = run_to_exit(&process, &events);
        assert!(process.exit_code().is_some());
    }
}
