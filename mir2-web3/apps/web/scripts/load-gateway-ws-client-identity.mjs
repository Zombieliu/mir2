import net from "node:net";

const MAX_SIMULATED_CLIENT_INDEX = 65_535;

export function isLoopbackWebSocketUrl(rawUrl) {
  const url = new URL(rawUrl);
  if (url.protocol !== "ws:" && url.protocol !== "wss:") return false;
  if (url.hostname.toLowerCase() === "localhost") return true;
  const hostname = url.hostname.replace(/^\[|\]$/g, "");
  const address = net.isIP(hostname) ? hostname : null;
  return address === "127.0.0.1" || address === "::1";
}

export function loadHarnessClientIdentity({
  index,
  runId,
  simulateDistinctClients,
  wsUrl,
}) {
  if (!Number.isSafeInteger(index) || index < 0) {
    throw new Error("load client index must be a non-negative safe integer");
  }
  if (!simulateDistinctClients) {
    return { userAgent: "mir2-ws-load/1", clientIp: null };
  }
  if (!isLoopbackWebSocketUrl(wsUrl)) {
    throw new Error("distinct-client identity simulation is restricted to a loopback WebSocket URL");
  }
  if (index > MAX_SIMULATED_CLIENT_INDEX) {
    throw new Error(`distinct-client index exceeds ${MAX_SIMULATED_CLIENT_INDEX}`);
  }
  const third = Math.floor(index / 256);
  const fourth = index % 256;
  const safeRunId = String(runId).replace(/[^a-zA-Z0-9._-]/g, "_").slice(0, 64);
  return {
    userAgent: `mir2-ws-load-distinct/${safeRunId}/${index}`,
    clientIp: `198.18.${third}.${fourth}`,
  };
}

export function assertSafeHandshakeHeaderValue(value, label) {
  const text = String(value);
  if ([...text].some((character) => character.charCodeAt(0) <= 0x1f || character.charCodeAt(0) === 0x7f)) {
    throw new Error(`${label} contains a forbidden ASCII control character`);
  }
  return text;
}

export function loadAccountId(prefix, index, indexWidth = 0) {
  if (!Number.isSafeInteger(index) || index < 0) {
    throw new Error("load account index must be a non-negative safe integer");
  }
  if (!Number.isSafeInteger(indexWidth) || indexWidth < 0 || indexWidth > 12) {
    throw new Error("load account index width must be an integer between 0 and 12");
  }
  const suffix = indexWidth === 0 ? String(index) : String(index).padStart(indexWidth, "0");
  return `${prefix}${suffix}`;
}
