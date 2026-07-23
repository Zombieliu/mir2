import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
let ts;
try {
  ts = require("typescript");
} catch {
  ts = require("../node_modules/.ignored/typescript/lib/typescript.js");
}

const modulePath = new URL("../lib/crystal-chat-history.ts", import.meta.url);
const source = readFileSync(modulePath, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(modulePath),
  reportDiagnostics: true,
});

const errors = (compiled.diagnostics ?? []).filter(
  (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
);
assert.deepEqual(errors, [], "crystal-chat-history.ts must transpile without diagnostics");

const module = { exports: {} };
new Function("exports", "module", "require", compiled.outputText)(
  module.exports,
  module,
  (specifier) => {
    throw new Error(`Unexpected test import: ${specifier}`);
  },
);

const {
  CRYSTAL_CHAT_LINE_COUNT,
  CRYSTAL_CHAT_STYLES,
  CRYSTAL_CHAT_WIDTH_PX,
  CrystalChatHistory,
  CrystalChatType,
  wrapCrystalChatText,
} = module.exports;

assert.equal(CRYSTAL_CHAT_WIDTH_PX, 614);
assert.equal(CRYSTAL_CHAT_LINE_COUNT, 4);
assert.deepEqual(Object.values(CrystalChatType), Array.from({ length: 17 }, (_, index) => index));
assert.equal(Object.keys(CRYSTAL_CHAT_STYLES).length, 17);

const expectedStyles = [
  [CrystalChatType.Normal, "#FF000000", "#FFFFFFFF", "normal"],
  [CrystalChatType.Shout, "#FF000000", "#FFFFFF00", "shout"],
  [CrystalChatType.System, "#FFFFFFFF", "#FFFF0000", "system"],
  [CrystalChatType.Hint, "#FF006400", "#FFFFFFFF", "hint"],
  [CrystalChatType.Announcement, "#FFFFFFFF", "#FF0000FF", "announcement"],
  [CrystalChatType.Group, "#FFA52A2A", "#FFFFFFFF", "group"],
  [CrystalChatType.WhisperIn, "#FF00008B", "#FFFFFFFF", "whisper"],
  [CrystalChatType.WhisperOut, "#FF6495ED", "#FFFFFFFF", "whisper"],
  [CrystalChatType.Guild, "#FF008000", "#FFFFFFFF", "guild"],
  [CrystalChatType.Trainer, "#FF000000", "#FFFFFFFF", "normal"],
  [CrystalChatType.LevelUp, "#FF0000FF", "#FFE1B9FA", "announcement"],
  [CrystalChatType.System2, "#FFFFFFFF", "#FF8B0000", "system"],
  [CrystalChatType.Relationship, "#FFFF69B4", "#00000000", "relationship"],
  [CrystalChatType.Mentor, "#FF800080", "#FFFFFFFF", "mentor"],
  [CrystalChatType.Shout2, "#FFFFFFFF", "#FF008000", "shout"],
  [CrystalChatType.Shout3, "#FFFFFFFF", "#FF800080", "shout"],
  [CrystalChatType.LineMessage, "#FFFFFFFF", "#FF0000FF", "line"],
];

for (const [type, ForeColour, BackColour, Channel] of expectedStyles) {
  assert.deepEqual(CRYSTAL_CHAT_STYLES[type], { ForeColour, BackColour, Channel });
}

const measuredLengths = [];
const exactLengthMeasure = (text) => {
  measuredLengths.push(text.length);
  return text.length;
};
assert.deepEqual(wrapCrystalChatText("a".repeat(614), exactLengthMeasure), ["a".repeat(614)]);
assert.deepEqual(wrapCrystalChatText("b".repeat(615), exactLengthMeasure), ["b".repeat(615)]);
assert.equal(Math.max(...measuredLengths), 614, "the source loop does not measure the complete 615-char value");
assert.deepEqual(wrapCrystalChatText("c".repeat(616), (text) => text.length), [
  "c".repeat(614),
  "c".repeat(2),
]);

assert.deepEqual(wrapCrystalChatText("abcdef", (text) => text.length * 205), ["ab", "cd", "ef"]);

const itemLinkCompatibilityText = "ABCDE12345 <X/1> tail";
assert.deepEqual(
  wrapCrystalChatText(itemLinkCompatibilityText, (text) =>
    text === "ABCDE1" || text === "12345 <X" ? 615 : 0,
  ),
  ["ABCDE", "12345", "2345 <X/1> tail"],
  "item-link wrapping must preserve Crystal's relative newIndex behavior",
);

assert.throws(
  () => wrapCrystalChatText("abc", () => Number.NaN),
  /finite non-negative width/,
);
assert.throws(
  () => new CrystalChatHistory(undefined),
  /injected text measure function/,
);

const scrolling = new CrystalChatHistory((text) => text.length);
assert.equal(scrolling.LineCount, 4);
for (const text of ["one", "two", "three", "four"]) {
  scrolling.receiveChat(text, CrystalChatType.Normal);
}
assert.equal(scrolling.StartIndex, 0);
assert.deepEqual(scrolling.VisibleHistory.map((line) => line.Text), ["one", "two", "three", "four"]);

scrolling.receiveChat("five", CrystalChatType.Normal);
assert.equal(scrolling.StartIndex, 1, "a new line must follow when the source is at its four-line tail");
assert.deepEqual(scrolling.VisibleHistory.map((line) => line.Text), ["two", "three", "four", "five"]);

scrolling.home();
assert.equal(scrolling.StartIndex, 0);
scrolling.receiveChat("six", CrystalChatType.Normal);
assert.equal(scrolling.StartIndex, 0, "a scrolled-up source view must not follow new lines");
scrolling.up();
assert.equal(scrolling.StartIndex, 0);
scrolling.down();
assert.equal(scrolling.StartIndex, 1);
scrolling.end();
assert.equal(scrolling.StartIndex, 5, "Crystal End points at History.Count - 1, not Count - LineCount");
assert.deepEqual(scrolling.VisibleHistory.map((line) => line.Text), ["six"]);
scrolling.down();
assert.equal(scrolling.StartIndex, 5);
scrolling.up();
assert.equal(scrolling.StartIndex, 4);
assert.deepEqual(scrolling.VisibleHistory.map((line) => line.Text), ["five", "six"]);
scrolling.home();
assert.equal(scrolling.StartIndex, 0);

const multilineFollow = new CrystalChatHistory((text) => text.length);
for (const text of ["one", "two", "three", "four"]) {
  multilineFollow.receiveChat(text, CrystalChatType.Normal);
}
multilineFollow.receiveChat("x".repeat(616), CrystalChatType.Normal);
assert.equal(multilineFollow.StartIndex, 2, "auto-follow must advance by the number of wrapped lines");
assert.deepEqual(multilineFollow.VisibleHistory.map((line) => line.Text), [
  "three",
  "four",
  "x".repeat(614),
  "x".repeat(2),
]);

const filtered = new CrystalChatHistory((text) => text.length);
for (const type of Object.values(CrystalChatType)) {
  filtered.receiveChat(`type-${type}`, type);
}
assert.equal(filtered.FullHistory.length, 17);
filtered.setFilters({
  FilterNormalChat: true,
  FilterWhisperChat: true,
  FilterShoutChat: true,
  FilterSystemChat: true,
  FilterGroupChat: true,
  FilterGuildChat: true,
});
assert.deepEqual(filtered.History.map((line) => line.Type), [
  CrystalChatType.Hint,
  CrystalChatType.Announcement,
  CrystalChatType.Trainer,
  CrystalChatType.LevelUp,
  CrystalChatType.Relationship,
  CrystalChatType.Mentor,
]);
assert.ok(
  !filtered.History.some((line) => line.Type === CrystalChatType.LineMessage),
  "LineMessage must use FilterNormalChat",
);
assert.equal(filtered.FullHistory.length, 17, "filtering must not mutate FullHistory");
assert.equal(filtered.StartIndex, 5, "Update must clamp to History.Count - 1");

const filteredFollow = new CrystalChatHistory((text) => text.length, {
  FilterNormalChat: true,
});
for (const text of ["one", "two", "three", "four"]) {
  filteredFollow.receiveChat(text, CrystalChatType.Hint);
}
filteredFollow.receiveChat("hidden", CrystalChatType.Normal);
assert.equal(filteredFollow.History.length, 4);
assert.equal(
  filteredFollow.StartIndex,
  1,
  "ReceiveChat advances before the newly received line is filtered, matching Crystal",
);

console.log("crystal chat history tests passed");
