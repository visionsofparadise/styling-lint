# styling-lint

Check and fix blank lines between Rust statement groups. Use alongside `rustfmt`.

By default, consecutive assignments stay together and consecutive calls stay
together. Other statement pairs require a blank line. Existing blank lines remain.

```rust
let first = read();
let second = read();

process(first);
process(second);

finish()
```

## Install

Requires Rust 1.85 or later.

```sh
cargo install styling-lint --locked
```

## Usage

```sh
styling-lint check
styling-lint fix
styling-lint check src tests
styling-lint fix src/main.rs
styling-lint config > styling-lint.toml
```

Paths default to the current directory. Directories are scanned recursively for
`.rs` files, respecting `.gitignore` and `.ignore` and skipping `.git` and `target`.
Explicit files bypass traversal ignores but still respect configuration exclusions.

Run `cargo fmt` first. Same-line statements and macro bodies are left unchanged.
Fixes preserve comments, strings, and line endings. A fix stops on errors; a write
failure can leave earlier files fixed.

Exit codes: **0** success, **1** check findings, **2** argument, configuration,
syntax, or filesystem error. Check diagnostics include file, line, and column.

## Configuration

Each Rust file uses the nearest `styling-lint.toml` or `.styling-lint.toml` in its
directory or ancestors. Use one filename per directory. A nested configuration
replaces its parent's settings; omitted options use defaults.

```toml
exclude = []

[spacing]
min-blank-lines = 1
group-assignments = true
group-calls = true
```

`min-blank-lines` accepts 1–10. Set either grouping option to `false` to require
spacing between those consecutive statements. Assignments include `let`, `=`, and
compound assignments; calls include semicolon-terminated calls, including `?` and
`.await`. Tail call expressions require separation.

Exclusions are relative to the configuration file and use forward-slash globs,
such as `exclude = ["generated/**", "**/*.generated.rs"]`.

```sh
styling-lint check --config path/to/config.toml
styling-lint check --no-config
```

`--config` applies one file to every input. `--no-config` uses built-in defaults.

MIT licensed.
