import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

const source = readFileSync(new URL("../lib/r16-companion-private-frame.ts", import.meta.url), "utf8");
const module = { exports: {} };
new Function("exports", ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } }).outputText)(module.exports);
const { readR16CompanionPrivateFrame: read } = module.exports;

test("private replies bypass ordinary decoding regardless of JSON member order", () => {
  for (const frame of [
    { type: "companionResult", payload: { requestId: "one", result: { summary: "private memory" } } },
    { payload: { result: { text: "private instruction" } }, type: "companionResult" },
  ]) assert.deepEqual(read(JSON.stringify(frame)), { private: true, event: frame });
});

test("malformed or oversized private frames are consumed without retaining raw error text", () => {
  assert.deepEqual(read('{"type":"companionResult","payload":"private unfinished'), { private: true, event: null });
  assert.deepEqual(read(JSON.stringify({ type: "companionResult", payload: "x".repeat(128 * 1024) })), { private: true, event: null });
  assert.equal(read('{"other":{"type":"companionResult","result":"private"}}').private, true);
});

test("escaped private type and property names are consumed before generic ingress", () => {
  for (const frame of [
    '{"type":"companion\\u0052esult","payload":{"text":"private"}}',
    '{"t\\u0079pe":"\\u0063ompanionResult","payload":{}}',
  ]) assert.equal(read(frame).private, true);
  assert.deepEqual(read('{"type":"companion\\u0052esult","payload":"unfinished'), { private: true, event: null });
  assert.deepEqual(read('{"type":"packet","payload":{"message":"\\u4f60\\u597d"}}'), { private: false });
});

test("ordinary packets and chat quoting the private envelope keep their existing route", () => {
  for (const frame of [
    { type: "worldSnapshot", payload: { playerObjectId: "18446744073709551615" } },
    { type: "packet", packet: "ObjectChat", payload: { message: 'a "companionResult" reply' } },
    { type: "realmInfo", payload: { features: { companionControl: true } } },
  ]) assert.deepEqual(read(JSON.stringify(frame)), { private: false });
  assert.deepEqual(read(new ArrayBuffer(16)), { private: false });
});
