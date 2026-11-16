import test from "node:test";
import assert from "node:assert/strict";
import childProcess from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { loadLocalModelSettings } from "../src/local-model-settings.mjs";

// These are synthetic strings, never usable credentials.
const KEY = "sk-synthetic-test-only-not-a-real-api-key";
const OTHER_KEY = "sk-other-synthetic-test-only-not-a-real-api-key";
const TARGET = `NumeronAlo/OpenAI/v1/${"a".repeat(64)}`;
const FIXED_ERROR = "Local OpenAI settings are invalid or unavailable.";
const metadata = () => ({ schemaVersion: 1, provider: "openai", enabled: true, model: "gpt-6-luna", credentialTarget: TARGET, maxCallsPerHour: 20 });

async function fixture(t, settings) {
  const bundleDir = await mkdtemp(join(tmpdir(), "alo-model-settings-"));
  t.after(async () => {
    assert.equal(dirname(resolve(bundleDir)), resolve(tmpdir()));
    assert.ok(basename(bundleDir).startsWith("alo-model-settings-"));
    await rm(bundleDir, { recursive: true, force: true });
  });
  const path = join(bundleDir, "apps/ai-player/.data/local-playtest/openai.local.json");
  if (settings !== undefined) {
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, typeof settings === "string" ? settings : JSON.stringify(settings));
  }
  return { bundleDir, path };
}

function helper(t, reply = { credentialTarget: TARGET, apiKey: KEY }, failure = null) {
  const calls = [];
  t.mock.method(childProcess, "execFile", (file, args, options, callback) => {
    const call = { file, args, options, stdinClosed: false }; calls.push(call);
    queueMicrotask(() => callback(failure, typeof reply === "string" ? reply : JSON.stringify(reply), "ignored-private-stderr"));
    return { stdin: { end() { call.stdinClosed = true; } } };
  });
  return calls;
}

function sanitized(error) {
  assert.equal(error.message, FIXED_ERROR);
  assert.equal(error.code, "LOCAL_MODEL_SETTINGS_UNAVAILABLE");
  assert.equal(error.cause, undefined);
  assert.ok(!String(error).includes(KEY));
  assert.ok(!String(error).includes(OTHER_KEY));
  return true;
}

test("unconfigured settings remain rules and never borrow Codex or AI Gateway credentials", async t => {
  const f = await fixture(t);
  const calls = helper(t);
  const value = await loadLocalModelSettings({ bundleDir: f.bundleDir, env: { CODEX_API_KEY: KEY, AI_GATEWAY_API_KEY: KEY, OPENAI_MODEL: "unrequested-model" } });
  assert.deepEqual(value, { provider: "rules", model: "rules", credentialRef: null });
  assert.equal(calls.length, 0);
});

test("only explicit OPENAI_API_KEY may enable the no-file fallback with fixed defaults", async t => {
  const f = await fixture(t);
  const calls = helper(t);
  const value = await loadLocalModelSettings({ bundleDir: f.bundleDir, env: { OPENAI_API_KEY: ` ${KEY}\n`, OPENAI_MODEL: "ignored" } });
  assert.equal(value.provider, "openai"); assert.equal(value.model, "gpt-6-luna");
  assert.equal(value.apiKey === KEY, true); assert.equal(value.credentialRef, "OPENAI_API_KEY");
  assert.equal(value.maxCallsPerHour, 20); assert.equal(calls.length, 0);
  await assert.rejects(readFile(f.path), { code: "ENOENT" });
});

test("local metadata wins and the helper uses only a fixed bounded hidden pipe", async t => {
  const configured = metadata(); configured.model = "gpt-6.1-terra";
  const f = await fixture(t, configured);
  const calls = helper(t);
  const value = await loadLocalModelSettings({ bundleDir: f.bundleDir, env: { OPENAI_API_KEY: OTHER_KEY, CODEX_API_KEY: OTHER_KEY, AI_GATEWAY_API_KEY: OTHER_KEY, SystemRoot: "C:\\Windows" } });
  assert.equal(value.apiKey === KEY, true); assert.equal(value.model, "gpt-6.1-terra");
  assert.equal(calls.length, 1);
  assert.equal(calls[0].file, resolve(f.bundleDir, "Numeron-Alo-Settings.exe"));
  assert.deepEqual(calls[0].args, ["--read-key"]);
  assert.equal(calls[0].options.windowsHide, true); assert.equal(calls[0].options.timeout, 5000);
  assert.equal(calls[0].options.maxBuffer, 8192); assert.equal(calls[0].options.cwd, resolve(f.bundleDir));
  assert.deepEqual(calls[0].options.env, { SystemRoot: "C:\\Windows" });
  assert.equal(calls[0].stdinClosed, true);
  assert.equal((await readFile(f.path, "utf8")).includes(KEY), false);
});

test("invalid configured metadata fails closed instead of using an environment key", async t => {
  const cases = [null, [], { ...metadata(), apiKey: KEY }, { ...metadata(), enabled: false }, { ...metadata(), maxCallsPerHour: 21 },
    { ...metadata(), credentialTarget: "arbitrary-target" }, { ...metadata(), provider: "other" }, { ...metadata(), model: "https://example.invalid" },
    { ...metadata(), model: "gpt\nother" }, { ...metadata(), model: "gpt-6-luna\n" }, { ...metadata(), credentialTarget: `${TARGET}\n` },
    { ...metadata(), schemaVersion: 2 }, { ...metadata(), model: "x".repeat(101) }];
  const calls = helper(t);
  for (const invalid of cases) {
    const f = await fixture(t, invalid);
    await assert.rejects(loadLocalModelSettings({ bundleDir: f.bundleDir, env: { OPENAI_API_KEY: OTHER_KEY } }), sanitized);
  }
  assert.equal(calls.length, 0);
});

test("malformed or oversized metadata cannot reach the credential helper", async t => {
  const calls = helper(t);
  for (const encoded of ["{invalid-json", " ".repeat(8193)]) {
    const f = await fixture(t, encoded);
    await assert.rejects(loadLocalModelSettings({ bundleDir: f.bundleDir, env: {} }), sanitized);
  }
  assert.equal(calls.length, 0);
});

test("a helper error, timeout or missing executable never exposes its output or cause", async t => {
  const f = await fixture(t, metadata());
  helper(t, KEY, Object.assign(new Error(KEY), { code: "ENOENT", stderr: OTHER_KEY }));
  await assert.rejects(loadLocalModelSettings({ bundleDir: f.bundleDir, env: {} }), sanitized);
});

test("helper replies must match the bound target and exact secret-only pipe schema", async t => {
  const f = await fixture(t, metadata());
  const cases = ["not-json", "x".repeat(8193), null, [], { credentialTarget: `${TARGET}x`, apiKey: KEY },
    { credentialTarget: TARGET }, { credentialTarget: TARGET, apiKey: "invalid" }, { credentialTarget: TARGET, apiKey: `${KEY}\n` },
    { credentialTarget: TARGET, apiKey: KEY, extra: "rejected" }];
  for (const reply of cases) {
    const calls = helper(t, reply);
    await assert.rejects(loadLocalModelSettings({ bundleDir: f.bundleDir, env: {} }), sanitized);
    assert.equal(calls.length, 1);
    t.mock.restoreAll();
  }
});

test("invalid environment credentials fail with fixed sanitized text", async t => {
  const f = await fixture(t);
  for (const apiKey of ["vck_synthetic_not_openai", "eyJ.synthetic.codex-token", "sk-short", `${KEY}\nother`]) {
    await assert.rejects(loadLocalModelSettings({ bundleDir: f.bundleDir, env: { OPENAI_API_KEY: apiKey } }), sanitized);
  }
});
