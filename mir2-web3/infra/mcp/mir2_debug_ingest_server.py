#!/usr/bin/env python3
"""HTTP ingest endpoint for Mir2 debug snapshots.

This runs next to the remote read-only MCP server. It accepts authenticated
browser snapshots forwarded by the web app and stores them in the isolated
`mir2_debug.snapshots` table.
"""

from __future__ import annotations

import json
import os
import hmac
from datetime import datetime, timezone
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any
from urllib.parse import urlparse

import psycopg


DATABASE_URL = os.environ["MIR2_DEBUG_WRITER_DATABASE_URL"]
INGEST_TOKEN = os.environ["MIR2_DEBUG_INGEST_TOKEN"]
HOST = os.environ.get("MIR2_DEBUG_INGEST_HOST", "127.0.0.1")
PORT = int(os.environ.get("MIR2_DEBUG_INGEST_PORT", "7126"))
PATH = os.environ.get("MIR2_DEBUG_INGEST_PATH", "/debug-snapshots")
MAX_BODY_BYTES = int(os.environ.get("MIR2_DEBUG_INGEST_MAX_BODY_BYTES", "900000"))
ALLOWED_ORIGINS = {
    origin.strip()
    for origin in os.environ.get(
        "MIR2_DEBUG_INGEST_ALLOWED_ORIGINS",
        "https://mir2.obelisk.build,http://localhost:3000,http://127.0.0.1:3000",
    ).split(",")
    if origin.strip()
}


def _json_response(handler: BaseHTTPRequestHandler, status: int, payload: dict[str, Any]) -> None:
    body = json.dumps(payload, ensure_ascii=False, default=str).encode("utf-8")
    handler.send_response(status)
    _cors_headers(handler)
    handler.send_header("content-type", "application/json; charset=utf-8")
    handler.send_header("cache-control", "no-store")
    handler.send_header("content-length", str(len(body)))
    handler.end_headers()
    handler.wfile.write(body)


def _cors_headers(handler: BaseHTTPRequestHandler) -> None:
    origin = handler.headers.get("origin")
    if origin in ALLOWED_ORIGINS:
        handler.send_header("access-control-allow-origin", origin)
        handler.send_header("vary", "origin")
        handler.send_header("access-control-allow-methods", "POST, OPTIONS")
        handler.send_header("access-control-allow-headers", "authorization, content-type")


def _object(value: Any) -> dict[str, Any] | None:
    return value if isinstance(value, dict) else None


def _text(value: Any, limit: int = 512) -> str | None:
    if not isinstance(value, str):
        return None
    stripped = value.strip()
    return stripped[:limit] if stripped else None


def _captured_at(snapshot: dict[str, Any]) -> datetime:
    raw = _text(snapshot.get("capturedAt"))
    if raw:
        try:
            return datetime.fromisoformat(raw.replace("Z", "+00:00"))
        except ValueError:
            pass
    return datetime.now(timezone.utc)


def _authorized(header: str | None) -> bool:
    prefix = "Bearer "
    if not header or not header.startswith(prefix):
        return False
    return hmac.compare_digest(header[len(prefix) :], INGEST_TOKEN)


def _insert_snapshot(body: dict[str, Any]) -> dict[str, Any]:
    snapshot = _object(body.get("snapshot")) or body
    context = _object(snapshot.get("context")) or {}
    session_id = _text(snapshot.get("sessionId")) or _text(context.get("sessionId")) or "unknown"
    subsystem = _text(snapshot.get("subsystem"), 128) or "manual"
    source = _text(body.get("source"), 128) or "web"
    page_url = _text(body.get("pageUrl"), 2048) or _text(snapshot.get("url"), 2048)
    user_agent = _text(body.get("userAgent"), 1024)

    with psycopg.connect(DATABASE_URL) as conn:
        with conn.cursor() as cur:
            cur.execute("SET statement_timeout = 5000")
            cur.execute(
                """
                insert into snapshots (captured_at, session_id, subsystem, source, page_url, user_agent, payload)
                values (%s, %s, %s, %s, %s, %s, %s::jsonb)
                returning id, captured_at
                """,
                (
                    _captured_at(snapshot),
                    session_id,
                    subsystem,
                    source,
                    page_url,
                    user_agent,
                    json.dumps(snapshot, ensure_ascii=False, default=str),
                ),
            )
            row = cur.fetchone()
            conn.commit()

    return {
        "ok": True,
        "id": row[0],
        "capturedAt": row[1],
        "sessionId": session_id,
        "subsystem": subsystem,
        "source": source,
    }


class Handler(BaseHTTPRequestHandler):
    server_version = "mir2-debug-ingest/1"

    def do_OPTIONS(self) -> None:  # noqa: N802 - stdlib naming
        self.send_response(HTTPStatus.NO_CONTENT)
        _cors_headers(self)
        self.send_header("cache-control", "no-store")
        self.end_headers()

    def do_POST(self) -> None:  # noqa: N802 - stdlib naming
        if urlparse(self.path).path != PATH:
            _json_response(self, HTTPStatus.NOT_FOUND, {"ok": False, "error": "not found"})
            return
        if not _authorized(self.headers.get("authorization")):
            _json_response(self, HTTPStatus.UNAUTHORIZED, {"ok": False, "error": "unauthorized"})
            return

        try:
            content_length = int(self.headers.get("content-length") or "0")
        except ValueError:
            _json_response(self, HTTPStatus.BAD_REQUEST, {"ok": False, "error": "invalid content length"})
            return
        if content_length <= 0 or content_length > MAX_BODY_BYTES:
            _json_response(self, HTTPStatus.REQUEST_ENTITY_TOO_LARGE, {"ok": False, "error": "payload too large"})
            return

        raw_body = self.rfile.read(content_length)
        try:
            body = _object(json.loads(raw_body.decode("utf-8")))
        except (UnicodeDecodeError, json.JSONDecodeError):
            body = None
        if not body:
            _json_response(self, HTTPStatus.BAD_REQUEST, {"ok": False, "error": "payload must be a JSON object"})
            return

        try:
            _json_response(self, HTTPStatus.OK, _insert_snapshot(body))
        except Exception as error:  # pragma: no cover - surfaced through systemd logs.
            _json_response(self, HTTPStatus.INTERNAL_SERVER_ERROR, {"ok": False, "error": str(error)})


def main() -> None:
    server = ThreadingHTTPServer((HOST, PORT), Handler)
    print(f"Mir2 debug ingest listening on http://{HOST}:{PORT}{PATH}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
