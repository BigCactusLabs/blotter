#!/bin/sh
# Run the required test gate five times, the way AGENTS.md asks for after a store.rs
# or concurrency-adjacent change. A single green run proves nothing about races.
#
# Each run's full output is kept, so a failure that does not reproduce still names the
# test that failed. Counting "ok" lines and throwing the rest away loses exactly the
# information the fifth run exists to capture.
#
# Usage: scripts/dev/gate-5x.sh [extra cargo test args...]
set -eu

runs=${GATE_RUNS:-5}
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$repo_root"

log_dir=${GATE_LOG_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/blotter-gate-XXXXXX")}
mkdir -p "$log_dir"

failed=0
for i in $(seq 1 "$runs"); do
    log_file="$log_dir/run-$i.log"
    printf 'run %s/%s ... ' "$i" "$runs"

    if cargo test --all-features "$@" >"$log_file" 2>&1; then
        printf 'ok (%s)\n' "$(grep -c '^test .* \.\.\. ok$' "$log_file" || true)"
        continue
    fi

    failed=$((failed + 1))
    printf 'FAILED\n'
    echo "  log: $log_file"
    # `cargo test` prints the roster under "failures:" twice; the bare-name block is the
    # second one. Print both the assertion output and the names so a flake is identifiable.
    sed -n '/^failures:$/,/^test result:/p' "$log_file" | sed 's/^/  /'
done

if [ "$failed" -ne 0 ]; then
    echo "gate-5x: $failed of $runs runs failed; logs in $log_dir" >&2
    exit 1
fi

echo "gate-5x: $runs/$runs runs passed; logs in $log_dir"
