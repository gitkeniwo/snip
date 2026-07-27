use std::ffi::OsString;
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
    let mut command = non_interactive_command(cwd, args, std::env::var_os("GIT_SSH_COMMAND"));
    let output = command.output().map_err(spawn_error)?;
    Ok(Output {
        status: output.status,
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

fn non_interactive_command(
    cwd: &Path,
    args: &[&str],
    existing_ssh_command: Option<OsString>,
) -> Command {
    let mut command = Command::new("git");
    command
        .args(["--no-pager", "-C"])
        .arg(cwd)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .env("SSH_ASKPASS_REQUIRE", "never")
        .env("GIT_HTTP_LOW_SPEED_LIMIT", "1000")
        .env("GIT_HTTP_LOW_SPEED_TIME", "30")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.env(
        "GIT_SSH_COMMAND",
        existing_ssh_command
            .unwrap_or_else(|| OsString::from("ssh -o BatchMode=yes -o ConnectTimeout=10")),
    );
    command
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn command_env<'a>(command: &'a Command, key: &str) -> Option<&'a OsStr> {
        command
            .get_envs()
            .find_map(|(name, value)| (name == key).then_some(value).flatten())
    }

    #[test]
    fn non_interactive_git_hardens_networks_and_preserves_custom_ssh() {
        let command = non_interactive_command(Path::new("/tmp"), &["push"], None);
        assert_eq!(
            command_env(&command, "GIT_SSH_COMMAND"),
            Some(OsStr::new("ssh -o BatchMode=yes -o ConnectTimeout=10"))
        );
        assert_eq!(
            command_env(&command, "SSH_ASKPASS_REQUIRE"),
            Some(OsStr::new("never"))
        );
        assert_eq!(
            command_env(&command, "GIT_HTTP_LOW_SPEED_LIMIT"),
            Some(OsStr::new("1000"))
        );
        assert_eq!(
            command_env(&command, "GIT_HTTP_LOW_SPEED_TIME"),
            Some(OsStr::new("30"))
        );

        let custom = non_interactive_command(
            Path::new("/tmp"),
            &["push"],
            Some(OsString::from("ssh -F custom.conf")),
        );
        assert_eq!(
            command_env(&custom, "GIT_SSH_COMMAND"),
            Some(OsStr::new("ssh -F custom.conf"))
        );
    }
}
