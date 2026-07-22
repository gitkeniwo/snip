use std::io::{self, IsTerminal, Write};

use crate::error::{Result, SnipError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipboardMethod {
    System,
    Osc52,
}

pub fn copy(text: &str) -> Result<ClipboardMethod> {
    if let Ok(mut clipboard) = arboard::Clipboard::new()
        && clipboard.set_text(text.to_owned()).is_ok()
    {
        return Ok(ClipboardMethod::System);
    }
    if !io::stdout().is_terminal() {
        return Err(SnipError::io(
            "system clipboard unavailable and stdout is not a terminal",
        ));
    }
    let encoded = base64(text.as_bytes());
    write!(io::stdout(), "\x1b]52;c;{encoded}\x07")?;
    io::stdout().flush()?;
    Ok(ClipboardMethod::Osc52)
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
    #[test]
    fn base64_encodes_osc52_payloads() {
        assert_eq!(super::base64(b"hello"), "aGVsbG8=");
    }
}
