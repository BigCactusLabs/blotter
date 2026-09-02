#!/usr/bin/env python3
"""Generate deterministic JSONL fixtures for the TASK-29.1 scale baseline.

This is development-only tooling. It uses Python's standard library only and
does not participate in the release crate or its dependency graph.
"""

import argparse
import hashlib
import json
import shlex
import struct
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path


FIXTURE_START = datetime(2026, 1, 15, tzinfo=timezone.utc)
MEASUREMENT_NOW = "2026-01-16T00:00:00.000Z"
DIGEST_SINCE = "2026-01-15T00:00:00.000Z"
AGENT = "scale-fixture"
DUPLICATE_TEXT = "baseline duplicate detection sentinel"
DUPLICATE_TAGS = ["baseline", "duplicate"]
RESOLVE_TARGET_TEXT = "baseline resolve mutation target"

FIXTURES = (("1k", 1_000), ("10k", 10_000), ("100k", 100_000), ("300k", 300_000))
DEFAULT_FIXTURE_LABELS = ("1k", "10k")
COMPOSITION_PER_1K = {
    "unrelated_open_cuts": 630,
    "repeated_title_open_cuts": 80,
    "tagged_near_duplicate_open_cuts": 80,
    "resolved_anchor_cut_events": 60,
    "resolve_events": 60,
    "recurrence_open_cuts": 40,
    "dogear_events": 30,
    "duplicate_cut_events": 10,
    "duplicate_resolve_events": 5,
    "malformed_lines": 5,
}


def timestamp(offset_seconds):
    value = FIXTURE_START + timedelta(seconds=offset_seconds)
    return value.strftime("%Y-%m-%dT%H:%M:%S.000Z")


def normalized_tags(tags):
    return sorted(set(tags))


def content_id(fields, digest_bytes):
    digest = hashlib.sha256()
    for field in fields:
        encoded = field.encode("utf-8")
        digest.update(struct.pack("<I", len(encoded)))
        digest.update(encoded)
    return "bl_" + digest.digest()[:digest_bytes].hex()


def cut_id(ts, agent, text, impact, tags):
    tags = normalized_tags(tags)
    return content_id(
        ["bl2", "cut", ts, agent, text, impact, str(len(tags)), *tags], 10
    )


def dogear_id(ts, agent, text, tags):
    tags = normalized_tags(tags)
    return content_id(["bl2", "dogear", ts, agent, text, str(len(tags)), *tags], 10)


def json_line(record):
    return json.dumps(record, ensure_ascii=False, separators=(",", ":"))


class FixtureBuilder:
    def __init__(self, scale):
        self.scale = scale
        self.lines = []
        self.clock = 0
        self.composition = {name: 0 for name in COMPOSITION_PER_1K}

    def next_timestamp(self):
        value = timestamp(self.clock)
        self.clock += 1
        return value

    def add_cut(self, text, tags, bucket):
        ts = self.next_timestamp()
        tags = normalized_tags(tags)
        record = {
            "v": 2,
            "kind": "cut",
            "id": cut_id(ts, AGENT, text, "low", tags),
            "ts": ts,
            "agent": AGENT,
            "text": text,
            "tags": tags,
            "impact": "low",
            "cwd": ".",
            "origin": {"type": "agent"},
        }
        self.lines.append(json_line(record))
        self.composition[bucket] += 1
        return record

    def add_dogear(self, text, tags):
        ts = self.next_timestamp()
        tags = normalized_tags(tags)
        record = {
            "v": 2,
            "kind": "dogear",
            "id": dogear_id(ts, AGENT, text, tags),
            "ts": ts,
            "agent": AGENT,
            "text": text,
            "tags": tags,
            "cwd": ".",
            "origin": {"type": "agent"},
        }
        self.lines.append(json_line(record))
        self.composition["dogear_events"] += 1
        return record

    def add_resolve(self, cut, note):
        ts = self.next_timestamp()
        record = {
            "v": 2,
            "kind": "resolve",
            "id": cut["id"],
            "ts": ts,
            "agent": AGENT,
            "note": note,
            "disposition": "fixed",
            "disposition_ts": ts,
        }
        self.lines.append(json_line(record))
        self.composition["resolve_events"] += 1
        return record

    def add_existing(self, record, bucket):
        self.lines.append(json_line(record))
        self.composition[bucket] += 1

    def add_malformed(self, line):
        self.lines.append(line)
        self.composition["malformed_lines"] += 1


def build_fixture(size):
    if size % 1_000:
        raise ValueError(f"fixture size must be a multiple of 1000, got {size}")
    scale = size // 1_000
    builder = FixtureBuilder(scale)

    unrelated = []
    for index in range(COMPOSITION_PER_1K["unrelated_open_cuts"] * scale):
        if index == 0:
            unrelated.append(
                builder.add_cut(DUPLICATE_TEXT, DUPLICATE_TAGS, "unrelated_open_cuts")
            )
        elif index == 1:
            unrelated.append(
                builder.add_cut(RESOLVE_TARGET_TEXT, [], "unrelated_open_cuts")
            )
        else:
            unrelated.append(
                builder.add_cut(
                    "unrelated fixture "
                    f"u{index:05d} v{index * 7:05d} w{index * 13:05d}",
                    [],
                    "unrelated_open_cuts",
                )
            )

    for group in range(20 * scale):
        text = f"exact recurring fixture title group {group:04d}"
        for _ in range(4):
            builder.add_cut(text, [f"exact-{group:04d}"], "repeated_title_open_cuts")

    variants = ("alpha", "beta", "gamma", "delta")
    for group in range(20 * scale):
        for variant in variants:
            builder.add_cut(
                f"tagged cache pressure regression group {group:04d} variant {variant}",
                [f"near-{group:04d}"],
                "tagged_near_duplicate_open_cuts",
            )

    anchors = []
    for index in range(COMPOSITION_PER_1K["resolved_anchor_cut_events"] * scale):
        anchors.append(
            builder.add_cut(
                f"resolved recurrence anchor {index:05d}",
                [f"verify-{index:05d}"],
                "resolved_anchor_cut_events",
            )
        )

    resolves = []
    for index, anchor in enumerate(anchors):
        resolves.append(builder.add_resolve(anchor, f"fixture resolved anchor {index:05d}"))

    for anchor in anchors[: COMPOSITION_PER_1K["recurrence_open_cuts"] * scale]:
        builder.add_cut(anchor["text"], anchor["tags"], "recurrence_open_cuts")

    for index in range(COMPOSITION_PER_1K["dogear_events"] * scale):
        builder.add_dogear(f"scale fixture dogear {index:05d}", ["idea", "scale"])

    for record in unrelated[2 : 2 + COMPOSITION_PER_1K["duplicate_cut_events"] * scale]:
        builder.add_existing(record, "duplicate_cut_events")

    for record in resolves[: COMPOSITION_PER_1K["duplicate_resolve_events"] * scale]:
        builder.add_existing(record, "duplicate_resolve_events")

    malformed = (
        # Each carries "v":2 so the version probe passes and each line stays
        # malformed for the reason it was written: a bad ts, a non-string id, a
        # missing required field. Without the marker the probe would refuse the
        # whole fixture and every scale run would measure a refusal.
        '{"v":2,"kind":"cut","id":"bl_bad","ts":"not-a-time"}',
        '{"v":2,"kind":"resolve","id":42,"ts":"2026-01-15T00:00:00.000Z","agent":"scale-fixture"}',
        '{"v":2,"kind":"dogear","id":"bl_bad","ts":"2026-01-15T00:00:00.000Z"}',
        '{"kind":"cut"',
        "not-json",
    )
    for index in range(COMPOSITION_PER_1K["malformed_lines"] * scale):
        builder.add_malformed(malformed[index % len(malformed)])

    expected = {name: count * scale for name, count in COMPOSITION_PER_1K.items()}
    if builder.composition != expected:
        raise AssertionError((builder.composition, expected))
    if len(builder.lines) != size:
        raise AssertionError(f"expected {size} physical lines, got {len(builder.lines)}")

    return {
        "bytes": ("\n".join(builder.lines) + "\n").encode("utf-8"),
        "composition": expected,
        "duplicate_add_now": unrelated[0]["ts"],
        "resolve_target_id": unrelated[1]["id"],
    }


def fixture_name(label):
    return f"scale-{label}.jsonl"


def selected_fixtures(labels):
    return tuple((label, size) for label, size in FIXTURES if label in labels)


def rendered_env(manifest):
    fixtures = manifest["fixtures"]
    duplicate = manifest["duplicate_add"]
    lines = [
        "# Generated by scripts/dev/generate-scale-fixtures.py; do not hand-edit.",
        "SCALE_FIXTURE_FORMAT=1",
        "SCALE_FIXTURE_LABELS="
        + shlex.quote(" ".join(label for label, _ in FIXTURES if label in fixtures)),
        f"SCALE_MEASUREMENT_NOW={shlex.quote(manifest['fixed_clock']['measurement_now'])}",
        f"SCALE_DIGEST_SINCE={shlex.quote(manifest['fixed_clock']['digest_since'])}",
        f"SCALE_DUPLICATE_ADD_NOW={shlex.quote(duplicate['now'])}",
        f"SCALE_DUPLICATE_TEXT={shlex.quote(duplicate['text'])}",
        f"SCALE_DUPLICATE_AGENT={shlex.quote(duplicate['agent'])}",
        f"SCALE_DUPLICATE_TAG_1={shlex.quote(duplicate['tags'][0])}",
        f"SCALE_DUPLICATE_TAG_2={shlex.quote(duplicate['tags'][1])}",
    ]
    for label, _ in FIXTURES:
        if label not in fixtures:
            continue
        fixture = fixtures[label]
        upper = label.upper()
        lines.extend(
            [
                f"SCALE_{upper}_FILE={shlex.quote(fixture['file'])}",
                f"SCALE_{upper}_LINES={fixture['physical_lines']}",
                f"SCALE_{upper}_SHA256={fixture['sha256']}",
                f"SCALE_{upper}_RESOLVE_ID={shlex.quote(fixture['resolve_target_id'])}",
            ]
        )
    return ("\n".join(lines) + "\n").encode("utf-8")


def expected_outputs(labels):
    fixture_outputs = {}
    manifest = {
        "format": 1,
        "generator": "scripts/dev/generate-scale-fixtures.py",
        "fixed_clock": {
            "fixture_start": timestamp(0),
            "measurement_now": MEASUREMENT_NOW,
            "digest_since": DIGEST_SINCE,
        },
        "duplicate_add": {
            "text": DUPLICATE_TEXT,
            "agent": AGENT,
            "tags": DUPLICATE_TAGS,
            "now": None,
        },
        "fixtures": {},
    }
    for label, size in selected_fixtures(labels):
        fixture = build_fixture(size)
        name = fixture_name(label)
        contents = fixture["bytes"]
        manifest["fixtures"][label] = {
            "file": name,
            "physical_lines": size,
            "bytes": len(contents),
            "sha256": hashlib.sha256(contents).hexdigest(),
            "composition": fixture["composition"],
            "resolve_target_id": fixture["resolve_target_id"],
        }
        fixture_outputs[name] = contents
        if manifest["duplicate_add"]["now"] is None:
            manifest["duplicate_add"]["now"] = fixture["duplicate_add_now"]
        elif manifest["duplicate_add"]["now"] != fixture["duplicate_add_now"]:
            raise AssertionError("duplicate sentinel timestamp drifted by fixture size")

    manifest_bytes = (
        json.dumps(manifest, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    ).encode("utf-8")
    outputs = dict(fixture_outputs)
    outputs["scale-fixtures.json"] = manifest_bytes
    outputs["scale-fixtures.env"] = rendered_env(manifest)
    return outputs


def write_outputs(output_dir, outputs, force):
    output_dir.mkdir(parents=True, exist_ok=True)
    conflicts = []
    for name, expected in outputs.items():
        path = output_dir / name
        if path.exists() and path.read_bytes() != expected:
            conflicts.append(path)
    if conflicts and not force:
        rendered = ", ".join(str(path) for path in conflicts)
        raise RuntimeError(
            f"refusing to overwrite different fixture data: {rendered}; rerun with --force"
        )
    for name, expected in outputs.items():
        path = output_dir / name
        if not path.exists() or path.read_bytes() != expected:
            path.write_bytes(expected)


def check_outputs(output_dir, outputs):
    failures = []
    for name, expected in outputs.items():
        path = output_dir / name
        if not path.exists():
            failures.append(f"missing {path}")
        elif path.read_bytes() != expected:
            failures.append(f"content differs: {path}")
    if failures:
        raise RuntimeError("; ".join(failures))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("target/scale-fixtures"),
        help="directory for selected scale fixtures and deterministic metadata",
    )
    parser.add_argument(
        "--fixtures",
        nargs="+",
        choices=[label for label, _ in FIXTURES],
        default=DEFAULT_FIXTURE_LABELS,
        help=(
            "fixture sizes to generate (default: 1k 10k). Metadata reflects "
            "the last generation; regenerate the fixture union in one --fixtures call."
        ),
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify that existing files exactly match the deterministic generator output",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="replace differing output files (required to overwrite existing fixture data)",
    )
    args = parser.parse_args()
    if args.check and args.force:
        parser.error("--check does not write files and cannot be combined with --force")

    outputs = expected_outputs(args.fixtures)
    try:
        if args.check:
            check_outputs(args.output_dir, outputs)
            print(f"fixtures verified: {args.output_dir}")
        else:
            write_outputs(args.output_dir, outputs, args.force)
            manifest = json.loads(outputs["scale-fixtures.json"])
            for label, _ in selected_fixtures(args.fixtures):
                fixture = manifest["fixtures"][label]
                print(
                    f"{fixture['file']}: {fixture['physical_lines']} lines, "
                    f"{fixture['bytes']} bytes, sha256 {fixture['sha256']}"
                )
            print(f"fixtures written: {args.output_dir}")
    except RuntimeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
