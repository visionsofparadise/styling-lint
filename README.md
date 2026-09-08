# styling-lint

A Rust statement spacing linter with configurable defaults and automatic fixes.
Use it alongside `rustfmt` to separate statement groups with blank lines.

```rust
fn example() {
    let first = read();
    let second = read();

    process(first);
    process(second);

    return;
}
```

Consecutive assignments stay together. Consecutive calls stay together. Other
statement pairs get at least one blank line. Existing blank lines are preserved.

## Installation

Requires Rust 1.85 or later. During initial development, install from the checkout:

```sh
cargo install --path . --locked
```

After the first crates.io release, installation will be:

```sh
cargo install styling-lint --locked
```

## Usage

```sh
styling-lint check                  # Check Rust files under the current directory
styling-lint fix                    # Insert missing blank lines in place
styling-lint check src tests        # Check several directories
styling-lint fix src/main.rs        # Fix a specific file
styling-lint check --config ci.toml # Use an explicit configuration
styling-lint check --no-config      # Use built-in defaults
styling-lint config > styling-lint.toml
```

Run `cargo fmt` before `styling-lint`. The linter works on parsed source files
without compiling the project or resolving its dependencies.

`check` prints `path:line:column` diagnostics to stdout. Summaries and errors go to
stderr. `fix` reports how many issues and files it changed.

| Exit code | Meaning |
| --- | --- |
| 0 | Check passed, fix completed, or configuration printed |
| 1 | Check found spacing issues |
| 2 | Invalid arguments, configuration, Rust syntax, or filesystem error |

## Configuration

For each Rust file, the linter searches that file's directory and then its ancestors
for `styling-lint.toml` or `.styling-lint.toml`. The nearest configuration wins.
Omitted options use built-in defaults; parent configurations are not merged.
Having both filenames in the same directory is an error. Lookup continues above
repository and workspace roots, so a home-directory configuration can supply
personal defaults.

`--config FILE` resolves from the current working directory and applies that file
to all inputs. `--no-config` bypasses configuration discovery. Unknown keys, invalid
types, and invalid glob patterns produce an error.

```toml
exclude = ["generated/**", "vendor/**", "**/*.generated.rs"]

[spacing]
min-blank-lines = 1
group-assignments = true
group-calls = true
```

| Option | Default | Behavior |
| --- | --- | --- |
| `exclude` | `[]` | Skip files matching any configuration-relative glob |
| `spacing.min-blank-lines` | `1` | Minimum blank lines at a required boundary, from 1 to 10 |
| `spacing.group-assignments` | `true` | Allow consecutive assignment statements without a blank line |
| `spacing.group-calls` | `true` | Allow consecutive call statements without a blank line |

Set both grouping options to `false` to require separation between all statements
on different lines. Increasing `min-blank-lines` adds only the missing blank lines
at required boundaries.

Exclusion globs use forward slashes on every platform. `*` matches within a path
component and `**` spans directories. For a whole directory use `generated/**`;
for a filename anywhere use `**/generated.rs`. An explicit configuration's globs
are relative to its own directory, including when invoked from another directory.
Files outside that directory's tree do not match its exclusions. A nested
configuration replaces the parent's exclusions along with its spacing options.

## File selection

Directory inputs recursively select `.rs` files, including hidden source folders.
Traversal respects `.gitignore`, `.ignore`, and repository-local Git exclude rules,
including ignore files in parent directories. Global Git excludes are disabled so
the user's global Git settings do not change CI results. Ignore files work even
outside a Git repository. `.git` and `target` entries are pruned during traversal.

An explicitly named Rust file bypasses traversal ignores and still respects
configuration exclusions. Duplicate and overlapping paths are checked once.
Symlinks encountered during traversal are skipped; explicit symlink inputs are
rejected. Inputs use canonical filesystem paths for configuration lookup and
diagnostics. An empty selection succeeds and reports zero checked files.

## Rule details and fix safety

- Assignments include `let`, `let ... else`, `=`, and all compound assignments.
- Calls include semicolon-terminated function calls, method calls, and macro
  expressions, including parentheses, `?`, and `.await` wrappers.
- Statement macros count as calls, including brace macros without a semicolon.
- Tail function-call expressions, `return`, local items, and control-flow
  statements use the ordinary separation rule.
- Nested Rust blocks are checked, including closures, async blocks, and branches.
- Macro token bodies remain opaque. The tool checks syntax available before macro
  expansion and does not inspect code inside strings.
- Statements sharing a physical line are left to `rustfmt` to lay out first.
- Blank lines inside block comments do not satisfy the spacing rule. Fixes preserve
  comment and literal contents, attributes, existing whitespace, line endings, and
  the presence or absence of a final newline.

Fix parses and validates all selected files and their configurations before writing
any changes. Each changed file is replaced through a temporary file in the same
directory, preserving its basic permissions. An unchanged file is left alone.
The tool checks for intervening source changes before replacing a file. A later
write failure can leave earlier files fixed; a multi-file fix is not a transaction.
File replacement does not promise preservation of hard-link relationships or
extended filesystem metadata.

## Development

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked -- check
cargo package --locked
```

CI runs formatting, Clippy, tests, self-linting, and package verification on Windows,
Linux, and macOS. A separate job checks the minimum supported Rust version.

The library exposes `lint`, `SpacingOptions`, `Outcome`, and `Diagnostic` for
in-memory use. See their Rust documentation for the source transformation contract.

Extracted from the Rust spacing checker developed for
[dump.txt](https://github.com/visionsofparadise/dump-txt). Licensed under MIT.
