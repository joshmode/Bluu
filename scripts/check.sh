#!/bin/sh
set -eu

echo "== format =="
cargo fmt --all -- --check

echo "== host tests =="
cargo test --workspace

echo "== host clippy =="
cargo clippy --workspace --all-targets -- -D warnings

if command -v rustup >/dev/null 2>&1 && rustup target list --installed 2>/dev/null | grep -qx 'x86_64-pc-windows-msvc'; then
  echo "== windows type/check =="
  cargo check --workspace --target x86_64-pc-windows-msvc
  cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings
elif [ -x .tooling/cargo/bin/cargo ] && [ -d .tooling/rustup ]; then
  echo "== windows type/check (local toolchain) =="
  RUSTUP_HOME="$PWD/.tooling/rustup" CARGO_HOME="$PWD/.tooling/cargo" \
    .tooling/cargo/bin/cargo check --workspace --target x86_64-pc-windows-msvc
  RUSTUP_HOME="$PWD/.tooling/rustup" CARGO_HOME="$PWD/.tooling/cargo" \
    .tooling/cargo/bin/cargo clippy --workspace --all-targets \
    --target x86_64-pc-windows-msvc -- -D warnings
else
  echo "windows target not installed; skipping cross-target check"
fi

echo "== screenshots =="
cargo test -p bluu-platform screenshot

echo "all checks completed"
