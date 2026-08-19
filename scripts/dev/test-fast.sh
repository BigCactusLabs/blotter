#!/bin/sh
set -eu

if cargo nextest --version >/dev/null 2>&1; then
    exec cargo nextest run --all-features "$@"
fi

echo "cargo-nextest is unavailable; falling back to cargo test --all-features" >&2
exec cargo test --all-features "$@"
