#!/usr/bin/env python3
"""Submit Blotter's public repository to Context7 or check actual retrieval.

Default: offline plan. Live actions use only CONTEXT7_API_KEY from the environment.
No retries, redirects, private-source uploads, or automatic publication on push.
Contract: https://context7.com/docs/openapi.json (checked 2026-09-19).
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import socket
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode, urlsplit
from urllib.request import Request, HTTPRedirectHandler, build_opener

BASE = "https://context7.com/api"
REPOSITORY = "https://github.com/BigCactusLabs/blotter"
LIBRARY = "/bigcactuslabs/blotter"
MAX_BYTES = 1_048_576
QUERIES = (
    ("installation", "Install blotter-cli and use blotter schema for the machine contract"),
    ("capture", "Capture qualifying coding-agent friction with blotter add --impact"),
    ("recurrence", "Use blotter verify to check recurrence after resolve --disposition fixed"),
)


class NoRedirects(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        # Do not forward a bearer key to a different URL, even on a 30x response.
        return None


class PublicationError(Exception):
    def __init__(self, status: str, http_status: int | None = None):
        super().__init__(status)
        self.status = status
        self.http_status = http_status


class Client:
    def __init__(self, key: str):
        if not key.strip() or any(ord(char) < 33 or ord(char) > 126 for char in key):
            raise PublicationError("missing_or_invalid_credential")
        self.key = key
        self.opener = build_opener(NoRedirects())
        self.observations: list[dict] = []

    def request(self, path: str, payload: dict | None = None) -> dict:
        if not (path == "/v2/add/repo/github" or path.startswith("/v2/libs/search?")
                or path.startswith("/v2/context?")):
            raise PublicationError("unsupported_endpoint")
        request = Request(BASE + path,
                          data=None if payload is None else json.dumps(payload).encode("utf-8"),
                          headers={"Authorization": "Bearer " + self.key, "Accept": "application/json",
                                   "Content-Type": "application/json", "User-Agent": "blotter-discovery/1"})
        observation = {"method": request.get_method(), "url": request.full_url,
                       "observed_at": datetime.now(timezone.utc).isoformat()}
        self.observations.append(observation)
        try:
            with self.opener.open(request, timeout=30) as response:
                observation["http_status"] = response.status
                if response.status != 200:
                    raise PublicationError("processing" if response.status == 202 else "unexpected_http_status",
                                           response.status)
                body = response.read(MAX_BYTES + 1)
                if len(body) > MAX_BYTES:
                    raise PublicationError("response_too_large")
                observation["response_sha256"] = hashlib.sha256(body).hexdigest()
                result = json.loads(body)
                if not isinstance(result, dict):
                    raise PublicationError("malformed_response")
                return result
        except HTTPError as exc:
            observation["http_status"] = exc.code
            # Do not log arbitrary response bodies, Location headers, or exceptions:
            # they may contain reflected credentials or private teamspace metadata.
            status = {401: "unauthorized", 403: "forbidden", 402: "spending_limit",
                      409: "already_exists_unverified", 429: "rate_limited",
                      404: "not_found_response"}.get(exc.code, "http_error")
            if 300 <= exc.code < 400:
                status = "redirect_refused"
            exc.close()
            raise PublicationError(status, exc.code) from None
        except (URLError, TimeoutError, socket.timeout, OSError):
            raise PublicationError("transport_unavailable") from None
        except (ValueError, UnicodeError):
            raise PublicationError("malformed_response") from None


def own_source(url: object) -> bool:
    if not isinstance(url, str):
        return False
    try:
        parsed = urlsplit(url)
        if parsed.scheme != "https" or parsed.username or parsed.password or parsed.port not in (None, 443):
            return False
    except ValueError:
        return False
    parts = parsed.path.strip("/").split("/")
    if [part.lower() for part in parts[:2]] != ["bigcactuslabs", "blotter"]:
        return False
    if parsed.hostname == "github.com":
        return len(parts) >= 5 and parts[2] == "blob"
    return parsed.hostname == "raw.githubusercontent.com" and len(parts) >= 4


def retrieval_summary(data: dict) -> dict:
    code = data.get("codeSnippets")
    info = data.get("infoSnippets")
    if not isinstance(code, list) or not isinstance(info, list):
        raise PublicationError("malformed_response")
    sources = []
    for row in code:
        if not isinstance(row, dict) or not isinstance(row.get("codeList"), list):
            raise PublicationError("malformed_response")
        nonempty = any(isinstance(item, dict) and isinstance(item.get("code"), str)
                       and item["code"].strip() for item in row["codeList"])
        if nonempty and own_source(row.get("codeId")):
            sources.append(row["codeId"])
    for row in info:
        if not isinstance(row, dict) or not isinstance(row.get("content"), str):
            raise PublicationError("malformed_response")
        if row["content"].strip() and own_source(row.get("pageId")):
            sources.append(row["pageId"])
    return {"own_source_snippets": len(sources), "source_urls": sorted(set(sources))}


def execute(action: str, client: Client | None) -> dict:
    if action == "plan":
        return {"status": "planned", "network_performed": False,
                "submit": {"method": "POST", "url": BASE + "/v2/add/repo/github",
                           "body": {"docsRepoUrl": REPOSITORY, "private": False}},
                "required_environment": "CONTEXT7_API_KEY", "index_verified": False}
    if client is None:
        raise PublicationError("missing_or_invalid_credential")
    if action == "submit":
        result = client.request("/v2/add/repo/github", {"docsRepoUrl": REPOSITORY, "private": False})
        identity = result.get("libraryName")
        if not isinstance(identity, str) or identity.lower() != LIBRARY:
            raise PublicationError("unexpected_library_identity")
        return {"status": "submission_accepted", "library_id": identity,
                "index_verified": False, "next_action": "verify after provider processing"}
    if action != "verify":
        raise PublicationError("unsupported_action")
    result = client.request("/v2/libs/search?" + urlencode({"libraryName": "blotter",
                            "query": "Local coding-agent friction ledger", "fast": "true"}))
    rows = result.get("results")
    if not isinstance(rows, list) or not all(isinstance(row, dict) for row in rows):
        raise PublicationError("malformed_response")
    matches = [row for row in rows if isinstance(row.get("id"), str) and row["id"].lower() == LIBRARY]
    common = {"search_filter_applied": result.get("searchFilterApplied"),
              "index_verified": False, "quality_review_required": True}
    if not matches:
        return common | {"status": "not_returned", "scope": "this search response only"}
    if len(matches) != 1:
        raise PublicationError("ambiguous_library_identity")
    library = matches[0]
    state = library.get("state")
    if state not in ("finalized", "initial", "processing", "error", "delete"):
        raise PublicationError("malformed_library_state")
    common.update(library_id=library["id"], provider_state=state)
    if state != "finalized":
        return common | {"status": "not_ready"}
    checks = []
    for name, query in QUERIES:
        result = client.request("/v2/context?" + urlencode({"libraryId": library["id"],
                                 "query": query, "type": "json", "fast": "true"}))
        checks.append({"name": name, "query": query} | retrieval_summary(result))
    available = all(check["own_source_snippets"] > 0 for check in checks)
    return common | {"status": "retrieval_available" if available else "retrieval_incomplete",
                     "index_verified": True, "retrieval_checks": checks}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--action", choices=("plan", "submit", "verify"), default="plan")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    report = {"schema_version": 1, "action": args.action, "repository": REPOSITORY,
              "model_activation_tested": False, "content_accuracy_verified": False}
    client = None
    try:
        if args.action != "plan":
            client = Client(os.environ.get("CONTEXT7_API_KEY", ""))
        report.update(execute(args.action, client))
    except PublicationError as error:
        report.update(status=error.status, index_verified=False)
        if error.http_status is not None:
            report["http_status"] = error.http_status
    report["requests"] = client.observations if client else []
    # Never persist a provider-echoed credential, including in source URLs.
    output = json.dumps(report, indent=2)
    key = os.environ.get("CONTEXT7_API_KEY", "")
    if key:
        output = output.replace(key, "[REDACTED]")
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(output + "\n", encoding="utf-8")
    print(output)
    return 0 if report["status"] in ("planned", "submission_accepted", "retrieval_available") else 2


if __name__ == "__main__":
    raise SystemExit(main())
