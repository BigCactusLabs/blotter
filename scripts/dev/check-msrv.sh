#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$repo_root"

msrv=$(sed -n 's/^rust-version = "\([^"]*\)"/\1/p' Cargo.toml)
if [ -z "$msrv" ]; then
    echo "Cargo.toml does not declare rust-version" >&2
    exit 1
fi

case "$msrv" in
    *.*.*) toolchain=$msrv ;;
    *.*) toolchain=$msrv.0 ;;
    *)
        echo "unsupported rust-version format: $msrv" >&2
        exit 1
        ;;
esac

if ! rustup run "$toolchain" rustc --version >/dev/null 2>&1; then
    echo "Rust $toolchain is not installed; run: rustup toolchain install $toolchain --profile minimal" >&2
    exit 1
fi

exec rustup run "$toolchain" cargo test --locked --all-features "$@"
