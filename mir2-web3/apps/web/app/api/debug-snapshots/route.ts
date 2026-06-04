import { NextResponse } from "next/server";

const MAX_BODY_CHARS = 900_000;

function configuredValue(name: string) {
  const value = process.env[name]?.trim();
  return value ? value : null;
}

function objectValue(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function stringValue(value: unknown) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

export async function POST(request: Request) {
  const ingestUrl = configuredValue("MIR2_DEBUG_SNAPSHOT_INGEST_URL");
  const ingestToken = configuredValue("MIR2_DEBUG_SNAPSHOT_INGEST_TOKEN");
  if (!ingestUrl || !ingestToken) {
    return NextResponse.json(
      {
        ok: false,
        uploaded: false,
        disabled: true,
        error: "debug snapshot ingest is not configured",
      },
      { status: 503 },
    );
  }

  const rawBody = await request.text();
  if (rawBody.length > MAX_BODY_CHARS) {
    return NextResponse.json(
      {
        ok: false,
        uploaded: false,
        error: "debug snapshot payload is too large",
      },
      { status: 413 },
    );
  }

  let parsedJson: unknown;
  try {
    parsedJson = JSON.parse(rawBody || "{}");
  } catch {
    return NextResponse.json(
      {
        ok: false,
        uploaded: false,
        error: "debug snapshot payload must be valid JSON",
      },
      { status: 400 },
    );
  }

  const parsed = objectValue(parsedJson);
  if (!parsed) {
    return NextResponse.json(
      {
        ok: false,
        uploaded: false,
        error: "debug snapshot payload must be an object",
      },
      { status: 400 },
    );
  }

  const snapshot = objectValue(parsed.snapshot) ?? parsed;
  const pageUrl = stringValue(parsed.pageUrl) ?? stringValue(snapshot.url);
  const userAgent = stringValue(parsed.userAgent) ?? request.headers.get("user-agent");
  const source = stringValue(parsed.source) ?? "web";

  let upstreamResponse: Response;
  try {
    upstreamResponse = await fetch(ingestUrl, {
      method: "POST",
      headers: {
        authorization: `Bearer ${ingestToken}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        source,
        pageUrl,
        userAgent,
        snapshot,
      }),
      cache: "no-store",
    });
  } catch (error) {
    return NextResponse.json(
      {
        ok: false,
        uploaded: false,
        error: error instanceof Error ? error.message : String(error),
      },
      { status: 502 },
    );
  }

  const upstreamBody = (await upstreamResponse.json().catch(() => ({}))) as Record<string, unknown>;
  if (!upstreamResponse.ok || upstreamBody.ok !== true) {
    return NextResponse.json(
      {
        ok: false,
        uploaded: false,
        status: upstreamResponse.status,
        error: stringValue(upstreamBody.error) ?? "debug snapshot ingest failed",
      },
      { status: 502 },
    );
  }

  return NextResponse.json(
    {
      ok: true,
      uploaded: true,
      id: upstreamBody.id ?? null,
      capturedAt: upstreamBody.capturedAt ?? null,
      sessionId: upstreamBody.sessionId ?? null,
    },
    {
      headers: {
        "cache-control": "no-store",
      },
    },
  );
}
