mod config;

use clap::{Parser, Subcommand};
use config::{DEFAULT_CONFIG, Resolver};
use ignore::WalkBuilder;
use std::{collections::BTreeSet, fs, io::Write, path::PathBuf, process::ExitCode};
use styling_lint::lint;

#[derive(Parser)]
#[command(
    version,
    about = "Check and fix blank lines between Rust statement groups"
)]
struct Cli {
    #[arg(long, global = true, value_name = "FILE", conflicts_with = "no_config")]
    /// Use this TOML configuration for every input file.
    config: Option<PathBuf>,
    #[arg(long, global = true)]
    /// Use built-in defaults without searching for configuration files.
    no_config: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Report missing blank lines without changing files (exit 1 for findings).
    Check {
        /// Rust files or directories to scan recursively (default: current directory).
        paths: Vec<PathBuf>,
    },
    /// Insert missing blank lines in place.
    Fix {
        /// Rust files or directories to scan recursively (default: current directory).
        paths: Vec<PathBuf>,
    },
    /// Print the default TOML configuration to stdout.
    Config,
}

fn discover(paths: Vec<PathBuf>) -> Result<BTreeSet<PathBuf>, String> {
    let paths = if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths
    };
    let mut files = BTreeSet::new();

    for path in paths {
        let metadata =
            fs::symlink_metadata(&path).map_err(|error| format!("{}: {error}", path.display()))?;

        if metadata.file_type().is_symlink() {
            return Err(format!(
                "{}: symbolic link inputs are unsupported",
                path.display()
            ));
        }

        let path =
            fs::canonicalize(&path).map_err(|error| format!("{}: {error}", path.display()))?;

        if metadata.is_file() {
            if path.extension().is_none_or(|extension| extension != "rs") {
                return Err(format!(
                    "{}: expected a .rs file or directory",
                    path.display()
                ));
            }

            files.insert(path);
        } else if metadata.is_dir() {
            let walker = WalkBuilder::new(&path)
                .hidden(false)
                .follow_links(false)
                .require_git(false)
                .git_global(false)
                .filter_entry(|entry| entry.file_name() != ".git" && entry.file_name() != "target")
                .build();

            for entry in walker {
                let entry = entry.map_err(|error| error.to_string())?;

                if let Some(error) = entry.error() {
                    return Err(error.to_string());
                }

                if entry.file_type().is_some_and(|kind| kind.is_file())
                    && entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "rs")
                {
                    files.insert(entry.into_path());
                }
            }
        } else {
            return Err(format!(
                "{}: expected a regular file or directory",
                path.display()
            ));
        }
    }

    Ok(files)
}

struct Change {
    path: PathBuf,
    original: String,
    output: String,
}

fn write_change(change: Change) -> Result<(), String> {
    let path = &change.path;
    let write = || -> Result<(), Box<dyn std::error::Error>> {
        let metadata = fs::symlink_metadata(path)?;

        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("file type changed during linting".into());
        }

        if fs::read_to_string(path)? != change.original {
            return Err("file changed during linting; run fix again".into());
        }

        if metadata.permissions().readonly() {
            return Err("file is read-only".into());
        }

        let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;

        temporary.write_all(change.output.as_bytes())?;
        temporary
            .as_file()
            .set_permissions(metadata.permissions())?;
        temporary.as_file().sync_all()?;
        temporary.persist(path)?;

        Ok(())
    };

    write().map_err(|error| format!("{}: {error}", path.display()))
}

fn run(cli: Cli) -> Result<u8, String> {
    let (paths, fixing) = match cli.command {
        Command::Check { paths } => (paths, false),
        Command::Fix { paths } => (paths, true),
        Command::Config => {
            print!("{DEFAULT_CONFIG}");

            return Ok(0);
        }
    };
    let mut resolver = Resolver::new(cli.config.as_deref(), cli.no_config)?;
    let files = discover(paths)?;
    let mut changes = Vec::new();
    let mut diagnostics = Vec::new();
    let mut checked = 0;

    for path in files {
        let config = resolver.for_file(&path)?;

        if config.excludes(&path) {
            continue;
        }

        let source =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let outcome = lint(&source, &config.spacing).map_err(|error| {
            let start = error.span().start();

            format!(
                "{}:{}:{}: {error}",
                path.display(),
                start.line,
                start.column + 1
            )
        })?;
        checked += 1;

        if outcome.diagnostics.is_empty() {
            continue;
        }

        for diagnostic in outcome.diagnostics {
            diagnostics.push(format!(
                "{}:{}:{}: statement-spacing: expected at least {} blank line(s) between statement groups, found {}",
                path.display(), diagnostic.line, diagnostic.column, diagnostic.required, diagnostic.found
            ));
        }

        if fixing {
            let metadata =
                fs::metadata(&path).map_err(|error| format!("{}: {error}", path.display()))?;

            if metadata.permissions().readonly() {
                return Err(format!("{}: file is read-only", path.display()));
            }

            changes.push(Change {
                path,
                original: source,
                output: outcome.output,
            });
        }
    }

    let findings = diagnostics.len();

    if fixing {
        let count = changes.len();

        for change in changes {
            write_change(change)?;
        }

        eprintln!(
            "Checked {checked} Rust file(s); fixed {findings} spacing issue(s) in {count} file(s)."
        );
    } else {
        for diagnostic in diagnostics {
            println!("{diagnostic}");
        }

        eprintln!("Checked {checked} Rust file(s); found {findings} spacing issue(s).");
    }

    Ok(u8::from(findings > 0 && !fixing))
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("styling-lint: {error}");

            ExitCode::from(2)
        }
    }
}
