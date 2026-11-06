//! Byte and line budgets for reading text, shared by `snip show`, `snip cat`
//! and the brief search shapes so every cut lands in the same place.

use crate::error::{Result, SnipError};

/// The longest prefix of `text` that fits in `max` bytes, ending on a line
/// boundary. Only when a single line is longer than the whole budget is it cut,
/// and then at a UTF-8 character boundary.
pub fn cut(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    match text[..end].rfind('\n') {
        Some(newline) => &text[..=newline],
        None => &text[..end],
    }
}

/// An inclusive 1-based line range: `A:B`, `A:`, `:B`, or a single `A`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineRange {
    pub start: usize,
    pub end: Option<usize>,
}

impl LineRange {
    pub fn parse(value: &str) -> Result<Self> {
        let invalid = || {
            SnipError::usage(format!(
                "invalid --lines {value:?}: use A:B, A:, :B, or a single line number (1-based)"
            ))
        };
        let number = |part: &str| -> Result<usize> {
            let parsed = part.trim().parse::<usize>().map_err(|_| invalid())?;
            if parsed == 0 {
                Err(invalid())
            } else {
                Ok(parsed)
            }
        };
        let range = match value.split_once(':') {
            None => {
                let line = number(value)?;
                Self {
                    start: line,
                    end: Some(line),
                }
            }
            Some((start, end)) => Self {
                start: if start.trim().is_empty() {
                    1
                } else {
                    number(start)?
                },
                end: if end.trim().is_empty() {
                    None
                } else {
                    Some(number(end)?)
                },
            },
        };
        if range.end.is_some_and(|end| end < range.start) {
            return Err(invalid());
        }
        Ok(range)
    }
}

/// The selected lines of `text` (each keeping its newline) and the 1-based
/// number of the first selected line. A start past the end is a usage error;
/// an end past the end clamps.
pub fn select_lines(text: &str, range: LineRange) -> Result<(&str, usize)> {
    let total = text.lines().count();
    if range.start > total.max(1) {
        return Err(SnipError::usage(format!(
            "--lines starts at {} but the fragment has {total} lines",
            range.start
        )));
    }
    let mut offsets = text.match_indices('\n').map(|(index, _)| index + 1);
    let begin = if range.start == 1 {
        0
    } else {
        offsets.nth(range.start - 2).unwrap_or(text.len())
    };
    let end = match range.end {
        None => text.len(),
        Some(end) => {
            let mut offsets = text.match_indices('\n').map(|(index, _)| index + 1);
            offsets.nth(end - 1).unwrap_or(text.len())
        }
    };
    Ok((&text[begin..end.max(begin)], range.start))
}

/// `812` → `812 B`, `30000` → `29.3 KB`.
pub fn human_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_prefers_line_boundaries() {
        let text = "aaaa\nbbbb\ncccc\n";
        assert_eq!(cut(text, 100), text);
        assert_eq!(cut(text, 10), "aaaa\nbbbb\n");
        assert_eq!(cut(text, 9), "aaaa\n");
        assert_eq!(cut(text, 0), "");
    }

    #[test]
    fn cut_splits_a_long_line_on_a_char_boundary() {
        let text = "你好世界";
        assert_eq!(cut(text, 7), "你好");
        assert_eq!(cut(text, 2), "");
    }

    #[test]
    fn ranges_parse_and_select() {
        let text = "1\n2\n3\n4\n";
        let pick = |spec: &str| {
            select_lines(text, LineRange::parse(spec).unwrap())
                .unwrap()
                .0
        };
        assert_eq!(pick("2:3"), "2\n3\n");
        assert_eq!(pick("3:"), "3\n4\n");
        assert_eq!(pick(":2"), "1\n2\n");
        assert_eq!(pick("4"), "4\n");
        assert_eq!(pick("3:99"), "3\n4\n");
        assert!(select_lines(text, LineRange::parse("5").unwrap()).is_err());
        assert!(LineRange::parse("0:3").is_err());
        assert!(LineRange::parse("3:2").is_err());
        assert!(LineRange::parse("x").is_err());
    }
}
