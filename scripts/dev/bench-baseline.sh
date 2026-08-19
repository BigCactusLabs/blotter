#!/bin/sh
# Reproducible, build-excluded TASK-29.1 release measurements.
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$repo_root"

binary=${BLOTTER_BIN:-"$repo_root/target/release/blotter"}
fixture_dir="$repo_root/target/scale-fixtures"
output="$repo_root/target/scale-baseline-results.tsv"
runs=5
inner_runs=3
overwrite=false
time_bin=${TIME_BIN:-/usr/bin/time}
generator="$repo_root/scripts/dev/generate-scale-fixtures.py"

usage() {
    cat <<'EOF'
Usage: scripts/dev/bench-baseline.sh [options]

Runs the release binary only. Build fixtures and `cargo build --release` before
calling this script; neither operation is part of the measured samples.

Options:
  --bin PATH           release binary (default: target/release/blotter)
  --fixtures-dir PATH  generated fixture directory (default: target/scale-fixtures)
  --runs N             batches per command and fixture (default: 5; minimum: 3)
  --inner N            CLI invocations per timed batch (default: 3)
  --output PATH        TSV sample output (default: target/scale-baseline-results.tsv)
  --overwrite          replace an existing --output file
  --help               show this help

The reported wall and CPU times are per CLI invocation. Peak RSS is the peak
for one sequential batch and is not divided by --inner. `resolve` gets an
untimed disposable copy per invocation; canonical fixtures are rechecked after
the run and never changed.
EOF
}

fail() {
    echo "error: $*" >&2
    exit 1
}

require_positive_integer() {
    case "$2" in
        '' | *[!0-9]*) fail "$1 must be a positive integer" ;;
    esac
    [ "$2" -gt 0 ] || fail "$1 must be a positive integer"
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --bin)
            [ "$#" -ge 2 ] || fail "--bin requires PATH"
            binary=$2
            shift 2
            ;;
        --fixtures-dir)
            [ "$#" -ge 2 ] || fail "--fixtures-dir requires PATH"
            fixture_dir=$2
            shift 2
            ;;
        --runs)
            [ "$#" -ge 2 ] || fail "--runs requires N"
            runs=$2
            shift 2
            ;;
        --inner)
            [ "$#" -ge 2 ] || fail "--inner requires N"
            inner_runs=$2
            shift 2
            ;;
        --output)
            [ "$#" -ge 2 ] || fail "--output requires PATH"
            output=$2
            shift 2
            ;;
        --overwrite)
            overwrite=true
            shift
            ;;
        --help)
            usage
            exit 0
            ;;
        *)
            fail "unknown option: $1"
            ;;
    esac
done

require_positive_integer "--runs" "$runs"
require_positive_integer "--inner" "$inner_runs"
[ "$runs" -ge 3 ] || fail "--runs must be at least 3"
[ -x "$binary" ] || fail "release binary is not executable: $binary; run cargo build --release first"
[ -x "$time_bin" ] || fail "time binary is not executable: $time_bin"
command -v python3 >/dev/null 2>&1 || fail "python3 is required to verify generated fixtures"
[ -f "$generator" ] || fail "fixture generator is missing: $generator"
[ -f "$fixture_dir/scale-fixtures.env" ] || fail "fixture metadata is missing; run $generator --output-dir $fixture_dir first"

case "$(uname -s)" in
    Darwin) time_style=darwin ;;
    Linux) time_style=gnu ;;
    *) fail "unsupported time implementation on $(uname -s); use macOS or GNU /usr/bin/time" ;;
esac

if [ -e "$output" ] && [ "$overwrite" != true ]; then
    fail "refusing to overwrite $output; pass --overwrite after reviewing it"
fi
output_parent=$(dirname -- "$output")
[ -d "$output_parent" ] || mkdir -p "$output_parent"

# The generated file contains shell-quoted fixed values only. It is checked
# against the generator immediately before and after measurement.
# shellcheck disable=SC1090
. "$fixture_dir/scale-fixtures.env"
[ "${SCALE_FIXTURE_FORMAT:-}" = 1 ] || fail "unsupported fixture metadata format"
export SCALE_MEASUREMENT_NOW SCALE_DIGEST_SINCE SCALE_DUPLICATE_ADD_NOW
export SCALE_DUPLICATE_TEXT SCALE_DUPLICATE_AGENT SCALE_DUPLICATE_TAG_1 SCALE_DUPLICATE_TAG_2

python3 "$generator" --output-dir "$fixture_dir" --check

scratch_root=$(mktemp -d "${TMPDIR:-/tmp}/blotter-scale-baseline.XXXXXX")
echo "scratch resolve copies retained for inspection: $scratch_root" >&2

expected_exit() {
    case "$1" in
        list | digest | add_duplicate | resolve) echo 0 ;;
        triage | verify | doctor) echo 1 ;;
        *) fail "unknown benchmark command: $1" ;;
    esac
}

prepare_resolve_copies() {
    sample_dir=$1
    fixture=$2
    count=$3
    mkdir "$sample_dir"
    index=0
    while [ "$index" -lt "$count" ]; do
        cp "$fixture" "$sample_dir/resolve-$index.jsonl"
        index=$((index + 1))
    done
}

run_batch() {
    command_name=$1
    fixture=$2
    resolve_id=$3
    sample_dir=$4
    count=$5
    expected=$6
    time_file=$7

    export BENCH_BIN="$binary"
    export BENCH_COMMAND="$command_name"
    export BENCH_FIXTURE="$fixture"
    export BENCH_RESOLVE_ID="$resolve_id"
    export BENCH_SCRATCH="$sample_dir"
    export BENCH_INNER_RUNS="$count"
    export BENCH_EXPECTED_EXIT="$expected"

    set +e
    if [ "$time_style" = darwin ]; then
        "$time_bin" -l /bin/sh -c '
            index=0
            while [ "$index" -lt "$BENCH_INNER_RUNS" ]; do
                case "$BENCH_COMMAND" in
                    list)
                        BLOTTER_NOW="$SCALE_MEASUREMENT_NOW" "$BENCH_BIN" --file "$BENCH_FIXTURE" list >/dev/null
                        ;;
                    triage)
                        BLOTTER_NOW="$SCALE_MEASUREMENT_NOW" "$BENCH_BIN" --file "$BENCH_FIXTURE" triage >/dev/null
                        ;;
                    verify)
                        BLOTTER_NOW="$SCALE_MEASUREMENT_NOW" "$BENCH_BIN" --file "$BENCH_FIXTURE" verify >/dev/null
                        ;;
                    digest)
                        BLOTTER_NOW="$SCALE_MEASUREMENT_NOW" "$BENCH_BIN" --file "$BENCH_FIXTURE" digest --since "$SCALE_DIGEST_SINCE" >/dev/null
                        ;;
                    doctor)
                        BLOTTER_NOW="$SCALE_MEASUREMENT_NOW" "$BENCH_BIN" --file "$BENCH_FIXTURE" doctor >/dev/null
                        ;;
                    add_duplicate)
                        BLOTTER_NOW="$SCALE_DUPLICATE_ADD_NOW" "$BENCH_BIN" --file "$BENCH_FIXTURE" add "$SCALE_DUPLICATE_TEXT" --agent "$SCALE_DUPLICATE_AGENT" --severity minor --tag "$SCALE_DUPLICATE_TAG_1" --tag "$SCALE_DUPLICATE_TAG_2" >/dev/null
                        ;;
                    resolve)
                        BLOTTER_NOW="$SCALE_MEASUREMENT_NOW" "$BENCH_BIN" --file "$BENCH_SCRATCH/resolve-$index.jsonl" resolve "$BENCH_RESOLVE_ID" --agent "$SCALE_DUPLICATE_AGENT" --note "scale baseline mutation" >/dev/null
                        ;;
                esac
                status=$?
                if [ "$status" -ne "$BENCH_EXPECTED_EXIT" ]; then
                    echo "unexpected $BENCH_COMMAND exit: got $status, expected $BENCH_EXPECTED_EXIT" >&2
                    exit 97
                fi
                index=$((index + 1))
            done
        ' >/dev/null 2>"$time_file"
    else
        "$time_bin" -f '%e\t%U\t%S\t%M' /bin/sh -c '
            index=0
            while [ "$index" -lt "$BENCH_INNER_RUNS" ]; do
                case "$BENCH_COMMAND" in
                    list)
                        BLOTTER_NOW="$SCALE_MEASUREMENT_NOW" "$BENCH_BIN" --file "$BENCH_FIXTURE" list >/dev/null
                        ;;
                    triage)
                        BLOTTER_NOW="$SCALE_MEASUREMENT_NOW" "$BENCH_BIN" --file "$BENCH_FIXTURE" triage >/dev/null
                        ;;
                    verify)
                        BLOTTER_NOW="$SCALE_MEASUREMENT_NOW" "$BENCH_BIN" --file "$BENCH_FIXTURE" verify >/dev/null
                        ;;
                    digest)
                        BLOTTER_NOW="$SCALE_MEASUREMENT_NOW" "$BENCH_BIN" --file "$BENCH_FIXTURE" digest --since "$SCALE_DIGEST_SINCE" >/dev/null
                        ;;
                    doctor)
                        BLOTTER_NOW="$SCALE_MEASUREMENT_NOW" "$BENCH_BIN" --file "$BENCH_FIXTURE" doctor >/dev/null
                        ;;
                    add_duplicate)
                        BLOTTER_NOW="$SCALE_DUPLICATE_ADD_NOW" "$BENCH_BIN" --file "$BENCH_FIXTURE" add "$SCALE_DUPLICATE_TEXT" --agent "$SCALE_DUPLICATE_AGENT" --severity minor --tag "$SCALE_DUPLICATE_TAG_1" --tag "$SCALE_DUPLICATE_TAG_2" >/dev/null
                        ;;
                    resolve)
                        BLOTTER_NOW="$SCALE_MEASUREMENT_NOW" "$BENCH_BIN" --file "$BENCH_SCRATCH/resolve-$index.jsonl" resolve "$BENCH_RESOLVE_ID" --agent "$SCALE_DUPLICATE_AGENT" --note "scale baseline mutation" >/dev/null
                        ;;
                esac
                status=$?
                if [ "$status" -ne "$BENCH_EXPECTED_EXIT" ]; then
                    echo "unexpected $BENCH_COMMAND exit: got $status, expected $BENCH_EXPECTED_EXIT" >&2
                    exit 97
                fi
                index=$((index + 1))
            done
        ' >/dev/null 2>"$time_file"
    fi
    status=$?
    set -e
    if [ "$status" -ne 0 ]; then
        cat "$time_file" >&2
        fail "timed $command_name batch failed"
    fi
}

parse_timing() {
    time_file=$1
    count=$2
    if [ "$time_style" = darwin ]; then
        raw=$(
            awk '
                / real[[:space:]]+.* user[[:space:]]+.* sys$/ {
                    wall = $1; user = $3; sys = $5
                }
                /maximum resident set size/ { rss = $1 }
                END {
                    if (wall == "" || user == "" || sys == "" || rss == "") exit 1
                    printf "%s\t%s\t%s\t%s\n", wall, user, sys, rss
                }
            ' "$time_file"
        ) || fail "could not parse macOS /usr/bin/time -l output in $time_file"
    else
        raw=$(awk -F '\t' 'NF == 4 { line = $0 } END { if (line == "") exit 1; print line }' "$time_file") || fail "could not parse GNU time output in $time_file"
    fi
    wall=$(printf '%s\n' "$raw" | awk -F '\t' '{ print $1 }')
    user=$(printf '%s\n' "$raw" | awk -F '\t' '{ print $2 }')
    sys=$(printf '%s\n' "$raw" | awk -F '\t' '{ print $3 }')
    rss=$(printf '%s\n' "$raw" | awk -F '\t' '{ print $4 }')
    # Darwin's /usr/bin/time -l exposes getrusage(2)'s ru_maxrss in bytes;
    # GNU time's %M is already KiB. Normalize the stored column to KiB.
    if [ "$time_style" = darwin ]; then
        rss=$(awk -v value="$rss" 'BEGIN { printf "%.0f\n", value / 1024 }')
    fi
    awk -v wall="$wall" -v user="$user" -v sys="$sys" -v rss="$rss" -v count="$count" 'BEGIN {
        printf "%.9f\t%.9f\t%.9f\t%.9f\t%s\n", wall / count, user / count, sys / count, (user + sys) / count, rss
    }'
}

run_sample() {
    label=$1
    command_name=$2
    sample=$3
    fixture=$4
    resolve_id=$5
    count=$6
    sample_dir="$scratch_root/$label-$command_name-$sample"
    time_file="$sample_dir/time.txt"
    prepare_resolve_copies "$sample_dir" "$fixture" "$count"
    run_batch "$command_name" "$fixture" "$resolve_id" "$sample_dir" "$count" "$(expected_exit "$command_name")" "$time_file"
    timing=$(parse_timing "$time_file" "$count")
    wall=$(printf '%s\n' "$timing" | cut -f1)
    user=$(printf '%s\n' "$timing" | cut -f2)
    sys=$(printf '%s\n' "$timing" | cut -f3)
    cpu=$(printf '%s\n' "$timing" | cut -f4)
    rss=$(printf '%s\n' "$timing" | cut -f5)
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$label" "$command_name" "$sample" "$count" "$wall" "$user" "$sys" "$cpu" "$rss" "$(expected_exit "$command_name")" >>"$output"
}

warm_up() {
    label=$1
    command_name=$2
    fixture=$3
    resolve_id=$4
    sample_dir="$scratch_root/warmup-$label-$command_name"
    time_file="$sample_dir/time.txt"
    prepare_resolve_copies "$sample_dir" "$fixture" 1
    run_batch "$command_name" "$fixture" "$resolve_id" "$sample_dir" 1 "$(expected_exit "$command_name")" "$time_file"
}

stats() {
    label=$1
    command_name=$2
    column=$3
    awk -F '\t' -v label="$label" -v command_name="$command_name" -v column="$column" '
        NR > 1 && $1 == label && $2 == command_name { print $column }
    ' "$output" | sort -n | awk '
        { values[NR] = $1 }
        END {
            if (NR == 0) exit 1
            min = values[1]
            if (NR % 2 == 1) median = values[(NR + 1) / 2]
            else median = (values[NR / 2] + values[NR / 2 + 1]) / 2
            printf "%.9f\t%.9f\n", min, median
        }
    '
}

milliseconds() {
    awk -v value="$1" 'BEGIN { printf "%.2f", value * 1000 }'
}

peak_kib() {
    awk -v value="$1" 'BEGIN { printf "%.0f", value }'
}

print_table() {
    printf '\n| fixture | command | wall ms min / median | CPU ms min / median | peak RSS KiB min / median |\n'
    printf '| --- | --- | ---: | ---: | ---: |\n'
    for label in 1k 10k; do
        for command_name in list triage verify digest doctor add_duplicate resolve; do
            wall_stats=$(stats "$label" "$command_name" 5)
            cpu_stats=$(stats "$label" "$command_name" 8)
            rss_stats=$(stats "$label" "$command_name" 9)
            wall_min=$(printf '%s\n' "$wall_stats" | cut -f1)
            wall_median=$(printf '%s\n' "$wall_stats" | cut -f2)
            cpu_min=$(printf '%s\n' "$cpu_stats" | cut -f1)
            cpu_median=$(printf '%s\n' "$cpu_stats" | cut -f2)
            rss_min=$(printf '%s\n' "$rss_stats" | cut -f1)
            rss_median=$(printf '%s\n' "$rss_stats" | cut -f2)
            printf '| %s | %s | %s / %s | %s / %s | %s / %s |\n' \
                "$label" "$command_name" \
                "$(milliseconds "$wall_min")" "$(milliseconds "$wall_median")" \
                "$(milliseconds "$cpu_min")" "$(milliseconds "$cpu_median")" \
                "$(peak_kib "$rss_min")" "$(peak_kib "$rss_median")"
        done
    done
}

printf 'fixture\tcommand\tsample\tinner_runs\twall_s\tuser_s\tsys_s\tcpu_s\tpeak_rss_kib\texpected_exit\n' >"$output"

for label in 1k 10k; do
    case "$label" in
        1k)
            fixture="$fixture_dir/$SCALE_1K_FILE"
            resolve_id=$SCALE_1K_RESOLVE_ID
            ;;
        10k)
            fixture="$fixture_dir/$SCALE_10K_FILE"
            resolve_id=$SCALE_10K_RESOLVE_ID
            ;;
    esac
    [ -f "$fixture" ] || fail "fixture is missing: $fixture"
    for command_name in list triage verify digest doctor add_duplicate resolve; do
        warm_up "$label" "$command_name" "$fixture" "$resolve_id"
    done
done

sample=1
while [ "$sample" -le "$runs" ]; do
    for label in 1k 10k; do
        case "$label" in
            1k)
                fixture="$fixture_dir/$SCALE_1K_FILE"
                resolve_id=$SCALE_1K_RESOLVE_ID
                ;;
            10k)
                fixture="$fixture_dir/$SCALE_10K_FILE"
                resolve_id=$SCALE_10K_RESOLVE_ID
                ;;
        esac
        for command_name in list triage verify digest doctor add_duplicate resolve; do
            run_sample "$label" "$command_name" "$sample" "$fixture" "$resolve_id" "$inner_runs"
        done
    done
    sample=$((sample + 1))
done

python3 "$generator" --output-dir "$fixture_dir" --check
print_table
printf '\nraw samples: %s\n' "$output"
printf 'samples: %s batches per command/fixture; %s invocations per batch\n' "$runs" "$inner_runs"
