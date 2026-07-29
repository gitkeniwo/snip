#[allow(dead_code)]
#[path = "../src/cli.rs"]
mod cli;

use clap::CommandFactory;
use cli::Cli;
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

enum Mode {
    Generate,
    Check,
    Preview(String),
}

#[derive(Default)]
struct Changes {
    missing: Vec<String>,
    changed: Vec<String>,
    extra: Vec<String>,
}

impl Changes {
    fn is_empty(&self) -> bool {
        self.missing.is_empty() && self.changed.is_empty() && self.extra.is_empty()
    }

    fn print(&self) {
        for name in &self.missing {
            eprintln!("missing: {name}");
        }
        for name in &self.changed {
            eprintln!("changed: {name}");
        }
        for name in &self.extra {
            eprintln!("extra: {name}");
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("generate-man: error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let mode = parse_mode()?;
    if !cfg!(feature = "tui") {
        return Err(io::Error::other(
            "man pages describe the full command tree; rerun with `--all-features`",
        )
        .into());
    }

    let generated = tempfile::tempdir()?;
    clap_mangen::generate_to(Cli::command(), generated.path())?;
    let expected = read_pages(generated.path())?;
    if expected.is_empty() {
        return Err(io::Error::other(
            "clap_mangen produced no section 1 pages; refusing to continue",
        )
        .into());
    }

    match mode {
        Mode::Generate => sync_pages(&expected),
        Mode::Check => check_pages(&expected),
        Mode::Preview(page) => preview_page(generated.path(), &expected, &page),
    }
}

fn parse_mode() -> Result<Mode> {
    let mut args = env::args().skip(1);
    let mode = match args.next().as_deref() {
        None => Mode::Generate,
        Some("--check") => Mode::Check,
        Some("--preview") => Mode::Preview(args.next().unwrap_or_else(|| "snip".to_owned())),
        Some(argument) => {
            return Err(io::Error::other(format!(
                "unknown argument `{argument}`; expected `--check` or `--preview [PAGE]`"
            ))
            .into());
        }
    };

    if let Some(argument) = args.next() {
        return Err(io::Error::other(format!("unexpected argument `{argument}`")).into());
    }
    Ok(mode)
}

fn man_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("man")
}

fn read_pages(directory: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut pages = BTreeMap::new();
    if !directory.exists() {
        return Ok(pages);
    }

    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension() != Some(OsStr::new("1")) {
            continue;
        }
        if !entry.file_type()?.is_file() {
            return Err(io::Error::other(format!(
                "managed man page is not a regular file: {}",
                path.display()
            ))
            .into());
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| io::Error::other("man page filename is not valid UTF-8"))?;
        pages.insert(name, fs::read(path)?);
    }
    Ok(pages)
}

fn changes(expected: &BTreeMap<String, Vec<u8>>, actual: &BTreeMap<String, Vec<u8>>) -> Changes {
    let mut changes = Changes::default();
    for (name, contents) in expected {
        match actual.get(name) {
            None => changes.missing.push(name.clone()),
            Some(actual_contents) if actual_contents != contents => {
                changes.changed.push(name.clone());
            }
            Some(_) => {}
        }
    }
    for name in actual.keys() {
        if !expected.contains_key(name) {
            changes.extra.push(name.clone());
        }
    }
    changes
}

fn sync_pages(expected: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    let directory = man_dir();
    fs::create_dir_all(&directory)?;
    let actual = read_pages(&directory)?;
    let changes = changes(expected, &actual);

    for name in changes.missing.iter().chain(&changes.changed) {
        fs::write(directory.join(name), &expected[name])?;
    }
    for name in &changes.extra {
        fs::remove_file(directory.join(name))?;
    }

    if changes.is_empty() {
        println!("man pages are up to date ({} pages)", expected.len());
    } else {
        println!(
            "updated man pages: {} written, {} removed",
            changes.missing.len() + changes.changed.len(),
            changes.extra.len()
        );
    }
    Ok(())
}

fn check_pages(expected: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    let actual = read_pages(&man_dir())?;
    let changes = changes(expected, &actual);
    if changes.is_empty() {
        println!("man pages are up to date ({} pages)", expected.len());
        return Ok(());
    }

    changes.print();
    Err(
        io::Error::other("generated man pages are stale; run the generator without `--check`")
            .into(),
    )
}

fn preview_page(generated_dir: &Path, pages: &BTreeMap<String, Vec<u8>>, page: &str) -> Result<()> {
    if page.is_empty()
        || !page
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(io::Error::other(format!("invalid page name `{page}`")).into());
    }

    let filename = format!("{page}.1");
    if !pages.contains_key(&filename) {
        return Err(io::Error::other(format!(
            "unknown page `{page}`; choose a generated page stem such as `snip` or `snip-create`"
        ))
        .into());
    }

    let path = generated_dir.join(filename).canonicalize()?;
    println!("previewing {}", path.display());
    let status = Command::new("man").arg(&path).status().map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            io::Error::new(
                error.kind(),
                format!(
                    "could not launch `man`; the generated page is {}",
                    path.display()
                ),
            )
        } else {
            error
        }
    })?;
    if !status.success() {
        return Err(io::Error::other(format!("`man` exited with status {status}")).into());
    }
    Ok(())
}
