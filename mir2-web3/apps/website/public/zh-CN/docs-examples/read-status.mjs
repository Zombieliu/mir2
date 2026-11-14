// Numeron read-only example. Node.js >= 22; no packages or credentials required.
const origin = new URL(process.argv[2] ?? "http://127.0.0.1:3080");
if (!["http:", "https:"].includes(origin.protocol) || origin.username || origin.password) {
  throw new Error("Provide an HTTP(S) website origin without credentials.");
}
const endpoint = new URL("/api/game-status", origin);
try {
  const response = await fetch(endpoint, {
    signal: AbortSignal.timeout(6000),
    redirect: "error",
    headers: { accept: "application/json" },
  });
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  const status = await response.json();
  if (
    typeof status.ok !== "boolean" ||
    typeof status.checkedAt !== "string" ||
    !Number.isFinite(Date.parse(status.checkedAt)) ||
    typeof status.latencyMs !== "number" ||
    !Number.isFinite(status.latencyMs) || status.latencyMs < 0 ||
    status.source !== "game-health"
  ) throw new Error("Unexpected status response; check API compatibility.");
  console.log(JSON.stringify({
    endpoint: endpoint.href,
    state: status.ok ? "healthy" : "degraded",
    checkedAt: status.checkedAt,
    latencyMs: status.latencyMs,
    scope: "Configured game-health probe only; not a login or payment check.",
  }, null, 2));
} catch (error) {
  console.error(`Status read failed: ${error.message}`);
  process.exitCode = 1;
}
