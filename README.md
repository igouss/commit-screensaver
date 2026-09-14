# commit-screensaver

A screensaver for Omarchy that replays Git history. Each time it starts it picks
a random repository from its config file and types out random commits from it
with [gitlogue](https://github.com/unhappychoice/gitlogue)'s editor animation:
file tree, syntax-highlighted edits, terminal pane. Any key, click or real mouse
movement dismisses it.

gitlogue's UI isn't part of its library, so this builds against a fork,
[igouss/gitlogue](https://github.com/igouss/gitlogue) (branch `embed`). The
fork exposes the UI as a library, lets the embedding program decide what
input does, and leaves signal handling to it.

## Install

    ./install.sh

This builds `~/.local/bin/commit-screensaver` (release profile: fat LTO, one
codegen unit, `target-cpu=native`) and links this checkout's `config.toml` to
`~/.config/commit-screensaver/config.toml`.

The idle screensaver starts it through
[shader-screensaver](https://github.com/igouss/shader-screensaver)'s launcher
override, according to `mode` in that repo's `settings.conf`:

- `shaders`: GLSL shaders only.
- `commits`: this, in a fullscreen `foot` window on each monitor.
- `mixed`: one of the two, picked at random each time.

## Config

```toml
repos = ["~/Projects/shader-screensaver", "~/src/linux"]  # picked at random
theme = "tokyo-night"   # `gitlogue theme list`
speed = 30              # milliseconds per typed character
```

Repositories that won't open, or have no commit gitlogue can replay, are passed
over.

## Running it by hand

    commit-screensaver           # in any terminal; any input stops it
    commit-screensaver --check   # print the repository, theme and first commit, then exit

`--app-id ID` runs it as the screensaver, in a terminal window with that app
id. It hides the pointer, stops once no such window has focus (or the session
locks), and takes the other monitors' instances down with it. `--seed` fixes
which repository it picks.

As in the shader screensaver, keys and clicks count only after 0.5 s and mouse
movement only after 1.5 s, so the tail of whatever woke the session doesn't
dismiss it. Mouse movement must cover more than one terminal cell, and focus
must stay lost for 1.5 s.

## Layout

A light hexagonal split:

- `src/domain/` holds the rules, free of I/O: parsing the config, picking the
  repository, which input dismisses the screensaver, and the grace period on
  focus loss.
- `src/ports.rs` defines `Player` (shows the replay) and `Presence` (should
  the screensaver keep running?).
- `src/app.rs` runs a session: the replay, plus a thread watching presence.
- `src/adapters/` talks to the outside world: the config file, Git and the UI
  through gitlogue, Hyprland (focus, lock, pointer), and sibling instances.

## Tests

    cargo test                                  # unit, property and CLI tests
    cargo clippy --all-targets -- -D warnings   # pedantic + nursery
    cargo mutants                               # see .cargo/mutants.toml for what's left out
