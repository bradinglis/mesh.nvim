//! Integration tests for the `mesh` CLI.
//!
//! They build a throwaway vault, index it, and exercise the read commands. No
//! model or network is needed: semantic commands are covered by `smoke.lua`.

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Vault {
    root: PathBuf,
    notes: PathBuf,
    db: PathBuf,
}

impl Vault {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "mesh-cli-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let notes = root.join("notes");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::write(
            notes.join("alpha.md"),
            "---\nid: Alpha\naliases: Alpha\ntype: note\ntags:\n  - topic\n---\n# Alpha\nSee [[Beta]] and #topic.\n",
        )
        .unwrap();
        std::fs::write(
            notes.join("beta.md"),
            "---\nid: Beta\naliases: Beta\ntype: note\ntags: []\n---\n# Beta\nBack to [[Alpha]].\n",
        )
        .unwrap();
        Self {
            db: root.join("mesh.duckdb"),
            notes,
            root,
        }
    }

    fn index(&self) {
        let (code, _, stderr) = self.run(&[
            "update",
            "--vault",
            self.notes.to_str().unwrap(),
            "--db",
            self.db.to_str().unwrap(),
        ]);
        assert_eq!(code, 0, "update failed: {stderr}");
    }

    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_mesh"))
            .args(args)
            .output()
            .expect("failed to run mesh");
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn indexes_and_reads_json() {
    let vault = Vault::new();
    vault.index();

    let (code, stdout, stderr) =
        vault.run(&["notes", "--db", vault.db.to_str().unwrap(), "--json"]);
    assert_eq!(code, 0, "notes failed: {stderr}");
    assert!(stdout.contains("\"id\":\"Alpha\""), "stdout: {stdout}");
    assert!(stdout.contains("\"id\":\"Beta\""), "stdout: {stdout}");
    // Nulls in list columns are dropped, matching the Lua module.
    assert!(!stdout.contains("null],"), "stdout: {stdout}");

    let (code, stdout, _) =
        vault.run(&["thin-notes", "--db", vault.db.to_str().unwrap(), "--jsonl"]);
    assert_eq!(code, 0);
    assert_eq!(stdout.lines().count(), 2);
}

#[test]
fn table_output_has_headers() {
    let vault = Vault::new();
    vault.index();

    let (code, stdout, _) = vault.run(&[
        "notes",
        "--db",
        vault.db.to_str().unwrap(),
        "--format",
        "table",
    ]);
    assert_eq!(code, 0);
    let header = stdout.lines().next().unwrap();
    for column in ["id", "type", "title", "tags", "authors", "path"] {
        assert!(
            header.contains(column),
            "header {header:?} missing {column}"
        );
    }
}

#[test]
fn id_is_pure_and_table_formatted() {
    let vault = Vault::new();
    let (code, stdout, _) = vault.run(&["id", "Foo Bar", "--format", "table"]);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "foo-bar");
}

#[test]
fn backlinks_include_both_directions() {
    let vault = Vault::new();
    vault.index();

    let (code, stdout, stderr) = vault.run(&[
        "backlinks",
        "Beta",
        "--db",
        vault.db.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code, 0, "backlinks failed: {stderr}");
    assert!(
        stdout.contains("\"direction\":\"back\""),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("\"direction\":\"forward\""),
        "stdout: {stdout}"
    );
}

#[test]
fn read_only_open_refuses_a_vault_mismatch() {
    let vault = Vault::new();
    vault.index();

    let (code, _, stderr) = vault.run(&[
        "notes",
        "--vault",
        "/definitely/not/the/vault",
        "--db",
        vault.db.to_str().unwrap(),
    ]);
    assert_eq!(code, 1);
    assert!(stderr.contains("belongs to vault"), "stderr: {stderr}");

    // The mismatch must not have wiped the index.
    let (code, stdout, _) = vault.run(&["notes", "--db", vault.db.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"id\":\"Alpha\""), "stdout: {stdout}");
}

#[test]
fn missing_database_is_a_clear_error() {
    let vault = Vault::new();
    let missing = vault.root.join("absent.duckdb");
    let (code, _, stderr) = vault.run(&["notes", "--db", missing.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(stderr.contains("no database at"), "stderr: {stderr}");
}

#[test]
fn unknown_command_is_a_usage_error() {
    let vault = Vault::new();
    let (code, _, stderr) = vault.run(&["frobnicate"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("unknown command"), "stderr: {stderr}");
}
