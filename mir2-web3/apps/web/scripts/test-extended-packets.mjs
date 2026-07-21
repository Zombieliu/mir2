// Behavioral tests for lib/extended-server-packets.ts.
//
// These are the pure normalizers / patchers / label helpers the [fe-packets]
// server->client handlers use to fold gateway JSON into the web client's
// WorldState. The wire shapes mix snake_case (UserItem) and camelCase
// (ClientFriend / ClientMail); the helpers tolerate both. We assert correct
// transforms on representative payloads, structural-sharing behavior (returning
// the original reference when nothing changed), and graceful fallback on junk.
//
// Pure logic only. Run with plain `node`; the .ts source is transpiled in-memory
// via the `typescript` devDependency.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function loadTypeScriptModule(url, requireMap = {}) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: fileURLToPath(url),
  });
  const module = { exports: {} };
  const require = (specifier) => {
    if (specifier in requireMap) return requireMap[specifier];
    throw new Error(`Unexpected require(${specifier}) while loading ${url}`);
  };
  const load = new Function("exports", "module", "require", compiled.outputText);
  load(module.exports, module, require);
  return module.exports;
}

const packets = loadTypeScriptModule(
  new URL("../lib/extended-server-packets.ts", import.meta.url),
);

const {
  packetNumber,
  packetString,
  packetBool,
  normalizeUserItem,
  patchItemsByUniqueId,
  removeItemByUniqueId,
  normalizeFriendList,
  normalizeMailList,
  attackModeChatMessage,
  attackModeLabel,
  petModeChatMessage,
  petModeLabel,
  formatCrystalExperiencePercent,
  mailResultMessage,
  heroCreateResultMessage,
  groupMembersAfterChange,
} = packets;

let passed = 0;
function check(label, fn) {
  fn();
  passed += 1;
}

// ---------------------------------------------------------------------------
// Primitive readers
// ---------------------------------------------------------------------------

check("packetNumber accepts finite numbers only", () => {
  assert.equal(packetNumber(5), 5);
  assert.equal(packetNumber(0), 0);
  assert.equal(packetNumber(-3.5), -3.5);
  assert.equal(packetNumber(NaN), undefined);
  assert.equal(packetNumber(Infinity), undefined);
  assert.equal(packetNumber("5"), undefined, "strings are not coerced");
  assert.equal(packetNumber(null), undefined);
  assert.equal(packetNumber(undefined), undefined);
});

check("packetString accepts strings only (no coercion)", () => {
  assert.equal(packetString("hi"), "hi");
  assert.equal(packetString(""), "", "empty string is a valid string");
  assert.equal(packetString(5), undefined);
  assert.equal(packetString(null), undefined);
  assert.equal(packetString(undefined), undefined);
});

check("packetBool accepts booleans only", () => {
  assert.equal(packetBool(true), true);
  assert.equal(packetBool(false), false);
  assert.equal(packetBool(1), undefined, "numbers are not coerced");
  assert.equal(packetBool("true"), undefined);
  assert.equal(packetBool(null), undefined);
});

// ---------------------------------------------------------------------------
// normalizeUserItem (snake_case wire shape + camelCase tolerance)
// ---------------------------------------------------------------------------

check("normalizeUserItem reads snake_case UserItem wire shape", () => {
  const item = normalizeUserItem({
    unique_id: 1234,
    item_index: 42,
    current_dura: 900,
    max_dura: 1000,
    count: 3,
  });
  assert.deepEqual(item, {
    uniqueId: 1234,
    itemIndex: 42,
    currentDura: 900,
    maxDura: 1000,
    count: 3,
  });
});

check("normalizeUserItem also tolerates camelCase + count default", () => {
  const item = normalizeUserItem({ uniqueId: 7, itemIndex: 1, currentDura: 5, maxDura: 10 });
  assert.equal(item.uniqueId, 7);
  assert.equal(item.itemIndex, 1);
  assert.equal(item.count, 1, "count defaults to 1 when absent");
});

check("normalizeUserItem prefers camelCase when both present", () => {
  // camelCase is read first (?? short-circuits), so it wins over snake_case.
  const item = normalizeUserItem({ uniqueId: 1, unique_id: 999, count: 2 });
  assert.equal(item.uniqueId, 1);
});

check("normalizeUserItem returns null without a numeric uniqueId", () => {
  assert.equal(normalizeUserItem(null), null);
  assert.equal(normalizeUserItem(undefined), null);
  assert.equal(normalizeUserItem(5), null);
  assert.equal(normalizeUserItem("x"), null);
  assert.equal(normalizeUserItem({}), null, "no uniqueId/unique_id");
  assert.equal(normalizeUserItem({ unique_id: "not-a-number" }), null);
  assert.doesNotThrow(() => normalizeUserItem([]));
});

// ---------------------------------------------------------------------------
// patchItemsByUniqueId
// ---------------------------------------------------------------------------

const sampleItems = () => [
  { uniqueId: 1, quantity: 5, durabilityCurrent: 100, durabilityMax: 100, name: "Sword" },
  { uniqueId: 2, quantity: 1, durabilityCurrent: 50, durabilityMax: 80, name: "Shield" },
];

check("patchItemsByUniqueId patches the matching item only", () => {
  const items = sampleItems();
  const next = patchItemsByUniqueId(items, {
    uniqueId: 2,
    quantity: 1,
    durabilityCurrent: 30,
    name: "Cracked Shield",
  });
  assert.notEqual(next, items, "returns a new array when something changed");
  assert.equal(next[0], items[0], "untouched item keeps identity");
  assert.equal(next[1].durabilityCurrent, 30);
  assert.equal(next[1].name, "Cracked Shield");
  assert.equal(next[1].durabilityMax, 80, "unspecified fields are preserved");
});

check("patchItemsByUniqueId keeps existing values when patch omits fields", () => {
  const items = sampleItems();
  const next = patchItemsByUniqueId(items, { uniqueId: 1, durabilityCurrent: 90 });
  assert.equal(next[0].quantity, 5, "quantity preserved when patch.quantity is undefined");
  assert.equal(next[0].durabilityCurrent, 90);
  assert.equal(next[0].name, "Sword");
});

check("patchItemsByUniqueId returns the SAME reference when nothing matched", () => {
  const items = sampleItems();
  const next = patchItemsByUniqueId(items, { uniqueId: 999, quantity: 3 });
  assert.equal(next, items, "no match -> original reference (avoids React churn)");
});

// ---------------------------------------------------------------------------
// removeItemByUniqueId
// ---------------------------------------------------------------------------

check("removeItemByUniqueId removes the whole stack when count covers it", () => {
  const items = sampleItems();
  const next = removeItemByUniqueId(items, 1, 5);
  assert.notEqual(next, items);
  assert.equal(next.length, 1);
  assert.equal(next[0].uniqueId, 2);
});

check("removeItemByUniqueId decrements when quantity exceeds count", () => {
  const items = sampleItems();
  const next = removeItemByUniqueId(items, 1, 2);
  assert.equal(next.length, 2);
  assert.equal(next[0].quantity, 3, "5 - 2 = 3");
  assert.equal(next[0].uniqueId, 1);
});

check("removeItemByUniqueId removes when count is 0 or >= quantity", () => {
  // count <= 0 path: condition `count > 0 && quantity > count` is false -> remove.
  assert.equal(removeItemByUniqueId(sampleItems(), 2, 0).length, 1);
  // count exactly equal to quantity -> not greater -> remove.
  assert.equal(removeItemByUniqueId(sampleItems(), 1, 5).length, 1);
});

check("removeItemByUniqueId only affects the first match and keeps reference when absent", () => {
  const items = [
    { uniqueId: 1, quantity: 2 },
    { uniqueId: 1, quantity: 9 },
  ];
  const next = removeItemByUniqueId(items, 1, 2);
  assert.equal(next.length, 1, "only the first matching stack is consumed");
  assert.equal(next[0].quantity, 9, "the second uniqueId=1 stack is left intact");
  // No match -> same reference.
  const noMatch = removeItemByUniqueId(items, 42, 1);
  assert.equal(noMatch, items);
});

// ---------------------------------------------------------------------------
// normalizeFriendList (camelCase ClientFriend)
// ---------------------------------------------------------------------------

check("normalizeFriendList maps ClientFriend rows + defaults", () => {
  const friends = normalizeFriendList([
    { index: 3, name: "Ally", memo: "guildmate", blocked: false, online: true },
    { name: "Sparse" }, // defaults: index 0, memo "", blocked/online false
  ]);
  assert.equal(friends.length, 2);
  assert.deepEqual(friends[0], {
    index: 3,
    name: "Ally",
    memo: "guildmate",
    blocked: false,
    online: true,
  });
  assert.deepEqual(friends[1], {
    index: 0,
    name: "Sparse",
    memo: "",
    blocked: false,
    online: false,
  });
});

check("normalizeFriendList drops nameless / junk rows and non-arrays", () => {
  assert.deepEqual(normalizeFriendList(null), []);
  assert.deepEqual(normalizeFriendList(undefined), []);
  assert.deepEqual(normalizeFriendList({}), []);
  assert.deepEqual(normalizeFriendList("x"), []);
  const friends = normalizeFriendList([{ index: 1 }, null, 5, "x", { name: "Keep" }]);
  assert.equal(friends.length, 1);
  assert.equal(friends[0].name, "Keep");
});

// ---------------------------------------------------------------------------
// normalizeMailList (camelCase + snake_case tolerance)
// ---------------------------------------------------------------------------

check("normalizeMailList maps ClientMail rows with item count", () => {
  const mail = normalizeMailList([
    {
      mailId: 10,
      senderName: "GM",
      message: "Welcome",
      opened: true,
      locked: false,
      canReply: true,
      collected: false,
      gold: 500,
      items: [{}, {}, {}],
    },
  ]);
  assert.equal(mail.length, 1);
  assert.deepEqual(mail[0], {
    mailId: 10,
    senderName: "GM",
    message: "Welcome",
    opened: true,
    locked: false,
    canReply: true,
    collected: false,
    gold: 500,
    itemCount: 3,
  });
});

check("normalizeMailList tolerates snake_case + applies defaults", () => {
  const mail = normalizeMailList([{ mail_id: 7, sender_name: "Sys", can_reply: false }]);
  assert.equal(mail[0].mailId, 7);
  assert.equal(mail[0].senderName, "Sys");
  assert.equal(mail[0].message, "");
  assert.equal(mail[0].opened, false);
  assert.equal(mail[0].gold, 0);
  assert.equal(mail[0].itemCount, 0, "no items array -> 0");
  assert.equal(mail[0].canReply, false);
});

check("normalizeMailList drops rows without a numeric mailId / non-arrays", () => {
  assert.deepEqual(normalizeMailList(null), []);
  assert.deepEqual(normalizeMailList({}), []);
  const mail = normalizeMailList([{ senderName: "NoId" }, null, 5, { mailId: 1 }]);
  assert.equal(mail.length, 1);
  assert.equal(mail[0].mailId, 1);
});

// ---------------------------------------------------------------------------
// Label helpers
// ---------------------------------------------------------------------------

check("attackModeLabel covers ChangeAMode values + fallback", () => {
  assert.deepEqual(
    [0, 1, 2, 3, 4, 5].map(attackModeLabel),
    ["Peaceful", "Group", "Guild", "EnemyGuild", "RedBrown", "All"],
  );
  assert.equal(attackModeLabel(42), "Mode 42");
});

check("petModeLabel covers ChangePMode values + fallback", () => {
  assert.deepEqual(
    [0, 1, 2, 3, 4].map(petModeLabel),
    ["Both", "MoveOnly", "AttackOnly", "None", "FocusMasterTarget"],
  );
  assert.equal(petModeLabel(42), "Pet Mode 42");
});

check("mode chat messages mirror Crystal startup packet handlers", () => {
  assert.deepEqual(
    [0, 1, 2, 3, 4, 5].map((mode) => attackModeChatMessage(mode)?.localizationKey),
    [
      "client.AttackMode_Peace",
      "client.AttackMode_Group",
      "client.AttackMode_Guild",
      "client.AttackMode_EnemyGuild",
      "client.AttackMode_RedBrown",
      "client.AttackMode_All",
    ],
  );
  assert.deepEqual(
    [0, 1, 2, 3, 4].map((mode) => petModeChatMessage(mode)?.localizationKey),
    [
      "client.PetMode_Both",
      "client.PetMode_MoveOnly",
      "client.PetMode_AttackOnly",
      "client.PetMode_None",
      "client.PetMode_FocusMasterTarget",
    ],
  );
  assert.equal(attackModeChatMessage(6), null);
  assert.equal(petModeChatMessage(5), null);
});

check("main HUD experience uses Crystal #0.##% formatting", () => {
  assert.equal(formatCrystalExperiencePercent(0), "0%");
  assert.equal(formatCrystalExperiencePercent(0.0001), "0.01%");
  assert.equal(formatCrystalExperiencePercent(0.1), "10%");
  assert.equal(formatCrystalExperiencePercent(0.105), "10.5%");
  assert.equal(formatCrystalExperiencePercent(0.4833), "48.33%");
  assert.equal(formatCrystalExperiencePercent(1), "100%");
  assert.equal(formatCrystalExperiencePercent(Number.NaN), "0%");
  assert.equal(formatCrystalExperiencePercent(-1), "0%");
});

check("mailResultMessage distinguishes mail vs parcel + error codes", () => {
  assert.equal(mailResultMessage(1, false), "Mail sent.");
  assert.equal(mailResultMessage(1, true), "Parcel collected.");
  assert.equal(mailResultMessage(0, false), "Mail could not be sent.");
  assert.equal(mailResultMessage(0, true), "Parcel could not be collected.");
  assert.equal(mailResultMessage(-1, false), "Recipient not found.");
  assert.equal(mailResultMessage(-2, false), "Not enough gold.");
  assert.equal(mailResultMessage(-3, false), "Mailbox is full.");
  assert.equal(mailResultMessage(7, false), "Mail send returned 7.");
  assert.equal(mailResultMessage(7, true), "Parcel collection returned 7.");
});

check("heroCreateResultMessage maps NewHero result codes", () => {
  assert.equal(heroCreateResultMessage(1), "Hero name is too long.");
  assert.equal(heroCreateResultMessage(2), "Hero name contains banned words.");
  assert.equal(heroCreateResultMessage(3), "A hero with that name already exists.");
  assert.equal(heroCreateResultMessage(4), "Maximum number of heroes reached.");
  assert.equal(heroCreateResultMessage(8), "Hero created successfully.");
  assert.equal(heroCreateResultMessage(99), "Hero creation returned 99.");
});

// ---------------------------------------------------------------------------
// groupMembersAfterChange
// ---------------------------------------------------------------------------

check("groupMembersAfterChange replace wins and copies", () => {
  const next = groupMembersAfterChange(["A"], { replace: ["X", "Y"] });
  assert.deepEqual(next, ["X", "Y"]);
});

check("groupMembersAfterChange add appends (de-duped), remove filters", () => {
  assert.deepEqual(groupMembersAfterChange(["A", "B"], { add: "C" }), ["A", "B", "C"]);
  assert.deepEqual(
    groupMembersAfterChange(["A", "B"], { add: "B" }),
    ["A", "B"],
    "add ignores duplicates",
  );
  assert.deepEqual(groupMembersAfterChange(["A", "B", "C"], { remove: "B" }), ["A", "C"]);
});

check("groupMembersAfterChange handles undefined current + no-op", () => {
  assert.deepEqual(groupMembersAfterChange(undefined, { add: "First" }), ["First"]);
  assert.deepEqual(groupMembersAfterChange(undefined, {}), []);
  // No add/remove/replace -> copy of current.
  const current = ["A"];
  const next = groupMembersAfterChange(current, {});
  assert.deepEqual(next, ["A"]);
  assert.notEqual(next, current, "returns a fresh array, not the original reference");
});

console.log(`extended server packet tests passed (${passed} groups)`);
