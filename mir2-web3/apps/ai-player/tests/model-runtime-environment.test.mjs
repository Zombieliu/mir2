import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { loadLocalModelNetwork, modelProcessEnvironment } from "../src/model-runtime-environment.mjs";

async function fixture(t) {
  const root = await mkdtemp(join(tmpdir(), "alo-model-network-"));
  t.after(async () => { assert.ok(resolve(root).startsWith(resolve(tmpdir()) + sep) && basename(root).startsWith("alo-model-network-")); await rm(root, { recursive: true, force: true }); });
  const path = join(root, "apps/ai-player/.data/local-playtest/openai-network.local.json"); await mkdir(resolve(path, ".."), { recursive: true });
  return { root, save: value => writeFile(path, JSON.stringify(value)) };
}
test("Actor environment forwards only explicit model key and standard proxy variables", () => {
  const selected = modelProcessEnvironment("sk-synthetic-only", { HTTPS_PROXY: "http://user:synthetic-pass@proxy.example:8080", no_proxy: "localhost,127.0.0.1", MIR2_NATIVE_PASSWORD: "synthetic-auth", AI_GATEWAY_API_KEY: "synthetic-other", NODE_TLS_REJECT_UNAUTHORIZED: "0" });
  assert.deepEqual(selected, { OPENAI_API_KEY: "sk-synthetic-only", HTTPS_PROXY: "http://user:synthetic-pass@proxy.example:8080", no_proxy: "localhost,127.0.0.1" });
  assert.throws(() => modelProcessEnvironment(undefined, { HTTPS_PROXY: "http://x\nheader" }), { code: "LOCAL_MODEL_NETWORK_UNAVAILABLE" });
});
test("missing network settings use only process variables, while explicit environment overrides saved routing", async t => {
  const f = await fixture(t); assert.deepEqual(await loadLocalModelNetwork({ bundleDir: f.root, environment: {} }), {});
  await f.save({ schemaVersion: 1, proxies: { HTTPS_PROXY: "http://127.0.0.1:12345", NO_PROXY: "localhost,127.0.0.1" } });
  assert.deepEqual(await loadLocalModelNetwork({ bundleDir: f.root, environment: { HTTPS_PROXY: "http://proxy.example:80", unrelated: "private" } }), { HTTPS_PROXY: "http://proxy.example:80", NO_PROXY: "localhost,127.0.0.1" });
});
test("persisted network metadata rejects credentials, extra authority and oversize content without exposing values", async t => {
  const f = await fixture(t);
  for (const settings of [
    { schemaVersion: 1, proxies: { HTTPS_PROXY: "http://user:synthetic-secret@proxy.example:80" } },
    { schemaVersion: 1, proxies: { API_KEY: "synthetic-secret" } },
    { schemaVersion: 1, proxies: { HTTPS_PROXY: "http://proxy.example/?key=synthetic-secret" } },
    { schemaVersion: 1, proxies: { HTTPS_PROXY: "socks5://proxy.example:80" } },
    { schemaVersion: 1, proxies: {}, NODE_TLS_REJECT_UNAUTHORIZED: "0" },
    { schemaVersion: 1, proxies: { NO_PROXY: "x".repeat(16385) } },
  ]) { await f.save(settings); await assert.rejects(loadLocalModelNetwork({ bundleDir: f.root, environment: {} }), error => { assert.equal(error.code, "LOCAL_MODEL_NETWORK_UNAVAILABLE"); assert.equal(String(error).includes("synthetic-secret"), false); return true; }); }
});
