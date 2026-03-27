use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;

use tempfile::tempdir;

/// Return the repository-local path used for fixtures and expected outputs.
fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(relative)
}

/// Return the path text that the CLI should render for a fixture invoked from the repo root.
fn fixture_display_path(relative: &str) -> String {
    fixture_path(relative)
        .strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .expect("fixture should live under the repository root")
        .to_string_lossy()
        .to_string()
}

/// Return the compiled binary path exposed by Cargo integration tests.
fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_treebrief")
}

/// Run TreeBrief for one input path and capture the full process output.
fn run_treebrief(path: &Path) -> Output {
    run_treebrief_with_args(path, &[])
}

/// Run TreeBrief for one input path plus extra CLI flags.
fn run_treebrief_with_args(path: &Path, args: &[&str]) -> Output {
    // Build the command explicitly so flag-based contract tests hit the real binary surface.
    let mut command = Command::new(binary_path());
    command.args(args).arg(path);
    command.output().expect("failed to run treebrief")
}

/// Run TreeBrief from a different working directory to verify cwd independence.
fn run_treebrief_from_cwd(path: &Path, cwd: &Path) -> Output {
    Command::new(binary_path())
        .current_dir(cwd)
        .arg(path)
        .output()
        .expect("failed to run treebrief from alternate cwd")
}

/// Decode stdout and stderr into UTF-8 strings for assertions.
fn decode_output(output: &Output) -> (String, String) {
    (
        String::from_utf8(output.stdout.clone()).expect("stdout was not valid UTF-8"),
        String::from_utf8(output.stderr.clone()).expect("stderr was not valid UTF-8"),
    )
}

/// Assert that a process completed successfully and return its stdout.
fn successful_stdout(output: Output) -> String {
    let (stdout, stderr) = decode_output(&output);
    assert!(
        output.status.success(),
        "command failed with status {:?}\nstderr:\n{}",
        output.status.code(),
        stderr
    );
    stdout
}

/// Assert that several substrings appear in order inside one output string.
fn assert_in_order(haystack: &str, needles: &[&str]) {
    let mut offset = 0;

    for needle in needles {
        let next = haystack[offset..]
            .find(needle)
            .unwrap_or_else(|| panic!("missing ordered fragment: {needle}"));
        offset += next + needle.len();
    }
}

/// Verify the Python reduced-source contract on a representative single file.
#[test]
fn python_single_file_matches_expected_output() {
    // Lock the CLI contract for comments, nested defs, docstrings, exits, and async members.
    let output = successful_stdout(run_treebrief(&fixture_path(
        "sample_project/src/python_sample.py",
    )));
    let expected = std::fs::read_to_string(fixture_path("expected/python_sample.txt"))
        .expect("failed to read python expectation");

    assert_eq!(output.trim_end(), expected.trim_end());
}

/// Verify the JavaScript reduced-source contract on a representative single file.
#[test]
fn javascript_single_file_matches_expected_output() {
    // Lock the CLI contract for exported functions, arrow constants, JSDoc, and nested closures.
    let output = successful_stdout(run_treebrief(&fixture_path(
        "sample_project/src/javascript_sample.js",
    )));
    let expected = std::fs::read_to_string(fixture_path("expected/javascript_sample.txt"))
        .expect("failed to read javascript expectation");

    assert_eq!(output.trim_end(), expected.trim_end());
}

/// Verify the TypeScript reduced-source contract on a representative single file.
#[test]
fn typescript_single_file_matches_expected_output() {
    // Lock the CLI contract for interfaces, type aliases, namespaces, fields, and nested helpers.
    let output = successful_stdout(run_treebrief(&fixture_path(
        "sample_project/src/typescript_sample.ts",
    )));
    let expected = std::fs::read_to_string(fixture_path("expected/typescript_sample.txt"))
        .expect("failed to read typescript expectation");

    assert_eq!(output.trim_end(), expected.trim_end());
}

/// Verify the Rust reduced-source contract on a representative single file.
#[test]
fn rust_single_file_matches_expected_output() {
    // Lock the CLI contract for attributes, fields, traits, impls, macros, `?`, and tail expressions.
    let output = successful_stdout(run_treebrief(&fixture_path(
        "sample_project/src/rust_sample.rs",
    )));
    let expected = std::fs::read_to_string(fixture_path("expected/rust_sample.txt"))
        .expect("failed to read rust expectation");

    assert_eq!(output.trim_end(), expected.trim_end());
}

/// Verify deterministic directory ordering, ignore rules, and syntax recovery.
#[test]
fn directory_scan_is_deterministic_and_skips_default_ignored_dirs() {
    // Exercise the whole directory contract instead of only isolated file parsing.
    let output = successful_stdout(run_treebrief(&fixture_path("sample_project")));

    assert_in_order(
        &output,
        &[
            "=== scripts/python_tool ===",
            "=== src/javascript_sample.js ===",
            "=== src/python_broken.py ===",
            "=== src/python_sample.py ===",
            "=== src/rust_sample.rs ===",
            "=== src/typescript_sample.ts ===",
        ],
    );
    assert!(output.contains("!! parse failed: parse failed for src/python_broken.py:"));
    assert!(!output.contains("node_modules/ignored.js"));
    assert!(!output.contains("target/ignored.rs"));
}

/// Verify that a broken single file fails clearly instead of pretending to be empty.
#[test]
fn broken_single_file_exits_non_zero_with_parse_error() {
    // Ensure single-file syntax errors are visible and actionable to the caller.
    let output = run_treebrief(&fixture_path("sample_project/src/python_broken.py"));
    let (stdout, stderr) = decode_output(&output);

    assert!(
        !output.status.success(),
        "broken file unexpectedly succeeded"
    );
    assert!(
        stdout.is_empty(),
        "broken single-file run should not print stdout"
    );
    assert_eq!(
        stderr.trim_end(),
        format!(
            "parse failed for {}: syntax error near line 1, column 1",
            fixture_display_path("sample_project/src/python_broken.py")
        )
    );
}

/// Verify that later syntax failures report stable line and column coordinates.
#[test]
fn later_broken_single_file_reports_precise_location() {
    // Pin a non-trivial recovery shape so parse-location reporting cannot drift silently.
    let output = run_treebrief(&fixture_path("broken/python_unexpected_token.py"));
    let (stdout, stderr) = decode_output(&output);

    assert!(
        !output.status.success(),
        "later broken file unexpectedly succeeded"
    );
    assert!(
        stdout.is_empty(),
        "later broken single-file run should not print stdout"
    );
    assert_eq!(
        stderr.trim_end(),
        format!(
            "parse failed for {}: syntax error near line 6, column 5",
            fixture_display_path("broken/python_unexpected_token.py")
        )
    );
}

/// Verify shebang detection and cwd independence for extensionless files.
#[test]
fn extensionless_python_file_works_from_other_cwd() {
    // Run from a throwaway cwd so the implementation cannot accidentally rely on the caller's location.
    let temp = tempdir().expect("failed to create temporary directory");
    let output = successful_stdout(run_treebrief_from_cwd(
        &fixture_path("sample_project/scripts/python_tool"),
        temp.path(),
    ));

    assert!(output.contains("#!/usr/bin/env python3"));
    assert!(output.contains("[5-7] Function: main"));
}

/// Verify that snippet numbering is opt-in and covers every retained item when enabled.
#[test]
fn show_line_numbers_for_all_items_numbers_snippets_too() {
    // Exercise the CLI flag directly so non-definition numbering stays part of the public contract.
    let output = successful_stdout(run_treebrief_with_args(
        &fixture_path("sample_project/src/python_sample.py"),
        &["--show-line-numbers-for-all-items"],
    ));

    assert!(output.contains("[1] \"\"\"Python module docs.\"\"\""));
    assert!(output.contains("[3] # Module context that should stay."));
    assert!(output.contains("    [12] \"\"\"Handle the top-level case.\"\"\""));
    assert!(output.contains("        [16] return value + 1"));
    assert!(output.contains("        [39] return [name.upper() for name in names]"));
}

/// Verify that concurrent invocations produce byte-identical results.
#[test]
fn concurrent_runs_produce_identical_output() {
    // Compare whole-process output so parallel safety is checked at the product boundary.
    let target = fixture_path("sample_project");
    let expected = successful_stdout(run_treebrief(&target));

    let handles = (0..4)
        .map(|_| {
            let target = target.clone();
            thread::spawn(move || successful_stdout(run_treebrief(&target)))
        })
        .collect::<Vec<_>>();

    for handle in handles {
        assert_eq!(expected, handle.join().expect("worker thread panicked"));
    }
}
