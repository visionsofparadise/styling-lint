use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use styling_lint::SpacingOptions;

pub const DEFAULT_CONFIG: &str = r#"# Paths are relative to this configuration file. Use forward slashes in globs.
exclude = []

[spacing]
min-blank-lines = 1
group-assignments = true
group-calls = true
"#;

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct Config {
    exclude: Vec<String>,
    spacing: SpacingOptions,
}

pub struct ResolvedConfig {
    pub spacing: SpacingOptions,
    base: PathBuf,
    exclude: GlobSet,
}

impl ResolvedConfig {
    fn new(config: Config, base: PathBuf) -> Result<Self, String> {
        if !(1..=10).contains(&config.spacing.min_blank_lines) {
            return Err("spacing.min-blank-lines must be between 1 and 10".into());
        }

        let mut builder = GlobSetBuilder::new();

        for pattern in config.exclude {
            let glob = GlobBuilder::new(&pattern)
                .literal_separator(true)
                .backslash_escape(false)
                .build()
                .map_err(|error| format!("invalid exclude glob {pattern:?}: {error}"))?;

            builder.add(glob);
        }

        Ok(Self {
            spacing: config.spacing,
            base,
            exclude: builder.build().map_err(|error| error.to_string())?,
        })
    }

    pub fn excludes(&self, path: &Path) -> bool {
        path.strip_prefix(&self.base)
            .is_ok_and(|relative| self.exclude.is_match(relative))
    }
}

pub struct Resolver {
    fixed: Option<Arc<ResolvedConfig>>,
    cache: HashMap<PathBuf, Arc<ResolvedConfig>>,
    defaults: Arc<ResolvedConfig>,
}

impl Resolver {
    pub fn new(explicit: Option<&Path>, no_config: bool) -> Result<Self, String> {
        let defaults = Arc::new(ResolvedConfig::new(Config::default(), PathBuf::new())?);
        let fixed = if let Some(path) = explicit {
            Some(Self::load(path)?)
        } else if no_config {
            Some(defaults.clone())
        } else {
            None
        };

        Ok(Self {
            fixed,
            cache: HashMap::new(),
            defaults,
        })
    }

    fn load(path: &Path) -> Result<Arc<ResolvedConfig>, String> {
        let path =
            fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let text =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let config: Config =
            toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;

        ResolvedConfig::new(config, path.parent().unwrap().to_path_buf())
            .map(Arc::new)
            .map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn for_file(&mut self, file: &Path) -> Result<Arc<ResolvedConfig>, String> {
        if let Some(config) = &self.fixed {
            return Ok(config.clone());
        }

        let mut visited = Vec::new();
        let mut result = self.defaults.clone();

        for directory in file.parent().unwrap().ancestors() {
            if let Some(config) = self.cache.get(directory) {
                result = config.clone();

                break;
            }

            visited.push(directory.to_path_buf());

            let visible = directory.join("styling-lint.toml");
            let hidden = directory.join(".styling-lint.toml");
            let visible_exists = visible
                .try_exists()
                .map_err(|error| format!("{}: {error}", visible.display()))?;
            let hidden_exists = hidden
                .try_exists()
                .map_err(|error| format!("{}: {error}", hidden.display()))?;

            match (visible_exists, hidden_exists) {
                (true, true) => {
                    return Err(format!(
                        "{}: both styling-lint.toml and .styling-lint.toml exist; keep one configuration file",
                        directory.display()
                    ));
                }
                (true, false) => {
                    result = Self::load(&visible)?;

                    break;
                }
                (false, true) => {
                    result = Self::load(&hidden)?;

                    break;
                }
                (false, false) => {}
            }
        }

        for directory in visited {
            self.cache.insert(directory, result.clone());
        }

        Ok(result)
    }
}
