#![allow(clippy::unwrap_used)]
//! Integration tests for `Probe::new_with_deps` and `Probe::for_definition_with_deps`.
//! These tests exercise the filesystem I/O path end-to-end without spawning rust-analyzer.

use rust_meth::Probe;
use std::fs;

// ── new_with_deps ────────────────────────────────────────────────────────────

#[test]
fn new_with_deps_none_creates_valid_project() {
    let p = Probe::new_with_deps("Vec<u8>", None).expect("probe creation should not fail");

    assert!(p.dir.exists(), "probe dir should exist");
    assert!(p.dir.join("Cargo.toml").exists());
    assert!(p.dir.join("src").join("main.rs").exists());

    let cargo = fs::read_to_string(p.dir.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("[package]"));
    assert!(cargo.contains(r#"name = "probe""#));
    assert!(cargo.contains(r#"edition = "2024""#));
    assert!(
        !cargo.contains("[dependencies]"),
        "no deps arg → no [dependencies] section"
    );
}

#[test]
fn new_with_deps_some_injects_dep_into_cargo_toml() {
    let p = Probe::new_with_deps("serde_json::Value", Some(r#"serde_json = "1.0""#))
        .expect("probe creation should not fail");

    let cargo = fs::read_to_string(p.dir.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("[dependencies]"));
    assert!(cargo.contains(r#"serde_json = "1.0""#));
}

#[test]
fn new_with_deps_source_contains_type_name() {
    let p = Probe::new_with_deps("HashMap<String, u32>", None).unwrap();
    let src = p.source().unwrap();
    assert!(
        src.contains("let _x: HashMap<String, u32> = todo!();"),
        "type name should appear in source"
    );
    assert!(
        src.contains("    _x.\n"),
        "completion dot trigger should be present"
    );
}

#[test]
fn new_with_deps_third_party_type_and_dep() {
    let p = Probe::new_with_deps("serde_json::Value", Some(r#"serde_json = "1.0""#)).unwrap();

    let cargo = fs::read_to_string(p.dir.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("[dependencies]"));

    let src = p.source().unwrap();
    assert!(src.contains("let _x: serde_json::Value = todo!();"));
    assert!(src.contains("    _x.\n"));
}

#[test]
fn new_with_deps_multiple_deps() {
    let deps = "serde = { version = \"1.0\", features = [\"derive\"] }\nserde_json = \"1.0\"";
    let p = Probe::new_with_deps("serde_json::Value", Some(deps)).unwrap();

    let cargo = fs::read_to_string(p.dir.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("[dependencies]"));
    assert!(cargo.contains("serde ="));
    assert!(cargo.contains(r#"serde_json = "1.0""#));
}

// ── for_definition_with_deps ─────────────────────────────────────────────────

#[test]
fn for_definition_with_deps_none_source_has_method_call() {
    let p = Probe::for_definition_with_deps("Vec<u8>", "len", None).unwrap();

    let cargo = fs::read_to_string(p.dir.join("Cargo.toml")).unwrap();
    assert!(!cargo.contains("[dependencies]"));

    let src = p.source().unwrap();
    assert!(src.contains("let _x: Vec<u8> = todo!();"));
    assert!(src.contains("_x.len();"), "method call should be present");
}

#[test]
fn for_definition_with_deps_some_injects_dep() {
    let p = Probe::for_definition_with_deps(
        "serde_json::Value",
        "as_str",
        Some(r#"serde_json = "1.0""#),
    )
    .unwrap();

    let cargo = fs::read_to_string(p.dir.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("[dependencies]"));
    assert!(cargo.contains(r#"serde_json = "1.0""#));

    let src = p.source().unwrap();
    assert!(src.contains("serde_json::Value"));
    assert!(src.contains("_x.as_str();"));
}

// ── dot position ─────────────────────────────────────────────────────────────

#[test]
fn dot_col_is_seven_for_completion_probe() {
    // "    _x." == 7 bytes
    let p = Probe::new_with_deps("Vec<u8>", None).unwrap();
    assert_eq!(p.dot_col, 7);
}

#[test]
fn dot_col_is_seven_for_definition_probe() {
    let p = Probe::for_definition_with_deps("Vec<u8>", "len", None).unwrap();
    assert_eq!(p.dot_col, 7);
}

// ── URIs ─────────────────────────────────────────────────────────────────────

#[test]
fn src_uri_is_file_uri_ending_in_main_rs() {
    let p = Probe::new_with_deps("Vec<u8>", None).unwrap();
    let uri = p.src_uri();
    assert!(uri.starts_with("file://"));
    assert!(uri.ends_with("/src/main.rs"));
}

#[test]
fn root_uri_is_file_uri_not_pointing_to_source_file() {
    let p = Probe::new_with_deps("Vec<u8>", None).unwrap();
    let uri = p.root_uri();
    assert!(uri.starts_with("file://"));
    assert!(
        !uri.ends_with("main.rs"),
        "root URI should point to project dir, not source file"
    );
}

// ── cleanup ───────────────────────────────────────────────────────────────────

#[test]
fn drop_removes_completion_probe_directory() {
    let dir = {
        let p = Probe::new_with_deps("Vec<u8>", None).unwrap();
        p.dir.clone()
    };
    assert!(!dir.exists(), "temp dir must be cleaned up on drop");
}

#[test]
fn drop_removes_definition_probe_directory() {
    let dir = {
        let p = Probe::for_definition_with_deps("Vec<u8>", "len", None).unwrap();
        p.dir.clone()
    };
    assert!(!dir.exists());
}
