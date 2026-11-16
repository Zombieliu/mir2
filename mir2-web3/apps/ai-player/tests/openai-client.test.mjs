import test from "node:test";
import assert from "node:assert/strict";
import { OpenAIPlanClient } from "../src/model/openai-client.mjs";
import { instructionText, PLAN_SCHEMA, validatePlan } from "../src/model/plan.mjs";

const KEY = "sk-synthetic-for-unit-test-only";
const plan = () => ({ reply: "我会先支援队友，打两只后回到你身边，途中不拾取。", objective: { kind: "hunt", monsterName: null, count: 2, maxDistanceFromOwner: 6, loot: "never", returnToOwner: true }, rememberedEpisodeIds: [] });
const response = extra => new Response(JSON.stringify({ status: "completed", output: [{ type: "message", role: "assistant", content: [{ type: "output_text", text: JSON.stringify(plan()) }] }], usage: { input_tokens: 120, output_tokens: 90 }, ...extra }), { headers: { "content-type": "application/json", "x-request-id": "req_mock-only" } });
const request = { instructions: "bounded-planner", context: { instruction: "打两只回来" }, schema: PLAN_SCHEMA };

test("Responses request uses official fixed endpoint, strict schema, none reasoning and no stored remote conversation", async () => {
  let calls = 0;
  const client = new OpenAIPlanClient({ apiKey: KEY, model: "gpt-6-luna", fetchImpl: async (url, options) => {
    calls++; assert.equal(url, "https://api.openai.com/v1/responses"); assert.equal(options.redirect, "error");
    assert.equal(options.headers.Authorization, `Bearer ${KEY}`);
    const body = JSON.parse(options.body); assert.equal(body.store, false); assert.equal(body.text.format.strict, true);
    assert.equal(body.reasoning.effort, "none"); assert.equal(body.max_output_tokens, 1024);
    assert.equal(JSON.stringify(body).includes(KEY), false); assert.equal(body.tools, undefined);
    return response();
  } });
  const result = await client.plan(request);
  assert.deepEqual(validatePlan(result.plan), plan()); assert.deepEqual(result.call, { model: "gpt-6-luna", status: "completed", requestId: "req_mock-only", inputTokens: 120, outputTokens: 90 });
  assert.equal(calls, 1); assert.equal(JSON.stringify(client).includes(KEY), false); client.close();
});

for (const [name, makeResponse, code] of [
  ["refusal", () => response({ output: [{ type: "message", role: "assistant", content: [{ type: "refusal", refusal: "private provider words" }] }] }), "refused"],
  ["incomplete", () => response({ status: "incomplete", incomplete_details: { reason: "max_output_tokens" } }), "incomplete"],
  ["authentication", () => new Response(`secret ${KEY}`, { status: 401 }), "authentication"],
  ["rate limit", () => new Response("private provider diagnostics", { status: 429 }), "rate-limit"],
  ["malformed", () => new Response("invalid-json"), "transport-or-format"],
  ["large response", () => new Response("x".repeat(65537)), "response-too-large"],
]) test(`${name} produces a safe failure and never retries`, async () => {
  let calls = 0;
  const client = new OpenAIPlanClient({ apiKey: KEY, model: "gpt-6-luna", fetchImpl: async () => { calls++; return makeResponse(); } });
  await assert.rejects(client.plan(request), error => { assert.equal(error.code, code); assert.equal(String(error).includes(KEY), false); assert.equal(String(error).includes("private"), false); return true; });
  assert.equal(calls, 1);
});

test("timeout and cancellation propagate to the actual request and discard network diagnostics", async () => {
  const make = timeoutMs => new OpenAIPlanClient({ apiKey: KEY, model: "gpt-6-luna", timeoutMs, fetchImpl: async (url, { signal }) => new Promise((resolve, reject) => signal.addEventListener("abort", () => reject(new Error(KEY)), { once: true })) });
  const keepAlive = setInterval(() => {}, 100);
  try {
    await assert.rejects(make(10).plan(request), { code: "timeout" });
    const abort = new AbortController(), pending = make(1000).plan({ ...request, signal: abort.signal }); abort.abort();
    await assert.rejects(pending, { code: "cancelled" });
  } finally { clearInterval(keepAlive); }
});

test("local plan validation rejects extra authority, forged quantities and credential-looking output", () => {
  for (const mutate of [
    p => { p.objective.kind = "trade"; }, p => { p.objective.count = 999; }, p => { p.objective.maxDistanceFromOwner = 9; },
    p => { p.objective.teleport = true; }, p => { p.reply = KEY; }, p => { p.rememberedEpisodeIds = ["same", "same"]; },
    p => { p.objective.kind = "follow"; }, p => { p.objective.monsterName = "bad\nname"; },
  ]) { const invalid = plan(); mutate(invalid); assert.throws(() => validatePlan(invalid), { code: "invalid-plan" }); }
  assert.equal(instructionText("先支援，战后拾取。"), "先支援,战后拾取。");
  for (const text of ["", " first", "line\nbreak", KEY, "a".repeat(1001), "api_key=abcdefghijk", "ｐａｓｓｗｏｒｄ＝synthetic-private", "a".repeat(496) + "ｐａｓｓｗｏｒｄ＝synthetic-boundary", "a".repeat(489) + "http://user:synthetic-private@proxy.example"]) assert.throws(() => instructionText(text));
});
