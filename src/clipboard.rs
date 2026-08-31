use std::env;
use std::io::{self, IsTerminal, Write};

use crate::error::{Result, SnipError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipboardMethod {
    System,
    Osc52,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClipboardMode {
    Auto,
    ForceSystem,
    ForceOsc52,
}

impl ClipboardMode {
    fn from_env() -> Self {
        Self::parse(env::var("SNIP_CLIPBOARD").ok().as_deref())
    }

    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("system") => Self::ForceSystem,
            Some("osc52") => Self::ForceOsc52,
            _ => Self::Auto,
        }
    }
}

fn in_ssh_session() -> bool {
    ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
        .iter()
        .any(|k| env::var_os(k).is_some())
}

fn copy_via_osc52(text: &str) -> Result<ClipboardMethod> {
    if !io::stdout().is_terminal() {
        return Err(SnipError::io(
            "OSC 52 unavailable and stdout is not a terminal",
        ));
    }
    let encoded = base64(text.as_bytes());
    write!(io::stdout(), "\x1b]52;c;{encoded}\x07")?;
    io::stdout().flush()?;
    Ok(ClipboardMethod::Osc52)
}

fn copy_via_system(text: &str, keep_alive: bool) -> Result<ClipboardMethod> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| SnipError::io(format!("system clipboard unavailable: {e}")))?;
    #[cfg(target_os = "linux")]
    if keep_alive {
        use arboard::SetExtLinux;
        eprintln!(
            "holding the clipboard open so it survives this process; it releases when another application copies something (Ctrl-C aborts)"
        );
        clipboard
            .set()
            .wait()
            .text(text.to_owned())
            .map_err(|e| SnipError::io(format!("failed to write system clipboard: {e}")))?;
        return Ok(ClipboardMethod::System);
    }
    let _ = keep_alive;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| SnipError::io(format!("failed to write system clipboard: {e}")))?;
    Ok(ClipboardMethod::System)
}

/// Copy `text`, returning the method actually used.
///
/// Mode resolution (`SNIP_CLIPBOARD=system|osc52|auto`, default `auto`):
/// - `auto`: prefer the local system clipboard, except over SSH where OSC 52
///   goes first so the text lands on the client machine instead of the host.
/// - `system`: only use the system clipboard (fails if unavailable).
/// - `osc52`: only use OSC 52 (fails if stdout is not a terminal).
pub fn copy(text: &str) -> Result<ClipboardMethod> {
    copy_inner(text, false)
}

/// Like [`copy`], for one-shot CLI processes that exit right after copying.
///
/// On X11 the clipboard dies with its owner, so without this the process
/// would exit before anyone could paste. Blocks on Linux/X11 until another
/// application takes over the clipboard.
pub fn copy_oneshot(text: &str) -> Result<ClipboardMethod> {
    copy_inner(text, true)
}

fn copy_inner(text: &str, keep_alive: bool) -> Result<ClipboardMethod> {
    match ClipboardMode::from_env() {
        ClipboardMode::ForceOsc52 => copy_via_osc52(text),
        ClipboardMode::ForceSystem => copy_via_system(text, keep_alive),
        ClipboardMode::Auto => {
            if in_ssh_session() {
                match copy_via_osc52(text) {
                    Ok(method) => Ok(method),
                    Err(_) => copy_via_system(text, keep_alive),
                }
            } else {
                match copy_via_system(text, keep_alive) {
                    Ok(method) => Ok(method),
                    Err(_) => copy_via_osc52(text),
                }
            }
        }
    }
}

fn base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let value = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        output.push(TABLE[((value >> 18) & 63) as usize] as char);
        output.push(TABLE[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{ClipboardMode, base64, in_ssh_session};

    #[test]
    fn base64_encodes_osc52_payloads() {
        assert_eq!(base64(b"hello"), "aGVsbG8=");
    }

    #[test]
    fn clipboard_mode_defaults_to_auto_on_unknown_value() {
        assert_eq!(ClipboardMode::parse(Some("bogus")), ClipboardMode::Auto);
        assert_eq!(ClipboardMode::parse(None), ClipboardMode::Auto);
        assert_eq!(ClipboardMode::parse(Some("")), ClipboardMode::Auto);
    }

    #[test]
    fn clipboard_mode_parses_explicit_values() {
        assert_eq!(
            ClipboardMode::parse(Some("system")),
            ClipboardMode::ForceSystem
        );
        assert_eq!(
            ClipboardMode::parse(Some("osc52")),
            ClipboardMode::ForceOsc52
        );
    }

    #[test]
    fn ssh_detection_checks_all_env_vars() {
        if std::env::var_os("SSH_CONNECTION").is_some()
            || std::env::var_os("SSH_CLIENT").is_some()
            || std::env::var_os("SSH_TTY").is_some()
        {
            return;
        }
        assert!(!in_ssh_session());
    }
}
