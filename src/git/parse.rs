use time::OffsetDateTime;

use super::{Branch, Commit};

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct ParsedStatus {
    pub branch: Option<Branch>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: usize,
    pub unstaged: usize,
    pub untracked: usize,
    pub conflicted: Vec<String>,
    pub head_oid: Option<String>,
}

pub(super) fn parse_status_v2(bytes: &[u8]) -> ParsedStatus {
    let mut parsed = ParsedStatus::default();
    let mut tokens = bytes.split(|byte| *byte == 0).peekable();
    while let Some(token) = tokens.next() {
        if token.is_empty() {
            continue;
        }
        match token[0] {
            b'#' => parse_header(token, &mut parsed),
            b'1' => parse_ordinary(token, &mut parsed),
            b'2' => {
                parse_rename(token, &mut parsed);
                // In -z mode a rename record is followed by its original path.
                let _ = tokens.next();
            }
            b'u' => parse_unmerged(token, &mut parsed),
            b'?' => parsed.untracked += 1,
            b'!' => {}
            _ => {}
        }
    }
    parsed
}

fn parse_header(record: &[u8], parsed: &mut ParsedStatus) {
    let Ok(record) = std::str::from_utf8(record) else {
        return;
    };
    if let Some(value) = record.strip_prefix("# branch.oid ") {
        if value == "(initial)" {
            parsed.branch = Some(Branch::Unborn);
        } else {
            parsed.head_oid = Some(value.to_owned());
        }
    } else if let Some(value) = record.strip_prefix("# branch.head ") {
        if value == "(detached)" {
            let short_id = parsed
                .head_oid
                .as_deref()
                .unwrap_or("unknown")
                .chars()
                .take(7)
                .collect();
            parsed.branch = Some(Branch::Detached { short_id });
        } else if !matches!(parsed.branch, Some(Branch::Unborn)) {
            parsed.branch = Some(Branch::Named {
                name: value.to_owned(),
            });
        }
    } else if let Some(value) = record.strip_prefix("# branch.upstream ") {
        parsed.upstream = Some(value.to_owned());
    } else if let Some(value) = record.strip_prefix("# branch.ab ") {
        let mut fields = value.split_whitespace();
        parsed.ahead = fields
            .next()
            .and_then(|value| value.strip_prefix('+'))
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        parsed.behind = fields
            .next()
            .and_then(|value| value.strip_prefix('-'))
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
    }
}

fn parse_ordinary(record: &[u8], parsed: &mut ParsedStatus) {
    let mut fields = record.splitn(9, |byte| *byte == b' ');
    let _kind = fields.next();
    let Some(xy) = fields.next() else {
        return;
    };
    count_xy(xy, parsed);
}

fn parse_rename(record: &[u8], parsed: &mut ParsedStatus) {
    let mut fields = record.splitn(10, |byte| *byte == b' ');
    let _kind = fields.next();
    let Some(xy) = fields.next() else {
        return;
    };
    count_xy(xy, parsed);
}

fn count_xy(xy: &[u8], parsed: &mut ParsedStatus) {
    // A file such as `1 MM` is both staged and unstaged, so it counts twice.
    if xy.first().is_some_and(|status| *status != b'.') {
        parsed.staged += 1;
    }
    if xy.get(1).is_some_and(|status| *status != b'.') {
        parsed.unstaged += 1;
    }
}

fn parse_unmerged(record: &[u8], parsed: &mut ParsedStatus) {
    let mut fields = record.splitn(11, |byte| *byte == b' ');
    for _ in 0..10 {
        let _ = fields.next();
    }
    if let Some(path) = fields.next() {
        parsed
            .conflicted
            .push(String::from_utf8_lossy(path).into_owned());
    }
}

pub(super) fn parse_log(bytes: &[u8]) -> Option<Commit> {
    let fields = bytes
        .strip_suffix(b"\n")
        .unwrap_or(bytes)
        .splitn(3, |byte| *byte == 0)
        .collect::<Vec<_>>();
    if fields.len() != 3 {
        return None;
    }
    let short_id = std::str::from_utf8(fields[0]).ok()?.to_owned();
    let timestamp = std::str::from_utf8(fields[1]).ok()?.parse().ok()?;
    OffsetDateTime::from_unix_timestamp(timestamp).ok()?;
    let subject = String::from_utf8_lossy(fields[2]).into_owned();
    Some(Commit {
        short_id,
        timestamp,
        subject,
    })
}

pub(super) fn parse_logs(bytes: &[u8]) -> Vec<Commit> {
    bytes
        .split(|byte| *byte == 0x1e)
        .filter(|record| !record.is_empty())
        .filter_map(parse_log)
        .collect()
}

pub fn relative_time(timestamp: i64, now: i64) -> String {
    let seconds = now.saturating_sub(timestamp).max(0);
    match seconds {
        0..=59 => "just now".to_owned(),
        60..=3_599 => plural(seconds / 60, "minute"),
        3_600..=86_399 => plural(seconds / 3_600, "hour"),
        86_400..=604_799 => plural(seconds / 86_400, "day"),
        604_800..=2_591_999 => plural(seconds / 604_800, "week"),
        2_592_000..=31_535_999 => plural(seconds / 2_592_000, "month"),
        _ => plural(seconds / 31_536_000, "year"),
    }
}

fn plural(value: i64, unit: &str) -> String {
    format!("{value} {unit}{} ago", if value == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::{parse_log, parse_logs, parse_status_v2, relative_time};
    use crate::git::Branch;

    #[test]
    fn parses_named_detached_and_unborn_branches() {
        let named = parse_status_v2(
            b"# branch.oid abcdef123456\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +3 -2\0",
        );
        assert_eq!(
            named.branch,
            Some(Branch::Named {
                name: "main".to_owned()
            })
        );
        assert_eq!(named.upstream.as_deref(), Some("origin/main"));
        assert_eq!((named.ahead, named.behind), (3, 2));

        let detached = parse_status_v2(b"# branch.oid abcdef123456\0# branch.head (detached)\0");
        assert_eq!(
            detached.branch,
            Some(Branch::Detached {
                short_id: "abcdef1".to_owned()
            })
        );

        let unborn = parse_status_v2(b"# branch.oid (initial)\0# branch.head main\0");
        assert_eq!(unborn.branch, Some(Branch::Unborn));
        assert_eq!(unborn.head_oid, None);
    }

    #[test]
    fn parses_every_record_type_and_double_counts_mm() {
        let bytes = concat!(
            "1 MM N... 100644 100644 100644 a b Scripts/space name.rs\0",
            "2 R. N... 100644 100644 100644 a b R100 新名字.rs\0",
            "旧名字.rs\0",
            "u UU N... 100644 100644 100644 100644 a b c conflict\nname.toml\0",
            "? untracked \"文件\".rs\0",
            "! ignored.rs\0"
        );
        let parsed = parse_status_v2(bytes.as_bytes());
        assert_eq!(parsed.staged, 2);
        assert_eq!(parsed.unstaged, 1);
        assert_eq!(parsed.untracked, 1);
        assert_eq!(parsed.conflicted, vec!["conflict\nname.toml"]);
    }

    #[test]
    fn empty_status_is_clean() {
        let parsed = parse_status_v2(b"");
        assert_eq!(parsed.staged + parsed.unstaged + parsed.untracked, 0);
        assert!(parsed.conflicted.is_empty());
    }

    #[test]
    fn parses_log_subject_and_rejects_invalid_timestamp() {
        let commit = parse_log(b"a1b2c3d\x001690704000\0Update two snippets\n").unwrap();
        assert_eq!(commit.short_id, "a1b2c3d");
        assert_eq!(commit.subject, "Update two snippets");
        assert!(parse_log(b"a\0not-a-time\0subject").is_none());
        assert!(parse_log(b"a\x009223372036854775807\0subject").is_none());
    }

    #[test]
    fn parses_multiple_record_separated_commits() {
        let commits =
            parse_logs(b"\x1ea1b2c3d\x001690704000\0Local\n\x1ed4e5f6a\x001690704100\0Remote\n");
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].subject, "Local");
        assert_eq!(commits[1].subject, "Remote");
    }

    #[test]
    fn formats_relative_time_boundaries() {
        assert_eq!(relative_time(1_000, 1_030), "just now");
        assert_eq!(relative_time(1_000, 1_060), "1 minute ago");
        assert_eq!(relative_time(1_000, 1_120), "2 minutes ago");
        assert_eq!(relative_time(1_000, 4_600), "1 hour ago");
        assert_eq!(relative_time(1_000, 87_400), "1 day ago");
        assert_eq!(relative_time(1_000, 1_210_600), "2 weeks ago");
        assert_eq!(relative_time(1_000, 34_561_000), "1 year ago");
        assert_eq!(relative_time(2_000, 1_000), "just now");
    }
}
