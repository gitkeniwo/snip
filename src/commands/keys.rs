use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;
use snip::error::{Result, SnipError};
use snip::keys::{Chord, DiagnosticLevel, Keymap, Mode};
use snip::tui::command::{self, CommandId};

use super::output::{print_record, print_records, resolve_output};
use crate::cli::{KeyModeArg, KeysArgs, KeysCommand, OutputMode};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum BindingSource {
    Default,
    User,
}

#[derive(Clone, Debug, Serialize)]
struct BindingRecord {
    mode: String,
    chord: String,
    action: String,
    source: BindingSource,
}

#[derive(Debug, Serialize)]
struct ShowRecord {
    action: String,
    bindings: Vec<BindingRecord>,
}

#[derive(Clone, Debug, Serialize)]
struct CheckDiagnostic {
    level: &'static str,
    message: String,
}

#[derive(Debug, Serialize)]
struct CheckRecord {
    path: PathBuf,
    ok: bool,
    diagnostics: Vec<CheckDiagnostic>,
}

pub fn command_keys(args: &KeysArgs, explicit_output: Option<OutputMode>) -> Result<()> {
    let config = snip::config::AppConfig::load()?;
    let output = resolve_output(explicit_output, &config);
    let path = snip::keys::path()?;
    match &args.command {
        KeysCommand::List { mode } => command_list(&path, mode.map(key_mode), output),
        KeysCommand::Show { action } => command_show(&path, action, output),
        KeysCommand::Path => command_path(&path, output),
        KeysCommand::Export { mode, force } => {
            command_export(&path, mode.map(key_mode), *force, output)
        }
        KeysCommand::Check => command_check(&path, output),
    }
}

fn command_list(path: &Path, mode: Option<Mode>, output: OutputMode) -> Result<()> {
    let (keymap, _) = Keymap::load_from(path)?;
    let defaults = Keymap::defaults();
    let modes = selected_modes(mode);
    let records = binding_records(&keymap, &defaults, &modes);
    if output != OutputMode::Human {
        return print_records(&records, output);
    }

    for (index, mode) in modes.into_iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!("{}:", mode.config_name());
        let mut found = false;
        for record in records
            .iter()
            .filter(|record| record.mode == mode.config_name())
        {
            found = true;
            println!(
                "  {:<14} {:<38} {}",
                record.chord,
                record.action,
                source_name(record.source)
            );
        }
        if !found {
            println!("  (no bindings)");
        }
    }
    Ok(())
}

fn command_show(path: &Path, action: &str, output: OutputMode) -> Result<()> {
    let id = command::by_slug(action).ok_or_else(|| unknown_action(action))?;
    let action = command::get(id).slug;
    let (keymap, _) = Keymap::load_from(path)?;
    let defaults = Keymap::defaults();
    let bindings = binding_records(&keymap, &defaults, &Mode::CONFIGURABLE)
        .into_iter()
        .filter(|record| record.action == action)
        .collect::<Vec<_>>();
    let record = ShowRecord {
        action: action.to_owned(),
        bindings,
    };
    if output != OutputMode::Human {
        return print_record(&record, output);
    }

    println!("action: {action}");
    if record.bindings.is_empty() {
        println!("  (no bindings)");
    } else {
        for binding in record.bindings {
            println!(
                "  {:<14} {:<14} {}",
                binding.mode,
                binding.chord,
                source_name(binding.source)
            );
        }
    }
    Ok(())
}

fn command_path(path: &Path, output: OutputMode) -> Result<()> {
    if output == OutputMode::Human {
        println!("{}", path.display());
        Ok(())
    } else {
        print_record(&json!({ "path": path }), output)
    }
}

fn command_export(path: &Path, mode: Option<Mode>, force: bool, output: OutputMode) -> Result<()> {
    if path.exists() && !force {
        return Err(SnipError::validation(format!(
            "{} already exists; pass --force to overwrite",
            path.display()
        )));
    }
    let modes = selected_modes(mode);
    let contents = export_toml(&modes, mode.is_none());
    let parent = path
        .parent()
        .ok_or_else(|| SnipError::io(format!("{} has no parent directory", path.display())))?;
    fs::create_dir_all(parent)
        .map_err(|error| SnipError::io(format!("cannot create {}: {error}", parent.display())))?;
    fs::write(path, contents)
        .map_err(|error| SnipError::io(format!("cannot write {}: {error}", path.display())))?;
    if output == OutputMode::Human {
        println!("exported: {}", path.display());
        Ok(())
    } else {
        print_record(
            &json!({
                "path": path,
                "modes": modes.iter().map(|mode| mode.config_name()).collect::<Vec<_>>(),
            }),
            output,
        )
    }
}

fn command_check(path: &Path, output: OutputMode) -> Result<()> {
    let (keymap, load_diagnostics) = Keymap::load_from(path)?;
    let defaults = Keymap::defaults();
    let mut diagnostics = load_diagnostics
        .into_iter()
        .map(|diagnostic| CheckDiagnostic {
            level: match diagnostic.level {
                DiagnosticLevel::Error => "error",
                DiagnosticLevel::Info => "info",
            },
            message: diagnostic.message,
        })
        .collect::<Vec<_>>();
    let missing_actions = missing_default_actions(&keymap, &defaults, &diagnostics);
    diagnostics.extend(missing_actions);
    diagnostics.extend(missing_exit_bindings(&keymap));
    diagnostics.extend(cross_mode_shadowing(&keymap, &defaults));
    let ok = !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.level == "error");
    let record = CheckRecord {
        path: path.to_owned(),
        ok,
        diagnostics,
    };

    if output == OutputMode::Human {
        println!("keys: {}", path.display());
        if record.diagnostics.is_empty() {
            println!("  ok  no issues found");
        } else {
            for diagnostic in &record.diagnostics {
                println!("  {:<7} {}", diagnostic.level, diagnostic.message);
            }
        }
    } else {
        print_record(&record, output)?;
    }
    if !ok {
        std::process::exit(1);
    }
    Ok(())
}

fn binding_records(keymap: &Keymap, defaults: &Keymap, modes: &[Mode]) -> Vec<BindingRecord> {
    let mut records = Vec::new();
    for &mode in modes {
        let mut bindings = keymap.bindings_for(mode).collect::<Vec<_>>();
        bindings.sort_unstable_by_key(|(chord, _)| chord.canonical());
        records.extend(bindings.into_iter().map(|(chord, id)| BindingRecord {
            mode: mode.config_name().to_owned(),
            chord: chord.canonical(),
            action: command::get(id).slug.to_owned(),
            source: binding_source(defaults, mode, chord, id),
        }));
    }
    records
}

fn binding_source(defaults: &Keymap, mode: Mode, chord: Chord, id: CommandId) -> BindingSource {
    if exact_binding(defaults, mode, chord) == Some(id) {
        BindingSource::Default
    } else {
        BindingSource::User
    }
}

fn exact_binding(keymap: &Keymap, mode: Mode, chord: Chord) -> Option<CommandId> {
    keymap
        .bindings_for(mode)
        .find_map(|(bound_chord, id)| (bound_chord == chord).then_some(id))
}

fn export_toml(modes: &[Mode], authoritative: bool) -> String {
    let defaults = Keymap::defaults();
    let mut output = String::new();
    if authoritative {
        output.push_str("inherit-defaults = false\n");
    }
    for &mode in modes {
        let mut actions = BTreeMap::<&str, Vec<String>>::new();
        for (chord, id) in defaults.bindings_for(mode) {
            actions
                .entry(command::get(id).slug)
                .or_default()
                .push(chord.canonical());
        }
        for chords in actions.values_mut() {
            chords.sort();
            chords.dedup();
        }
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&format!("[{}]\n", mode.config_name()));
        for (action, chords) in actions {
            let value = toml::Value::Array(
                chords
                    .into_iter()
                    .map(toml::Value::String)
                    .collect::<Vec<_>>(),
            );
            output.push_str(&format!("\"{action}\" = {value}\n"));
        }
    }
    output
}

fn missing_default_actions(
    keymap: &Keymap,
    defaults: &Keymap,
    existing: &[CheckDiagnostic],
) -> Vec<CheckDiagnostic> {
    let mut diagnostics = Vec::new();
    for mode in Mode::CONFIGURABLE {
        let mut default_actions = defaults
            .bindings_for(mode)
            .map(|(_, id)| id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        default_actions.sort_unstable_by_key(|action| command::get(*action).slug);
        for action in default_actions {
            let slug = command::get(action).slug;
            let eviction_suffix =
                format!("; {slug} now has no key in mode \"{}\"", mode.config_name());
            let eviction_already_reported = existing.iter().any(|diagnostic| {
                diagnostic.level == "info" && diagnostic.message.ends_with(&eviction_suffix)
            });
            if keymap.chords_for(&[mode], action).is_empty() && !eviction_already_reported {
                diagnostics.push(CheckDiagnostic {
                    level: "info",
                    message: format!("{} has no key in mode \"{}\"", slug, mode.config_name()),
                });
            }
        }
    }
    diagnostics
}

fn missing_exit_bindings(keymap: &Keymap) -> Vec<CheckDiagnostic> {
    let modes: &[(Mode, &[CommandId])] = &[
        (
            Mode::FragmentGrab,
            &[CommandId::GrabDrop, CommandId::UiDismiss],
        ),
        (
            Mode::Trash,
            &[CommandId::UiDismiss, CommandId::LibraryToggleTrash],
        ),
        (
            Mode::Help,
            &[CommandId::UiDismiss, CommandId::ViewToggleHelp],
        ),
        (
            Mode::Git,
            &[CommandId::UiDismiss, CommandId::GitToggleConsole],
        ),
        (
            Mode::Gist,
            &[CommandId::UiDismiss, CommandId::GistTogglePanel],
        ),
    ];
    modes
        .iter()
        .filter(|(mode, actions)| {
            !actions
                .iter()
                .any(|action| action_reachable(keymap, *mode, *action))
        })
        .map(|(mode, _)| CheckDiagnostic {
            level: "warning",
            message: format!("mode \"{}\" has no exit binding", mode.config_name()),
        })
        .collect()
}

fn action_reachable(keymap: &Keymap, mode: Mode, action: CommandId) -> bool {
    !keymap.chords_for(&[mode], action).is_empty()
        || mode.inherits().contains(&action)
            && !keymap.chords_for(&[Mode::Global], action).is_empty()
}

fn cross_mode_shadowing(keymap: &Keymap, defaults: &Keymap) -> Vec<CheckDiagnostic> {
    let relationships = [
        (Mode::Sidebar, Mode::Global, false),
        (Mode::List, Mode::Global, false),
        (Mode::Preview, Mode::Global, false),
        (Mode::Fragment, Mode::Preview, false),
        (Mode::Fragment, Mode::Global, false),
        (Mode::FragmentGrab, Mode::Global, true),
        (Mode::Trash, Mode::Global, true),
        (Mode::Help, Mode::Global, true),
        (Mode::Git, Mode::Global, true),
        (Mode::Gist, Mode::Global, true),
    ];
    let mut messages = BTreeSet::new();
    for (mode, inherited, allowlist_only) in relationships {
        for (chord, action) in keymap.bindings_for(mode) {
            let Some(inherited_action) = exact_binding(keymap, inherited, chord) else {
                continue;
            };
            if allowlist_only && !mode.inherits().contains(&inherited_action) {
                continue;
            }
            if action == inherited_action
                || binding_source(defaults, mode, chord, action) == BindingSource::Default
                    && binding_source(defaults, inherited, chord, inherited_action)
                        == BindingSource::Default
            {
                continue;
            }
            messages.insert(format!(
                "{} runs {} in mode \"{}\" and shadows {} from mode \"{}\"",
                chord.canonical(),
                command::get(action).slug,
                mode.config_name(),
                command::get(inherited_action).slug,
                inherited.config_name()
            ));
        }
    }
    messages
        .into_iter()
        .map(|message| CheckDiagnostic {
            level: "info",
            message,
        })
        .collect()
}

fn selected_modes(mode: Option<Mode>) -> Vec<Mode> {
    mode.map_or_else(|| Mode::CONFIGURABLE.to_vec(), |mode| vec![mode])
}

fn key_mode(mode: KeyModeArg) -> Mode {
    match mode {
        KeyModeArg::Global => Mode::Global,
        KeyModeArg::Sidebar => Mode::Sidebar,
        KeyModeArg::List => Mode::List,
        KeyModeArg::Preview => Mode::Preview,
        KeyModeArg::Fragment => Mode::Fragment,
        KeyModeArg::FragmentGrab => Mode::FragmentGrab,
        KeyModeArg::Trash => Mode::Trash,
        KeyModeArg::Help => Mode::Help,
        KeyModeArg::Git => Mode::Git,
        KeyModeArg::Gist => Mode::Gist,
    }
}

fn source_name(source: BindingSource) -> &'static str {
    match source {
        BindingSource::Default => "default",
        BindingSource::User => "user",
    }
}

fn unknown_action(action: &str) -> SnipError {
    let suggestion = command::registry()
        .iter()
        .map(|candidate| (edit_distance(action, candidate.slug), candidate.slug))
        .min_by_key(|(distance, _)| *distance)
        .filter(|(distance, _)| *distance <= 3)
        .map(|(_, candidate)| candidate);
    let error = SnipError::not_found(format!("unknown key action \"{action}\""));
    match suggestion {
        Some(candidate) => error.with_hint(format!("did you mean \"{candidate}\"?")),
        None => error,
    }
}

fn edit_distance(left: &str, right: &str) -> usize {
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    for (left_index, left_byte) in left.bytes().enumerate() {
        let mut current = vec![left_index + 1];
        for (right_index, right_byte) in right.bytes().enumerate() {
            current.push(
                (previous[right_index + 1] + 1).min(
                    (current[right_index] + 1)
                        .min(previous[right_index] + usize::from(left_byte != right_byte)),
                ),
            );
        }
        previous = current;
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_is_authoritative_stable_and_mode_filtered() {
        let full_export = export_toml(&Mode::CONFIGURABLE, true);
        assert!(full_export.starts_with("inherit-defaults = false\n\n[global]\n"));

        let export = export_toml(&[Mode::Global], false);
        assert!(export.starts_with("[global]\n"));
        assert!(!export.contains("inherit-defaults"));
        assert!(export.contains("\"palette.open\" = [\":\", \"ctrl-p\"]"));
        assert!(!export.contains("[list]"));
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("keys.toml");
        fs::write(&path, export).unwrap();
        let (loaded, diagnostics) = Keymap::load_from(&path).unwrap();
        assert!(diagnostics.is_empty());
        assert_eq!(
            loaded.resolve(&[Mode::Global], "ctrl-p".parse().unwrap()),
            Some(CommandId::PaletteOpen)
        );
        assert_eq!(
            loaded.resolve(&[Mode::List], "e".parse().unwrap()),
            Some(CommandId::SnippetEditContent)
        );
    }

    #[test]
    fn edit_distance_supports_action_suggestions() {
        assert_eq!(
            edit_distance("snippet.edit-contnt", "snippet.edit-content"),
            1
        );
        assert_eq!(edit_distance("abc", "xyz"), 3);
    }
}
