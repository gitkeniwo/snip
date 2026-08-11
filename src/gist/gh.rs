use std::collections::BTreeMap;
use std::io::{ErrorKind as IoErrorKind, Write};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{Result, SnipError};

/// Why the `gh` CLI could not do its job. Three causes: the `gh` binary is
/// missing, the user is not authenticated, or the request failed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Unavailable {
    BinaryMissing,
    NotAuthenticated,
    Failed { message: String },
}

#[derive(Clone, Debug, Deserialize)]
pub struct Gist {
    pub id: String,
    pub html_url: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub files: BTreeMap<String, GistFile>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GistFile {
    #[serde(default)]
    pub filename: Option<String>,
}

/// Derive the record's `host` from a gist URL: everything between `https://`
/// and the next `/`, with a leading `gist.` stripped. Falls back to
/// `github.com` when the URL is malformed.
pub fn derive_host(html_url: &str) -> String {
    html_url
        .strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .map(|host| host.strip_prefix("gist.").unwrap_or(host))
        .unwrap_or("github.com")
        .to_owned()
}

pub fn create(description: &str, public: bool, files: &BTreeMap<String, String>) -> Result<Gist> {
    let mut file_map = serde_json::Map::new();
    for (name, content) in files {
        file_map.insert(name.clone(), json!({"content": content}));
    }
    let body = json!({
        "description": description,
        "public": public,
        "files": file_map,
    });
    let value = run(
        &["api", "--method", "POST", "/gists", "--input", "-"],
        Some(&body.to_string()),
    )
    .map_err(classify)?;
    parse_gist(value)
}

pub fn update(
    id: &str,
    description: &str,
    files: &BTreeMap<String, Option<String>>,
) -> Result<Gist> {
    let mut body = serde_json::Map::new();
    body.insert("description".to_owned(), json!(description));
    let mut file_map = serde_json::Map::new();
    for (name, content) in files {
        match content {
            Some(content) => {
                file_map.insert(name.clone(), json!({"content": content}));
            }
            None => {
                file_map.insert(name.clone(), Value::Null);
            }
        }
    }
    body.insert("files".to_owned(), Value::Object(file_map));
    let value = run(
        &[
            "api",
            "--method",
            "PATCH",
            &format!("/gists/{id}"),
            "--input",
            "-",
        ],
        Some(&Value::Object(body).to_string()),
    )
    .map_err(|unavailable| classify_with_id(unavailable, Some(id)))?;
    parse_gist(value)
}

pub fn fetch(id: &str) -> Result<Gist> {
    let value = run(&["api", &format!("/gists/{id}")], None).map_err(classify)?;
    parse_gist(value)
}

pub fn delete(id: &str) -> Result<()> {
    run_empty(&["api", "--method", "DELETE", &format!("/gists/{id}")])
        .map_err(|unavailable| classify_with_id(unavailable, Some(id)))
}

/// Open the gist in the default browser via `gh gist view --web`. This is the
/// one sanctioned use of a `gh gist` subcommand: it only launches a browser and
/// performs no data operations, so the ban on `gh gist` subcommands does not
/// apply. It is also the only cross-platform way to open a browser without
/// hand-rolling
/// `open`/`xdg-open`/`start`.
pub fn open_web(id: &str) -> Result<()> {
    run_empty(&["gist", "view", id, "--web"]).map_err(classify)
}

fn run(args: &[&str], body: Option<&str>) -> std::result::Result<Value, Unavailable> {
    let mut command = gh_command(args);
    command.stdin(if body.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = command.spawn().map_err(spawn_error)?;
    if let Some(body) = body {
        let mut stdin = child.stdin.take().ok_or_else(|| Unavailable::Failed {
            message: "gh stdin is unavailable".to_owned(),
        })?;
        stdin
            .write_all(body.as_bytes())
            .map_err(|error| Unavailable::Failed {
                message: format!("cannot write gh request body: {error}"),
            })?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| Unavailable::Failed {
            message: format!("cannot wait for gh: {error}"),
        })?;
    if !output.status.success() {
        return Err(classify_stderr(
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| Unavailable::Failed {
        message: format!("gh returned invalid JSON: {error}"),
    })
}

/// Sibling of [`run`] for requests whose response carries no body, such as
/// DELETE or `gist view --web`. Discards stdout and returns success on a zero
/// exit.
fn run_empty(args: &[&str]) -> std::result::Result<(), Unavailable> {
    let output = gh_command(args)
        .stdin(Stdio::null())
        .output()
        .map_err(spawn_error)?;
    if !output.status.success() {
        return Err(classify_stderr(
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }
    Ok(())
}

fn gh_command(args: &[&str]) -> Command {
    let binary = std::env::var("SNIP_GH_BIN")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "gh".to_owned());
    let mut command = Command::new(binary);
    command
        .args(args)
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_PAGER", "")
        .env("NO_COLOR", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn spawn_error(error: std::io::Error) -> Unavailable {
    if error.kind() == IoErrorKind::NotFound {
        Unavailable::BinaryMissing
    } else {
        Unavailable::Failed {
            message: error.to_string(),
        }
    }
}

fn classify_stderr(stderr: &str) -> Unavailable {
    if stderr.contains("auth login") || stderr.contains("not logged") {
        Unavailable::NotAuthenticated
    } else {
        Unavailable::Failed {
            message: stderr.to_owned(),
        }
    }
}

fn classify(unavailable: Unavailable) -> SnipError {
    classify_with_id(unavailable, None)
}

fn classify_with_id(unavailable: Unavailable, id: Option<&str>) -> SnipError {
    match unavailable {
        Unavailable::BinaryMissing => SnipError::io("gh not found in PATH")
            .with_hint("install the GitHub CLI from https://cli.github.com"),
        Unavailable::NotAuthenticated => {
            SnipError::io("gh is not authenticated").with_hint("run: gh auth login")
        }
        Unavailable::Failed { message } => {
            if let Some(code) = http_code(&message) {
                if let Some(id) = id
                    && code == 404
                {
                    return SnipError::not_found(format!("gist {id} no longer exists"))
                        .with_hint(
                            "run snip gist push <selector> --new to publish a new one, or snip gist detach <selector> to forget it",
                        );
                }
                if matches!(code, 401 | 403 | 404) {
                    return SnipError::not_found(format!(
                        "GitHub rejected the request (HTTP {code})"
                    ))
                    .with_hint(
                        "the gh token may be missing the 'gist' scope; run: gh auth refresh -h github.com -s gist",
                    );
                }
            }
            SnipError::io(format!("gh failed: {message}"))
        }
    }
}

/// `gh api` writes `gh: … (HTTP 404)` on failure; extract the status. The
/// marker is `(HTTP ` so that a gist description or message containing "HTTP"
/// elsewhere cannot be mistaken for a status.
fn http_code(stderr: &str) -> Option<u32> {
    let marker = "(HTTP ";
    stderr.find(marker).and_then(|index| {
        stderr[index + marker.len()..]
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>()
            .parse::<u32>()
            .ok()
    })
}

fn parse_gist(value: Value) -> Result<Gist> {
    serde_json::from_value(value).map_err(|error| {
        SnipError::validation(format!("cannot parse gh response as a gist: {error}"))
    })
}

#[cfg(test)]
mod tests {
    use super::{Unavailable, classify_stderr, derive_host, http_code};

    #[test]
    fn host_derivation_strips_gist_and_falls_back() {
        assert_eq!(
            derive_host("https://gist.github.com/octocat/5b0e0062eb8e9654adad7bb1d81cc75f"),
            "github.com"
        );
        assert_eq!(
            derive_host("https://github.example.com/octocat/5b0e0062eb8e9654adad7bb1d81cc75f"),
            "github.example.com"
        );
        assert_eq!(derive_host("not a url"), "github.com");
    }

    #[test]
    fn http_code_extracts_the_status_from_gh_stderr() {
        assert_eq!(http_code("gh: Not Found (HTTP 404)"), Some(404));
        assert_eq!(http_code("gh: Forbidden (HTTP 403)"), Some(403));
        assert_eq!(http_code("gh: some other failure"), None);
        assert_eq!(http_code("a gist about HTTP 404 in the description"), None);
        assert_eq!(http_code("gh: failed with HTTP 500"), None);
    }

    #[test]
    fn classify_stderr_detects_missing_login() {
        assert_eq!(
            classify_stderr("gh: To get started with GitHub CLI, please run:  gh auth login"),
            Unavailable::NotAuthenticated
        );
        assert_eq!(
            classify_stderr("gh: not logged into any GitHub hosts"),
            Unavailable::NotAuthenticated
        );
        assert_eq!(
            classify_stderr("gh: Not Found (HTTP 404)"),
            Unavailable::Failed {
                message: "gh: Not Found (HTTP 404)".to_owned(),
            }
        );
    }

    #[test]
    fn unavailable_serializes_with_a_kind_tag() {
        let value = serde_json::to_value(Unavailable::BinaryMissing).unwrap();
        assert_eq!(value["kind"], "binary_missing");
        let value = serde_json::to_value(Unavailable::NotAuthenticated).unwrap();
        assert_eq!(value["kind"], "not_authenticated");
    }
}
