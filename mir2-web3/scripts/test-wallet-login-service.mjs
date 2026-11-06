import assert from "node:assert/strict";
import { createHash, randomBytes, randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { buildEnvironment, inspectPayload, requireChild } from "./package-wallet-login-release.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

test("wallet service imports the verified shared handlers and exposes only required routes", async () => {
  const app = path.join(root, "apps/wallet-login/app");
  for (const [route, symbol, source] of [
    ["api/wallet/dubhe/session/route.ts", "createDubheConnectLogin", "web/lib/server/dubhe-connect-auth"],
    ["api/wallet/dubhe/session/status/route.ts", "readDubheConnectLogin", "web/lib/server/dubhe-connect-auth"],
    ["api/channels/session/exchange/route.ts", "POST", "web/app/api/channels/session/exchange/route"],
  ]) {
    const text = await readFile(path.join(app, route), "utf8");
    assert.ok(text.includes(symbol));
    const reference = text.match(/from "([^"]+)"/)[1];
    assert.equal(path.resolve(path.dirname(path.join(app, route)), reference), path.join(root, "apps", source));
    assert.equal(text.includes("createHmac"), false);
  }
  const health = await readFile(path.join(app, "health/route.ts"), "utf8");
  assert.ok(health.includes("MIR2_WALLET_LOGIN_BUILD_REVISION"));
  assert.equal(health.includes("SECRET"), false);
});

test("build environment excludes authentication and deployment credentials", () => {
  const environment = buildEnvironment({ PATH: "tools", SystemRoot: "system", NODE_OPTIONS: "injected", MIR2_PASSKEY_AUTH_SECRET: "private", AWS_SECRET_ACCESS_KEY: "private", NEXT_PUBLIC_TOKEN: "private" }, "a".repeat(40));
  assert.deepEqual(environment, { PATH: "tools", SystemRoot: "system", NODE_ENV: "production", NEXT_TELEMETRY_DISABLED: "1", MIR2_WALLET_LOGIN_BUILD_REVISION: "a".repeat(40) });
  assert.throws(() => requireChild(root, root));
  assert.throws(() => requireChild(root, path.resolve(root, "../elsewhere")));
});

test("payload hashes actual bytes and rejects secrets and game media", async () => {
  const temporary = await mkdtemp(path.join(os.tmpdir(), "mir2-wallet-package-test-"));
  try {
    await writeFile(path.join(temporary, "server.js"), "verified bytes");
    const files = await inspectPayload(temporary);
    assert.deepEqual(files, [{ path: "server.js", sizeBytes: 14, sha256: createHash("sha256").update("verified bytes").digest("hex") }]);
    await writeFile(path.join(temporary, ".env.production"), "fixture only");
    await assert.rejects(inspectPayload(temporary), /private environment/);
    await rm(path.join(temporary, ".env.production"));
    await mkdir(path.join(temporary, "mir2-web3/apps/web/public"), { recursive: true });
    await assert.rejects(inspectPayload(temporary), /game assets/);
  } finally {
    requireChild(os.tmpdir(), temporary);
    await rm(temporary, { recursive: true, force: true });
  }
});

test("payload refuses a link before following it", async () => {
  const temporary = await mkdtemp(path.join(os.tmpdir(), "mir2-wallet-package-link-"));
  try {
    await symlink(os.tmpdir(), path.join(temporary, "escape"), process.platform === "win32" ? "junction" : "dir");
    await assert.rejects(inspectPayload(temporary), /link or special/);
  } finally {
    requireChild(os.tmpdir(), temporary);
    await rm(temporary, { recursive: true, force: true });
  }
});

test("built standalone service handles origin, pending, invalid proof and unknown paths", {
  skip: process.env.MIR2_WALLET_LOGIN_SMOKE_STANDALONE !== "1",
  timeout: 45_000,
}, async () => {
  let envelope;
  let exchanged = 0;
  const upstream = http.createServer(async (request, response) => {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    const body = Buffer.concat(chunks).toString("utf8");
    response.setHeader("content-type", "application/json");
    if (request.method === "POST" && request.url === "/sessions") {
      const source = JSON.parse(body);
      const id = randomUUID();
      const requestedAt = new Date().toISOString();
      const expiresAt = new Date(Date.parse(requestedAt) + 120_000).toISOString();
      const relayOrigin = `http://127.0.0.1:${upstream.address().port}`;
      envelope = {
        id, relayOrigin, status: "pending",
        approvalUrl: `${source.walletOrigin}/connect/approve?session=${id}&relay=${encodeURIComponent(relayOrigin)}`,
        pollUrl: `${relayOrigin}/sessions/${id}`,
        request: {
          appName: source.appName, origin: source.origin, network: source.network,
          walletOrigin: source.walletOrigin, statement: source.statement,
          nonce: randomUUID(), requestedAt, expiresAt,
        },
      };
      response.end(JSON.stringify(envelope));
    } else if (request.method === "GET" && request.url === `/sessions/${envelope?.id}`) {
      response.end(JSON.stringify(envelope));
    } else if (request.method === "POST" && request.url === "/v1/channels/session/exchange") {
      assert.deepEqual(JSON.parse(body), { provider: "suiWallet", credential: "invalid-fixture" });
      exchanged += 1;
      response.statusCode = 401;
      response.end(JSON.stringify({ error: "fixture proof rejected" }));
    } else {
      response.statusCode = 404;
      response.end("{}");
    }
  });
  upstream.listen(0, "127.0.0.1");
  await once(upstream, "listening");
  const reservation = http.createServer();
  reservation.listen(0, "127.0.0.1");
  await once(reservation, "listening");
  const port = reservation.address().port;
  await new Promise((resolve) => reservation.close(resolve));
  const origin = `http://127.0.0.1:${port}`;
  const upstreamOrigin = `http://127.0.0.1:${upstream.address().port}`;
  const serverEntry = path.join(root, "apps/wallet-login/.next/standalone/mir2-web3/apps/wallet-login/server.js");
  const built = JSON.parse(await readFile(path.join(root, "apps/wallet-login/.next/required-server-files.json"), "utf8"));
  const environment = buildEnvironment(process.env, "0".repeat(40));
  const child = spawn(process.execPath, [serverEntry], {
    env: {
      ...environment, HOSTNAME: "127.0.0.1", PORT: String(port), MIR2_ENV: "staging",
      MIR2_PASSKEY_AUTH_SECRET: randomBytes(32).toString("hex"),
      MIR2_PASSKEY_ALLOWED_ORIGINS: origin,
      MIR2_DUBHE_CONNECT_RELAY_URL: upstreamOrigin,
      MIR2_GATEWAY_HTTP_URL: upstreamOrigin,
    },
    stdio: "ignore", windowsHide: true,
  });
  try {
    let health;
    const deadline = Date.now() + 30_000;
    while (Date.now() < deadline) {
      if (child.exitCode !== null) throw new Error("standalone service exited before readiness");
      try {
        const response = await fetch(`${origin}/health`, { signal: AbortSignal.timeout(500) });
        if (response.ok) { health = await response.json(); break; }
      } catch { /* bounded process startup */ }
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert.deepEqual(Object.keys(health ?? {}).sort(), ["ready", "revision"]);
    assert.equal(health.ready, true);
    assert.match(health.revision, /^[a-f0-9]{40}$/);
    assert.equal(health.revision, built.config.env.MIR2_WALLET_LOGIN_BUILD_REVISION, "health must report the built source revision rather than a runtime override");
    assert.equal((await fetch(origin)).status, 404);
    assert.equal((await fetch(`${origin}/.env`)).status, 404);
    const create = (headers, body) => fetch(`${origin}/api/wallet/dubhe/session`, { method: "POST", headers, body });
    assert.equal((await create({ Origin: "https://untrusted.invalid" })).status, 403);
    assert.equal((await create({ Origin: origin, "content-type": "application/json" }, '{"network":"mainnet"}')).status, 400);
    const created = await create({ Origin: origin });
    assert.equal(created.status, 200);
    assert.equal(created.headers.get("cache-control"), "no-store");
    const session = await created.json();
    assert.equal(new URL(session.approvalUrl).origin, "https://wallet.dubhe.obelisk.build");
    const pending = await fetch(`${origin}/api/wallet/dubhe/session/status`, {
      method: "POST", headers: { Origin: origin, Authorization: `Bearer ${session.intentToken}` },
    });
    assert.equal(pending.status, 200);
    assert.deepEqual(await pending.json(), { status: "pending" });
    assert.equal((await fetch(`${origin}/api/wallet/dubhe/session/status`, {
      method: "POST", headers: { Origin: origin, Authorization: "Bearer invalid" },
    })).status, 401);
    const rejected = await fetch(`${origin}/api/channels/session/exchange`, {
      method: "POST", headers: { "content-type": "application/json" },
      body: JSON.stringify({ provider: "suiWallet", credential: "invalid-fixture" }),
    });
    assert.equal(rejected.status, 401);
    assert.equal(exchanged, 1);
  } finally {
    child.kill();
    if (child.exitCode === null) await once(child, "exit");
    await new Promise((resolve) => upstream.close(resolve));
  }
});
