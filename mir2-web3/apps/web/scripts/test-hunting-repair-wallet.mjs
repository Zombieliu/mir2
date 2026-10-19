import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import test from "node:test";

// Run only actual modules against in-memory native Response/stream fixtures.
// No request reaches the network and no identity credential is printed.
const ts = createRequire(import.meta.url)(process.env.MIR2_TEST_TYPESCRIPT_PATH ?? "typescript");
const read = path => readFileSync(new URL(path, import.meta.url), "utf8");
const compilerOptions = { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, strict: true,
  moduleResolution: ts.ModuleResolutionKind.Node10, skipLibCheck: true, types: [], lib: ["lib.es2022.d.ts", "lib.dom.d.ts"] };
const files = { wallet: "../lib/hunting-repair-wallet.ts", purchase: "../lib/npc-purchase-client.ts" };
const program = ts.createProgram([fileURLToPath(new URL(files.wallet, import.meta.url))], { ...compilerOptions, noEmit: true });
const diagnostics = ts.getPreEmitDiagnostics(program).filter(value => value.category === ts.DiagnosticCategory.Error);
assert.equal(diagnostics.length, 0, diagnostics.map(value => ts.flattenDiagnosticMessageText(value.messageText, "\n")).join("\n"));
function compile(text) {
  const compiled = ts.transpileModule(text, { compilerOptions, reportDiagnostics: true });
  assert.equal((compiled.diagnostics ?? []).filter(value => value.category === ts.DiagnosticCategory.Error).length, 0);
  return compiled.outputText;
}
const modules = new Map();
function load(name) {
  if (modules.has(name)) return modules.get(name);
  assert(Object.hasOwn(files, name)); const module = { exports: {} }; modules.set(name, module.exports);
  new Function("require", "module", "exports", compile(read(files[name])))(id => {
    assert.equal(name, "wallet"); assert.equal(id, "./npc-purchase-client"); return load("purchase");
  }, module, module.exports); return module.exports;
}
const { parseHuntingRepairWallet, HuntingRepairWalletClient } = load("wallet");
const U64 = "18446744073709551615";
function payload(patch = {}) { return { characterIndex: 2, initialized: true, enabled: true,
  boundGold: "123", unboundGold: "456", walletRevision: "7", permitStockAtoms: "1000000",
  policyVersion: "policy-v1", settledThroughDay: 20000, ...patch }; }
const raw = patch => JSON.stringify(payload(patch));
const response = patch => new Response(raw(patch), { status: 200 });
function deferred() { let resolve, reject; const promise = new Promise((ok, fail) => { resolve = ok; reject = fail; }); return { promise, resolve, reject }; }
function fixture(overrides = {}) {
  const owner = {}, actorScope = Object.freeze({ characterIndex: 2, incarnation: "trusted-original" });
  const identity = Object.freeze({ token: "unit.test-token", sessionId: "identity-session", accountId: "actor", expiresAtMs: 5000 });
  const state = { owner, actorScope, identity, now: 1000, valid: true, duringCurrent: null, duringChange: null };
  const calls = [], requests = []; let changes = 0;
  const client = new HuntingRepairWalletClient({ owner, actorScope, identity, characterIndex: 2,
    isCurrent: (capturedOwner, capturedActor, capturedIdentity) => {
      state.duringCurrent?.();
      return state.valid && capturedOwner === state.owner && capturedActor === state.actorScope && capturedIdentity === state.identity;
    }, now: () => state.now,
    onChange: () => { ++changes; state.duringChange?.(); },
    fetch: (input, init) => { const pending = deferred(); calls.push({ input, init }); requests.push(pending); return pending.promise; }, ...overrides });
  return { client, owner, actorScope, identity, state, calls, requests, get changes() { return changes; },
    async complete(patch = {}) { const refreshing = client.refresh(); requests.at(-1).resolve(response(patch)); return refreshing; } };
}

test("strict wallet parser preserves exact u64 strings and immutable observational facts", () => {
  const balance = parseHuntingRepairWallet(raw({ boundGold: U64, unboundGold: "0", walletRevision: U64, permitStockAtoms: U64 }), 2);
  assert(Object.isFrozen(balance)); assert.equal(balance.boundGold, U64); assert.equal(balance.walletRevision, U64);
  assert.equal(parseHuntingRepairWallet(raw({ settledThroughDay: -1, policyVersion: "policy-😀" }), 2).settledThroughDay, -1);
  const uninitialized = { initialized: false, boundGold: "0", unboundGold: "0", walletRevision: "0" };
  assert.equal(parseHuntingRepairWallet(raw(uninitialized), 2).initialized, false);
});
test("wallet parser rejects missing/unknown/duplicate fields and non-object JSON", () => {
  for (const malformed of ["null", "[]", "1", JSON.stringify({ ...payload(), extra: true }),
    JSON.stringify(Object.fromEntries(Object.entries(payload()).filter(([key]) => key !== "enabled"))),
    raw().replace('"enabled":true', '"enabled":true,"enabled":false'),
    raw().replace('"walletRevision":"7"', '"walletRevision":"7","wallet\\u0052evision":"7"')])
    assert.throws(() => parseHuntingRepairWallet(malformed, 2));
});
test("wallet parser rejects noncanonical/overflow/numeric currency and mismatched character", () => {
  for (const field of ["boundGold", "unboundGold", "walletRevision", "permitStockAtoms"])
    for (const value of [0, -1, 1.2, "", "01", "+1", "-1", "1e3", "1.0", "18446744073709551616", "１", null])
      assert.throws(() => parseHuntingRepairWallet(raw({ [field]: value }), 2));
  for (const characterIndex of [0, -1, 1.5, 2147483648, "2", null])
    assert.throws(() => parseHuntingRepairWallet(raw({ characterIndex }), 2));
});
test("uninitialized wallets cannot carry previous balances or revision", () => {
  for (const field of ["boundGold", "unboundGold", "walletRevision"])
    assert.throws(() => parseHuntingRepairWallet(raw({ initialized: false, boundGold: "0", unboundGold: "0", walletRevision: "0", [field]: "1" }), 2));
  for (const patch of [{ initialized: 1 }, { enabled: "true" }, { policyVersion: "" }, { policyVersion: "bad\u0000value" },
    { policyVersion: "bad\u0085value" }, { policyVersion: "bad\ud800" }, { policyVersion: "é".repeat(129) },
    { settledThroughDay: 1.5 }, { settledThroughDay: Number.MAX_SAFE_INTEGER + 1 }])
    assert.throws(() => parseHuntingRepairWallet(raw(patch), 2));
});
test("wallet parser enforces the UTF-8 16KiB boundary before JSON parsing", () => {
  const text = raw(); assert(parseHuntingRepairWallet(text + " ".repeat(16384 - Buffer.byteLength(text)), 2));
  assert.throws(() => parseHuntingRepairWallet(text + " ".repeat(16385 - Buffer.byteLength(text)), 2));
  assert.throws(() => parseHuntingRepairWallet(text + "é".repeat(8192), 2));
});
test("wallet client uses only Authorization and fixed same-origin no-store GET", async () => {
  const f = fixture(); assert(await f.complete()); const call = f.calls[0];
  assert.equal(call.input, "/api/economy/hunting-wallet?characterIndex=2");
  assert(!call.input.includes(f.identity.token)); assert.equal(call.init.method, "GET");
  assert.deepEqual(Object.keys(call.init.headers).sort(), ["accept", "authorization"]);
  assert(call.init.headers.authorization.startsWith("Bearer ")); assert.equal(call.init.cache, "no-store");
  assert.equal(call.init.credentials, "same-origin"); assert.equal(call.init.redirect, "error"); assert(call.init.signal instanceof AbortSignal);
  assert.equal(f.client.balance().boundGold, "123"); assert(Object.isFrozen(f.client.balance()));
});
test("wallet client rejects invalid mutable custody, token, index and expiry declarations", () => {
  const valid = { owner: {}, actorScope: Object.freeze({ actor: 1 }), identity: Object.freeze({ token: "unit.test", sessionId: "s", accountId: "a", expiresAtMs: 5000 }),
    characterIndex: 2, isCurrent: () => true, now: () => 1000 };
  for (const patch of [{ owner: null }, { actorScope: {} }, { identity: { ...valid.identity } }, { characterIndex: -1 },
    { characterIndex: 2147483648 }, { isCurrent: null }]) assert.throws(() => new HuntingRepairWalletClient({ ...valid, ...patch }));
  for (const token of ["", "x y", "x\r\ny", "x".repeat(4090), "é"])
    assert.throws(() => new HuntingRepairWalletClient({ ...valid, identity: Object.freeze({ ...valid.identity, token }) }));
});
test("pending refresh clears old observations until a new current balance succeeds", async () => {
  const f = fixture(); assert(await f.complete()); const pending = f.client.refresh();
  assert.equal(f.client.balance(), null); f.requests[1].resolve(new Response("{}", { status: 503 }));
  assert.equal(await pending, false); assert.equal(f.client.balance(), null);
  assert(await f.complete({ walletRevision: "8", boundGold: "9" })); assert.equal(f.client.balance().boundGold, "9");
});
test("late response cannot cross physical socket, actor, identity or expiry boundaries", async () => {
  for (const change of [f => { f.state.owner = {}; }, f => { f.state.actorScope = Object.freeze({ actor: "new" }); },
    f => { f.state.identity = Object.freeze({ ...f.identity, sessionId: "new" }); }, f => { f.state.now = 5000; }, f => { f.state.valid = false; }]) {
    const f = fixture(), pending = f.client.refresh(); change(f); f.requests[0].resolve(response());
    assert.equal(await pending, false); assert.equal(f.client.balance(), null);
  }
});
test("identity expiry and invalid clocks block refresh without issuing a request", async () => {
  for (const now of [5000, 5001, NaN, -1, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
    const f = fixture(); f.state.now = now; assert.equal(await f.client.refresh(), false); assert.equal(f.calls.length, 0); assert.equal(f.client.balance(), null);
  }
});
test("invalidate and retire abort pending fetch and reject its late result", async () => {
  for (const action of ["invalidate", "retire"]) {
    const f = fixture(), pending = f.client.refresh(), signal = f.calls[0].init.signal; f.client[action]();
    assert.equal(signal.aborted, true); f.requests[0].resolve(response());
    assert.equal(await pending, false); assert.equal(f.client.balance(), null);
    if (action === "retire") { assert.equal(await f.client.refresh(), false); assert.equal(f.calls.length, 1); }
  }
});
test("concurrent refresh supersedes original request even if old transport ignores abort", async () => {
  const f = fixture(), first = f.client.refresh(), second = f.client.refresh();
  assert(f.calls[0].init.signal.aborted); f.requests[1].resolve(response({ walletRevision: "9", boundGold: "9" }));
  assert(await second); f.requests[0].resolve(response({ walletRevision: "8", boundGold: "8" }));
  assert.equal(await first, false); assert.equal(f.client.balance().boundGold, "9");
});
test("older wallet revisions never resurrect pre-mutation balances after invalidation", async () => {
  const f = fixture(); assert(await f.complete({ walletRevision: "9007199254740993" })); f.client.invalidate();
  assert.equal(await f.complete({ walletRevision: "9007199254740992", boundGold: "999" }), false); assert.equal(f.client.balance(), null);
  assert(await f.complete({ walletRevision: "9007199254740993", boundGold: "12" })); assert.equal(f.client.balance().boundGold, "12");
});
test("malformed HTTP, JSON, wrong character and rejected fetch leave no old balance", async () => {
  for (const answer of [new Response(raw(), { status: 401 }), new Response("{"), new Response(raw({ characterIndex: 3 })),
    new Response(raw().replace('"enabled":true', '"enabled":true,"enabled":true'))]) {
    const f = fixture(); assert(await f.complete()); const pending = f.client.refresh(); f.requests.at(-1).resolve(answer);
    assert.equal(await pending, false); assert.equal(f.client.balance(), null);
  }
  const f = fixture(), pending = f.client.refresh(); f.requests[0].reject(Error("simulated failure"));
  assert.equal(await pending, false); assert.equal(f.client.balance(), null);
});
test("callback reentry cannot install a retired or invalidated balance", async () => {
  for (const action of ["invalidate", "retire"]) {
    const f = fixture(), pending = f.client.refresh(); f.state.duringChange = () => f.client[action]();
    f.requests[0].resolve(response()); assert.equal(await pending, false); assert.equal(f.client.balance(), null);
  }
  for (const action of ["invalidate", "retire"]) {
    const f = fixture(), pending = f.client.refresh(); f.state.duringCurrent = () => f.client[action]();
    f.requests[0].resolve(response()); assert.equal(await pending, false); assert.equal(f.client.balance(), null);
  }
});
test("selection callbacks cannot reenter refresh during wallet change notifications", async () => {
  const f = fixture(), reentries = []; f.state.duringChange = () => reentries.push(f.client.refresh());
  const pending = f.client.refresh(); f.requests[0].resolve(response()); assert(await pending);
  assert.deepEqual(await Promise.all(reentries), [false, false]); assert.equal(f.calls.length, 1);
});
test("late body completion after owner change cannot publish observation", async () => {
  const f = fixture(), body = deferred(), pending = f.client.refresh();
  f.requests[0].resolve({ status: 200, text: () => body.promise }); await Promise.resolve();
  f.state.actorScope = {}; body.resolve(raw()); assert.equal(await pending, false); assert.equal(f.client.balance(), null);
});

const routeCode = compile(read("../app/api/economy/hunting-wallet/route.ts"));
class MockNextResponse extends Response {
  static json(value, init) { return new MockNextResponse(JSON.stringify(value), init); }
}
function proxyFixture({ configured = "http://127.0.0.1:19500", answer, fetchError } = {}) {
  const calls = [], timeouts = [], controller = new AbortController(), module = { exports: {} };
  new Function("require", "module", "exports", "process", "fetch", "AbortSignal", routeCode)(id => {
    assert.equal(id, "next/server"); return { NextResponse: MockNextResponse };
  }, module, module.exports, { env: { MIR2_GATEWAY_HTTP_URL: configured } }, async (input, init) => {
    calls.push({ input, init }); if (fetchError) throw Error("simulated network failure");
    return typeof answer === "function" ? answer(input, init) : answer ?? response();
  }, { timeout: milliseconds => { timeouts.push(milliseconds); return controller.signal; } });
  return { ...module.exports, calls, timeouts, signal: controller.signal };
}
function request(query = "characterIndex=2", headers = {}) {
  return new Request(`https://game.example/api/economy/hunting-wallet${query ? "?" + query : ""}`,
    { headers: { authorization: "Bearer unit.test-token", ...headers } });
}
function privateResponse(value) {
  assert.equal(value.headers.get("cache-control"), "no-store, private"); assert.equal(value.headers.get("x-content-type-options"), "nosniff");
  assert.equal(value.headers.get("content-type"), "application/json; charset=utf-8"); assert.equal(value.headers.get("set-cookie"), null);
}
test("proxy only calls trusted env host/path with canonical index and private no-store output", async () => {
  const f = proxyFixture({ configured: " https://trusted.example/prefix/// " });
  const result = await f.GET(request("characterIndex=2147483647", { cookie: "do-not-forward", "x-forwarded-host": "attacker.example" }));
  assert.equal(result.status, 200); privateResponse(result); assert.equal(await result.text(), raw());
  assert.equal(f.calls[0].input, "https://trusted.example/prefix/v1/economy/hunting-wallet?characterIndex=2147483647");
  assert.equal(f.calls[0].init.method, "GET"); assert.deepEqual(Object.keys(f.calls[0].init.headers).sort(), ["accept", "authorization"]);
  assert.equal(f.calls[0].init.headers.cookie, undefined); assert.equal(f.calls[0].init.headers["x-forwarded-host"], undefined);
  assert.equal(f.calls[0].init.cache, "no-store"); assert.equal(f.calls[0].init.redirect, "error");
  assert.equal(f.calls[0].init.signal, f.signal); assert.deepEqual(f.timeouts, [8000]);
});
test("proxy rejects duplicate, unknown, missing and noncanonical index queries before fetch", async () => {
  for (const query of ["", "characterIndex=", "characterIndex=-1", "characterIndex=01", "characterIndex=+1", "characterIndex=1.0",
    "characterIndex=1e2", "characterIndex=2147483648", "characterIndex=999999999999999999999", "characterIndex=2&characterIndex=2",
    "characterIndex=2&host=https://attacker.example", "foo=2", "characterIndex=2&url=//attacker.example", "characterIndex=2&path=/other"]) {
    const f = proxyFixture(), result = await f.GET(request(query)); assert.equal(result.status, 400, query);
    privateResponse(result); assert.equal(f.calls.length, 0);
  }
});
test("proxy requires bounded Bearer identity and never substitutes cookie authorization", async () => {
  for (const authorization of [null, "", "Basic unit.test", "bearer unit.test", "Bearer bad token", "Bearer " + "x".repeat(4090)]) {
    const f = proxyFixture(), req = request(); req.headers.set("cookie", "identity=not-authority");
    if (authorization === null) req.headers.delete("authorization"); else req.headers.set("authorization", authorization);
    const result = await f.GET(req); assert.equal(result.status, 401); privateResponse(result); assert.equal(f.calls.length, 0);
  }
});
test("proxy rejects absent, malformed, credentialed and unsafe trusted upstream declarations", async () => {
  for (const configured of [undefined, "", " ", "not a url", "ftp://trusted.example", "file:///etc/passwd", "https://user:pass@trusted.example",
    "https://trusted.example?target=evil", "https://trusted.example/#fragment"]) {
    const f = proxyFixture({ configured: configured ?? "" }), result = await f.GET(request());
    assert.equal(result.status, 503); privateResponse(result); assert.equal(f.calls.length, 0);
  }
});
test("proxy invalid request URL fails closed as a private bounded response", async () => {
  const f = proxyFixture(), result = await f.GET({ url: "not a valid URL", headers: new Headers({ authorization: "Bearer unit.test" }) });
  assert([400, 503].includes(result.status)); privateResponse(result); assert.equal(f.calls.length, 0);
});
test("proxy never forwards upstream Set-Cookie, cache or redirect headers", async () => {
  const f = proxyFixture({ answer: new Response(raw(), { status: 200, headers: { "set-cookie": "do-not-copy", "cache-control": "public,max-age=3600",
    location: "https://other.example", "x-secret-upstream": "do-not-copy" } }) }), result = await f.GET(request());
  assert.equal(result.status, 200); privateResponse(result); assert.equal(result.headers.get("location"), null);
  assert.equal(result.headers.get("x-secret-upstream"), null);
});
test("proxy preserves explicit HTTP failures but rejects redirects and unsupported status", async () => {
  for (const status of [400, 401, 403, 404, 409, 429, 500, 503]) {
    const f = proxyFixture({ answer: new Response('{"error":"upstream"}', { status }) }), result = await f.GET(request());
    assert.equal(result.status, status); privateResponse(result); assert.equal(await result.text(), '{"error":"upstream"}');
  }
  for (const status of [201, 204, 301, 302, 307, 399, 600]) {
    const f = proxyFixture({ answer: { status } }), result = await f.GET(request()); assert.equal(result.status, 502); privateResponse(result);
  }
});
test("proxy accepts an exact 16KiB streamed response but cancels over-limit bodies", async () => {
  const text = "{}" + " ".repeat(16382), exact = proxyFixture({ answer: new Response(text) });
  assert.equal((await exact.GET(request())).status, 200);
  let canceled = false;
  const stream = new ReadableStream({ start(controller) { controller.enqueue(new Uint8Array(16385)); }, cancel() { canceled = true; } });
  const f = proxyFixture({ answer: new Response(stream) }), result = await f.GET(request());
  assert.equal(result.status, 503); privateResponse(result); assert.equal(canceled, true);
});
test("proxy counts raw UTF-8 bytes across chunks and decodes split codepoints safely", async () => {
  const bytes = new TextEncoder().encode('{"policy":"😀"}'), split = bytes.indexOf(0xf0) + 2;
  const stream = new ReadableStream({ start(controller) { controller.enqueue(bytes.slice(0, split)); controller.enqueue(bytes.slice(split)); controller.close(); } });
  const f = proxyFixture({ answer: new Response(stream) }), result = await f.GET(request());
  assert.equal(result.status, 200); assert.equal(await result.text(), '{"policy":"😀"}');
  const tooLarge = proxyFixture({ answer: new Response("é".repeat(8193)) }); assert.equal((await tooLarge.GET(request())).status, 503);
});
test("proxy malformed UTF-8, missing body and network exceptions fail closed", async () => {
  for (const answer of [new Response(new Uint8Array([0xc3, 0x28])), new Response(null, { status: 200 }),
    new Response(new ReadableStream({ start(controller) { controller.error(Error("simulated body failure")); } }))]) {
    const f = proxyFixture({ answer }), result = await f.GET(request()); assert.equal(result.status, 503); privateResponse(result);
  }
  const f = proxyFixture({ fetchError: true }), result = await f.GET(request()); assert.equal(result.status, 503); privateResponse(result);
});
test("proxy exports only the fixed GET node runtime and dynamic route behavior", () => {
  const f = proxyFixture(); assert.equal(f.runtime, "nodejs"); assert.equal(f.dynamic, "force-dynamic");
  assert.equal(f.POST, undefined); assert.equal(f.PUT, undefined); assert.equal(f.DELETE, undefined);
});
