#!/usr/bin/env python3
"""Exercise Blotter's friction lifecycle in a disposable synthetic workspace.

No model, installer, network request, or real repository ledger is involved.
Requires Python 3.10+ and an already installed/built Blotter executable.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

TEXT = "Synthetic fixture: the documented test command uses the wrong workspace"
AGENTS = ("fixture-agent-a", "fixture-agent-b", "fixture-agent-c")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def isolated_env(home: Path) -> dict[str, str]:
    # Never inherit credentials, existing Blotter destinations, or agent settings.
    env = {key: os.environ[key] for key in ("PATH", "SystemRoot", "WINDIR") if key in os.environ}
    env.update(HOME=str(home), USERPROFILE=str(home), XDG_CONFIG_HOME=str(home / ".config"),
               LANG="C.UTF-8", LC_ALL="C.UTF-8", TZ="UTC")
    return env


def decode_result(result: subprocess.CompletedProcess, expected_exit: int) -> dict:
    require(result.returncode == expected_exit, f"expected exit {expected_exit}, got {result.returncode}")
    require(not result.stderr, "unexpected stderr from the fixture command")
    envelope = json.loads(result.stdout)
    require(isinstance(envelope, dict) and envelope.get("ok") is True,
            "expected a successful JSON envelope")
    require(isinstance(envelope.get("data"), dict), "missing command data")
    return envelope["data"]


def verify_snapshot(data: dict, anchor: str, later: list[str], disposition_ts: str) -> None:
    require(data["count"] == 1, "expected one resolved anchor, not an occurrence count")
    require(data["distinct_recurring_cuts"] == 2, "expected two distinct later occurrences")
    groups = data["recurrences"]
    require(len(groups) == 1 and groups[0]["resolved_id"] == anchor, "wrong resolved anchor")
    require(set(groups[0]["recurrence_ids"]) == set(later),
            "verification must exclude matching pre-fix observations")
    require(groups[0]["resolution"]["disposition_ts"] == disposition_ts,
            "a note-only amendment must not move the fix boundary")


def demonstrate(binary: Path) -> dict:
    with tempfile.TemporaryDirectory(prefix="blotter-proof-") as temporary:
        home = Path(temporary) / "home"
        workspace = home / "workspace"
        workspace.mkdir(parents=True)
        ledger = workspace / "synthetic.jsonl"
        env = isolated_env(home)
        env["BLOTTER_FILE"] = str(ledger)
        tick = 0
        reads = 0

        def invoke(*args: str, expected_exit: int = 0, read_only: bool = False) -> dict:
            nonlocal tick, reads
            tick += 1
            # Fixed fixture time, not a claim about real incidents or measurements.
            env["BLOTTER_NOW"] = f"2026-09-01T09:{tick:02d}:00.000Z"
            before = ledger.read_bytes() if ledger.exists() else None
            process = subprocess.run(
                [str(binary), "--file", str(ledger), *args], cwd=workspace, env=env,
                capture_output=True, text=True, timeout=15, check=False)
            data = decode_result(process, expected_exit)
            if read_only:
                after = ledger.read_bytes() if ledger.exists() else None
                require(before == after, f"read-only command {args[0]} changed the ledger")
                reads += 1
            return data

        contract = invoke("schema", read_only=True)
        require(contract["contract"] == 6, "this demonstration targets CLI contract 6")
        commands = contract["commands"]
        for command in ("triage", "verify", "retrospect", "digest", "list"):
            require(commands[command]["read_only"] is True, f"{command} contract changed")

        original = []
        for agent in AGENTS:
            record = invoke("add", TEXT, "--agent", agent, "--tag", "tests", "--impact", "material")
            original.append(record["record"]["id"])
        require(len(set(original)) == 3, "independent fixture observations must remain distinct")

        triage = invoke("triage", expected_exit=1, read_only=True)
        require(triage["count"] == 1, "expected one chronic cluster")
        require(set(triage["clusters"][0]["ids"]) == set(original), "cluster lost an occurrence")

        # Resolve only the first cut: two matching pre-fix cuts deliberately remain open.
        invoke("resolve", original[0], "--disposition", "fixed", "--agent", "fixture-maintainer")
        fixed_at = env["BLOTTER_NOW"]
        empty_verify = invoke("verify", read_only=True)
        require(empty_verify["count"] == 0, "pre-fix observations are not later recurrences")

        later = []
        for agent in AGENTS[:2]:
            record = invoke("add", TEXT, "--agent", agent, "--tag", "tests", "--impact", "material")
            later.append(record["record"]["id"])
        before_amend = invoke("verify", expected_exit=1, read_only=True)
        verify_snapshot(before_amend, original[0], later, fixed_at)

        invoke("resolve", original[0], "--amend", "--note", "Synthetic later annotation only",
               "--agent", "fixture-maintainer")
        after_amend = invoke("verify", expected_exit=1, read_only=True)
        verify_snapshot(after_amend, original[0], later, fixed_at)
        require(after_amend["recurrences"][0]["resolution"]["ts"] != fixed_at,
                "fixture must actually exercise a later note-only amendment")

        retrospect = invoke("retrospect", expected_exit=1, read_only=True)
        require(any(item["pattern"] == "failed_intervention" for item in retrospect["candidates"]),
                "later recurrence should produce failed-intervention evidence")
        invoke("digest", "--since", "7d", read_only=True)
        promotions = invoke("list", "--kind", "promotion", read_only=True)
        require(promotions["count"] == 0, "analysis must not automatically promote anything")

        return {
            "schema_version": 1,
            "status": "passed",
            "scenario": "synthetic CLI demonstration; agent names are fixture labels",
            "model_activation_tested": False,
            "network_or_installation_performed": False,
            "contract": contract["contract"],
            "independent_observations_before_fix": len(original),
            "chronic_clusters": triage["count"],
            "resolved_anchors": after_amend["count"],
            "distinct_later_recurrences": after_amend["distinct_recurring_cuts"],
            "matching_pre_fix_cuts_excluded": 2,
            "note_only_amend_preserved_fix_boundary": True,
            "read_only_commands_checked": reads,
            "automatic_promotions": promotions["count"],
            "evidence": {"triage": triage, "verify": after_amend, "retrospect": retrospect},
            "synthetic_ledger_sha256": hashlib.sha256(ledger.read_bytes()).hexdigest(),
        }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="blotter", help="installed/built executable; never downloaded")
    args = parser.parse_args()
    candidate = shutil.which(args.binary)
    if candidate is None:
        parser.error("Blotter is unavailable; install it separately or pass --binary")
    try:
        report = demonstrate(Path(candidate).resolve())
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, subprocess.TimeoutExpired) as error:
        print(json.dumps({"status": "failed", "error": str(error)}), file=sys.stderr)
        return 1
    print(json.dumps(report, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
