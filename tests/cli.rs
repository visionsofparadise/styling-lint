use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;

const MISSING: &str = "fn main() {\n    let value = 1;\n    consume(value);\n}\n";
const FIXED: &str = "fn main() {\n    let value = 1;\n\n    consume(value);\n}\n";
const GROUPED: &str = "fn main() {\n    let first = 1;\n    let second = 2;\n\n    consume(first);\n    consume(second);\n}\n";

struct Fixture(TempDir);

impl Fixture {
    fn new() -> Self {
        let fixture = Self(tempfile::tempdir().unwrap());

        fixture.write("styling-lint.toml", "");

        fixture
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.path().join(relative)
    }

    fn write(&self, relative: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.path(relative);

        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();

        path
    }

    fn run(&self, arguments: &[&str]) -> Output {
        self.run_from(self.0.path(), arguments)
    }

    fn run_from(&self, directory: &Path, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_styling-lint"))
            .current_dir(directory)
            .args(arguments)
            .output()
            .unwrap()
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path(relative)).unwrap()
    }
}

fn assert_exit(output: &Output, expected: i32) {
    assert_eq!(
        output.status.code(),
        Some(expected),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn check_reports_location_and_does_not_write() {
    let fixture = Fixture::new();

    fixture.write("main.rs", MISSING);

    let output = fixture.run(&["check", "main.rs"]);

    assert_exit(&output, 1);
    assert!(String::from_utf8(output.stdout).unwrap().contains(
        "main.rs:3:5: statement-spacing: expected at least 1 blank line(s) between statement groups, found 0"
    ));
    assert_eq!(fixture.read("main.rs"), MISSING);
}

#[test]
fn fix_has_exact_output_and_is_idempotent() {
    let fixture = Fixture::new();

    fixture.write("main.rs", MISSING);

    let first = fixture.run(&["fix", "main.rs"]);

    assert_exit(&first, 0);
    assert!(first.stdout.is_empty());
    assert!(stderr(&first).contains("fixed 1 spacing issue(s) in 1 file(s)"));
    assert_eq!(fixture.read("main.rs"), FIXED);

    let second = fixture.run(&["fix", "main.rs"]);

    assert_exit(&second, 0);
    assert!(stderr(&second).contains("fixed 0 spacing issue(s) in 0 file(s)"));
    assert_eq!(fixture.read("main.rs"), FIXED);
    assert_exit(&fixture.run(&["check", "main.rs"]), 0);
}

#[test]
fn no_paths_scans_current_directory_recursively() {
    let fixture = Fixture::new();

    fixture.write("project/src/main.rs", MISSING);
    fixture.write("outside.rs", "invalid Rust");

    let output = fixture.run_from(&fixture.path("project"), &["fix"]);

    assert_exit(&output, 0);
    assert_eq!(fixture.read("project/src/main.rs"), FIXED);
    assert_eq!(fixture.read("outside.rs"), "invalid Rust");
}

#[test]
fn scanning_respects_ignore_files_without_a_git_repository() {
    let fixture = Fixture::new();

    fixture.write(".gitignore", "gitignored/\n");
    fixture.write(".ignore", "ignored.rs\n");
    fixture.write("gitignored/invalid.rs", "invalid Rust");
    fixture.write("ignored.rs", "invalid Rust");
    fixture.write("target/invalid.rs", "invalid Rust");
    fixture.write("nested/target/invalid.rs", "invalid Rust");
    fixture.write(".git/invalid.rs", "invalid Rust");
    fixture.write("not-rust.txt", "invalid Rust");
    fixture.write("src/main.rs", FIXED);
    fixture.write(".hidden/source.rs", FIXED);

    let output = fixture.run(&["check"]);

    assert_exit(&output, 0);
    assert!(stderr(&output).contains("Checked 2 Rust file(s)"));
}

#[test]
fn explicit_rust_inputs_override_ignore_files() {
    let fixture = Fixture::new();

    fixture.write(".ignore", "ignored.rs\n");
    fixture.write("ignored.rs", MISSING);

    assert_exit(&fixture.run(&["check"]), 0);
    assert_exit(&fixture.run(&["fix", "ignored.rs"]), 0);
    assert_eq!(fixture.read("ignored.rs"), FIXED);
}

#[test]
fn overlapping_and_repeated_inputs_are_processed_once() {
    let fixture = Fixture::new();

    fixture.write("src/main.rs", MISSING);

    let output = fixture.run(&["check", "src", "src/main.rs", "./src/main.rs", "."]);

    assert_exit(&output, 1);
    assert_eq!(
        String::from_utf8(output.stdout.clone())
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert!(stderr(&output).contains("Checked 1 Rust file(s)"));
}

#[test]
fn unicode_paths_and_contents_are_preserved() {
    let fixture = Fixture::new();
    let before = MISSING.replace("value", "café");
    let expected = FIXED.replace("value", "café");

    fixture.write("日本語/entrée.rs", before);

    assert_exit(&fixture.run(&["fix", "日本語/entrée.rs"]), 0);
    assert_eq!(fixture.read("日本語/entrée.rs"), expected);
}

#[test]
fn fixing_preserves_crlf_bytes() {
    let fixture = Fixture::new();

    fixture.write("main.rs", MISSING.replace('\n', "\r\n"));

    assert_exit(&fixture.run(&["fix"]), 0);
    assert_eq!(fixture.read("main.rs"), FIXED.replace('\n', "\r\n"));
}

#[test]
fn nearest_ancestor_config_is_found_from_source_location() {
    let fixture = Fixture::new();

    fixture.write(
        "project/styling-lint.toml",
        "[spacing]\nmin-blank-lines = 2\n",
    );
    fixture.write("project/src/main.rs", FIXED);

    assert_exit(&fixture.run(&["fix", "project/src/main.rs"]), 0);
    assert_eq!(
        fixture.read("project/src/main.rs"),
        FIXED.replace("\n\n", "\n\n\n")
    );
}

#[test]
fn child_config_replaces_parent_and_defaults_omitted_fields() {
    let fixture = Fixture::new();

    fixture.write(
        "styling-lint.toml",
        "exclude = [\"child/**\"]\n[spacing]\nmin-blank-lines = 3\ngroup-assignments = false\n",
    );
    fixture.write(
        "child/styling-lint.toml",
        "[spacing]\ngroup-calls = false\n",
    );
    fixture.write("child/main.rs", GROUPED);

    assert_exit(&fixture.run(&["fix"]), 0);
    assert_eq!(
        fixture.read("child/main.rs"),
        GROUPED.replace("    consume(second);", "\n    consume(second);")
    );
}

#[test]
fn sibling_directories_resolve_separate_configs() {
    let fixture = Fixture::new();

    fixture.write("a/styling-lint.toml", "[spacing]\nmin-blank-lines = 2\n");
    fixture.write("a/main.rs", FIXED);
    fixture.write("b/main.rs", FIXED);

    assert_exit(&fixture.run(&["fix"]), 0);
    assert_eq!(fixture.read("a/main.rs"), FIXED.replace("\n\n", "\n\n\n"));
    assert_eq!(fixture.read("b/main.rs"), FIXED);
}

#[test]
fn hidden_config_is_supported() {
    let fixture = Fixture::new();

    fixture.write(
        "child/.styling-lint.toml",
        "[spacing]\nmin-blank-lines = 2\n",
    );
    fixture.write("child/main.rs", FIXED);

    assert_exit(&fixture.run(&["check", "child"]), 1);
}

#[test]
fn explicit_config_overrides_local_discovery_for_all_targets() {
    let fixture = Fixture::new();

    fixture.write("styling-lint.toml", "malformed = [");
    fixture.write("options.toml", "[spacing]\nmin-blank-lines = 2\n");
    fixture.write("a/main.rs", FIXED);
    fixture.write("b/.styling-lint.toml", "malformed = [");
    fixture.write("b/main.rs", FIXED);

    let output = fixture.run(&["fix", "a", "b", "--config", "options.toml"]);

    assert_exit(&output, 0);

    for path in ["a/main.rs", "b/main.rs"] {
        assert_eq!(fixture.read(path), FIXED.replace("\n\n", "\n\n\n"));
    }
}

#[test]
fn no_config_uses_defaults_even_with_invalid_local_config() {
    let fixture = Fixture::new();

    fixture.write("styling-lint.toml", "malformed = [");
    fixture.write("main.rs", GROUPED);

    assert_exit(&fixture.run(&["--no-config", "check"]), 0);
}

#[test]
fn excludes_are_relative_to_config_directory_including_explicit_inputs() {
    let fixture = Fixture::new();

    fixture.write(
        "project/styling-lint.toml",
        "exclude = [\"generated/**\", \"src/skip_*.rs\"]\n",
    );
    fixture.write("project/generated/invalid.rs", "invalid Rust");
    fixture.write("project/src/skip_bad.rs", "invalid Rust");
    fixture.write("project/src/keep.rs", FIXED);

    let output = fixture.run(&["check", "project"]);

    assert_exit(&output, 0);
    assert!(stderr(&output).contains("Checked 1 Rust file(s)"));
    assert_exit(&fixture.run(&["check", "project/generated/invalid.rs"]), 0);
    assert_exit(
        &fixture.run_from(
            &fixture.path("project/src"),
            &["check", "..", "--config", "../styling-lint.toml"],
        ),
        0,
    );
}

#[test]
fn invalid_configs_are_actionable_errors() {
    let cases = [
        ("malformed = [", "styling-lint.toml"),
        ("unknown = true\n", "unknown field"),
        ("[spacing]\nunknown = true\n", "unknown field"),
        ("[spacing]\nmin-blank-lines = 0\n", "between 1 and 10"),
        ("[spacing]\nmin-blank-lines = 11\n", "between 1 and 10"),
        ("[spacing]\nmin-blank-lines = -1\n", "styling-lint.toml"),
        ("[spacing]\ngroup-calls = 1\n", "styling-lint.toml"),
        ("exclude = [\"[\"]\n", "invalid exclude glob"),
    ];

    for (config, message) in cases {
        let fixture = Fixture::new();

        fixture.write("styling-lint.toml", config);
        fixture.write("main.rs", MISSING);

        let output = fixture.run(&["fix"]);

        assert_exit(&output, 2);
        assert!(stderr(&output).contains(message), "{}", stderr(&output));
        assert_eq!(fixture.read("main.rs"), MISSING);
    }
}

#[test]
fn two_config_names_in_one_directory_are_rejected() {
    let fixture = Fixture::new();

    fixture.write(".styling-lint.toml", "");
    fixture.write("main.rs", MISSING);

    let output = fixture.run(&["fix"]);

    assert_exit(&output, 2);
    assert!(stderr(&output).contains("both styling-lint.toml and .styling-lint.toml exist"));
    assert_eq!(fixture.read("main.rs"), MISSING);
    assert_exit(&fixture.run(&["check", "--config", "styling-lint.toml"]), 1);
}

#[test]
fn syntax_error_preflight_leaves_earlier_files_unchanged() {
    let fixture = Fixture::new();

    fixture.write("a.rs", MISSING);
    fixture.write("z.rs", "fn broken( {\n");

    let output = fixture.run(&["fix"]);

    assert_exit(&output, 2);
    assert!(stderr(&output).contains("z.rs:"));
    assert_eq!(fixture.read("a.rs"), MISSING);
    assert_eq!(fixture.read("z.rs"), "fn broken( {\n");
}

#[test]
fn invalid_utf8_preflight_leaves_earlier_files_unchanged() {
    let fixture = Fixture::new();

    fixture.write("a.rs", MISSING);
    fixture.write("z.rs", [0xff, 0xfe]);

    let output = fixture.run(&["fix"]);

    assert_exit(&output, 2);
    assert!(stderr(&output).contains("z.rs"));
    assert_eq!(fixture.read("a.rs"), MISSING);
    assert_eq!(fs::read(fixture.path("z.rs")).unwrap(), [0xff, 0xfe]);
}

#[test]
fn config_error_preflight_leaves_earlier_files_unchanged() {
    let fixture = Fixture::new();

    fixture.write("a/main.rs", MISSING);
    fixture.write("z/styling-lint.toml", "unknown = true\n");
    fixture.write("z/main.rs", MISSING);

    assert_exit(&fixture.run(&["fix"]), 2);
    assert_eq!(fixture.read("a/main.rs"), MISSING);
    assert_eq!(fixture.read("z/main.rs"), MISSING);
}

#[test]
fn readonly_preflight_leaves_all_files_unchanged_and_check_still_works() {
    let fixture = Fixture::new();

    fixture.write("a.rs", MISSING);

    let readonly = fixture.write("z.rs", MISSING);
    let original_permissions = fs::metadata(&readonly).unwrap().permissions();
    let mut permissions = original_permissions.clone();

    permissions.set_readonly(true);
    fs::set_permissions(&readonly, permissions).unwrap();

    let fix = fixture.run(&["fix"]);
    let check = fixture.run(&["check"]);

    fs::set_permissions(&readonly, original_permissions).unwrap();
    assert_exit(&fix, 2);
    assert!(stderr(&fix).contains("read-only"));
    assert_exit(&check, 1);
    assert_eq!(fixture.read("a.rs"), MISSING);
    assert_eq!(fixture.read("z.rs"), MISSING);
}

#[test]
fn invalid_inputs_and_conflicting_options_return_usage_errors() {
    let fixture = Fixture::new();

    fixture.write("notes.txt", FIXED);

    for arguments in [
        vec!["check", "missing.rs"],
        vec!["check", "notes.txt"],
        vec!["check", "--config", "missing.toml"],
        vec!["check", "--config", "styling-lint.toml", "--no-config"],
        vec!["check", "--unknown"],
        vec!["unknown-command"],
        vec![],
    ] {
        let output = fixture.run(&arguments);

        assert_exit(&output, 2);
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn empty_directory_succeeds() {
    let fixture = Fixture::new();

    let output = fixture.run(&["check"]);

    assert_exit(&output, 0);
    assert!(stderr(&output).contains("Checked 0 Rust file(s)"));
}

#[test]
fn config_command_prints_a_usable_default_configuration() {
    let fixture = Fixture::new();

    let output = fixture.run(&["config"]);

    assert_exit(&output, 0);
    assert!(output.stderr.is_empty());

    let config = String::from_utf8(output.stdout).unwrap();
    let parsed: toml::Value = toml::from_str(&config).unwrap();

    assert_eq!(parsed["spacing"]["min-blank-lines"].as_integer(), Some(1));
    assert_eq!(parsed["spacing"]["group-assignments"].as_bool(), Some(true));
    assert_eq!(parsed["spacing"]["group-calls"].as_bool(), Some(true));
    assert!(parsed["exclude"].as_array().unwrap().is_empty());
    fixture.write("styling-lint.toml", config);
    fixture.write("main.rs", GROUPED);
    assert_exit(&fixture.run(&["check"]), 0);
}

#[test]
fn help_and_version_succeed() {
    let fixture = Fixture::new();

    assert_exit(&fixture.run(&["--help"]), 0);

    let version = fixture.run(&["--version"]);

    assert_exit(&version, 0);
    assert!(
        String::from_utf8(version.stdout)
            .unwrap()
            .contains(env!("CARGO_PKG_VERSION"))
    );
}

#[cfg(unix)]
#[test]
fn scanning_skips_symlinks_and_explicit_symlinks_are_rejected() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();

    fixture.write("outside/main.rs", MISSING);
    fixture.write("project/real.rs", FIXED);
    symlink(
        fixture.path("outside/main.rs"),
        fixture.path("project/link.rs"),
    )
    .unwrap();
    symlink(
        fixture.path("outside"),
        fixture.path("project/linked-directory"),
    )
    .unwrap();

    let scan = fixture.run(&["fix", "project"]);

    assert_exit(&scan, 0);
    assert!(stderr(&scan).contains("Checked 1 Rust file(s)"));
    assert_eq!(fixture.read("outside/main.rs"), MISSING);
    assert_exit(&fixture.run(&["fix", "project/link.rs"]), 2);
    assert_exit(&fixture.run(&["fix", "project/linked-directory"]), 2);
}
