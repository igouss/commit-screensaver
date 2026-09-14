//! Repositories, through gitlogue's Git model.

use std::path::Path;

use gitlogue::git::{CommitMetadata, GitRepository};

/// A repository ready to replay, and the first commit to show.
pub struct Opened {
    pub repo: GitRepository,
    pub first: CommitMetadata,
}

/// Opens the repository at `path` and picks a random commit from it.
///
/// # Errors
///
/// If `path` isn't a repository, or has no commit gitlogue can replay.
pub fn open(path: &Path) -> Result<Opened, String> {
    let repo = GitRepository::open(path).map_err(|error| format!("{error:#}"))?;
    let first = repo.random_commit().map_err(|error| format!("{error:#}"))?;
    Ok(Opened { repo, first })
}

/// A commit's short hash and subject line.
#[must_use]
pub fn describe(commit: &CommitMetadata) -> String {
    let hash = commit.hash.get(..7).unwrap_or(&commit.hash);
    let subject = commit.message.lines().next().unwrap_or_default();
    format!("{hash} {subject}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command};

    /// Runs git in `dir`, away from the user's own git config.
    fn git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}");
        String::from_utf8(output.stdout).unwrap()
    }

    fn repository(subjects: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        for (i, subject) in subjects.iter().enumerate() {
            fs::write(dir.path().join("lib.rs"), format!("fn step_{i}() {{}}\n")).unwrap();
            git(dir.path(), &["add", "."]);
            git(dir.path(), &["commit", "-q", "-m", subject]);
        }
        dir
    }

    #[test]
    fn opens_a_repository_at_one_of_its_commits() {
        let dir = repository(&["one", "two", "three"]);
        let hashes = git(dir.path(), &["log", "--format=%H"]);
        let opened = open(dir.path()).unwrap();
        assert!(hashes.lines().any(|hash| hash == opened.first.hash));
        assert!(!opened.first.changes.is_empty());
    }

    #[test]
    fn a_directory_that_is_no_repository_fails() {
        let dir = tempfile::tempdir().unwrap();
        assert_ne!(open(dir.path()).err().unwrap(), "");
    }

    #[test]
    fn a_repository_without_commits_fails() {
        let dir = repository(&[]);
        assert_ne!(open(dir.path()).err().unwrap(), "");
    }

    #[test]
    fn commits_are_described_by_short_hash_and_subject() {
        let dir = repository(&["Add the parser\n\nWith a body."]);
        let commit = open(dir.path()).unwrap().first;
        assert_eq!(
            describe(&commit),
            format!("{} Add the parser", &commit.hash[..7])
        );

        let short = CommitMetadata {
            hash: "abc".to_owned(),
            message: String::new(),
            ..commit
        };
        assert_eq!(describe(&short), "abc ");
    }
}
