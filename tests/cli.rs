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
    env!("CARGO_BIN_EXE_uberview")
}

/// Run Uberview for one input path and capture the full process output.
fn run_uberview(path: &Path) -> Output {
    run_uberview_for_inputs(&[path])
}

/// Run Uberview for one input path plus extra CLI flags.
fn run_uberview_with_args(path: &Path, args: &[&str]) -> Output {
    run_uberview_for_inputs_with_args(&[path], args)
}

/// Run Uberview for several input paths and capture the full process output.
fn run_uberview_for_inputs(paths: &[&Path]) -> Output {
    run_uberview_for_inputs_with_args(paths, &[])
}

/// Run Uberview for several input paths plus extra CLI flags.
fn run_uberview_for_inputs_with_args(paths: &[&Path], args: &[&str]) -> Output {
    // Build the command explicitly so flag-based contract tests hit the real binary surface.
    let mut command = Command::new(binary_path());
    command.args(args).args(paths.iter().copied());
    command.output().expect("failed to run uberview")
}

/// Run Uberview from a different working directory to verify cwd independence.
fn run_uberview_from_cwd(path: &Path, cwd: &Path) -> Output {
    Command::new(binary_path())
        .current_dir(cwd)
        .arg(path)
        .output()
        .expect("failed to run uberview from alternate cwd")
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
    // Lock the CLI contract for comments, nested defs, docstrings, and async members.
    let output = successful_stdout(run_uberview(&fixture_path(
        "sample_project/src/python_sample.py",
    )));
    let expected = std::fs::read_to_string(fixture_path("expected/python_sample.txt"))
        .expect("failed to read python expectation");

    assert_eq!(output.trim_end(), expected.trim_end());
}

/// Verify the Markdown reduced-source contract on a representative single file.
#[test]
fn markdown_single_file_matches_expected_output() {
    // Lock the CLI contract for heading-only structure, level nesting, and section-sized line ranges.
    let output = successful_stdout(run_uberview(&fixture_path("sample_project/docs/guide.md")));
    let expected = std::fs::read_to_string(fixture_path("expected/markdown_guide.txt"))
        .expect("failed to read markdown expectation");

    assert_eq!(output.trim_end(), expected.trim_end());
    assert!(!output.contains("# Uberview Markdown"));
    assert!(!output.contains("### Flags"));
    assert!(!output.contains("Install And Run\n    ---------------"));
}

/// Verify the JavaScript reduced-source contract on a representative single file.
#[test]
fn javascript_single_file_matches_expected_output() {
    // Lock the CLI contract for exported functions, arrow constants, JSDoc, and nested closures.
    let output = successful_stdout(run_uberview(&fixture_path(
        "sample_project/src/javascript_sample.js",
    )));
    let expected = std::fs::read_to_string(fixture_path("expected/javascript_sample.txt"))
        .expect("failed to read javascript expectation");

    assert_eq!(output.trim_end(), expected.trim_end());
}

/// Verify the TypeScript reduced-source contract on a representative single file.
#[test]
fn typescript_single_file_matches_expected_output() {
    // Lock the CLI contract for retained grouping headers, callables, and nested helpers.
    let output = successful_stdout(run_uberview(&fixture_path(
        "sample_project/src/typescript_sample.ts",
    )));
    let expected = std::fs::read_to_string(fixture_path("expected/typescript_sample.txt"))
        .expect("failed to read typescript expectation");

    assert_eq!(output.trim_end(), expected.trim_end());
}

/// Verify the Rust reduced-source contract on a representative single file.
#[test]
fn rust_single_file_matches_expected_output() {
    // Lock the CLI contract for retained grouping headers, methods, and opt-in returns.
    let output = successful_stdout(run_uberview(&fixture_path(
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
    let output = successful_stdout(run_uberview(&fixture_path("sample_project")));

    assert_in_order(
        &output,
        &[
            "=== docs/guide.md ===",
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

/// Verify that unsupported explicit inputs stay inline and do not abort later valid files.
#[test]
fn unsupported_explicit_file_does_not_abort_multi_input_run() {
    // Keep multi-input runs productive even when one explicit path cannot resolve to a supported source file.
    let unsupported = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let supported = fixture_path("sample_project/src/python_sample.py");
    let output = successful_stdout(run_uberview_for_inputs(&[
        unsupported.as_path(),
        supported.as_path(),
    ]));
    let unsupported_header = "=== Cargo.toml ===";
    let supported_header = format!(
        "=== {} ===",
        fixture_display_path("sample_project/src/python_sample.py")
    );

    assert_in_order(
        &output,
        &[
            unsupported_header,
            "!! parse failed: unsupported source file:",
            supported_header.as_str(),
            "[8-21] Function: top_level",
        ],
    );
}

/// Verify that Markdown files can be excluded from otherwise normal directory scans.
#[test]
fn exclude_markdown_omits_markdown_files_from_directory_scans() {
    // Keep Markdown in the default scan while preserving an explicit code-only escape hatch.
    let output = successful_stdout(run_uberview_with_args(
        &fixture_path("sample_project"),
        &["--exclude-markdown"],
    ));

    assert!(!output.contains("=== docs/guide.md ==="));
    assert!(output.contains("=== scripts/python_tool ==="));
    assert!(output.contains("=== src/python_sample.py ==="));
}

/// Verify that a broken single file fails clearly instead of pretending to be empty.
#[test]
fn broken_single_file_exits_non_zero_with_parse_error() {
    // Ensure single-file syntax errors are visible and actionable to the caller.
    let output = run_uberview(&fixture_path("sample_project/src/python_broken.py"));
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
    let output = run_uberview(&fixture_path("broken/python_unexpected_token.py"));
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
    let output = successful_stdout(run_uberview_from_cwd(
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
    let output = successful_stdout(run_uberview_with_args(
        &fixture_path("sample_project/src/python_sample.py"),
        &["--show-line-numbers-for-all-items"],
    ));

    assert!(output.contains("[1] \"\"\"Python module docs.\"\"\""));
    assert!(output.contains("[3] # Module context that should stay."));
    assert!(output.contains("    [12] \"\"\"Handle the top-level case.\"\"\""));
    assert!(output.contains("        [15] # Leave the nested exit visible."));
    assert!(output.contains("    [25] # Yield the normalized values."));
    assert!(!output.contains("return value + 1"));
    assert!(!output.contains("return [name.upper() for name in names]"));
}

/// Verify that `--hide-comments` removes plain comments without deleting documentation text.
#[test]
fn hide_comments_preserves_docstrings_and_documentation_comments() {
    // Cover Python docstrings, JavaScript JSDoc, and Rust doc comments under the public CLI flag.
    let python_output = successful_stdout(run_uberview_with_args(
        &fixture_path("sample_project/src/python_sample.py"),
        &["--hide-comments"],
    ));
    let javascript_output = successful_stdout(run_uberview_with_args(
        &fixture_path("sample_project/src/javascript_sample.js"),
        &["--hide-comments"],
    ));
    let rust_output = successful_stdout(run_uberview_with_args(
        &fixture_path("sample_project/src/rust_sample.rs"),
        &["--hide-comments"],
    ));

    assert!(python_output.contains("\"\"\"Python module docs.\"\"\""));
    assert!(python_output.contains("\"\"\"Handle the top-level case.\"\"\""));
    assert!(python_output.contains("\"\"\"Greeter docs.\"\"\""));
    assert!(!python_output.contains("# Module context that should stay."));
    assert!(!python_output.contains("# Normalize before branching."));
    assert!(!python_output.contains("# Keep the final result visible."));

    assert!(javascript_output.contains("/** JavaScript module docs. */"));
    assert!(javascript_output.contains("/** Normalize before returning. */"));
    assert!(javascript_output.contains("/** Format one name. */"));
    assert!(!javascript_output.contains("// Exported helper context."));
    assert!(!javascript_output.contains("// Reject empty prefixes."));
    assert!(!javascript_output.contains("// Keep the method exit."));

    assert!(rust_output.contains("//! Crate docs."));
    assert!(rust_output.contains("/// Public name surface."));
    assert!(rust_output.contains("/// Service behavior."));
    assert!(!rust_output.contains("// Keep the early-exit line."));
    assert!(!rust_output.contains("// Preserve the tail expression."));
}

/// Verify that return-like lines disappear from the default reduced output.
#[test]
fn default_output_omits_exit_like_lines() {
    // Keep the default view focused on structure instead of control-flow exits.
    let python_output = successful_stdout(run_uberview(&fixture_path(
        "sample_project/src/python_sample.py",
    )));
    let javascript_output = successful_stdout(run_uberview(&fixture_path(
        "sample_project/src/javascript_sample.js",
    )));
    let rust_output = successful_stdout(run_uberview(&fixture_path(
        "sample_project/src/rust_sample.rs",
    )));

    assert!(!python_output.contains("return value + 1"));
    assert!(!python_output.contains("raise ValueError"));
    assert!(!python_output.contains("yield name.upper()"));
    assert!(!javascript_output.contains("return normalize(name);"));
    assert!(!javascript_output.contains("throw new Error(\"missing prefix\")"));
    assert!(!rust_output.contains("parse::<usize>()?"));
    assert!(!rust_output.contains("return Err(\"zero\".to_owned())"));
    assert!(!rust_output.contains("Ok(self.name.clone())"));
}

/// Verify that `--show-returns` restores only actual return statements.
#[test]
fn show_returns_restores_actual_returns_only() {
    // Exercise multiple language families so keyword returns come back without reintroducing other exits.
    let python_output = successful_stdout(run_uberview_with_args(
        &fixture_path("sample_project/src/python_sample.py"),
        &["--show-returns"],
    ));
    let javascript_output = successful_stdout(run_uberview_with_args(
        &fixture_path("sample_project/src/javascript_sample.js"),
        &["--show-returns"],
    ));
    let rust_output = successful_stdout(run_uberview_with_args(
        &fixture_path("sample_project/src/rust_sample.rs"),
        &["--show-returns"],
    ));

    assert!(python_output.contains("return value + 1"));
    assert!(python_output.contains("return nested(value)"));
    assert!(python_output.contains("return [name.upper() for name in names]"));
    assert!(!python_output.contains("raise ValueError"));
    assert!(!python_output.contains("yield name.upper()"));

    assert!(javascript_output.contains("return value.trim();"));
    assert!(javascript_output.contains("return normalize(name);"));
    assert!(javascript_output.contains("return (name) => `${prefix}: ${name}`;"));
    assert!(javascript_output.contains("return `${name}!`;"));
    assert!(!javascript_output.contains("throw new Error(\"missing prefix\")"));

    assert!(rust_output.contains("return Err(\"zero\".to_owned())"));
    assert!(!rust_output.contains("parse::<usize>()?"));
    assert!(!rust_output.contains("Ok(self.name.clone())"));
}

/// Verify that mixed file and directory inputs preserve root order while deduplicating overlaps.
#[test]
fn mixed_file_and_directory_inputs_keep_order_without_duplicates() {
    // Keep the caller's root order stable even when one explicit file is also reachable via a later directory root.
    let file = fixture_path("sample_project/src/python_sample.py");
    let directory = fixture_path("sample_project");
    let output = successful_stdout(
        Command::new(binary_path())
            .arg(&file)
            .arg(&file)
            .arg(&directory)
            .output()
            .expect("failed to run uberview with mixed inputs"),
    );

    assert_in_order(
        &output,
        &[
            "=== tests/fixtures/sample_project/src/python_sample.py ===",
            "=== docs/guide.md ===",
            "=== scripts/python_tool ===",
            "=== src/javascript_sample.js ===",
            "=== src/python_broken.py ===",
            "=== src/rust_sample.rs ===",
            "=== src/typescript_sample.ts ===",
        ],
    );
    assert_eq!(
        output
            .matches("=== tests/fixtures/sample_project/src/python_sample.py ===")
            .count(),
        1
    );
}

/// Verify that normalized-equivalent input spellings still deduplicate to one concrete file.
#[test]
fn normalized_file_input_deduplicates_against_later_directory_root() {
    // Collapse spelling-only path differences so `..` segments cannot reintroduce duplicate sections.
    let odd_spelling = PathBuf::from("tests/fixtures/sample_project/src/../src/python_sample.py");
    let directory = PathBuf::from("tests/fixtures/sample_project");
    let output = successful_stdout(
        Command::new(binary_path())
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .arg(&odd_spelling)
            .arg(&directory)
            .output()
            .expect("failed to run uberview with normalized-equivalent inputs"),
    );

    assert_in_order(
        &output,
        &[
            "=== tests/fixtures/sample_project/src/../src/python_sample.py ===",
            "=== docs/guide.md ===",
            "=== scripts/python_tool ===",
            "=== src/javascript_sample.js ===",
            "=== src/python_broken.py ===",
            "=== src/rust_sample.rs ===",
            "=== src/typescript_sample.ts ===",
        ],
    );
    assert_eq!(
        output
            .matches("=== tests/fixtures/sample_project/src/../src/python_sample.py ===")
            .count(),
        1
    );
    assert!(!output.contains("=== src/python_sample.py ==="));
}

/// Verify that overlapping directory roots contribute unique files in root order.
#[test]
fn overlapping_directory_roots_keep_stable_unique_order() {
    // Make sure the later broader root only adds files the earlier narrower root did not already cover.
    let narrow_root = fixture_path("sample_project/src");
    let broad_root = fixture_path("sample_project");
    let output = successful_stdout(
        Command::new(binary_path())
            .arg(&narrow_root)
            .arg(&broad_root)
            .output()
            .expect("failed to run uberview with overlapping directories"),
    );

    assert_in_order(
        &output,
        &[
            "=== javascript_sample.js ===",
            "=== python_broken.py ===",
            "=== python_sample.py ===",
            "=== rust_sample.rs ===",
            "=== typescript_sample.ts ===",
            "=== docs/guide.md ===",
            "=== scripts/python_tool ===",
        ],
    );
    assert_eq!(output.matches("=== javascript_sample.js ===").count(), 1);
    assert_eq!(output.matches("=== python_sample.py ===").count(), 1);
}

/// Verify that top-level symbol-only definitions disappear from the default reduced output.
#[test]
fn default_output_omits_top_level_symbol_runs() {
    // Keep the default map focused on places worth opening while leaving an opt-in escape hatch.
    let temp = tempdir().expect("failed to create temporary directory");
    let file = temp.path().join("top_level_symbols.py");
    std::fs::write(
        &file,
        "A = 1\nB = 2\nC = 3\n\ndef visible() -> int:\n    \"\"\"Visible docs.\"\"\"\n    return 1\n\nD = 4\n",
    )
    .expect("failed to write python fixture");

    let default_output = successful_stdout(run_uberview(&file));
    let explicit_output =
        successful_stdout(run_uberview_with_args(&file, &["--show-top-level-symbols"]));

    assert!(!default_output.contains("Skipped top-level assignments/constants"));
    assert!(!default_output.contains("Assignment: A"));
    assert!(!default_output.contains("Assignment: D"));
    assert!(explicit_output.contains("[1] Assignment: A"));
    assert!(explicit_output.contains("[2] Assignment: B"));
    assert!(explicit_output.contains("[3] Assignment: C"));
    assert!(explicit_output.contains("[9] Assignment: D"));
    assert!(!explicit_output.contains("Skipped top-level assignments/constants"));
}

/// Verify that plain top-level comments survive even when the following symbol run is omitted.
#[test]
fn plain_top_level_comments_are_not_swallowed_by_omitted_symbols() {
    // Keep generic module-context comments visible even when the adjacent symbol run disappears.
    let temp = tempdir().expect("failed to create temporary directory");
    let file = temp.path().join("comment_then_symbols.py");
    std::fs::write(&file, "# module context\nA = 1\nB = 2\n")
        .expect("failed to write python fixture");

    let output = successful_stdout(run_uberview(&file));

    assert!(output.contains("# module context"));
    assert!(!output.contains("Skipped top-level assignments/constants"));
    assert!(!output.contains("Assignment: A"));
    assert!(!output.contains("Assignment: B"));
}

/// Verify that omitted runtime-scope indentation does not leak into retained body output.
#[test]
fn omitted_runtime_scope_indentation_collapses_to_retained_container_depth() {
    // Keep indentation tied only to retained containers so hidden `if` blocks do not skew the map.
    let temp = tempdir().expect("failed to create temporary directory");
    let file = temp.path().join("hidden_scope_indent.py");
    std::fs::write(
        &file,
        "def outer(flag):\n    if flag:\n        # Keep only the retained depth.\n        def nested():\n            # Nested comment.\n            return 1\n",
    )
    .expect("failed to write python fixture");

    let output = successful_stdout(run_uberview(&file));

    assert!(output.contains("    # Keep only the retained depth."));
    assert!(!output.contains("        # Keep only the retained depth."));
    assert!(output.contains("\n\n    # Keep only the retained depth.\n    [4-6] Function: nested"));
    assert!(!output.contains("\n\n        [4-6] Function: nested"));
    assert!(output.contains("        # Nested comment."));
}

/// Verify that decorated methods still read as methods instead of plain functions.
#[test]
fn decorated_python_method_keeps_method_label() {
    // Pin the common `@classmethod` shape so method classification does not regress on wrapped defs.
    let temp = tempdir().expect("failed to create temporary directory");
    let file = temp.path().join("decorated_method.py");
    std::fs::write(
        &file,
        "class Example:\n    @classmethod\n    def build(cls):\n        return cls()\n",
    )
    .expect("failed to write python fixture");

    let output = successful_stdout(run_uberview(&file));

    assert!(output.contains("[1-4] Class: Example"));
    assert!(output.contains("    [2-4] Method: build"));
}

/// Verify that concurrent invocations produce byte-identical results.
#[test]
fn concurrent_runs_produce_identical_output() {
    // Compare whole-process output so parallel safety is checked at the product boundary.
    let target = fixture_path("sample_project");
    let expected = successful_stdout(run_uberview(&target));

    let handles = (0..4)
        .map(|_| {
            let target = target.clone();
            thread::spawn(move || successful_stdout(run_uberview(&target)))
        })
        .collect::<Vec<_>>();

    for handle in handles {
        assert_eq!(expected, handle.join().expect("worker thread panicked"));
    }
}
