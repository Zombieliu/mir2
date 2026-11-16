import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { once } from "node:events";
import { createServer } from "node:http";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import test from "node:test";

const execute = promisify(execFile);
const example = fileURLToPath(new URL("../public/zh-CN/docs-examples/read-status.mjs", import.meta.url));
const valid = { ok: true, checkedAt: "2026-11-02T00:00:00.000Z", latencyMs: 12, source: "game-health" };

async function fixture(handler, action) {
  const server = createServer(handler).listen(0, "127.0.0.1");
  await once(server, "listening");
  try {
    await action(`http://127.0.0.1:${server.address().port}`);
  } finally {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
}

for (const [ok, state] of [[true, "healthy"], [false, "degraded"]]) {
  test(`real CLI reads ${state} without credentials or writes`, async () => {
    await fixture((request, response) => {
      assert.equal(request.method, "GET");
      assert.equal(request.url, "/api/game-status");
      assert.equal(request.headers.authorization, undefined);
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify({ ...valid, ok, extraField: "ignored" }));
    }, async (origin) => {
      const { stdout, stderr } = await execute(process.execPath, [example, origin]);
      assert.equal(stderr, "");
      assert.equal(JSON.parse(stdout).state, state);
    });
  });
}

test("CLI rejects non-success HTTP", async () => {
  await fixture((_, response) => { response.writeHead(503).end("unavailable"); }, async (origin) => {
    await assert.rejects(execute(process.execPath, [example, origin]), (error) => error.code === 1 && error.stderr.includes("HTTP 503"));
  });
});

test("CLI rejects redirects instead of following to a different service", async () => {
  await fixture((_, response) => { response.writeHead(302, { location: "/unexpected" }).end(); }, async (origin) => {
    await assert.rejects(execute(process.execPath, [example, origin]), (error) => error.code === 1);
  });
});

test("CLI rejects incompatible schema", async () => {
  await fixture((_, response) => { response.end(JSON.stringify({ ...valid, ok: "true" })); }, async (origin) => {
    await assert.rejects(execute(process.execPath, [example, origin]), (error) => error.code === 1 && error.stderr.includes("compatibility"));
  });
});

test("CLI rejects non-JSON", async () => {
  await fixture((_, response) => { response.end("<html>not the API</html>"); }, async (origin) => {
    await assert.rejects(execute(process.execPath, [example, origin]), (error) => error.code === 1);
  });
});

test("CLI has a bounded timeout", async () => {
  await fixture(() => {}, async (origin) => {
    await assert.rejects(execute(process.execPath, [example, origin], { timeout: 12000 }), (error) => error.code === 1 && !error.killed);
  });
});
