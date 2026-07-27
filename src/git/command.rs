use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

pub(super) struct Output {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

#[derive(Debug)]
pub(super) enum SpawnError {
    NotInstalled,
    Io(String),
}

pub(super) fn run(cwd: &Path, args: &[&str]) -> Result<Output, SpawnError> {
    let output = Command::new("git")
        // Status polling must not contend with Git operations in another terminal.
        .args(["--no-optional-locks", "--no-pager", "-C"])
        .arg(cwd)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => SpawnError::NotInstalled,
            _ => SpawnError::Io(error.to_string()),
        })?;
    Ok(Output {
        status: output.status,
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

pub(super) fn run_non_interactive(cwd: &Path, args: &[&str]) -> Result<Output, SpawnError> {
    let output = Command::new("git")
        .args(["--no-pager", "-C"])
        .arg(cwd)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(spawn_error)?;
    Ok(Output {
        status: output.status,
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

pub(super) fn run_interactive(cwd: &Path, args: &[&str]) -> Result<ExitStatus, SpawnError> {
    Command::new("git")
        .args(["--no-pager", "-C"])
        .arg(cwd)
        .args(args)
        .env_remove("GIT_TERMINAL_PROMPT")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(spawn_error)
}

fn spawn_error(error: io::Error) -> SpawnError {
    match error.kind() {
        io::ErrorKind::NotFound => SpawnError::NotInstalled,
        _ => SpawnError::Io(error.to_string()),
    }
}
