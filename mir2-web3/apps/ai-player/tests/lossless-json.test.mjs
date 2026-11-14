import test from "node:test";
import assert from "node:assert/strict";
import { parseGatewayJson, stringifyGatewayJson } from "../src/lossless-json.mjs";

test("u64 item identities survive nested parse and exact numeric-literal serialization", () => {
  const text = '{"small":9007199254740991,"item":{"unique_id":9007199254740993,"slots":[18446744073709551615]},"signed":-9223372036854775808}';
  const parsed = parseGatewayJson(text);
  assert.equal(parsed.small, Number.MAX_SAFE_INTEGER);
  assert.equal(parsed.item.unique_id, 9007199254740993n);
  assert.equal(parsed.item.slots[0], 18446744073709551615n);
  assert.equal(parsed.signed, -9223372036854775808n);
  assert.equal(stringifyGatewayJson(parsed), text);
});

test("text that looks like a uid is not changed and ordinary decimal rates remain numbers", () => {
  const parsed = parseGatewayJson('{"text":"18446744073709551615","rate":1.25,"nested":[true,null,2]}');
  assert.equal(parsed.text, "18446744073709551615");
  assert.equal(parsed.rate, 1.25);
  assert.deepEqual(parsed.nested, [true, null, 2]);
});

test("already unsafe Numbers and non-finite output fail instead of silently rounding or emitting null", () => {
  for (const value of [Number(9007199254740993n), NaN, Infinity, -Infinity]) {
    assert.throws(() => stringifyGatewayJson({ value }), RangeError);
  }
  assert.equal(stringifyGatewayJson(18446744073709551615n), "18446744073709551615");
});

test("ambiguous unsafe decimal/exponent numbers and invalid input fail closed", () => {
  for (const text of ['{"id":9007199254740993.1}', '{"id":1e20}', '{"id":1e400}', '{invalid}']) {
    assert.throws(() => parseGatewayJson(text), SyntaxError);
  }
  assert.throws(() => parseGatewayJson({ value: 1 }), TypeError);
});
