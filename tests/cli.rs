use assert_cmd::Command;
use predicates::prelude::*;

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

    for (key, value) in [
        ("default-language", "rust"),
        ("default-folder", "Agents/Generated"),
        ("default-tags", "ai, generated, AI"),
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
