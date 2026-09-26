#!/usr/bin/env python3
"""Patterns before prescriptions: a removable, read-only presentation experiment.

Compose `triage --min-count 2` with `retrospect`, retaining clusters for which
retrospect has no artifact suggestion. Reuse the CLI's matching, counts and
ordering; do not parse/rewrite JSONL or invent a second clustering algorithm.

Requires Python 3.11+ and an installed/built Blotter. --file is mandatory. Output
is experimental, not part of blotter schema, and is not shipped in the crate.
Two CLI reads are not an atomic snapshot: use a quiescent ledger for comparisons.
Detected file changes or inconsistent source sets fail rather than mixing them.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys


class PreviewError(ValueError):
    def __init__(self, message: str, exit_code: int = 2):
        super().__init__(message)
        self.exit_code = exit_code


def strings(value, label: str) -> list[str]:
    if not isinstance(value, list) or any(not isinstance(v, str) for v in value):
        raise PreviewError(f"Invalid {label}: expected a string array")
    return value


def source_key(value) -> tuple[str, ...]:
    ids = strings(value, "source IDs")
    if not ids or any(not re.fullmatch(r"bl_[0-9a-f]{20}", v) for v in ids) or len(set(ids)) != len(ids):
        raise PreviewError("Invalid source IDs: expected distinct full Blotter IDs")
    return tuple(sorted(ids))


def count(value, label: str) -> int:
    if type(value) is not int or value < 0:
        raise PreviewError(f"Invalid {label}: expected a nonnegative integer")
    return value


def compose(triage: dict, retrospect: dict) -> dict:
    """Attach advice only to the exact source set; unknown advice never hides data."""
    try:
        clusters, candidates = triage["clusters"], retrospect["candidates"]
        if not isinstance(clusters, list) or not isinstance(candidates, list):
            raise PreviewError("Invalid analysis collections")
        if count(triage["count"], "triage count") != len(clusters) or count(retrospect["count"], "retrospect count") != len(candidates):
            raise PreviewError("Analysis count does not match its collection")
        scanned = count(triage["scanned"], "scanned")
        if scanned != count(retrospect["scanned"], "scanned"):
            raise PreviewError("Analyses disagree; retry on an unchanged ledger")
        advice: dict[tuple[str, ...], list[str]] = {}
        interventions = []
        for candidate in candidates:
            key = source_key(candidate["record_ids"])
            suggested = strings(candidate["suggested"], "suggested artifacts")
            if candidate["pattern"] == "recurrent_friction":
                if key in advice:
                    raise PreviewError("Duplicate advice for one source set")
                advice[key] = suggested
            elif candidate["pattern"] == "failed_intervention":
                source_key(candidate["resolved_anchor_ids"])
                if not isinstance(candidate["title"], str):
                    raise PreviewError("Invalid intervention title")
                interventions.append({
                    "title": candidate["title"],
                    "record_ids": candidate["record_ids"],
                    "resolved_anchor_ids": candidate["resolved_anchor_ids"],
                    "occurrences": count(candidate["occurrences"], "occurrences"),
                    "suggested": suggested,
                })
            else:
                raise PreviewError("Unsupported retrospect pattern; inspect the installed schema")
        patterns = []
        seen = set()
        for cluster in clusters:
            key = source_key(cluster["ids"])
            if key in seen or count(cluster["count"], "record count") != len(key):
                raise PreviewError("Invalid or duplicate cluster")
            seen.add(key)
            if not isinstance(cluster["text"], str):
                raise PreviewError("Invalid cluster text")
            patterns.append({
                "title": cluster["text"],
                "record_ids": cluster["ids"],
                "record_count": cluster["count"],
                "occurrences": count(cluster["occurrences"], "occurrences"),
                "tags": strings(cluster["tags"], "tags"),
                "suggested": advice.pop(key, []),
            })
        if advice:
            raise PreviewError("Analysis source sets disagree; retry on an unchanged ledger")
        return {"experimental": True, "min_count": 2, "scanned": scanned,
                "patterns": patterns, "failed_interventions": interventions}
    except (KeyError, TypeError) as exc:
        raise PreviewError("Unexpected CLI data shape; inspect the installed schema") from exc


def fingerprint(file: Path) -> tuple[int, ...]:
    info = file.stat()
    if not stat.S_ISREG(info.st_mode):
        raise PreviewError("Choose an existing regular ledger file")
    return info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns


def read_analysis(binary: str, file: Path, command: list[str]) -> tuple[dict, list[str]]:
    env = {k: v for k, v in os.environ.items() if not k.startswith("BLOTTER_")}
    result = subprocess.run([binary, "--file", str(file), *command],
                            capture_output=True, text=True, timeout=30, env=env)
    # These two readers use 1 for findings. A lock timeout remains retryable.
    if result.returncode not in (0, 1):
        raise PreviewError(f"{command[0]} failed (exit {result.returncode}); inspect it directly",
                           75 if result.returncode == 75 else 2)
    if result.stderr:
        raise PreviewError(f"Unexpected stderr from {command[0]}; inspect it directly")
    try:
        envelope = json.loads(result.stdout)
        if not isinstance(envelope, dict) or envelope.get("ok") is not True:
            raise PreviewError(f"{command[0]} did not return a success envelope")
        data = envelope["data"]
        if not isinstance(data, dict):
            raise PreviewError("Invalid analysis data")
        warnings = strings(envelope["meta"].get("warnings", []), "warnings")
        expected_exit = int(count(data["count"], "findings count") > 0)
        if result.returncode != expected_exit:
            raise PreviewError("Finding status disagrees with the result count")
        return data, warnings
    except (json.JSONDecodeError, KeyError, TypeError, AttributeError) as exc:
        raise PreviewError(f"Invalid {command[0]} envelope; inspect the installed schema") from exc


def preview(binary: str, file: Path) -> dict:
    file = file.resolve(strict=True)
    before = fingerprint(file)
    triage, tw = read_analysis(binary, file, ["triage", "--min-count", "2"])
    retrospect, rw = read_analysis(binary, file, ["retrospect"])
    if fingerprint(file) != before:
        raise PreviewError("Ledger changed during review; retry on an unchanged ledger")
    result = compose(triage, retrospect)
    result["warnings"] = list(dict.fromkeys([*tw, *rw]))
    return result


def prose(text: str) -> str:
    """Render ledger text as inert prose, not Markdown links/HTML/terminal escapes."""
    text = " ".join(text.splitlines())
    text = "".join(c if c.isprintable() else f"\\u{ord(c):04x}" for c in text)
    return re.sub(r"([\\`*_{}\[\]<>()#+.!|~\-])", r"\\\1", text)


def markdown(report: dict) -> str:
    lines = ["# Patterns before prescriptions", "",
             "Experimental read-only view; two linked records are enough to appear.",
             "Matching suggests what to inspect, not a proven cause or permission to act.", ""]
    for warning in report["warnings"]:
        lines.extend([f"Warning: {prose(warning)}", ""])
    if not report["patterns"]:
        lines.extend(["No qualifying open patterns in this read.", ""])
    for number, item in enumerate(report["patterns"], 1):
        advice = ", ".join(prose(v) for v in item["suggested"]) or "Unknown — inspect the sources before choosing a remedy."
        lines.extend([f"## Pattern {number}", "", prose(item["title"]), "",
                      f"Recorded members: {item['record_count']}; occurrences reported by triage: {item['occurrences']}.",
                      "Tags: " + (", ".join(prose(v) for v in item["tags"]) or "none"),
                      "Source IDs: " + ", ".join(item["record_ids"]),
                      "Suggested artifact: " + advice, ""])
    if report["failed_interventions"]:
        lines.extend(["## Later recurrence after an intervention", "",
                      "These may share records with the patterns above; do not add their counts together.", ""])
    for item in report["failed_interventions"]:
        lines.extend([prose(item["title"]),
                      "Resolved anchor IDs: " + ", ".join(item["resolved_anchor_ids"]),
                      "Later record IDs: " + ", ".join(item["record_ids"]),
                      "Suggested artifact: " + ", ".join(prose(v) for v in item["suggested"]), ""])
    lines.append("No records were promoted or resolved by this view. Output may contain private ledger text; review before sharing.")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="blotter", help="Installed or locally built executable")
    parser.add_argument("--file", type=Path, required=True, help="Existing ledger to read (no automatic discovery)")
    parser.add_argument("--format", choices=("md", "json"), default="md")
    args = parser.parse_args()
    try:
        binary = shutil.which(args.binary)
        if binary is None:
            raise PreviewError("Blotter binary not found; build it or provide --binary")
        report = preview(str(Path(binary).resolve()), args.file)
        print(json.dumps(report, indent=2) if args.format == "json" else markdown(report), end="\n" if args.format == "json" else "")
        return 0  # A preview with patterns is still a successful presentation.
    except PreviewError as exc:
        print(f"patterns: {exc}", file=sys.stderr)
        return exc.exit_code
    except (OSError, UnicodeError, subprocess.SubprocessError) as exc:
        print(f"patterns: unable to complete review ({type(exc).__name__})", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
