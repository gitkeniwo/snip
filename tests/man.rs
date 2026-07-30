use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::Path;

fn command(temporary: &Path) -> Command {
    let mut command = Command::cargo_bin("snip").unwrap();
    command
        .env("HOME", temporary.join("home"))
        .env("XDG_DATA_HOME", temporary.join("data"))
        .env("XDG_CONFIG_HOME", temporary.join("config"))
        .env_remove("SNIP_LIBRARY");
    command
}

fn manifest(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn page_count() -> usize {
    fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("man"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|value| value == "1"))
        .count()
}

#[test]
fn man_path_respects_xdg_data_home_and_json_output() {
    let temporary = tempfile::tempdir().unwrap();
    let output = command(temporary.path())
        .args(["--output", "json", "man", "path"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["path"],
        temporary.path().join("data/man/man1").to_str().unwrap()
    );
}

#[test]
fn man_path_resolves_explicit_prefix_without_loading_config_for_json_output() {
    let temporary = tempfile::tempdir().unwrap();
    let prefix = temporary.path().join("prefix");
    let config = temporary.path().join("config/snip/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(config, "not valid = [toml").unwrap();

    let output = command(temporary.path())
        .arg("--output")
        .arg("json")
        .arg("man")
        .arg("path")
        .arg("--prefix")
        .arg(&prefix)
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["path"],
        prefix.join("share/man/man1").to_str().unwrap()
    );
}

#[cfg(not(windows))]
#[test]
fn man_show_does_not_load_a_broken_config() {
    let temporary = tempfile::tempdir().unwrap();
    let config = temporary.path().join("config/snip/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(config, "not valid = [toml").unwrap();

    command(temporary.path())
        .args(["man", "show", "does-not-exist"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("unknown man page"));
}

#[cfg(not(windows))]
#[test]
fn man_install_is_idempotent_and_uninstall_preserves_modified_pages() {
    let temporary = tempfile::tempdir().unwrap();
    let data_home = temporary.path().join("data");
    let man_dir = data_home.join("man/man1");
    let manifest_path = data_home.join("snip/man-install.json");
    let pages = page_count();

    command(temporary.path())
        .args(["man", "install"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "installed {pages} man pages"
        )));

    let root_page = man_dir.join("snip.1");
    let create_page = man_dir.join("snip-create.1");
    assert!(root_page.is_file());
    assert!(create_page.is_file());
    let first_manifest = fs::read(&manifest_path).unwrap();
    let value = manifest(&manifest_path);
    assert_eq!(value["version"], 1);
    assert_eq!(value["snip_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["files"].as_object().unwrap().len(), pages);
    assert!(
        value["files"]["man/man1/snip.1"]
            .as_str()
            .unwrap()
            .starts_with("blake3:")
    );

    command(temporary.path())
        .args(["man", "install"])
        .assert()
        .success()
        .stdout(predicate::str::contains("(0 updated)"));
    assert_eq!(fs::read(&manifest_path).unwrap(), first_manifest);

    fs::write(&root_page, "locally modified\n").unwrap();
    command(temporary.path())
        .args(["man", "uninstall"])
        .assert()
        .success()
        .stdout(predicate::str::contains("skipped (modified):"))
        .stdout(predicate::str::contains(format!(
            "removed {} man pages",
            pages - 1
        )));

    assert_eq!(
        fs::read_to_string(&root_page).unwrap(),
        "locally modified\n"
    );
    assert!(!create_page.exists());
    let remaining = manifest(&manifest_path);
    let files = remaining["files"].as_object().unwrap();
    assert_eq!(files.len(), 1);
    assert!(files.contains_key("man/man1/snip.1"));
}

#[cfg(not(windows))]
#[test]
fn man_prefix_install_and_uninstall_use_prefix_local_manifest() {
    let temporary = tempfile::tempdir().unwrap();
    let prefix = temporary.path().join("prefix");
    let page = prefix.join("share/man/man1/snip.1");
    let manifest_path = prefix.join("share/snip/man-install.json");
    let pages = page_count();

    command(temporary.path())
        .arg("man")
        .arg("install")
        .arg("--prefix")
        .arg(&prefix)
        .assert()
        .success();

    assert!(page.is_file());
    let value = manifest(&manifest_path);
    assert!(
        value["files"]
            .as_object()
            .unwrap()
            .contains_key("share/man/man1/snip.1")
    );

    command(temporary.path())
        .arg("man")
        .arg("uninstall")
        .arg("--prefix")
        .arg(&prefix)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "removed {pages} man pages"
        )));

    assert!(!page.exists());
    assert!(!manifest_path.exists());
}

#[cfg(not(windows))]
#[test]
fn man_install_refuses_unrecorded_pages_without_force() {
    let temporary = tempfile::tempdir().unwrap();
    let page = temporary.path().join("data/man/man1/snip.1");
    fs::create_dir_all(page.parent().unwrap()).unwrap();
    fs::write(&page, "foreign page\n").unwrap();

    command(temporary.path())
        .args(["man", "install"])
        .assert()
        .code(4)
        .stderr(predicate::str::contains("is not recorded"))
        .stderr(predicate::str::contains("--force"));
    assert_eq!(fs::read_to_string(&page).unwrap(), "foreign page\n");

    command(temporary.path())
        .args(["man", "install", "--force"])
        .assert()
        .success();
    assert_ne!(fs::read_to_string(&page).unwrap(), "foreign page\n");
}

#[cfg(unix)]
#[test]
fn man_force_replaces_a_page_symlink_without_writing_through_it() {
    use std::os::unix::fs::symlink;

    let temporary = tempfile::tempdir().unwrap();
    let page = temporary.path().join("data/man/man1/snip.1");
    let external = temporary.path().join("external.1");
    fs::create_dir_all(page.parent().unwrap()).unwrap();
    let embedded = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("man/snip.1")).unwrap();
    fs::write(&external, &embedded).unwrap();
    symlink(&external, &page).unwrap();

    command(temporary.path())
        .args(["man", "install", "--force"])
        .assert()
        .success();

    assert!(
        !fs::symlink_metadata(&page)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&external).unwrap(), embedded);
}

#[test]
fn man_generate_exports_embedded_pages() {
    let temporary = tempfile::tempdir().unwrap();
    let destination = temporary.path().join("exported");
    let pages = page_count();
    command(temporary.path())
        .arg("man")
        .arg("generate")
        .arg(&destination)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "generated {pages} man pages"
        )));
    assert!(destination.join("snip.1").is_file());
    assert!(destination.join("snip-man-install.1").is_file());
}

#[cfg(not(windows))]
#[test]
fn man_show_rejects_unknown_pages_and_traversal() {
    let temporary = tempfile::tempdir().unwrap();
    command(temporary.path())
        .args(["man", "show", "does-not-exist"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("unknown man page"));
    command(temporary.path())
        .args(["man", "show", "../snip"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("invalid man page name"));
}

#[cfg(windows)]
#[test]
fn man_platform_specific_operations_are_clear_on_windows() {
    let temporary = tempfile::tempdir().unwrap();
    for arguments in [
        vec!["man", "install"],
        vec!["man", "uninstall"],
        vec!["man", "show", "snip"],
    ] {
        command(temporary.path())
            .args(arguments)
            .assert()
            .code(2)
            .stderr(predicate::str::contains("not supported on Windows"));
    }
}
