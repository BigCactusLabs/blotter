#!/usr/bin/env python3
"""Opt-in, read-only snapshot of the search endpoint used by skills@1.7.0.

This is an implementation-level endpoint, not the authenticated /api/v1 API.
Unavailable/malformed responses never become evidence of catalog absence.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import Request, urlopen

TARGET = "bigcactuslabs/blotter/blotter"
QUERIES = ("blotter", "friction", "agent friction", "retrospective", "recurring tool failures")
MAX_BYTES = 1024 * 1024


def classify(payload: object) -> dict:
    if not isinstance(payload, dict) or not isinstance(payload.get("skills"), list):
        raise ValueError("expected an object with a skills array")
    rows = []
    for item in payload["skills"]:
        if not isinstance(item, dict) or not all(isinstance(item.get(key), str) and item[key] for key in ("id", "name", "source")):
            raise ValueError("search row missing string identity fields")
        installs = item.get("installs", 0)
        if type(installs) is not int or installs < 0:
            raise ValueError("invalid install count")
        rows.append({key: item[key] for key in ("id", "name", "source")} | {"installs": installs})
    # The client sorts the API response by installs. Preserve both orders.
    displayed = sorted(rows, key=lambda row: row["installs"], reverse=True)

    def rank(items: list[dict]) -> int | None:
        return next((i for i, row in enumerate(items, 1)
                     if row["id"].casefold() == TARGET
                     and row["source"].casefold() == "bigcactuslabs/blotter"), None)

    api_rank, cli_rank = rank(rows), rank(displayed)
    return {"status": "found" if api_rank else "not_returned", "api_rank": api_rank,
            "cli_rank": cli_rank, "returned_count": len(rows), "results": rows}


def probe(query: str) -> dict:
    url = "https://skills.sh/api/search?" + urlencode({"q": query, "limit": 20})
    result = {"query": query, "url": url, "observed_at": datetime.now(timezone.utc).isoformat(),
              "status": "unavailable", "http_status": None}
    try:
        request = Request(url, headers={"Accept": "application/json", "User-Agent": "blotter-discovery-audit/1.0"})
        with urlopen(request, timeout=20) as response:
            result["http_status"] = response.status
            body = response.read(MAX_BYTES + 1)
            if len(body) > MAX_BYTES:
                raise ValueError("response exceeds 1 MiB")
            result["response_sha256"] = hashlib.sha256(body).hexdigest()
            result.update(classify(json.loads(body)))
    except HTTPError as exc:
        result.update(http_status=exc.code, error="http_error")
    except (URLError, OSError) as exc:
        result["error"] = type(exc).__name__
    except (ValueError, UnicodeError) as exc:
        result.update(status="malformed", error=str(exc))
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--live", action="store_true", help="allow five public HTTP GETs; no writes or installs")
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if not args.live:
        parser.error("network access is opt-in: pass --live")
    report = {"schema_version": 1, "target": TARGET, "limit": 20,
              "scope": "one response per query; not global indexing, Google rank, or usage attribution",
              "observations": [probe(query) for query in QUERIES]}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    # An audit can complete without visibility; statuses retain that distinction.
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
