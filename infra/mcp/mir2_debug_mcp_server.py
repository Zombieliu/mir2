#!/usr/bin/env python3
"""Remote MCP server for Mir2 debug snapshots.

This server intentionally exposes only the isolated `mir2_debug.snapshots`
table. It is designed for Claude Custom Connectors / remote MCP clients and
uses a read-only Postgres role in production.
"""

from __future__ import annotations

import json
import os
import re
from typing import Any

import psycopg
from mcp.server.fastmcp import FastMCP
from psycopg.rows import dict_row


DATABASE_URL = os.environ["MIR2_DEBUG_RO_DATABASE_URL"]
MAX_ROWS = int(os.environ.get("MIR2_DEBUG_MCP_MAX_ROWS", "50"))
MAX_TEXT_CHARS = int(os.environ.get("MIR2_DEBUG_MCP_MAX_TEXT_CHARS", "120000"))
STATEMENT_TIMEOUT_MS = int(os.environ.get("MIR2_DEBUG_MCP_STATEMENT_TIMEOUT_MS", "5000"))

HOST = os.environ.get("MIR2_DEBUG_MCP_HOST", "127.0.0.1")
PORT = int(os.environ.get("MIR2_DEBUG_MCP_PORT", "7125"))
PATH = os.environ.get("MIR2_DEBUG_MCP_PATH", "/mcp")

FORBIDDEN_SQL = re.compile(
    r"\b(insert|update|delete|drop|alter|create|truncate|copy|grant|revoke|"
    r"vacuum|analyze|call|execute|prepare|deallocate|listen|notify|set)\b",
    re.IGNORECASE,
)


mcp = FastMCP(
    "Mir2 Debug DB",
    instructions=(
        "Read-only access to Mir2 debug snapshots. The data source is the "
        "isolated mir2_debug Postgres database, not the gameplay database."
    ),
    host=HOST,
    port=PORT,
    streamable_http_path=PATH,
    stateless_http=True,
    json_response=True,
)


def _connect() -> psycopg.Connection:
    conn = psycopg.connect(DATABASE_URL, row_factory=dict_row)
    conn.execute(f"SET statement_timeout = {STATEMENT_TIMEOUT_MS}")
    conn.execute("SET default_transaction_read_only = on")
    return conn


def _json(value: Any) -> str:
    text = json.dumps(value, ensure_ascii=False, default=str, indent=2)
    if len(text) > MAX_TEXT_CHARS:
        return text[:MAX_TEXT_CHARS] + "\n... truncated ..."
    return text


def _clean_limit(limit: int | None, default: int = 20) -> int:
    if limit is None:
        return default
    return max(1, min(int(limit), MAX_ROWS))


def _query(sql: str, params: tuple[Any, ...] = ()) -> list[dict[str, Any]]:
    with _connect() as conn:
        with conn.cursor() as cur:
            cur.execute(sql, params)
            return list(cur.fetchall())


@mcp.tool()
def count_snapshots() -> str:
    """Return the total number of stored debug snapshots."""
    rows = _query("select count(*) as snapshots from snapshots")
    return _json(rows[0])


@mcp.tool()
def list_recent_snapshots(limit: int = 10) -> str:
    """List recent snapshot metadata without returning the full JSON payload."""
    rows = _query(
        """
        select id, captured_at, session_id, subsystem, source, page_url, user_agent
        from snapshots
        order by captured_at desc
        limit %s
        """,
        (_clean_limit(limit, 10),),
    )
    return _json(rows)


@mcp.tool()
def get_snapshot(snapshot_id: int, include_payload: bool = True) -> str:
    """Fetch one snapshot by id."""
    if include_payload:
        sql = """
            select id, captured_at, session_id, subsystem, source, page_url,
                   user_agent, payload
            from snapshots
            where id = %s
        """
    else:
        sql = """
            select id, captured_at, session_id, subsystem, source, page_url,
                   user_agent
            from snapshots
            where id = %s
        """
    rows = _query(sql, (int(snapshot_id),))
    return _json(rows[0] if rows else {"error": "snapshot not found"})


@mcp.tool()
def find_snapshots_by_session(session_id: str, limit: int = 20) -> str:
    """List snapshots for a session id, newest first."""
    rows = _query(
        """
        select id, captured_at, session_id, subsystem, source, page_url
        from snapshots
        where session_id = %s
        order by captured_at desc
        limit %s
        """,
        (session_id, _clean_limit(limit, 20)),
    )
    return _json(rows)


@mcp.tool()
def summarize_subsystems() -> str:
    """Count snapshots by subsystem."""
    rows = _query(
        """
        select coalesce(subsystem, '(unset)') as subsystem, count(*) as snapshots
        from snapshots
        group by coalesce(subsystem, '(unset)')
        order by snapshots desc, subsystem asc
        """
    )
    return _json(rows)


@mcp.tool()
def search_payload_text(term: str, limit: int = 20) -> str:
    """Search serialized payload text for a case-insensitive term."""
    term = term.strip()
    if not term:
        return _json({"error": "term is required"})
    rows = _query(
        """
        select id, captured_at, session_id, subsystem, source, page_url
        from snapshots
        where payload::text ilike '%' || %s || '%'
        order by captured_at desc
        limit %s
        """,
        (term, _clean_limit(limit, 20)),
    )
    return _json(rows)


@mcp.tool()
def run_readonly_sql(sql: str, max_rows: int = 20) -> str:
    """Run a restricted read-only SQL query against the snapshots table."""
    statement = sql.strip()
    lowered = statement.lower()
    if ";" in statement.rstrip(";"):
        return _json({"error": "only one SQL statement is allowed"})
    statement = statement.rstrip(";").strip()
    lowered = statement.lower()
    if not (lowered.startswith("select ") or lowered.startswith("with ")):
        return _json({"error": "only SELECT/WITH queries are allowed"})
    if FORBIDDEN_SQL.search(statement):
        return _json({"error": "mutating SQL is not allowed"})
    if "snapshots" not in lowered:
        return _json({"error": "queries must reference the snapshots table"})

    with _connect() as conn:
        with conn.cursor() as cur:
            cur.execute("begin read only")
            cur.execute(statement)
            rows = cur.fetchmany(_clean_limit(max_rows, 20) + 1)
            truncated = len(rows) > _clean_limit(max_rows, 20)
            rows = rows[: _clean_limit(max_rows, 20)]
            return _json({"rows": rows, "truncated": truncated})


if __name__ == "__main__":
    mcp.run(transport="streamable-http")
