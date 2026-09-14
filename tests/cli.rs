//! The binary run as the launcher would, short of drawing on a terminal:
//! `--check` stops once it knows what it would play.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;

/// Runs git in `dir`, away from the user's own git config.
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A repository at `dir` with a commit per subject.
fn repository(dir: &Path, subjects: &[&str]) -> PathBuf {
    fs::create_dir_all(dir).expect("repository directory");
    git(dir, &["init", "-q"]);
    for (i, subject) in subjects.iter().enumerate() {
        fs::write(dir.join("lib.rs"), format!("fn step_{i}() {{}}\n")).expect("source file");
        git(dir, &["add", "."]);
        git(dir, &["commit", "-q", "-m", subject]);
    }
    dir.to_path_buf()
}

/// A config file in `dir` listing `repos`, plus `extra` lines.
fn config(dir: &Path, repos: &[&Path], extra: &str) -> PathBuf {
    let quoted: Vec<String> = repos
        .iter()
        .map(|repo| format!("\"{}\"", repo.display()))
        .collect();
    let path = dir.join("config.toml");
    fs::write(&path, format!("repos = [{}]\n{extra}", quoted.join(", "))).expect("config file");
    path
}

fn check(config: &Path, seed: u64) -> Output {
    Command::new(env!("CARGO_BIN_EXE_commit-screensaver"))
        .arg("--config")
        .arg(config)
        .args(["--check", "--seed", &seed.to_string()])
        .output()
        .expect("the binary runs")
}

/// Standard output of a run that succeeded.
fn stdout(output: &Output) -> String {
    assert!(output.status.success(), "{output:?}");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Standard error of a run that failed.
fn stderr(output: &Output) -> String {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The value of `key: ` in `--check` output.
fn field<'a>(output: &'a str, key: &str) -> &'a str {
    output
        .lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix(": "))
        .unwrap_or_else(|| panic!("no {key} in {output}"))
}

#[test]
fn check_names_the_repository_theme_and_a_commit_from_it() {
    let dir = TempDir::new().unwrap();
    let repo = repository(&dir.path().join("repo"), &["Add the parser", "Fix a typo"]);
    let output = stdout(&check(&config(dir.path(), &[&repo], "theme = \"nord\""), 1));

    assert_eq!(field(&output, "repo"), repo.display().to_string());
    assert_eq!(field(&output, "theme"), "nord");
    let (hash, subject) = field(&output, "commit").split_once(' ').unwrap();
    assert!(
        hash.len() == 7 && hash.chars().all(|c| c.is_ascii_hexdigit()),
        "{hash}"
    );
    assert!(
        ["Add the parser", "Fix a typo"].contains(&subject),
        "{subject}"
    );
}

#[test]
fn the_config_is_found_in_xdg_config_home() {
    let dir = TempDir::new().unwrap();
    let repo = repository(&dir.path().join("repo"), &["Only"]);
    let app_dir = dir.path().join("commit-screensaver");
    fs::create_dir(&app_dir).unwrap();
    config(&app_dir, &[&repo], "");

    let output = Command::new(env!("CARGO_BIN_EXE_commit-screensaver"))
        .arg("--check")
        .env("XDG_CONFIG_HOME", dir.path())
        .output()
        .unwrap();
    assert_eq!(field(&stdout(&output), "repo"), repo.display().to_string());
}

#[test]
fn seeds_pick_among_the_repositories() {
    let dir = TempDir::new().unwrap();
    let a = repository(&dir.path().join("a"), &["In a"]);
    let b = repository(&dir.path().join("b"), &["In b"]);
    let config = config(dir.path(), &[&a, &b], "");

    let picked: HashSet<String> = (0..30)
        .map(|seed| field(&stdout(&check(&config, seed)), "repo").to_owned())
        .collect();
    assert_eq!(picked.len(), 2, "{picked:?}");
    assert_eq!(stdout(&check(&config, 7)), stdout(&check(&config, 7)));
}

#[test]
fn repositories_that_cannot_be_replayed_are_passed_over() {
    let dir = TempDir::new().unwrap();
    let good = repository(&dir.path().join("good"), &["Works"]);
    let empty = repository(&dir.path().join("empty"), &[]);
    let config = config(
        dir.path(),
        &[&empty, &dir.path().join("missing"), &good],
        "",
    );
    for seed in 0..10 {
        assert_eq!(
            field(&stdout(&check(&config, seed)), "repo"),
            good.display().to_string()
        );
    }
}

#[test]
fn with_no_repository_to_replay_it_says_why_for_each() {
    let dir = TempDir::new().unwrap();
    let empty = repository(&dir.path().join("empty"), &[]);
    let missing = dir.path().join("missing");
    let config = config(dir.path(), &[&empty, &missing], "");

    let error = stderr(&check(&config, 1));
    assert!(
        error.starts_with(&format!(
            "commit-screensaver: no repository in {} can be replayed:\n",
            config.display()
        )),
        "{error}"
    );
    for repo in [&empty, &missing] {
        assert!(
            error.contains(&format!("\n  {}: ", repo.display())),
            "{error}"
        );
    }
}

#[test]
fn a_bad_config_is_an_error_naming_the_file() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("missing.toml");
    assert!(stderr(&check(&missing, 1)).contains(&missing.display().to_string()));

    let repo = repository(&dir.path().join("repo"), &["Only"]);
    let typo = config(dir.path(), &[&repo], "sped = 3");
    let error = stderr(&check(&typo, 1));
    assert!(
        error.contains(&typo.display().to_string()) && error.contains("sped"),
        "{error}"
    );
}

#[test]
fn an_unknown_theme_is_an_error() {
    let dir = TempDir::new().unwrap();
    let repo = repository(&dir.path().join("repo"), &["Only"]);
    let config = config(dir.path(), &[&repo], "theme = \"no-such-theme\"");
    assert!(stderr(&check(&config, 1)).contains("no-such-theme"));
}
