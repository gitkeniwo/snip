use serde_json::json;
use snip::Library;
use snip::clipboard::ClipboardMethod;
use snip::domain::{RemoteRecord, Snippet};
use snip::error::{ErrorKind, Result, SnipError};
use snip::gist::{self, StatusReport, StatusState};
use std::io::{self, Write};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::output::{print_record, print_records};
use crate::cli::{
    GistArgs, GistAttachArgs, GistCommand, GistDeleteArgs, GistPushArgs, GistSelectorArgs,
    GistStatusArgs, GistUrlArgs, OutputMode,
};

pub fn command_gist(library: &Library, args: &GistArgs, output: OutputMode) -> Result<()> {
    match &args.command {
        GistCommand::Push(args) => command_gist_push(library, args, output),
        GistCommand::Url(args) => command_gist_url(library, args, output),
        GistCommand::Status(args) => command_gist_status(library, args, output),
        GistCommand::Attach(args) => command_gist_attach(library, args, output),
        GistCommand::Detach(args) => command_gist_detach(library, args, output),
        GistCommand::Delete(args) => command_gist_delete(library, args, output),
        GistCommand::Open(args) => command_gist_open(library, args, output),
    }
}

fn command_gist_push(library: &Library, args: &GistPushArgs, output: OutputMode) -> Result<()> {
    let options = gist::PushOptions {
        public: args.public,
        description: args.desc.clone(),
        new: args.new,
        include_notes: args.include_notes,
        include_readme: !args.no_readme,
        if_hash: args.if_hash.clone(),
        force: args.force,
    };
    let outcome = gist::push(library, &args.selector, &options)?;
    if args.web {
        gist::gh::open_web(&outcome.record().id)?;
    }
    if output == OutputMode::Human {
        match &outcome {
            gist::PushOutcome::Created { record, .. }
            | gist::PushOutcome::Updated { record, .. } => {
                let label = if matches!(outcome, gist::PushOutcome::Created { .. }) {
                    "created gist:"
                } else {
                    "updated gist:"
                };
                println!("{label} {}", record.url);
                println!("visibility: {}", visibility_label(record.public));
                println!("files: {}", record.files.join(", "));
            }
            gist::PushOutcome::Unchanged { record, .. } => {
                println!("gist is already up to date: {}", record.url);
            }
        }
    } else {
        print_record(
            &json!({
                "action": outcome.action(),
                "snippet": outcome.snippet(),
                "gist": outcome.record(),
                "fingerprint": outcome.snippet().fingerprint,
            }),
            output,
        )?;
    }
    Ok(())
}

fn command_gist_url(library: &Library, args: &GistUrlArgs, output: OutputMode) -> Result<()> {
    let snippet = resolve(library, &args.selector)?;
    let record = require_linked(&snippet)?;
    if output == OutputMode::Human {
        println!("{}", record.url);
        if args.copy {
            match snip::clipboard::copy(&record.url)? {
                ClipboardMethod::System => eprintln!("copied to system clipboard"),
                ClipboardMethod::Osc52 => eprintln!("copied to terminal clipboard"),
            }
        }
    } else {
        print_record(
            &json!({
                "url": record.url,
                "id": record.id,
                "host": record.host,
            }),
            output,
        )?;
    }
    Ok(())
}

fn command_gist_status(library: &Library, args: &GistStatusArgs, output: OutputMode) -> Result<()> {
    if args.selector.is_none() && !args.all {
        return Err(SnipError::usage("a selector or --all is required"));
    }
    if args.all {
        let _lock = library.lock()?;
        let catalog = library.scan()?;
        let linked = catalog
            .snippets
            .iter()
            .filter(|snippet| gist::find(snippet).is_some())
            .collect::<Vec<_>>();
        let reports = linked
            .iter()
            .map(|snippet| build_status_report(snippet, args.remote))
            .collect::<Result<Vec<_>>>()?;
        if output == OutputMode::Human {
            let mut first = true;
            for report in &reports {
                if !first {
                    println!();
                }
                first = false;
                println!("snippet: {}", report.snippet.title);
                print_status_human(report);
            }
        } else {
            print_records(&reports, output)?;
        }
    } else {
        let selector = args.selector.as_deref().expect("selector checked above");
        let snippet = resolve(library, selector)?;
        let report = build_status_report(&snippet, args.remote)?;
        if output == OutputMode::Human {
            print_status_human(&report);
        } else {
            print_record(&report, output)?;
        }
    }
    Ok(())
}

fn command_gist_attach(library: &Library, args: &GistAttachArgs, output: OutputMode) -> Result<()> {
    let snippet = gist::attach(library, &args.selector, &args.gist)?;
    let record = gist::find(&snippet).expect("attach writes a gist record");
    if output == OutputMode::Human {
        println!("attached gist: {}", record.url);
        println!("files: {}", record.files.join(", "));
    } else {
        print_record(
            &json!({
                "action": "attached",
                "snippet": snippet,
                "gist": record,
            }),
            output,
        )?;
    }
    Ok(())
}

fn command_gist_detach(
    library: &Library,
    args: &GistSelectorArgs,
    output: OutputMode,
) -> Result<()> {
    let (snippet, record) = gist::detach(library, &args.selector)?;
    if output == OutputMode::Human {
        println!("detached gist: {}", record.id);
    } else {
        print_record(
            &json!({
                "action": "detached",
                "snippet": snippet,
                "gist": record,
            }),
            output,
        )?;
    }
    Ok(())
}

fn command_gist_delete(library: &Library, args: &GistDeleteArgs, output: OutputMode) -> Result<()> {
    if !args.yes && output != OutputMode::Human {
        return Err(SnipError::usage(
            "--yes is required when output is not human-readable",
        ));
    }
    let (snippet, record) = if args.yes {
        gist::delete(library, &args.selector)?
    } else {
        let snippet = resolve(library, &args.selector)?;
        let record = require_linked(&snippet)?.clone();
        eprint!("delete gist {} on GitHub? [y/N] ", record.id);
        io::stderr().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        if !matches!(line.trim(), "y" | "Y") {
            if output == OutputMode::Human {
                println!("cancelled");
            }
            return Ok(());
        }
        gist::delete(library, &args.selector)?
    };
    if output == OutputMode::Human {
        println!("deleted gist: {}", record.id);
    } else {
        print_record(
            &json!({
                "action": "deleted",
                "snippet": snippet,
                "gist": record,
            }),
            output,
        )?;
    }
    Ok(())
}

fn command_gist_open(
    library: &Library,
    args: &GistSelectorArgs,
    _output: OutputMode,
) -> Result<()> {
    let snippet = resolve(library, &args.selector)?;
    let record = require_linked(&snippet)?;
    gist::gh::open_web(&record.id)?;
    Ok(())
}

fn resolve(library: &Library, selector: &str) -> Result<Snippet> {
    let _lock = library.lock()?;
    let catalog = library.scan()?;
    Ok(library.resolve_snippet(&catalog, selector)?.clone())
}

fn require_linked(snippet: &Snippet) -> Result<&RemoteRecord> {
    gist::find(snippet).ok_or_else(|| {
        SnipError::not_found(format!("snippet {} has no gist", snippet.title))
            .with_hint("run: snip gist push <selector>")
    })
}

fn build_status_report(snippet: &Snippet, remote: bool) -> Result<StatusReport> {
    let mut report = gist::status(snippet)?;
    if remote && report.state != StatusState::Unlinked {
        let record = report.record.as_ref().expect("linked report has a record");
        match gist::gh::fetch(&record.id) {
            Ok(_) => {}
            Err(error) if error.kind == ErrorKind::NotFound => {
                report.state = StatusState::Missing;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(report)
}

fn print_status_human(report: &StatusReport) {
    let Some(record) = &report.record else {
        println!("gist: none");
        return;
    };
    println!("gist: {}", record.url);
    println!("visibility: {}", visibility_label(record.public));
    println!("state: {}", report.state.label());
    if let Some(pushed_at) = &record.pushed_at {
        let relative = OffsetDateTime::parse(pushed_at, &Rfc3339)
            .ok()
            .map(|value| {
                snip::git::relative_time(
                    value.unix_timestamp(),
                    OffsetDateTime::now_utc().unix_timestamp(),
                )
            })
            .unwrap_or_else(|| "unknown".to_owned());
        println!("pushed: {pushed_at} ({relative})");
    }
}

fn visibility_label(public: bool) -> &'static str {
    if public { "public" } else { "secret" }
}
