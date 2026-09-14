//! A screensaver that replays random commits from the Git repositories in its
//! config file with gitlogue's editor animation. With `--app-id` it acts as a
//! screensaver in a fullscreen terminal window.

mod adapters;
mod app;
mod domain;
mod ports;

use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use clap::Parser;
use gitlogue::theme::Theme;

use crate::{
    adapters::{
        files, git,
        hyprland::{HiddenPointer, Hyprland},
        player::Replay,
        siblings,
    },
    domain::pick,
};

/// How often the screensaver asks Hyprland whether it still has focus.
const PRESENCE_POLL: Duration = Duration::from_millis(250);

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Config file [default: $XDG_CONFIG_HOME/commit-screensaver/config.toml]
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Run as the screensaver, in a terminal window with this app id: hide the
    /// pointer, stop once no such window has focus or the session locks, and
    /// stop the other monitors' instances along with this one
    #[arg(long, value_name = "ID")]
    app_id: Option<String>,

    /// Seed for picking the repository
    #[arg(long)]
    seed: Option<u64>,

    /// Print the repository, theme and first commit that would play, then exit
    #[arg(long)]
    check: bool,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("commit-screensaver: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let config_path = cli.config.unwrap_or_else(files::default_config);
    let config = files::load_config(&config_path).map_err(|error| error.to_string())?;
    let mut rng = cli
        .seed
        .map_or_else(fastrand::Rng::new, fastrand::Rng::with_seed);
    let (repo_path, opened) = pick::open_random(&config.repos, &mut rng, |path| {
        git::open(path).map(|opened| (path.clone(), opened))
    })
    .map_err(|failures| unusable(&config_path, &failures))?;
    let theme = Theme::load(&config.theme).map_err(|error| format!("{error:#}"))?;

    if cli.check {
        println!("repo: {}", repo_path.display());
        println!("theme: {}", config.theme);
        println!("commit: {}", git::describe(&opened.first));
        return Ok(());
    }

    let stop = Arc::new(AtomicBool::new(false));
    let on_signal = Arc::clone(&stop);
    ctrlc::set_handler(move || on_signal.store(true, Ordering::Relaxed))
        .map_err(|error| format!("can't handle signals: {error}"))?;

    let screensaver = cli.app_id.as_deref();
    let pointer = screensaver.map(|_| HiddenPointer::new());
    let replay = Replay {
        repo: &opened.repo,
        first: opened.first,
        theme,
        speed_ms: config.speed_ms,
    };
    let started = Instant::now();
    let stopped = app::run(
        replay,
        screensaver.map(Hyprland::new),
        &stop,
        || started.elapsed(),
        || thread::sleep(PRESENCE_POLL),
    );
    drop(pointer);

    match (screensaver, stopped?) {
        // Dismissing one monitor's screensaver dismisses them all. Those
        // stopped by a signal (likely from a sibling) leave the rest be.
        (Some(_), Some(_)) => siblings::terminate_siblings(),
        (None, Some(reason)) => eprintln!("stopped: {reason}"),
        (_, None) => {}
    }
    Ok(())
}

/// Why no repository in the config at `config` can be replayed.
fn unusable(config: &Path, failures: &[(&PathBuf, String)]) -> String {
    let mut message = format!("no repository in {} can be replayed:", config.display());
    for (repo, why) in failures {
        let _ = write!(message, "\n  {}: {why}", repo.display());
    }
    message
}
