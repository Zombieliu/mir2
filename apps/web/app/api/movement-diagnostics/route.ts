import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import { NextResponse } from "next/server";

import { requestFileWritesDisabled } from "../../../lib/deployment-env";

type MovementDiagnosticsBatch = {
  sessionId?: unknown;
  startedAt?: unknown;
  reason?: unknown;
  pageUrl?: unknown;
  userAgent?: unknown;
  events?: unknown;
};

const OUTPUT_DIR = path.resolve(
  process.cwd(),
  "../../docs/generated/player-qa/movement-diagnostics",
);

function safeSessionId(value: unknown) {
  const raw = typeof value === "string" && value.trim() ? value.trim() : `manual-${Date.now()}`;
  return raw.replace(/[^a-zA-Z0-9._-]/g, "_").slice(0, 96);
}

export async function POST(request: Request) {
  const body = (await request.json().catch(() => ({}))) as MovementDiagnosticsBatch;
  const sessionId = safeSessionId(body.sessionId);
  const events = Array.isArray(body.events) ? body.events : [];
  if (!events.length) {
    return NextResponse.json({ ok: true, sessionId, eventCount: 0 });
  }

  if (requestFileWritesDisabled()) {
    return NextResponse.json({
      ok: true,
      disabled: true,
      sessionId,
      eventCount: events.length,
    });
  }

  await mkdir(OUTPUT_DIR, { recursive: true });

  const receivedAt = Date.now();
  const batch = {
    sessionId,
    receivedAt,
    startedAt: typeof body.startedAt === "number" ? body.startedAt : null,
    reason: typeof body.reason === "string" ? body.reason : "manual",
    pageUrl: typeof body.pageUrl === "string" ? body.pageUrl : null,
    userAgent: typeof body.userAgent === "string" ? body.userAgent : null,
    eventCount: events.length,
    events,
  };
  const sessionPath = path.join(OUTPUT_DIR, `${sessionId}.jsonl`);
  await appendFile(sessionPath, `${JSON.stringify(batch)}\n`, "utf8");
  await writeFile(path.join(OUTPUT_DIR, "latest.json"), `${JSON.stringify(batch, null, 2)}\n`, "utf8");
  await writeFile(path.join(OUTPUT_DIR, "latest-session.txt"), `${sessionId}\n`, "utf8");

  return NextResponse.json({
    ok: true,
    sessionId,
    eventCount: events.length,
    path: sessionPath,
  });
}

export async function GET() {
  if (requestFileWritesDisabled()) {
    return NextResponse.json(
      { ok: false, disabled: true, error: "movement diagnostics are disabled in this deployment" },
      { status: 404 },
    );
  }

  try {
    const latest = await readFile(path.join(OUTPUT_DIR, "latest.json"), "utf8");
    return new NextResponse(latest, {
      headers: { "content-type": "application/json; charset=utf-8" },
    });
  } catch {
    return NextResponse.json({ ok: false, error: "no movement diagnostics captured" }, { status: 404 });
  }
}
