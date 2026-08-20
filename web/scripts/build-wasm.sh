#!/usr/bin/env bash
# Builds the renderer crate's wasm-bindgen bundle into web/public/wasm, so the Next.js app can
# dynamically import it as a static asset.
#
# A Homebrew-installed cargo/rustc ahead of rustup's on PATH silently lacks the
# wasm32-unknown-unknown std, failing with "can't find crate for `core`/`std`" (or, via
# wasm-pack, "It looks like Rustup is not being used" even though it is). `rustup run
# <toolchain> ...` does NOT reliably fix this -- the invoked binary can still resolve `cargo`/
# `rustc` from a plain PATH lookup instead of respecting rustup's env. Prepending the toolchain's
# own bin directory to PATH ourselves is the fix that actually works.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
out_dir="$script_dir/../public/wasm"

toolchain="$(rustup show active-toolchain 2>/dev/null | awk '{print $1}')"
if [ -z "$toolchain" ]; then
  echo "error: no active rustup toolchain found; run 'rustup default stable' first" >&2
  exit 1
fi

toolchain_bin="$HOME/.rustup/toolchains/$toolchain/bin"
if [ ! -d "$toolchain_bin" ]; then
  echo "error: rustup toolchain bin dir not found at $toolchain_bin" >&2
  exit 1
fi

export PATH="$toolchain_bin:$PATH"

if ! rustup target list --installed --toolchain "$toolchain" | grep -q wasm32-unknown-unknown; then
  echo "installing wasm32-unknown-unknown for $toolchain..."
  rustup target add wasm32-unknown-unknown --toolchain "$toolchain"
fi

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "error: wasm-pack is not installed; run 'cargo install wasm-pack'" >&2
  exit 1
fi

cd "$repo_root"
wasm-pack build --target web --out-dir "$out_dir" --out-name rust_renderer

echo "wasm bundle written to $out_dir"
