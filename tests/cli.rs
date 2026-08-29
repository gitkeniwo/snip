use assert_cmd::Command;
use predicates::prelude::*;
use snip::config::AppConfig;

#[cfg(feature = "tui")]
#[test]
fn tui_requires_a_terminal_and_bare_non_tty_fails_fast() {
    let temporary = tempfile::tempdir().unwrap();
    let library = temporary.path().join("TuiCli.sniplib");
    Command::cargo_bin("snip")
        .unwrap()
        .args(["init", library.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", library.to_str().unwrap(), "tui"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("requires an interactive terminal"));

    Command::cargo_bin("snip")
        .unwrap()
        .env_remove("SNIP_LIBRARY")
        .env("XDG_CONFIG_HOME", temporary.path().join("empty-config"))
        .assert()
        .code(2)
        .stderr(predicate::str::contains("a command is required"))
        .stderr(predicate::str::contains("Usage: snip"));
}

#[test]
fn cli_json_contract_and_exit_codes() {
    let temporary = tempfile::tempdir().unwrap();
    let library = temporary.path().join("Cli.sniplib");

    Command::cargo_bin("snip")
        .unwrap()
        .args(["init", library.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "--output",
            "json",
            "create",
            "--title",
            "CLI example",
            "--language",
            "text",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"fingerprint\""));

    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "--output",
            "json",
            "list",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("CLI example"));

    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "--output",
            "json",
            "show",
            "missing",
        ])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("\"code\":\"not_found\""));
}

#[test]
fn missing_library_has_a_stable_json_code_and_actionable_hint() {
    let temporary = tempfile::tempdir().unwrap();
    let config_home = temporary.path().join("empty-config");

    Command::cargo_bin("snip")
        .unwrap()
        .current_dir(temporary.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .env_remove("SNIP_LIBRARY")
        .args(["list"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains(
            "no snip library found; pass --library, set SNIP_LIBRARY, run inside a library, or configure default_library",
        ))
        .stderr(predicate::str::contains("snip init"));

    let output = Command::cargo_bin("snip")
        .unwrap()
        .current_dir(temporary.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .env_remove("SNIP_LIBRARY")
        .args(["--output", "json", "list"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["error"]["code"], "no_library");
    assert_eq!(
        value["error"]["message"],
        "no snip library found; pass --library, set SNIP_LIBRARY, run inside a library, or configure default_library"
    );
    assert!(
        value["error"]["hint"]
            .as_str()
            .is_some_and(|hint| hint.contains("snip init"))
    );
}

#[test]
fn piped_bare_snip_keeps_the_usage_error_and_does_not_wait_for_onboarding() {
    let temporary = tempfile::tempdir().unwrap();
    Command::cargo_bin("snip")
        .unwrap()
        .current_dir(temporary.path())
        .env("XDG_CONFIG_HOME", temporary.path().join("empty-config"))
        .env_remove("SNIP_LIBRARY")
        .write_stdin("")
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "a command is required when stdin or stdout is not a terminal",
        ))
        .stderr(predicate::str::contains("snip — set up your library").not());
}

#[test]
fn non_interactive_bare_init_keeps_creating_a_library_in_the_current_directory() {
    let temporary = tempfile::tempdir().unwrap();
    Command::cargo_bin("snip")
        .unwrap()
        .current_dir(temporary.path())
        .env("XDG_CONFIG_HOME", temporary.path().join("empty-config"))
        .env_remove("SNIP_LIBRARY")
        .write_stdin("")
        .arg("init")
        .assert()
        .success();
    assert!(temporary.path().join("snip.toml").is_file());
}

#[test]
fn missing_snippet_remains_not_found_without_a_hint() {
    let temporary = tempfile::tempdir().unwrap();
    let library = temporary.path().join("Real.sniplib");
    Command::cargo_bin("snip")
        .unwrap()
        .args(["init", library.to_str().unwrap()])
        .assert()
        .success();

    let output = Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "--output",
            "json",
            "show",
            "does-not-exist",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["error"]["code"], "not_found");
    assert!(value["error"].get("hint").is_none());
}

#[test]
fn ancestor_discovery_and_raw_cat_work() {
    let temporary = tempfile::tempdir().unwrap();
    let library = temporary.path().join("Discover.sniplib");
    Command::cargo_bin("snip")
        .unwrap()
        .args(["init", library.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "create",
            "--title",
            "Discovered",
        ])
        .assert()
        .success();

    let nested = library.join("nested/deeper");
    std::fs::create_dir_all(&nested).unwrap();
    Command::cargo_bin("snip")
        .unwrap()
        .current_dir(nested)
        .args(["cat", "Discovered"])
        .assert()
        .success()
        .stdout("");
}

#[test]
fn cat_warns_when_defaulting_to_the_first_of_multiple_fragments() {
    let temporary = tempfile::tempdir().unwrap();
    let library = temporary.path().join("Cat.sniplib");
    Command::cargo_bin("snip")
        .unwrap()
        .args(["init", library.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "create",
            "--title",
            "Multiple",
            "--content",
            "first\n",
        ])
        .assert()
        .success();
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "fragment",
            "add",
            "Multiple",
            "--title",
            "Second",
            "--content",
            "second\n",
        ])
        .assert()
        .success();
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "create",
            "--title",
            "Single",
            "--content",
            "only\n",
        ])
        .assert()
        .success();

    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", library.to_str().unwrap(), "cat", "Multiple"])
        .assert()
        .success()
        .stdout("first\n")
        .stderr(predicate::str::contains("1/2"));
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "cat",
            "Multiple",
            "--fragment",
            "2",
        ])
        .assert()
        .success()
        .stdout("second\n")
        .stderr("");
    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", library.to_str().unwrap(), "cat", "Single"])
        .assert()
        .success()
        .stdout("only\n")
        .stderr("");
}

#[test]
fn bare_selector_matches_preview_and_handles_cli_edge_cases() {
    let temporary = tempfile::tempdir().unwrap();
    let library = temporary.path().join("Bare.sniplib");
    Command::cargo_bin("snip")
        .unwrap()
        .args(["init", library.to_str().unwrap()])
        .assert()
        .success();
    for (title, content) in [("Single", "one\n"), ("Multiple", "first\n")] {
        Command::cargo_bin("snip")
            .unwrap()
            .args([
                "--library",
                library.to_str().unwrap(),
                "create",
                "--title",
                title,
                "--content",
                content,
            ])
            .assert()
            .success();
    }
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "fragment",
            "add",
            "Multiple",
            "--title",
            "Second",
            "--content",
            "second\n",
        ])
        .assert()
        .success();
    for folder in ["One", "Two"] {
        Command::cargo_bin("snip")
            .unwrap()
            .args([
                "--library",
                library.to_str().unwrap(),
                "create",
                "--title",
                "Duplicate",
                "--folder",
                folder,
            ])
            .assert()
            .success();
    }

    let bare = Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "--color",
            "never",
            "Single",
        ])
        .output()
        .unwrap();
    let preview = Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "--color",
            "never",
            "preview",
            "Single",
        ])
        .output()
        .unwrap();
    assert!(bare.status.success());
    assert_eq!(bare.stdout, preview.stdout);
    let single = String::from_utf8(bare.stdout).unwrap();
    assert!(single.contains("Single"), "{single}");
    assert!(single.contains("one"), "{single}");

    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "--color",
            "never",
            "Multiple",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("--- 1."))
        .stdout(predicate::str::contains("--- 2."));
    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", library.to_str().unwrap(), "Single", "extra"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("the bare form takes one selector"));
    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", library.to_str().unwrap(), "lst"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("did you mean \"snip list\"?"));
    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", library.to_str().unwrap(), "Duplicate"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("(folder: One)"))
        .stderr(predicate::str::contains("(folder: Two)"));
    Command::cargo_bin("snip")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("preview"));
}

#[test]
fn edit_create_rejects_non_editor_paths_without_creating_a_snippet() {
    let temporary = tempfile::tempdir().unwrap();
    let library = temporary.path().join("EditCreate.sniplib");
    Command::cargo_bin("snip")
        .unwrap()
        .args(["init", library.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "edit",
            "Missing",
            "--create",
            "--content",
            "x",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "--create cannot be combined with structured changes",
        ));
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "edit",
            "Missing",
            "--create",
            "--if-hash",
            "abc",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "--create cannot be combined with --if-hash",
        ));

    let before = Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "--output",
            "json",
            "list",
        ])
        .output()
        .unwrap();
    assert!(before.status.success());
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "edit",
            "Missing",
            "--create",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "external editing requires an interactive terminal",
        ));
    let after = Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "--output",
            "json",
            "list",
        ])
        .output()
        .unwrap();
    assert!(after.status.success());
    assert_eq!(before.stdout, after.stdout);
}

#[test]
fn config_binds_default_library_and_supplies_create_defaults() {
    let temporary = tempfile::tempdir().unwrap();
    let config_home = temporary.path().join("config-home");
    let default_library = temporary.path().join("Default.sniplib");
    let local_library = temporary.path().join("Local.sniplib");

    for library in [&default_library, &local_library] {
        Command::cargo_bin("snip")
            .unwrap()
            .env("XDG_CONFIG_HOME", &config_home)
            .env_remove("SNIP_LIBRARY")
            .args(["init", library.to_str().unwrap()])
            .assert()
            .success();
    }

    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .env_remove("SNIP_LIBRARY")
        .args([
            "config",
            "init",
            "--library",
            default_library.to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("snip")
        .unwrap()
        .current_dir(temporary.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .env_remove("SNIP_LIBRARY")
        .args(["--output", "json", "info"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Default"));

    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args(["config", "set", "git-auto-push", "true"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "automatic push is enabled but git-auto-commit-interval is 0",
        ));

    for (key, value) in [
        ("default-language", "rust"),
        ("default-folder", "Agents/Generated"),
        ("default-tags", "ai, generated, AI"),
        ("tui-theme", "dark"),
        ("tui-sort", "modified"),
        ("tui-density", "compact"),
        ("tui-simplified-ui", "true"),
        ("editor-cwd", "folder"),
        ("git-auto-commit-interval", "15"),
        ("git-auto-pull", "true"),
        ("git-backup-on-quit", "true"),
    ] {
        Command::cargo_bin("snip")
            .unwrap()
            .env("XDG_CONFIG_HOME", &config_home)
            .args(["config", "set", key, value])
            .assert()
            .success();
    }

    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args(["--output", "json", "config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"auto_commit_interval\": 15"))
        .stdout(predicate::str::contains("\"auto_push\": true"))
        .stdout(predicate::str::contains("\"auto_pull\": true"))
        .stdout(predicate::str::contains("\"backup_on_quit\": true"))
        .stdout(predicate::str::contains("\"simplified_ui\": true"))
        .stdout(predicate::str::contains("\"editor_cwd\": \"folder\""));

    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args(["config", "set", "editor-cwd", "somewhere"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "editor-cwd must be inherit, library, folder, snippet, or fragment",
        ));

    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args(["config", "unset", "editor-cwd"])
        .assert()
        .success()
        .stdout(predicate::str::contains("editor_cwd").not());

    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args(["config", "unset", "tui-simplified-ui"])
        .assert()
        .success()
        .stdout(predicate::str::contains("simplified_ui = false"));

    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args(["config", "unset", "git-auto-push"])
        .assert()
        .success()
        .stdout(predicate::str::contains("auto_push = false"));

    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args(["config", "unset", "git-auto-pull"])
        .assert()
        .success()
        .stdout(predicate::str::contains("auto_pull = false"));

    Command::cargo_bin("snip")
        .unwrap()
        .current_dir(temporary.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .env_remove("SNIP_LIBRARY")
        .args(["create", "--title", "Configured"])
        .assert()
        .success();
    Command::cargo_bin("snip")
        .unwrap()
        .current_dir(temporary.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .env_remove("SNIP_LIBRARY")
        .args(["--output", "json", "show", "Configured"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Agents/Generated"))
        .stdout(predicate::str::contains("\"language\": \"rust\""))
        .stdout(predicate::str::contains("\"ai\""))
        .stdout(predicate::str::contains("\"generated\""));

    let nested = local_library.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    Command::cargo_bin("snip")
        .unwrap()
        .current_dir(nested)
        .env("XDG_CONFIG_HOME", &config_home)
        .env_remove("SNIP_LIBRARY")
        .args(["--output", "json", "info"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Local"));
}

#[test]
fn gui_editor_config_round_trips_and_preserves_the_legacy_key() {
    let temporary = tempfile::tempdir().unwrap();
    let config_home = temporary.path().join("config-home");
    let config_path = config_home.join("snip/config.toml");

    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args(["config", "set", "gui-editor", "zed"])
        .assert()
        .success()
        .stdout(predicate::str::contains("gui_editor = \"zed\""));

    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args(["config", "set", "vscode_cmd", "zed"])
        .assert()
        .code(2);

    let mut config = AppConfig::load_from(&config_path).unwrap();
    assert_eq!(config.gui_editor.as_deref(), Some("zed"));
    config.vscode_cmd = Some("code-insiders".to_owned());
    config.save_to(&config_path).unwrap();

    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args(["config", "unset", "gui-editor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("gui_editor").not())
        .stdout(predicate::str::contains("vscode_cmd = \"code-insiders\""))
        .stderr(predicate::str::contains(
            "vscode_cmd is deprecated; use gui_editor instead",
        ));

    let config = AppConfig::load_from(&config_path).unwrap();
    assert_eq!(config.gui_editor, None);
    assert_eq!(config.vscode_cmd.as_deref(), Some("code-insiders"));
}

#[test]
fn config_output_is_default_but_cli_override_wins() {
    let temporary = tempfile::tempdir().unwrap();
    let config_home = temporary.path().join("config-home");
    let library = temporary.path().join("Output.sniplib");
    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args(["init", library.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args(["config", "init", "--library", library.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args(["config", "set", "output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{"));

    Command::cargo_bin("snip")
        .unwrap()
        .current_dir(temporary.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .env_remove("SNIP_LIBRARY")
        .arg("info")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{"));
    Command::cargo_bin("snip")
        .unwrap()
        .current_dir(temporary.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .env_remove("SNIP_LIBRARY")
        .args(["--output", "human", "info"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("path:"));
}
