const gameUrl = (process.env.NEXT_PUBLIC_GAME_URL ?? "https://mir2.obelisk.build").replace(/\/$/, "");
const healthUrl = process.env.GAME_HEALTH_URL ?? `${gameUrl}/health`;

export async function GET() {
  const startedAt = Date.now();
  try {
    const response = await fetch(healthUrl, { cache: "no-store", signal: AbortSignal.timeout(3500) });
    return Response.json({ ok: response.ok, checkedAt: new Date().toISOString(), latencyMs: Date.now() - startedAt, source: "game-health" }, { headers: { "cache-control": "no-store" } });
  } catch {
    return Response.json({ ok: false, checkedAt: new Date().toISOString(), latencyMs: Date.now() - startedAt, source: "game-health" }, { headers: { "cache-control": "no-store" } });
  }
}
