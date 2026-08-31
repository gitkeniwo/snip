#[cfg(feature = "tui")]
#[path = "common/keydoc.rs"]
mod keydoc;

#[allow(dead_code)]
#[path = "../src/cli.rs"]
mod cli;

use clap::CommandFactory;
use cli::Cli;
use snip::config::{CONFIG_FIELDS, ConfigFieldSpec, FieldKind};
#[cfg(feature = "tui")]
use snip::keys::Keymap;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
type CommandPath = &'static [&'static str];

const SOURCE: &str = concat!("snip ", env!("CARGO_PKG_VERSION"));
const MANUAL: &str = "Snip Manual";
const GLOBAL_ARGS: &[&str] = &["library", "output", "color", "simplified_ui"];
const MANAGED_SECTIONS: &[&str] = &["1", "5", "7"];
const FORMAT_DIGEST: &str =
    "blake3:570984a131cfa5f49c587efe207ceb73f74d86a1a686a1226235ae50abab591f";
const THEMES_DIGEST: &str =
    "blake3:e89bb1c502bc22f30b0c6657816e4aa3f45236d4fd1a422e33ee7c9053a348b5";
const SNIPLIB_SOURCES: &[DerivedSource] =
    &[derived_source("FORMAT.md", FORMAT_DIGEST, "FORMAT_DIGEST")];
const THEME_SOURCES: &[DerivedSource] = &[derived_source(
    "docs/themes.md",
    THEMES_DIGEST,
    "THEMES_DIGEST",
)];

struct PageSpec {
    name: &'static str,
    section: u8,
    title: &'static str,
    commands: &'static [CommandPath],
    parts: Option<&'static str>,
    derived_from: &'static [DerivedSource],
}

struct DerivedSource {
    path: &'static str,
    digest: &'static str,
    digest_constant: &'static str,
}

struct AliasSpec {
    name: &'static str,
    title: &'static str,
    target: &'static str,
}

const PAGES: &[PageSpec] = &[
    page(
        "snip",
        "filesystem-native snippet library",
        &[&[]],
        "parts/snip.md",
    ),
    page(
        "snip-tui",
        "open the interactive terminal browser",
        &[&["tui"]],
        "parts/snip-tui.md",
    ),
    page(
        "snip-query",
        "query and inspect snippets without modifying them",
        &[
            &["list"],
            &["search"],
            &["show"],
            &["cat"],
            &["preview"],
            &["path"],
            &["open"],
            &["info"],
        ],
        "parts/snip-query.md",
    ),
    page(
        "snip-create",
        "create a snippet",
        &[&["create"]],
        "parts/snip-create.md",
    ),
    page(
        "snip-edit",
        "modify and organize snippets",
        &[
            &["edit"],
            &["fragment"],
            &["fragment", "add"],
            &["fragment", "edit"],
            &["fragment", "remove"],
            &["fragment", "reorder"],
            &["folder"],
            &["folder", "list"],
            &["folder", "create"],
            &["folder", "rename"],
            &["folder", "move"],
            &["folder", "delete"],
            &["tag"],
            &["tag", "list"],
            &["tag", "rename"],
            &["tag", "delete"],
            &["organize"],
        ],
        "parts/snip-edit.md",
    ),
    page(
        "snip-config",
        "inspect and modify snip configuration",
        &[
            &["config"],
            &["config", "path"],
            &["config", "show"],
            &["config", "init"],
            &["config", "set"],
            &["config", "unset"],
        ],
        "parts/snip-config.md",
    ),
    page(
        "snip-theme",
        "inspect and manage TUI color themes",
        &[
            &["theme"],
            &["theme", "list"],
            &["theme", "show"],
            &["theme", "check"],
            &["theme", "path"],
            &["theme", "export"],
            &["theme", "import"],
            &["theme", "use"],
        ],
        "parts/snip-theme.md",
    ),
    page(
        "snip-keys",
        "inspect and manage TUI key bindings",
        &[
            &["keys"],
            &["keys", "list"],
            &["keys", "show"],
            &["keys", "path"],
            &["keys", "export"],
            &["keys", "check"],
        ],
        "parts/snip-keys.md",
    ),
    page(
        "snip-trash",
        "manage the soft-deletion lifecycle",
        &[&["delete"], &["trash"], &["restore"], &["purge"]],
        "parts/snip-trash.md",
    ),
    page(
        "snip-init",
        "create and import snippet libraries",
        &[&["init"], &["import"], &["import", "snippetslab"]],
        "parts/snip-init.md",
    ),
    page(
        "snip-doctor",
        "validate and repair a library",
        &[&["doctor"]],
        "parts/snip-doctor.md",
    ),
    page(
        "snip-git",
        "run Git operations scoped to a library",
        &[
            &["git"],
            &["git", "clone"],
            &["git", "status"],
            &["git", "init"],
            &["git", "commit"],
            &["git", "backup"],
            &["git", "push"],
            &["git", "fetch"],
            &["git", "pull"],
        ],
        "parts/snip-git.md",
    ),
    page(
        "snip-gist",
        "publish snippets through GitHub Gists",
        &[
            &["gist"],
            &["gist", "push"],
            &["gist", "url"],
            &["gist", "status"],
            &["gist", "attach"],
            &["gist", "detach"],
            &["gist", "delete"],
            &["gist", "open"],
        ],
        "parts/snip-gist.md",
    ),
    page(
        "snip-man",
        "install, inspect, and export manual pages",
        &[
            &["man"],
            &["man", "path"],
            &["man", "install"],
            &["man", "uninstall"],
            &["man", "show"],
            &["man", "generate"],
        ],
        "parts/snip-man.md",
    ),
    page(
        "snip-completion",
        "generate shell completion code",
        &[&["completion"]],
        "parts/snip-completion.md",
    ),
    derived_prose_page(
        "sniplib",
        5,
        "snip library file format",
        "parts/sniplib.md",
        SNIPLIB_SOURCES,
    ),
    prose_page(
        "snip-config",
        5,
        "snip user configuration format",
        "parts/snip-config.5.md",
    ),
    prose_page(
        "snip-keys",
        5,
        "snip TUI key binding format",
        "parts/snip-keys.5.md",
    ),
    derived_prose_page(
        "snip-theme",
        5,
        "snip TUI theme format",
        "parts/snip-theme.5.md",
        THEME_SOURCES,
    ),
    prose_page(
        "snip-agents",
        7,
        "using snip from scripts and agents",
        "parts/snip-agents.md",
    ),
];

const ALIASES: &[AliasSpec] = &[
    alias(
        "snip-list",
        "list snippets without their full content",
        "snip-query",
    ),
    alias(
        "snip-search",
        "search snippet metadata and content",
        "snip-query",
    ),
    alias("snip-show", "show a complete snippet", "snip-query"),
    alias(
        "snip-cat",
        "print one fragment without decorations",
        "snip-query",
    ),
    alias("snip-preview", "render a snippet for preview", "snip-query"),
    alias("snip-path", "print a managed filesystem path", "snip-query"),
    alias(
        "snip-open",
        "open a managed path in an external application",
        "snip-query",
    ),
    alias(
        "snip-info",
        "show library metadata and counts",
        "snip-query",
    ),
    alias("snip-fragment", "manage snippet fragments", "snip-edit"),
    alias("snip-folder", "manage physical folder paths", "snip-edit"),
    alias("snip-tag", "manage tags across snippets", "snip-edit"),
    alias(
        "snip-organize",
        "normalize snippet package directory names",
        "snip-edit",
    ),
    alias("snip-delete", "move a snippet to the trash", "snip-trash"),
    alias("snip-restore", "restore a deleted snippet", "snip-trash"),
    alias(
        "snip-purge",
        "permanently remove a trash entry",
        "snip-trash",
    ),
    alias("snip-import", "import another snippet format", "snip-init"),
];

const fn page(
    name: &'static str,
    title: &'static str,
    commands: &'static [CommandPath],
    parts: &'static str,
) -> PageSpec {
    PageSpec {
        name,
        section: 1,
        title,
        commands,
        parts: Some(parts),
        derived_from: &[],
    }
}

const fn prose_page(
    name: &'static str,
    section: u8,
    title: &'static str,
    parts: &'static str,
) -> PageSpec {
    PageSpec {
        name,
        section,
        title,
        commands: &[],
        parts: Some(parts),
        derived_from: &[],
    }
}

const fn derived_prose_page(
    name: &'static str,
    section: u8,
    title: &'static str,
    parts: &'static str,
    derived_from: &'static [DerivedSource],
) -> PageSpec {
    PageSpec {
        name,
        section,
        title,
        commands: &[],
        parts: Some(parts),
        derived_from,
    }
}

const fn derived_source(
    path: &'static str,
    digest: &'static str,
    digest_constant: &'static str,
) -> DerivedSource {
    DerivedSource {
        path,
        digest,
        digest_constant,
    }
}

const fn alias(name: &'static str, title: &'static str, target: &'static str) -> AliasSpec {
    AliasSpec {
        name,
        title,
        target,
    }
}

enum Mode {
    Generate,
    Check,
    Preview(String),
    AcceptSources,
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

#[derive(Default)]
struct Parts {
    sections: BTreeMap<String, Vec<Block>>,
    order: Vec<String>,
}

enum Block {
    Paragraph(String),
    Literal(Vec<String>),
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

    let mut root = Cli::command().disable_help_subcommand(true);
    root.build();
    validate_manifest_structure(&root)?;
    if matches!(mode, Mode::AcceptSources) {
        return accept_derived_sources();
    }

    let expected = render_pages(&root)?;
    if expected.is_empty() {
        return Err(io::Error::other("page manifest produced no manual pages").into());
    }

    match mode {
        Mode::Generate => sync_artifacts(&expected),
        Mode::Check => {
            validate_derived_sources()?;
            check_artifacts(&expected)
        }
        Mode::Preview(page) => preview_page(&expected, &page),
        Mode::AcceptSources => unreachable!("handled before rendering"),
    }
}

fn parse_mode() -> Result<Mode> {
    let mut args = env::args().skip(1);
    let mode = match args.next().as_deref() {
        None => Mode::Generate,
        Some("--check") => Mode::Check,
        Some("--preview") => Mode::Preview(args.next().unwrap_or_else(|| "snip".to_owned())),
        Some("--accept-sources") => Mode::AcceptSources,
        Some(argument) => {
            return Err(io::Error::other(format!(
                "unknown argument `{argument}`; expected `--check`, `--preview [PAGE]`, or \
                 `--accept-sources`"
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

fn index_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/man_pages.rs")
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn normalized_digest(bytes: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new();
    let mut start = 0;
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] == b'\r' && bytes[index + 1] == b'\n' {
            hasher.update(&bytes[start..index]);
            hasher.update(b"\n");
            index += 2;
            start = index;
        } else {
            index += 1;
        }
    }
    hasher.update(&bytes[start..]);
    format!("blake3:{}", hasher.finalize().to_hex())
}

fn validate_derived_sources() -> Result<()> {
    validate_derived_sources_at(repo_root())
}

fn validate_derived_sources_at(root: &Path) -> Result<()> {
    for page in PAGES {
        for source in page.derived_from {
            let source_path = root.join(source.path);
            let bytes = fs::read(&source_path).map_err(|error| {
                io::Error::other(format!(
                    "cannot read derived source for `{}`: {}: {error}",
                    page.name,
                    source_path.display()
                ))
            })?;
            let actual = normalized_digest(&bytes);
            if actual != source.digest {
                let reviewed = page
                    .parts
                    .map(|parts| format!("man/{parts}"))
                    .unwrap_or_else(|| format!("{}.{}", page.name, page.section));
                return Err(io::Error::other(format!(
                    "{} changed since {reviewed} was last reviewed.\n  recorded: {}\n  actual:   \
                     {actual}\nRe-read {}, update {reviewed} if the prose is now wrong,\nthen run \
                     `cargo run --all-features --example generate-man -- --accept-sources`.",
                    source.path, source.digest, source.path
                ))
                .into());
            }
        }
    }
    Ok(())
}

fn accept_derived_sources() -> Result<()> {
    let generator_path = repo_root().join("examples/generate-man.rs");
    let mut generator = fs::read_to_string(&generator_path)?;
    let original = generator.clone();
    let mut seen = BTreeSet::new();
    for page in PAGES {
        for source in page.derived_from {
            if !seen.insert(source.path) {
                continue;
            }
            let actual = normalized_digest(&fs::read(repo_root().join(source.path))?);
            replace_digest_literal(&mut generator, source, &actual)?;
        }
    }
    if generator != original {
        fs::write(generator_path, generator)?;
    }
    Ok(())
}

fn replace_digest_literal(
    generator: &mut String,
    source: &DerivedSource,
    digest: &str,
) -> Result<()> {
    let marker = format!("const {}: &str =", source.digest_constant);
    let marker_start = generator.find(&marker).ok_or_else(|| {
        io::Error::other(format!(
            "cannot find digest constant `{}` in examples/generate-man.rs",
            source.digest_constant
        ))
    })?;
    let value_start = marker_start
        + generator[marker_start..].find('"').ok_or_else(|| {
            io::Error::other(format!("{} has no string value", source.digest_constant))
        })?;
    let value_end = value_start
        + 1
        + generator[value_start + 1..].find('"').ok_or_else(|| {
            io::Error::other(format!("{} has no closing quote", source.digest_constant))
        })?;
    if &generator[value_start + 1..value_end] != source.digest {
        return Err(io::Error::other(format!(
            "{} contains an unexpected digest",
            source.digest_constant
        ))
        .into());
    }
    generator.replace_range(value_start + 1..value_end, digest);
    Ok(())
}

fn validate_manifest_structure(root: &clap::Command) -> Result<()> {
    let mut actual = BTreeSet::new();
    collect_command_paths(root, &mut Vec::new(), &mut actual);
    let mut covered = BTreeSet::new();

    for page in PAGES {
        if page.commands.is_empty() && page.section == 1 {
            return Err(io::Error::other(format!(
                "section 1 page `{}` does not cover a clap command",
                page.name
            ))
            .into());
        }
        if let Some(parts) = page.parts {
            let path = man_dir().join(parts);
            if !path.is_file() {
                return Err(io::Error::other(format!(
                    "missing prose parts for `{}`: {}",
                    page.name,
                    path.display()
                ))
                .into());
            }
            let parsed = parse_parts(&fs::read_to_string(&path)?)?;
            if page.section == 1 {
                let examples = parsed
                    .sections
                    .get("EXAMPLES")
                    .into_iter()
                    .flatten()
                    .filter(|block| matches!(block, Block::Literal(_)))
                    .count();
                if examples < 2 {
                    return Err(io::Error::other(format!(
                        "section 1 page `{}` needs at least two literal examples",
                        page.name
                    ))
                    .into());
                }
                if parsed
                    .sections
                    .get("SEE ALSO")
                    .is_none_or(|blocks| blocks.is_empty())
                {
                    return Err(io::Error::other(format!(
                        "section 1 page `{}` needs a non-empty SEE ALSO section",
                        page.name
                    ))
                    .into());
                }
            }
        }
        for path in page.commands {
            if find_command(root, path).is_none() {
                return Err(io::Error::other(format!(
                    "page `{}` references unknown command: `{}`",
                    page.name,
                    display_command_path(path)
                ))
                .into());
            }
            covered.insert(path.iter().map(|part| (*part).to_owned()).collect());
        }
    }

    if let Some(path) = actual.difference(&covered).next() {
        return Err(io::Error::other(format!(
            "undocumented command: `{}` is not covered by any page in PAGES",
            display_owned_command_path(path)
        ))
        .into());
    }
    let section_one_pages = PAGES
        .iter()
        .filter(|page| page.section == 1)
        .map(|page| page.name)
        .collect::<BTreeSet<_>>();
    for alias in ALIASES {
        if !section_one_pages.contains(alias.target) {
            return Err(io::Error::other(format!(
                "alias `{}` references unknown section 1 page `{}`",
                alias.name, alias.target
            ))
            .into());
        }
    }
    Ok(())
}

fn collect_command_paths(
    command: &clap::Command,
    path: &mut Vec<String>,
    paths: &mut BTreeSet<Vec<String>>,
) {
    paths.insert(path.clone());
    for child in command
        .get_subcommands()
        .filter(|child| !child.is_hide_set())
    {
        path.push(child.get_name().to_owned());
        collect_command_paths(child, path, paths);
        path.pop();
    }
}

fn find_command<'a>(root: &'a clap::Command, path: &[&str]) -> Option<&'a clap::Command> {
    let mut command = root;
    for name in path {
        command = command
            .get_subcommands()
            .find(|child| !child.is_hide_set() && child.get_name() == *name)?;
    }
    Some(command)
}

fn display_command_path(path: &[&str]) -> String {
    if path.is_empty() {
        "snip".to_owned()
    } else {
        format!("snip {}", path.join(" "))
    }
}

fn display_owned_command_path(path: &[String]) -> String {
    if path.is_empty() {
        "snip".to_owned()
    } else {
        format!("snip {}", path.join(" "))
    }
}

fn render_pages(root: &clap::Command) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut pages = BTreeMap::new();
    for spec in PAGES {
        let filename = format!("{}.{}", spec.name, spec.section);
        if pages
            .insert(filename.clone(), render_page(root, spec)?)
            .is_some()
        {
            return Err(io::Error::other(format!("duplicate manual page `{filename}`")).into());
        }
    }
    for alias in ALIASES {
        let filename = format!("{}.1", alias.name);
        if pages
            .insert(filename.clone(), render_alias(alias)?)
            .is_some()
        {
            return Err(io::Error::other(format!("duplicate manual page `{filename}`")).into());
        }
    }
    Ok(pages)
}

fn render_page(root: &clap::Command, spec: &PageSpec) -> Result<Vec<u8>> {
    let commands = spec
        .commands
        .iter()
        .map(|path| {
            find_command(root, path)
                .cloned()
                .ok_or_else(|| io::Error::other("page manifest was not validated"))
                .map(|command| (path, command))
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let parts = match spec.parts {
        Some(path) => parse_parts(&fs::read_to_string(man_dir().join(path))?)?,
        None => Parts::default(),
    };
    let mut output = Vec::new();
    let title_command = clap::Command::new(spec.name)
        .version(env!("CARGO_PKG_VERSION"))
        .about(spec.title);
    clap_mangen::Man::new(title_command)
        .section(spec.section.to_string())
        .date("\"\"")
        .source(SOURCE)
        .manual(MANUAL)
        .render_title(&mut output)?;

    push_section_heading(&mut output, "NAME");
    push_text_line(&mut output, &format!("{} - {}", spec.name, spec.title));

    if !commands.is_empty() {
        push_section_heading(&mut output, "SYNOPSIS");
        for (path, command) in &commands {
            let command = command_for_page(command.clone(), path.is_empty());
            append_rendered_body(
                &mut output,
                |buffer| clap_mangen::Man::new(command).render_synopsis_section(buffer),
                "SYNOPSIS",
            )?;
        }
        render_part_section(&mut output, &parts, "SYNOPSIS");
    }

    push_section_heading(&mut output, "DESCRIPTION");
    if !render_part_section(&mut output, &parts, "DESCRIPTION") {
        push_paragraph(&mut output, spec.title);
    }
    if spec.section == 1 && spec.name != "snip" {
        push_paragraph(&mut output, "Global options are described in snip(1).");
    }

    if commands.len() == 1 {
        let (path, command) = &commands[0];
        let command = command_for_page(command.clone(), path.is_empty());
        push_section_heading(&mut output, "OPTIONS");
        append_rendered_body(
            &mut output,
            |buffer| clap_mangen::Man::new(command).render_options_section(buffer),
            "OPTIONS",
        )?;
        if path.is_empty() {
            push_section_heading(&mut output, "COMMANDS");
            append_rendered_body(
                &mut output,
                |buffer| clap_mangen::Man::new(root.clone()).render_subcommands_section(buffer),
                "SUBCOMMANDS",
            )?;
        }
    } else if !commands.is_empty() {
        push_section_heading(&mut output, "COMMANDS");
        for (path, command) in &commands {
            push_subsection_heading(&mut output, &display_command_path(path));
            if let Some(about) = command.get_long_about().or_else(|| command.get_about()) {
                push_paragraph(&mut output, &about.to_string());
            }
            let command = command_for_page(command.clone(), false);
            append_rendered_body(
                &mut output,
                |buffer| clap_mangen::Man::new(command).render_options_section(buffer),
                "OPTIONS",
            )?;
        }
    }

    if spec.section == 1 {
        for section in [
            "EXIT STATUS",
            "ENVIRONMENT",
            "FILES",
            "EXAMPLES",
            "SEE ALSO",
        ] {
            if parts.sections.contains_key(section) {
                push_section_heading(&mut output, section);
                render_part_section(&mut output, &parts, section);
            }
        }
    } else {
        for section in &parts.order {
            if section != "DESCRIPTION" {
                push_section_heading(&mut output, section);
                render_part_section(&mut output, &parts, section);
                render_generated_sections(&mut output, spec, section);
            }
        }
    }

    push_section_heading(&mut output, "VERSION");
    append_rendered_body(
        &mut output,
        |buffer| clap_mangen::Man::new(root.clone()).render_version_section(buffer),
        "VERSION",
    )?;
    Ok(output)
}

fn render_generated_sections(output: &mut Vec<u8>, spec: &PageSpec, after_section: &str) {
    if spec.name == "snip-config" && spec.section == 5 && after_section == "LOCATION" {
        push_section_heading(output, "SCHEMA");
        push_paragraph(
            output,
            "The complete schema and representative values are shown below.",
        );
        render_config_schema(output);
        push_section_heading(output, "CONFIGURATION KEYS");
        render_config_fields(output);
    }
    if spec.name == "snip-keys" && spec.section == 5 && after_section == "MODES" {
        push_section_heading(output, "DEFAULT BINDINGS");
        #[cfg(feature = "tui")]
        render_key_bindings(output);
    }
}

fn render_config_schema(output: &mut Vec<u8>) {
    output.extend_from_slice(b".RS 4\n.nf\n");
    let mut current_table = "";
    for field in CONFIG_FIELDS {
        let (table, key) = field
            .toml_path
            .rsplit_once('.')
            .unwrap_or(("", field.toml_path));
        if table != current_table {
            output.push(b'\n');
            push_text_line(output, &format!("[{table}]"));
            current_table = table;
        }
        push_text_line(output, &format!("{key} = {}", field.example));
    }
    output.extend_from_slice(b".fi\n.RE\n");
}

fn render_config_fields(output: &mut Vec<u8>) {
    for (kind, heading) in [
        (FieldKind::Settable, "Settable keys"),
        (FieldKind::FileOnly, "File-only keys"),
        (FieldKind::Managed, "Managed keys"),
    ] {
        push_subsection_heading(output, heading);
        for field in CONFIG_FIELDS.iter().filter(|field| field.kind == kind) {
            let label = config_field_label(field);
            let description = format!("{} Values: {}.", field.summary, field.values);
            push_tagged_paragraph(output, &label, &description);
        }
    }
}

fn config_field_label(field: &ConfigFieldSpec) -> String {
    match (field.kind, field.cli_key) {
        (FieldKind::Settable, Some(key)) => {
            format!("{} (snip config set {})", field.toml_path, key.name())
        }
        (FieldKind::FileOnly, None) => format!("{} (file only)", field.toml_path),
        (FieldKind::Managed, None) => format!("{} (managed by snip)", field.toml_path),
        _ => format!("{} (invalid registry entry)", field.toml_path),
    }
}

#[cfg(feature = "tui")]
fn render_key_bindings(output: &mut Vec<u8>) {
    let document = keydoc::collect(&Keymap::defaults());
    for mode in document.modes {
        push_subsection_heading(output, mode.label);
        push_paragraph(output, mode.blurb);
        if mode.rows.is_empty() {
            push_paragraph(output, "No bindings of its own.");
        }
        for row in mode.rows {
            push_key_row(output, &row);
        }
        if !mode.inherited.is_empty() {
            push_paragraph(output, "Inherited from global:");
            for row in mode.inherited {
                push_key_row(output, &row);
            }
        }
    }
    push_subsection_heading(output, "mouse");
    for mouse in document.mouse {
        push_tagged_paragraph(
            output,
            mouse.key,
            &format!(
                "{} Available in {}.",
                mouse.description,
                mouse.modes.join(", ")
            ),
        );
    }
}

#[cfg(feature = "tui")]
fn push_key_row(output: &mut Vec<u8>, row: &keydoc::KeyRow) {
    let action = row.action.unwrap_or("fixed input");
    push_tagged_paragraph(
        output,
        &row.keys.join(" / "),
        &format!("{action}: {}", row.description),
    );
}

fn push_tagged_paragraph(output: &mut Vec<u8>, tag: &str, description: &str) {
    output.extend_from_slice(b".TP\n");
    output.extend_from_slice(format!("\\fB{}\\fR\n", roff_escape(tag)).as_bytes());
    push_text_line(output, description);
}

fn command_for_page(mut command: clap::Command, is_root: bool) -> clap::Command {
    if !is_root {
        for id in GLOBAL_ARGS {
            if command
                .get_arguments()
                .any(|argument| argument.get_id().as_str() == *id)
            {
                command = command.mut_arg(*id, |argument| argument.hide(true));
            }
        }
    }
    command
}

fn append_rendered_body(
    output: &mut Vec<u8>,
    render: impl FnOnce(&mut Vec<u8>) -> io::Result<()>,
    section: &str,
) -> Result<()> {
    let mut rendered = Vec::new();
    render(&mut rendered)?;
    let rendered = String::from_utf8(rendered)?;
    let heading = format!(".SH {section}\n");
    if let Some(body) = rendered
        .find(&heading)
        .map(|offset| &rendered[offset + heading.len()..])
    {
        output.extend_from_slice(body.as_bytes());
    }
    Ok(())
}

fn render_alias(alias: &AliasSpec) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let title_command = clap::Command::new(alias.name)
        .version(env!("CARGO_PKG_VERSION"))
        .about(alias.title);
    clap_mangen::Man::new(title_command)
        .section("1")
        .date("\"\"")
        .source(SOURCE)
        .manual(MANUAL)
        .render_title(&mut output)?;
    push_section_heading(&mut output, "NAME");
    push_text_line(&mut output, &format!("{} - {}", alias.name, alias.title));
    push_section_heading(&mut output, "DESCRIPTION");
    push_paragraph(&mut output, "This command is documented in");
    push_man_reference(&mut output, alias.target, "1", ".");
    push_section_heading(&mut output, "SEE ALSO");
    push_man_reference(&mut output, "snip", "1", ",");
    push_man_reference(&mut output, alias.target, "1", "");
    Ok(output)
}

fn parse_parts(contents: &str) -> Result<Parts> {
    let mut parts = Parts::default();
    let mut section: Option<String> = None;
    let mut lines = Vec::new();
    for line in contents.lines().chain(std::iter::once("%%__END__")) {
        if let Some(name) = line.strip_prefix("%%") {
            if let Some(previous) = section.take() {
                parts.order.push(previous.clone());
                parts.sections.insert(previous, parse_blocks(&lines));
                lines.clear();
            }
            if name != "__END__" {
                let name = name.trim().to_uppercase();
                if name.is_empty()
                    || !name.bytes().all(|byte| {
                        byte.is_ascii_uppercase()
                            || byte.is_ascii_digit()
                            || matches!(byte, b' ' | b'-')
                    })
                {
                    return Err(io::Error::other(format!(
                        "invalid prose section marker `%%{name}`"
                    ))
                    .into());
                }
                if parts.sections.contains_key(&name) {
                    return Err(io::Error::other(format!(
                        "duplicate prose section marker `%%{name}`"
                    ))
                    .into());
                }
                section = Some(name);
            }
        } else if section.is_some() {
            lines.push(line.to_owned());
        } else if !line.trim().is_empty() {
            return Err(
                io::Error::other("prose content appears before the first %%SECTION").into(),
            );
        }
    }
    Ok(parts)
}

fn parse_blocks(lines: &[String]) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if lines[index].trim().is_empty() {
            index += 1;
            continue;
        }
        if let Some(first) = lines[index].strip_prefix("    ") {
            let mut literal = vec![first.to_owned()];
            index += 1;
            while index < lines.len() {
                if let Some(line) = lines[index].strip_prefix("    ") {
                    literal.push(line.to_owned());
                    index += 1;
                } else {
                    break;
                }
            }
            blocks.push(Block::Literal(literal));
        } else {
            let mut paragraph = Vec::new();
            while index < lines.len()
                && !lines[index].trim().is_empty()
                && !lines[index].starts_with("    ")
            {
                paragraph.extend(lines[index].split_whitespace().map(str::to_owned));
                index += 1;
            }
            blocks.push(Block::Paragraph(paragraph.join(" ")));
        }
    }
    blocks
}

fn render_part_section(output: &mut Vec<u8>, parts: &Parts, section: &str) -> bool {
    let Some(blocks) = parts.sections.get(section) else {
        return false;
    };
    if section == "SEE ALSO" && render_see_also(output, blocks) {
        return true;
    }
    for block in blocks {
        match block {
            Block::Paragraph(text) => push_paragraph(output, text),
            Block::Literal(lines) => {
                output.extend_from_slice(b".RS 4\n.nf\n");
                for line in lines {
                    push_text_line(output, line);
                }
                output.extend_from_slice(b".fi\n.RE\n");
            }
        }
    }
    !blocks.is_empty()
}

fn render_see_also(output: &mut Vec<u8>, blocks: &[Block]) -> bool {
    let mut references = Vec::new();
    for block in blocks {
        let Block::Paragraph(text) = block else {
            return false;
        };
        for reference in text.split(',').map(str::trim) {
            let Some(reference) = parse_man_reference(reference) else {
                return false;
            };
            references.push(reference);
        }
    }
    if references.is_empty() {
        return false;
    }
    for (index, (name, section)) in references.iter().enumerate() {
        let punctuation = if index + 1 == references.len() {
            ""
        } else {
            ","
        };
        push_man_reference(output, name, section, punctuation);
    }
    true
}

fn parse_man_reference(reference: &str) -> Option<(&str, &str)> {
    let (name, section) = reference.rsplit_once('(')?;
    let section = section.strip_suffix(')')?;
    if name.is_empty()
        || name.bytes().any(|byte| byte.is_ascii_whitespace())
        || section.is_empty()
        || !section.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some((name, section))
}

fn push_man_reference(output: &mut Vec<u8>, name: &str, section: &str, punctuation: &str) {
    output.extend_from_slice(
        format!(
            ".BR {} ({}){}\n",
            roff_escape(name),
            roff_escape(section),
            punctuation
        )
        .as_bytes(),
    );
}

fn push_section_heading(output: &mut Vec<u8>, section: &str) {
    output.extend_from_slice(format!(".SH \"{section}\"\n").as_bytes());
}

fn push_subsection_heading(output: &mut Vec<u8>, title: &str) {
    output.extend_from_slice(format!(".SS \"{}\"\n", roff_escape(title)).as_bytes());
}

fn push_paragraph(output: &mut Vec<u8>, text: &str) {
    if !output_ends_with_heading(output) {
        output.extend_from_slice(b".PP\n");
    }
    push_text_line(output, text);
}

fn output_ends_with_heading(output: &[u8]) -> bool {
    let end = output
        .len()
        .saturating_sub(usize::from(output.ends_with(b"\n")));
    let start = output[..end]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    output[start..end].starts_with(b".SH ") || output[start..end].starts_with(b".SS ")
}

fn push_text_line(output: &mut Vec<u8>, text: &str) {
    output.extend_from_slice(roff_escape(text).as_bytes());
    output.push(b'\n');
}

fn roff_escape(text: &str) -> String {
    let mut escaped = String::new();
    if matches!(text.as_bytes().first(), Some(b'.' | b'\'')) {
        escaped.push_str("\\&");
    }
    for character in text.chars() {
        match character {
            '\\' => escaped.push_str("\\e"),
            '-' => escaped.push_str("\\-"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn read_pages(directory: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut pages = BTreeMap::new();
    if !directory.exists() {
        return Ok(pages);
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if !path
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|section| MANAGED_SECTIONS.contains(&section))
        {
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
                changes.changed.push(name.clone())
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

fn render_index(pages: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut index = String::from(
        "// @generated by examples/generate-man.rs; do not edit by hand.\n\
         #[rustfmt::skip]\n\
         pub static PAGES: &[(&str, &[u8])] = &[\n",
    );
    for name in pages.keys() {
        index.push_str(&format!(
            "    (\"{name}\", include_bytes!(\"../../man/{name}\")),\n"
        ));
    }
    index.push_str("];\n");
    index.into_bytes()
}

fn sync_artifacts(expected: &BTreeMap<String, Vec<u8>>) -> Result<()> {
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

    let expected_index = render_index(expected);
    let index = index_path();
    let index_changed = fs::read(&index).ok().as_deref() != Some(expected_index.as_slice());
    if index_changed {
        fs::write(&index, expected_index)?;
    }

    if changes.is_empty() && !index_changed {
        println!(
            "man pages and embedded index are up to date ({} pages)",
            expected.len()
        );
    } else {
        println!(
            "updated man artifacts: {} pages written, {} removed, index {}",
            changes.missing.len() + changes.changed.len(),
            changes.extra.len(),
            if index_changed {
                "written"
            } else {
                "unchanged"
            }
        );
    }
    Ok(())
}

fn check_artifacts(expected: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    let actual = read_pages(&man_dir())?;
    let changes = changes(expected, &actual);
    let expected_index = render_index(expected);
    let index = index_path();
    let actual_index = fs::read(&index);
    let index_changed = actual_index
        .as_ref()
        .is_ok_and(|contents| contents != &expected_index);
    let index_missing = actual_index
        .as_ref()
        .is_err_and(|error| error.kind() == io::ErrorKind::NotFound);
    if let Err(error) = &actual_index
        && error.kind() != io::ErrorKind::NotFound
    {
        return Err(io::Error::new(
            error.kind(),
            format!("cannot read generated index {}: {error}", index.display()),
        )
        .into());
    }
    if changes.is_empty() && !index_missing && !index_changed {
        println!(
            "man pages and embedded index are up to date ({} pages)",
            expected.len()
        );
        return Ok(());
    }
    changes.print();
    if index_missing {
        eprintln!("missing: {}", index.display());
    } else if index_changed {
        eprintln!("changed: {}", index.display());
    }
    Err(
        io::Error::other("generated man artifacts are stale; run the generator without `--check`")
            .into(),
    )
}

fn preview_page(pages: &BTreeMap<String, Vec<u8>>, page: &str) -> Result<()> {
    if page.is_empty()
        || !page
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
    {
        return Err(io::Error::other(format!("invalid page name `{page}`")).into());
    }
    let candidates = if page
        .rsplit_once('.')
        .is_some_and(|(_, section)| MANAGED_SECTIONS.contains(&section))
    {
        vec![page.to_owned()]
    } else {
        MANAGED_SECTIONS
            .iter()
            .map(|section| format!("{page}.{section}"))
            .collect()
    };
    let (filename, contents) = candidates
        .iter()
        .find_map(|filename| pages.get(filename).map(|contents| (filename, contents)))
        .ok_or_else(|| {
        io::Error::other(format!(
            "unknown page `{page}`; choose a generated page such as `snip`, `snip-create`, or `sniplib.5`"
        ))
    })?;
    let generated = tempfile::tempdir()?;
    let path = generated.path().join(filename);
    fs::write(&path, contents)?;
    let path = path.canonicalize()?;
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

#[cfg(test)]
mod tests {
    use super::{
        Block, MANUAL, PAGES, SOURCE, cli::Cli, normalized_digest, parse_parts, render_pages,
        roff_escape, validate_derived_sources_at, validate_manifest_structure,
    };
    use clap::CommandFactory;
    use std::fs;

    #[test]
    fn prose_parser_keeps_literal_blocks_and_collapses_paragraphs() {
        let parts = parse_parts(
            "%%DESCRIPTION\nfirst  line\ncontinued\n\n    snip --output json list\n    .literal\n",
        )
        .unwrap();
        let blocks = &parts.sections["DESCRIPTION"];
        assert!(matches!(&blocks[0], Block::Paragraph(text) if text == "first line continued"));
        assert!(
            matches!(&blocks[1], Block::Literal(lines) if lines == &["snip --output json list", ".literal"])
        );
    }

    #[test]
    fn roff_escape_protects_control_lines_backslashes_and_hyphens() {
        assert_eq!(roff_escape(".snip --tag a\\b"), "\\&.snip \\-\\-tag a\\eb");
        assert_eq!(roff_escape("'quoted'"), "\\&'quoted'");
    }

    #[test]
    fn manifest_rejects_an_uncovered_visible_command() {
        let root = Cli::command().subcommand(clap::Command::new("future-command"));
        let error = validate_manifest_structure(&root).unwrap_err().to_string();
        assert!(error.contains("undocumented command: `snip future-command`"));
    }

    #[test]
    fn rendered_pages_have_one_roff_preamble_and_structured_cross_references() {
        let mut root = Cli::command().disable_help_subcommand(true);
        root.build();
        validate_manifest_structure(&root).unwrap();
        let pages = render_pages(&root).unwrap();
        let preamble = ".ie \\n(.g .ds Aq \\(aq\n.el .ds Aq '\n";

        for (name, page) in &pages {
            let page = String::from_utf8(page.clone()).unwrap();
            assert_eq!(page.matches(preamble).count(), 1, "{name}");
            let (title, section) = name.rsplit_once('.').unwrap();
            let title_line = page.lines().find(|line| line.starts_with(".TH ")).unwrap();
            assert_eq!(
                title_line,
                format!(".TH {title} {section} \"\" \"{SOURCE}\" \"{MANUAL}\"")
            );
            for lines in page.lines().collect::<Vec<_>>().windows(2) {
                assert!(
                    !((lines[0].starts_with(".SH ") || lines[0].starts_with(".SS "))
                        && lines[1] == ".PP"),
                    "{name}: redundant .PP after {}",
                    lines[0]
                );
            }
            let see_also = page
                .split_once(".SH \"SEE ALSO\"\n")
                .unwrap_or_else(|| panic!("{name} has no SEE ALSO section"))
                .1
                .split(".SH ")
                .next()
                .unwrap();
            assert!(see_also.starts_with(".BR "), "{name}: {see_also}");
            assert!(!see_also.contains("\n.PP\n"), "{name}: {see_also}");
        }

        for spec in PAGES {
            let name = format!("{}.{}", spec.name, spec.section);
            let page = String::from_utf8(pages[&name].clone()).unwrap();
            assert_eq!(page.matches(".SH \"VERSION\"\n").count(), 1, "{name}");
            if spec.commands.len() == 1 {
                assert_eq!(page.matches(".SH \"OPTIONS\"\n").count(), 1, "{name}");
            }
        }
    }

    #[test]
    fn normalized_digest_treats_crlf_and_lf_as_the_same_content() {
        assert_eq!(
            normalized_digest(b"first\nsecond\n"),
            normalized_digest(b"first\r\nsecond\r\n")
        );
    }

    #[test]
    fn derived_source_mismatch_names_source_and_recovery_command() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("docs")).unwrap();
        fs::write(root.path().join("FORMAT.md"), "changed\n").unwrap();
        fs::write(root.path().join("docs/themes.md"), "changed\n").unwrap();

        let error = validate_derived_sources_at(root.path())
            .unwrap_err()
            .to_string();
        assert!(error.contains("FORMAT.md changed"));
        assert!(error.contains("recorded: blake3:"));
        assert!(error.contains("actual:   blake3:"));
        assert!(error.contains("--accept-sources"));
    }

    #[test]
    fn derived_source_missing_file_names_the_path() {
        let root = tempfile::tempdir().unwrap();
        let error = validate_derived_sources_at(root.path())
            .unwrap_err()
            .to_string();
        assert!(error.contains("FORMAT.md"));
        assert!(error.contains("cannot read derived source"));
    }
}
