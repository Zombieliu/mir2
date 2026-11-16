import test from "node:test";
import assert from "node:assert/strict";
import { prepareNearbyChat } from "../src/skills/conversation.mjs";

test("nearby chat normalizes and trims the actual transmitted text", () => {
  assert.equal(prepareNearbyChat("　你好，我准备回城补药。　"), "你好,我准备回城补药。");
  assert.equal(prepareNearbyChat("Ｃａｆｅ\u0301"), "Café");
  assert.equal(prepareNearbyChat("你好 @朋友，待会儿见 :)"), "你好 @朋友,待会儿见 :)");
});

test("all command and channel prefixes are rejected after normalization", () => {
  for (const text of ["@ADDSTORAGE", "  @addstorage", "＠ＡＤＤＳＴＯＲＡＧＥ", "/Alice hello", "／Alice hi", "!hi", "!!group", "!~guild", "!#mentor", ":)hello", "：）hello", "@!notice"]) {
    assert.throws(() => prepareNearbyChat(text), /prefixes/, text);
  }
});

test("line breaks, invisible controls and unpaired surrogates are rejected before trim", () => {
  for (const text of ["hello\n", "\t@ADDSTORAGE", "hello\rworld", "hello\u2028world", "\u200b@ADDSTORAGE", "hi\u202eworld", "hi\ud800"]) {
    assert.throws(() => prepareNearbyChat(text), /characters|line breaks/, JSON.stringify(text));
  }
});

test("position and item-link interpolation is rejected even in ordinary prose", () => {
  for (const text of ["I am at $pos", "我在 ＄ＰｏＳ", "here $PoS", "look <WoodenSword>", "[item:42]", "item://42", "${item}", "{{item}}"]) {
    assert.throws(() => prepareNearbyChat(text), /interpolation/, text);
  }
});

test("limits apply to final Unicode text without truncating into another message", () => {
  assert.equal(prepareNearbyChat("　😀你好　", { maxLength: 3 }), "😀你好");
  assert.throws(() => prepareNearbyChat("😀你好啊", { maxLength: 3 }), RangeError);
  assert.throws(() => prepareNearbyChat("   "), /empty/);
  assert.throws(() => prepareNearbyChat(null), TypeError);
  assert.throws(() => prepareNearbyChat("hi", { maxLength: 0 }), RangeError);
});
