//! The config file: which repositories to replay, and how.

use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use serde::Deserialize;

/// gitlogue's own default theme...
const DEFAULT_THEME: &str = "tokyo-night";
/// ...and typing speed, in milliseconds per character.
const DEFAULT_SPEED_MS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The repositories to pick from.
    pub repos: Vec<PathBuf>,
    /// A gitlogue theme name.
    pub theme: String,
    /// Milliseconds per typed character.
    pub speed_ms: u64,
}

/// The file as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    repos: Vec<String>,
    theme: Option<String>,
    speed: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Not TOML, or not the keys this file takes.
    Invalid(String),
    NoRepos,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => f.write_str(message.trim_end()),
            Self::NoRepos => f.write_str("`repos` lists no repositories"),
        }
    }
}

impl Error for ConfigError {}

impl Config {
    /// Parses a config file's `text`. A repository path may start with `~`
    /// for `home`; a relative one is taken from `dir`, the file's directory.
    ///
    /// # Errors
    ///
    /// If `text` isn't a valid config file, or lists no repositories.
    pub fn parse(text: &str, dir: &Path, home: &Path) -> Result<Self, ConfigError> {
        let file: File =
            toml::from_str(text).map_err(|error| ConfigError::Invalid(error.to_string()))?;
        if file.repos.is_empty() {
            return Err(ConfigError::NoRepos);
        }
        Ok(Self {
            repos: file
                .repos
                .iter()
                .map(|repo| resolve(repo, dir, home))
                .collect(),
            theme: file.theme.unwrap_or_else(|| DEFAULT_THEME.to_owned()),
            speed_ms: file.speed.unwrap_or(DEFAULT_SPEED_MS),
        })
    }
}

/// `path`, with a leading `~` meaning `home`, relative to `dir`.
fn resolve(path: &str, dir: &Path, home: &Path) -> PathBuf {
    match path.strip_prefix('~') {
        Some("") => home.to_path_buf(),
        Some(rest) if rest.starts_with('/') => home.join(rest.trim_start_matches('/')),
        _ => dir.join(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::collection::vec;
    use proptest::prelude::*;

    const DIR: &str = "/etc/cs";
    const HOME: &str = "/home/u";

    fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::parse(text, Path::new(DIR), Path::new(HOME))
    }

    #[test]
    fn reads_every_key() {
        assert_eq!(
            parse("repos = [\"/src/a\"]\ntheme = \"nord\"\nspeed = 12\n"),
            Ok(Config {
                repos: vec![PathBuf::from("/src/a")],
                theme: "nord".to_owned(),
                speed_ms: 12,
            })
        );
    }

    #[test]
    fn theme_and_speed_default_to_gitlogues() {
        let config = parse("repos = [\"/src/a\"]").unwrap();
        assert_eq!(
            (config.theme.as_str(), config.speed_ms),
            ("tokyo-night", 30)
        );
    }

    #[test]
    fn repository_paths_expand_home_and_resolve_relative_ones() {
        let config = parse(r#"repos = ["~", "~/src/a", "~//b", "c", "/abs", "~d"]"#).unwrap();
        assert_eq!(
            config.repos,
            [
                "/home/u",
                "/home/u/src/a",
                "/home/u/b",
                "/etc/cs/c",
                "/abs",
                "/etc/cs/~d"
            ]
            .map(PathBuf::from)
        );
    }

    #[test]
    fn a_config_needs_repositories() {
        assert_eq!(parse("repos = []"), Err(ConfigError::NoRepos));
        assert!(
            matches!(parse("theme = \"nord\""), Err(ConfigError::Invalid(m)) if m.contains("repos"))
        );
    }

    #[test]
    fn unknown_keys_and_bad_values_are_invalid() {
        for text in [
            "repos = [\"/a\"]\nsped = 3",
            "repos = \"/a\"",
            "repos = [\"/a\"]\nspeed = -1",
            "repos = [",
        ] {
            assert!(
                matches!(parse(text), Err(ConfigError::Invalid(_))),
                "{text}"
            );
        }
    }

    #[test]
    fn errors_read_as_one_message() {
        assert_eq!(
            ConfigError::NoRepos.to_string(),
            "`repos` lists no repositories"
        );
        assert_eq!(
            ConfigError::Invalid("bad\n\n".to_owned()).to_string(),
            "bad"
        );
    }

    proptest! {
        #[test]
        fn every_repository_is_kept_in_order(names in vec("[a-z]{1,8}", 1..6)) {
            let quoted: Vec<String> = names.iter().map(|name| format!("\"~/{name}\"")).collect();
            let config = parse(&format!("repos = [{}]", quoted.join(", "))).unwrap();
            let expected: Vec<PathBuf> = names.iter().map(|name| Path::new(HOME).join(name)).collect();
            prop_assert_eq!(config.repos, expected);
        }
    }
}
