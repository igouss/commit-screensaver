#!/bin/bash

# Install commit-screensaver from this checkout: build it, and link the config
# into place.

set -euo pipefail

repo=$(cd "$(dirname "$0")" && pwd)

# Points $2 at $1. An existing file is kept as $2.orig.
link() {
  local target=$1 path=$2
  mkdir -p "$(dirname "$path")"
  if [[ -L $path ]]; then
    rm "$path"
  elif [[ -e $path ]]; then
    mv "$path" "$path.orig"
    echo "kept the previous $path as $path.orig"
  fi
  ln -s "$target" "$path"
  echo "linked $path -> $target"
}

cargo build --release --locked --manifest-path "$repo/Cargo.toml"
# install(1) replaces the file rather than rewriting it, so a running
# screensaver keeps its copy.
install -Dm755 "$repo/target/release/commit-screensaver" "$HOME/.local/bin/commit-screensaver"
echo "built ~/.local/bin/commit-screensaver"

link "$repo/config.toml" "${XDG_CONFIG_HOME:-$HOME/.config}/commit-screensaver/config.toml"
