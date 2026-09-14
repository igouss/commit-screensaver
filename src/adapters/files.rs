//! Where the config file lives, and reading it.

use std::{
    env,
    error::Error,
    ffi::OsString,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use crate::domain::config::{Config, ConfigError};

/// `$XDG_CONFIG_HOME/commit-screensaver/config.toml`.
#[must_use]
pub fn default_config() -> PathBuf {
    config_dir(env::var_os("XDG_CONFIG_HOME"), env::var_os("HOME")).join("config.toml")
}

/// `<base>/commit-screensaver`, where base is `$XDG_CONFIG_HOME` if it's set
/// to an absolute path (as the spec requires), else `$HOME/.config`.
fn config_dir(xdg: Option<OsString>, home: Option<OsString>) -> PathBuf {
    xdg.map(PathBuf::from)
        .filter(|base| base.is_absolute())
        .unwrap_or_else(|| PathBuf::from(home.unwrap_or_default()).join(".config"))
        .join("commit-screensaver")
}

#[derive(Debug)]
pub enum LoadError {
    Read(PathBuf, io::Error),
    Invalid(PathBuf, ConfigError),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(path, error) => write!(f, "can't read {}: {error}", path.display()),
            Self::Invalid(path, error) => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl Error for LoadError {}

/// Reads the config file at `path`.
///
/// # Errors
///
/// If it can't be read, or isn't a valid config.
pub fn load_config(path: &Path) -> Result<Config, LoadError> {
    let text = fs::read_to_string(path).map_err(|error| LoadError::Read(path.into(), error))?;
    let dir = path.parent().unwrap_or_else(|| Path::new(""));
    let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    Config::parse(&text, dir, &home).map_err(|error| LoadError::Invalid(path.into(), error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_config_is_named_for_the_app() {
        assert!(default_config().ends_with("commit-screensaver/config.toml"));
    }

    #[test]
    fn xdg_config_home_wins_over_home() {
        assert_eq!(
            config_dir(Some("/xdg".into()), Some("/home/u".into())),
            Path::new("/xdg/commit-screensaver")
        );
    }

    #[test]
    fn unset_empty_or_relative_xdg_config_home_falls_back_to_home() {
        for xdg in [None, Some("".into()), Some("relative".into())] {
            assert_eq!(
                config_dir(xdg, Some("/home/u".into())),
                Path::new("/home/u/.config/commit-screensaver")
            );
        }
    }

    #[test]
    fn relative_repositories_are_found_next_to_the_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "repos = [\"here\"]\n").unwrap();
        assert_eq!(load_config(&path).unwrap().repos, [dir.path().join("here")]);
    }

    #[test]
    fn errors_name_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.toml");
        let error = load_config(&missing).unwrap_err();
        assert!(matches!(error, LoadError::Read(..)));
        assert!(
            error
                .to_string()
                .starts_with(&format!("can't read {}: ", missing.display()))
        );

        let invalid = dir.path().join("invalid.toml");
        fs::write(&invalid, "repos = []\n").unwrap();
        assert_eq!(
            load_config(&invalid).unwrap_err().to_string(),
            format!("{}: `repos` lists no repositories", invalid.display())
        );
    }
}
