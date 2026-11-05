use assert_cmd::Command;
use predicates::prelude::*;

fn cairn(home: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("cairn").unwrap();
    c.env("CAIRN_HOME", home);
    c
}

#[test]
fn help_lists_subcommands() {
    let d = tempfile::tempdir().unwrap();
    cairn(d.path()).arg("--help").assert().success().stdout(
        predicate::str::contains("versions")
            .and(predicate::str::contains("instance"))
            .and(predicate::str::contains("login")),
    );
}

#[test]
fn instance_list_empty() {
    let d = tempfile::tempdir().unwrap();
    cairn(d.path())
        .args(["instance", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No instances"));
}

#[test]
fn offline_name_error_is_readable() {
    let d = tempfile::tempdir().unwrap();
    cairn(d.path())
        .args(["instance", "launch", "x", "--offline", "a b"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "error: Player names are 3–16 characters",
        ));
}

#[test]
fn accounts_list_after_offline_add() {
    let d = tempfile::tempdir().unwrap();
    cairn(d.path())
        .args(["accounts", "add-offline", "Tester"])
        .assert()
        .success();
    cairn(d.path())
        .args(["accounts", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Tester").and(predicate::str::contains("offline")));
}

#[test]
fn launch_unknown_instance_errors() {
    let d = tempfile::tempdir().unwrap();
    cairn(d.path())
        .args(["instance", "launch", "nope", "--offline", "Tester"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "error: That instance doesn't exist any more.",
        ));
}

#[test]
fn bad_loader_spec_is_rejected() {
    let d = tempfile::tempdir().unwrap();
    cairn(d.path())
        .args([
            "instance", "create", "X", "--mc", "1.20.1", "--loader", "rift",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("rift"));
}
