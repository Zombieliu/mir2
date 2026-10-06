// Behavioral tests for lib/stage5-window-adapters.ts.
//
// These adapters defensively map the page's loosely-typed stage-5 state
// (Record<string, unknown> slices that arrive over the wire) into the strict
// prop shapes the Crystal UI windows consume. We feed representative raw
// camelCase payloads (matching the gateway serialization) AND malformed / empty
// inputs, asserting the typed output is correct and that nothing throws.
//
// Pure logic only: no DOM, no network. Run with plain `node` — the .ts source is
// transpiled in-memory via the `typescript` devDependency (same harness the
// other test-*.mjs scripts use).

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

const adapters = loadTypeScriptModule(
  new URL("../lib/stage5-window-adapters.ts", import.meta.url),
);
const socialReplies = loadTypeScriptModule(new URL("../lib/social-incoming-replies.ts", import.meta.url));
const socialOperations = loadTypeScriptModule(new URL("../lib/social-window-operations.ts", import.meta.url));
const socialParityActions = loadTypeScriptModule(new URL("../lib/social-parity-actions.ts", import.meta.url));
const extendedPackets = loadTypeScriptModule(new URL("../lib/extended-server-packets.ts", import.meta.url));
const heroUi = loadTypeScriptModule(new URL("../lib/hero-player-ui.ts", import.meta.url));
const guildBuffUi = loadTypeScriptModule(new URL("../lib/guild-buff-ui.ts", import.meta.url));
const sharedTooltip = loadTypeScriptModule(new URL("../lib/shared-item-tooltip.ts", import.meta.url));
const skillBarDocument = loadTypeScriptModule(new URL("../lib/shared-skill-bar.ts", import.meta.url));
const combatModeKeys = loadTypeScriptModule(new URL("../lib/shared-combat-mode-keys.ts", import.meta.url));
const equipmentGateway = loadTypeScriptModule(new URL("../lib/equipment-gateway-adapter.ts", import.meta.url), {
  "./world-model/item-identity": loadTypeScriptModule(new URL("../lib/world-model/item-identity.ts", import.meta.url)),
});
const mailParcelGateway = loadTypeScriptModule(new URL("../lib/mail-parcel-gateway-adapter.ts", import.meta.url), {
  "./equipment-gateway-adapter": equipmentGateway,
});
const creatureUi = loadTypeScriptModule(new URL("../lib/creature-player-ui.ts", import.meta.url), {
  "./social-incoming-replies": socialReplies,
});
const cashShopUi = loadTypeScriptModule(new URL("../lib/cash-game-shop-ui.ts", import.meta.url), {
  "./social-incoming-replies": socialReplies, "./creature-player-ui": creatureUi,
});
const storageRental = loadTypeScriptModule(new URL("../lib/storage-rental-confirmation.ts", import.meta.url), {
  "./social-incoming-replies": socialReplies,
});
const itemIdentity = loadTypeScriptModule(new URL("../lib/world-model/item-identity.ts", import.meta.url));
const bagModel = loadTypeScriptModule(new URL("../lib/bevy-bag-model.ts", import.meta.url), {
  "./world-model/item-identity": itemIdentity,
});
const socialItems = loadTypeScriptModule(new URL("../lib/social-item-window-model.ts", import.meta.url), {
  "./bevy-bag-model": bagModel, "./world-model/item-identity": itemIdentity,
});

const questWorldDocument = loadTypeScriptModule(new URL("../lib/bevy-quest-world-context.ts", import.meta.url), { "./bevy-bag-model": bagModel });
const questNameDocument = loadTypeScriptModule(new URL("../lib/bevy-quest-ui.ts", import.meta.url), {
  "./bevy-quest-world-context": questWorldDocument,
  "./quest-action-policy": loadTypeScriptModule(new URL("../lib/quest-action-policy.ts", import.meta.url)),
});

const {
  classKeyFromUnknown,
  adaptHero,
  adaptCreatures,
  adaptGroup,
  adaptFriends,
  adaptRelationship,
  adaptMentor,
  rankingTabKey,
  rankingPageKeyForTab,
  adaptRankingPage,
  adaptActiveRankingPage,
  adaptMarketListings,
  adaptConquest,
  adaptGuildTerritory,
  adaptTrade,
  adaptBuffs,
} = adapters;

let passed = 0;
function check(label, fn) {
  fn();
  passed += 1;
  // Per-assertion progress stays quiet; the suite prints a summary at the end.
}

// A grab-bag of inputs every adapter must tolerate without throwing.
const MALFORMED_INPUTS = [
  null,
  undefined,
  {},
  [],
  42,
  "string",
  true,
  NaN,
  { unrelated: "field" },
  Object.create(null),
];

// ---------------------------------------------------------------------------
// classKeyFromUnknown
// ---------------------------------------------------------------------------

check("classKeyFromUnknown maps protocol string variants", () => {
  assert.equal(classKeyFromUnknown("Warrior"), "warrior");
  assert.equal(classKeyFromUnknown("Wizard"), "wizard");
  assert.equal(classKeyFromUnknown("Taoist"), "taoist");
  assert.equal(classKeyFromUnknown("Assassin"), "assassin");
  assert.equal(classKeyFromUnknown("Archer"), "archer");
  // Case-insensitive + substring matching.
  assert.equal(classKeyFromUnknown("TAO"), "taoist");
  assert.equal(classKeyFromUnknown("some-wizard-thing"), "wizard");
});

check("classKeyFromUnknown maps numeric MirClass ordinals", () => {
  assert.equal(classKeyFromUnknown(0), "warrior");
  assert.equal(classKeyFromUnknown(1), "wizard");
  assert.equal(classKeyFromUnknown(2), "taoist");
  assert.equal(classKeyFromUnknown(3), "assassin");
  assert.equal(classKeyFromUnknown(4), "archer");
});

check("classKeyFromUnknown returns undefined for unknown / junk", () => {
  assert.equal(classKeyFromUnknown(99), undefined);
  assert.equal(classKeyFromUnknown("druid"), undefined);
  assert.equal(classKeyFromUnknown(null), undefined);
  assert.equal(classKeyFromUnknown(undefined), undefined);
  assert.equal(classKeyFromUnknown({}), undefined);
  assert.equal(classKeyFromUnknown([]), undefined);
});

// ---------------------------------------------------------------------------
// adaptHero
// ---------------------------------------------------------------------------

check("adaptHero returns null with no name/hp/level", () => {
  assert.equal(adaptHero(null), null);
  assert.equal(adaptHero(undefined), null);
  assert.equal(adaptHero({}), null);
  // currentHero present but empty -> still nothing meaningful.
  assert.equal(adaptHero({ currentHero: {} }), null);
});

check("adaptHero merges ManageHeroes + HUD mirror fields", () => {
  const hero = adaptHero({
    currentHero: { name: "Valkyrie", level: 12, class: "Wizard" },
    hp: 80,
    maxHp: 120,
    mp: 40,
    maxMp: 90,
    experience: 1500,
    maxExperience: 3000,
    loyalty: 7,
    maxLoyalty: 10,
    attack: 22,
    defence: 14,
    summoned: true,
  });
  assert.ok(hero, "hero should be present");
  assert.equal(hero.name, "Valkyrie");
  assert.equal(hero.classKey, "wizard");
  assert.equal(hero.level, 12);
  assert.equal(hero.hp, 80);
  assert.equal(hero.maxHp, 120);
  assert.equal(hero.mp, 40);
  assert.equal(hero.maxMp, 90);
  assert.equal(hero.experience, 1500);
  assert.equal(hero.maxExperience, 3000);
  assert.equal(hero.loyalty, 7);
  assert.equal(hero.maxLoyalty, 10);
  assert.equal(hero.attack, 22);
  assert.equal(hero.defence, 14);
  assert.equal(hero.active, true);
});

check("adaptHero tolerates alternate field spellings (maxHP/ac/dc/exp)", () => {
  const hero = adaptHero({ name: "Alt", hp: 10, maxHP: 50, ac: 5, dc: 9, exp: 7, maxExp: 100 });
  assert.ok(hero);
  assert.equal(hero.maxHp, 50);
  assert.equal(hero.attack, 5);
  assert.equal(hero.defence, 9);
  assert.equal(hero.experience, 7);
  assert.equal(hero.maxExperience, 100);
});

check("adaptHero applies sensible defaults", () => {
  // Only a level present: name defaults to "Hero", hp to 0, maxHp to >=1.
  const onlyLevel = adaptHero({ level: 3 });
  assert.ok(onlyLevel);
  assert.equal(onlyLevel.name, "Hero");
  assert.equal(onlyLevel.level, 3);
  assert.equal(onlyLevel.hp, 0);
  assert.equal(onlyLevel.maxHp, 1, "maxHp falls back to at least 1");
  // hp present, no maxHp -> maxHp clamps up to hp.
  const onlyHp = adaptHero({ hp: 75 });
  assert.ok(onlyHp);
  assert.equal(onlyHp.maxHp, 75);
  assert.equal(onlyHp.level, 1, "level defaults to 1");
});

check("adaptHero derives active from spawnState when no explicit flag", () => {
  assert.equal(adaptHero({ name: "S", spawnState: 2 }).active, true);
  assert.equal(adaptHero({ name: "S", spawnState: 0 }).active, false);
  assert.equal(adaptHero({ name: "S", spawnState: 1, active: true }).active, false);
  assert.equal(adaptHero({ name: "S", spawnState: 1 }).spawnState, "unsummoned");
  assert.equal(adaptHero({ name: "S", spawnState: 2 }).spawnState, "summoned");
  assert.equal(adaptHero({ name: "S", behaviour: 99 }).behaviour, undefined);
  // No active/summoned/spawnState -> active stays undefined.
  assert.equal(adaptHero({ name: "S" }).active, undefined);
});

check("adaptHero never throws on malformed input", () => {
  for (const input of MALFORMED_INPUTS) {
    assert.doesNotThrow(() => adaptHero(input), `adaptHero(${JSON.stringify(input)})`);
  }
});

// ---------------------------------------------------------------------------
// adaptCreatures
// ---------------------------------------------------------------------------

check("adaptCreatures maps ClientIntelligentCreature rows", () => {
  const creatures = adaptCreatures([
    {
      slotIndex: 0,
      customName: "Frostling",
      icon: 42,
      petLevel: 5,
      hp: 30,
      maxHp: 60,
      fullness: 750,
      petMode: 1,
      summoned: true,
    },
  ]);
  assert.equal(creatures.length, 1);
  const c = creatures[0];
  assert.equal(c.id, "creature-0");
  assert.equal(c.name, "Frostling");
  assert.equal(c.icon, 42);
  assert.equal(c.level, 5);
  assert.equal(c.hp, 30);
  assert.equal(c.maxHp, 60);
  assert.equal(c.pickupMode, "Group");
  assert.equal(c.summoned, true);
  assert.equal(c.lifespan, 750, "lifespan derived from fullness");
  assert.equal(c.maxLifespan, 1000);
});

check("adaptCreatures derives id/name from index when slot/name missing", () => {
  const creatures = adaptCreatures([{ icon: 0 }, { petName: "Named" }]);
  assert.equal(creatures.length, 2);
  assert.equal(creatures[0].id, "creature-0");
  assert.equal(creatures[0].name, "Pet 1");
  assert.equal(creatures[0].icon, undefined, "icon <= 0 is dropped");
  assert.equal(creatures[1].name, "Named");
});

check("adaptCreatures pickupModeLabel covers all modes + unknown", () => {
  const modes = [0, 1, 2, 3, 4, 5, 9].map(
    (m) => adaptCreatures([{ slotIndex: m, petMode: m }])[0].pickupMode,
  );
  assert.deepEqual(modes, ["Both", "Group", "Guild", "None", "Attack", "Move", "Mode 9"]);
  // No petMode -> undefined label.
  assert.equal(adaptCreatures([{ slotIndex: 0 }])[0].pickupMode, undefined);
});

check("adaptCreatures skips non-object entries and tolerates junk", () => {
  const creatures = adaptCreatures([null, 5, "x", { slotIndex: 1, customName: "Keep" }]);
  assert.equal(creatures.length, 1);
  assert.equal(creatures[0].name, "Keep");
  for (const input of MALFORMED_INPUTS) {
    assert.doesNotThrow(() => adaptCreatures(input));
    if (!Array.isArray(input)) {
      assert.deepEqual(adaptCreatures(input), [], "non-array returns []");
    }
  }
});

// ---------------------------------------------------------------------------
// adaptGroup
// ---------------------------------------------------------------------------

check("adaptGroup marks the first member leader and passes lootMode", () => {
  const group = adaptGroup({ members: ["Alice", "Bob", "Cara"], lootMode: "group" });
  assert.ok(group);
  assert.equal(group.lootMode, "group");
  assert.equal(group.members.length, 3);
  assert.deepEqual(group.members[0], { name: "Alice", leader: true });
  assert.equal(group.members[1].leader, false);
  assert.equal(group.members[2].leader, false);
});

check("adaptGroup enriches members but keeps canonical name", () => {
  const group = adaptGroup(
    { members: ["Alice", "Ghost"] },
    {
      enrich: (name) =>
        name === "Alice" ? { online: true, level: 40, name: "SHOULD_BE_OVERRIDDEN" } : undefined,
    },
  );
  assert.equal(group.members[0].name, "Alice", "enrich cannot override the canonical name");
  assert.equal(group.members[0].online, true);
  assert.equal(group.members[0].level, 40);
  assert.equal(group.members[0].leader, true);
  // Unenriched member is just name + leader flag.
  assert.deepEqual(group.members[1], { name: "Ghost", leader: false });
});

check("adaptGroup returns null/empty gracefully", () => {
  assert.equal(adaptGroup(null), null);
  assert.equal(adaptGroup(undefined), null);
  const empty = adaptGroup({});
  assert.deepEqual(empty.members, []);
  assert.equal(empty.lootMode, undefined);
  // Non-string members are filtered out.
  const filtered = adaptGroup({ members: ["Real", 5, null, "Also"] });
  assert.deepEqual(
    filtered.members.map((m) => m.name),
    ["Real", "Also"],
  );
});

check("adaptGroup reads the enriched object member shape", () => {
  const group = adaptGroup({
    members: [
      { name: "Alice", level: 40, class: "Wizard", hp: 80, maxHp: 100, online: true },
      { name: "Bob", level: 22, class: "Warrior", hp: 30, maxHp: 60, online: false },
    ],
    lootMode: "group",
  });
  assert.ok(group);
  assert.equal(group.lootMode, "group");
  assert.equal(group.members.length, 2);
  // First member is the leader (no explicit leaderName -> index 0).
  assert.deepEqual(group.members[0], {
    name: "Alice",
    leader: true,
    level: 40,
    classKey: "wizard",
    hp: 80,
    maxHp: 100,
    online: true,
  });
  assert.deepEqual(group.members[1], {
    name: "Bob",
    leader: false,
    level: 22,
    classKey: "warrior",
    hp: 30,
    maxHp: 60,
    online: false,
  });
});

check("adaptGroup honours top-level leaderName over index-0 fallback", () => {
  const group = adaptGroup({
    leaderName: "Bob",
    members: [{ name: "Alice" }, { name: "Bob" }, { name: "Cara" }],
  });
  assert.equal(group.members[0].leader, false, "Alice is not the named leader");
  assert.equal(group.members[1].leader, true, "Bob matches leaderName");
  assert.equal(group.members[2].leader, false);
});

check("adaptGroup handles mixed legacy strings + enriched objects", () => {
  const group = adaptGroup({
    members: ["Alice", { name: "Bob", level: 10, online: true }, null, { level: 5 }],
  });
  // Bare string -> name only; object with no name -> dropped; nameless obj dropped.
  assert.deepEqual(
    group.members.map((m) => m.name),
    ["Alice", "Bob"],
  );
  assert.deepEqual(group.members[0], { name: "Alice", leader: true });
  assert.equal(group.members[1].level, 10);
  assert.equal(group.members[1].online, true);
  assert.equal(group.members[1].leader, false);
});

check("adaptGroup enrich layers in but adapter owns name + leader", () => {
  const group = adaptGroup(
    {
      leaderName: "Bob",
      members: [{ name: "Alice", online: true }, { name: "Bob" }],
    },
    {
      // enrich tries (and fails) to hijack name + leader.
      enrich: (name) => (name === "Alice" ? { name: "X", leader: true, level: 99 } : undefined),
    },
  );
  assert.equal(group.members[0].name, "Alice", "canonical name wins");
  assert.equal(group.members[0].leader, false, "leaderName=Bob means Alice is not leader");
  assert.equal(group.members[0].level, 99, "other enrich fields are kept");
  assert.equal(group.members[0].online, true, "object-shape fields are kept");
  assert.equal(group.members[1].leader, true);
});

// ---------------------------------------------------------------------------
// adaptFriends
// ---------------------------------------------------------------------------

check("adaptFriends splits friends/blocked from string arrays", () => {
  const social = adaptFriends({ friends: ["Pat", "Sam"], blocked: ["Troll"] });
  assert.ok(social);
  assert.deepEqual(
    social.friends.map((f) => f.name),
    ["Pat", "Sam"],
  );
  assert.deepEqual(
    social.blocked.map((f) => f.name),
    ["Troll"],
  );
});

check("adaptFriends enriches entries but keeps canonical name", () => {
  const social = adaptFriends(
    { friends: ["Pat"] },
    { enrich: (name) => ({ online: true, memo: `memo:${name}`, name: "X" }) },
  );
  assert.equal(social.friends[0].name, "Pat");
  assert.equal(social.friends[0].online, true);
  assert.equal(social.friends[0].memo, "memo:Pat");
});

check("adaptFriends returns null/empty gracefully", () => {
  assert.equal(adaptFriends(null), null);
  assert.equal(adaptFriends(undefined), null);
  const empty = adaptFriends({});
  assert.deepEqual(empty.friends, []);
  assert.deepEqual(empty.blocked, []);
});

check("adaptFriends reads the enriched object entry shape", () => {
  const social = adaptFriends({
    friends: [
      { name: "Pat", online: true, memo: "guildmate", level: 35, mapName: "Bichon" },
      { name: "Sam", online: false, level: 12 },
    ],
    blocked: [{ name: "Troll", memo: "spammer" }],
  });
  assert.ok(social);
  // mapName -> location; online/memo/level carried through.
  assert.deepEqual(social.friends[0], {
    name: "Pat",
    online: true,
    memo: "guildmate",
    level: 35,
    location: "Bichon",
  });
  assert.deepEqual(social.friends[1], { name: "Sam", online: false, level: 12 });
  assert.deepEqual(social.blocked[0], { name: "Troll", memo: "spammer" });
});

check("adaptFriends handles mixed legacy strings + enriched objects", () => {
  const social = adaptFriends({
    friends: ["Pat", { name: "Sam", online: true }, null, { online: true }],
  });
  // Bare string -> name only; nameless object dropped.
  assert.deepEqual(
    social.friends.map((f) => f.name),
    ["Pat", "Sam"],
  );
  assert.deepEqual(social.friends[0], { name: "Pat" });
  assert.equal(social.friends[1].online, true);
});

check("adaptFriends enrich (e.g. lastSeen) layers in but name stays canonical", () => {
  const social = adaptFriends(
    { friends: [{ name: "Pat", online: false, mapName: "Town" }] },
    { enrich: (name) => ({ name: "X", lastSeen: `yesterday:${name}` }) },
  );
  assert.equal(social.friends[0].name, "Pat");
  assert.equal(social.friends[0].location, "Town");
  assert.equal(social.friends[0].lastSeen, "yesterday:Pat");
});

check("adaptFriends preserves actual character indexes including zero and never invents them from array positions", () => {
  const social = adaptFriends({friends: ["Legacy", {name: "Zero", index: 0, online: false},
    {name: "Maximum", index: 2147483647, memo: "memo"}, {name: "Different", index: 42}],
    blocked: [{name: "Blocked", index: 9}]});
  assert.deepEqual(social.friends.map(row => row.index), [undefined, 0, 2147483647, 42]);
  assert.deepEqual(social.blocked[0], {name: "Blocked", index: 9});
  assert.equal(social.friends[1].online, false); assert.equal(social.friends[2].memo, "memo");
});

check("adaptFriends rejects malformed character indexes and enrichment cannot replace received identity", () => {
  for (const index of [-1, 2147483648, 1.5, NaN, Infinity, "5", null]) {
    const social = adaptFriends({friends: [{name: "Pat", index}]});
    assert.deepEqual(social.friends[0], {name: "Pat"});
  }
  const social = adaptFriends({friendInfos: [{name: "Pat", index: 7, online: true}], friends: ["Legacy"]}, {
    enrich: () => ({name: "Wrong", index: 9, memo: "extra"}),
  });
  assert.deepEqual(social.friends[0], {name: "Pat", index: 7, online: true, memo: "extra"});
  assert.deepEqual(adaptFriends({friends: ["Pat"]}, {enrich: () => ({index: -1})}).friends[0], {name: "Pat"});
});

// ---------------------------------------------------------------------------
// adaptRelationship
// ---------------------------------------------------------------------------

check("adaptRelationship maps LoverUpdate-style fields", () => {
  const rel = adaptRelationship({
    partnerName: "Juliet",
    mapName: "Bichon",
    marriedDays: 99,
    allowMarriage: true,
    pendingRequestFrom: "Romeo",
  });
  assert.ok(rel);
  assert.equal(rel.partnerName, "Juliet");
  assert.equal(rel.partnerMap, "Bichon");
  assert.equal(rel.marriedDays, 99);
  assert.equal(rel.allowMarriage, true);
  assert.equal(rel.pendingRequestFrom, "Romeo");
});

check("adaptRelationship reads alternate keys + divorce request", () => {
  const rel = adaptRelationship({
    name: "Spouse",
    partnerMap: "Town",
    pendingDivorceFrom: "Spouse",
  });
  assert.equal(rel.partnerName, "Spouse");
  assert.equal(rel.partnerMap, "Town");
  assert.equal(rel.pendingRequestFrom, "Spouse");
});

check("adaptRelationship returns null / tolerates junk", () => {
  assert.equal(adaptRelationship(null), null);
  assert.equal(adaptRelationship(undefined), null);
  for (const input of MALFORMED_INPUTS) {
    assert.doesNotThrow(() => adaptRelationship(input));
  }
  // Empty object yields an all-undefined summary (not null — record exists).
  const empty = adaptRelationship({});
  assert.ok(empty);
  assert.equal(empty.partnerName, undefined);
});

// ---------------------------------------------------------------------------
// adaptMentor
// ---------------------------------------------------------------------------

check("adaptMentor maps mentor slice", () => {
  const mentor = adaptMentor({
    name: "Sensei",
    level: 50,
    online: true,
    menteeExp: 1234,
    allowMentor: false,
    pendingRequestFrom: "Newbie",
  });
  assert.ok(mentor);
  assert.equal(mentor.name, "Sensei");
  assert.equal(mentor.level, 50);
  assert.equal(mentor.online, true);
  assert.equal(mentor.menteeExp, 1234);
  assert.equal(mentor.allowMentor, false);
  assert.equal(mentor.pendingRequestFrom, "Newbie");
});

check("adaptMentor returns null / tolerates junk", () => {
  assert.equal(adaptMentor(null), null);
  assert.equal(adaptMentor(undefined), null);
  for (const input of MALFORMED_INPUTS) {
    assert.doesNotThrow(() => adaptMentor(input));
  }
});

// ---------------------------------------------------------------------------
// Ranking: rankingTabKey / rankingPageKeyForTab / adaptRankingPage
// ---------------------------------------------------------------------------

check("rankingTabKey maps (rankType, onlineOnly)", () => {
  assert.equal(rankingTabKey(0, false), "overall");
  assert.equal(rankingTabKey(1, false), "warrior");
  assert.equal(rankingTabKey(2, false), "wizard");
  assert.equal(rankingTabKey(3, false), "taoist");
  assert.equal(rankingTabKey(4, false), "assassin");
  assert.equal(rankingTabKey(5, false), "archer");
  // F11: onlineOnly filters the selected class; it no longer replaces that
  // class with a separate "online" tab as the historical adapter did.
  assert.equal(rankingTabKey(3, true), "taoist");
  // Unknown rankType -> overall.
  assert.equal(rankingTabKey(99, false), "overall");
});

check("rankingPageKeyForTab is the inverse-ish key builder", () => {
  assert.equal(rankingPageKeyForTab("overall"), "0:all");
  assert.equal(rankingPageKeyForTab("warrior"), "1:all");
  assert.equal(rankingPageKeyForTab("wizard"), "2:all");
  assert.equal(rankingPageKeyForTab("taoist"), "3:all");
  assert.equal(rankingPageKeyForTab("assassin"), "4:all");
  assert.equal(rankingPageKeyForTab("archer"), "5:all");
  assert.equal(rankingPageKeyForTab("online"), "0:online");
  // round-trip: key -> tab -> key holds for the class tabs.
  for (const tab of ["overall", "warrior", "wizard", "taoist", "assassin", "archer"]) {
    const key = rankingPageKeyForTab(tab);
    const [rankType] = key.split(":");
    assert.equal(rankingTabKey(Number(rankType), false), tab);
  }
});

check("adaptRankingPage maps entries, drops nameless rows, fills defaults", () => {
  const page = adaptRankingPage({
    rankType: 1,
    onlineOnly: false,
    myRank: 7,
    count: 2,
    entries: [
      { rank: 1, playerId: 100, name: "Top", level: 60, classKey: "Warrior" },
      { name: "NoRank", level: 5, classKey: 9 }, // unknown classKey -> warrior default
      { playerId: 5, level: 3 }, // no name -> dropped
      "junk",
      null,
    ],
  });
  assert.ok(page);
  assert.equal(page.rankType, 1);
  assert.equal(page.onlineOnly, false);
  assert.equal(page.myRank, 7);
  assert.equal(page.entries.length, 2, "nameless / junk rows are dropped");
  assert.deepEqual(page.entries[0], {
    rank: 1,
    playerId: 100,
    name: "Top",
    level: 60,
    classKey: "warrior",
  });
  // Second row: rank defaults to index+1 (=> 2), playerId defaults to 0,
  // level (5) is preserved, unknown classKey (9) falls back to warrior.
  assert.deepEqual(page.entries[1], {
    rank: 2,
    playerId: 0,
    name: "NoRank",
    level: 5,
    classKey: "warrior",
  });
});

check("adaptRankingPage defaults count to entries length + fills row defaults", () => {
  const page = adaptRankingPage({
    entries: [{ name: "Only" }], // no rank/playerId/level/classKey
  });
  assert.equal(page.count, 1, "count falls back to entries.length");
  assert.equal(page.rankType, 0);
  assert.equal(page.onlineOnly, false);
  assert.equal(page.myRank, 0);
  // Absent numeric fields default to 0; absent classKey -> warrior.
  assert.deepEqual(page.entries[0], {
    rank: 1,
    playerId: 0,
    name: "Only",
    level: 0,
    classKey: "warrior",
  });
});

check("adaptRankingPage returns null / tolerates junk", () => {
  assert.equal(adaptRankingPage(null), null);
  assert.equal(adaptRankingPage(undefined), null);
  for (const input of MALFORMED_INPUTS) {
    assert.doesNotThrow(() => adaptRankingPage(input));
  }
  // Object with no entries -> empty entries, count 0.
  const empty = adaptRankingPage({});
  assert.ok(empty);
  assert.deepEqual(empty.entries, []);
  assert.equal(empty.count, 0);
});

// ---------------------------------------------------------------------------
// adaptActiveRankingPage
// ---------------------------------------------------------------------------

check("adaptActiveRankingPage resolves tab + page for current key", () => {
  const rankings = {
    "1:all": {
      rankType: 1,
      onlineOnly: false,
      myRank: 3,
      count: 1,
      entries: [{ rank: 1, name: "Hero", playerId: 1, level: 9, classKey: "Warrior" }],
    },
    "0:online": {
      rankType: 0,
      onlineOnly: true,
      entries: [{ rank: 1, name: "OnlineHero", playerId: 2, level: 8, classKey: "Wizard" }],
    },
  };
  const warriorResult = adaptActiveRankingPage(rankings, "1:all");
  assert.equal(warriorResult.tab, "warrior");
  assert.ok(warriorResult.page);
  assert.equal(warriorResult.page.entries[0].name, "Hero");

  const onlineResult = adaptActiveRankingPage(rankings, "0:online");
  assert.equal(onlineResult.tab, "overall", "onlineOnly preserves the packet's overall rankType");
});

check("adaptActiveRankingPage falls back to overall/null for missing key", () => {
  assert.deepEqual(adaptActiveRankingPage(null, "1:all"), { tab: "overall", page: null });
  assert.deepEqual(adaptActiveRankingPage({}, null), { tab: "overall", page: null });
  assert.deepEqual(adaptActiveRankingPage({ "1:all": {} }, "nope"), {
    tab: "overall",
    page: null,
  });
});

check("ranking online filter retains each selected rankType and current absolute page offset", () => {
  const tabs = ["overall", "warrior", "wizard", "taoist", "assassin", "archer"];
  for (const [rankType, tab] of tabs.entries()) for (const onlineOnly of [false, true]) {
    const key = rankType + (onlineOnly ? ":online" : ":all");
    const raw = {rankType, rankIndex: 20, onlineOnly, count: 44, entries: [{name: "Next"},
      {name: "Explicit", rank: 99, classKey: "Wizard"}, {}, {name: "After gap"}]};
    const active = adaptActiveRankingPage({[key]: raw}, key);
    assert.equal(active.tab, tab); assert.equal(active.page.rankType, rankType);
    assert.equal(active.page.onlineOnly, onlineOnly); assert.equal(active.page.rankIndex, 20);
    assert.deepEqual(active.page.entries.map(row => row.rank), [21, 99, 24], "raw row positions preserve rank gaps");
    assert.equal(active.page.count, 44); assert.equal(active.page.entries[1].classKey, "wizard");
  }
});

check("adaptRankingPage permits only actual nonnegative i32 offsets and preserves existing explicit ranks", () => {
  for (const rankIndex of [0, 2147483647]) {
    const page = adaptRankingPage({rankType: 3, rankIndex, onlineOnly: true, entries: [{name: "Taoist"}]});
    assert.equal(page.rankIndex, rankIndex); assert.equal(page.entries[0].rank, rankIndex + 1);
    assert.equal(page.rankType, 3); assert.equal(page.onlineOnly, true);
  }
  for (const rankIndex of [-1, 2147483648, NaN, Infinity, 1.5, "20", null]) {
    const page = adaptRankingPage({rankIndex, entries: [{name: "Default"}, {name: "Explicit", rank: 8}]});
    assert.equal(page.rankIndex, undefined); assert.deepEqual(page.entries.map(row => row.rank), [1, 8]);
  }
});

// ---------------------------------------------------------------------------
// adaptMarketListings
// ---------------------------------------------------------------------------

check("adaptMarketListings maps auction rows with all fields", () => {
  const listings = adaptMarketListings([
    {
      id: "auc-1",
      item: "Dragon Sword",
      seller: "Merchant",
      price: 5000,
      icon: 12,
      count: 1,
      state: "Open",
    },
  ]);
  assert.equal(listings.length, 1);
  assert.deepEqual(listings[0], {
    id: "auc-1",
    itemName: "Dragon Sword",
    seller: "Merchant",
    price: 5000,
    icon: 12,
    count: 1,
    state: "Open",
    mine: false,
  });
});

check("adaptMarketListings flags viewer-owned listings", () => {
  const listings = adaptMarketListings(
    [
      { id: "a", item: "Mine", seller: "Me", price: 10 },
      { id: "b", item: "Theirs", seller: "Other", price: 20 },
      { id: "c", item: "Explicit", seller: "Other", price: 30, isOwner: true },
    ],
    { viewerName: "Me" },
  );
  assert.equal(listings[0].mine, true, "seller === viewer -> mine");
  assert.equal(listings[1].mine, false);
  assert.equal(listings[2].mine, true, "explicit isOwner flag wins");
});

check("adaptMarketListings derives defaults + alternate keys", () => {
  const listings = adaptMarketListings([
    { listingId: "L9", itemName: "Alt", owner: "Owner", gold: 75, image: 3, quantity: 4, status: "Sold" },
    {}, // fully empty row
  ]);
  assert.equal(listings[0].id, "L9");
  assert.equal(listings[0].seller, "Owner");
  assert.equal(listings[0].price, 75, "gold maps to price");
  assert.equal(listings[0].icon, 3, "image maps to icon");
  assert.equal(listings[0].count, 4, "quantity maps to count");
  assert.equal(listings[0].state, "Sold");
  // Empty row gets indexed fallbacks.
  assert.equal(listings[1].id, "listing-1");
  assert.equal(listings[1].itemName, "Listing 2");
  assert.equal(listings[1].seller, "Market");
  assert.equal(listings[1].price, 0);
});

check("adaptMarketListings returns [] for non-arrays / junk", () => {
  for (const input of MALFORMED_INPUTS) {
    assert.doesNotThrow(() => adaptMarketListings(input));
    if (!Array.isArray(input)) {
      assert.deepEqual(adaptMarketListings(input), []);
    }
  }
  // Non-object rows are skipped.
  assert.deepEqual(adaptMarketListings([null, 1, "x"]), []);
});

check("adaptMarketListings reads the enriched auction shape", () => {
  const listings = adaptMarketListings([
    {
      id: "auc-7",
      itemName: "Mystery Helmet",
      seller: "Bidder",
      price: 1000,
      type: "Armour",
      level: 30,
      expiry: "2h 15m",
      highestBid: 1500,
      auction: true,
      sold: false,
    },
  ]);
  assert.equal(listings.length, 1);
  const l = listings[0];
  // Existing baseline fields still resolve from the camelCase keys.
  assert.equal(l.id, "auc-7");
  assert.equal(l.itemName, "Mystery Helmet");
  assert.equal(l.seller, "Bidder");
  assert.equal(l.price, 1000);
  assert.equal(l.mine, false);
  // Enriched optional fields are forwarded verbatim.
  assert.equal(l.type, "Armour");
  assert.equal(l.level, 30);
  assert.equal(l.expiry, "2h 15m");
  assert.equal(l.highestBid, 1500);
  assert.equal(l.auction, true);
  assert.equal(l.sold, false);
});

check("adaptMarketListings keeps fixed-price rows additive (no enriched keys)", () => {
  // A legacy row with none of the enriched keys must NOT gain undefined slots.
  const [listing] = adaptMarketListings([
    { id: "p1", itemName: "Plain Sword", seller: "Smith", price: 250 },
  ]);
  assert.deepEqual(listing, {
    id: "p1",
    itemName: "Plain Sword",
    seller: "Smith",
    price: 250,
    icon: undefined,
    count: undefined,
    state: undefined,
    mine: false,
  });
  for (const key of ["type", "level", "expiry", "highestBid", "auction", "sold"]) {
    assert.ok(!(key in listing), `enriched key "${key}" should be absent on a fixed-price row`);
  }
});

check("adaptMarketListings surfaces city-currency pricing, omits it for gold", () => {
  const [cityListing, goldListing] = adaptMarketListings([
    { id: "c1", itemName: "Bone Ring", seller: "Scout", price: 120, currency: "feitian" },
    { id: "g1", itemName: "Plain Sword", seller: "Smith", price: 250, currency: "gold" },
  ]);
  assert.equal(cityListing.currency, "feitian");
  assert.equal(cityListing.currencyLabel, "飞天城币");
  // Gold (the default) must NOT add the optional keys, keeping legacy rows clean.
  assert.ok(!("currency" in goldListing), "gold listing should omit currency");
  assert.ok(!("currencyLabel" in goldListing), "gold listing should omit currencyLabel");
});

check("adaptTrade surfaces the partner's city-currency offer", () => {
  const trade = adaptTrade({ partner: "Buyer", state: "open", offeredCurrency: "bichon" });
  assert.equal(trade.partnerCurrency, "bichon");
  assert.equal(trade.partnerCurrencyLabel, "比奇城币");
  const goldTrade = adaptTrade({ partner: "Buyer", state: "open", offeredCurrency: "gold" });
  assert.ok(!("partnerCurrency" in goldTrade), "gold trade should omit partnerCurrency");
});

check("adaptMarketListings reads alternate enriched keys + drops non-positive bid handling", () => {
  const [listing] = adaptMarketListings([
    { id: "x", itemName: "Robe", seller: "S", price: 5, category: "Robe", requiredLevel: 12 },
  ]);
  // `category` -> type, `requiredLevel` -> level.
  assert.equal(listing.type, "Robe");
  assert.equal(listing.level, 12);
  // No auction/expiry/highestBid/sold keys -> those stay absent.
  assert.ok(!("auction" in listing));
  assert.ok(!("expiry" in listing));
  assert.ok(!("highestBid" in listing));
  assert.ok(!("sold" in listing));
});

// ---------------------------------------------------------------------------
// adaptConquest
// ---------------------------------------------------------------------------

check("adaptConquest maps castle/war/structure data", () => {
  const conquest = adaptConquest({
    castleOwner: "Sabuk Guild",
    activeWars: ["Guild A", "Guild B"],
    eventLog: ["War declared"],
    taxRatePercent: 15,
    gold: 100000,
    guards: [100, 50, 0],
    walls: [80, 90],
    gates: [100],
    openGates: [0],
  });
  assert.ok(conquest);
  assert.equal(conquest.castleOwner, "Sabuk Guild");
  assert.deepEqual(conquest.activeWars, ["Guild A", "Guild B"]);
  assert.deepEqual(conquest.eventLog, ["War declared"]);
  assert.equal(conquest.taxRatePercent, 15);
  assert.equal(conquest.gold, 100000);
  assert.deepEqual(conquest.guards, [100, 50, 0]);
  assert.deepEqual(conquest.walls, [80, 90]);
  assert.deepEqual(conquest.gates, [100]);
  assert.deepEqual(conquest.openGates, [0]);
});

check("adaptConquest reads alternate keys + filters bad array entries", () => {
  const conquest = adaptConquest({
    owner: "Owner",
    taxRate: 5,
    guards: [10, "bad", null, 20, NaN],
  });
  assert.equal(conquest.castleOwner, "Owner");
  assert.equal(conquest.taxRatePercent, 5);
  assert.deepEqual(conquest.guards, [10, 20], "non-finite numbers filtered out");
  // Missing arrays become [].
  assert.deepEqual(conquest.activeWars, []);
  assert.deepEqual(conquest.walls, []);
});

check("adaptConquest returns null / tolerates junk", () => {
  assert.equal(adaptConquest(null), null);
  assert.equal(adaptConquest(undefined), null);
  for (const input of MALFORMED_INPUTS) {
    assert.doesNotThrow(() => adaptConquest(input));
  }
});

// ---------------------------------------------------------------------------
// adaptGuildTerritory
// ---------------------------------------------------------------------------

check("adaptGuildTerritory maps territory slice", () => {
  const territory = adaptGuildTerritory({
    owned: true,
    mapFileName: "GuildWar",
    rentalDaysLeft: 7,
    recallLog: ["Recalled X"],
  });
  assert.ok(territory);
  assert.equal(territory.owned, true);
  assert.equal(territory.mapFileName, "GuildWar");
  assert.equal(territory.rentalDaysLeft, 7);
  assert.deepEqual(territory.recallLog, ["Recalled X"]);
});

check("adaptGuildTerritory returns null / tolerates junk", () => {
  assert.equal(adaptGuildTerritory(null), null);
  assert.equal(adaptGuildTerritory(undefined), null);
  for (const input of MALFORMED_INPUTS) {
    assert.doesNotThrow(() => adaptGuildTerritory(input));
  }
  const empty = adaptGuildTerritory({});
  assert.ok(empty);
  assert.deepEqual(empty.recallLog, []);
});

// ---------------------------------------------------------------------------
// adaptTrade
// ---------------------------------------------------------------------------

check("adaptTrade returns null when no partner and no state", () => {
  assert.equal(adaptTrade(null), null);
  assert.equal(adaptTrade(undefined), null);
  assert.equal(adaptTrade({}), null);
  for (const input of MALFORMED_INPUTS) {
    assert.doesNotThrow(() => adaptTrade(input));
  }
});

check("adaptTrade maps the baseline incremental slice", () => {
  const trade = adaptTrade({
    partner: "Merchant",
    state: "open",
    partnerGold: 500,
    partnerItemCount: 3,
    confirmed: true,
  });
  assert.ok(trade);
  assert.equal(trade.partner, "Merchant");
  assert.equal(trade.state, "open");
  assert.equal(trade.partnerGold, 500);
  assert.equal(trade.partnerItemCount, 3);
  assert.equal(trade.confirmed, true);
  // No enriched item list present -> partnerItems stays absent.
  assert.equal(trade.partnerItems, undefined);
});

check("adaptTrade maps the enriched contract (partnerName/partnerLocked/partnerItems)", () => {
  const trade = adaptTrade({
    partnerName: "Trader",
    state: "open",
    partnerGold: 1200,
    partnerLocked: true,
    partnerItems: [
      { name: "Dragon Sword", count: 1, grade: "Legend" },
      { name: "Health Potion", count: 25 },
    ],
  });
  assert.ok(trade);
  // partnerName resolves into partner.
  assert.equal(trade.partner, "Trader");
  assert.equal(trade.partnerGold, 1200);
  // partnerLocked -> confirmed.
  assert.equal(trade.confirmed, true);
  // Display ids and physical cells preserve the original ten-slot layout; grade stays absent.
  assert.deepEqual(trade.partnerItems, [
    { id: 0, slot: 0, name: "Dragon Sword", count: 1 },
    { id: 1, slot: 1, name: "Health Potion", count: 25 },
    ...Array(8).fill(null),
  ]);
  // Count is derived from the slot list when not sent explicitly.
  assert.equal(trade.partnerItemCount, 2);
});

check("adaptTrade keeps explicit partnerItemCount over derived slot length", () => {
  const trade = adaptTrade({
    partner: "T",
    partnerItemCount: 9,
    partnerItems: [{ name: "Only One" }],
  });
  assert.equal(trade.partnerItemCount, 9, "explicit count is not overwritten");
  assert.equal(trade.partnerItems.length, 10);
  assert.deepEqual(trade.partnerItems[0], { id: 0, slot: 0, name: "Only One" });
  assert.equal(trade.partnerItems.slice(1).every(row => row === null), true);
});

check("adaptTrade keeps nameless and junk partner rows as empty physical slots", () => {
  const trade = adaptTrade({
    partner: "T",
    partnerItems: [{ name: "Keep", count: 2 }, { count: 5 }, null, "x", {}],
  });
  assert.deepEqual(trade.partnerItems, [{ id: 0, slot: 0, name: "Keep", count: 2 }, ...Array(9).fill(null)]);
});

check("adaptTrade distinguishes a provided empty grid from an absent or invalid grid", () => {
  assert.deepEqual(adaptTrade({ partner: "T", partnerItems: [] }).partnerItems, Array(10).fill(null));
  assert.equal(adaptTrade({ partner: "T", partnerItems: "nope" }).partnerItems, undefined);
  assert.equal(adaptTrade({ partner: "T" }).partnerItems, undefined);
  assert.equal(adaptTrade({ partner: "T", partnerItems: [] }).partnerItemCount, 0);
  assert.equal(adaptTrade({ partner: "T", partnerItems: "nope" }).partnerItemCount, undefined);
});

check("adaptTrade partner slot holes cannot compact a visible offer into another cell", () => {
  const rows = Array(10).fill(null); rows[3] = { name: "Sword", count: 1 }; rows[9] = { itemName: "Potion", quantity: 2, icon: 0 };
  const trade = adaptTrade({ partner: "T", partnerItems: rows });
  assert.deepEqual(trade.partnerItems, [null, null, null, { id: 3, slot: 3, name: "Sword", count: 1 },
    null, null, null, null, null, { id: 9, slot: 9, name: "Potion", count: 2, icon: 0 }]);
  assert.equal(trade.partnerItemCount, 2);
  assert.equal(adaptTrade({ partner: "T", partnerItems: Array(11).fill(null) }).partnerItems, undefined);
});

check("adaptTrade ignores viewer-side fields not on TradeSummary", () => {
  // myGold / myLocked / myItems are separate window props sourced from the page;
  // the adapter must not try to surface them on the summary.
  const trade = adaptTrade({
    partner: "T",
    myGold: 999,
    myLocked: true,
    myItems: [{ name: "Mine" }],
  });
  assert.ok(trade);
  assert.ok(!("myGold" in trade));
  assert.ok(!("myLocked" in trade));
  assert.ok(!("myItems" in trade));
});

// ---------------------------------------------------------------------------
// adaptBuffs
// ---------------------------------------------------------------------------

check("adaptBuffs maps the legacy ActiveBuff shape", () => {
  const buffs = adaptBuffs([
    {
      key: "haste",
      name: "Haste",
      description: "Move faster",
      remainingTicks: 300,
      attackBonus: 5,
      defenceBonus: 2,
    },
  ]);
  assert.equal(buffs.length, 1);
  assert.equal(buffs[0].key, "haste");
  assert.equal(buffs[0].name, "Haste");
  assert.equal(buffs[0].description, "Move faster");
  assert.equal(buffs[0].remainingTicks, 300);
  assert.equal(buffs[0].attackBonus, 5);
  assert.equal(buffs[0].defenceBonus, 2);
  // No enriched fields present.
  assert.equal(buffs[0].kind, undefined);
  assert.equal(buffs[0].infinite, undefined);
  assert.equal(buffs[0].stats, undefined);
});

check("adaptBuffs maps the enriched gateway shape", () => {
  const buffs = adaptBuffs([
    {
      type: "PoisonCloud",
      name: "Poison",
      remainingMs: 5000,
      paused: false,
      caster: "Boss",
      stats: [
        { label: "HP/s", value: -10 },
        { label: "AC", value: 3, suffix: "%" },
      ],
    },
  ]);
  assert.equal(buffs.length, 1);
  const b = buffs[0];
  assert.equal(b.name, "Poison");
  assert.equal(b.kind, "debuff", "PoisonCloud type -> debuff heuristic");
  // remainingMs (5000) -> ticks at 10 ticks/sec -> 50.
  assert.equal(b.remainingTicks, 50, "remainingMs converted to ticks");
  assert.equal(b.paused, false);
  assert.equal(b.caster, "Boss");
  assert.deepEqual(b.stats, [
    { label: "HP/s", value: -10 },
    { label: "AC", value: 3, suffix: "%" },
  ]);
});

check("adaptBuffs classifies buff vs debuff from type heuristic", () => {
  const kinds = [
    adaptBuffs([{ type: "MagicShield", name: "Shield" }])[0].kind,
    adaptBuffs([{ type: "SomeBuff", name: "X" }])[0].kind,
    adaptBuffs([{ type: "Curse", name: "C" }])[0].kind,
    adaptBuffs([{ type: "Slow", name: "S" }])[0].kind,
    adaptBuffs([{ type: "Frozen", name: "F" }])[0].kind,
    adaptBuffs([{ name: "Unknown" }])[0].kind,
  ];
  // No hint and no "buff" substring -> undefined (treated as a buff by the window).
  assert.deepEqual(kinds, [undefined, "buff", "debuff", "debuff", "debuff", undefined]);
});

check("adaptBuffs honours explicit infinite/paused flags", () => {
  const buff = adaptBuffs([{ name: "Blessing", infinite: true, paused: true }])[0];
  assert.equal(buff.infinite, true);
  assert.equal(buff.paused, true);
});

check("adaptBuffs prefers remainingMs but falls back to remainingTicks", () => {
  // Both present -> remainingMs wins.
  const both = adaptBuffs([{ name: "B", remainingMs: 2000, remainingTicks: 999 }])[0];
  assert.equal(both.remainingTicks, 20, "remainingMs (2000) -> 20 ticks wins over legacy field");
  // Only legacy ticks present -> used directly.
  const legacy = adaptBuffs([{ name: "L", remainingTicks: 123 }])[0];
  assert.equal(legacy.remainingTicks, 123);
});

check("adaptBuffs falls back name to type and drops nameless entries", () => {
  // type-only -> name derived from type.
  const typed = adaptBuffs([{ type: "Regeneration" }]);
  assert.equal(typed.length, 1);
  assert.equal(typed[0].name, "Regeneration");
  // Truly nameless (no name, no type) -> dropped.
  assert.deepEqual(adaptBuffs([{ remainingMs: 100 }]), []);
});

check("adaptBuffs assigns positional keys + passes icon hints through", () => {
  const buffs = adaptBuffs([
    { name: "First" },
    { name: "Second", icon: 7, iconLibrary: "magic" },
    { name: "Third", iconLibrary: "bogus" },
  ]);
  assert.equal(buffs[0].key, "buff-0", "missing key -> positional id");
  assert.equal(buffs[1].key, "buff-1");
  assert.equal(buffs[1].icon, 7);
  assert.equal(buffs[1].iconLibrary, "magic");
  assert.equal(buffs[2].icon, undefined);
  assert.equal(buffs[2].iconLibrary, undefined, "invalid iconLibrary is ignored");
});

check("adaptBuffs filters malformed stat rows", () => {
  const buff = adaptBuffs([
    {
      name: "Mix",
      stats: [
        { label: "AC", value: 5 },
        { label: "noValue" },
        { value: 3 },
        null,
        "junk",
        { label: "Suffixed", value: 2, suffix: "x" },
      ],
    },
  ])[0];
  assert.deepEqual(buff.stats, [
    { label: "AC", value: 5 },
    { label: "Suffixed", value: 2, suffix: "x" },
  ]);
});

check("adaptBuffs returns [] for non-arrays / junk and skips bad rows", () => {
  for (const input of MALFORMED_INPUTS) {
    assert.doesNotThrow(() => adaptBuffs(input));
    if (!Array.isArray(input)) {
      assert.deepEqual(adaptBuffs(input), []);
    }
  }
  // Non-object rows are skipped.
  assert.deepEqual(adaptBuffs([null, 1, "x", true]), []);
});

const socialOwner = { connectionGeneration: 3, sessionGeneration: 5, sceneRevision: 7,
  playerObjectId: 11, mapFileName: "0", socket: {} };
const socialKinds = ["group", "marriage", "divorce", "mentor"];

check("social incoming same-name packets and independent module loads receive fresh epochs", () => {
  const ledger = new socialReplies.SocialIncomingReplies();
  assert.equal(ledger.receive("group", "Alice", undefined, socialOwner), true);
  const first = ledger.list(socialOwner)[0];
  assert.equal(ledger.receive("group", "Alice", undefined, socialOwner), true);
  const next = ledger.list(socialOwner)[0];
  assert.equal(next.name, first.name); assert.ok(next.epoch > first.epoch);
  assert.equal(ledger.reserve("group", first.epoch, true, socialOwner), null);
  const reloaded = loadTypeScriptModule(new URL("../lib/social-incoming-replies.ts", import.meta.url));
  const successor = new reloaded.SocialIncomingReplies();
  assert.equal(successor.receive("group", "Alice", undefined, socialOwner), true);
  assert.ok(successor.list(socialOwner)[0].epoch > next.epoch, "a host remount cannot recycle an old button epoch");
});

check("social replies reject each stale owner dimension and require the exact pending epoch", () => {
  for (const field of Object.keys(socialOwner)) {
    const ledger = new socialReplies.SocialIncomingReplies();
    ledger.receive("mentor", "Alice", 22, socialOwner);
    const request = ledger.list(socialOwner)[0], proof = ledger.reserve("mentor", request.epoch, false, socialOwner);
    const changed = { ...socialOwner, [field]: field === "socket" ? {} : field === "mapFileName" ? "1" : socialOwner[field] + 1 };
    const command = socialReplies.socialReplyCommand(proof);
    assert.deepEqual(ledger.list(changed), [], field);
    assert.equal(ledger.reserve("mentor", request.epoch, true, changed), null, field);
    assert.equal(ledger.allows(proof, changed, command), false, field);
    assert.equal(ledger.claim(proof, changed, command), false, field);
    assert.equal(ledger.claim(proof, null, command), false, "closed/disconnected owner");
    assert.equal(ledger.reserve("mentor", request.epoch + 1, true, socialOwner), null, "epoch mismatch");
  }
});

check("social window close retires its kinds independently and session retirement rejects every old proof", () => {
  const ledger = new socialReplies.SocialIncomingReplies();
  for (const kind of socialKinds) ledger.receive(kind, "Alice", 12, socialOwner);
  const requests = ledger.list(socialOwner), proofs = requests.map(row => ledger.reserve(row.kind, row.epoch, true, socialOwner));
  ledger.dismiss("group");
  assert.deepEqual(ledger.list(socialOwner).map(row => row.kind), ["marriage", "divorce", "mentor"]);
  assert.equal(ledger.claim(proofs[0], socialOwner, socialReplies.socialReplyCommand(proofs[0])), false);
  assert.equal(ledger.reserve("group", requests[0].epoch, false, socialOwner), null);
  ledger.retire(); assert.deepEqual(ledger.list(socialOwner), []);
  for (const proof of proofs) assert.equal(ledger.claim(proof, socialOwner, socialReplies.socialReplyCommand(proof)), false);
});

check("social same-kind reentrant receive retires only the old request and preserves other pending kinds", () => {
  for (const kind of socialKinds) {
    const ledger = new socialReplies.SocialIncomingReplies();
    for (const entry of socialKinds) ledger.receive(entry, "Alice", 12, socialOwner);
    const before = ledger.list(socialOwner), old = before.find(row => row.kind === kind);
    const proof = ledger.reserve(kind, old.epoch, true, socialOwner);
    assert.equal(ledger.allows(proof, socialOwner, socialReplies.socialReplyCommand(proof)), true);
    ledger.receive(kind, "Alice", 13, socialOwner);
    const fresh = ledger.list(socialOwner).find(row => row.kind === kind);
    assert.ok(fresh.epoch > old.epoch); assert.equal(fresh.level, 13);
    assert.equal(ledger.claim(proof, socialOwner, socialReplies.socialReplyCommand(proof)), false);
    ledger.cancelDefinitelyUnsent(proof); ledger.finish(proof);
    assert.equal(ledger.list(socialOwner).find(row => row.kind === kind), fresh);
    for (const row of before.filter(row => row.kind !== kind))
      assert.equal(ledger.list(socialOwner).find(current => current.kind === row.kind), row);
  }
});

check("social four ordinary reply wires require a boolean and exact two-key command then claim once", () => {
  const wires = { group: "groupInvite", marriage: "marriageReply", divorce: "divorceReply", mentor: "mentorReply" };
  for (const kind of socialKinds) for (const acceptInvite of [true, false]) {
    const ledger = new socialReplies.SocialIncomingReplies(); ledger.receive(kind, "Alice", undefined, socialOwner);
    const request = ledger.list(socialOwner)[0];
    for (const bad of [0, 1, "true", "false", null, undefined]) assert.equal(ledger.reserve(kind, request.epoch, bad, socialOwner), null);
    const proof = ledger.reserve(kind, request.epoch, acceptInvite, socialOwner), command = socialReplies.socialReplyCommand(proof);
    assert.deepEqual(command, { type: wires[kind], acceptInvite });
    assert.equal(ledger.reserve(kind, request.epoch, acceptInvite, socialOwner), null, "one reservation per incoming request");
    for (const bad of [{ ...command, acceptInvite: String(acceptInvite) }, { ...command, acceptInvite: !acceptInvite },
      { ...command, type: "wrongReply" }, { ...command, requestId: request.epoch }])
      assert.equal(ledger.claim(proof, socialOwner, bad), false);
    assert.equal(ledger.claim(proof, socialOwner, command), true);
    assert.equal(ledger.claim(proof, socialOwner, command), false);
    assert.deepEqual(ledger.list(socialOwner), []); ledger.finish(proof);
    assert.equal(ledger.reserve(kind, request.epoch, acceptInvite, socialOwner), null);
  }
});

check("social definitely-unsent releases only the reservation and cannot restore a replaced request", () => {
  const ledger = new socialReplies.SocialIncomingReplies(); ledger.receive("group", "Alice", undefined, socialOwner);
  const request = ledger.list(socialOwner)[0], old = ledger.reserve("group", request.epoch, true, socialOwner);
  ledger.cancelDefinitelyUnsent(old);
  assert.equal(ledger.list(socialOwner)[0], request);
  const retry = ledger.reserve("group", request.epoch, false, socialOwner); assert.ok(retry);
  assert.equal(ledger.claim(old, socialOwner, socialReplies.socialReplyCommand(old)), false);
  ledger.receive("group", "Alice", undefined, socialOwner); const fresh = ledger.list(socialOwner)[0];
  ledger.cancelDefinitelyUnsent(retry); ledger.finish(retry);
  assert.equal(ledger.list(socialOwner)[0], fresh); assert.ok(fresh.epoch > request.epoch);
  assert.equal(ledger.reserve("group", request.epoch, true, socialOwner), null);
});

check("social entered reply remains consumed after a throwing or unknown transport outcome", () => {
  for (const kind of socialKinds) {
    const ledger = new socialReplies.SocialIncomingReplies(); ledger.receive(kind, "Alice", undefined, socialOwner);
    const request = ledger.list(socialOwner)[0], proof = ledger.reserve(kind, request.epoch, true, socialOwner);
    const command = socialReplies.socialReplyCommand(proof);
    assert.throws(() => {
      assert.equal(ledger.claim(proof, socialOwner, command), true);
      throw Error("injected outcome unknown after final claim");
    }, /injected outcome unknown/);
    ledger.cancelDefinitelyUnsent(proof);
    assert.equal(ledger.allows(proof, socialOwner, command), false);
    assert.equal(ledger.claim(proof, socialOwner, command), false);
    assert.equal(ledger.reserve(kind, request.epoch, false, socialOwner), null);
    ledger.finish(proof); assert.deepEqual(ledger.list(socialOwner), []);
    assert.equal(ledger.reserve(kind, request.epoch, true, socialOwner), null);
  }
});

check("social GuildInvite uses the actual guildInvite boolean wire and claims its current epoch once", () => {
  for (const acceptInvite of [false, true]) {
    const ledger = new socialReplies.SocialIncomingReplies();
    assert.equal(ledger.receive("guild", "Guild", undefined, socialOwner), true);
    const request = ledger.list(socialOwner).find(row => row.kind === "guild"); assert.ok(request);
    for (const value of [0, 1, "true", null, undefined]) assert.equal(ledger.reserve("guild", request.epoch, value, socialOwner), null);
    const proof = ledger.reserve("guild", request.epoch, acceptInvite, socialOwner);
    const command = socialReplies.socialReplyCommand(proof);
    assert.deepEqual(command, {type: "guildInvite", acceptInvite});
    for (const bad of [{...command, acceptInvite: !acceptInvite}, {...command, type: "groupInvite"},
      {...command, epoch: request.epoch}, {...command, acceptInvite: String(acceptInvite)}])
      assert.equal(ledger.claim(proof, socialOwner, bad), false);
    assert.equal(ledger.claim(proof, socialOwner, command), true);
    assert.equal(ledger.claim(proof, socialOwner, command), false);
    ledger.cancelDefinitelyUnsent(proof); ledger.finish(proof);
    assert.equal(ledger.reserve("guild", request.epoch, !acceptInvite, socialOwner), null);
  }
});

check("social same-name GuildInvite replaces only the Guild epoch and keeps other incoming kinds", () => {
  const ledger = new socialReplies.SocialIncomingReplies();
  for (const kind of [...socialKinds, "guild"]) assert.equal(ledger.receive(kind, "Same", 22, socialOwner), true);
  const before = ledger.list(socialOwner), old = before.find(row => row.kind === "guild");
  const proof = ledger.reserve("guild", old.epoch, true, socialOwner);
  assert.equal(ledger.receive("guild", "Same", 23, socialOwner), true);
  const next = ledger.list(socialOwner).find(row => row.kind === "guild"); assert.ok(next.epoch > old.epoch);
  assert.equal(ledger.claim(proof, socialOwner, socialReplies.socialReplyCommand(proof)), false);
  ledger.cancelDefinitelyUnsent(proof); ledger.finish(proof);
  assert.strictEqual(ledger.list(socialOwner).find(row => row.kind === "guild"), next);
  for (const row of before.filter(row => row.kind !== "guild"))
    assert.strictEqual(ledger.list(socialOwner).find(current => current.kind === row.kind), row);
  ledger.dismiss("guild"); assert.deepEqual(ledger.list(socialOwner).map(row => row.kind), socialKinds);
});

check("social Guild reply rejects stale session socket scene epoch and dismissed owner", () => {
  const ledger = new socialReplies.SocialIncomingReplies(); ledger.receive("guild", "Guild", undefined, socialOwner);
  const request = ledger.list(socialOwner)[0], proof = ledger.reserve("guild", request.epoch, true, socialOwner);
  const command = socialReplies.socialReplyCommand(proof);
  for (const field of Object.keys(socialOwner)) {
    const changed = {...socialOwner, [field]: field === "socket" ? {} : field === "mapFileName" ? "1" : socialOwner[field] + 1};
    assert.equal(ledger.claim(proof, changed, command), false, field);
    assert.equal(ledger.reserve("guild", request.epoch, false, changed), null, field);
  }
  assert.equal(ledger.reserve("guild", request.epoch + 1, true, socialOwner), null);
  ledger.dismiss("guild"); assert.equal(ledger.claim(proof, socialOwner, command), false);
  assert.equal(ledger.reserve("guild", request.epoch, true, socialOwner), null);
});

function socialWorldItem(id, container, slot) {
  return { key: `${container}:${slot}`, uniqueId: id, authoritativeUniqueId: id, container, slot,
    name: "Item", quantity: 1, icon: 0, description: "" };
}
function socialWorld() {
  return { inventoryCapacity: 54, maxBagSlots: 48, gold: 0,
    inventoryItems: [socialWorldItem(0, "bag1", 3), socialWorldItem(22, "bag2", 3), socialWorldItem(33, "bag2", 7)],
    beltItems: [], equipmentItems: [] };
}
function socialTrade(patch = {}) {
  return { partner: "Alice", settlementNonce: "ordinary-current-trade", offeredGold: 0,
    accepted: false, locked: false, escrowPrepared: false, completed: false,
    offeredSlots: { 2: 3, 9: 43 }, offeredUniqueIds: { 2: 0, 9: 22 }, ...patch };
}
function guildItems() {
  const rows = Array(112).fill(null);
  rows[3] = { item: { unique_id: 0, item_index: 7, count: 2 }, userId: 11 };
  rows[111] = { item: { unique_id: 22, item_index: 8, count: 1 }, userId: 12 };
  return rows;
}
const socialCatalog = index => index === 7 ? { name: "Guild A", icon: 0 } : index === 8 ? { name: "Guild B", icon: 5 } : null;

check("social inventory uses physical Bag2 slots without a Belt offset and includes every empty cell", () => {
  const inventory = socialItems.projectSocialInventory(socialWorld()); assert.equal(inventory.ok, true);
  assert.deepEqual(inventory.items.map(row => [row.slot, row.uniqueId]), [[3, 0], [43, 22], [47, 33]]);
  assert.equal(inventory.capacity, 48); assert.equal(inventory.emptySlots.length, 45);
  assert.equal(socialItems.validSocialInventoryProjection(inventory), true);
  assert.deepEqual([...inventory.emptySlots, ...inventory.items.map(row => row.slot)].sort((a, b) => a - b), Array.from({ length: 48 }, (_, slot) => slot));
  assert.equal(inventory.items.some(row => row.slot === 49 || row.slot === 53), false, "Belt cells are not added to Bag2 protocol slots");
});

check("social inventory rejects missing arrays, unsafe or duplicate authority, invalid counts and incomplete cells", () => {
  for (const alter of [world => { delete world.beltItems; }, world => { world.equipmentItems = null; },
    world => { delete world.inventoryItems[0].authoritativeUniqueId; },
    world => { world.inventoryItems[0].authoritativeUniqueId = Number.MAX_SAFE_INTEGER + 1; },
    world => { world.inventoryItems[1].authoritativeUniqueId = 0; }, world => { world.inventoryItems[0].quantity = 0; },
    world => { world.inventoryItems[0].quantity = 65536; }]) {
    const world = socialWorld(); alter(world);
    assert.deepEqual(socialItems.projectSocialInventory(world), { ok: false, error: "invalidInventory" });
  }
  const inventory = socialItems.projectSocialInventory(socialWorld());
  assert.equal(socialItems.validSocialInventoryProjection({ ...inventory, emptySlots: inventory.emptySlots.slice(1) }), false);
  assert.equal(socialItems.validSocialInventoryProjection({ ...inventory, emptySlots: [...inventory.emptySlots, inventory.emptySlots[0]] }), false);
});

check("social own trade projects ten exact cells with holes and resolves both slot and UID maps", () => {
  const projected = socialItems.projectOwnTradeItems(socialTrade(), socialItems.projectSocialInventory(socialWorld()));
  assert.equal(projected.ok, true); assert.equal(projected.slots.length, 10);
  assert.deepEqual(projected.slots.map(row => row?.uniqueId ?? null), [null, null, 0, null, null, null, null, null, null, 22]);
  assert.equal(projected.slots[2].slot, 2); assert.equal(projected.slots[9].slot, 9);
  assert.equal(projected.slots[9].count, 1); assert.equal(projected.slots[9].name, "Item");
});

check("social trade refuses map disagreement, repeated identities, bad cells and incomplete live inventory", () => {
  const inventory = socialItems.projectSocialInventory(socialWorld());
  for (const patch of [{ offeredUniqueIds: { 2: 0 } }, { offeredUniqueIds: { 2: 0, 8: 22 } },
    { offeredUniqueIds: { 2: 22, 9: 0 } }, { offeredUniqueIds: { 2: 0, 9: Number.MAX_SAFE_INTEGER + 1 } },
    { offeredSlots: { 2: 3, 9: 3 }, offeredUniqueIds: { 2: 0, 9: 0 } },
    { offeredSlots: { "02": 3, 9: 43 }, offeredUniqueIds: { "02": 0, 9: 22 } },
    { offeredSlots: { 2: 3, 9: 49 } }, { offeredSlots: null }, { settlementNonce: "" }])
    assert.deepEqual(socialItems.projectOwnTradeItems(socialTrade(patch), inventory), { ok: false, error: "invalidTrade" });
  assert.deepEqual(socialItems.projectOwnTradeItems(socialTrade(), { ...inventory, emptySlots: [] }), { ok: false, error: "invalidTrade" });
});

check("social Guild projects the exact nested snake packet into 112 cells without compacting holes", () => {
  const requested = [];
  const projected = socialItems.projectGuildStorageItems(guildItems(), index => { requested.push(index); return socialCatalog(index); });
  assert.equal(projected.ok, true); assert.equal(projected.slots.length, 112);
  assert.deepEqual(requested, [7, 8]);
  assert.deepEqual(projected.slots[3], { slot: 3, uniqueId: 0, name: "Guild A", icon: 0, count: 2 });
  assert.deepEqual(projected.slots[111], { slot: 111, uniqueId: 22, name: "Guild B", icon: 5, count: 1 });
  assert.equal(projected.slots.filter(row => row === null).length, 110);
  assert.equal(projected.slots[0], null); assert.equal(projected.slots[110], null);
});

check("social Guild rejects missing catalog, duplicate or unsafe UID and partial or wrong-shaped lists", () => {
  assert.deepEqual(socialItems.projectGuildStorageItems(guildItems(), () => null), { ok: false, error: "invalidGuildStorage" });
  assert.deepEqual(socialItems.projectGuildStorageItems(guildItems(), () => { throw Error("catalog unavailable"); }), { ok: false, error: "invalidGuildStorage" });
  for (const alter of [rows => { rows.pop(); }, rows => { rows.push(null); }, rows => { delete rows[0]; },
    rows => { rows[111].item.unique_id = 0; }, rows => { rows[3].item.unique_id = Number.MAX_SAFE_INTEGER + 1; },
    rows => { rows[3].item.unique_id = "0"; }, rows => { rows[3].item.uniqueId = 0; },
    rows => { rows[3].item.itemIndex = 7; }, rows => { rows[3].item.count = 0; },
    rows => { rows[3].userId = "11"; }, rows => { rows[3] = rows[3].item; },
    rows => { rows[3].item = { uniqueId: 0, itemIndex: 7, count: 2 }; }]) {
    const rows = guildItems(); alter(rows);
    assert.deepEqual(socialItems.projectGuildStorageItems(rows, socialCatalog), { ok: false, error: "invalidGuildStorage" });
  }
});

check("social item selection requires the unique unchanged physical slot and every displayed carrier field", () => {
  const inventory = socialItems.projectSocialInventory(socialWorld()), selected = inventory.items[1];
  assert.equal(socialItems.currentSocialItem(inventory.items, selected, 48), selected);
  for (const patch of [{ slot: 3 }, { uniqueId: 0 }, { count: 2 }, { name: "Changed" }, { icon: 1 }])
    assert.equal(socialItems.currentSocialItem(inventory.items, { ...selected, ...patch }, 48), null);
  assert.equal(socialItems.currentSocialItem([...inventory.items, { ...selected }], selected, 48), null);
  assert.equal(socialItems.currentSocialItem(inventory.items, { ...selected, uniqueId: Number.MAX_SAFE_INTEGER + 1 }, 48), null);
});

check("Guild permissions decode exactly eight rank bits and unknown masks grant nothing", () => {
  const names = ["CanChangeRank", "CanRecruit", "CanKick", "CanStoreItem", "CanRetrieveItem", "CanAlterAlliance", "CanChangeNotice", "CanActivateBuff"];
  assert.deepEqual(socialOperations.guildPermissionsFromOptions(0), []);
  assert.deepEqual(socialOperations.guildPermissionsFromOptions(255), names);
  for (let bit = 0; bit < 8; bit++) assert.deepEqual(socialOperations.guildPermissionsFromOptions(1 << bit), [names[bit]]);
  for (const bad of [undefined, null, "255", -1, 256, 1.5, NaN, Number.MAX_SAFE_INTEGER + 1])
    assert.equal(socialOperations.guildPermissionsFromOptions(bad), null);
});

check("Guild server change reducer deposits retrieves and swaps exact 112-cell snake items without optimistic mutation", () => {
  const original = guildItems(), item = { item: { unique_id: 33, item_index: 9, count: 4 }, userId: 11 };
  const deposited = socialOperations.applyGuildStorageChange(original, { changeType: 0, from: 43, to: 5, user: 11, item });
  assert.equal(deposited.length, 112); assert.equal(deposited[5], item); assert.equal(deposited[3], original[3]); assert.equal(original[5], null);
  const retrieved = socialOperations.applyGuildStorageChange(original, { changeType: 1, from: 3, to: 43, user: 11, item: null });
  assert.equal(retrieved[3], null); assert.equal(retrieved[111], original[111]); assert.ok(original[3]);
  const swapped = socialOperations.applyGuildStorageChange(original, { changeType: 2, from: 3, to: 111, user: 11, item: original[3] });
  assert.equal(swapped[3], original[111]); assert.equal(swapped[111], original[3]);
  const moved = socialOperations.applyGuildStorageChange(original, { changeType: 2, from: 3, to: 5, user: 11, item: original[3] });
  assert.equal(moved[3], null); assert.equal(moved[5], original[3]); assert.equal(moved[4], null);
});

check("Guild failure variants 3 to 5 keep the existing grid and malformed or incomplete changes cannot mutate it", () => {
  const original = guildItems();
  for (const [changeType, from, to] of [[3, 43, 5], [4, 3, 43], [5, 3, 111]]) {
    const failure = { changeType, from, to, user: 0, item: null };
    assert.equal(socialOperations.applyGuildStorageChange(original, failure), original);
    assert.equal(socialOperations.applyGuildStorageChange(original, { ...failure, item: original[3] }), null);
  }
  for (const packet of [{ changeType: 0, from: 43, to: 5, user: 11, item: null },
    { changeType: 1, from: 3, to: 43, user: 11, item: original[3] },
    { changeType: 2, from: 4, to: 111, user: 11, item: original[3] },
    { changeType: 2, from: 3, to: 111, user: 11, item: original[111] },
    { changeType: 0, from: 43, to: 5, user: 11, item: original[3] },
    { changeType: 6, from: 43, to: 5, user: 11, item: original[3] }])
    assert.equal(socialOperations.applyGuildStorageChange(original, packet), null);
  assert.equal(socialOperations.applyGuildStorageChange(original.slice(1), { changeType: 4, from: 3, to: 43, user: 0, item: null }), null);
});

function operationInput(kind = "depositTrade", patch = {}) {
  const tuples = { depositTrade: [43, 9], retrieveTrade: [9, 47], guild0: [43, 111], guild1: [111, 47], guild2: [3, 111] };
  return { owner: socialOwner, surface: kind.startsWith("guild") ? "guild" : "trade", localSourceKey: "complete-current-source",
    kind, from: tuples[kind][0], to: tuples[kind][1], uniqueId: 22, ownCharacterIndex: 11, beforeSnapshotRevision: 20, ...patch };
}

check("social item operations claim only exact current local proof and ordinary wire once across both surfaces", () => {
  for (const kind of ["depositTrade", "retrieveTrade", "guild0", "guild1", "guild2"]) {
    const operations = new socialOperations.SocialWindowOperations(), input = operationInput(kind), proof = operations.reserve(input);
    assert.ok(proof); const command = socialOperations.socialWindowItemCommand(proof);
    assert.deepEqual(Object.keys(command).sort(), kind.startsWith("guild") ? ["changeType", "from", "to", "type"] : ["from", "to", "type"]);
    assert.equal(command.from, input.from); assert.equal(command.to, input.to);
    assert.equal(operations.claim({ ...proof }, socialOwner, input.localSourceKey, command), false);
    assert.equal(operations.claim(proof, { ...socialOwner, socket: {} }, input.localSourceKey, command), false);
    assert.equal(operations.claim(proof, socialOwner, "changed-source", command), false);
    assert.equal(operations.claim(proof, socialOwner, input.localSourceKey, { ...command, uniqueId: input.uniqueId }), false);
    assert.equal(operations.claim(proof, socialOwner, input.localSourceKey, { ...command, to: input.to - 1 }), false);
    assert.equal(operations.claim(proof, socialOwner, input.localSourceKey, command), true);
    assert.equal(operations.claim(proof, socialOwner, input.localSourceKey, command), false);
    assert.equal(operations.reserve(operationInput(kind === "depositTrade" ? "guild0" : "depositTrade")), null,
      "one unresolved operation blocks every social item surface");
  }
});

check("Trade item receipt requires exact typed tuple and a later complete same-owner snapshot before release", () => {
  for (const kind of ["depositTrade", "retrieveTrade"]) for (const success of [true, false]) {
    const operations = new socialOperations.SocialWindowOperations(), input = operationInput(kind), proof = operations.reserve(input);
    const command = socialOperations.socialWindowItemCommand(proof), packet = kind === "depositTrade" ? "DepositTradeItem" : "RetrieveTradeItem";
    const payload = { from: input.from, to: input.to, success };
    assert.equal(operations.receipt(packet, payload, socialOwner).matched, false, "receipt before entry has no authority");
    operations.claim(proof, socialOwner, input.localSourceKey, command);
    assert.equal(operations.markSnapshot(proof, socialOwner, 21), false, "a world before ACK cannot satisfy the barrier");
    for (const bad of [{ from: input.from, to: input.to }, { ...payload, success: 1 }, { ...payload, requestId: "invented" },
      { ...payload, to: input.to - 1 }]) assert.equal(operations.receipt(packet, bad, socialOwner).matched, false);
    assert.equal(operations.receipt(packet, payload, { ...socialOwner, sessionGeneration: 6 }).matched, false);
    const result = operations.receipt(packet, payload, socialOwner); assert.equal(result.matched, true); assert.equal(result.success, success);
    assert.equal(operations.releaseAfterSnapshot(proof, socialOwner), false, "ACK alone is not a complete world update");
    assert.equal(operations.markSnapshot(proof, socialOwner, 20), false);
    assert.equal(operations.markSnapshot(proof, { ...socialOwner, sceneRevision: 8 }, 21), false);
    assert.equal(operations.markSnapshot(proof, socialOwner, 21), true);
    assert.equal(operations.markSnapshot(proof, socialOwner, 21), false);
    assert.equal(operations.releaseAfterSnapshot(proof, { ...socialOwner, socket: {} }), false);
    assert.equal(operations.releaseAfterSnapshot(proof, socialOwner), true);
    assert.equal(operations.pending, null); assert.equal(operations.releaseAfterSnapshot(proof, socialOwner), false);
  }
});

check("Guild matching receipt uses actual change variants and authenticated sender before unlocking a later snapshot", () => {
  for (const kind of ["guild0", "guild1", "guild2"]) for (const success of [true, false]) {
    const operations = new socialOperations.SocialWindowOperations(), input = operationInput(kind), proof = operations.reserve(input);
    operations.claim(proof, socialOwner, input.localSourceKey, socialOperations.socialWindowItemCommand(proof));
    const requested = Number(kind.slice(-1)), item = success && requested !== 1 ? { item: { unique_id: 22, item_index: 7, count: 1 }, userId: 11 } : null;
    const payload = { changeType: requested + (success ? 0 : 3), from: input.from, to: input.to, user: success ? 11 : 0, item };
    assert.equal(operations.receipt("GuildStorageItemChange", { ...payload, user: 12 }, socialOwner).matched, false,
      "another Guild member's update cannot acknowledge this operation");
    if (item) assert.equal(operations.receipt("GuildStorageItemChange", { ...payload, item: { ...item, item: { ...item.item, unique_id: 23 } } }, socialOwner).matched, false);
    assert.equal(operations.receipt("GuildStorageItemChange", { ...payload, success }, socialOwner).matched, false,
      "Guild uses changeType failure variants, not an invented success field");
    const result = operations.receipt("GuildStorageItemChange", payload, socialOwner);
    assert.equal(result.matched, true); assert.equal(result.success, success);
    assert.equal(operations.releaseAfterSnapshot(proof, socialOwner), false);
    assert.equal(operations.markSnapshot(proof, socialOwner, 21), true);
    assert.equal(operations.releaseAfterSnapshot(proof, socialOwner), true);
  }
});

check("Guild typed true envelope accepts actual six-key changes while foreign broadcasts and malformed flags keep own operation pending", () => {
  for (const kind of ["guild0", "guild1", "guild2"]) for (const success of [true, false]) {
    const original = guildItems(), requested = Number(kind.slice(-1));
    const moving = requested === 0 ? { item: { unique_id: 33, item_index: 9, count: 4 }, userId: 11 }
      : requested === 2 ? original[3] : null;
    const input = operationInput(kind, requested === 0 ? { to: 5, uniqueId: 33 }
      : requested === 2 ? { uniqueId: 0 } : {});
    const operations = new socialOperations.SocialWindowOperations(), proof = operations.reserve(input);
    operations.claim(proof, socialOwner, input.localSourceKey, socialOperations.socialWindowItemCommand(proof));
    const payload = { changeType: requested + (success ? 0 : 3), from: input.from, to: input.to,
      user: success ? 11 : 0, item: success ? moving : null, typed: true };
    for (const bad of [{ ...payload, typed: false }, { ...payload, anotherEnvelopeField: true }]) {
      assert.equal(socialOperations.applyGuildStorageChange(original, bad), null);
      assert.equal(operations.receipt("GuildStorageItemChange", bad, socialOwner).matched, false);
    }
    const broadcast = { ...payload, user: 12 };
    assert.ok(socialOperations.applyGuildStorageChange(original, broadcast), "another member's real broadcast may update the shared grid");
    assert.equal(operations.receipt("GuildStorageItemChange", broadcast, socialOwner).matched, false);
    assert.equal(operations.pending.acked, false); assert.equal(operations.pending.proof, proof);
    assert.equal(operations.markSnapshot(proof, socialOwner, 21), false);
    const next = socialOperations.applyGuildStorageChange(original, payload); assert.ok(next);
    if (!success) assert.equal(next, original);
    else if (requested === 0) assert.equal(next[5], moving);
    else if (requested === 1) assert.equal(next[111], null);
    else { assert.equal(next[3], original[111]); assert.equal(next[111], original[3]); }
    const result = operations.receipt("GuildStorageItemChange", payload, socialOwner);
    assert.equal(result.matched, true); assert.equal(result.success, success);
    assert.equal(operations.releaseAfterSnapshot(proof, socialOwner), false);
    assert.equal(operations.markSnapshot(proof, socialOwner, 22), true);
    assert.equal(operations.releaseAfterSnapshot(proof, socialOwner), true);
  }
});

check("social item unknown outcome and partial or stale receipt keep the barrier without replay until connection retirement", () => {
  const operations = new socialOperations.SocialWindowOperations(), input = operationInput(), proof = operations.reserve(input);
  const command = socialOperations.socialWindowItemCommand(proof);
  assert.equal(operations.cancelDefinitelyUnsent({ ...proof }), false);
  assert.equal(operations.outcomeUnknown(proof), false);
  operations.claim(proof, socialOwner, input.localSourceKey, command);
  assert.equal(operations.outcomeUnknown(proof), true); assert.equal(operations.cancelDefinitelyUnsent(proof), false);
  assert.equal(operations.receipt("DepositTradeItem", { from: input.from, to: input.to }, socialOwner).matched, false);
  assert.equal(operations.markSnapshot(proof, socialOwner, 999), false, "even a newer revision before ACK cannot establish the missing ACK");
  assert.equal(operations.releaseAfterSnapshot(proof, socialOwner), false);
  assert.equal(operations.claim(proof, socialOwner, input.localSourceKey, command), false);
  assert.equal(operations.reserve(operationInput("guild0")), null);
  assert.equal(operations.reserve(operationInput("depositTrade", { owner: { ...socialOwner, socket: {} } })), null,
    "a source or owner change does not by itself retire an unresolved operation");
  assert.equal(operations.pending.proof, proof);
  operations.retire(); assert.equal(operations.pending, null);
  assert.ok(operations.reserve(operationInput("guild0")));
});

check("social item pre-entry cancellation releases only its own proof and never cancels a successor", () => {
  const operations = new socialOperations.SocialWindowOperations(), input = operationInput(), old = operations.reserve(input);
  assert.equal(operations.cancelDefinitelyUnsent(old), true);
  const fresh = operations.reserve(operationInput("guild0")); assert.ok(fresh);
  assert.equal(operations.cancelDefinitelyUnsent(old), false); assert.equal(operations.pending.proof, fresh);
  assert.equal(operations.claim(old, socialOwner, input.localSourceKey, socialOperations.socialWindowItemCommand(old)), false);
  assert.equal(operations.claim(fresh, socialOwner, fresh.localSourceKey, socialOperations.socialWindowItemCommand(fresh)), true);
});

function rentalFacts(patch = {}) {
  return { owner: socialOwner, serviceRevision: 3, hasExpandedStorage: false, expiryObservation: 0,
    storageSize: 80, gold: 1_500_000, ...patch };
}

check("storage rental uses ten days and one million gold for both new storage and renewal without granting state", () => {
  assert.equal(storageRental.STORAGE_RENTAL_DAYS, 10); assert.equal(storageRental.STORAGE_RENTAL_GOLD, 1_000_000);
  for (const hasExpandedStorage of [false, true]) for (const shared of [false, true]) {
    const confirmation = new storageRental.StorageRentalConfirmation();
    const facts = rentalFacts({ hasExpandedStorage, storageSize: hasExpandedStorage ? 160 : 80, expiryObservation: hasExpandedStorage ? 3 : 0 });
    const proof = confirmation.open(facts, shared); assert.ok(proof);
    assert.equal(proof.shared, shared); assert.deepEqual(proof.facts, facts); assert.notEqual(proof.facts, facts);
    assert.equal(confirmation.allows(proof, facts), true);
    assert.equal(confirmation.claim(proof, facts, { type: "chat", message: "@ADDSTORAGE" }), true);
    assert.equal(confirmation.pending, null); assert.equal(confirmation.claim(proof, facts, { type: "chat", message: "@ADDSTORAGE" }), false);
    assert.equal(facts.hasExpandedStorage, hasExpandedStorage); assert.equal(facts.gold, 1_500_000,
      "a local confirmation never changes size expiry or balance");
  }
});

check("storage rental confirmation rejects changed owner service size expiry state and insufficient current balance", () => {
  const confirmation = new storageRental.StorageRentalConfirmation(), facts = rentalFacts(), proof = confirmation.open(facts, false);
  for (const field of Object.keys(socialOwner)) {
    const owner = { ...socialOwner, [field]: field === "socket" ? {} : field === "mapFileName" ? "1" : socialOwner[field] + 1 };
    assert.equal(confirmation.allows(proof, { ...facts, owner }), false, field);
  }
  for (const patch of [{ serviceRevision: 4 }, { storageSize: 160 }, { expiryObservation: 1 },
    { hasExpandedStorage: true }, { gold: 999_999 }]) {
    assert.equal(confirmation.allows(proof, { ...facts, ...patch }), false);
    assert.equal(confirmation.claim(proof, { ...facts, ...patch }, { type: "chat", message: "@ADDSTORAGE" }), false);
  }
  assert.equal(confirmation.allows(proof, { ...facts, gold: 1_000_000 }), true, "current sufficient funds support the fixed price");
  assert.equal(confirmation.pending, proof);
  assert.equal(confirmation.allows(proof, null), false);
  for (const invalid of [null, rentalFacts({ gold: 999_999 }), rentalFacts({ serviceRevision: 0 }),
    rentalFacts({ storageSize: 0 }), rentalFacts({ expiryObservation: NaN })])
    assert.equal(new storageRental.StorageRentalConfirmation().open(invalid, false), null);
});

check("storage rental cancel and exact once claim never let an old prompt cancel or claim a new prompt", () => {
  const confirmation = new storageRental.StorageRentalConfirmation(), facts = rentalFacts(), old = confirmation.open(facts, false);
  assert.equal(confirmation.open(facts, true), null, "one prompt at a time");
  assert.equal(confirmation.cancel(old), true); assert.equal(confirmation.cancel(old), false);
  const fresh = confirmation.open(facts, true); assert.ok(fresh.id > old.id);
  assert.equal(confirmation.cancel(old), false); assert.equal(confirmation.pending, fresh);
  const command = { type: "chat", message: "@ADDSTORAGE" };
  assert.equal(confirmation.claim(old, facts, command), false); assert.equal(confirmation.claim({ ...fresh }, facts, command), false);
  for (const bad of [{ ...command, message: "@addstorage" }, { ...command, cost: 1_000_000 }, { ...command, type: "command" }])
    assert.equal(confirmation.claim(fresh, facts, bad), false);
  assert.equal(confirmation.claim(fresh, facts, command), true); assert.equal(confirmation.claim(fresh, facts, command), false);
  const last = confirmation.open(facts, false); assert.ok(last); confirmation.retire();
  assert.equal(confirmation.allows(last, facts), false); assert.equal(confirmation.pending, null);
});

check("Guild rank save emits actual rename type3 then eight option type5 commands with exact ordinary fields", () => {
  const commands = socialParityActions.guildRankSaveCommands(7, "  Officer  ", ["CanRecruit", "CanStoreItem", "CanActivateBuff"]);
  assert.deepEqual(commands, [
    {type: "editGuildMember", changeType: 3, rankIndex: 7, name: "", rankName: "Officer"},
    {type: "editGuildMember", changeType: 5, rankIndex: 7, name: "false", rankName: "0"},
    {type: "editGuildMember", changeType: 5, rankIndex: 7, name: "true", rankName: "1"},
    {type: "editGuildMember", changeType: 5, rankIndex: 7, name: "false", rankName: "2"},
    {type: "editGuildMember", changeType: 5, rankIndex: 7, name: "true", rankName: "3"},
    {type: "editGuildMember", changeType: 5, rankIndex: 7, name: "false", rankName: "4"},
    {type: "editGuildMember", changeType: 5, rankIndex: 7, name: "false", rankName: "5"},
    {type: "editGuildMember", changeType: 5, rankIndex: 7, name: "false", rankName: "6"},
    {type: "editGuildMember", changeType: 5, rankIndex: 7, name: "true", rankName: "7"},
  ]);
  assert.equal(commands.some(command => command.changeType === 2 || command.changeType === 4), false,
    "saving a rank cannot assign a member or create a rank");
  assert.deepEqual(socialParityActions.GUILD_RANK_PERMISSION_KEYS,
    ["CanChangeRank", "CanRecruit", "CanKick", "CanStoreItem", "CanRetrieveItem", "CanAlterAlliance", "CanChangeNotice", "CanActivateBuff"]);
});

check("Guild rank save validates exact case-sensitive permissions and nonnegative i32 rank identity before emitting any command", () => {
  for (const rankIndex of [-1, 2147483648, NaN, Infinity, 1.5, "7", null])
    assert.equal(socialParityActions.guildRankSaveCommands(rankIndex, "Officer", []), null);
  for (const name of ["", "   ", "x\0y", "x".repeat(21), "x".repeat(257), null, 7])
    assert.equal(socialParityActions.guildRankSaveCommands(7, name, []), null);
  for (const permissions of [null, "CanRecruit", ["canRecruit"], ["CanRecruit", "CanRecruit"],
    ["CanUnknown"], [7], [...socialParityActions.GUILD_RANK_PERMISSION_KEYS, "extra"]])
    assert.equal(socialParityActions.guildRankSaveCommands(7, "Officer", permissions), null);
  for (const rankIndex of [0, 2147483647]) {
    const commands = socialParityActions.guildRankSaveCommands(rankIndex, "x".repeat(20), []);
    assert.equal(commands.length, 9); assert.equal(commands[0].rankIndex, rankIndex);
    assert.deepEqual(commands.slice(1).map(command => command.name), Array(8).fill("false"));
  }
  const all = socialParityActions.guildRankSaveCommands(1, "All", socialParityActions.GUILD_RANK_PERMISSION_KEYS);
  assert.deepEqual(all.slice(1).map(command => command.name), Array(8).fill("true"));
});

const rankMember = (name = "Pat", id = 0, online = false) => ({name, id, online,
  lastLoginBinaryDatetime: "638000000000000000", hasVoted: false});
const guildRanksPacket = () => [{index: 2, name: "Officer", options: 24, members: [rankMember()]},
  {index: 7, name: "Member", options: 255, members: [rankMember("Sam", 2147483647, true)]}];

check("Guild rank projection consumes actual camelCase GuildRank and GuildMember identity with all eight wire options", () => {
  const raw = guildRanksPacket(), before = JSON.stringify(raw), projected = socialParityActions.projectGuildRanks(raw);
  assert.deepEqual(projected, {
    ranks: [{index: 2, name: "Officer", permissions: ["CanStoreItem", "CanRetrieveItem"]},
      {index: 7, name: "Member", permissions: [...socialParityActions.GUILD_RANK_PERMISSION_KEYS]}],
    members: [{name: "Pat", id: 0, online: false, rank: "Officer", rankIndex: 2},
      {name: "Sam", id: 2147483647, online: true, rank: "Member", rankIndex: 7}],
  });
  assert.equal(JSON.stringify(raw), before, "received protocol rows remain unchanged");
  assert.deepEqual(socialParityActions.projectGuildRanks([]), {ranks: [], members: []});
  for (const [bit, permission] of socialParityActions.GUILD_RANK_PERMISSION_KEYS.entries()) {
    const projected = socialParityActions.projectGuildRanks([{index: bit, name: "Rank", options: 2 ** bit, members: []}]);
    assert.deepEqual(projected.ranks[0].permissions, [permission]);
  }
});

check("Guild rank projection rejects malformed u8 options i32 identities missing rosters and ambiguous rank or member identities", () => {
  for (const input of MALFORMED_INPUTS.filter(value => !Array.isArray(value)))
    assert.equal(socialParityActions.projectGuildRanks(input), null);
  for (const patch of [{index: -1}, {index: 2147483648}, {index: 1.5}, {index: "2"}, {name: ""},
    {name: "x\0y"}, {name: "x".repeat(257)}, {options: -1}, {options: 256}, {options: 1.5},
    {options: "24"}, {options: undefined}, {members: null}, {members: undefined}]) {
    const raw = guildRanksPacket(); Object.assign(raw[0], patch);
    assert.equal(socialParityActions.projectGuildRanks(raw), null, JSON.stringify(patch));
  }
  for (const mutate of [raw => { raw[1].index = raw[0].index; },
    raw => { raw[1].members[0].id = raw[0].members[0].id; },
    raw => { raw[1].members[0].name = "pAT"; },
    raw => { raw[0].members[0].id = -1; }, raw => { raw[0].members[0].id = 2147483648; },
    raw => { raw[0].members[0].id = "0"; }, raw => { raw[0].members[0].name = " "; },
    raw => { raw[0].members[0].online = 1; }, raw => { raw[0].members[0].online = undefined; },
    raw => { raw[0].members = [null]; }, raw => { raw[0].members[0] = {name: "Pat", characterIndex: 0, online: true}; }]) {
    const raw = guildRanksPacket(); mutate(raw); assert.equal(socialParityActions.projectGuildRanks(raw), null);
  }
});

check("Guild rank projection enforces complete bounded rank and member arrays without compaction", () => {
  const ranks = Array.from({length: 256}, (_, index) => ({index, name: "Rank " + index, options: 0, members: []}));
  assert.equal(socialParityActions.projectGuildRanks(ranks).ranks.length, 256);
  assert.equal(socialParityActions.projectGuildRanks([...ranks, {index: 256, name: "Extra", options: 0, members: []}]), null);
  const members = Array.from({length: 8192}, (_, id) => rankMember("Member " + id, id));
  const raw = [{index: 0, name: "Rank", options: 0, members}];
  assert.equal(socialParityActions.projectGuildRanks(raw).members.length, 8192);
  assert.equal(socialParityActions.projectGuildRanks([...raw, {index: 1, name: "Other", options: 0,
    members: [rankMember("Extra", 8192)]}]), null);
  const hole = [{index: 0, name: "Rank", options: 0, members: Array(1)}];
  assert.equal(socialParityActions.projectGuildRanks(hole), null, "a missing member is not an empty roster");
});

check("Friend character resolution authorizes only actual explicit indexes including zero and never substitutes display order", () => {
  const entries = [{name: "Legacy"}, {name: "Pat", index: 42}, {name: "Zero", index: 0},
    {name: "Maximum", index: 2147483647}];
  assert.equal(socialParityActions.resolveFriendCharacterIndex(entries, "pAT"), 42);
  assert.equal(socialParityActions.resolveFriendCharacterIndex(entries, "Zero"), 0);
  assert.equal(socialParityActions.resolveFriendCharacterIndex(entries, "Maximum"), 2147483647);
  assert.equal(socialParityActions.resolveFriendCharacterIndex(entries, "Legacy"), null);
  assert.equal(socialParityActions.resolveFriendCharacterIndex(entries, "Missing"), null);
  assert.equal(socialParityActions.resolveFriendCharacterIndex([{name: "Pat", characterIndex: 42}], "Pat"), null,
    "the actual friend packet field is index");
  assert.equal(socialParityActions.resolveFriendCharacterIndex([{name: "Älpha", index: 7}], "älpha"), null,
    "ASCII protocol folding does not invent a locale-dependent name match");
});

check("Friend character resolution rejects invalid i32 values duplicate names and duplicate indexes before mutation authority", () => {
  for (const index of [-1, 2147483648, 1.5, NaN, Infinity, "42", null, undefined])
    assert.equal(socialParityActions.resolveFriendCharacterIndex([{name: "Pat", index}], "Pat"), null);
  for (const name of ["", "   ", "x\0y", "x".repeat(257), null, 7])
    assert.equal(socialParityActions.resolveFriendCharacterIndex([{name: "Pat", index: 42}], name), null);
  assert.equal(socialParityActions.resolveFriendCharacterIndex([{name: "Pat", index: 42}, {name: "pAT", index: 43}], "Pat"), null);
  assert.equal(socialParityActions.resolveFriendCharacterIndex([{name: "Pat", index: 42}, {name: "Sam", index: 42}], "Pat"), null);
  const oversized = Array.from({length: 8193}, (_, index) => ({name: "Friend " + index, index}));
  assert.equal(socialParityActions.resolveFriendCharacterIndex(oversized, "Friend 0"), null);
});

check("Friends actual adapter index feeds the production resolver while canonical received identity defeats enrichment", () => {
  const raw = {friendInfos: [{name: "Pat", index: 42, online: true, memo: "memo"}, {name: "Legacy", online: false}],
    friends: ["Display only"]};
  const projected = adaptFriends(raw, {enrich: () => ({name: "Changed", index: 7})});
  assert.equal(socialParityActions.resolveFriendCharacterIndex(projected.friends, "Pat"), 42);
  assert.deepEqual(projected.friends[0], {name: "Pat", index: 42, online: true, memo: "memo"});
  assert.equal(socialParityActions.resolveFriendCharacterIndex(adaptFriends(raw).friends, "Legacy"), null);
});

check("Ranking query validates class filter offset and strict boolean before changing desired or reserving authority", () => {
  const queries = new socialParityActions.RankingQueries();
  assert.deepEqual(queries.desired, {rankType: 0, rankIndex: 0, onlineOnly: false});
  for (const query of [{rankType: -1, rankIndex: 0, onlineOnly: false}, {rankType: 6, rankIndex: 0, onlineOnly: false},
    {rankType: 1.5, rankIndex: 0, onlineOnly: false}, {rankType: 1, rankIndex: -1, onlineOnly: false},
    {rankType: 1, rankIndex: 2147483648, onlineOnly: false}, {rankType: 1, rankIndex: 1.5, onlineOnly: false},
    {rankType: 1, rankIndex: "20", onlineOnly: false}, {rankType: 1, rankIndex: 0, onlineOnly: 1}]) {
    assert.equal(queries.request(query, socialOwner), null); assert.equal(queries.pending, null);
    assert.deepEqual(queries.desired, {rankType: 0, rankIndex: 0, onlineOnly: false});
  }
  assert.equal(queries.request({rankType: 5, rankIndex: 2147483647, onlineOnly: true}, null), null);
  const query = {rankType: 5, rankIndex: 2147483647, onlineOnly: true}, proof = queries.request(query, socialOwner);
  assert.ok(proof); query.rankIndex = 0; assert.equal(proof.query.rankIndex, 2147483647, "query is captured before UI state can mutate it");
});

check("Ranking one outstanding query queues a different page or online filter without relabeling its old response", () => {
  const queries = new socialParityActions.RankingQueries(), first = {rankType: 3, rankIndex: 0, onlineOnly: false};
  const proof = queries.request(first, socialOwner), wire = {type: "getRanking", ...first};
  assert.equal(queries.claim(proof, socialOwner, wire), true);
  const desired = {rankType: 3, rankIndex: 20, onlineOnly: true};
  assert.equal(queries.request(desired, socialOwner), null); assert.strictEqual(queries.pending, proof);
  assert.deepEqual(queries.desired, desired);
  assert.equal(queries.claim(proof, socialOwner, {type: "getRanking", ...desired}), false);
  const received = queries.receive(3, socialOwner); assert.deepEqual(received, first);
  assert.equal(queries.pending, null); assert.equal(queries.wantsAnother(received), true);
  assert.equal(received.onlineOnly, false); assert.equal(received.rankIndex, 0, "first response cannot borrow queued filter/offset");
  const next = queries.request(queries.desired, socialOwner); assert.ok(next); assert.notStrictEqual(next, proof);
  assert.equal(queries.claim(next, socialOwner, {type: "getRanking", ...desired}), true);
  assert.deepEqual(queries.receive(3, socialOwner), desired); assert.equal(queries.wantsAnother(desired), false);
});

check("Ranking claim checks exact current scene map socket session and immutable four-field query wire", () => {
  const queries = new socialParityActions.RankingQueries(), query = {rankType: 2, rankIndex: 20, onlineOnly: true};
  const proof = queries.request(query, socialOwner), wire = {type: "getRanking", ...query};
  for (const field of Object.keys(socialOwner)) {
    const changed = {...socialOwner, [field]: field === "socket" ? {} : field === "mapFileName" ? "1" : socialOwner[field] + 1};
    assert.equal(queries.claim(proof, changed, wire), false, field);
  }
  for (const command of [{...wire, type: "ranking"}, {...wire, rankType: 3}, {...wire, rankIndex: 40},
    {...wire, onlineOnly: false}, {...wire, onlineOnly: "true"}, {...wire, requestId: 1},
    {type: "getRanking", rankType: 2, rankIndex: 20}]) assert.equal(queries.claim(proof, socialOwner, command), false);
  assert.equal(queries.claim({...proof}, socialOwner, wire), false);
  assert.equal(queries.claim(proof, socialOwner, wire), true); assert.equal(queries.claim(proof, socialOwner, wire), false);
});

check("Ranking receive rejects foreign physical owners and class types but an entered same-session scene change can settle", () => {
  const queries = new socialParityActions.RankingQueries(), query = {rankType: 4, rankIndex: 40, onlineOnly: false};
  const proof = queries.request(query, socialOwner); assert.equal(queries.claim(proof, socialOwner, {type: "getRanking", ...query}), true);
  for (const field of ["socket", "connectionGeneration", "sessionGeneration", "playerObjectId"]) {
    const changed = {...socialOwner, [field]: field === "socket" ? {} : socialOwner[field] + 1};
    assert.equal(queries.receive(4, changed), null, field); assert.strictEqual(queries.pending, proof);
  }
  for (const type of [3, "4", null, undefined]) assert.equal(queries.receive(type, socialOwner), null);
  const nextScene = {...socialOwner, sceneRevision: 8, mapFileName: "1"};
  assert.deepEqual(queries.receive(4, nextScene), query, "receive uses actual physical session, independently of current UI scene");
  assert.equal(queries.pending, null); assert.equal(queries.receive(4, nextScene), null);
  const unsent = queries.request(query, socialOwner);
  assert.equal(queries.claim(unsent, nextScene, {type: "getRanking", ...query}), false, "the receive exception cannot authorize a new send");
});

check("Ranking definitely-unsent cancellation cannot retire a successor or accept an unentered response", () => {
  const queries = new socialParityActions.RankingQueries(), query = {rankType: 1, rankIndex: 0, onlineOnly: false};
  const old = queries.request(query, socialOwner); assert.equal(queries.receive(1, socialOwner), null);
  queries.cancelDefinitelyUnsent(old); assert.equal(queries.pending, null);
  const fresh = queries.request({...query, rankIndex: 20}, socialOwner); assert.ok(fresh);
  queries.cancelDefinitelyUnsent(old); assert.strictEqual(queries.pending, fresh);
  assert.equal(queries.claim(old, socialOwner, {type: "getRanking", ...query}), false);
  assert.equal(queries.claim(fresh, socialOwner, {type: "getRanking", ...fresh.query}), true);
  queries.cancelDefinitelyUnsent(fresh); assert.strictEqual(queries.pending, fresh, "an entered query is not proven unsent");
});

check("Ranking entered unknown transport cannot cancel replay or automatically promote a queued desire", () => {
  const queries = new socialParityActions.RankingQueries(), query = {rankType: 2, rankIndex: 0, onlineOnly: false};
  const proof = queries.request(query, socialOwner);
  assert.throws(() => { assert.equal(queries.claim(proof, socialOwner, {type: "getRanking", ...query}), true); throw Error("unknown after socket entry"); }, /unknown after socket entry/);
  queries.cancelDefinitelyUnsent(proof); assert.strictEqual(queries.pending, proof);
  assert.equal(queries.request({...query, onlineOnly: true}, socialOwner), null);
  assert.strictEqual(queries.pending, proof); assert.equal(queries.claim(proof, socialOwner, {type: "getRanking", ...query}), false);
  assert.equal(queries.receive(3, socialOwner), null); assert.strictEqual(queries.pending, proof);
  const received = queries.receive(2, socialOwner); assert.deepEqual(received, query); assert.equal(queries.wantsAnother(received), true);
  assert.equal(queries.pending, null, "only an explicit host request can reserve the queued next query");
});

check("Ranking actual connection retirement clears old pending without allowing its same-type response into a new session", () => {
  const queries = new socialParityActions.RankingQueries(), query = {rankType: 1, rankIndex: 20, onlineOnly: true};
  const old = queries.request(query, socialOwner); assert.equal(queries.claim(old, socialOwner, {type: "getRanking", ...query}), true);
  queries.retire(); assert.equal(queries.pending, null); assert.deepEqual(queries.desired, {rankType: 0, rankIndex: 0, onlineOnly: false});
  const nextOwner = {...socialOwner, connectionGeneration: 4, sessionGeneration: 6, socket: {}};
  const next = queries.request(query, nextOwner); assert.equal(queries.claim(next, nextOwner, {type: "getRanking", ...query}), true);
  assert.equal(queries.receive(1, socialOwner), null); assert.strictEqual(queries.pending, next);
  queries.cancelDefinitelyUnsent(old); assert.strictEqual(queries.pending, next);
  assert.deepEqual(queries.receive(1, nextOwner), query);
});

check("Friend packet normalization preserves actual safe index zero and i32 maximum with memo blocked and online fields", () => {
  const rows = extendedPackets.normalizeFriendList([{name: "Zero", index: 0, memo: "memo", blocked: true, online: false},
    {name: "Maximum", index: 2147483647, online: true}, "Name array only", null]);
  assert.deepEqual(rows, [{name: "Zero", index: 0, memo: "memo", blocked: true, online: false},
    {name: "Maximum", index: 2147483647, memo: "", blocked: false, online: true}]);
  assert.equal(socialParityActions.resolveFriendCharacterIndex(rows, "Zero"), 0);
});

check("Friend packet normalization never creates zero identity for missing malformed or unsafe actual index", () => {
  for (const index of [undefined, null, -1, 2147483648, 1.5, NaN, Infinity, "42"]) {
    const row = extendedPackets.normalizeFriendList([{name: "Pat", index, memo: "kept", online: true}])[0];
    assert.ok(row); assert.equal(Object.hasOwn(row, "index"), false); assert.equal(row.memo, "kept");
    assert.equal(socialParityActions.resolveFriendCharacterIndex([row], "Pat"), null);
  }
  assert.deepEqual(extendedPackets.normalizeFriendList(["Pat", "Sam"]), []);
  assert.deepEqual(extendedPackets.normalizeFriendList(null), []);
});

check("Friend independent legal target survives another target ambiguity while the selected name or ID remains denied", () => {
  const rows = [{name: "Pat", index: 42}, {name: "Other", index: 7}, {name: "oTHER", index: 8},
    {name: "Shared ID A", index: 9}, {name: "Shared ID B", index: 9}];
  assert.equal(socialParityActions.resolveFriendCharacterIndex(rows, "Pat"), 42);
  assert.equal(socialParityActions.resolveFriendCharacterIndex(rows, "Other"), null);
  assert.equal(socialParityActions.resolveFriendCharacterIndex(rows, "Shared ID A"), null);
});

// Hero actions execute the actual production projector/planner/ledger. Sparse world
// rows carry real snake-case UserItem + ItemInfo, never inferred names or display IDs.
function heroItemFixture(uid, itemIndex = 10, count = 1) {
  return { unique_id: uid, item_index: itemIndex, count, current_dura: 10, max_dura: 10,
    soul_bound_id: 0, identified: true, cursed: false, slots: [], gem_count: 0, added_stats: [],
    awake_type: 0, awake_values: [], refined_value: 0, refine_added: 0, refine_success_chance: 0,
    wedding_ring: 0, expire_info: null, rental_information: null, is_shop_item: false,
    sealed_info: null, gm_made: false };
}
function heroInfoFixture(itemIndex = 10, extra = {}) {
  return { item_index: itemIndex, name: `Item ${itemIndex}`, item_type: 13, grade: 0,
    required_type: 0, required_class: 31, required_gender: 3, item_set: 0, shape: 0, weight: 1,
    light: 0, required_amount: 1, image: 1, durability: 10, stack_size: 20, price: 1,
    start_item: false, effect: 0, need_identify: false, show_group_pickup: false,
    class_based: false, level_based: false, can_mine: false, global_drop_notify: false,
    bind: 0, unique: 0, random_stats_id: 0, can_fast_run: false, can_awakening: false,
    slots: 0, stats: [], tooltip: null, ...extra };
}
function heroWorldRowFixture(item, slot, container = "bag1", info = heroInfoFixture(item.item_index)) {
  return { slot, container, uniqueId: item.unique_id, quantity: item.count, name: info.name, icon: info.image,
    tooltipSource: { info, userItem: item } };
}
function heroFixture() {
  const owner = { connectionGeneration: 1, sessionGeneration: 7, playerObjectId: 40,
    socket: {}, sceneRevision: 2, mapFileName: "0" };
  const potion = heroItemFixture(501), bag = heroItemFixture(502, 10, 3), player = heroItemFixture(601, 11);
  const magic = { name: "Fireball", spell: "FireBall", base_cost: 1, level_cost: 1, icon: 1,
    level1: 1, level2: 1, level3: 1, need1: 1, need2: 1, need3: 1, level: 1, key: 17,
    experience: 0, delay: 100, range: 5, cast_time: -100 };
  const info = { object_id: 70, name: "Hero", class: "Warrior", gender: "Male", level: 20,
    hair: 0, hp: 100, mp: 50, experience: 1, max_experience: 100,
    inventory: [potion, null, bag, ...Array(7).fill(null)], equipment: Array(14).fill(null),
    magics: [magic], auto_pot: true, auto_hp_percent: 40, auto_mp_percent: 30,
    hp_item_index: 10, mp_item_index: 0 };
  const world = { playerObjectId: 40, mapFileName: "0", inventoryCapacity: 46, maxBagSlots: 40,
    inventoryItems: [heroWorldRowFixture(player, 3)], beltItems: [],
    heroInventoryCapacity: 10, heroInventoryItems: [heroWorldRowFixture(potion, 0), heroWorldRowFixture(bag, 2)],
    heroEquipmentItems: [], heroStats: [{ stat: 1, value: 2 }], heroVitals: { hp: 100, mp: 50 },
    heroWeights: { bag: 1, wear: 0, hand: 0 }, stage5Systems: { hero: { name: "Hero", class: "Warrior", gender: "Male", level: 20,
      spawned: true, autoPot: true, autoHpPercent: 40, autoMpPercent: 30, hpItemIndex: 10, mpItemIndex: 0 },
      heroLearnedMagics: [{ spell: "FireBall", level: 1, key: 17, experience: 0 }] } };
  const authority = new heroUi.HeroPlayerAuthority();
  assert.equal(authority.receiveInformation({ info }, owner), true);
  assert.equal(authority.receiveSnapshot(world, owner), true);
  const model = authority.read(owner); assert.ok(model);
  return { owner, info, world, authority, model };
}
check("Hero exact snake carrier + camel snapshot preserves physical belt and bag holes", () => {
  const f = heroFixture();
  assert.equal(f.model.experience, 1); assert.equal(f.model.maxExperience, 100);
  assert.equal(f.model.inventory.length, 10); assert.equal(f.model.equipment.length, 14);
  assert.equal(f.model.inventory[0].uniqueId, 501); assert.equal(f.model.inventory[1], null);
  assert.equal(f.model.inventory[2].slot, 2); assert.equal(f.model.personalInventory[3].uniqueId, 601);
  assert.equal(f.model.inventory[2].userItem.item_index, 10);
  assert.equal(f.model.magics[0].raw.cast_time, -100);
  assert.equal(Object.isFrozen(f.model.inventory[2].tooltipSource.info), true);
  assert.equal(Object.isFrozen(f.owner.socket), false);
  f.world.heroInventoryItems[0].tooltipSource.realInfo = heroInfoFixture(777, { shape: 4 });
  f.authority.receiveSnapshot(f.world, f.owner); const resolved = f.authority.read(f.owner);
  assert.equal(resolved.inventory[0].itemIndex, 10); assert.equal(resolved.inventory[0].info.item_index, 777);
  assert.equal(heroUi.planHeroAction(resolved, { kind: "use", slot: 0 }).confirmationRequired, true);
});
check("Hero strict partial information and missing full ItemInfo fail closed", () => {
  const f = heroFixture();
  delete f.info.inventory; assert.equal(f.authority.receiveInformation({ info: f.info }, f.owner), false);
  assert.equal(f.authority.read(f.owner), null);
  const g = heroFixture(); delete g.world.heroInventoryItems[0].tooltipSource.info.stats;
  g.authority.receiveSnapshot(g.world, g.owner); assert.equal(g.authority.read(g.owner), null);
});
check("Hero capacities only 10/18/26/34/42 and equipment exactly14", () => {
  for (const capacity of [10, 18, 26, 34, 42]) {
    const f = heroFixture(); f.info.inventory.length = capacity; f.info.inventory.fill(null, 10);
    f.world.heroInventoryCapacity = capacity; assert.equal(f.authority.receiveInformation(f.info, f.owner), true);
    f.authority.receiveSnapshot(f.world, f.owner); assert.equal(f.authority.read(f.owner).inventoryCapacity, capacity);
  }
  for (const capacity of [0, 9, 11, 40, 43]) {
    const f = heroFixture(); f.world.heroInventoryCapacity = capacity;
    f.authority.receiveSnapshot(f.world, f.owner); assert.equal(f.authority.read(f.owner), null);
  }
  const f = heroFixture(); f.info.equipment.pop(); assert.equal(f.authority.receiveInformation(f.info, f.owner), false);
});
check("Hero duplicate unsafe or fabricated UID and nested duplicate custody rejected", () => {
  for (const id of [0, -1, Number.MAX_SAFE_INTEGER + 1]) {
    const f = heroFixture(); f.world.heroInventoryItems[0].uniqueId = id;
    f.authority.receiveSnapshot(f.world, f.owner); assert.equal(f.authority.read(f.owner), null);
  }
  const f = heroFixture(); f.world.heroInventoryItems.push(f.world.heroInventoryItems[0]);
  f.authority.receiveSnapshot(f.world, f.owner); assert.equal(f.authority.read(f.owner), null);
  const g = heroFixture(); g.info.inventory[0].slots.push(heroItemFixture(502));
  assert.equal(g.authority.receiveInformation(g.info, g.owner), false);
});
check("Hero Bag2 uses real physical +40 bag index and never +6", () => {
  const f = heroFixture(); f.world.inventoryCapacity = 58; f.world.maxBagSlots = 52;
  f.world.inventoryItems = [heroWorldRowFixture(heroItemFixture(601, 11), 1, "bag2")];
  f.authority.receiveSnapshot(f.world, f.owner); const m = f.authority.read(f.owner);
  assert.equal(m.personalInventory[41].uniqueId, 601); assert.equal(m.personalInventory[47], null);
  assert.deepEqual(heroUi.planHeroAction(m, { kind: "transfer", from: 41, to: 1 }).wire,
    { type: "transferHeroItem", from: 41, to: 1 });
});
check("Hero latest information enriches real UID only and same-name actor replacement retires old source", () => {
  const f = heroFixture(); const old = heroUi.captureHeroAction(f.model, { kind: "move", from: 2, to: 3 });
  f.info.inventory[2].count = 2; f.authority.receiveInformation(f.info, f.owner);
  assert.equal(f.authority.read(f.owner).inventory[2].count, 2);
  assert.equal(heroUi.heroActionCurrent(old, f.authority.read(f.owner)), false);
  f.info.object_id = 71; f.authority.receiveInformation(f.info, f.owner); assert.equal(f.authority.read(f.owner), null);
  // The fixture shares this raw UserItem with HeroInformation; its snapshot quantity must agree.
  f.authority.receiveSnapshot(f.world, f.owner); assert.equal(f.authority.read(f.owner), null);
  f.world.heroInventoryItems[1].quantity = f.info.inventory[2].count;
  f.authority.receiveSnapshot(f.world, f.owner); const next = f.authority.read(f.owner);
  assert.equal(next.actor.objectId, 71); assert.ok(next.actor.generation > f.model.actor.generation);
  assert.equal(next.inventory[2].count, 2);
  assert.equal(heroUi.heroActionCurrent(old, next), false);
});
check("Hero source isolation strict scene while receive rejects foreign physical session", () => {
  const f = heroFixture(), scene = { ...f.owner, sceneRevision: 3 };
  assert.equal(f.authority.read(scene), null);
  assert.equal(f.authority.receiveInformation(f.info, { ...f.owner, socket: {} }), false);
  assert.equal(f.authority.receiveSnapshot(f.world, { ...f.owner, sessionGeneration: 8 }), false);
  assert.equal(f.authority.retireSession(scene), false); assert.ok(f.authority.read(f.owner));
});
check("Hero Move/Equip/Remove/Merge use exact grids IDs and alive riding requirements", () => {
  const f = heroFixture();
  assert.deepEqual(heroUi.planHeroAction(f.model, { kind: "move", from: 2, to: 1 }).wire, { type: "moveItem", grid: "HeroInventory", from: 2, to: 1 });
  assert.deepEqual(heroUi.planHeroAction(f.model, { kind: "merge", from: { grid: "HeroInventory", slot: 2 }, to: { grid: "HeroInventory", slot: 0 } }).wire,
    { type: "mergeItem", gridFrom: "HeroInventory", gridTo: "HeroInventory", idFrom: 502, idTo: 501 });
  f.world.heroInventoryItems[1].tooltipSource.info = heroInfoFixture(10, { item_type: 1 });
  f.authority.receiveSnapshot(f.world, f.owner); const weapon = f.authority.read(f.owner);
  assert.deepEqual(heroUi.planHeroAction(weapon, { kind: "use", slot: 2 }).wire, { type: "equipItem", grid: "HeroInventory", uniqueId: 502, to: 0 });
  assert.equal(heroUi.planHeroAction(weapon, { kind: "equip", from: 2, to: 5 }), null);
  f.authority.receiveActor("MountUpdate", { objectId: 70, ridingMount: true }, f.owner);
  assert.equal(heroUi.planHeroAction(f.authority.read(f.owner), { kind: "use", slot: 2 }), null);
  const worn = heroItemFixture(503, 12); f.world.heroEquipmentItems = [heroWorldRowFixture(worn, 3, "bag1", heroInfoFixture(12, { item_type: 12 }))];
  f.authority.receiveSnapshot(f.world, f.owner); const gear = f.authority.read(f.owner);
  assert.deepEqual(heroUi.planHeroAction(gear, { kind: "remove", from: 3 }).wire, { type: "removeItem", grid: "HeroInventory", uniqueId: 503, to: 3 });
});
check("Hero Transfer/TakeBack destination bounds and full personal source required", () => {
  const f = heroFixture();
  assert.deepEqual(heroUi.planHeroAction(f.model, { kind: "takeBack", from: 2, to: 5 }).wire, { type: "takeBackHeroItem", from: 2, to: 5 });
  assert.equal(heroUi.planHeroAction(f.model, { kind: "transfer", from: 3, to: 10 }), null);
  delete f.world.inventoryItems; f.authority.receiveSnapshot(f.world, f.owner); const m = f.authority.read(f.owner);
  assert.ok(m); assert.equal(m.personalInventory, null); assert.equal(heroUi.planHeroAction(m, { kind: "takeBack", from: 2, to: 5 }), null);
});
check("Hero shape4 confirmation cannot reserve before explicit source-bound confirmation", () => {
  const f = heroFixture(); f.world.heroInventoryItems[0].tooltipSource.info.shape = 4;
  f.authority.receiveSnapshot(f.world, f.owner); const m = f.authority.read(f.owner), ledger = new heroUi.HeroPlayerOperations();
  assert.equal(heroUi.planHeroAction(m, { kind: "use", slot: 0 }).confirmationRequired, true);
  assert.equal(ledger.reserve(m, { kind: "use", slot: 0 }), null);
  const dto = heroUi.captureHeroAction(m, { kind: "use", slot: 0, confirmed: true });
  f.world.heroInventoryItems[0].quantity = 2; f.world.heroInventoryItems[0].tooltipSource.userItem.count = 2;
  f.authority.receiveSnapshot(f.world, f.owner); assert.equal(ledger.reserve(f.authority.read(f.owner), dto), null);
  assert.ok(ledger.reserve(m, dto));
});
check("Hero supported Use excludes attachments and exposes no drop split combine cast or recall DTO", () => {
  const f = heroFixture();
  for (const type of [22, 23, 24, 25, 26, 28, 29, 30, 31, 32, 39]) {
    f.world.heroInventoryItems[0].tooltipSource.info.item_type = type;
    f.authority.receiveSnapshot(f.world, f.owner); assert.equal(heroUi.planHeroAction(f.authority.read(f.owner), { kind: "use", slot: 0 }), null);
  }
  for (const kind of ["drop", "split", "combine", "cast", "recall"]) assert.equal(heroUi.planHeroAction(f.model, { kind }), null);
});
check("Hero autopot stat12/13 and true itemIndex shape0/1 only", () => {
  const f = heroFixture();
  assert.deepEqual(heroUi.planHeroAction(f.model, { kind: "autoPotValue", stat: 12, value: 99 }).wire, { type: "setAutoPotValue", stat: 12, value: 99 });
  assert.equal(heroUi.planHeroAction(f.model, { kind: "autoPotValue", stat: 13, value: 100 }), null);
  assert.deepEqual(heroUi.planHeroAction(f.model, { kind: "autoPotItem", grid: "HeroHpItem", slot: 2 }).wire, { type: "setAutoPotItem", grid: "HeroHpItem", itemIndex: 10 });
  assert.deepEqual(heroUi.planHeroAction(f.model, { kind: "autoPotItem", grid: "HeroMpItem", slot: null }).wire, { type: "setAutoPotItem", grid: "HeroMpItem", itemIndex: 0 });
  f.world.heroInventoryItems[1].tooltipSource.info.shape = 4; f.authority.receiveSnapshot(f.world, f.owner);
  assert.equal(heroUi.planHeroAction(f.authority.read(f.owner), { kind: "autoPotItem", grid: "HeroHpItem", slot: 2 }), null);
});
check("Hero belt restock remains explicit and leases original live source UID", () => {
  const f = heroFixture(), candidate = heroUi.heroRestockCandidate(f.model, 0);
  assert.equal(candidate.from, 2); assert.equal(heroUi.heroRestockAction(candidate, f.model), null);
  f.world.heroInventoryItems.shift(); f.authority.receiveSnapshot(f.world, f.owner); const m = f.authority.read(f.owner);
  assert.deepEqual(heroUi.heroRestockAction(candidate, m).action, { kind: "move", from: 2, to: 0 });
  f.world.heroInventoryItems[0] = heroWorldRowFixture(heroItemFixture(504, 10, 3), 2);
  f.authority.receiveSnapshot(f.world, f.owner); assert.equal(heroUi.heroRestockAction(candidate, f.authority.read(f.owner)), null);
});

// Accepted belt use records an intent in the real operation ledger, not in a
// mounted window. These fixtures publish actual complete Hero carriers.
function publishHeroBeltConsumption(f, owner = f.owner) {
  f.info.inventory[0] = null;
  f.world.heroInventoryItems = f.world.heroInventoryItems.filter(item => item.slot !== 0);
  f.world.mapFileName = owner.mapFileName;
  assert.equal(f.authority.receiveInformation(f.info, owner), true);
  assert.equal(f.authority.receiveSnapshot(f.world, owner), true);
  const model = f.authority.read(owner); assert.ok(model); return model;
}
check("Hero belt successor is recorded only at accepted use claim and cannot originate from an unsent reservation", () => {
  const f = heroFixture(), ledger = new heroUi.HeroPlayerOperations();
  const unsent = ledger.reserve(f.model, {kind:"use", slot:0});
  assert.equal(ledger.restockCandidate, null); assert.equal(ledger.reserveRestock(f.model), null);
  assert.equal(ledger.cancelDefinitelyUnsent(unsent), true); assert.equal(ledger.restockCandidate, null);
  const proof = ledger.reserve(f.model, {kind:"use", slot:0});
  assert.equal(ledger.claim(proof, {...f.model, owner:{...f.owner, sceneRevision:99}}), false);
  assert.equal(ledger.restockCandidate, null);
  assert.equal(ledger.claim(proof, f.model), true);
  assert.equal(ledger.restockCandidate.uniqueId, 502); assert.equal(ledger.restockCandidate.from, 2);
  assert.equal(Object.isFrozen(ledger.restockCandidate), true);
  assert.equal(ledger.reserveRestock(f.model), null); assert.equal(ledger.claim(proof, f.model), false);
});
check("Hero unknown belt use waits for exact ACK and a later complete model then reserves one same-physical scene-rebased move", () => {
  const f = heroFixture(), ledger = new heroUi.HeroPlayerOperations(), proof = ledger.reserve(f.model, {kind:"use", slot:0});
  assert.equal(ledger.claim(proof, f.model), true); assert.equal(ledger.outcomeUnknown(proof), true);
  const scene = {...f.owner, sceneRevision:9, mapFileName:"1"};
  assert.equal(ledger.retireSession(scene), false);
  let model = publishHeroBeltConsumption(f, scene);
  assert.equal(ledger.observe(model), false); assert.equal(ledger.reserveRestock(model), null);
  assert.equal(ledger.cancelDefinitelyUnsent(proof), false);
  assert.equal(ledger.receipt("UseItem", {grid:"HeroInventory", uniqueId:501, success:true}, {...scene, socket:{}}, model.authoritySerial), false);
  assert.equal(ledger.receipt("UseItem", {grid:"HeroInventory", uniqueId:502, success:true}, scene, model.authoritySerial), false);
  assert.equal(ledger.receipt("UseItem", {grid:"HeroInventory", uniqueId:501, success:true}, scene, model.authoritySerial), true);
  assert.equal(ledger.observe(model), false); assert.equal(ledger.reserveRestock(model), null);
  assert.equal(f.authority.receiveInformation({...f.info, equipment:null}, scene), false);
  assert.equal(f.authority.read(scene), null); assert.ok(ledger.pending); assert.ok(ledger.restockCandidate);
  model = publishHeroBeltConsumption(f, scene);
  assert.equal(ledger.observe(model), true); assert.equal(ledger.pending, null);
  const refill = ledger.reserveRestock(model); assert.ok(refill); assert.ok(refill.id > proof.id);
  assert.deepEqual(refill.wire, {type:"moveItem",grid:"HeroInventory",from:2,to:0});
  assert.equal(refill.dto.owner.sceneRevision, 9); assert.equal(refill.dto.owner.mapFileName, "1");
  assert.equal(ledger.restockCandidate, null); assert.equal(ledger.reserveRestock(model), null);
  assert.equal(ledger.claim(refill, {...model, owner:f.owner}), false);
  assert.equal(ledger.claim(refill, model), true); assert.equal(ledger.outcomeUnknown(refill), true);
  assert.equal(ledger.cancelDefinitelyUnsent(refill), false); assert.equal(ledger.reserveRestock(model), null);
});
check("Hero failed exact belt-use receipt discards only its successor while retaining the authoritative item barrier", () => {
  const f = heroFixture(), ledger = new heroUi.HeroPlayerOperations(), proof = ledger.reserve(f.model, {kind:"use",slot:0});
  ledger.claim(proof, f.model);
  assert.equal(ledger.receipt("UseItem", {grid:"HeroInventory",uniqueId:501,success:false}, f.owner, f.model.authoritySerial), true);
  assert.equal(ledger.restockCandidate, null); assert.ok(ledger.pending);
  const model = publishHeroBeltConsumption(f); assert.equal(ledger.observe(model), true);
  assert.equal(ledger.reserveRestock(model), null);
});
check("Hero successor cannot consume changed stock UID itemIndex or a replaced actor even when its use remains unknown", () => {
  for (const change of ["uid", "itemIndex", "actor"]) {
    const f = heroFixture(), ledger = new heroUi.HeroPlayerOperations(), proof = ledger.reserve(f.model, {kind:"use",slot:0});
    ledger.claim(proof, f.model); ledger.outcomeUnknown(proof);
    let model = publishHeroBeltConsumption(f);
    if (change === "uid") {
      f.info.inventory[2] = heroItemFixture(504,10,3);
      f.world.heroInventoryItems[0] = heroWorldRowFixture(f.info.inventory[2],2);
    } else if (change === "itemIndex") {
      f.info.inventory[2] = heroItemFixture(502,11,3);
      f.world.heroInventoryItems[0] = heroWorldRowFixture(f.info.inventory[2],2);
    } else f.info.object_id = 71;
    assert.equal(f.authority.receiveInformation(f.info,f.owner),true);
    assert.equal(f.authority.receiveSnapshot(f.world,f.owner),true);
    model = f.authority.read(f.owner); assert.ok(model);
    assert.equal(ledger.observe(model),false); assert.equal(ledger.restockCandidate,null,change);
    assert.strictEqual(ledger.pending.proof,proof); assert.equal(ledger.reserveRestock(model),null);
  }
});
check("Hero complete post-use occupied belt and stack count greater than one do not create a refill", () => {
  const f = heroFixture(), ledger = new heroUi.HeroPlayerOperations(), proof = ledger.reserve(f.model,{kind:"use",slot:0});
  ledger.claim(proof,f.model); ledger.receipt("UseItem",{grid:"HeroInventory",uniqueId:501,success:true},f.owner,f.model.authoritySerial);
  f.authority.receiveSnapshot(f.world,f.owner); const unchanged = f.authority.read(f.owner);
  assert.equal(ledger.observe(unchanged),true); assert.equal(ledger.restockCandidate,null);
  assert.equal(ledger.reserveRestock(unchanged),null);
  const g = heroFixture(); g.info.inventory[0].count=2; g.world.heroInventoryItems[0].quantity=2;
  assert.equal(g.authority.receiveInformation(g.info,g.owner),true); assert.equal(g.authority.receiveSnapshot(g.world,g.owner),true);
  const model=g.authority.read(g.owner); assert.ok(model);
  const next=ledger.reserve(model,{kind:"use",slot:0}); assert.ok(next); assert.equal(ledger.claim(next,model),true);
  assert.equal(ledger.restockCandidate,null);
});
check("Hero ready successor survives same-connection UI scene loss but a real physical retirement clears it without replay", () => {
  const f = heroFixture(), ledger = new heroUi.HeroPlayerOperations(), proof = ledger.reserve(f.model,{kind:"use",slot:0});
  ledger.claim(proof,f.model); ledger.receipt("UseItem",{grid:"HeroInventory",uniqueId:501,success:true},f.owner,f.model.authoritySerial);
  const model=publishHeroBeltConsumption(f); assert.equal(ledger.observe(model),true);
  assert.equal(ledger.retireSession({...f.owner,sceneRevision:50,mapFileName:"1"}),false);
  assert.ok(ledger.restockCandidate); assert.equal(ledger.pending,null);
  assert.equal(ledger.retireSession({...f.owner,sessionGeneration:8}),true);
  assert.equal(ledger.restockCandidate,null); assert.equal(ledger.reserveRestock(model),null);
});

check("Hero last claim checks owner source wire and only enters once", () => {
  const f = heroFixture(), l = new heroUi.HeroPlayerOperations(), p = l.reserve(f.model, { kind: "move", from: 2, to: 3 });
  assert.ok(p); assert.equal(l.allows(p, f.model, { ...p.wire, to: 4 }), false);
  assert.equal(l.claim(p, { ...f.model, owner: { ...f.owner, sceneRevision: 3 } }), false);
  assert.equal(l.claim(p, f.model), true); assert.equal(l.claim(p, f.model), false);
  assert.equal(l.reserve(f.model, { kind: "move", from: 2, to: 4 }), null);
});
check("Hero unknown entered retains pending through same session scene status and foreign ACK", () => {
  const f = heroFixture(), l = new heroUi.HeroPlayerOperations(), p = l.reserve(f.model, { kind: "move", from: 2, to: 3 });
  l.claim(p, f.model); assert.equal(l.outcomeUnknown(p), true);
  assert.equal(l.retireSession({ ...f.owner, sceneRevision: 99, mapFileName: "1" }), false);
  assert.equal(l.receipt("MoveItem", { grid: "HeroInventory", from: 2, to: 3, success: true }, { ...f.owner, socket: {} }, 2), false);
  f.authority.receiveSnapshot(f.world, f.owner); assert.equal(l.observe(f.authority.read(f.owner)), false);
  assert.equal(l.claim(p, f.model), false); assert.equal(l.cancelDefinitelyUnsent(p), false);
  assert.equal(l.retireSession({ ...f.owner, sessionGeneration: 8 }), true); assert.equal(l.pending, null);
});
check("Hero exact ACK still waits for newer complete authoritative Hero barrier", () => {
  const f = heroFixture(), l = new heroUi.HeroPlayerOperations(), p = l.reserve(f.model, { kind: "move", from: 2, to: 3 });
  l.claim(p, f.model);
  assert.equal(l.receipt("MoveItem", { grid: "HeroInventory", from: 2, to: 4, success: true }, f.owner, 2), false);
  assert.equal(l.receipt("MoveItem", { grid: "HeroInventory", from: 2, to: 3, success: true }, f.owner, 2), true);
  assert.equal(l.observe(f.model), false); assert.equal(l.pending.state, "acknowledged");
  const partial = { ...f.info, equipment: null }; f.authority.receiveInformation(partial, f.owner);
  assert.equal(f.authority.read(f.owner), null); assert.ok(l.pending);
  f.info.inventory[3] = f.info.inventory[2]; f.info.inventory[2] = null;
  f.authority.receiveInformation(f.info, f.owner); assert.equal(l.observe(f.authority.read(f.owner)), true);
  assert.equal(l.pending, null);
});
check("Hero cross ACK requires new fullpersonal snapshot as well as Hero info", () => {
  const f = heroFixture(), l = new heroUi.HeroPlayerOperations(), p = l.reserve(f.model, { kind: "transfer", from: 3, to: 3 });
  l.claim(p, f.model); l.receipt("TransferHeroItem", { from: 3, to: 3, success: false }, f.owner, 2);
  f.authority.receiveInformation(f.info, f.owner); assert.equal(l.observe(f.authority.read(f.owner)), false);
  f.authority.receiveSnapshot(f.world, f.owner); assert.equal(l.observe(f.authority.read(f.owner)), true);
});
check("Hero definitely-unsent old proof cannot release a replacement reservation", () => {
  const f = heroFixture(), l = new heroUi.HeroPlayerOperations(), p = l.reserve(f.model, { kind: "move", from: 2, to: 3 });
  assert.equal(l.claim(p, f.model), true); assert.equal(l.cancelDefinitelyUnsent(p), false);
  assert.equal(l.pending.proof, p); assert.equal(l.outcomeUnknown(p), true); assert.equal(l.pending.proof, p);
  const nextOwner = { ...f.owner, socket: {}, connectionGeneration: f.owner.connectionGeneration + 1,
    sessionGeneration: f.owner.sessionGeneration + 1 };
  assert.equal(l.retireSession(nextOwner), true);
  const nextAuthority = new heroUi.HeroPlayerAuthority();
  assert.equal(nextAuthority.receiveInformation({ info: f.info }, nextOwner), true);
  assert.equal(nextAuthority.receiveSnapshot(f.world, nextOwner), true);
  const q = l.reserve(nextAuthority.read(nextOwner), { kind: "move", from: 2, to: 4 }); assert.ok(q.id > p.id);
  assert.equal(l.cancelDefinitelyUnsent(p), false); assert.equal(l.pending.proof, q);
});
check("Hero autopot echo requires exact numeric ABI and then a fresh config snapshot", () => {
  const f = heroFixture(), l = new heroUi.HeroPlayerOperations(), p = l.reserve(f.model, { kind: "autoPotItem", grid: "HeroHpItem", slot: 2 });
  l.claim(p, f.model);
  assert.equal(l.receipt("SetAutoPotItem", { grid: 24, item_index: 10 }, f.owner, 2), false);
  assert.equal(l.receipt("SetAutoPotItem", { grid: 23, item_index: 10 }, f.owner, 2), true);
  assert.equal(l.observe(f.model), false); f.authority.receiveSnapshot(f.world, f.owner);
  assert.equal(l.observe(f.authority.read(f.owner)), true);
});
check("Hero MagicKey real17..24/0 oldKey and monotonic memory request allocation", () => {
  const f = heroFixture(), a = new heroUi.HeroPlayerOperations(), b = new heroUi.HeroPlayerOperations();
  assert.equal(heroUi.planHeroAction(f.model, { kind: "magicKey", spell: "FireBall", key: 16 }), null);
  const p = a.reserve(f.model, { kind: "magicKey", spell: "FireBall", key: 24 });
  const q = b.reserve(f.model, { kind: "magicKey", spell: "FireBall", key: 0 });
  assert.equal(p.wire.oldKey, 17); assert.equal(p.wire.key, 24); assert.ok(q.wire.requestId > p.wire.requestId);
  f.world.stage5Systems.heroLearnedMagics[0].key = 0; f.authority.receiveSnapshot(f.world, f.owner);
  assert.equal(heroUi.planHeroAction(f.authority.read(f.owner), { kind: "magicKey", spell: "FireBall", key: 0 }), null);
});
check("Hero MagicKey ACK request/spell/key/oldKey actor serial and same actual keys all required", () => {
  const f = heroFixture(), l = new heroUi.HeroPlayerOperations(), p = l.reserve(f.model, { kind: "magicKey", spell: "FireBall", key: 18 });
  l.claim(p, f.model); l.outcomeUnknown(p);
  const ack = { requestId: p.wire.requestId, spell: "FireBall", key: 18, oldKey: 17, accepted: true };
  for (const bad of [{ requestId: p.wire.requestId + 1 }, { spell: "Heal" }, { key: 19 }, { oldKey: 0 }]) {
    f.world.skillKeyAck = { ...ack, ...bad }; f.world.stage5Systems.heroLearnedMagics[0].key = 18;
    f.authority.receiveSnapshot(f.world, f.owner); assert.equal(l.observe(f.authority.read(f.owner)), false);
  }
  f.world.skillKeyAck = ack; f.world.stage5Systems.heroLearnedMagics[0].key = 17;
  f.authority.receiveSnapshot(f.world, f.owner); assert.equal(l.observe(f.authority.read(f.owner)), false);
  f.world.stage5Systems.heroLearnedMagics[0].key = 18; f.authority.receiveSnapshot(f.world, f.owner);
  assert.equal(l.observe(f.authority.read(f.owner)), true);
});
check("Hero rejected MagicKey needs matching old authoritative key and no automatic retry", () => {
  const f = heroFixture(), l = new heroUi.HeroPlayerOperations(), p = l.reserve(f.model, { kind: "magicKey", spell: "FireBall", key: 0 });
  l.claim(p, f.model); f.world.skillKeyAck = { requestId: p.id, spell: "FireBall", key: 0, oldKey: 17, accepted: false };
  f.authority.receiveSnapshot(f.world, f.owner); assert.equal(l.observe(f.authority.read(f.owner)), true);
  assert.equal(l.pending, null); assert.equal(l.claim(p, f.authority.read(f.owner)), false);
});

// Cash/Pet source and operation checks consume the actual production modules.
function cashPayloadFixture(overrides = {}) {
  return { stockLevel: 10, item: { item_index: 23, g_index: 31,
    info: { item_index: 23, name: "Actual sword", image: 77, item_type: 1, shape: 0, stack_size: 20 },
    gold_price: 100, credit_price: 10, count: 2, class: "All", category: "Weapons", stock: 10,
    i_stock: false, deal: false, top_item: false, date_binary_datetime: "638900000000000000",
    can_buy_credit: true, can_buy_gold: true, ...overrides } };
}
function cashFixture(options = {}) {
  const owner = { ...socialOwner, socket: {} }, payload = cashPayloadFixture(options.item);
  const catalog = cashShopUi.upsertCashGameShopInfo(cashShopUi.emptyCashGameShopCatalog(), payload);
  assert.ok(catalog);
  const source = cashShopUi.makeCashGameShopSource(catalog, { gold: 500, credit: 100, className: "Warrior", ...options.wallet },
    owner, 1, options.receiptEnabled ?? true);
  assert.ok(source);
  const confirmation = cashShopUi.makeCashGameShopConfirmation(source, 31, 2, "gold");
  return { owner, payload, catalog, source, confirmation };
}
function cashReceiptFixture(proof, overrides = {}) {
  const { requestId, gIndex, quantity, priceType } = proof.command;
  return { type: "gameShopReceipt", protocol: "nativeGameShopReceiptV1", requestId, gIndex, quantity, priceType,
    success: true, mailId: 701, ...overrides };
}
check("Cash catalog comes from exact GameShopInfo and keyed stock races preserve product metadata", () => {
  const empty = cashShopUi.emptyCashGameShopCatalog();
  const patched = cashShopUi.applyCashGameShopStock(empty, { gIndex: 31, stockLevel: 3, typed: true });
  assert.equal(patched.entries.length, 0);
  const catalog = cashShopUi.upsertCashGameShopInfo(patched, cashPayloadFixture());
  assert.equal(catalog.entries.length, 1); assert.equal(catalog.entries[0].stock_level, 3);
  assert.equal(catalog.entries[0].info.image, 77); assert.equal(catalog.pendingStock.length, 0);
  assert.equal(cashShopUi.applyCashGameShopStock(catalog, { gIndex: 31, stockLevel: 2, typed: false }), null);
  assert.equal(cashShopUi.applyCashGameShopStock(catalog, { gIndex: 31, stockLevel: 2, alias: true }), null);
});
check("Cash metadata rejects unsafe date numbers mismatched indexes incomplete prices and unsupported flags", () => {
  const date = cashPayloadFixture(); date.item.date_binary_datetime = 638900000000000000;
  assert.equal(cashShopUi.readCashGameShopInfo(date), null);
  for (const bad of [p => delete p.item.can_buy_gold, p => p.item.info.item_index = 24,
    p => p.item.count = 0, p => p.item.gold_price = 4294967296, p => p.stockLevel = -1]) {
    const payload = cashPayloadFixture(); bad(payload); assert.equal(cashShopUi.readCashGameShopInfo(payload), null);
  }
  const row = cashShopUi.readCashGameShopInfo(cashPayloadFixture()); assert.ok(row); assert.equal(Object.isFrozen(row.info), true);
});
check("Cash confirmation enforces actual wallet class payment positive price quantity and five-stack stock limits", () => {
  const f = cashFixture(); assert.equal(f.confirmation.total, 200); assert.equal(f.confirmation.count * f.confirmation.quantity, 4);
  assert.equal(cashShopUi.makeCashGameShopConfirmation(f.source, 31, 2, "credit").total, 20);
  for (const qty of [0, 11, 100, 1.5]) assert.equal(cashShopUi.makeCashGameShopConfirmation(f.source, 31, qty, "gold"), null);
  assert.equal(cashFixture({ item: { class: "Wizard" } }).confirmation, null);
  assert.equal(cashFixture({ item: { can_buy_gold: false } }).confirmation, null);
  assert.equal(cashFixture({ item: { gold_price: 0 } }).confirmation, null);
  assert.equal(cashFixture({ wallet: { gold: 199 } }).confirmation, null);
  assert.equal(cashFixture({ receiptEnabled: false }).confirmation, null);
  const thin = cashFixture({ item: { count: 20, info: { ...f.payload.item.info, stack_size: 1 } } });
  assert.equal(cashShopUi.cashGameShopQuantityLimit(thin.source.entries[0]), 0); assert.equal(thin.confirmation, null);
});
check("Cash confirm revalidates source price wallet scene and physical owner before any wire", () => {
  const f = cashFixture(), ledger = new cashShopUi.CashGameShopPurchases();
  for (const changed of [{ ...f.source, revision: 2 }, { ...f.source, wallet: { ...f.source.wallet, gold: 499 } },
    { ...f.source, owner: { ...f.owner, sceneRevision: 8 } }, { ...f.source, owner: { ...f.owner, socket: {} } }]) {
    assert.equal(cashShopUi.cashGameShopConfirmationCurrent(f.confirmation, changed), false);
    assert.equal(ledger.reserve(f.confirmation, changed, "cash-1"), null);
  }
  assert.ok(ledger.reserve(f.confirmation, f.source, "cash-1"));
});
check("Cash claim is exact five-key CREDIT/GOLD wire and synchronously single-flight", () => {
  const f = cashFixture(), ledger = new cashShopUi.CashGameShopPurchases();
  const p = ledger.reserve(f.confirmation, f.source, "cash-2"); assert.ok(p);
  assert.deepEqual(p.command, { type: "gameShopBuy", gIndex: 31, quantity: 2, priceType: 1, requestId: "cash-2" });
  assert.equal(ledger.allows(p, f.source, { ...p.command, quantity: 3 }), false);
  assert.equal(ledger.allows(p, f.source, { ...p.command, extra: 1 }), false);
  assert.equal(ledger.claim(p, f.source, p.command), true); assert.equal(ledger.claim(p, f.source, p.command), false);
  assert.equal(ledger.reserve(f.confirmation, f.source, "cash-3"), null);
});
check("Cash only exact protocol request and tuple receipt retires entered purchase", () => {
  const f = cashFixture(), ledger = new cashShopUi.CashGameShopPurchases();
  const p = ledger.reserve(f.confirmation, f.source, "cash-4"); ledger.claim(p, f.source, p.command);
  for (const bad of [{ protocol: "legacy" }, { requestId: "other" }, { gIndex: 32 }, { quantity: 3 }, { priceType: 0 },
    { mailId: null }, { code: "invalidRequest" }, { type: "MailSent" }]) {
    assert.equal(ledger.applyReceipt(cashReceiptFixture(p, bad), f.owner).matched, false); assert.equal(ledger.pending(), p);
  }
  assert.equal(ledger.applyReceipt(cashReceiptFixture(p), { ...f.owner, sessionGeneration: 9 }).matched, false);
  assert.equal(ledger.applyReceipt(cashReceiptFixture(p), { ...f.owner, sceneRevision: 99, mapFileName: "changed" }).matched, true);
  assert.equal(ledger.pending(), null); assert.equal(ledger.applyReceipt(cashReceiptFixture(p), f.owner).matched, false);
});
check("Cash unknown stays pending through close scene wallet updates and same socket generation changes", () => {
  const f = cashFixture(), ledger = new cashShopUi.CashGameShopPurchases();
  const p = ledger.reserve(f.confirmation, f.source, "cash-5"); ledger.claim(p, f.source, p.command); ledger.markUnknown(p);
  assert.equal(ledger.cancelDefinitelyUnsent(p), false); assert.equal(ledger.pending(), p);
  assert.equal(ledger.retireConnection(f.owner.socket, 99), false); assert.equal(ledger.retireConnection({}, 3), false);
  assert.equal(ledger.pending(), p);
  assert.equal(ledger.applyReceipt({ type: "MailSent", requestId: "cash-5" }, f.owner).matched, false);
  assert.equal(ledger.retireConnection({}, 4), true); assert.equal(ledger.pending(), null);
});
check("Cash failure receipt shape and new stock are restricted to native failure contract", () => {
  const f = cashFixture(), ledger = new cashShopUi.CashGameShopPurchases(), p = ledger.reserve(f.confirmation, f.source, "cash-6");
  ledger.claim(p, f.source, p.command);
  assert.equal(cashShopUi.readCashGameShopReceipt(cashReceiptFixture(p, { success: false, mailId: null, code: "" })), null);
  assert.equal(cashShopUi.readCashGameShopReceipt(cashReceiptFixture(p, { success: false, mailId: null, code: "mailFull", newStockLevel: 0 })), null);
  const result = ledger.applyReceipt(cashReceiptFixture(p, { success: false, mailId: null, code: "stockUnavailable", newStockLevel: 0 }), f.owner);
  assert.equal(result.matched, true); assert.equal(result.receipt.success, false); assert.equal(result.receipt.newStockLevel, 0);
});
check("Cash definitely-unsent requires exact unentered proof and never reuses request identity", () => {
  const f = cashFixture(), ledger = new cashShopUi.CashGameShopPurchases(), p = ledger.reserve(f.confirmation, f.source, "cash-7");
  assert.equal(ledger.cancelDefinitelyUnsent(p), true);
  assert.equal(ledger.reserve(f.confirmation, f.source, "cash-7"), null);
  const q = ledger.reserve(f.confirmation, f.source, "cash-8"); assert.ok(q);
  assert.equal(ledger.cancelDefinitelyUnsent(p), false); assert.equal(ledger.pending(), q);
});

function creatureRecordFixture(overrides = {}) {
  return { petType: 4, icon: 40, customName: "Frostling", fullness: 800, slotIndex: 0,
    expireBinaryDatetime: "638900000000000001", blackstoneTime: "638900000000000002", petMode: 0,
    creatureRules: { minimalFullness: 100, mousePickupEnabled: true, mousePickupRange: 2,
      autoPickupEnabled: true, autoPickupRange: 3, semiAutoPickupEnabled: true, semiAutoPickupRange: 4, canProduceBlackstone: false },
    filter: Object.fromEntries(creatureUi.CREATURE_FILTER_KEYS.map(k => [k, true])), pickupGrade: 2,
    maintainFoodTime: "638900000000000003", ...overrides };
}
function creatureFixture(options = {}) {
  const owner = { ...socialOwner, socket: {} }, record = creatureRecordFixture(options.record);
  const source = creatureUi.makeCreaturePlayerSource([record], options.summoned ?? null,
    options.renameEnabled ?? true, owner, 1, true); assert.ok(source);
  return { owner, record, source };
}
check("Pet identity is real petType and physical slot holes are not compact indexes", () => {
  const f = creatureFixture({ record: { slotIndex: 8 } });
  assert.equal(f.source.records[0].petType, 4); assert.equal(f.source.records[0].slotIndex, 8);
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "summon", petType: 8 }, f.source), null);
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "summon", petType: 4 }, f.source).creature.slotIndex, 8);
  assert.equal(creatureUi.makeCreaturePlayerSource([f.record, { ...f.record, slotIndex: 1 }], null, true, f.owner, 2, true), null);
  assert.equal(creatureUi.makeCreaturePlayerSource([f.record, { ...f.record, petType: 5 }], null, true, f.owner, 2, true), null);
  assert.equal(creatureUi.makeCreaturePlayerSource([f.record], 5, true, f.owner, 2, true), null);
});
check("Pet whole raw record rejects missing filter rules illegal modes grades and unsafe i64 Numbers", () => {
  for (const change of [r => delete r.filter.petPickupGold, r => delete r.creatureRules.autoPickupEnabled,
    r => r.petMode = 2, r => r.pickupGrade = 6, r => r.slotIndex = 10,
    r => r.expireBinaryDatetime = 638900000000000001, r => r.petType = Number.MAX_SAFE_INTEGER + 1]) {
    const record = creatureRecordFixture(); change(record); assert.equal(creatureUi.readCreaturePlayerRecord(record), null);
  }
  assert.equal(creatureUi.exactSocialServiceI64("01"), null); assert.equal(creatureUi.exactSocialServiceI64("9223372036854775808"), null);
  assert.equal(creatureUi.exactSocialServiceI64("-9223372036854775808"), "-9223372036854775808");
});
check("Pet summon dismiss and release flags use exact complete five-key Update wire", () => {
  const f = creatureFixture(), summon = creatureUi.creaturePlayerCommand({ kind: "summon", petType: 4 }, f.source);
  assert.deepEqual(Object.keys(summon).sort(), ["creature", "releaseMe", "summonMe", "type", "unsummonMe"]);
  assert.equal(summon.summonMe, true); assert.equal(summon.unsummonMe, false); assert.equal(summon.releaseMe, false);
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "dismiss", petType: 4 }, f.source), null);
  const summoned = creatureFixture({ summoned: 4 });
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "summon", petType: 4 }, summoned.source), null);
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "dismiss", petType: 4 }, summoned.source).unsummonMe, true);
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "release", petType: 4, confirmationName: "Frostling" }, summoned.source), null);
});
check("Pet release ASCII-fold confirmation and rename server permission match Native limits", () => {
  const f = creatureFixture();
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "release", petType: 4, confirmationName: "fRoStLiNg" }, f.source).releaseMe, true);
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "release", petType: 4, confirmationName: "Frostling " }, f.source), null);
  for (const text of ["ab", "a".repeat(16), "名字A", "bad name", "Frostling"]) {
    assert.equal(creatureUi.creaturePlayerCommand({ kind: "rename", petType: 4, text }, f.source), null);
  }
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "rename", petType: 4, text: "New123" }, f.source).creature.customName, "New123");
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "rename", petType: 4, text: "New123" }, creatureFixture({ renameEnabled: false }).source), null);
  const unicode = creatureFixture({ record: { customName: "ÄPet" } });
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "release", petType: 4, confirmationName: "äPet" }, unicode.source), null);
});
check("Pet mode is only legal auto/semi-auto transition and obeys server rules", () => {
  const f = creatureFixture(); assert.equal(creatureUi.creaturePlayerCommand({ kind: "mode", petType: 4, mode: 1 }, f.source).creature.petMode, 1);
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "mode", petType: 4, mode: 2 }, f.source), null);
  const noSemi = creatureFixture({ record: { creatureRules: { ...f.record.creatureRules, semiAutoPickupEnabled: false } } });
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "mode", petType: 4, mode: 1 }, noSemi.source), null);
  const semi = creatureFixture({ record: { petMode: 1, creatureRules: { ...f.record.creatureRules, autoPickupEnabled: false } } });
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "mode", petType: 4, mode: 0 }, semi.source), null);
});
check("Pet options only patch exact nine flags/grade and preserve all other authority fields", () => {
  const f = creatureFixture(), filter = { ...f.record.filter, petPickupGold: false };
  const command = creatureUi.creaturePlayerCommand({ kind: "options", petType: 4, filter, pickupGrade: 5 }, f.source);
  assert.ok(command); assert.equal(command.creature.filter.petPickupGold, false); assert.equal(command.creature.pickupGrade, 5);
  for (const k of Object.keys(f.source.records[0]).filter(k => k !== "filter" && k !== "pickupGrade")) {
    assert.deepEqual(command.creature[k], f.source.records[0][k]);
  }
  assert.equal(f.source.records[0].filter.petPickupGold, true); assert.equal(f.source.records[0].pickupGrade, 2);
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "options", petType: 4, filter: f.record.filter, pickupGrade: 2 }, f.source), null);
  assert.equal(creatureUi.creaturePlayerCommand({ kind: "slot", petType: 4, slotIndex: 9 }, f.source).creature.slotIndex, 9);
});
check("Pet i64 encoder writes exact number literals without quoting or changing other record text", () => {
  const f = creatureFixture(), command = creatureUi.creaturePlayerCommand({ kind: "summon", petType: 4 }, f.source);
  const wire = creatureUi.creaturePlayerCommandJson(command);
  for (const [key, value] of Object.entries(f.source.records[0]).filter(([k]) => ["expireBinaryDatetime", "blackstoneTime", "maintainFoodTime"].includes(k))) {
    assert.ok(wire.includes(`"${key}":${value}`)); assert.equal(wire.includes(`"${key}":"${value}"`), false);
  }
  assert.equal(JSON.parse(wire).creature.customName, "Frostling"); assert.equal(creatureUi.creaturePlayerCommandJson(null), null);
  assert.equal(creatureUi.creaturePlayerCommandJson({ ...command, extra: 1 }), null);
});
check("Pet final proof requires original owner revision record and exact wire and enters once", () => {
  const f = creatureFixture(), ledger = new creatureUi.CreaturePlayerOperations();
  const p = ledger.reserve({ kind: "mode", petType: 4, mode: 1 }, f.source); assert.ok(p);
  assert.equal(ledger.allows(p, { ...f.source, owner: { ...f.owner, sceneRevision: 8 } }, p.command), false);
  assert.equal(ledger.allows(p, { ...f.source, revision: 2 }, p.command), false);
  assert.equal(ledger.allows(p, f.source, { ...p.command, releaseMe: true }), false);
  assert.equal(ledger.claim(p, f.source, p.command), true); assert.equal(ledger.claim(p, f.source, p.command), false);
  assert.equal(ledger.reserve({ kind: "slot", petType: 4, slotIndex: 1 }, f.source), null);
  assert.equal(ledger.cancelDefinitelyUnsent(p), false);
});
check("Pet unknown does not replay or release for unrelated/unchanged/full snapshots", () => {
  const f = creatureFixture(), ledger = new creatureUi.CreaturePlayerOperations();
  const p = ledger.reserve({ kind: "mode", petType: 4, mode: 1 }, f.source); ledger.claim(p, f.source, p.command); ledger.markUnknown(p);
  assert.equal(ledger.observeAuthority({ ...f.source, revision: 2 }), false); assert.equal(ledger.pending(), p);
  const changed = creatureUi.makeCreaturePlayerSource([{ ...f.record, petMode: 1 }], null, true, { ...f.owner, socket: {} }, 2, true);
  assert.equal(ledger.observeAuthority(changed), false); assert.equal(ledger.pending(), p);
  assert.equal(ledger.retireConnection(f.owner.socket, 99), false); assert.equal(ledger.pending(), p);
  const actual = creatureUi.makeCreaturePlayerSource([{ ...f.record, petMode: 1 }], null, true, { ...f.owner, sceneRevision: 99 }, 2, true);
  assert.equal(ledger.observeAuthority(actual), true); assert.equal(ledger.pending(), null);
});
check("Pet New upsert replaces exact petType and release absence requires authoritative full list", () => {
  const f = creatureFixture(), ledger = new creatureUi.CreaturePlayerOperations();
  const p = ledger.reserve({ kind: "release", petType: 4, confirmationName: "Frostling" }, f.source); ledger.claim(p, f.source, p.command);
  const extra = creatureRecordFixture({ petType: 5, slotIndex: 9, customName: "Other" });
  const next = creatureUi.upsertCreaturePlayerRecord(f.source, extra, 2); assert.ok(next); assert.equal(next.records.length, 2);
  assert.equal(next.fullList, false); assert.equal(ledger.observeAuthority(next), false);
  assert.equal(ledger.observeAuthority({ ...next, revision: 3, records: [extra], fullList: false }), false);
  const actual = creatureUi.makeCreaturePlayerSource([extra], null, true, f.owner, 4, true);
  assert.equal(ledger.observeAuthority(actual), true); assert.equal(ledger.pending(), null);
  const replacement = creatureUi.upsertCreaturePlayerRecord(f.source, { ...f.record, customName: "Fresh" }, 2);
  assert.equal(replacement.records.length, 1); assert.equal(replacement.records[0].customName, "Fresh");
});
check("Pet stale definitely-unsent proof cannot cancel successor and only strict new physical connection retires", () => {
  const f = creatureFixture(), ledger = new creatureUi.CreaturePlayerOperations();
  const p = ledger.reserve({ kind: "mode", petType: 4, mode: 1 }, f.source); assert.equal(ledger.cancelDefinitelyUnsent(p), true);
  const q = ledger.reserve({ kind: "slot", petType: 4, slotIndex: 1 }, f.source);
  assert.equal(ledger.cancelDefinitelyUnsent(p), false); assert.equal(ledger.pending(), q); ledger.claim(q, f.source, q.command);
  assert.equal(ledger.retireConnection({}, 3), false); assert.equal(ledger.retireConnection({}, 4), true); assert.equal(ledger.pending(), null);
});
check("Raw creature parser rescues only actual three-field carrier times and never unsafe identity", () => {
  const raw = creatureRecordFixture();
  const encoded = JSON.stringify({ type: "packet", packet: "NewIntelligentCreature", payload: { creature: raw } })
    .replace(/"(expireBinaryDatetime|blackstoneTime|maintainFoodTime)":"(-?\d+)"/g, '"$1":$2');
  const parsed = extendedPackets.parseGatewayMailDates(encoded);
  assert.equal(parsed.payload.creature.expireBinaryDatetime, raw.expireBinaryDatetime);
  assert.equal(parsed.payload.creature.blackstoneTime, raw.blackstoneTime);
  assert.equal(parsed.payload.creature.maintainFoodTime, raw.maintainFoodTime);
  assert.ok(creatureUi.readCreaturePlayerRecord(parsed.payload.creature));
  const unsafeId = encoded.replace('"petType":4', '"petType":9007199254740993');
  const broken = extendedPackets.parseGatewayMailDates(unsafeId);
  assert.equal(typeof broken.payload.creature.petType, "number"); assert.equal(creatureUi.readCreaturePlayerRecord(broken.payload.creature), null);
  const outside = extendedPackets.parseGatewayMailDates('{"date_binary_datetime":638900000000000001,"unique_id":9007199254740993}');
  assert.equal(typeof outside.date_binary_datetime, "number"); assert.equal(typeof outside.unique_id, "number");
});
check("Raw shop parser preserves only exact GameShopItem date carrier and never item identity", () => {
  const payload = cashPayloadFixture(), encoded = JSON.stringify({ type: "packet", packet: "GameShopInfo", payload })
    .replace('"date_binary_datetime":"638900000000000000"', '"date_binary_datetime":638900000000000000');
  const parsed = extendedPackets.parseGatewayMailDates(encoded);
  assert.equal(parsed.payload.item.date_binary_datetime, "638900000000000000");
  assert.ok(cashShopUi.readCashGameShopInfo(parsed.payload));
  const foreign = extendedPackets.parseGatewayMailDates(encoded.replace('"g_index":31', '"g_index":9007199254740993'));
  assert.equal(typeof foreign.payload.item.g_index, "number"); assert.equal(cashShopUi.readCashGameShopInfo(foreign.payload), null);
  const outside = extendedPackets.parseGatewayMailDates('{"date_binary_datetime":638900000000000000}');
  assert.equal(typeof outside.date_binary_datetime, "number");
});
check("Raw source parser preserves existing item expiry and ReceiveMail dates with pet list carriers", () => {
  const record = creatureRecordFixture(), rawList = JSON.stringify({ type: "packet", packet: "UpdateIntelligentCreatureList",
    payload: { creatureList: [record], creatureSummoned: false, summonedCreatureType: 99, pearlCount: 0 } })
    .replace(/"(expireBinaryDatetime|blackstoneTime|maintainFoodTime)":"(-?\d+)"/g, '"$1":$2');
  assert.ok(creatureUi.readCreaturePlayerRecord(extendedPackets.parseGatewayMailDates(rawList).payload.creatureList[0]));
  const item = extendedPackets.parseGatewayMailDates('{"expiry_binary_datetime":638900000000000001}');
  assert.equal(item.expiry_binary_datetime, "638900000000000001");
  const mail = extendedPackets.parseGatewayMailDates('{"type":"packet","packet":"ReceiveMail","payload":{"mailId":3,"dateSentBinaryDatetime":638900000000000001}}');
  assert.equal(mail.payload.dateSentBinaryDatetime, "638900000000000001");
});

// Production shared GuildBuff path: real camel envelope, snake inner carriers.
// Success has no request ID/ACK. Observations follow active delta -> GuildStatus
// -> GuildMemberChange -> full active list (gateway/routing.rs social refresh).
function guildBuffDefinitionFixture(id = 17, extra = {}) {
  return { id, icon: 3, name: `Guild skill ${id}`, level_requirement: 2, points_requirement: 2,
    time_limit: 2, activation_cost: 100, stats: [{ stat: 46, value: 5 }], ...extra };
}
function guildBuffFixture(extraStatus = {}) {
  const owner = { connectionGeneration: 1, sessionGeneration: 7, sceneRevision: 2,
    playerObjectId: 40, mapFileName: "0", socket: {} };
  const status = { guildName: "Knights", guildRankName: "Leader", level: 4,
    experience: 10, maxExperience: 100, gold: 1000, sparePoints: 10,
    memberCount: 3, maxMembers: 15, voting: false, itemCount: 0, buffCount: 0,
    myOptions: 128, myRankId: 0, typed: true, ...extraStatus };
  const catalog = [guildBuffDefinitionFixture(), guildBuffDefinitionFixture(18, {
    icon: 6, time_limit: 0, activation_cost: 500, points_requirement: 3 })];
  const authority = new guildBuffUi.GuildBuffAuthority();
  assert.equal(authority.receiveStatus(status, owner), true);
  assert.equal(authority.receiveList({ remove: 0, activeBuffs: [], guildBuffs: catalog }, owner, "Knights"), true);
  const source = authority.read(owner, "Knights"); assert.ok(source);
  return { owner, status, catalog, authority, source };
}
check("GuildBuff actual snake definitions and active carriers only under true camel envelope", () => {
  const raw = { remove: 0, activeBuffs: [{ id: 17, active: false, active_time_remaining: -1 }], guildBuffs: [guildBuffDefinitionFixture()] };
  const parsed = guildBuffUi.parseGuildBuffList(raw);
  assert.equal(parsed.activeBuffs[0].activeTimeRemaining, -1);
  assert.equal(parsed.guildBuffs[0].pointsRequirement, 2); assert.equal(parsed.guildBuffs[0].activationCost, 100);
  assert.equal(guildBuffUi.parseGuildBuffList({ ...raw, remove: true }), null);
  assert.equal(guildBuffUi.parseGuildBuffList({ ...raw, activeBuffs: [{ id: 17, active: true, activeTimeRemaining: 2 }] }), null);
  assert.equal(guildBuffUi.parseGuildBuffDefinition({ ...guildBuffDefinitionFixture(), level_requirement: undefined, levelRequirement: 2 }), null);
  assert.equal(guildBuffUi.parseGuildBuffList({ ...raw, activeBuffs: [...raw.activeBuffs, ...raw.activeBuffs] }), null);
  assert.equal(guildBuffUi.parseGuildBuffList({ ...raw, guildBuffs: [guildBuffDefinitionFixture(), guildBuffDefinitionFixture()] }), null);
});
check("GuildBuff malformed i32/u8/cost/stat bounds reject before any authority", () => {
  for (const extra of [{ id: -1 }, { id: Number.MAX_SAFE_INTEGER }, { points_requirement: 256 }, { level_requirement: -1 },
    { time_limit: -1 }, { activation_cost: -1 }, { activation_cost: 2147483648 }, { stats: [{ stat: 256, value: 1 }] }, { stats: [{ stat: 12, value: 1.5 }] }]) {
    assert.equal(guildBuffUi.parseGuildBuffDefinition(guildBuffDefinitionFixture(17, extra)), null);
  }
  assert.equal(guildBuffUi.parseGuildBuffDefinition(guildBuffDefinitionFixture(17, { icon: -1 })).icon, -1);
  const f = guildBuffFixture(); delete f.status.sparePoints;
  assert.equal(f.authority.receiveStatus(f.status, f.owner), false); assert.equal(f.authority.read(f.owner, "Knights"), null);
});
check("GuildBuff complete owned count protects absence and partial active lists grant no acquire", () => {
  const f = guildBuffFixture({ buffCount: 2 });
  assert.equal(f.source.enabledReady, false); assert.equal(guildBuffUi.planGuildBuffAction(f.source, 1, 17), null);
  f.authority.receiveList({ remove: 0, activeBuffs: [{ id: 17, active: false, active_time_remaining: -1 }], guildBuffs: [] }, f.owner, "Knights");
  assert.equal(f.authority.read(f.owner, "Knights").enabledReady, false);
  f.authority.receiveList({ remove: 0, activeBuffs: [{ id: 18, active: true, active_time_remaining: 0 }], guildBuffs: [] }, f.owner, "Knights");
  assert.equal(f.authority.read(f.owner, "Knights").enabledReady, true);
});
check("GuildBuff empty active/catalog deltas retain acquired rows and explicit remove1 alone removes", () => {
  const f = guildBuffFixture({ buffCount: 1 });
  f.authority.receiveList({ remove: 0, activeBuffs: [{ id: 17, active: false, active_time_remaining: -1 }], guildBuffs: [] }, f.owner, "Knights");
  f.authority.receiveList({ remove: 0, activeBuffs: [], guildBuffs: [] }, f.owner, "Knights");
  const retained = f.authority.read(f.owner, "Knights"); assert.equal(retained.catalog.length, 2); assert.equal(retained.enabled.length, 1);
  f.authority.receiveList({ remove: 1, activeBuffs: [{ id: 17, active: false, active_time_remaining: -1 }], guildBuffs: [] }, f.owner, "Knights");
  assert.equal(f.authority.read(f.owner, "Knights").enabled.length, 0); assert.equal(f.authority.read(f.owner, "Knights").enabledReady, false);
  f.status.buffCount = 0; f.authority.receiveStatus(f.status, f.owner); assert.equal(f.authority.read(f.owner, "Knights").enabledReady, true);
});
check("GuildBuff true myOptions bit128 alone authorizes mutation while action0 only needs membership", () => {
  for (const options of [0, 1, 64, 127]) {
    const f = guildBuffFixture({ myOptions: options, permissions: ["CanActivateBuff"] });
    assert.equal(guildBuffUi.guildBuffActionError(f.source, 17), "permission");
    assert.equal(guildBuffUi.planGuildBuffAction(f.source, 1, 17), null);
    assert.deepEqual(guildBuffUi.planGuildBuffAction(f.source, 0, 0).wire, { type: "guildBuffUpdate", action: 0, id: 0 });
  }
  const f = guildBuffFixture({ myOptions: 255 }); assert.ok(guildBuffUi.planGuildBuffAction(f.source, 1, 17));
  assert.equal(guildBuffUi.planGuildBuffAction(f.source, 0, 17), null);
  assert.equal(guildBuffUi.planGuildBuffAction(f.source, 3, 17), null);
});
check("GuildBuff acquisition checks true level spare points and timed server gold cost", () => {
  for (const [extra, reason] of [[{ level: 1 }, "level"], [{ sparePoints: 1 }, "points"], [{ gold: 99 }, "funds"]]) {
    const f = guildBuffFixture(extra); assert.equal(guildBuffUi.guildBuffActionError(f.source, 17), reason);
  }
  const f = guildBuffFixture({ gold: 100, sparePoints: 2 });
  assert.deepEqual(guildBuffUi.planGuildBuffAction(f.source, 1, 17), { wire: { type: "guildBuffUpdate", action: 1, id: 17 }, goldCost: 100, pointsCost: 2 });
});
check("GuildBuff permanent acquisition is free gold but reactivation uses full activation cost", () => {
  const f = guildBuffFixture({ gold: 0, sparePoints: 3 });
  assert.deepEqual(guildBuffUi.planGuildBuffAction(f.source, 1, 18), { wire: { type: "guildBuffUpdate", action: 1, id: 18 }, goldCost: 0, pointsCost: 3 });
  f.status.buffCount = 1; f.status.level = 0; f.status.sparePoints = 0; f.authority.receiveStatus(f.status, f.owner);
  f.authority.receiveList({ remove: 0, activeBuffs: [{ id: 18, active: false, active_time_remaining: 0 }], guildBuffs: [] }, f.owner, "Knights");
  assert.equal(guildBuffUi.guildBuffActionError(f.authority.read(f.owner, "Knights"), 18), "funds");
  f.status.gold = 500; f.authority.receiveStatus(f.status, f.owner);
  assert.deepEqual(guildBuffUi.planGuildBuffAction(f.authority.read(f.owner, "Knights"), 2, 18), { wire: { type: "guildBuffUpdate", action: 2, id: 18 }, goldCost: 500, pointsCost: 0 });
});
check("GuildBuff Native row status icon offsets and protocol stat labels use actual IDs", () => {
  const f = guildBuffFixture(); let rows = guildBuffUi.guildBuffRows(f.source);
  assert.equal(rows[0].status, "available"); assert.equal(rows[0].icon, 5);
  f.status.buffCount = 2; f.authority.receiveStatus(f.status, f.owner);
  f.authority.receiveList({ remove: 0, activeBuffs: [{ id: 17, active: true, active_time_remaining: 2 }, { id: 18, active: true, active_time_remaining: 0 }], guildBuffs: [] }, f.owner, "Knights");
  rows = guildBuffUi.guildBuffRows(f.authority.read(f.owner, "Knights"));
  assert.equal(rows[0].status, "countingDown"); assert.equal(rows[0].icon, 4); assert.equal(rows[0].error, "active");
  assert.equal(rows[1].status, "obtained"); assert.equal(guildBuffUi.guildBuffStatLabel(46), "HPRatePercent");
  f.authority.receiveList({ remove: 0, activeBuffs: [{ id: 17, active: false, active_time_remaining: -1 }], guildBuffs: [] }, f.owner, "Knights");
  rows = guildBuffUi.guildBuffRows(f.authority.read(f.owner, "Knights")); assert.equal(rows[0].status, "expired"); assert.equal(rows[0].icon, 3);
});
check("GuildBuff source DTO freezes confirmation and latest status/revision changes reject old intent", () => {
  const f = guildBuffFixture(), dto = guildBuffUi.captureGuildBuffAction(f.source, 1, 17), l = new guildBuffUi.GuildBuffOperations();
  assert.equal(Object.isFrozen(dto), true); assert.equal(Object.isFrozen(f.owner.socket), false);
  f.catalog[0].activation_cost = 999; assert.equal(f.source.catalog[0].activationCost, 100);
  f.status.gold = 99; f.authority.receiveStatus(f.status, f.owner);
  assert.equal(guildBuffUi.guildBuffActionCurrent(dto, f.authority.read(f.owner, "Knights")), false);
  assert.equal(l.reserve(dto, f.authority.read(f.owner, "Knights")), null);
  assert.equal(f.source.gold, 1000);
});
check("GuildBuff final claim checks exact wire owner scene permission and consumes once", () => {
  const f = guildBuffFixture(), l = new guildBuffUi.GuildBuffOperations();
  const dto = guildBuffUi.captureGuildBuffAction(f.source, 1, 17), proof = l.reserve(dto, f.source);
  assert.ok(proof); assert.equal(l.allows(proof, f.source, { ...proof.wire, id: 18 }), false);
  assert.equal(l.allows(proof, f.source, { ...proof.wire, typed: true }), false);
  assert.equal(l.claim(proof, f.authority.read({ ...f.owner, sceneRevision: 3 }, "Knights")), false);
  f.status.myOptions = 0; f.authority.receiveStatus(f.status, f.owner);
  assert.equal(l.claim(proof, f.authority.read(f.owner, "Knights")), false);
  const g = guildBuffFixture(), fresh = new guildBuffUi.GuildBuffOperations();
  const q = fresh.reserve(guildBuffUi.captureGuildBuffAction(g.source, 1, 17), g.source);
  assert.equal(fresh.claim(q, g.source), true); assert.equal(fresh.claim(q, g.source), false);
});
check("GuildBuff receive rejects other guild socket session and invalidated ranks revoke cached options", () => {
  const f = guildBuffFixture(), packet = { remove: 0, activeBuffs: [], guildBuffs: f.catalog };
  assert.equal(f.authority.receiveList(packet, { ...f.owner, socket: {} }, "Knights"), false);
  assert.equal(f.authority.receiveList(packet, { ...f.owner, sessionGeneration: 8 }, "Knights"), false);
  assert.equal(f.authority.receiveList(packet, f.owner, "Other"), false);
  assert.equal(f.authority.receiveStatus(f.status, { ...f.owner, socket: {} }), false);
  assert.equal(f.authority.invalidatePermissions(f.owner, "Knights"), true); assert.equal(f.authority.read(f.owner, "Knights"), null);
});
check("GuildBuff entered unknown survives same physical scene rebase and cannot auto replay", () => {
  const f = guildBuffFixture(), l = new guildBuffUi.GuildBuffOperations(), dto = guildBuffUi.captureGuildBuffAction(f.source, 1, 17), proof = l.reserve(dto, f.source);
  l.claim(proof, f.source); assert.equal(l.outcomeUnknown(proof), true);
  const scene = { ...f.owner, sceneRevision: 3, mapFileName: "1" }, rebased = f.authority.read(scene, "Knights");
  assert.ok(rebased); assert.equal(l.observe(rebased), null); assert.equal(l.retireSession(scene), false);
  assert.equal(f.authority.retireSession(scene), false); assert.equal(l.cancelDefinitelyUnsent(proof), false);
  assert.equal(l.claim(proof, rebased), false); assert.equal(l.reserve(guildBuffUi.captureGuildBuffAction(rebased, 1, 18), rebased), null);
});
check("GuildBuff unrelated authoritative delta is not an owned ACK or intended target", () => {
  const f = guildBuffFixture(), l = new guildBuffUi.GuildBuffOperations(), p = l.reserve(guildBuffUi.captureGuildBuffAction(f.source, 1, 17), f.source);
  l.claim(p, f.source); l.outcomeUnknown(p);
  f.authority.receiveList({ remove: 0, activeBuffs: [{ id: 18, active: true, active_time_remaining: 0 }], guildBuffs: [] }, f.owner, "Knights");
  f.status.buffCount = 1; f.authority.receiveStatus(f.status, f.owner);
  assert.equal(l.observe(f.authority.read(f.owner, "Knights")), null); assert.equal(l.pending.state, "unknown");
});
check("GuildBuff actual delta -> status -> fullactive order resolves only observed target with fresh costs", () => {
  const f = guildBuffFixture(), l = new guildBuffUi.GuildBuffOperations(), p = l.reserve(guildBuffUi.captureGuildBuffAction(f.source, 1, 17), f.source);
  l.claim(p, f.source); l.outcomeUnknown(p);
  const buff = { id: 17, active: true, active_time_remaining: 2 };
  f.authority.receiveList({ remove: 0, activeBuffs: [buff], guildBuffs: [] }, f.owner, "Knights");
  const afterDelta = f.authority.read(f.owner, "Knights"), targetRevision = afterDelta.enabled[0].revision;
  assert.equal(l.observe(afterDelta), null); assert.equal(afterDelta.enabledReady, false);
  f.status.buffCount = 1; f.status.gold = 900; f.status.sparePoints = 8; f.authority.receiveStatus(f.status, f.owner);
  f.authority.receiveList({ remove: 0, activeBuffs: [buff], guildBuffs: [] }, f.owner, "Knights");
  const afterRefresh = f.authority.read(f.owner, "Knights");
  assert.equal(afterRefresh.enabled[0].revision, targetRevision); assert.ok(afterRefresh.statusRevision > targetRevision);
  assert.equal(l.observe(afterRefresh), "observed"); assert.equal(l.pending, null); assert.equal(l.claim(p, afterRefresh), false);
});
check("GuildBuff inactive target reactivation requires true active transition plus later bank status", () => {
  const f = guildBuffFixture({ buffCount: 1 });
  f.authority.receiveList({ remove: 0, activeBuffs: [{ id: 17, active: false, active_time_remaining: -1 }], guildBuffs: [] }, f.owner, "Knights");
  const source = f.authority.read(f.owner, "Knights"), l = new guildBuffUi.GuildBuffOperations();
  const p = l.reserve(guildBuffUi.captureGuildBuffAction(source, 2, 17), source); l.claim(p, source);
  f.authority.receiveStatus(f.status, f.owner); assert.equal(l.observe(f.authority.read(f.owner, "Knights")), null);
  f.authority.receiveList({ remove: 0, activeBuffs: [{ id: 17, active: true, active_time_remaining: 2 }], guildBuffs: [] }, f.owner, "Knights");
  assert.equal(l.observe(f.authority.read(f.owner, "Knights")), null);
  f.status.gold = 900; f.authority.receiveStatus(f.status, f.owner); assert.equal(l.observe(f.authority.read(f.owner, "Knights")), "observed");
});
check("GuildBuff action0 catalogue observation is readonly and empty packet cannot fake completion", () => {
  const f = guildBuffFixture({ myOptions: 0 }), l = new guildBuffUi.GuildBuffOperations();
  const p = l.reserve(guildBuffUi.captureGuildBuffAction(f.source, 0, 0), f.source); l.claim(p, f.source);
  f.authority.receiveList({ remove: 0, activeBuffs: [], guildBuffs: [] }, f.owner, "Knights");
  assert.equal(l.observe(f.authority.read(f.owner, "Knights")), null);
  f.authority.receiveList({ remove: 0, activeBuffs: [], guildBuffs: f.catalog }, f.owner, "Knights");
  assert.equal(l.observe(f.authority.read(f.owner, "Knights")), "observed");
});
check("GuildBuff stale definitely-unsent proof cannot release a newer source reservation", () => {
  const f = guildBuffFixture(), l = new guildBuffUi.GuildBuffOperations(), p = l.reserve(guildBuffUi.captureGuildBuffAction(f.source, 1, 17), f.source);
  l.claim(p, f.source); assert.equal(l.cancelDefinitelyUnsent(p), true);
  const q = l.reserve(guildBuffUi.captureGuildBuffAction(f.source, 1, 18), f.source);
  assert.equal(l.cancelDefinitelyUnsent(p), false); assert.equal(l.pending.proof, q);
});
check("GuildBuff leave/rejoin same-name generation cannot clear old unknown and true session alone retires", () => {
  const f = guildBuffFixture(), l = new guildBuffUi.GuildBuffOperations(), p = l.reserve(guildBuffUi.captureGuildBuffAction(f.source, 1, 17), f.source);
  l.claim(p, f.source); l.outcomeUnknown(p);
  assert.equal(f.authority.receiveStatus({ ...f.status, guildName: "", myRankId: -1, myOptions: 0 }, f.owner), true); assert.equal(f.authority.read(f.owner, "Knights"), null);
  f.status.buffCount = 1; f.authority.receiveStatus(f.status, f.owner);
  f.authority.receiveList({ remove: 0, activeBuffs: [{ id: 17, active: true, active_time_remaining: 2 }], guildBuffs: f.catalog }, f.owner, "Knights");
  f.authority.receiveStatus(f.status, f.owner); const rejoined = f.authority.read(f.owner, "Knights");
  assert.ok(rejoined.guildGeneration > f.source.guildGeneration); assert.equal(l.observe(rejoined), null);
  const next = { ...f.owner, sessionGeneration: 8 };
  assert.equal(l.retireSession(next), true); assert.equal(f.authority.retireSession(next), true);
  assert.equal(l.pending, null); assert.equal(f.authority.read(next, "Knights"), null);
});

// Extract only the parity service boundary from the actual Page. Each selected
// statement is retained whole; the large unrelated movement/storage pipeline is
// covered by its existing finite scripts rather than duplicated here.
const parityPageUrl = new URL("../app/page.tsx", import.meta.url);
const parityPageText = readFileSync(parityPageUrl, "utf8");
const parityPageAst = ts.createSourceFile(fileURLToPath(parityPageUrl), parityPageText, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const parityPageNames = ["currentSpellsOwner", "currentSocialReplyOwner", "currentSocialReceiveOwner", "sameSocialPhysicalOwner",
  "currentHeroModel", "parityItemMutationAllowed", "heroProofCurrent", "heroInputAllowed", "otherPlayerUiBlocksInput",
  "referenceWindowsBlockGameplay", "heroUiLeaseCurrent", "advanceHeroWindowEpochs", "changeHeroWindows", "syncHeroWindowActor", "scheduleHeroRestock", "syncObservePreference", "currentCreatureSource", "currentCashGameShopSource",
  "currentQuestWorldIdentity", "currentCombatModeOwner", "sameCombatModeOwner", "nextCombatModeRevision",
  "captureCombatModeSnapshot", "captureCombatModeReceipt", "readCombatModeSource",
  "currentGuildBuffSource", "captureParityPacket", "captureParitySnapshot", "readPlayerItemTooltip", "readHeroItemTooltip",
  "readCashGameShopItemTooltip", "guildStorageTooltipItem", "readGuildStorageItemTooltip", "readSocialItemSurface", "readNpcRepairOwner", "captureNpcRepairDialog", "currentNpcRepairDialogSource", "readNpcRepairView", "selectNpcRepair", "toggleNpcRepairHold", "beginNpcRepairDrag", "cancelNpcRepairDrag", "dropNpcRepairDrag", "confirmNpcRepair"];
const parityDeclarations = new Map();
let paritySendRawNode, parityReceiptNode, parityReceiveGuard;
function visitParityPage(node) {
  if (ts.isFunctionDeclaration(node) && node.name) {
    const name = node.name.text;
    if (parityPageNames.includes(name)) {
      assert.equal(parityDeclarations.has(name), false, "sole actual parity Page declaration " + name);
      parityDeclarations.set(name, node.getText(parityPageAst));
    }
    if (name === "sendRaw") paritySendRawNode = node;
    if (name === "handleGatewayEvent") {
      parityReceiveGuard = node.body.statements[0];
      const receipts = node.body.statements.filter(statement => ts.isIfStatement(statement)
        && statement.expression.getText(parityPageAst) === '(event as {type:string}).type === "gameShopReceipt"');
      assert.equal(receipts.length, 1, "sole actual cash receipt entry"); parityReceiptNode = receipts[0];
    }
  }
  ts.forEachChild(node, visitParityPage);
}
visitParityPage(parityPageAst);
for (const name of parityPageNames) assert.ok(parityDeclarations.has(name), "actual Page function " + name);
assert.ok(paritySendRawNode && parityReceiptNode && parityReceiveGuard);
const paritySendStatements = paritySendRawNode.body.statements;
const parityFinalStart = paritySendStatements.findIndex(statement => ts.isIfStatement(statement)
  && statement.expression.getText(parityPageAst) === "options?.guildBuffProof"
  && statement.thenStatement.getText(parityPageAst).includes(".claim("));
const paritySocketIndex = paritySendStatements.findIndex(statement => ts.isTryStatement(statement)
  && statement.tryBlock.statements.some(child => ts.isExpressionStatement(child)
    && child.expression.getText(parityPageAst) === "socket.send(serialized)"));
assert.ok(parityFinalStart > 0 && paritySocketIndex > parityFinalStart, "actual final parity claims through socket entry");
const parityPreflightStart = paritySendStatements.findIndex(statement => ts.isIfStatement(statement)
  && statement.expression.getText(parityPageAst).includes('command.type === "gameShopBuy"'));
const parityPreflightEnd = paritySendStatements.findIndex((statement, index) => index > parityPreflightStart
  && ts.isIfStatement(statement) && statement.expression.getText(parityPageAst).includes('"removeFriend"'));
assert.ok(parityPreflightStart > 0 && parityPreflightEnd > parityPreflightStart, "actual contiguous service ingress guards");
const paritySerializeIndex = paritySendStatements.findIndex(statement => ts.isVariableStatement(statement)
  && statement.declarationList.declarations.some(declaration => declaration.name.getText(parityPageAst) === "serialized"));
const parityDtoIndex = paritySendStatements.findIndex(statement => ts.isVariableStatement(statement)
  && statement.declarationList.declarations.some(declaration => declaration.name.getText(parityPageAst) === "wireCommand"));
assert.ok(paritySerializeIndex > parityPreflightEnd && parityDtoIndex > paritySerializeIndex && parityDtoIndex < parityFinalStart);
assert.equal(ts.isIfStatement(paritySendStatements[parityDtoIndex + 1]),true);
assert.equal(ts.isIfStatement(paritySendStatements[parityDtoIndex + 2]),true);
const parityCollectClaim = paritySendStatements.filter(statement => ts.isIfStatement(statement)
  && statement.expression.getText(parityPageAst) === 'options?.mailProof?.commandType === "collectParcel"');
assert.equal(parityCollectClaim.length,1,"sole actual final parcel collection barrier claim");
// sendRaw's ordinary-command final slice also reads the real Auth classifier.
// Extract that sole pure declaration, without loading wallet/network modules or
// replacing any final claim. All parity commands naturally classify as null.
const parityAuthUrl = new URL("../lib/client-login-runtime.ts", import.meta.url);
const parityAuthAst = ts.createSourceFile(fileURLToPath(parityAuthUrl), readFileSync(parityAuthUrl,"utf8"), ts.ScriptTarget.Latest, true);
const parityAuthClassifiers = parityAuthAst.statements.filter(node => ts.isFunctionDeclaration(node) && node.name?.text === "preauthCommandKind");
assert.equal(parityAuthClassifiers.length, 1, "sole actual preauth command classifier");
const parityAuthClassifierText = parityAuthClassifiers[0].getText(parityAuthAst);
assert.match(parityAuthClassifierText, /^export function preauthCommandKind\(/);
parityDeclarations.set("preauthCommandKind", parityAuthClassifierText.replace(/^export /, ""));
const parityPageJs = ts.transpileModule([...parityDeclarations.values()].join("\n")
  + "\nfunction parityIngress(command, options) {\n"
  + paritySendStatements.slice(parityPreflightStart, parityPreflightEnd).map(n => n.getText(parityPageAst)).join("\n")
  + "\nreturn true;}\nfunction parityFinal(command, options, socket, beforeFinal) {\n"
  + paritySendStatements.slice(paritySerializeIndex, paritySerializeIndex + 2).map(n=>n.getText(parityPageAst)).join("\n")
  // Fake synchronous mir2:action listener, before all actual final DTO/claim checks.
  + "\nif (beforeFinal) beforeFinal();\n"
  + paritySendStatements.slice(parityDtoIndex, parityDtoIndex + 3).map(n=>n.getText(parityPageAst)).join("\n") + "\n"
  + paritySendStatements.slice(parityFinalStart, paritySocketIndex + 1).map(n => n.getText(parityPageAst)).join("\n")
  + "\nreturn true;}\nfunction parityReceipt(event, connectionGeneration, source) {\n"
  + parityReceiveGuard.getText(parityPageAst) + "\n" + parityReceiptNode.getText(parityPageAst) + "\n}"
  + "\nfunction parityCollectClaim(wireCommand,options){" + parityCollectClaim[0].getText(parityPageAst) + "\nreturn true;}",
  {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;

function parityPageFixture(owner, world = {}) {
  const sent = [], sentBodies = [], microtasks = [], socket = owner.socket; socket.readyState = 1;
  socket.send = body => {sentBodies.push(body); sent.push(JSON.parse(body));};
  const ref = current => ({current});
  const scope = {
    WebSocket:{OPEN:1}, document:{visibilityState:"visible",hasFocus:()=>true},
    isSpectatorBrowserMode:()=>false, initialSceneAssetsReadyRef:ref(true), queueMicrotask:callback=>microtasks.push(callback),
    playerReferenceWindowsRef:ref({help:false,hotkeys:false,options:false,capture:false}),
    bevyHpLocalOverlayOpenRef:ref(false), questReactModalOpenRef:ref(false), bagOpenRef:ref(false), characterOpenRef:ref(false), questLogOpenRef:ref(false),
    socialReplyWindowsRef:ref({group:false,bonds:false}), socialRosterWindowsRef:ref({friends:false}), rankingWindowRef:ref(false),
    marketOpenRef:ref(false), conquestOpenRef:ref(false), buffsOpenRef:ref(false), mailUiOpenRef:ref(false), worldMapOpenRef:ref(false),
    chatSettingsOpenRef:ref(false), tutorialOpenRef:ref(false), npcShopServiceRef:ref(null), npcRepairServiceRef:ref(null),
    storageServiceActiveRef:ref(false), npcShopUiIngressRef:ref(null), npcRepairAuthorityRef:ref({invalidateInventory:()=>{}}), storageUiIngressRef:ref(null), combatIngressRef:ref(null), spellsIngressRef:ref(null),
    skillBarPointerHeldRef:ref(false), heroUiProofLeasesRef:ref(new WeakMap()),
    heroWindowEpochsRef:ref({inventory:1,character:1,belt:1}), heroWindowsRef:ref({inventoryOpen:true,characterOpen:false,characterPage:"equipment",beltVisible:true,beltVertical:false}),
    heroWindowActorRef:ref(null), heroRestockScheduledRef:ref(false), setHeroWindows:()=>{}, sameHeroSession:heroUi.sameHeroSession,
    observeBootstrapRef:ref(null), observePreferenceRef:ref(null),
    authSendRef:ref(null), chatUiRuntimeRef:ref(null), questCoreRuntimeRef:ref(null),
    npcRepairDialogSourceRef:ref(null), npcRepairDialogBindingRef:ref(null), activeInventoryTabRef:ref("bag1"), sendRawHandler:ref(null),
    sendRaw:(...args)=>scope.sendRawHandler.current(...args), mailMutationAllowed:mailParcelGateway.mailMutationAllowed,
    mapImageRouteRef:ref(null), mapRouteLocalModalRef:ref(false),
    questSceneRevisionRef:ref(owner.sceneRevision), combatModePhysicalRef:ref(null),
    combatModeRawRef:ref(null), combatModeRevisionRef:ref(0), combatModeHostRef:ref(null),
    normalizeQuestMapFileName:questWorldDocument.normalizeQuestMapFileName,
    nextCombatModePhysicalGeneration:combatModeKeys.nextCombatModePhysicalGeneration,
    worldRef:ref({playerObjectId:owner.playerObjectId,mapFileName:owner.mapFileName,connected:true,entities:[],stage5Systems:{},...world}),
    socketRef:ref(socket), screenRef:ref("game"), equipmentStartGameRef:ref({...owner}),
    equipmentConnectionGenerationRef:ref(owner.connectionGeneration), equipmentSessionGenerationRef:ref(owner.sessionGeneration),
    equipmentHostSuspendReasonRef:ref(null), equipmentBagOwnerRef:ref({ownerRevision:1}), socialSceneRevisionRef:ref(owner.sceneRevision),
    heroAuthorityRef:ref(new heroUi.HeroPlayerAuthority()), heroOperationsRef:ref(new heroUi.HeroPlayerOperations()),
    guildBuffAuthorityRef:ref(new guildBuffUi.GuildBuffAuthority()), guildBuffOperationsRef:ref(new guildBuffUi.GuildBuffOperations()),
    creatureOperationsRef:ref(new creatureUi.CreaturePlayerOperations()), creatureSourceRef:ref(null), creatureRenameRef:ref(null),
    creatureRevisionRef:ref(1), cashCatalogRef:ref(null), cashReceiptSocketRef:ref(socket), cashPurchasesRef:ref(new cashShopUi.CashGameShopPurchases()),
    heroManagementOpenRef:ref(true), heroBeltProofRef:ref(null), heroPetOpenRef:ref(true), cashShopOpenRef:ref(true), socialItemWindowsRef:ref({guild:true}),
    equipmentControllerRef:ref(null), pendingStorageRequestsRef:ref(new Map()), socialItemOperationsRef:ref(new socialOperations.SocialWindowOperations()),
    storageRentalRef:ref(new storageRental.StorageRentalConfirmation()), mailCollectBarrierRef:ref(null), mailDispatcherRef:ref(null),
    mailParcelRef:ref(null), npcBuyDispatcherRef:ref(null), worldSnapshotVersionRef:ref(1), equipmentSnapshotRef:ref({...owner}),
    renderParityServices:()=>{}, appendLog:()=>{}, t:(_key,_args,fallback)=>fallback,
    isMailItemMutation:mailParcelGateway.isMailItemMutation,
    makeCreaturePlayerSource:creatureUi.makeCreaturePlayerSource, upsertCreaturePlayerRecord:creatureUi.upsertCreaturePlayerRecord,
    creaturePlayerCommandJson:creatureUi.creaturePlayerCommandJson,
    makeCashGameShopSource:cashShopUi.makeCashGameShopSource, emptyCashGameShopCatalog:cashShopUi.emptyCashGameShopCatalog,
    upsertCashGameShopInfo:cashShopUi.upsertCashGameShopInfo, applyCashGameShopStock:cashShopUi.applyCashGameShopStock,
    parseMailList:extendedPackets.parseMailList,
    runtimeRef:ref(null), readSharedItemTooltip:sharedTooltip.readSharedItemTooltip, readSharedItemCatalogInfo:sharedTooltip.readSharedItemCatalogInfo,
    ...socialItems, socialCatalogRef:ref(new Map()), guildStorageRawRef:ref(null), guildPermissionsRef:ref(null),
    socialOwnTradeRef:ref(null), tradeLifecycleRef:ref({state:"closed",partner:""}), tradeIncarnationRef:ref(1),
  };
  const keys = Object.keys(scope), api = new Function(...keys, parityPageJs + "\nreturn {currentCombatModeOwner,readCombatModeSource,captureCombatModeSnapshot,captureCombatModeReceipt,parityIngress,parityFinal,parityReceipt,parityCollectClaim,heroProofCurrent,heroInputAllowed,heroUiLeaseCurrent,changeHeroWindows,parityItemMutationAllowed,currentHeroModel,currentCreatureSource,currentCashGameShopSource,currentGuildBuffSource,captureParityPacket,captureParitySnapshot,readPlayerItemTooltip,readHeroItemTooltip,readCashGameShopItemTooltip,guildStorageTooltipItem,readGuildStorageItemTooltip,readSocialItemSurface,readNpcRepairView,captureNpcRepairDialog,toggleNpcRepairHold,beginNpcRepairDrag,dropNpcRepairDrag,confirmNpcRepair};")(...keys.map(k=>scope[k]));
  return {scope,api,socket,sent,sentBodies,microtasks};
}
function heroPageFixture() {
  const f=heroFixture(), page=parityPageFixture(f.owner,f.world);
  page.scope.heroAuthorityRef.current=f.authority; page.scope.heroWindowActorRef.current=f.model;
  // The host updates management visibility together; link the fake refs so old
  // visibility mutations retain that real relationship without stubbing a guard.
  page.scope.heroWindowsRef.current={...page.scope.heroWindowsRef.current,
    get inventoryOpen(){return page.scope.heroManagementOpenRef.current;}};
  page.scope.heroPetOpenRef.current=false; page.scope.cashShopOpenRef.current=false; page.scope.socialItemWindowsRef.current={guild:false,trade:false};
  const proof=page.scope.heroOperationsRef.current.reserve(f.model,{kind:"move",from:2,to:3}); assert.ok(proof);
  page.scope.heroUiProofLeasesRef.current.set(proof,{kind:"window",lease:{origin:"inventory",epoch:1}});
  return {...f,...page,proof};
}
function creaturePageFixture() {
  const f=creatureFixture(), page=parityPageFixture(f.owner);
  page.scope.creatureSourceRef.current=f.source; page.scope.creatureRenameRef.current={owner:f.owner,enabled:true};
  const proof=page.scope.creatureOperationsRef.current.reserve({kind:"rename",petType:4,text:"NewName"},f.source); assert.ok(proof);
  return {...f,...page,proof};
}

check("Page Hero actual ingress and final claim recheck exact DTO source and consumes once", () => {
  const f=heroPageFixture(), options={heroProof:f.proof};
  assert.equal(f.api.parityIngress({...f.proof.wire},options),true);
  assert.equal(f.api.parityFinal({...f.proof.wire,to:4},options,f.socket),false);
  assert.equal(f.scope.heroOperationsRef.current.pending.state,"reserved"); assert.equal(f.sent.length,0);
  const changed={...f.proof.wire};
  assert.equal(f.api.parityFinal(changed,options,f.socket,()=>{changed.to=4;}),false);
  assert.equal(f.scope.heroOperationsRef.current.pending.state,"reserved");
  f.info.inventory[2].count=2; f.authority.receiveInformation(f.info,f.owner);
  assert.equal(f.api.parityFinal({...f.proof.wire},options,f.socket),false); assert.equal(f.sent.length,0);
  const g=heroPageFixture(); assert.equal(g.api.parityFinal({...g.proof.wire},{heroProof:g.proof},g.socket),true);
  assert.deepEqual(g.sent,[g.proof.wire]); assert.equal(g.api.parityFinal({...g.proof.wire},{heroProof:g.proof},g.socket),false);
  assert.equal(g.scope.heroOperationsRef.current.pending.state,"entered");
  g.api.captureParityPacket("MoveItem",{grid:"HeroInventory",from:2,to:4,success:true});
  assert.equal(g.scope.heroOperationsRef.current.pending.state,"entered");
  g.api.captureParityPacket("MoveItem",{grid:"HeroInventory",from:2,to:3,success:true});
  assert.equal(g.scope.heroOperationsRef.current.pending.state,"acknowledged");
  g.api.captureParitySnapshot({...g.world,heroStats:undefined});
  assert.equal(g.scope.heroOperationsRef.current.pending.state,"acknowledged","partial source cannot release a matching ACK");
  g.world.heroInventoryItems[1].slot=3; g.api.captureParitySnapshot(g.world);
  assert.equal(g.scope.heroOperationsRef.current.pending,null); assert.equal(g.api.currentHeroModel().inventory[3].uniqueId,502);
});
check("Page Hero and parcel collect actual reciprocal barrier releases only collected then newer complete personal snapshot", () => {
  const f=heroPageFixture();
  assert.equal(f.api.parityItemMutationAllowed({type:"collectParcel",mailId:7}),false);
  assert.equal(f.api.parityIngress({type:"collectParcel",mailId:7},{}),false);
  assert.equal(f.api.parityItemMutationAllowed(f.proof.wire,f.proof),true);
  assert.equal(f.api.parityItemMutationAllowed({type:"chat",message:"hello"}),true);
  f.scope.heroOperationsRef.current.cancelDefinitelyUnsent(f.proof);
  assert.equal(f.api.parityFinal({type:"collectParcel",mailId:7},{mailProof:{commandType:"collectParcel"}},f.socket),true);
  assert.equal(f.api.parityItemMutationAllowed(f.proof.wire),false);
  const next=f.scope.heroOperationsRef.current.reserve(f.model,{kind:"move",from:2,to:3});
  assert.equal(f.api.heroProofCurrent(next,next.wire),false);
  f.api.captureParitySnapshot(f.world); assert.ok(f.scope.mailCollectBarrierRef.current);
  f.api.captureParityPacket("ReceiveMail",{mail:[{mailId:8,collected:true}]});
  assert.equal(f.scope.mailCollectBarrierRef.current.observedSnapshot,null);
  f.api.captureParityPacket("ReceiveMail",{mail:[{mailId:7,collected:true}]});
  assert.equal(f.scope.mailCollectBarrierRef.current.observedSnapshot,1);
  f.api.captureParitySnapshot(f.world); assert.ok(f.scope.mailCollectBarrierRef.current);
  f.scope.worldSnapshotVersionRef.current=2; f.scope.equipmentSnapshotRef.current={...f.owner,sessionGeneration:99};
  f.api.captureParitySnapshot(f.world); assert.ok(f.scope.mailCollectBarrierRef.current);
  f.scope.equipmentSnapshotRef.current={...f.owner}; f.api.captureParitySnapshot(f.world);
  assert.equal(f.scope.mailCollectBarrierRef.current,null); assert.equal(f.api.parityItemMutationAllowed(next.wire,next),true);
});
check("Page Hero actual final source guard rejects paused owner closed window and concurrent item custody", () => {
  for (const block of [s=>s.equipmentHostSuspendReasonRef.current="logoutPending",s=>s.heroManagementOpenRef.current=false,
    s=>s.pendingStorageRequestsRef.current.set("pending",{}),s=>s.mailParcelRef.current={state:{blockedUniqueIds:[502]}},
    s=>s.npcBuyDispatcherRef.current={status:()=>({flight:{}})},s=>s.equipmentControllerRef.current={status:()=>({pending:1})}]) {
    const f=heroPageFixture();
    assert.equal(f.api.parityFinal({...f.proof.wire},{heroProof:f.proof},f.socket,()=>block(f.scope)),false); assert.equal(f.sent.length,0);
    assert.equal(f.scope.heroOperationsRef.current.pending.state,"reserved");
  }
  const f=heroPageFixture(); f.scope.heroManagementOpenRef.current=false; f.scope.heroBeltProofRef.current=f.proof;
  // This protected baseline still checks the independent temporary exception.
  // A plain old window Move cannot acquire that scope just by setting a ref.
  assert.equal(f.api.heroProofCurrent(f.proof,f.proof.wire),false);
  const ledger=f.scope.heroOperationsRef.current;
  assert.equal(ledger.cancelDefinitelyUnsent(f.proof),true);
  const acceptedUse=ledger.reserve(f.model,{kind:"use",slot:0}); assert.ok(acceptedUse);
  assert.equal(ledger.claim(acceptedUse,f.model),true);
  assert.equal(ledger.receipt("UseItem",{grid:"HeroInventory",uniqueId:501,success:true},f.owner,f.authority.authoritySerial),true);
  const afterUse=publishHeroBeltConsumption(f);
  assert.equal(ledger.observe(afterUse),true);
  f.proof=ledger.reserveRestock(afterUse); assert.ok(f.proof);
  assert.deepEqual(f.proof.wire,{type:"moveItem",grid:"HeroInventory",from:2,to:0});
  f.scope.heroUiProofLeasesRef.current.set(f.proof,{kind:"acceptedRestock"});
  f.scope.heroBeltProofRef.current=f.proof;
  assert.equal(f.api.heroProofCurrent(f.proof,f.proof.wire),true,"captured belt proof is the actual temporary window exception");
});
check("Page Pet rename permit is spent only after final exact claim and unknown cannot restore it", () => {
  const f=creaturePageFixture(), options={creatureProof:f.proof};
  assert.equal(f.api.parityIngress({...f.proof.command},options),true); assert.equal(f.scope.creatureRenameRef.current.enabled,true);
  assert.equal(f.api.parityFinal({...f.proof.command,extra:true},options,f.socket),false);
  assert.equal(f.scope.creatureRenameRef.current.enabled,true);
  f.socket.send=()=>{throw Error("fake transport uncertainty");};
  assert.throws(()=>f.api.parityFinal({...f.proof.command},options,f.socket),/fake transport uncertainty/);
  assert.equal(f.scope.creatureRenameRef.current.enabled,false); assert.equal(f.scope.creatureSourceRef.current.renameEnabled,false);
  assert.strictEqual(f.scope.creatureOperationsRef.current.pending(),f.proof);
  assert.equal(f.scope.creatureOperationsRef.current.cancelDefinitelyUnsent(f.proof),false);
  f.api.captureParityPacket("UpdateIntelligentCreatureList",{creatureList:[f.record],creatureSummoned:false});
  assert.equal(f.scope.creatureSourceRef.current.renameEnabled,false); assert.strictEqual(f.scope.creatureOperationsRef.current.pending(),f.proof);
  assert.equal(f.api.parityFinal({...f.proof.command},options,f.socket),false);
});
check("Page Pet final claim rechecks current window source revision and true physical owner before spending rename", () => {
  for (const change of [f=>f.scope.heroPetOpenRef.current=false,f=>f.scope.socialSceneRevisionRef.current++,
    f=>f.scope.creatureSourceRef.current=creatureUi.makeCreaturePlayerSource([f.record],null,true,f.owner,2,true),
    f=>f.scope.equipmentSessionGenerationRef.current++]) {
    const f=creaturePageFixture();
    assert.equal(f.api.parityFinal({...f.proof.command},{creatureProof:f.proof},f.socket,()=>change(f)),false);
    assert.equal(f.scope.creatureRenameRef.current.enabled,true); assert.equal(f.sent.length,0);
  }
  const f=creaturePageFixture(); assert.equal(f.api.parityFinal({...f.proof.command},{creatureProof:f.proof},f.socket),true);
  assert.equal(f.scope.creatureSourceRef.current.renameEnabled,false);
  assert.deepEqual(f.sentBodies,[creatureUi.creaturePlayerCommandJson(f.proof.command)],"actual numeric i64 serialization survives the final claim");
});
check("Page Cash actual catalog stock receive parses keyed metadata and malformed packet revokes source", () => {
  const f=cashFixture(), page=parityPageFixture(f.owner,{gold:500,credit:100,entities:[{objectId:f.owner.playerObjectId,classKey:"Warrior"}]});
  page.api.captureParityPacket("GameShopStock",{gIndex:31,stockLevel:3,typed:true});
  page.api.captureParityPacket("GameShopInfo",f.payload);
  const source=page.api.currentCashGameShopSource(); assert.ok(source); assert.equal(source.entries[0].stock_level,3);
  assert.equal(source.entries[0].info.image,77); assert.equal(source.receiptEnabled,true);
  const confirmation=cashShopUi.makeCashGameShopConfirmation(source,31,2,"gold");
  const proof=page.scope.cashPurchasesRef.current.reserve(confirmation,source,"web-stock-race"); assert.ok(proof);
  assert.equal(page.api.parityFinal({...proof.command},{cashProof:proof},page.socket,
    ()=>page.api.captureParityPacket("GameShopStock",{gIndex:31,stockLevel:1,typed:true})),false);
  assert.strictEqual(page.scope.cashPurchasesRef.current.pending(),proof); assert.equal(page.sent.length,0);
  page.scope.cashReceiptSocketRef.current={}; assert.equal(page.api.currentCashGameShopSource().receiptEnabled,false);
  page.api.captureParityPacket("GameShopStock",{gIndex:31,stockLevel:2,typed:false});
  assert.equal(page.scope.cashCatalogRef.current,null); assert.equal(page.api.currentCashGameShopSource(),null);
});
check("Page Cash final exact claim and real receipt entry reject wrong request stream and update stock only on match", () => {
  const f=cashFixture(), page=parityPageFixture(f.owner,{gold:500,credit:100,entities:[{objectId:f.owner.playerObjectId,classKey:"Warrior"}]});
  page.scope.cashCatalogRef.current={owner:f.owner,catalog:f.catalog,revision:1};
  const proof=page.scope.cashPurchasesRef.current.reserve(f.confirmation,page.api.currentCashGameShopSource(),"web-page-1"); assert.ok(proof);
  assert.equal(page.api.parityFinal({...proof.command,requestId:"foreign"},{cashProof:proof},page.socket),false);
  assert.equal(page.api.parityFinal({...proof.command},{cashProof:proof},page.socket),true);
  const receipt=cashReceiptFixture(proof,{newStockLevel:8});
  page.api.parityReceipt({...receipt,requestId:"old"},f.owner.connectionGeneration,page.socket);
  page.api.parityReceipt(receipt,f.owner.connectionGeneration+1,page.socket);
  page.api.parityReceipt(receipt,f.owner.connectionGeneration,{});
  assert.strictEqual(page.scope.cashPurchasesRef.current.pending(),proof); assert.equal(page.scope.cashCatalogRef.current.revision,1);
  page.api.parityReceipt(receipt,f.owner.connectionGeneration,page.socket);
  assert.equal(page.scope.cashPurchasesRef.current.pending(),null); assert.equal(page.scope.cashCatalogRef.current.catalog.entries[0].stock_level,8);
  assert.equal(page.scope.cashCatalogRef.current.revision,2);
  page.api.parityReceipt(receipt,f.owner.connectionGeneration,page.socket); assert.equal(page.scope.cashCatalogRef.current.revision,2);
});
check("Page GuildBuff final actual source rejects changed permission inventory catalog bank and closed guild before claim", () => {
  for (const change of [f=>f.status.myOptions=0,f=>f.status.buffCount=1,f=>f.status.gold=99,
    f=>f.catalog[0].activation_cost=999]) {
    const f=guildBuffFixture(), page=parityPageFixture(f.owner,{stage5Systems:{guild:{name:"Knights"}}});
    page.scope.guildBuffAuthorityRef.current=f.authority;
    const proof=page.scope.guildBuffOperationsRef.current.reserve(guildBuffUi.captureGuildBuffAction(f.source,1,17),f.source);
    assert.equal(page.api.parityIngress({...proof.wire},{guildBuffProof:proof}),true);
    assert.equal(page.api.parityFinal({...proof.wire},{guildBuffProof:proof},page.socket,()=>{
      change(f); page.api.captureParityPacket("GuildStatus",f.status);
      page.api.captureParityPacket("GuildBuffList",{remove:0,activeBuffs:[],guildBuffs:f.catalog});
    }),false); assert.equal(page.sent.length,0);
    assert.equal(page.scope.guildBuffOperationsRef.current.pending.state,"reserved");
  }
  const f=guildBuffFixture(), page=parityPageFixture(f.owner,{stage5Systems:{guild:{name:"Knights"}}}); page.scope.guildBuffAuthorityRef.current=f.authority;
  const proof=page.scope.guildBuffOperationsRef.current.reserve(guildBuffUi.captureGuildBuffAction(f.source,1,17),f.source);
  page.scope.socialItemWindowsRef.current.guild=false;
  assert.equal(page.api.parityFinal({...proof.wire},{guildBuffProof:proof},page.socket),false);
  page.scope.socialItemWindowsRef.current.guild=true;
  assert.equal(page.api.parityFinal({...proof.wire,typed:true},{guildBuffProof:proof},page.socket),false);
  assert.equal(page.api.parityFinal({...proof.wire},{guildBuffProof:proof},page.socket),true); assert.deepEqual(page.sent,[proof.wire]);
});
check("Shared tooltip actual parser rejects malformed ordered sections colours text and size", () => {
  const valid={broken:false,sourceComplete:true,sections:[{kind:"name",lines:[{text:"Sword",colour:"white"}]},{kind:"attack",lines:[]}]};
  const parsed=sharedTooltip.parseCrystalTooltipDocument(valid); assert.ok(parsed); assert.equal(Object.isFrozen(parsed.sections[0].lines[0]),true);
  for (const value of [{...valid,broken:0},{...valid,sections:[valid.sections[1],valid.sections[0]]},
    {...valid,sections:[valid.sections[0],valid.sections[0]]},{...valid,sections:[{kind:"unknown",lines:[]}]},
    {...valid,sections:[{kind:"name",lines:[{text:"x\0y",colour:"white"}]}]},
    {...valid,sections:[{kind:"name",lines:[{text:"x",colour:"unknown"}]}]},
    {...valid,sections:[{kind:"name",lines:[{text:"x".repeat(8193),colour:"white"}]}]},
    {...valid,sections:[{kind:"name",lines:Array(513).fill({text:"x",colour:"white"})}]}])
    assert.equal(sharedTooltip.parseCrystalTooltipDocument(value),null);
});
check("Shared tooltip JS memory getter caches exact request per second and never caches malformed ABI responses", () => {
  let calls=0; const document={broken:false,sourceComplete:true,sections:[{kind:"name",lines:[{text:"Actual",colour:"white"}]}]};
  const runtime={getMir2ItemTooltipDocument(json){calls++; assert.equal(JSON.parse(json).version,1); return JSON.stringify({version:1,ok:true,document});}};
  const item={uniqueId:501,itemIndex:10,name:"Actual",icon:1,count:1,tooltipSource:{userItem:heroItemFixture(501)}}, player={level:20};
  const first=sharedTooltip.readSharedItemTooltip(runtime,item,player,1000); assert.ok(first);
  assert.strictEqual(sharedTooltip.readSharedItemTooltip(runtime,item,player,1999),first); assert.equal(calls,1);
  sharedTooltip.readSharedItemTooltip(runtime,{...item,count:2},player,1999); sharedTooltip.readSharedItemTooltip(runtime,item,player,2000); assert.equal(calls,3);
  for (const result of ["not json",JSON.stringify({version:2,ok:true,document}),JSON.stringify({version:1,ok:false,document}),
    JSON.stringify({version:1,ok:true,document:{...document,broken:"false"}})]) {
    let reads=0; const bad={getMir2ItemTooltipDocument:()=>{reads++; return result;}};
    assert.equal(sharedTooltip.readSharedItemTooltip(bad,item,player,1000),null);
    assert.equal(sharedTooltip.readSharedItemTooltip(bad,item,player,1000),null); assert.equal(reads,2);
  }
  assert.equal(sharedTooltip.readSharedItemTooltip({getMir2ItemTooltipDocument:()=>{throw Error("fake getter");}},item,player,1000),null);
  assert.equal(sharedTooltip.readSharedItemTooltip(runtime,{...item,uniqueId:Number.MAX_SAFE_INTEGER+1},player,1000),null);
  assert.equal(sharedTooltip.readSharedItemTooltip(runtime,item,{level:NaN},1000),null); assert.equal(calls,3);
});

function reserveCollectBlockingSocial(page, owner) {
  const proof=page.scope.socialItemOperationsRef.current.reserve({surface:"guild",kind:"guild2",from:3,to:5,
    uniqueId:66,ownCharacterIndex:7,beforeSnapshotRevision:1,localSourceKey:"bank-current",owner});
  assert.ok(proof); return proof;
}
check("Page collectParcel actual ingress and final claim reject every live reciprocal item reservation", () => {
  const blockers=[
    page=>reserveCollectBlockingSocial(page,{...socialOwner,socket:page.socket}),
    page=>{page.scope.equipmentControllerRef.current={status:()=>({pending:1})};},
    page=>{page.scope.pendingStorageRequestsRef.current.set("st-pending",{enteredSocket:true});},
    page=>{assert.ok(page.scope.storageRentalRef.current.open({owner:{...socialOwner,socket:page.socket},serviceRevision:1,
      hasExpandedStorage:false,expiryObservation:0,storageSize:80,gold:1_000_000},false));},
    page=>{page.scope.npcBuyDispatcherRef.current={status:()=>({flight:{entered:true}})};},
    page=>{page.scope.mailParcelRef.current={state:{blockedUniqueIds:[66]}};},
    page=>{page.scope.mailDispatcherRef.current={composer:{pending:owner=>{
      assert.equal(owner.playerObjectId,socialOwner.playerObjectId); assert.equal(owner.sessionGeneration,socialOwner.sessionGeneration);
      return {entered:true};}}};},
  ];
  const command={type:"collectParcel",mailId:7}, options={mailProof:{commandType:"collectParcel"}};
  for (const [index,block] of blockers.entries()) {
    const early=parityPageFixture({...socialOwner,socket:{}}); block(early);
    assert.equal(early.api.parityIngress(command,options),false,"initial guard "+index);
    assert.equal(early.sent.length,0); assert.equal(early.scope.mailCollectBarrierRef.current,null);
    const later=parityPageFixture({...socialOwner,socket:{}}); assert.equal(later.api.parityIngress(command,options),true);
    assert.equal(later.api.parityFinal(command,options,later.socket,()=>block(later)),false,"listener guard "+index);
    assert.equal(later.sent.length,0); assert.equal(later.scope.mailCollectBarrierRef.current,null);
    assert.equal(later.api.parityCollectClaim(command,options),false,"actual final claim also repeats guard "+index);
    assert.equal(later.scope.mailCollectBarrierRef.current,null);
  }
});
check("Page explicit collect with empty pending sends once while unknown and window closure retain existing barriers", () => {
  const owner={...socialOwner,socket:{}}, page=parityPageFixture(owner);
  const command={type:"collectParcel",mailId:7}, options={mailProof:{commandType:"collectParcel"}};
  assert.equal(page.api.parityIngress(command,options),true);
  page.socket.send=()=>{throw Error("unknown collection send");};
  assert.throws(()=>page.api.parityFinal(command,options,page.socket),/unknown collection send/);
  const collected=page.scope.mailCollectBarrierRef.current; assert.equal(collected.mailId,7); assert.equal(collected.observedSnapshot,null);
  page.scope.socialItemWindowsRef.current.guild=false; page.scope.heroManagementOpenRef.current=false;
  assert.equal(page.api.parityIngress(command,options),false); assert.equal(page.api.parityFinal(command,options,page.socket),false);
  assert.strictEqual(page.scope.mailCollectBarrierRef.current,collected);
  const pending=parityPageFixture(owner), proof=reserveCollectBlockingSocial(pending,owner);
  assert.equal(pending.scope.socialItemOperationsRef.current.claim(proof,owner,"bank-current",socialOperations.socialWindowItemCommand(proof)),true);
  assert.equal(pending.scope.socialItemOperationsRef.current.outcomeUnknown(proof),true);
  pending.scope.socialItemWindowsRef.current.guild=false;
  assert.equal(pending.api.parityIngress(command,options),false); assert.strictEqual(pending.scope.socialItemOperationsRef.current.pending.proof,proof);
  const fresh=parityPageFixture({...owner,socket:{}}); assert.equal(fresh.api.parityIngress(command,options),true);
  assert.equal(fresh.api.parityFinal(command,options,fresh.socket),true); assert.deepEqual(fresh.sent,[command]);
});
check("Shared exact catalog getter binds runtime and getter identity and bounded cache never invents template fields", () => {
  let reads=0; const raw=heroInfoFixture(6), runtime={getMir2ItemCatalogInfo(json){
    reads++; assert.strictEqual(this,runtime); assert.deepEqual(JSON.parse(json),{version:1,itemIndex:6});
    return JSON.stringify({version:1,ok:true,itemInfo:raw});}};
  assert.equal(Object.keys(raw).length,32);
  const first=sharedTooltip.readSharedItemCatalogInfo(runtime,6); assert.ok(first); assert.deepEqual(first.raw,raw);
  assert.equal(first.itemIndex,6); assert.equal(first.name,raw.name); assert.equal(first.icon,raw.image);
  assert.equal(Object.isFrozen(first.raw.stats),true); assert.strictEqual(sharedTooltip.readSharedItemCatalogInfo(runtime,6),first); assert.equal(reads,1);
  runtime.getMir2ItemCatalogInfo=()=>{reads++; return JSON.stringify({version:1,ok:true,itemInfo:heroInfoFixture(6,{name:"Replacement"})});};
  assert.equal(sharedTooltip.readSharedItemCatalogInfo(runtime,6).name,"Replacement"); assert.equal(reads,2);
  let boundedReads=0; const bounded={getMir2ItemCatalogInfo(json){boundedReads++; const {itemIndex}=JSON.parse(json);
    return JSON.stringify({version:1,ok:true,itemInfo:heroInfoFixture(itemIndex)});}};
  for (let index=0;index<513;index++) assert.ok(sharedTooltip.readSharedItemCatalogInfo(bounded,index));
  sharedTooltip.readSharedItemCatalogInfo(bounded,512); assert.equal(boundedReads,513);
  sharedTooltip.readSharedItemCatalogInfo(bounded,0); assert.equal(boundedReads,514,"oldest entry evicted after 512 real templates");
});
check("Shared catalog malformed index schema protocol unsafe stats getter throw and null are never cached authority", () => {
  for (const alter of [raw=>raw.item_index=7,raw=>delete raw.can_awakening,raw=>raw.extra=true,
    raw=>raw.item_type=256,raw=>raw.shape=-32769,raw=>raw.stats=[{stat:1,value:536870912}],
    raw=>raw.stats=[{stat:1,value:1,extra:true}],raw=>raw.name="x\0y"]) {
    let calls=0; const raw=heroInfoFixture(6); alter(raw);
    const runtime={getMir2ItemCatalogInfo:()=>{calls++;return JSON.stringify({version:1,ok:true,itemInfo:raw});}};
    assert.equal(sharedTooltip.readSharedItemCatalogInfo(runtime,6),null); assert.equal(sharedTooltip.readSharedItemCatalogInfo(runtime,6),null); assert.equal(calls,2);
  }
  for (const response of ["broken JSON",JSON.stringify({version:2,ok:true,itemInfo:heroInfoFixture(6)}),
    JSON.stringify({version:1,ok:false,itemInfo:heroInfoFixture(6)}),JSON.stringify({version:1,ok:true,itemInfo:null})])
    assert.equal(sharedTooltip.readSharedItemCatalogInfo({getMir2ItemCatalogInfo:()=>response},6),null);
  assert.equal(sharedTooltip.readSharedItemCatalogInfo({getMir2ItemCatalogInfo:()=>{throw Error("fake getter");}},6),null);
  let calls=0; const runtime={getMir2ItemCatalogInfo:()=>{calls++;return "";}};
  for (const index of [-1,1.5,2147483648,NaN,"6"]) assert.equal(sharedTooltip.readSharedItemCatalogInfo(runtime,index),null);
  assert.equal(calls,0);
});
function captureTooltipRequests(page) {
  const requests=[]; page.scope.runtimeRef.current={getMir2ItemTooltipDocument(json){requests.push(JSON.parse(json));
    return JSON.stringify({version:1,ok:true,document:{broken:false,sourceComplete:true,sections:[{kind:"name",lines:[{text:"Common",colour:"white"}]}]}});}};
  return requests;
}
check("Page actual Hero viewer and Cash catalog preview bind current source and never turn preview into an owned instance", () => {
  const hero=heroPageFixture(), heroRequests=captureTooltipRequests(hero), selected=hero.model.inventory[2];
  hero.scope.worldRef.current.entities=[{objectId:hero.owner.playerObjectId,level:99,classKey:"Wizard",genderKey:"Female"}];
  assert.ok(hero.api.readHeroItemTooltip(selected)); const request=heroRequests[0];
  assert.equal(request.item.uniqueId,502); assert.deepEqual(request.item.tooltipSource,selected.tooltipSource);
  assert.equal(request.player.level,20); assert.equal(request.player.className,"Warrior"); assert.equal(request.player.gender,"Male");
  assert.equal(hero.api.readHeroItemTooltip({...selected,slot:3}),null); assert.equal(hero.api.readHeroItemTooltip({...selected,count:2}),null);
  hero.scope.heroManagementOpenRef.current=false; assert.equal(hero.api.readHeroItemTooltip(selected),null); assert.equal(heroRequests.length,1);
  const bag=heroPageFixture(), bagRequests=captureTooltipRequests(bag), personal=bag.model.personalInventory[3];
  assert.ok(personal);
  bag.scope.worldRef.current.entities=[{objectId:bag.owner.playerObjectId,level:99,classKey:"Wizard",genderKey:"Female"}];
  bag.scope.worldRef.current.playerCrystalStats=[{stat:1,value:99}];
  bag.scope.worldRef.current.playerWeights={bag:10,wear:11,hand:12};
  bag.scope.worldRef.current.currentWeight=3; bag.scope.worldRef.current.maxWeight=99;
  assert.ok(bag.api.readHeroItemTooltip(personal));
  assert.equal(bagRequests[0].item.uniqueId,601); assert.deepEqual(bagRequests[0].item.tooltipSource,personal.tooltipSource);
  assert.deepEqual(bagRequests[0].player,{level:99,className:"Wizard",gender:"Female",crystalStats:[{stat:1,value:99}],
    weights:{bag:10,wear:11,hand:12},currentWeightKnown:true,currentWeight:3,maxWeight:99});
  for (const changed of [{...personal,slot:4},{...personal,count:2},{...personal,itemIndex:12},
    {...personal,name:"Old name"},{...personal,icon:2},
    {...personal,tooltipSource:{...personal.tooltipSource,userItem:{...personal.userItem,count:2}}}])
    assert.equal(bag.api.readHeroItemTooltip(changed),null,"personal Bag preview must match its complete current source");
  bag.world.inventoryItems[0].quantity=2; bag.world.inventoryItems[0].tooltipSource.userItem.count=2;
  bag.authority.receiveSnapshot(bag.world,bag.owner);
  assert.equal(bag.api.readHeroItemTooltip(personal),null,"new authoritative quantity retires the rendered personal source");
  const currentPersonal=bag.api.currentHeroModel().personalInventory[3]; assert.equal(currentPersonal.count,2);
  bag.world.inventoryItems.push({...bag.world.inventoryItems[0],slot:4}); bag.authority.receiveSnapshot(bag.world,bag.owner);
  assert.equal(bag.api.currentHeroModel().personalInventory,null,"duplicate personal UID cannot supply a unique current cell");
  assert.equal(bag.api.readHeroItemTooltip(currentPersonal),null); assert.equal(bagRequests.length,1);
  const f=cashFixture({item:{info:heroInfoFixture(23)}}), cash=parityPageFixture(f.owner,{gold:500,credit:100,
    entities:[{objectId:f.owner.playerObjectId,level:30,classKey:"Warrior",genderKey:"Male"}]}), cashRequests=captureTooltipRequests(cash);
  cash.scope.cashCatalogRef.current={owner:f.owner,catalog:f.catalog,revision:1}; const entry=cash.api.currentCashGameShopSource().entries[0];
  assert.ok(cash.api.readCashGameShopItemTooltip(entry)); const preview=cashRequests[0].item;
  assert.equal(preview.sourceKind,"catalogPreview"); assert.equal(preview.uniqueId,0); assert.equal(preview.itemIndex,23);
  assert.deepEqual(preview.tooltipSource,{info:entry.info}); assert.equal(Object.hasOwn(preview.tooltipSource,"userItem"),false);
  assert.equal(cash.api.readCashGameShopItemTooltip({...entry}),null); cash.scope.cashCatalogRef.current=null;
  assert.equal(cash.api.readCashGameShopItemTooltip(entry),null); assert.equal(cashRequests.length,1);
});
check("Page actual Guild instance tooltip binds complete projected grid current raw UID slot count and exact original UserItem", () => {
  const owner={...socialOwner,socket:{}}, page=parityPageFixture(owner,{gold:0,inventoryCapacity:46,maxBagSlots:40,
    inventoryItems:[],beltItems:[],equipmentItems:[],stage5Systems:{guild:{name:"Guild"}},
    entities:[{objectId:owner.playerObjectId,level:30,classKey:"Warrior",genderKey:"Male"}]}), requests=captureTooltipRequests(page);
  const items=Array(112).fill(null); items[3]={item:heroItemFixture(66,6,2),userId:7};
  page.scope.guildStorageRawRef.current={owner,guildName:"Guild",items,revision:1};
  page.scope.guildPermissionsRef.current={owner,guildName:"Guild",permissions:["CanRetrieveItem"]};
  page.scope.runtimeRef.current.getMir2ItemCatalogInfo=json=>JSON.stringify({version:1,ok:true,itemInfo:heroInfoFixture(JSON.parse(json).itemIndex)});
  const source=page.api.readSocialItemSurface("guild"); assert.ok(source); const selected=page.api.guildStorageTooltipItem(source.slots[3]);
  assert.ok(page.api.readGuildStorageItemTooltip(selected)); const request=requests[0].item;
  assert.equal(request.sourceKind,"catalogInstance"); assert.equal(request.uniqueId,66); assert.equal(request.itemIndex,6);
  assert.deepEqual(request.tooltipSource,{userItem:items[3].item}); assert.equal(request.count,2);
  assert.equal(page.api.readGuildStorageItemTooltip({...selected,userItem:{...selected.userItem}}),null);
  assert.equal(page.api.readGuildStorageItemTooltip({...selected,uniqueId:67}),null);
  assert.equal(page.api.readGuildStorageItemTooltip({...selected,count:1}),null);
  items[5]=items[3]; items[3]=null; assert.equal(page.api.readGuildStorageItemTooltip(selected),null);
  const moved=page.api.guildStorageTooltipItem(page.api.readSocialItemSurface("guild").slots[5]);
  page.scope.socialItemWindowsRef.current.guild=false; assert.equal(page.api.readGuildStorageItemTooltip(moved),null);
  page.scope.socialItemWindowsRef.current.guild=true; items[5].item.unique_id=Number.MAX_SAFE_INTEGER+1;
  assert.equal(page.api.readGuildStorageItemTooltip(moved),null); assert.equal(requests.length,1);
});


// ---------------------------------------------------------------------------
// Native Crystal Options / KeyboardLayoutDialog / HelpDialog pure contracts.
// The production module is loaded by the existing bounded harness; defaults
// come directly from the Native source JSON, never from a fixture copy.
// ---------------------------------------------------------------------------
const nativeKeyboardDefaults = JSON.parse(readFileSync(
  new URL("../../game-client/client-bevy/src/crystal_ui/keyboard_defaults.json", import.meta.url), "utf8",
).replace(/^\uFEFF/, ""));
const playerPreferences = loadTypeScriptModule(new URL("../lib/player-ui-preferences.ts", import.meta.url), {
  "../../game-client/client-bevy/src/crystal_ui/keyboard_defaults.json": { default: nativeKeyboardDefaults },
});
const noKeyModifiers = { alt: false, ctrl: false, shift: false, tilde: false };
function nativePreferenceBinding(functionId) {
  const binding = playerPreferences.defaultCrystalKeyBindings().find(row => row.function === functionId);
  assert.ok(binding, "real Native function exists: " + functionId);
  return binding;
}
check("Player keyboard defaults preserve all 96 exact Native rows metadata and declaration order without sharing mutations", () => {
  const first = playerPreferences.defaultCrystalKeyBindings();
  assert.equal(first.length, 96); assert.deepEqual(first, nativeKeyboardDefaults);
  assert.deepEqual([...new Set(first.map(row => row.group))], ["Dialogs", "Skillbar", "Belt", "General", "Toggle", "Combat"]);
  assert.equal(first.find(row => row.function === "Inventory").key, "F9");
  assert.equal(first.find(row => row.function === "Inventory2").key, "I");
  assert.equal(first.find(row => row.function === "Guilds").ctrl, 0);
  assert.equal(first.find(row => row.function === "GameShop").key, "Y");
  assert.equal(first.find(row => row.function === "Keybind").key, "U");
  first[0].key = "None"; first[0].description = "invented label";
  assert.deepEqual(playerPreferences.defaultCrystalKeyBindings(), nativeKeyboardDefaults);
});
check("Player keyboard matching preserves intentional Tab multi action source order and tri-state modifiers", () => {
  const bindings = playerPreferences.defaultCrystalKeyBindings();
  assert.deepEqual(playerPreferences.matchingCrystalKeyFunctions(bindings, "Tab", noKeyModifiers), ["Pickup", "DropView"]);
  assert.deepEqual(playerPreferences.matchingCrystalKeyFunctions(bindings, "Tab", { alt: true, ctrl: true, shift: true, tilde: true }), ["Pickup", "DropView"]);
  assert.equal(playerPreferences.crystalKeyBindingMatches(nativePreferenceBinding("Inventory"), "F9", { alt: true, ctrl: true, shift: true, tilde: true }), true);
  assert.equal(playerPreferences.crystalKeyBindingMatches(nativePreferenceBinding("Inventory2"), "I", { ...noKeyModifiers, ctrl: true }), false);
  assert.equal(playerPreferences.crystalKeyBindingMatches(nativePreferenceBinding("HeroInventory"), "I", { ...noKeyModifiers, ctrl: true }), true);
  assert.equal(playerPreferences.crystalKeyBindingMatches(nativePreferenceBinding("Quests"), "Q", { ...noKeyModifiers, alt: true }), false);
  assert.equal(playerPreferences.crystalKeyBindingMatches(nativePreferenceBinding("AttackmodeAll"), "None", noKeyModifiers), false);
});
check("Player keyboard Ctrl Insert exclusion follows Native even after rebinding and only for the exact modifier chord", () => {
  const camera = nativePreferenceBinding("Cameramode");
  assert.equal(playerPreferences.crystalKeyBindingMatches(camera, "Insert", noKeyModifiers), true);
  assert.equal(playerPreferences.crystalKeyBindingMatches(camera, "Insert", { ...noKeyModifiers, ctrl: true }), false);
  assert.equal(playerPreferences.crystalKeyBindingMatches(camera, "Insert", { ...noKeyModifiers, ctrl: true, alt: true }), true);
  assert.equal(playerPreferences.crystalKeyBindingMatches(camera, "Insert", { ...noKeyModifiers, ctrl: true, shift: true }), true);
  assert.equal(playerPreferences.crystalKeyBindingMatches({ ...camera, function: "Inventory" }, "Insert", { ...noKeyModifiers, ctrl: true }), false);
});
check("Player browser key adapter prefers logical ASCII and preserves physical Numpad digit OEM and Native letter fallback", () => {
  const name = playerPreferences.crystalKeyNameFromBrowserEvent;
  assert.equal(name({ key: "i", code: "KeyQ" }), "I", "logical layout letter overrides the physical Q");
  assert.equal(name({ key: "1", code: "Numpad1" }), "NumPad1");
  assert.equal(name({ key: "!", code: "Digit1" }), "D1");
  assert.equal(name({ key: "é", code: "KeyE" }), "E", "non-ASCII logical character uses Native physical KeyE fallback");
  assert.equal(name({ key: "Dead", code: "KeyQ" }), "Q");
  assert.equal(name({ key: "~", code: "Backquote" }), "Oem8");
  assert.equal(name({ key: "Enter", code: "NumpadEnter" }), "Return");
  assert.equal(name({ key: "Control", code: "ControlRight" }), "ControlKey");
  assert.equal(name({ key: "F24", code: "F24" }), "F24");
  assert.equal(name({ key: "F25", code: "F25" }), null);
  assert.equal(name({ key: "Unidentified", code: "Unidentified" }), null);
});
check("Player rebind capture Delete clears all modifier rules Escape is assignable and modifier-only input remains waiting", () => {
  const defaults = playerPreferences.defaultCrystalKeyBindings(), before = structuredClone(defaults);
  const clear = playerPreferences.captureCrystalKeyBinding(defaults, "Inventory2", "Delete", { alt: true, ctrl: true, shift: true, tilde: true });
  assert.ok(clear); assert.deepEqual(clear.find(row => row.function === "Inventory2"), { ...nativePreferenceBinding("Inventory2"), key: "None", alt: 2, ctrl: 2, shift: 2, tilde: 2 });
  const escape = playerPreferences.captureCrystalKeyBinding(defaults, "Inventory2", "Escape", noKeyModifiers);
  assert.ok(escape); assert.equal(escape.find(row => row.function === "Inventory2").key, "Escape");
  assert.deepEqual(playerPreferences.clearCrystalKeyBinding(defaults, "Inventory2"), clear);
  for (const key of ["None", "ControlKey", "Menu", "ShiftKey", "Oem8", "", "A B", "x".repeat(33)])
    assert.equal(playerPreferences.captureCrystalKeyBinding(defaults, "Inventory2", key, noKeyModifiers), null);
  assert.equal(playerPreferences.captureCrystalKeyBinding(defaults, "NotANativeFunction", "F1", noKeyModifiers), null);
  assert.deepEqual(defaults, before, "capture never mutates the current source");
});
check("Player rebind Enforce uses exact 0 versus unconstrained 2 and preserves intentional overlapping actions", () => {
  const defaults = playerPreferences.defaultCrystalKeyBindings(), modifiers = { ...noKeyModifiers, ctrl: true };
  const enforced = playerPreferences.captureCrystalKeyBinding(defaults, "Inventory2", "F3", modifiers, true);
  const free = playerPreferences.captureCrystalKeyBinding(defaults, "Inventory2", "F3", modifiers, false);
  assert.ok(enforced); assert.ok(free);
  const a = enforced.find(row => row.function === "Inventory2"), b = free.find(row => row.function === "Inventory2");
  assert.deepEqual([a.alt, a.ctrl, a.shift, a.tilde], [0, 1, 0, 0]);
  assert.deepEqual([b.alt, b.ctrl, b.shift, b.tilde], [2, 1, 2, 2]);
  assert.equal(playerPreferences.displayCrystalKeyBinding(a), "Ctrl + F3");
  assert.equal(playerPreferences.crystalKeyBindingMatches(a, "F3", { ...modifiers, shift: true }), false);
  assert.equal(playerPreferences.crystalKeyBindingMatches(b, "F3", { ...modifiers, shift: true }), true);
  const overlap = playerPreferences.captureCrystalKeyBinding(defaults, "Inventory2", "Tab", noKeyModifiers, false);
  assert.deepEqual(playerPreferences.matchingCrystalKeyFunctions(overlap, "Tab", noKeyModifiers), ["Inventory2", "Pickup", "DropView"]);
  assert.equal(playerPreferences.displayCrystalKeyBinding({ ...a, key: "None" }), "");
});
check("Player binding persistence roundtrip merges partial files with canonical Native metadata and ordering", () => {
  const edited = playerPreferences.captureCrystalKeyBinding(playerPreferences.defaultCrystalKeyBindings(), "Inventory2", "Escape", noKeyModifiers);
  const json = playerPreferences.serializeCrystalKeyBindings(edited); assert.equal(typeof json, "string");
  assert.deepEqual(playerPreferences.parseCrystalKeyBindings(json), edited);
  const saved = { ...nativePreferenceBinding("Inventory2"), key: "Home", group: "Injected", description: "Changed" };
  const loaded = playerPreferences.parseCrystalKeyBindings(JSON.stringify([saved])); assert.ok(loaded);
  assert.equal(loaded.length, 96); assert.equal(loaded.find(row => row.function === "Inventory2").key, "Home");
  assert.equal(loaded.find(row => row.function === "Inventory2").group, "Dialogs");
  assert.equal(loaded.find(row => row.function === "Inventory2").description, nativePreferenceBinding("Inventory2").description);
  assert.deepEqual(loaded.map(row => row.function), nativeKeyboardDefaults.map(row => row.function));
  assert.deepEqual(playerPreferences.parseCrystalKeyBindings("[]"), nativeKeyboardDefaults);
});
check("Player binding persistence rejects invalid unknown duplicate unsafe modifier and oversized input atomically", () => {
  const row = nativePreferenceBinding("Inventory2");
  const malformed = [null, {}, [row, row], [{ ...row, function: "Unknown" }], [{ ...row, key: "" }],
    [{ ...row, key: "Alt + Q" }], [{ ...row, key: "x".repeat(33) }], [{ ...row, key: "é" }],
    [{ ...row, ctrl: 3 }], [{ ...row, alt: -1 }], [{ ...row, shift: true }], [{ ...row, tilde: 1.5 }],
    [{ ...row, description: null }]];
  for (const value of malformed) assert.equal(playerPreferences.parseCrystalKeyBindings(JSON.stringify(value)), null);
  assert.equal(playerPreferences.parseCrystalKeyBindings("broken json"), null);
  assert.equal(playerPreferences.parseCrystalKeyBindings(" ".repeat(65537)), null);
  assert.equal(playerPreferences.serializeCrystalKeyBindings([{ ...row, ctrl: 99 }]), null);
  assert.deepEqual(playerPreferences.defaultCrystalKeyBindings(), nativeKeyboardDefaults, "invalid loads never poison Native defaults");
});
check("Player SkillMode only changes enabled Bar1 Bar2 rows with existing explicit Ctrl or tilde requirements", () => {
  const source = playerPreferences.defaultCrystalKeyBindings(), set = (id, patch) => Object.assign(source.find(row => row.function === id), patch);
  set("Bar2Skill1", { key: "Home", alt: 1, shift: 2, ctrl: 1, tilde: 0 });
  set("Bar2Skill2", { key: "None", ctrl: 1, tilde: 0 });
  set("Bar2Skill3", { ctrl: 2, tilde: 2 });
  set("Bar1Skill2", { ctrl: 1, tilde: 1 });
  set("HeroSkill1", { ctrl: 1, tilde: 0 }); set("Inventory2", { ctrl: 1, tilde: 0 });
  const before = structuredClone(source), tilde = playerPreferences.applyCrystalSkillMode(source, true);
  assert.deepEqual(source, before); assert.equal(tilde.length, 96);
  for (const row of before) {
    const actual = tilde.find(candidate => candidate.function === row.function);
    const affected = /^Bar[12]Skill[1-8]$/.test(row.function) && row.key !== "None" && (row.ctrl === 1 || row.tilde === 1);
    assert.deepEqual(actual, affected ? { ...row, ctrl: 0, tilde: 1 } : row, row.function);
  }
  const control = playerPreferences.applyCrystalSkillMode(tilde, false), custom = control.find(row => row.function === "Bar2Skill1");
  assert.deepEqual([custom.key, custom.alt, custom.shift, custom.ctrl, custom.tilde], ["Home", 1, 2, 1, 0]);
  assert.deepEqual(control.find(row => row.function === "HeroSkill1"), before.find(row => row.function === "HeroSkill1"));
  assert.deepEqual(control.find(row => row.function === "Bar2Skill2"), before.find(row => row.function === "Bar2Skill2"));
});
check("Player preferences use exact seven Native defaults strict persisted schema and real audio gain mapping", () => {
  const expected = { skillMode: false, skillBar: true, effect: true, dropView: true, nameView: true, hpView: true, newMove: false,
    musicEnabled: true, musicVolume: 80, soundEnabled: true, soundVolume: 80 };
  assert.deepEqual(playerPreferences.CRYSTAL_OPTION_KEYS, ["skillMode", "skillBar", "effect", "dropView", "nameView", "hpView", "newMove"]);
  assert.deepEqual(playerPreferences.DEFAULT_PLAYER_UI_PREFERENCES, expected);
  assert.deepEqual(playerPreferences.parsePlayerUiPreferences(playerPreferences.serializePlayerUiPreferences(expected)), expected);
  assert.deepEqual(playerPreferences.playerUiPreferencesAudioSettings(expected), { musicEnabled: true, effectsEnabled: true, musicVolume: 0.8, effectsVolume: 0.8 });
  assert.deepEqual(playerPreferences.playerUiPreferencesAudioSettings({ ...expected, musicEnabled: false, soundEnabled: false, musicVolume: 0, soundVolume: 100 }),
    { musicEnabled: false, effectsEnabled: false, musicVolume: 0, effectsVolume: 1 });
  for (const patch of [{ musicVolume: -1 }, { soundVolume: 101 }, { musicVolume: 1.5 }, { skillMode: 1 }, { newMove: "true" }, { extra: true }]) {
    const prefs = { ...expected, ...patch };
    assert.equal(playerPreferences.serializePlayerUiPreferences(prefs), null);
    assert.equal(playerPreferences.parsePlayerUiPreferences(JSON.stringify({ version: 1, preferences: prefs })), null);
  }
  const missing = { ...expected }; delete missing.skillBar;
  for (const value of [{ version: 2, preferences: expected }, { version: 1, preferences: missing }, { version: 1, preferences: expected, extra: true }, null])
    assert.equal(playerPreferences.parsePlayerUiPreferences(JSON.stringify(value)), null);
  assert.equal(playerPreferences.parsePlayerUiPreferences(" ".repeat(2049)), null);
});
check("Player Help has 45 cyclic pages Native drag limits and independent position preserved through persistence and page steps", () => {
  assert.equal(playerPreferences.CRYSTAL_HELP_PAGE_COUNT, 45);
  assert.deepEqual(playerPreferences.DEFAULT_CRYSTAL_HELP_STATE, { page: 0, left: 244, top: 129 });
  assert.equal(playerPreferences.stepCrystalHelpPage(0, -1), 44); assert.equal(playerPreferences.stepCrystalHelpPage(44, 1), 0);
  for (let page = 0; page < 45; page++) assert.equal(playerPreferences.stepCrystalHelpPage(playerPreferences.stepCrystalHelpPage(page, 1), -1), page);
  assert.equal(playerPreferences.crystalHelpPage(99), 44); assert.equal(playerPreferences.crystalHelpPage(-1), 0);
  assert.equal(playerPreferences.crystalHelpPage(3.9), 3); assert.equal(playerPreferences.crystalHelpPage(NaN), 0);
  assert.deepEqual(playerPreferences.clampCrystalHelpPosition({ left: 999, top: -20 }), { left: 487, top: 0 });
  assert.deepEqual(playerPreferences.clampCrystalHelpPosition({ left: NaN, top: Infinity }), { left: 244, top: 129 });
  const retained = { page: 44, left: 381.25, top: 201.5 };
  const saved = playerPreferences.serializeCrystalHelpState(retained), restored = playerPreferences.parseCrystalHelpState(saved);
  assert.deepEqual(restored, retained);
  const next = { ...restored, page: playerPreferences.stepCrystalHelpPage(restored.page, 1) };
  assert.deepEqual(next, { page: 0, left: 381.25, top: 201.5 });
  for (const patch of [{ page: 45 }, { page: -1 }, { page: 1.5 }, { left: 488 }, { top: 259 }, { left: null }, { extra: true }])
    assert.equal(playerPreferences.parseCrystalHelpState(JSON.stringify({ version: 1, ...retained, ...patch })), null);
  assert.equal(playerPreferences.parseCrystalHelpState(" ".repeat(1025)), null);
});


const nativeSkillBarIconMetadata = JSON.parse(readFileSync(new URL("../public/original-ui/MagIcon/meta.json", import.meta.url), "utf8"));
// JSX emit is enabled only for this production component. No hooks, JSX or UI
// are called: these checks use the exported helpers/gesture that the UI uses.
const skillBarSource = readFileSync(new URL("../app/components/original-client-skill-bars.tsx", import.meta.url), "utf8");
const skillBarCompiled = ts.transpileModule(skillBarSource, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, strict: true, jsx: ts.JsxEmit.ReactJSX },
  fileName: "original-client-skill-bars.tsx",
});
const skillBarModule = { exports: {} };
const skillBarRequires = { react: {}, "react/jsx-runtime": {},
  "../../public/original-ui/MagIcon/meta.json": { default: nativeSkillBarIconMetadata } };
new Function("exports", "module", "require", skillBarCompiled.outputText)(skillBarModule.exports, skillBarModule, specifier => {
  if (!(specifier in skillBarRequires)) throw Error("Unexpected skill bar dependency " + specifier);
  return skillBarRequires[specifier];
});
const skillBars = skillBarModule.exports;
function barLease(patch = {}) { return { slot: 9, skillKey: "FireBall", sourceKey: "physical-owner/model-3", cursor: [50, 60], ...patch }; }
check("Native skill bar inclusive nine boundary shows both bars while actionable slots remain exactly 1 through 16", () => {
  const skills = [{ key: "FireBall", name: "Fire Ball", hotkey: 9, icon: 1 }];
  assert.equal(skillBars.crystalSkillBarHasSkill(skills, 0), true); assert.equal(skillBars.crystalSkillBarHasSkill(skills, 1), true);
  assert.strictEqual(skillBars.crystalSkillBarSkillForSlot(skills, 9), skills[0]);
  assert.equal(skillBars.crystalSkillBarSkillForSlot(skills, 1), null, "never fill slots from list order");
  assert.equal(skillBars.crystalSkillBarHasSkill([{ ...skills[0], hotkey: 17 }], 0), false);
  assert.equal(skillBars.crystalSkillBarHasSkill([{ ...skills[0], hotkey: 17 }], 1), true, "keep Native display boundary");
  for (const slot of [0, 17, 1.5, NaN]) assert.equal(skillBars.crystalSkillBarSkillForSlot(skills, slot), null);
  assert.equal(skillBars.crystalSkillBarHasSkill([{ ...skills[0], hotkey: "9" }], 1), false);
});
check("Native skill bar explicit slot resolution rejects duplicate source keys hotkeys and never invents identity", () => {
  const skill = { key: "FireBall", name: "Fire Ball", hotkey: 9 };
  assert.equal(skillBars.crystalSkillBarSkillForSlot([skill, { ...skill, key: "Healing" }], 9), null);
  assert.equal(skillBars.crystalSkillBarSkillForSlot([skill, { ...skill, hotkey: 10 }], 9), null);
  assert.equal(skillBars.crystalSkillBarSkillForSlot([{ ...skill, key: "" }], 9), null);
  assert.equal(skillBars.crystalSkillBarSkillForSlot([{ ...skill, key: "x".repeat(129) }], 9), null);
  assert.strictEqual(skillBars.crystalSkillBarSkillForSlot([null, skill], 9), skill);
  assert.equal(skillBars.crystalSkillBarHasSkill([null], 0), false);
});
check("Native skill bar icon lookup uses exact MagIcon even index and measured metadata with no guessed sizes", () => {
  const skill = { key: "FireBall", name: "Fire Ball", hotkey: 9, icon: 1 };
  const native = nativeSkillBarIconMetadata.frames.find(frame => frame.index === 2);
  assert.deepEqual(skillBars.crystalSkillBarIconForSkill(skill), { index: 2, width: native.width, height: native.height });
  assert.deepEqual(skillBars.crystalSkillBarIconForSkill({ ...skill, icon: 0 }), { index: 0, width: 24, height: 22 });
  for (const icon of [undefined, -1, 1.5, 256, 255, "1", NaN])
    assert.equal(skillBars.crystalSkillBarIconForSkill({ ...skill, icon }), null);
  assert.equal(skillBars.crystalSkillBarIconForSkill(null), null);
});
check("Native skill bar lease requires same source same slot same skill same pointer and exactly one same-cell release", () => {
  const gesture = new skillBars.CrystalSkillBarGesture(), source = barLease();
  assert.equal(gesture.arm(8, source), true); assert.equal(gesture.isHeld(), true);
  assert.equal(gesture.arm(9, barLease()), false, "one active UI gesture");
  source.slot = 10; source.cursor[0] = 999;
  assert.equal(gesture.release(9, "physical-owner/model-3", 9, "FireBall", true, false), null);
  assert.equal(gesture.isHeld(), true, "other pointer cannot release the active gesture");
  const lease = gesture.release(8, "physical-owner/model-3", 9, "FireBall", true, false);
  assert.deepEqual(lease, barLease()); assert.equal(Object.isFrozen(lease), true); assert.equal(Object.isFrozen(lease.cursor), true);
  assert.equal(gesture.isHeld(), false);
  assert.equal(gesture.release(8, "physical-owner/model-3", 9, "FireBall", true, false), null);
  assert.equal(gesture.arm(10, barLease()), true);
  assert.deepEqual(gesture.release(10, "physical-owner/model-3", 9, "FireBall", true, false, [51, 61]), barLease({cursor:[51,61]}), "Native queues the current release cursor");
});
check("Native skill bar changed source disabled hidden modal focus cancellation and outside release cannot cast successor", () => {
  for (const [source, slot, skill, inside, blocked] of [
    ["physical-owner/model-4",9,"FireBall",true,false], [null,9,"FireBall",true,false],
    ["physical-owner/model-3",10,"FireBall",true,false], ["physical-owner/model-3",9,"Healing",true,false],
    ["physical-owner/model-3",9,"FireBall",false,false], ["physical-owner/model-3",9,"FireBall",true,true],
  ]) {
    const gesture = new skillBars.CrystalSkillBarGesture(); assert.equal(gesture.arm(8,barLease()),true);
    assert.equal(gesture.release(8,source,slot,skill,inside,blocked),null); assert.equal(gesture.isHeld(),false);
  }
  const gesture = new skillBars.CrystalSkillBarGesture(); assert.equal(gesture.arm(8,barLease()),true);
  gesture.cancel(); assert.equal(gesture.release(8,"physical-owner/model-3",9,"FireBall",true,false),null);
  assert.equal(gesture.arm(9,barLease({sourceKey:"physical-owner/model-4"})),true);
  assert.equal(gesture.release(8,"physical-owner/model-3",9,"FireBall",true,false),null); assert.equal(gesture.isHeld(),true);
  assert.ok(gesture.release(9,"physical-owner/model-4",9,"FireBall",true,false));
});
check("Native skill bar malformed leases cannot arm and keyboard activation retains the same strict lease checks", () => {
  for (const patch of [{slot:0},{slot:17},{slot:1.5},{skillKey:""},{skillKey:"x".repeat(129)},
    {sourceKey:""},{sourceKey:null},{cursor:[NaN,1]},{cursor:[1,2,3]}]) {
    const gesture = new skillBars.CrystalSkillBarGesture(); assert.equal(gesture.arm(8,barLease(patch)),false); assert.equal(gesture.isHeld(),false);
  }
  const gesture = new skillBars.CrystalSkillBarGesture();
  assert.equal(gesture.arm(-2,barLease()),false); assert.equal(gesture.arm(1.5,barLease()),false);
  assert.equal(gesture.arm(-1,barLease({cursor:null})),true);
  assert.deepEqual(gesture.release(-1,"physical-owner/model-3",9,"FireBall",true,false),barLease({cursor:null}));
});
check("Native skill bar positions retain both source origins and use native rounded clamp bounds without mutating source", () => {
  const positions = [[-10,745],[807.6,739.4]], before = structuredClone(positions);
  assert.deepEqual(skillBars.clampCrystalSkillBarPositions(positions),[[0,740],[808,739]]); assert.deepEqual(positions,before);
  assert.deepEqual(skillBars.clampCrystalSkillBarPositions([[0,0],[216,0]]),[[0,0],[216,0]]);
  assert.deepEqual(skillBars.clampCrystalSkillBarPositions([[NaN,Infinity],[808,740]]),[[0,0],[808,740]]);
});
check("Native skill bar cooldown uses only verified units and real delay with exact common frame arithmetic", () => {
  const skill = {key:"FireBall",name:"Fire Ball",hotkey:9,delayMs:2200,cooldownRemainingTicks:1000};
  assert.equal(skillBars.crystalSkillBarCooldownFrame(skill,1),1272);
  assert.equal(skillBars.crystalSkillBarCooldownFrame({...skill,cooldownRemainingTicks:20},50),1272);
  assert.equal(skillBars.crystalSkillBarCooldownFrame({...skill,cooldownRemainingTicks:2200},1),1260);
  assert.equal(skillBars.crystalSkillBarCooldownFrame(skill),null,"unknown carrier unit does not invent a cooldown frame");
  for (const patch of [{delayMs:undefined},{cooldownRemainingTicks:undefined},{delayMs:21},{delayMs:NaN},
    {cooldownRemainingTicks:99},{cooldownRemainingTicks:-1},{cooldownRemainingTicks:1.5},{cooldownRemainingTicks:NaN}])
    assert.equal(skillBars.crystalSkillBarCooldownFrame({...skill,...patch},1),null);
  for (const unit of [0,-1,NaN,Infinity]) assert.equal(skillBars.crystalSkillBarCooldownFrame(skill,unit),null);
});


// ---------------------------------------------------------------------------
// Source07 read-only common character_stats document and Native action identity.
// ---------------------------------------------------------------------------
const characterStatsUi = loadTypeScriptModule(new URL("../lib/shared-character-stats.ts", import.meta.url));
const crystalShortcuts = loadTypeScriptModule(new URL("../lib/crystal-shortcut-actions.ts", import.meta.url), {
  "./shared-combat-mode-keys": combatModeKeys,
});
function characterStatsPlayerFixture(patch = {}) {
  return { hp: 80, mp: 40, level: 20, experience: 500, maxExperience: 1000,
    crystalStats: [{ stat: 1, value: 20 }, { stat: 2, value: 12 }], weights: { bag: 100, wear: 30, hand: 10 }, ...patch };
}
function characterStatsResponseFixture(statePage, patch = {}) {
  return { version: 1, ok: true, statePage, rows: Array.from({ length: statePage ? 12 : 13 }, (_, index) =>
    ({ text: "Actual Rust row " + index, top: 110 + index * 18 })), ...patch };
}
check("Shared character stats sends exact version player context and requested page to the actual read-only getter", () => {
  const player = characterStatsPlayerFixture(), before = structuredClone(player), calls = [];
  const runtime = { getMir2CharacterStatsDocument(json) { const query = JSON.parse(json); calls.push(query);
    return JSON.stringify(characterStatsResponseFixture(query.statePage)); } };
  for (const statePage of [false, true]) {
    const rows = characterStatsUi.readSharedCharacterStats(runtime, player, statePage);
    assert.ok(rows); assert.equal(rows.length, statePage ? 12 : 13);
    assert.deepEqual(rows, characterStatsResponseFixture(statePage).rows);
    assert.deepEqual(calls.at(-1), { version: 1, player, statePage });
    assert.deepEqual(Object.keys(calls.at(-1)).sort(), ["player", "statePage", "version"]);
  }
  assert.equal(calls.length, 2); assert.deepEqual(player, before, "query never mutates or defaults the viewer context");
  const noWeights = characterStatsPlayerFixture({ weights: null });
  characterStatsUi.readSharedCharacterStats(runtime, noWeights, true);
  assert.equal(calls.at(-1).player.weights, null, "unknown weights remain null rather than invented capacities");
});
check("Shared character stats returns immutable rows and each query reflects the current viewer without cross-owner reuse", () => {
  let serial = 0;
  const runtime = { getMir2CharacterStatsDocument(json) { const { player, statePage } = JSON.parse(json);
    const result = characterStatsResponseFixture(statePage); result.rows[0].text = player.level + "/" + (++serial); return JSON.stringify(result); } };
  const first = characterStatsUi.readSharedCharacterStats(runtime, characterStatsPlayerFixture(), true);
  const second = characterStatsUi.readSharedCharacterStats(runtime, characterStatsPlayerFixture({ level: 99, hp: 1 }), true);
  assert.ok(first); assert.ok(second); assert.equal(first[0].text, "20/1"); assert.equal(second[0].text, "99/2");
  assert.notStrictEqual(first, second); assert.equal(Object.isFrozen(first), true);
  assert.ok(first.every(Object.isFrozen)); assert.throws(() => { first[0].text = "stale rewrite"; }, TypeError);
  assert.throws(() => { first.push({ text: "extra", top: 900 }); }, TypeError);
});
check("Shared character stats rejects wrong ABI root fields error responses page mismatches and exact row count errors", () => {
  const player = characterStatsPlayerFixture();
  for (const statePage of [false, true]) {
    const count = statePage ? 12 : 13;
    const candidates = [null, [], {}, { version: 1, ok: false, error: "missing level" },
      characterStatsResponseFixture(statePage, { version: 2 }), characterStatsResponseFixture(statePage, { ok: false }),
      characterStatsResponseFixture(statePage, { statePage: !statePage }), characterStatsResponseFixture(statePage, { extra: true }),
      characterStatsResponseFixture(statePage, { rows: [] }), characterStatsResponseFixture(statePage, { rows: {} }),
      characterStatsResponseFixture(statePage, { rows: Array.from({ length: count - 1 }, (_, index) => ({ text: "x", top: 110 + index * 18 })) }),
      characterStatsResponseFixture(statePage, { rows: Array.from({ length: count + 1 }, (_, index) => ({ text: "x", top: 110 + index * 18 })) })];
    for (const value of candidates) assert.equal(characterStatsUi.readSharedCharacterStats({ getMir2CharacterStatsDocument: () => JSON.stringify(value) }, player, statePage), null);
  }
});
check("Shared character stats validates every exact row field top and text bound without truncating or fabricating values", () => {
  const player = characterStatsPlayerFixture();
  for (const alter of [row => { row.top++; }, row => { row.top = "110"; }, row => { row.text = 1; },
    row => { row.text = null; }, row => { row.text = "x".repeat(257); }, row => { row.extra = true; },
    row => { delete row.top; }, row => { delete row.text; }]) {
    const result = characterStatsResponseFixture(false); alter(result.rows[0]);
    assert.equal(characterStatsUi.readSharedCharacterStats({ getMir2CharacterStatsDocument: () => JSON.stringify(result) }, player, false), null);
  }
  for (const badRow of [null, [], 42, "string"]) {
    const result = characterStatsResponseFixture(true); result.rows[7] = badRow;
    assert.equal(characterStatsUi.readSharedCharacterStats({ getMir2CharacterStatsDocument: () => JSON.stringify(result) }, player, true), null);
  }
  const bounded = characterStatsResponseFixture(true); bounded.rows[0].text = "x".repeat(256); bounded.rows[1].text = "";
  const rows = characterStatsUi.readSharedCharacterStats({ getMir2CharacterStatsDocument: () => JSON.stringify(bounded) }, player, true);
  assert.ok(rows); assert.equal(rows[0].text.length, 256); assert.equal(rows[1].text, "");
});
check("Shared character stats bounds input output and handles absent throwing malformed and changing runtime getters", () => {
  const player = characterStatsPlayerFixture();
  for (const runtime of [null, {}, { getMir2CharacterStatsDocument: null }, { getMir2CharacterStatsDocument: 1 },
    { getMir2CharacterStatsDocument: () => { throw Error("readonly getter failed"); } },
    { getMir2CharacterStatsDocument: () => "invalid JSON" }, { getMir2CharacterStatsDocument: () => ({ version: 1 }) },
    { getMir2CharacterStatsDocument: () => " ".repeat(16385) }])
    assert.equal(characterStatsUi.readSharedCharacterStats(runtime, player, false), null);
  let calls = 0; const runtime = { getMir2CharacterStatsDocument: () => { calls++; return JSON.stringify(characterStatsResponseFixture(false)); } };
  for (const page of [null, 0, 1, "false", undefined]) assert.equal(characterStatsUi.readSharedCharacterStats(runtime, player, page), null);
  const oversized = characterStatsPlayerFixture({ crystalStats: Array.from({ length: 20000 }, () => ({ stat: 1, value: 1 })) });
  assert.equal(characterStatsUi.readSharedCharacterStats(runtime, oversized, false), null);
  const circular = characterStatsPlayerFixture(); circular.extra = circular;
  assert.equal(characterStatsUi.readSharedCharacterStats(runtime, circular, false), null); assert.equal(calls, 0);
  assert.ok(characterStatsUi.readSharedCharacterStats(runtime, player, false)); assert.equal(calls, 1);
  runtime.getMir2CharacterStatsDocument = () => JSON.stringify(characterStatsResponseFixture(true));
  assert.equal(characterStatsUi.readSharedCharacterStats(runtime, player, false), null, "new getter wrong-page response cannot reuse old rows");
});
check("Native shortcut window identities use exact declared function names and reject prototype spelling and unknown actions", () => {
  const expected = {
    Inventory: "inventory", Inventory2: "inventory", Equipment: "equipment", Equipment2: "equipment",
    Skills: "skills", Skills2: "skills", HeroInventory: "heroInventory", HeroEquipment: "heroEquipment", HeroSkills: "heroSkills",
    Creature: "creatures", Mentor: "bonds", Relationship: "bonds", Friends: "friends", Guilds: "guild", GameShop: "gameShop",
    Quests: "quests", Options: "options", Options2: "options", Group: "group", Belt: "belt", BeltFlip: "beltFlip",
    Minimap: "minimap", Bigmap: "bigmap", Ranking: "ranking", Help: "help", Keybind: "keybind", Closeall: "closeAll", Skillbar: "skillBar",
  };
  assert.deepEqual(crystalShortcuts.CRYSTAL_WINDOW_SHORTCUTS, expected);
  for (const [functionId, action] of Object.entries(expected)) {
    assert.ok(nativeKeyboardDefaults.some(row => row.function === functionId));
    assert.equal(crystalShortcuts.crystalWindowShortcut(functionId), action);
  }
  for (const functionId of ["", "inventory", "Inventory3", "HeroSkill1", "Bar1Skill1", "Belt1", "MountWindow", "constructor", "toString", "__proto__", "hasOwnProperty"])
    assert.equal(crystalShortcuts.crystalWindowShortcut(functionId), null);
});
check("Native belt shortcut slots preserve both physical number and Numpad Alt identities with exact zero-based 0 through 7 bounds", () => {
  for (let slot = 1; slot <= 8; slot++) {
    for (const functionId of ["Belt" + slot, "Belt" + slot + "Alt"]) {
      assert.ok(nativeKeyboardDefaults.some(row => row.function === functionId));
      assert.equal(crystalShortcuts.crystalBeltShortcutSlot(functionId), slot - 1);
    }
  }
  for (const functionId of ["Belt", "BeltFlip", "Belt0", "Belt9", "Belt01", "Belt1alt", "Belt1AltExtra", "belt1", "Bar1Skill1", "Belt1\n"])
    assert.equal(crystalShortcuts.crystalBeltShortcutSlot(functionId), null);
});
check("Native skill shortcut slots preserve Bar1 1 through 8 Bar2 9 through 16 and never map Hero or display index fallback", () => {
  for (let bar = 1; bar <= 2; bar++) for (let cell = 1; cell <= 8; cell++) {
    const functionId = "Bar" + bar + "Skill" + cell;
    assert.ok(nativeKeyboardDefaults.some(row => row.function === functionId));
    assert.equal(crystalShortcuts.crystalSkillShortcutSlot(functionId), (bar - 1) * 8 + cell);
  }
  for (const functionId of ["Bar0Skill1", "Bar3Skill1", "Bar1Skill0", "Bar2Skill9", "Bar1Skill01", "bar1Skill1", "Bar1Skill1Alt", "HeroSkill1", "F1", "1", "Bar1Skill1\n"])
    assert.equal(crystalShortcuts.crystalSkillShortcutSlot(functionId), null);
});
check("Native supported browser function inventory is immutable unique and each identity exists in the source default catalog", () => {
  const functions = crystalShortcuts.SUPPORTED_CRYSTAL_BROWSER_FUNCTIONS;
  assert.equal(Object.isFrozen(functions), true); assert.equal(new Set(functions).size, functions.length);
  for (const functionId of functions) assert.ok(nativeKeyboardDefaults.some(row => row.function === functionId), functionId);
  for (const functionId of Object.keys(crystalShortcuts.CRYSTAL_WINDOW_SHORTCUTS)) assert.ok(functions.includes(functionId));
  for (let index = 1; index <= 8; index++) {
    assert.ok(functions.includes("Bar1Skill" + index)); assert.ok(functions.includes("Bar2Skill" + index));
    assert.ok(functions.includes("Belt" + index)); assert.ok(functions.includes("Belt" + index + "Alt"));
  }
  for (const functionId of ["Logout", "Pickup", "Trade", "AddGroupMember", "DropView"])
    assert.ok(functions.includes(functionId));
  for (const functionId of ["ChangeAttackmode", "ChangePetmode", "AttackmodePeace", "AttackmodeGroup",
    "AttackmodeGuild", "AttackmodeEnemyguild", "AttackmodeRedbrown", "AttackmodeAll", "PetmodeBoth",
    "PetmodeMoveonly", "PetmodeAttackonly", "PetmodeNone", "PetmodeFocusMasterTarget"])
    assert.equal(functions.includes(functionId), true, "mode actions use the shared Native reducer and final browser claim");
  assert.equal(functions.includes("HeroSkill1"), false, "player slot helper does not grant unsupported Hero manual casting");
  assert.equal(functions.includes("Unknown"), false);
});


// Production document reader + persistence helpers; the getter below is an
// in-memory JSON facade only, never a renderer or cast authority.
function skillBarRaw(key = "fire-ball", extra = {}) {
  return {key, name:"Fire Ball", description:"Actual raw", spell:"FireBall", level:1, hotkey:1,
    mpCost:7, delayMs:2200, castTimeMs:-100, cooldownRemainingTicks:0, ...extra};
}
function skillBarMemoryGetter(rows, inspect) {
  return {getMir2SkillBarDocument(input) {
    inspect?.(JSON.parse(input));
    return JSON.stringify({version:1,ok:true,rows});
  }};
}
check("Shared skill bar document preserves exact complete raw order true key and zero metadata without granting cast identity", () => {
  const learned=[skillBarRaw("fire",{hotkey:9,mpCost:0,delayMs:0,icon:0}),skillBarRaw("other",{hotkey:0,icon:null,mpCost:null,delayMs:null})];
  const output=[{key:"fire",name:"Fire Ball",hotkey:9,mpCost:0,delayMs:0,icon:0,cooldownRemainingTicks:0},
    {key:"other",name:"Fire Ball",hotkey:0,cooldownRemainingTicks:0}];
  let calls=0;
  const rows=skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter(output,input=>{
    calls++; assert.deepEqual(input,{version:1,learned});
  }),learned);
  assert.equal(calls,1); assert.deepEqual(rows,output); assert.equal(Object.isFrozen(rows),true);
  assert.equal(Object.isFrozen(rows[0]),true); assert.equal(Object.isFrozen(rows[1]),true);
  assert.equal(rows[0].key,"fire"); assert.equal(rows[1].key,"other");
  for(const row of rows) for(const field of ["id","rawIndex","requestId","spell","sourceKey","owner"])
    assert.equal(Object.hasOwn(row,field),false);
});
check("Shared skill bar document admits the complete512 boundary but never a truncated compacted or malformed learned list", () => {
  const learned=Array.from({length:512},(_,i)=>({key:"key"+i,name:"Skill",hotkey:0}));
  const rows=skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter(learned),learned);
  assert.equal(rows.length,512); assert.equal(rows[511].key,"key511");
  assert.deepEqual(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([]),[]),[]);
  assert.equal(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter(learned),[...learned,{key:"extra",name:"Extra"}]),null);
  assert.equal(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter(learned.slice(1)),learned),null);
  const sparse=Array(2); sparse[1]={key:"real",name:"Actual"};
  assert.equal(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([{key:"fake",name:"Fake"},sparse[1]]),sparse),null);
  for(const bad of [null,undefined,{},"learned",[null],[7],[{}],[{key:"x"}],[{name:"Actual"}]])
    assert.equal(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([{key:"x",name:"Actual"}]),bad),null);
});
check("Shared skill bar document rejects changed raw identity duplicate keys duplicate castable slots and extra result authority", () => {
  const raw=skillBarRaw(); const good={key:raw.key,name:raw.name,hotkey:1,mpCost:7,delayMs:2200,cooldownRemainingTicks:0};
  for(const change of [{key:"other"},{name:"Other"},{id:0},{spell:"FireBall"},{requestId:1},{hotkey:2},{mpCost:8},{delayMs:2201},{cooldownRemainingTicks:1}])
    assert.equal(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([{...good,...change}]),[raw]),null);
  assert.equal(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([good,good]),[raw,raw]),null);
  const second=skillBarRaw("other");
  assert.equal(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([good,{...good,key:"other"}]),[raw,second]),null);
  const unset=[{key:"a",name:"A",hotkey:0},{key:"b",name:"B",hotkey:0}];
  assert.deepEqual(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter(unset),unset),unset);
  for (const hotkey of [17,24,255]) {
    const displayOnly=[{key:"a",name:"A",hotkey},{key:"b",name:"B",hotkey}];
    assert.deepEqual(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter(displayOnly),displayOnly),displayOnly,
      "noncastable presentation bindings do not discard the complete model");
  }
});
check("Shared skill bar document allows only catalog derived icon while preserving explicit unknown optional source fields", () => {
  const raw={key:"catalog",name:"Actual",spell:"FireBall"};
  assert.deepEqual(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([{key:raw.key,name:raw.name,icon:1}]),[raw]),
    [{key:raw.key,name:raw.name,icon:1}]);
  assert.deepEqual(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([{key:raw.key,name:raw.name}]),[raw]),
    [{key:raw.key,name:raw.name}],"unknown catalog is not a fabricated icon");
  for(const field of ["hotkey","mpCost","delayMs","cooldownRemainingTicks"])
    assert.equal(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([{key:raw.key,name:raw.name,[field]:1}]),[raw]),null,field);
  for(const field of ["icon","hotkey","mpCost","delayMs","cooldownRemainingTicks"]) {
    const unknown={...raw,[field]:null};
    assert.ok(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([{key:raw.key,name:raw.name}]),[unknown]));
    assert.equal(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([{key:raw.key,name:raw.name,[field]:0}]),[unknown]),null,field);
  }
});
check("Shared skill bar document keeps legal Native hotkey17 and productionu8 maximum without converting either into an active16 slot", () => {
  for(const hotkey of [0,1,16,17,24,255]) {
    const raw={key:"actual",name:"Actual",hotkey};
    const result=skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([raw]),[raw]);
    assert.ok(result,hotkey); assert.equal(result[0].hotkey,hotkey);
    assert.equal(crystalShortcuts.crystalSkillShortcutSlot("Bar2Skill9"),null,"document metadata does not expand player shortcut slots");
  }
  for (const hotkey of [-1,256,1.5,NaN,Infinity]) {
    const raw={key:"actual",name:"Actual",hotkey};
    assert.equal(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([raw]),[raw]),null,
      "Native raw i32 does not expand the production u8 document contract");
  }
});
check("Shared skill bar document rejects unsafe numeric result fields invalid envelopes oversized text and getter exceptions", () => {
  const raw={key:"actual",name:"Actual"};
  for(const row of [{...raw,icon:256},{...raw,icon:-1},{...raw,icon:NaN},{...raw,icon:Infinity},
    {...raw,icon:1.5},{...raw,mpCost:4294967296},{...raw,delayMs:-1},{...raw,cooldownRemainingTicks:4294967296},
    {...raw,name:""},{...raw,key:""},{...raw,key:"x".repeat(129)},{...raw,name:"x".repeat(257)}])
    assert.equal(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([row]),[raw]),null);
  for(const value of [null,[],{version:1,ok:false,rows:[]},{version:2,ok:true,rows:[raw]},
    {version:1,ok:true,rows:[raw],authority:true},{version:1,ok:"true",rows:[raw]}, {version:1,ok:true,rows:null}])
    assert.equal(skillBarDocument.readSharedSkillBarDocument({getMir2SkillBarDocument:()=>JSON.stringify(value)},[raw]),null);
  for(const value of ["not JSON"," ".repeat(262145),17,undefined])
    assert.equal(skillBarDocument.readSharedSkillBarDocument({getMir2SkillBarDocument:()=>value},[raw]),null);
  assert.equal(skillBarDocument.readSharedSkillBarDocument({getMir2SkillBarDocument(){throw Error("fake getter failure");}},[raw]),null);
  assert.equal(skillBarDocument.readSharedSkillBarDocument(null,[raw]),null);
  assert.equal(skillBarDocument.readSharedSkillBarDocument({},[raw]),null);
  const cyclic={...raw}; cyclic.self=cyclic;
  assert.equal(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter([raw]),[cyclic]),null);
});
check("Shared skill bar document rejects synchronous getter source replacement and rereads a later current source instead of caching stale rows", () => {
  const raw=skillBarRaw(); const learned=[raw];
  const output={key:raw.key,name:raw.name,hotkey:1,mpCost:7,delayMs:2200,cooldownRemainingTicks:0,icon:1};
  const replace={getMir2SkillBarDocument(){learned[0]={...raw,spell:"Teleport"}; return JSON.stringify({version:1,ok:true,rows:[output]});}};
  assert.equal(skillBarDocument.readSharedSkillBarDocument(replace,learned),null,"same key/name cannot relabel an old catalog icon after source changes");
  let calls=0;
  const getter={getMir2SkillBarDocument(input){calls++;const current=JSON.parse(input).learned[0];
    return JSON.stringify({version:1,ok:true,rows:[{key:current.key,name:current.name,hotkey:current.hotkey,mpCost:current.mpCost,delayMs:current.delayMs,cooldownRemainingTicks:current.cooldownRemainingTicks}]});}};
  const source=[skillBarRaw()];const first=skillBarDocument.readSharedSkillBarDocument(getter,source);
  source[0]={...source[0],delayMs:3300}; const second=skillBarDocument.readSharedSkillBarDocument(getter,source);
  assert.equal(calls,2); assert.equal(first[0].delayMs,2200); assert.equal(second[0].delayMs,3300);
  assert.equal(skillBarDocument.readSharedSkillBarDocument(skillBarMemoryGetter(first),source),null);
});
check("Two skill bar positions use exact authored bounds independent persistent points and frozen complete256 character envelope", () => {
  assert.equal(skillBarDocument.CRYSTAL_SKILL_BAR_POSITIONS_STORAGE_KEY,"mir2.crystalSkillBars.v1");
  assert.deepEqual(skillBarDocument.DEFAULT_CRYSTAL_SKILL_BAR_POSITIONS,[[0,0],[216,0]]);
  const points=[[0,740],[808,0]],text=skillBarDocument.serializeCrystalSkillBarPositions(points);
  assert.deepEqual(JSON.parse(text),{version:1,positions:points});
  const parsed=skillBarDocument.parseCrystalSkillBarPositions(text);assert.deepEqual(parsed,points);
  assert.equal(Object.isFrozen(parsed),true); assert.equal(Object.isFrozen(parsed[0]),true); assert.equal(Object.isFrozen(parsed[1]),true);
  points[0][0]=42; assert.equal(parsed[0][0],0);
  const exact=text+" ".repeat(256-text.length); assert.equal(exact.length,256);
  assert.deepEqual(skillBarDocument.parseCrystalSkillBarPositions(exact),parsed);
  assert.equal(skillBarDocument.parseCrystalSkillBarPositions(exact+" "),null);
});
check("Two skill bar positions reject invalid persistence atomically without rounding clamping or converting unknown values to zero", () => {
  for(const points of [null,[],[[0,0]],[[0,0],[0,0],[0,0]],[[0,0,0],[0,0]],[[0,0],[0]],
    [[-1,0],[0,0]],[[809,0],[0,0]],[[0,741],[0,0]],[[0,-1],[0,0]],
    [[.5,0],[0,0]],[[NaN,0],[0,0]],[[Infinity,0],[0,0]],[[null,0],[0,0]],[["0",0],[0,0]]]) {
    assert.equal(skillBarDocument.serializeCrystalSkillBarPositions(points),null);
    assert.equal(skillBarDocument.parseCrystalSkillBarPositions(JSON.stringify({version:1,positions:points})),null);
  }
  for(const value of [null,[],{version:2,positions:[[0,0],[0,0]]},{version:1,positions:[[0,0],[0,0]],extra:true},
    {version:1,positions:[[0,0],[0,0]],position:[0,0]}, {positions:[[0,0],[0,0]]}])
    assert.equal(skillBarDocument.parseCrystalSkillBarPositions(JSON.stringify(value)),null);
  for(const text of ["not JSON",null,undefined,17]) assert.equal(skillBarDocument.parseCrystalSkillBarPositions(text),null);
});


// Transpile only actual pure production declarations; no component, hook, DOM,
// renderer, source module import or factory is invoked by this extraction.
function loadPureProductionDeclarations(url, names) {
  const text = readFileSync(url, "utf8");
  const ast = ts.createSourceFile(fileURLToPath(url), text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const declarations = ast.statements.filter(node => (ts.isFunctionDeclaration(node) || ts.isClassDeclaration(node))
    && node.name && names.includes(node.name.text));
  for (const name of names) assert.equal(declarations.filter(node => node.name.text === name).length, 1, "sole actual pure declaration " + name);
  const compiled = ts.transpileModule(declarations.map(node => node.getText(ast)).join("\n"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, strict: true },
  }).outputText;
  const module = { exports: {} };
  new Function("exports", "module", compiled)(module.exports, module);
  return module.exports;
}
const heroWindowUi = loadPureProductionDeclarations(new URL("../app/components/original-client-hero-management-window.tsx", import.meta.url),
  ["heroUiLeaseCurrent", "captureHeroUiLease", "HeroWindowDrag"]);
const hudHealthLabels = loadPureProductionDeclarations(new URL("../app/components/original-client-overlays.tsx", import.meta.url),
  ["crystalMainHudHealthLabels"]);
check("Hero surface leases retain original epoch across close reopen page and actor changes and never borrow belt", () => {
  const windows={inventoryOpen:true,characterOpen:true,characterPage:"equipment",beltVisible:true,beltVertical:false};
  const epochs={inventory:3,character:7,belt:11};
  const inventory=heroWindowUi.captureHeroUiLease("inventory",windows,epochs);
  const character=heroWindowUi.captureHeroUiLease("character",windows,epochs);
  const belt=heroWindowUi.captureHeroUiLease("belt",windows,epochs);
  assert.deepEqual(inventory,{origin:"inventory",epoch:3}); assert.deepEqual(character,{origin:"character",epoch:7});
  assert.deepEqual(belt,{origin:"belt",epoch:11}); assert.equal(Object.isFrozen(inventory),true);
  assert.equal(heroWindowUi.heroUiLeaseCurrent(inventory,{...windows,inventoryOpen:false},epochs),false);
  assert.equal(heroWindowUi.heroUiLeaseCurrent(inventory,windows,{...epochs,inventory:5}),false,"close then reopen does not revive the old inventory lease");
  assert.equal(heroWindowUi.heroUiLeaseCurrent(character,{...windows,characterPage:"skills"},{...epochs,character:8}),false);
  assert.equal(heroWindowUi.heroUiLeaseCurrent(belt,windows,{inventory:4,character:8,belt:12}),false,"actor replacement advances every origin");
  assert.equal(heroWindowUi.heroUiLeaseCurrent(inventory,{...windows,inventoryOpen:false,beltVisible:true},epochs),false);
  assert.equal(heroWindowUi.heroUiLeaseCurrent(character,{...windows,characterOpen:false,inventoryOpen:true},epochs),false);
  const moved={...windows,inventoryPosition:{x:200,y:100},characterPosition:{x:80,y:40},beltPosition:{x:100,y:200}};
  assert.equal(heroWindowUi.heroUiLeaseCurrent(inventory,moved,epochs),true,"presentation position changes retain the existing window epoch");
});
check("Hero legacy single management origin zero never fabricates managed or floating belt authority", () => {
  assert.deepEqual(heroWindowUi.captureHeroUiLease("inventory",undefined,undefined,"inventory"),{origin:"inventory",epoch:0});
  assert.deepEqual(heroWindowUi.captureHeroUiLease("character",undefined,undefined,"skills"),{origin:"character",epoch:0});
  assert.equal(heroWindowUi.captureHeroUiLease("belt",undefined,undefined,"inventory"),null);
  assert.equal(heroWindowUi.captureHeroUiLease("character",undefined,undefined,"inventory"),null);
  const windows={inventoryOpen:true,characterOpen:true,characterPage:"equipment",beltVisible:true,beltVertical:false};
  assert.equal(heroWindowUi.captureHeroUiLease("inventory",windows),null);
  assert.equal(heroWindowUi.captureHeroUiLease("belt",windows),null);
  for(const epoch of [-1,NaN,Infinity,1.5,Number.MAX_SAFE_INTEGER+1,undefined]) {
    assert.equal(heroWindowUi.captureHeroUiLease("inventory",windows,{inventory:epoch,character:1,belt:1}),null);
  }
  assert.equal(heroWindowUi.heroUiLeaseCurrent({origin:"unknown",epoch:1},windows,{inventory:1,character:1,belt:1}),false);
});
check("Hero drag uses captured surface and pointer lease without advancing epoch or letting a foreign pointer end it", () => {
  const windows={inventoryOpen:true,characterOpen:true,characterPage:"equipment",beltVisible:true,beltVertical:false};
  const epochs={inventory:3,character:7,belt:11};
  const lease=heroWindowUi.captureHeroUiLease("inventory",windows,epochs),drag=new heroWindowUi.HeroWindowDrag();
  assert.equal(drag.begin(4,lease,{x:120,y:80},{x:100,y:50},windows,epochs),true);
  assert.equal(drag.begin(5,lease,{x:120,y:80},{x:100,y:50},windows,epochs),false);
  assert.equal(drag.move(5,lease,{x:200,y:160},windows,epochs),null); assert.equal(drag.isHeld(),true);
  assert.equal(drag.finish(5,lease),false); assert.equal(drag.isHeld(),true);
  assert.equal(drag.move(4,{origin:"character",epoch:7},{x:200,y:160},windows,epochs),null);
  const position=drag.move(4,lease,{x:200,y:160},windows,epochs); assert.deepEqual(position,{x:180,y:130});
  assert.deepEqual(epochs,{inventory:3,character:7,belt:11});
  assert.deepEqual(drag.move(4,lease,{x:0,y:0},{...windows,inventoryPosition:position},epochs),{x:0,y:0});
  assert.equal(drag.finish(4,lease),true); assert.equal(drag.isHeld(),false);
  assert.equal(drag.move(4,lease,{x:201,y:161},windows,epochs),null);
});
check("Hero drag retires hidden closed changed-epoch and cancelled captures while late old capture cannot cancel successor", () => {
  const windows={inventoryOpen:true,characterOpen:false,characterPage:"equipment",beltVisible:true,beltVertical:false};
  const epochs={inventory:3,character:7,belt:11},lease={origin:"inventory",epoch:3};
  for(const [current,currentEpoch] of [[{...windows,inventoryOpen:false},epochs],[windows,{...epochs,inventory:4}]]) {
    const drag=new heroWindowUi.HeroWindowDrag(); assert.equal(drag.begin(4,lease,{x:10,y:20},{x:0,y:0},windows,epochs),true);
    assert.equal(drag.validate(current,currentEpoch),false); assert.equal(drag.isHeld(),false);
    assert.equal(drag.move(4,lease,{x:50,y:60},windows,epochs),null,"a later open cannot restore a retired capture");
  }
  const drag=new heroWindowUi.HeroWindowDrag();
  assert.equal(drag.begin(4,lease,{x:10,y:20},{x:0,y:0},windows,epochs),true);
  drag.cancel(); assert.equal(drag.isHeld(),false,"the blur/hidden/cleanup handler uses actual cancel");
  const next={origin:"inventory",epoch:4},nextEpoch={...epochs,inventory:4};
  assert.equal(drag.begin(4,next,{x:10,y:20},{x:0,y:0},windows,nextEpoch),true);
  assert.equal(drag.finish(4,lease),false,"late lost-capture/cancel from the old epoch must not retire the successor");
  assert.equal(drag.isHeld(),true); assert.equal(drag.finish(4,next),true);
  for(const pointerId of [-1,NaN,Infinity,1.5]) assert.equal(drag.begin(pointerId,next,{x:10,y:20},{x:0,y:0},windows,nextEpoch),false);
  assert.equal(drag.begin(4,next,{x:NaN,y:20},{x:0,y:0},windows,nextEpoch),false);
  assert.equal(drag.begin(4,next,{x:10,y:20},{x:0,y:0},windows,nextEpoch),true);
  assert.equal(drag.move(4,next,{x:Infinity,y:20},windows,nextEpoch),null); assert.equal(drag.isHeld(),false);
});
check("Native DropView uses strict now greater than deadline and repeat never extends its five second window", () => {
  const deadline=playerPreferences.crystalDropViewDeadline(null,1000); assert.equal(deadline,6000);
  for(const now of [1000,1001,5000,5999,6000]) assert.equal(playerPreferences.crystalDropViewDeadline(deadline,now),6000);
  assert.equal(playerPreferences.crystalDropViewDeadline(deadline,6001),11001);
  assert.equal(playerPreferences.crystalDropViewDeadline(0,0),0,"the exact zero deadline does not reopen");
  assert.equal(playerPreferences.crystalDropViewDeadline(0,1),5001);
  assert.equal(playerPreferences.crystalDropViewDeadline(null,0),5000);
});
check("Native DropView rejects unknown nonfinite negative and overflowing clock without inventing a new deadline", () => {
  for(const now of [NaN,Infinity,-Infinity,-1,Number.MAX_SAFE_INTEGER-4999]) assert.equal(playerPreferences.crystalDropViewDeadline(null,now),null);
  for(const until of [NaN,Infinity,-Infinity,-1,Number.MAX_SAFE_INTEGER+1]) assert.equal(playerPreferences.crystalDropViewDeadline(until,1000),null);
  assert.equal(playerPreferences.crystalDropViewDeadline(null,Number.MAX_SAFE_INTEGER-5000),Number.MAX_SAFE_INTEGER);
  assert.equal(playerPreferences.crystalDropViewDeadline(Number.MAX_SAFE_INTEGER,1000),Number.MAX_SAFE_INTEGER);
});
check("Crystal compact HP and MP labels preserve Native class level threshold exact text and authored rectangles", () => {
  const world={playerHp:0,playerMaxHp:100,playerMp:7,playerMaxMp:50};
  const hp={kind:"compactHp",text:"HP 0/100",left:0,top:673,width:100,height:14,compact:true};
  const mp={kind:"compactMp",text:"MP 7/50 ",left:0,top:688,width:100,height:14,compact:true};
  assert.deepEqual(hudHealthLabels.crystalMainHudHealthLabels({player:{classKey:"warrior",level:25},world,hpView:true}),[hp]);
  assert.deepEqual(hudHealthLabels.crystalMainHudHealthLabels({player:{classKey:"warrior",level:26},world,hpView:true}),[hp,mp]);
  assert.deepEqual(hudHealthLabels.crystalMainHudHealthLabels({player:{classKey:"wizard",level:1},world}),[hp,mp]);
  assert.deepEqual(world,{playerHp:0,playerMaxHp:100,playerMp:7,playerMaxMp:50});
});
check("Crystal alternate HP and MP labels preserve spaces line breaks two remaining authored rectangles and unknown fallback", () => {
  const world={playerHp:9,playerMaxHp:100,playerMp:7,playerMaxMp:50};
  assert.deepEqual(hudHealthLabels.crystalMainHudHealthLabels({player:{classKey:"warrior",level:26},world,hpView:false}),[
    {kind:"alternateTop",text:" 9    7 \n---------------",left:9,top:666,width:85,height:30,compact:false},
    {kind:"alternateBottom",text:" 100    50 ",left:9,top:696,width:85,height:30,compact:false},
  ]);
  assert.deepEqual(hudHealthLabels.crystalMainHudHealthLabels({player:{classKey:"warrior",level:25},world,hpView:false}),[
    {kind:"alternateTop",text:"9\n--",left:9,top:666,width:85,height:30,compact:false},
    {kind:"alternateBottom",text:"100",left:9,top:696,width:85,height:30,compact:false},
  ]);
  assert.deepEqual(hudHealthLabels.crystalMainHudHealthLabels({player:null,world:{},hpView:false}).map(row=>row.text),["0\n--","0"]);
});
check("Page Hero real final claim rejects ordinary Move ref-only exception close reopen and synchronous newly opened input owner", () => {
  const closed=heroPageFixture();
  closed.api.changeHeroWindows({...closed.scope.heroWindowsRef.current,inventoryOpen:false});
  closed.scope.heroBeltProofRef.current=closed.proof;
  assert.equal(closed.api.heroProofCurrent(closed.proof,closed.proof.wire),false);
  closed.api.changeHeroWindows({...closed.scope.heroWindowsRef.current,inventoryOpen:true});
  assert.equal(closed.api.heroProofCurrent(closed.proof,closed.proof.wire),false,"the source lease remains the original epoch");
  assert.equal(closed.api.parityFinal({...closed.proof.wire},{heroProof:closed.proof},closed.socket),false); assert.equal(closed.sent.length,0);
  for(const block of [s=>s.document.hasFocus=()=>false,s=>s.document.visibilityState="hidden",s=>s.initialSceneAssetsReadyRef.current=false,
    s=>s.bevyHpLocalOverlayOpenRef.current=true,s=>s.questReactModalOpenRef.current=true,s=>s.questLogOpenRef.current=true,
    s=>s.characterOpenRef.current=true,s=>s.spellsIngressRef.current={pointerContext:()=>({modal:true})},
    s=>s.npcShopServiceRef.current={},s=>s.npcShopUiIngressRef.current={blocksInput:()=>true},s=>s.npcRepairServiceRef.current="repair",
    s=>s.storageServiceActiveRef.current=true,s=>s.storageUiIngressRef.current={active:false,transitioning:true},
    s=>s.combatIngressRef.current={hasUiHeld:()=>true},s=>s.skillBarPointerHeldRef.current=true,
    s=>s.playerReferenceWindowsRef.current.options=true]) {
    const f=heroPageFixture(); assert.equal(f.api.parityIngress({...f.proof.wire},{heroProof:f.proof}),true);
    assert.equal(f.api.parityFinal({...f.proof.wire},{heroProof:f.proof},f.socket,()=>block(f.scope)),false);
    assert.equal(f.sent.length,0); assert.equal(f.scope.heroOperationsRef.current.pending.state,"reserved");
  }
});
check("Page accepted Hero restock enters once across closed management and unknown transport cannot be cleared or automatically replayed", () => {
  const f=heroPageFixture(),ledger=f.scope.heroOperationsRef.current;
  assert.equal(ledger.cancelDefinitelyUnsent(f.proof),true);
  const use=ledger.reserve(f.model,{kind:"use",slot:0}); assert.equal(ledger.claim(use,f.model),true);
  assert.equal(ledger.receipt("UseItem",{grid:"HeroInventory",uniqueId:501,success:true},f.owner,f.authority.authoritySerial),true);
  const model=publishHeroBeltConsumption(f); assert.equal(ledger.observe(model),true);
  const proof=ledger.reserveRestock(model); assert.ok(proof);
  f.scope.heroUiProofLeasesRef.current.set(proof,{kind:"acceptedRestock"}); f.scope.heroBeltProofRef.current=proof;
  f.api.changeHeroWindows({...f.scope.heroWindowsRef.current,inventoryOpen:false});
  assert.equal(f.api.heroProofCurrent(proof,proof.wire),true);
  let sends=0;f.socket.send=()=>{sends++;throw Error("fake uncertain restock send");};
  assert.throws(()=>f.api.parityFinal({...proof.wire},{heroProof:proof},f.socket),/fake uncertain restock send/);
  assert.equal(ledger.pending.state,"unknown"); assert.equal(ledger.cancelDefinitelyUnsent(proof),false);
  f.api.changeHeroWindows({...f.scope.heroWindowsRef.current,inventoryOpen:true});
  assert.equal(f.api.parityFinal({...proof.wire},{heroProof:proof},f.socket),false);
  assert.equal(ledger.reserveRestock(f.api.currentHeroModel()),null); assert.equal(sends,1);
  assert.equal(ledger.pending.state,"unknown"); assert.equal(ledger.retireSession({...f.owner,sceneRevision:99,mapFileName:"1"}),false);
});


// Actual TS transport around fixed Rust ABI replies. These getters do not
// reproduce Native mode selection, clocks, pending rules or quest membership.
check("Shared mode host accepts reordered owner fields and spends only the exact immutable command proof", () => {
  const owner={generation:1,connectionGeneration:2,sessionGeneration:3,playerObjectId:40,sceneRevision:4,mapFileName:"0"};
  let source={owner,revision:7,attackMode:0,petMode:1,enabled:true}, host, observed, claims=0;
  const calls=[], command={type:"changeAMode",mode:3};
  const runtime={getMir2CombatModeKeysVersion:()=>1,processMir2CombatModeKeys:json=>{
    const input=JSON.parse(json); calls.push(input);
    if(input.operation==="request") return JSON.stringify({version:1,ok:true,command:{mode:3,type:"changeAMode"},
      proof:{requestId:11,owner:Object.fromEntries(Object.entries(input.owner).reverse()),revision:input.revision,function:input.function,command}});
    if(input.operation==="claim"){claims++; return JSON.stringify({version:1,ok:true,entered:true});}
    return JSON.stringify({version:1,ok:true,observedSnapshot:true});
  }};
  host=new combatModeKeys.SharedCombatModeKeys({runtime:()=>runtime,read:()=>source,now:()=>123.75,onWire:(proof,wire)=>{
    observed=proof; assert.equal(Object.isFrozen(proof),true); assert.equal(Object.isFrozen(proof.owner),true); assert.equal(Object.isFrozen(wire),true);
    assert.equal(host.allows({...proof},wire),false,"a cloned proof has no active identity");
    for(const wrong of [{type:"changeAMode",mode:2},{type:"changePMode",mode:3},{...wire,extra:true}]) {
      assert.equal(host.allows(proof,wrong),false); assert.equal(host.claim(proof,wrong),false);
    }
    source={...source,owner:Object.fromEntries(Object.entries(owner).reverse())};
    assert.equal(host.allows(proof,{mode:3,type:"changeAMode"}),true);
    assert.equal(host.claim(proof,{mode:3,type:"changeAMode"}),true);
    assert.equal(host.claim(proof,wire),false); assert.equal(host.cancelDefinitelyUnsent(proof),false);
    return "confirmedSend";
  }});
  assert.equal(host.request("AttackmodeEnemyguild"),true); assert.equal(claims,1);
  assert.deepEqual(calls.map(row=>row.operation),["snapshot","request","claim"]);
  assert.ok(calls.every(row=>row.version===1&&row.clockMs===123));
  assert.equal(host.allows(observed,command),false); assert.equal(host.claim(observed,command),false);
});
check("Shared mode claim consumes attempted Rust entry despite synchronous modal owner or runtime changes", () => {
  for(const change of ["modal","owner","runtime"]){
    const owner={generation:1,connectionGeneration:2,sessionGeneration:3,playerObjectId:40,sceneRevision:4,mapFileName:"0"};
    let source={owner,revision:7,attackMode:0,petMode:1,enabled:true}, host, activeRuntime, enteredProof;
    const operations=[], command={type:"changePMode",mode:4};
    const runtime={getMir2CombatModeKeysVersion:()=>1,processMir2CombatModeKeys:json=>{
      const input=JSON.parse(json); operations.push(input.operation);
      if(input.operation==="request")return JSON.stringify({version:1,ok:true,command,
        proof:{requestId:12,owner:input.owner,revision:input.revision,function:input.function,command}});
      if(input.operation==="claim"){
        if(change==="modal")source={...source,enabled:false};
        else if(change==="owner")source={...source,owner:{...owner,sessionGeneration:4}};
        else activeRuntime=null;
        return JSON.stringify({version:1,ok:true,entered:true});
      }
      if(input.operation==="outcomeUnknown")return JSON.stringify({version:1,ok:true,entered:true,outcomeUnknown:true});
      return JSON.stringify({version:1,ok:true,observedSnapshot:true});
    }};
    activeRuntime=runtime;
    host=new combatModeKeys.SharedCombatModeKeys({runtime:()=>activeRuntime,read:()=>source,now:()=>200,onWire:(proof,wire)=>{
      enteredProof=proof; assert.equal(host.claim(proof,wire),false,change);
      assert.equal(host.claim(proof,wire),false,"spent claim never retries");
      assert.equal(host.cancelDefinitelyUnsent(proof),false,"entered cannot roll back even after failed final fence");
      return "definitelyUnsent";
    }});
    assert.equal(host.request("PetmodeFocusMasterTarget"),true);
    assert.equal(operations.filter(op=>op==="claim").length,1); assert.equal(operations.includes("definitelyUnsent"),false);
    assert.equal(operations.filter(op=>op==="request").length,1);
    assert.equal(operations.filter(op=>op==="outcomeUnknown").length,change==="runtime"?0:1);
    source={...source,owner,enabled:true}; activeRuntime=runtime;
    assert.equal(host.claim(enteredProof,command),false); assert.equal(operations.filter(op=>op==="request").length,1);
  }
});
check("Shared mode ABI exceptions remain spent unknown while explicit unsent replies alone allow cancellation", () => {
  for(const phase of ["claimThrow","wireThrow","unsent"]){
    const owner={generation:1,connectionGeneration:2,sessionGeneration:3,playerObjectId:40,sceneRevision:4,mapFileName:"0"};
    const source={owner,revision:7,attackMode:0,petMode:1,enabled:true}, operations=[], command={type:"changeAMode",mode:1};
    let host, proofSeen;
    const runtime={getMir2CombatModeKeysVersion:()=>1,processMir2CombatModeKeys:json=>{
      const input=JSON.parse(json); operations.push(input.operation);
      if(input.operation==="request")return JSON.stringify({version:1,ok:true,command,
        proof:{requestId:13,owner:input.owner,revision:input.revision,function:input.function,command}});
      if(input.operation==="claim"){if(phase==="claimThrow")throw Error("uncertain reducer entry");return JSON.stringify({version:1,ok:true,entered:true});}
      if(input.operation==="outcomeUnknown")return JSON.stringify({version:1,ok:true,entered:true,outcomeUnknown:true});
      if(input.operation==="definitelyUnsent")return JSON.stringify({version:1,ok:true,cancelled:true});
      return JSON.stringify({version:1,ok:true,observedSnapshot:true});
    }};
    host=new combatModeKeys.SharedCombatModeKeys({runtime:()=>runtime,read:()=>source,now:()=>301,onWire:(proof,wire)=>{
      proofSeen=proof;
      if(phase==="unsent"){assert.equal(host.cancelDefinitelyUnsent(proof),true,"actual cancelled ABI reply");return "confirmedSend";}
      assert.equal(host.claim(proof,wire),phase!=="claimThrow");
      assert.equal(host.cancelDefinitelyUnsent(proof),false);
      if(phase==="wireThrow")throw Error("uncertain transport");
      return "outcomeUnknown";
    }});
    assert.equal(host.request("ChangeAttackmode"),true); assert.equal(host.claim(proofSeen,command),false);
    assert.deepEqual(operations,phase==="unsent"?["snapshot","request","definitelyUnsent"]:["snapshot","request","claim","outcomeUnknown"]);
    assert.equal(operations.filter(op=>op==="request").length,1,"no automatic retry on exception");
  }
});
check("Shared mode receipts forward actual bounded state only and direct functions stop dispatch while cycles continue", () => {
  const owner={generation:1,connectionGeneration:2,sessionGeneration:3,playerObjectId:40,sceneRevision:4,mapFileName:"0"};
  const source={owner,revision:8,attackMode:null,petMode:null,enabled:false}, inputs=[];
  const runtime={getMir2CombatModeKeysVersion:()=>1,processMir2CombatModeKeys:json=>{inputs.push(JSON.parse(json));return JSON.stringify({version:1,ok:true,observedReceipt:true,notice:"[Mode: Peaceful]"});}};
  const host=new combatModeKeys.SharedCombatModeKeys({runtime:()=>runtime,read:()=>source,now:()=>900,onWire:()=>{throw Error("receipt cannot send");}});
  assert.equal(host.observeReceipt("ChangeAMode",5),true); assert.equal(host.observeReceipt("ChangePMode",4),true);
  assert.deepEqual(inputs.map(({operation,packet,mode,owner:actual,revision})=>({operation,packet,mode,owner:actual,revision})),[
    {operation:"receipt",packet:"ChangeAMode",mode:5,owner,revision:8},{operation:"receipt",packet:"ChangePMode",mode:4,owner,revision:8}]);
  for(const mode of [-1,1.5,NaN,Infinity,"1",6])assert.equal(host.observeReceipt("ChangeAMode",mode),false);
  assert.equal(host.observeReceipt("ChangePMode",5),false); assert.equal(inputs.length,2);
  assert.equal(host.request("AttackmodePeace"),false); assert.equal(inputs.length,2,"disabled presentation receipts grant no send");
  assert.equal(combatModeKeys.CRYSTAL_COMBAT_MODE_FUNCTIONS.length,13);
  for(const functionId of combatModeKeys.CRYSTAL_COMBAT_MODE_FUNCTIONS){
    assert.equal(combatModeKeys.isCrystalCombatModeFunction(functionId),true);
    assert.equal(combatModeKeys.crystalCombatModeStopsDispatch(functionId),!["ChangeAttackmode","ChangePetmode"].includes(functionId));
  }
  assert.equal(combatModeKeys.crystalCombatModeStopsDispatch("Unknown"),false);
  assert.equal(combatModeKeys.supportsCombatModeKeys({getMir2CombatModeKeysVersion:()=>{throw Error("getter");}}),false);
  assert.equal(combatModeKeys.supportsCombatModeKeys({getMir2CombatModeKeysVersion:()=>0,processMir2CombatModeKeys:()=>"{}"}),false);
});
check("Actual Page mode owner preserves scene generation and requires real bounded snapshot or receipt state", () => {
  const f=heroFixture(), page=parityPageFixture(f.owner,{...f.world,connected:true});
  page.scope.heroManagementOpenRef.current=false; page.scope.heroPetOpenRef.current=false;
  page.scope.cashShopOpenRef.current=false; page.scope.socialItemWindowsRef.current={guild:false,trade:false};
  page.api.captureCombatModeSnapshot({...page.scope.worldRef.current,stage5Systems:{attackMode:5,petMode:4}});
  const first=page.api.currentCombatModeOwner(); assert.ok(first);
  assert.deepEqual(page.api.readCombatModeSource(),{owner:first,revision:1,attackMode:5,petMode:4,enabled:true});
  page.scope.questSceneRevisionRef.current++; page.scope.worldRef.current.mapFileName="1";
  assert.equal(page.api.readCombatModeSource(),null,"old scene data cannot be relabeled");
  page.api.captureCombatModeSnapshot({...page.scope.worldRef.current,stage5Systems:{attackMode:6,petMode:-1}});
  const scene=page.api.currentCombatModeOwner(); assert.equal(scene.generation,first.generation);
  assert.equal(page.api.readCombatModeSource().attackMode,null); assert.equal(page.api.readCombatModeSource().petMode,null);
  page.api.captureCombatModeReceipt("ChangeAMode",3); const received=page.api.readCombatModeSource();
  assert.equal(received.attackMode,3); assert.equal(received.petMode,null);
  page.api.captureCombatModeReceipt("ChangePMode","4"); assert.deepEqual(page.api.readCombatModeSource(),received);
  page.scope.bevyHpLocalOverlayOpenRef.current=true;
  assert.equal(page.api.readCombatModeSource().enabled,false); page.scope.bevyHpLocalOverlayOpenRef.current=false;
  page.scope.equipmentSessionGenerationRef.current++; page.scope.equipmentStartGameRef.current.sessionGeneration++;
  const session=page.api.currentCombatModeOwner(); assert.ok(session.generation>first.generation);
  assert.equal(page.api.readCombatModeSource(),null);
  page.api.captureCombatModeSnapshot({...page.scope.worldRef.current,playerObjectId:41,stage5Systems:{attackMode:1,petMode:1}});
  assert.equal(page.api.readCombatModeSource(),null,"foreign actor snapshot cannot initialize modes");
});
check("Actual Page final mode claim rejects listener modal and consumed reentrant claim never sends or rolls back", () => {
  for(const phase of ["beforeClaim","duringClaim","sendThrow"]){
    const f=heroFixture(), page=parityPageFixture(f.owner,{...f.world,connected:true});
    page.scope.heroManagementOpenRef.current=false; page.scope.heroPetOpenRef.current=false;
    page.scope.cashShopOpenRef.current=false; page.scope.socialItemWindowsRef.current={guild:false,trade:false};
    page.api.captureCombatModeSnapshot({...page.scope.worldRef.current,stage5Systems:{attackMode:0,petMode:0}});
    const operations=[], command={type:"changeAMode",mode:1}; let host, proofSeen;
    const runtime={getMir2CombatModeKeysVersion:()=>1,processMir2CombatModeKeys:json=>{
      const input=JSON.parse(json); operations.push(input.operation);
      if(input.operation==="request")return JSON.stringify({version:1,ok:true,command,
        proof:{requestId:21,owner:input.owner,revision:input.revision,function:input.function,command}});
      if(input.operation==="claim"){if(phase==="duringClaim")page.scope.questReactModalOpenRef.current=true;return JSON.stringify({version:1,ok:true,entered:true});}
      if(input.operation==="definitelyUnsent")return JSON.stringify({version:1,ok:true,cancelled:true});
      if(input.operation==="outcomeUnknown")return JSON.stringify({version:1,ok:true,entered:true,outcomeUnknown:true});
      return JSON.stringify({version:1,ok:true,observedSnapshot:true});
    }};
    page.scope.runtimeRef.current=runtime;
    if(phase==="sendThrow")page.socket.send=()=>{throw Error("mode send unknown");};
    host=new combatModeKeys.SharedCombatModeKeys({runtime:()=>page.scope.runtimeRef.current,read:page.api.readCombatModeSource,now:()=>400,onWire:(proof,wire)=>{
      proofSeen=proof;
      if(phase==="sendThrow"){assert.throws(()=>page.api.parityFinal({...wire},{modeProof:proof},page.socket),/mode send unknown/);return "outcomeUnknown";}
      assert.equal(page.api.parityFinal({...wire},{modeProof:proof},page.socket,
        phase==="beforeClaim"?()=>{page.scope.heroManagementOpenRef.current=true;}:undefined),false);
      return "definitelyUnsent";
    }});
    page.scope.combatModeHostRef.current=host;
    assert.equal(host.request("ChangeAttackmode"),true); assert.equal(page.sent.length,0);
    assert.equal(operations.filter(op=>op==="claim").length,phase==="beforeClaim"?0:1);
    assert.equal(operations.filter(op=>op==="definitelyUnsent").length,phase==="beforeClaim"?1:0);
    assert.equal(operations.includes("outcomeUnknown"),phase!=="beforeClaim");
    page.scope.heroManagementOpenRef.current=false; page.scope.questReactModalOpenRef.current=false;
    assert.equal(host.claim(proofSeen,command),false); assert.equal(operations.filter(op=>op==="request").length,1);
  }
});
const questNameContext=questWorldDocument.stampBevyQuestWorldContext({generation:3,connectionGeneration:2,sessionGeneration:7,
  sceneRevision:4,playerObjectId:40,mapFileName:"0",inventory:{capacity:46,gold:0,items:[]},
  player:{className:"warrior",gender:"male",level:20,gold:0,currentWeight:0,currentWeightKnown:true,maxWeight:100},
  mapIndex:null,selectedObjectId:null,groundDrops:[],entities:[
    {objectId:40,kind:"selfPlayer",name:"Player",x:100,y:100,direction:null,level:20,dead:false,hp:100,maxHp:100},
    {objectId:44,kind:"monster",name:"Hen",x:101,y:100,direction:null,level:null,dead:false,hp:null,maxHp:null},
    {objectId:45,kind:"monster",name:"Unrelated catalog name",x:102,y:100,direction:null,level:null,dead:false,hp:null,maxHp:null},
    {objectId:46,kind:"player",name:"Hen",x:103,y:100,direction:null,level:null,dead:false,hp:null,maxHp:null},
    {objectId:47,kind:"monster",name:"Hen",x:104,y:100,direction:null,level:null,dead:true,hp:null,maxHp:null},
  ]},9);
check("Quest name getter accepts actual complete context stamp and preserves Native target membership order only", () => {
  assert.ok(questNameContext);
  const stamp=Object.fromEntries(["generation","revision","connectionGeneration","sessionGeneration","sceneRevision","playerObjectId","mapFileName"].map(key=>[key,questNameContext[key]]));
  let reply={version:1,known:true,stamp,objectIds:[45,44]}, calls=0;
  const runtime={getMir2QuestNameTargetsVersion:()=>1,getMir2QuestNameTargets:()=>{calls++;return JSON.stringify(reply);}};
  const ids=questNameDocument.readBevyQuestNameTargets(runtime,questNameContext);
  assert.deepEqual(ids,[45,44]); assert.equal(Object.isFrozen(ids),true); assert.equal(calls,1);
  reply={...reply,objectIds:[]}; assert.deepEqual(questNameDocument.readBevyQuestNameTargets(runtime,questNameContext),[]);
  reply={version:1,known:false,stamp:null,objectIds:[]}; assert.equal(questNameDocument.readBevyQuestNameTargets(runtime,questNameContext),null);
  assert.equal(questNameDocument.readBevyQuestNameTargets({getMir2QuestNameTargetsVersion:()=>0,getMir2QuestNameTargets:()=>{throw Error("unsupported getter must not run");}},questNameContext),null);
});
check("Quest name getter rejects stale full stamp foreign or dead entities duplicates malformed and exceptional replies", () => {
  const stamp=Object.fromEntries(["generation","revision","connectionGeneration","sessionGeneration","sceneRevision","playerObjectId","mapFileName"].map(key=>[key,questNameContext[key]]));
  let reply={version:1,known:true,stamp,objectIds:[44]};
  const runtime={getMir2QuestNameTargetsVersion:()=>1,getMir2QuestNameTargets:()=>JSON.stringify(reply)};
  for(const key of ["generation","revision","connectionGeneration","sessionGeneration","sceneRevision","playerObjectId","mapFileName"]){
    reply={version:1,known:true,stamp:{...stamp,[key]:key==="mapFileName"?"1":stamp[key]+1},objectIds:[44]};
    assert.equal(questNameDocument.readBevyQuestNameTargets(runtime,questNameContext),null,key);
  }
  for(const objectIds of [[46],[47],[40],[999],[44,44],[0],[-1],[NaN],[4294967296],["44"]]){
    reply={version:1,known:true,stamp,objectIds}; assert.equal(questNameDocument.readBevyQuestNameTargets(runtime,questNameContext),null);
  }
  for(const malformed of [null,[],{version:1,known:true,stamp,objectIds:[44],extra:true},
    {version:1,known:true,stamp:{...stamp,extra:true},objectIds:[44]},{version:1,known:1,stamp,objectIds:[44]}]){
    reply=malformed; assert.equal(questNameDocument.readBevyQuestNameTargets(runtime,questNameContext),null);
  }
  assert.equal(questNameDocument.readBevyQuestNameTargets({getMir2QuestNameTargetsVersion:()=>1,getMir2QuestNameTargets:()=>{throw Error("getter");}},questNameContext),null);
  assert.equal(questNameDocument.readBevyQuestNameTargets({getMir2QuestNameTargetsVersion:()=>1,getMir2QuestNameTargets:()=>"x".repeat(16385)},questNameContext),null);
  assert.equal(questNameDocument.readBevyQuestNameTargets(runtime,{...questNameContext,revision:0}),null);
});


// ---------------------------------------------------------------------------
// Shared presentation adapter boundary fixtures. These load the real TS adapter
// through the existing transpiler. WASM methods below are data mocks, not Rust
// execution, route-search coverage, pricing proof, or player/UI acceptance.
// ---------------------------------------------------------------------------
const presentationRuntime = loadTypeScriptModule(new URL("../lib/client-presentation-runtime.ts", import.meta.url));
const presentationChatDocument = {
  version:1, epoch:7, open:false, size:0, lineCount:4, frameIndex:0, countBarIndex:0,
  top:500, height:100, controlTop:600, inputTop:650, track:80, knobTop:0, index:0,
  historyCount:20, appliedMask:341, draftMask:null,
};
const presentationCashInput = { itemType:1, shape:9, requiredGender:1, armourShape:0,
  female:false, direction:2, elapsedMs:150 };
const presentationRepairInput = {
  uniqueId:41, tooltipSource:{info:{item_index:77,price:1000,durability:1000},
    userItem:{unique_id:41,item_index:77,count:9,current_dura:900,max_dura:1100,
      added_stats:[{stat:5,value:5},{stat:6,value:-3}],rental_information:null}},
  count:2,currentDura:250,maxDura:1000,rate:0.1,special:false,gold:18,
};

check("presentation map ABI forwards exact geometry and edge bytes and validates an immutable diagonal route", () => {
  const edges = new Uint8Array(9); edges[0]=8; edges[4]=8;
  let argumentsSeen;
  const module={getMir2MapRouteVersion:()=>1,getMir2MapRoutePlan:(...args)=>{argumentsSeen=args;return Int32Array.of(0,1,1,2,2);}};
  const result=presentationRuntime.searchSharedMapRoute(module,{width:3,height:3,origin:{x:0,y:0},goal:{x:2,y:2},edges});
  assert.deepEqual(argumentsSeen,[3,3,0,0,2,2,edges]); assert.equal(argumentsSeen[6],edges);
  assert.deepEqual(result,{status:"ok",steps:[{x:1,y:1},{x:2,y:2}]});
  assert.equal(Object.isFrozen(result.steps),true); assert.equal(Object.isFrozen(result.steps[0]),true);
  module.getMir2MapRoutePlan=()=>Int32Array.of(0);
  assert.deepEqual(presentationRuntime.searchSharedMapRoute(module,{width:1,height:1,origin:{x:0,y:0},goal:{x:0,y:0},edges:new Uint8Array(1)}),{status:"ok",steps:[]});
});
check("presentation map ABI accepts only odd typed responses and the three exact rejection codes", () => {
  const input={width:2,height:2,origin:{x:0,y:0},goal:{x:1,y:1},edges:Uint8Array.of(8,0,0,0)};
  let reply=Int32Array.of(1);
  const module={getMir2MapRouteVersion:()=>1,getMir2MapRoutePlan:()=>reply};
  for(const [code,status] of [[1,"outside"],[2,"budget"],[3,"unreachable"]]){
    reply=Int32Array.of(code); assert.deepEqual(presentationRuntime.searchSharedMapRoute(module,input),{status});
  }
  for(const bad of [[],[0,1,1],Float32Array.of(0,1,1),new Int32Array(),Int32Array.of(0,1),Int32Array.of(4),Int32Array.of(1,1,1),new Int32Array(500002)]){
    reply=bad; assert.throws(()=>presentationRuntime.searchSharedMapRoute(module,input),/Invalid shared route/);
  }
  assert.throws(()=>presentationRuntime.searchSharedMapRoute({},input),/unavailable/);
  assert.throws(()=>presentationRuntime.searchSharedMapRoute({...module,getMir2MapRouteVersion:()=>2},input),/unavailable/);
});
check("presentation map rejects leaps stationary points blocked source edges and a different terminal goal", () => {
  const edges=new Uint8Array(9);edges[0]=8;edges[4]=8;
  const input={width:3,height:3,origin:{x:0,y:0},goal:{x:2,y:2},edges};
  let reply;
  const module={getMir2MapRouteVersion:()=>1,getMir2MapRoutePlan:()=>reply};
  for(const points of [[0,0,0],[0,2,2],[0,1,1,3,3],[0,1,1]]){
    reply=Int32Array.from(points);assert.throws(()=>presentationRuntime.searchSharedMapRoute(module,input),/shared route/);
  }
  reply=Int32Array.of(0,1,1,2,2);edges[4]=0;
  assert.throws(()=>presentationRuntime.searchSharedMapRoute(module,input),/Invalid shared route step/);
  edges[4]=8;assert.throws(()=>presentationRuntime.searchSharedMapRoute(module,{...input,goal:{x:2,y:1}}),/Incomplete shared route/);
});
check("presentation map input rejection never calls Rust with malformed dimensions origin goal or edge shape", () => {
  let calls=0;
  const module={getMir2MapRouteVersion:()=>1,getMir2MapRoutePlan:()=>{calls++;return Int32Array.of(3);}};
  const good={width:2,height:2,origin:{x:0,y:0},goal:{x:1,y:1},edges:new Uint8Array(4)};
  for(const delta of [{width:0},{width:1.5},{width:16777217},{width:16777216,height:2},{edges:new Uint8Array(3)},
    {edges:[0,0,0,0]},{origin:null},{origin:{x:-1,y:0}},{goal:{x:2,y:1}},{goal:{x:NaN,y:1}}]){
    assert.deepEqual(presentationRuntime.searchSharedMapRoute(module,{...good,...delta}),{status:"outside"});
  }
  assert.equal(calls,0);
});

check("presentation chat strict documents reject extra keys wrong mask epoch geometry and draft state", () => {
  let raw=JSON.stringify(presentationChatDocument),free=0;
  class RawChat { document(){return raw;} free(){free++;} }
  const ui=presentationRuntime.createSharedChatUi({chat_ui_abi_version:()=>1,ChatUiBridge:RawChat},341);
  assert.deepEqual(ui.document(),presentationChatDocument);assert.equal(Object.isFrozen(ui.document()),true);
  for(const bad of [null,[],{...presentationChatDocument,extra:1},{...presentationChatDocument,version:2},
    {...presentationChatDocument,epoch:0},{...presentationChatDocument,lineCount:5},{...presentationChatDocument,size:3},
    {...presentationChatDocument,open:true},{...presentationChatDocument,draftMask:0},{...presentationChatDocument,appliedMask:1024},
    {...presentationChatDocument,historyCount:1000001},{...presentationChatDocument,index:20},
    {...presentationChatDocument,top:769},{...presentationChatDocument,height:0},{...presentationChatDocument,track:769}]){
    raw=JSON.stringify(bad);assert.throws(()=>ui.document(),/Invalid shared chat/);
  }
  raw="{";assert.throws(()=>ui.document());raw="x".repeat(2049);assert.throws(()=>ui.document(),/Invalid shared chat/);
  ui.dispose();ui.dispose();assert.equal(free,1);
});
check("presentation chat calls preserve exact action epoch and raw stale-epoch refusal", () => {
  const calls=[];let expectedEpoch=7;
  class RawChat {
    constructor(mask){calls.push(["constructor",mask]);}
    document(){return JSON.stringify({...presentationChatDocument,epoch:expectedEpoch});}
    scroll(action,epoch){calls.push(["scroll",action,epoch]);return epoch===expectedEpoch;}
    drag(y,grab,epoch){calls.push(["drag",y,grab,epoch]);return epoch===expectedEpoch;}
    resize(epoch){calls.push(["resize",epoch]);return epoch===expectedEpoch;}
    open(epoch){calls.push(["open",epoch]);return epoch===expectedEpoch;}
    edit_filter(channel,value,epoch){calls.push(["filter",channel,value,epoch]);return epoch===expectedEpoch;}
    edit_all(value,epoch){calls.push(["all",value,epoch]);return epoch===expectedEpoch;}
    edit_transparent(value,epoch){calls.push(["transparent",value,epoch]);return epoch===expectedEpoch;}
    observe(count){calls.push(["observe",count]);return count===20;}
    apply(epoch){calls.push(["apply",epoch]);return epoch===expectedEpoch?682:-1;}
    cancel(epoch){calls.push(["cancel",epoch]);return epoch===expectedEpoch;}
    defaults(epoch){calls.push(["defaults",epoch]);return epoch===expectedEpoch;}
    retire(){calls.push(["retire"]);return true;}
    restore(mask){calls.push(["restore",mask]);return mask===341;}
    free(){calls.push(["free"]);}
  }
  const ui=presentationRuntime.createSharedChatUi({chat_ui_abi_version:()=>1,ChatUiBridge:RawChat},341);
  assert.equal(ui.observe(20),true);assert.equal(ui.observe(21),false);
  for(const [name,wire] of [["home",0],["up",1],["down",2],["end",3]]){
    assert.equal(ui.scroll(name,7),true);assert.deepEqual(calls.at(-1),["scroll",wire,7]);
  }
  assert.equal(ui.drag(12.5,-2,7),true);assert.deepEqual(calls.at(-1),["drag",12.5,-2,7]);
  assert.equal(ui.editFilter(8,false,7),true);assert.deepEqual(calls.at(-1),["filter",8,false,7]);
  assert.equal(ui.editAll(false,7),true);assert.deepEqual(calls.at(-1),["all",false,7]);
  assert.equal(ui.editTransparent(true,7),true);assert.deepEqual(calls.at(-1),["transparent",true,7]);
  assert.equal(ui.resize(7),true);assert.equal(ui.open(7),true);assert.equal(ui.apply(7),682);
  assert.equal(ui.cancel(7),true);assert.equal(ui.defaults(7),true);
  assert.equal(ui.retire(),true);
  // A recorded new native epoch is supplied explicitly, not computed by a JS policy mirror.
  expectedEpoch=8;assert.equal(ui.document().epoch,8);
  assert.equal(ui.scroll("down",7),false);assert.deepEqual(calls.at(-1),["scroll",2,7]);
  assert.equal(ui.apply(7),-1);assert.equal(ui.restore(341),true);
  ui.dispose();const afterFree=calls.length;
  assert.equal(ui.observe(20),false);assert.equal(ui.scroll("down",8),false);assert.equal(ui.drag(0,0,8),false);
  assert.equal(ui.resize(8),false);assert.equal(ui.open(8),false);assert.equal(ui.editFilter(0,true,8),false);
  assert.equal(ui.editAll(true,8),false);assert.equal(ui.editTransparent(true,8),false);assert.equal(ui.apply(8),-1);
  assert.equal(ui.cancel(8),false);assert.equal(ui.defaults(8),false);assert.equal(ui.retire(),false);assert.equal(ui.restore(341),false);
  assert.throws(()=>ui.document(),/retired/);ui.dispose();assert.equal(calls.length,afterFree);
  assert.equal(calls.filter(row=>row[0]==="free").length,1);
});
check("presentation chat bounds reject invalid commands before touching the bridge and source identities are distinct", () => {
  let calls=0,constructors=0,frees=0;
  class RawChat {
    constructor(mask){constructors++;assert.equal(mask,341);}
    document(){return JSON.stringify(presentationChatDocument);}
    observe(){calls++;return false;} scroll(){calls++;return false;} drag(){calls++;return false;}
    resize(){calls++;return false;} open(){calls++;return false;} edit_filter(){calls++;return false;}
    edit_all(){calls++;return false;} edit_transparent(){calls++;return false;} apply(){calls++;return -1;}
    cancel(){calls++;return false;} defaults(){calls++;return false;} retire(){calls++;return false;}
    restore(){calls++;return false;} free(){frees++;}
  }
  const module={chat_ui_abi_version:()=>1,ChatUiBridge:RawChat};
  const one=presentationRuntime.createSharedChatUi(module,341),two=presentationRuntime.createSharedChatUi(module,341);
  assert.notEqual(one.source,two.source);assert.equal(Object.isFrozen(one.source),true);
  for(const bad of [0,-1,1.5,4294967296,NaN]){
    assert.equal(one.scroll("home",bad),false);assert.equal(one.resize(bad),false);assert.equal(one.open(bad),false);
    assert.equal(one.apply(bad),-1);assert.equal(one.cancel(bad),false);assert.equal(one.defaults(bad),false);
  }
  assert.equal(one.scroll("__proto__",7),false);assert.equal(one.drag(Infinity,0,7),false);assert.equal(one.drag(0,1000001,7),false);
  assert.equal(one.editFilter(9,true,7),false);assert.equal(one.editFilter(0,1,7),false);
  assert.equal(one.editAll("true",7),false);assert.equal(one.editTransparent(1,7),false);
  assert.equal(one.observe(1000001),false);assert.equal(one.restore(1024),false);assert.equal(calls,0);
  for(const mask of [-1,1024,0.5])assert.throws(()=>presentationRuntime.createSharedChatUi(module,mask),/Invalid chat preference/);
  assert.equal(constructors,2);one.dispose();two.dispose();assert.equal(frees,2);
  assert.throws(()=>presentationRuntime.createSharedChatUi({},341),/unavailable/);
});
check("presentation chat refuses malformed native apply replies instead of inventing committed settings", () => {
  let reply=-1;
  class RawChat { apply(){return reply;} free(){} }
  const ui=presentationRuntime.createSharedChatUi({chat_ui_abi_version:()=>1,ChatUiBridge:RawChat},341);
  assert.equal(ui.apply(7),-1);reply=0;assert.equal(ui.apply(7),0);reply=1023;assert.equal(ui.apply(7),1023);
  for(const bad of [1024,-2,0.5,NaN,true]){reply=bad;assert.throws(()=>ui.apply(7),/Invalid shared chat commit/);}
  ui.dispose();
});

check("presentation cash sends only shared layer inputs with exact bigint elapsed and preserves ordered immutable output", () => {
  let args;
  const reply={version:1,known:true,direction:2,layers:[{library:"AWeaponL/09",frame:39},{library:"CArmour/00",frame:39},{library:"AWeaponR/09",frame:39}]};
  const module={cash_preview_abi_version:()=>1,cash_preview_layers:(...values)=>{args=values;return JSON.stringify(reply);}};
  const result=presentationRuntime.readSharedCashPreview(module,presentationCashInput);
  assert.deepEqual(args,[1,9,1,0,false,2,150n]);assert.deepEqual(result,reply);
  assert.equal(Object.isFrozen(result),true);assert.equal(Object.isFrozen(result.layers),true);assert.equal(Object.isFrozen(result.layers[0]),true);
  module.cash_preview_layers=()=>JSON.stringify({version:1,known:false,direction:2,layers:[]});
  assert.deepEqual(presentationRuntime.readSharedCashPreview(module,presentationCashInput),{version:1,known:false,direction:2,layers:[]});
});
check("presentation cash rejects malformed complete or unknown documents library aliases duplicates and oversized layers", () => {
  let reply;const good={version:1,known:true,direction:2,layers:[{library:"CArmour/00",frame:39}]};
  const module={cash_preview_abi_version:()=>1,cash_preview_layers:()=>JSON.stringify(reply)};
  for(const bad of [null,[],{...good,extra:true},{...good,version:2},{...good,known:1},{...good,direction:3},
    {...good,layers:[]},{...good,known:false},{...good,layers:Array(4).fill(good.layers[0])},
    {...good,layers:[good.layers[0],good.layers[0]]},
    ...["CArmour/000","CArmour/32768","CArmour/../00","Monster/00","/CArmour/00"].map(library=>({...good,layers:[{library,frame:39}]})),
    ...[-1,65536,0.5].map(frame=>({...good,layers:[{library:"CArmour/00",frame}]})),
    {...good,layers:[{library:"CArmour/00",frame:39,extra:1}]}]){
    reply=bad;assert.equal(presentationRuntime.readSharedCashPreview(module,presentationCashInput),null);
  }
  module.cash_preview_layers=()=>"{";assert.equal(presentationRuntime.readSharedCashPreview(module,presentationCashInput),null);
  module.cash_preview_layers=()=>"x".repeat(2049);assert.equal(presentationRuntime.readSharedCashPreview(module,presentationCashInput),null);
});
check("presentation cash invalid real-domain inputs and unsupported ABI never invoke layer policy", () => {
  let calls=0;const module={cash_preview_abi_version:()=>1,cash_preview_layers:()=>{calls++;return "{}";}};
  for(const delta of [{shape:-1},{shape:32768},{itemType:256},{requiredGender:-1},{armourShape:-32769},{armourShape:32768},
    {female:1},{direction:0},{direction:9},{elapsedMs:-1},{elapsedMs:Number.MAX_SAFE_INTEGER+1},{elapsedMs:1.5}]){
    assert.equal(presentationRuntime.readSharedCashPreview(module,{...presentationCashInput,...delta}),null);
  }
  assert.equal(presentationRuntime.readSharedCashPreview({...module,cash_preview_abi_version:()=>0},presentationCashInput),null);
  assert.equal(presentationRuntime.readSharedCashPreview({},presentationCashInput),null);assert.equal(calls,0);
});
check("presentation cash turning delegates only to local shared turn and never invokes layers or gameplay callbacks", () => {
  const calls=[];let output=1;
  const module={cash_preview_abi_version:()=>1,cash_preview_turn:(direction,right)=>{calls.push([direction,right]);return output;},
    cash_preview_layers:()=>{throw Error("turn must not create layers");},sendRaw:()=>{throw Error("no wire");},
    buy:()=>{throw Error("no purchase");},equip:()=>{throw Error("no equipment");}};
  assert.equal(presentationRuntime.turnSharedCashPreview(module,8,true),1);assert.deepEqual(calls,[[8,true]]);
  output=8;assert.equal(presentationRuntime.turnSharedCashPreview(module,1,false),8);assert.deepEqual(calls.at(-1),[1,false]);
  for(const bad of [0,9,1.5,NaN]){output=bad;assert.equal(presentationRuntime.turnSharedCashPreview(module,6,true),null);}
  const before=calls.length;assert.equal(presentationRuntime.turnSharedCashPreview(module,0,true),null);
  assert.equal(presentationRuntime.turnSharedCashPreview(module,6,1),null);
  assert.equal(presentationRuntime.turnSharedCashPreview({...module,cash_preview_abi_version:()=>0},6,true),null);assert.equal(calls.length,before);
  const syntax=ts.createSourceFile("client-presentation-runtime.ts",readFileSync(new URL("../lib/client-presentation-runtime.ts",import.meta.url),"utf8"),ts.ScriptTarget.Latest,true);
  const fn=syntax.statements.find(node=>ts.isFunctionDeclaration(node)&&node.name?.text==="turnSharedCashPreview");assert.ok(fn?.body);
  const moduleCalls=[];function walk(node){if(ts.isCallExpression(node)&&ts.isPropertyAccessExpression(node.expression)&&ts.isIdentifier(node.expression.expression)&&node.expression.expression.text==="module")moduleCalls.push(node.expression.name.text);ts.forEachChild(node,walk);}walk(fn.body);
  assert.deepEqual(moduleCalls,["cash_preview_abi_version","cash_preview_turn"]);
});

check("presentation repair forwards fourteen exact Rust fields raw signed stats rental and live fields without pricing", () => {
  let args;const quote={repairPrice:188,displayedTotal:18.80000114440918,totalPrice:18,affordable:false};
  const module={npc_repair_quote_abi_version:()=>1,npc_repair_quote:(...values)=>{args=values;return JSON.stringify({version:1,known:true,quote});}};
  const result=presentationRuntime.readSharedNpcRepairQuote(module,presentationRepairInput);
  assert.equal(args.length,14);assert.deepEqual(args.slice(0,9),[41n,41n,77,77,1000,1000,2,250,1000]);
  assert.ok(args[9] instanceof Int32Array);assert.deepEqual(Array.from(args[9]),[5,-3]);
  assert.deepEqual(args.slice(10),[false,0.1,false,18]);assert.deepEqual(result,quote);assert.equal(Object.isFrozen(result),true);
  const rented=structuredClone(presentationRepairInput);rented.tooltipSource.userItem.rental_information={};rented.special=true;
  presentationRuntime.readSharedNpcRepairQuote(module,rented);assert.equal(args[10],true);assert.equal(args[12],true);
  const zeroUid=structuredClone(presentationRepairInput);zeroUid.uniqueId=0;zeroUid.tooltipSource.userItem.unique_id=0;
  presentationRuntime.readSharedNpcRepairQuote(module,zeroUid);assert.equal(args[0],0n);assert.equal(args[1],0n);
});
check("presentation repair unsafe identity missing source fields and invalid live fields stay unknown without Rust calls", () => {
  let calls=0;const module={npc_repair_quote_abi_version:()=>1,npc_repair_quote:()=>{calls++;return "{}";}};
  const bad=[];
  for(const delta of [{uniqueId:Number.MAX_SAFE_INTEGER+1},{uniqueId:-1},{count:0},{count:65536},{currentDura:undefined},
    {maxDura:undefined},{currentDura:65536},{gold:4294967296},{rate:-1},{rate:Infinity},{rate:Number.MAX_VALUE},{special:1},{tooltipSource:null}])bad.push({...presentationRepairInput,...delta});
  for(const path of [["info","price"],["info","durability"],["info","item_index"],["userItem","unique_id"],
    ["userItem","item_index"],["userItem","added_stats"],["userItem","rental_information"]]){
    const item=structuredClone(presentationRepairInput);delete item.tooltipSource[path[0]][path[1]];bad.push(item);
  }
  const mismatch=structuredClone(presentationRepairInput);mismatch.tooltipSource.userItem.unique_id=42;bad.push(mismatch);
  const wrongIndex=structuredClone(presentationRepairInput);wrongIndex.tooltipSource.userItem.item_index=78;bad.push(wrongIndex);
  const statsAlias=structuredClone(presentationRepairInput);statsAlias.tooltipSource.userItem.added_stats=[{stat:5,value:1,alias:true}];bad.push(statsAlias);
  const statsUnsafe=structuredClone(presentationRepairInput);statsUnsafe.tooltipSource.userItem.added_stats=[{stat:5,value:2147483648}];bad.push(statsUnsafe);
  const statsBudget=structuredClone(presentationRepairInput);statsBudget.tooltipSource.userItem.added_stats=Array(257).fill({stat:5,value:1});bad.push(statsBudget);
  for(const item of bad)assert.equal(presentationRuntime.readSharedNpcRepairQuote(module,item),null);
  assert.equal(presentationRuntime.readSharedNpcRepairQuote({},presentationRepairInput),null);assert.equal(calls,0);
});
check("presentation repair preserves opaque Rust float affordability instead of computing a JS gold or rounding gate", () => {
  // Deliberately chosen opaque mock outcomes prove forwarding only. These values
  // are not claimed to be a real Rust price or authority to send a repair action.
  let answer={repairPrice:188,displayedTotal:18.80000114440918,totalPrice:18,affordable:true};
  const module={npc_repair_quote_abi_version:()=>1,npc_repair_quote:()=>JSON.stringify({version:1,known:true,quote:answer})};
  assert.equal(presentationRuntime.readSharedNpcRepairQuote(module,presentationRepairInput).affordable,true);
  answer={...answer,affordable:false};assert.equal(presentationRuntime.readSharedNpcRepairQuote(module,{...presentationRepairInput,gold:999999}).affordable,false);
  const syntax=ts.createSourceFile("client-presentation-runtime.ts",readFileSync(new URL("../lib/client-presentation-runtime.ts",import.meta.url),"utf8"),ts.ScriptTarget.Latest,true);
  const fn=syntax.statements.find(node=>ts.isFunctionDeclaration(node)&&node.name?.text==="readSharedNpcRepairQuote");assert.ok(fn?.body);
  const pricingOperators=[],roundingCalls=[];
  function walk(node){if(ts.isBinaryExpression(node)&&[ts.SyntaxKind.AsteriskToken,ts.SyntaxKind.SlashToken].includes(node.operatorToken.kind))pricingOperators.push(node.getText(syntax));
    if(ts.isCallExpression(node)&&ts.isPropertyAccessExpression(node.expression)&&["floor","ceil","round","reduce"].includes(node.expression.name.text))roundingCalls.push(node.expression.name.text);ts.forEachChild(node,walk);}walk(fn.body);
  assert.deepEqual(pricingOperators,[]);assert.deepEqual(roundingCalls,[]);
});
check("presentation repair unknown malformed exceptional or nonfinite output never grants a fabricated quote", () => {
  const good={version:1,known:true,quote:{repairPrice:188,displayedTotal:18.80000114440918,totalPrice:18,affordable:false}};
  let answer;const module={npc_repair_quote_abi_version:()=>1,npc_repair_quote:()=>JSON.stringify(answer)};
  for(const bad of [{version:1,known:false,quote:null},null,[],{...good,extra:1},{...good,version:2},{...good,known:1},
    {...good,quote:{...good.quote,extra:1}},...[-1,1.5,4294967296].map(totalPrice=>({...good,quote:{...good.quote,totalPrice}})),
    {...good,quote:{...good.quote,displayedTotal:-1}},{...good,quote:{...good.quote,displayedTotal:Infinity}},
    {...good,quote:{...good.quote,displayedTotal:4294967552}},{...good,quote:{...good.quote,affordable:1}}]){
    answer=bad;assert.equal(presentationRuntime.readSharedNpcRepairQuote(module,presentationRepairInput),null);
  }
  module.npc_repair_quote=()=>"{";assert.equal(presentationRuntime.readSharedNpcRepairQuote(module,presentationRepairInput),null);
  module.npc_repair_quote=()=>"x".repeat(1025);assert.equal(presentationRuntime.readSharedNpcRepairQuote(module,presentationRepairInput),null);
  module.npc_repair_quote=()=>{throw Error("mock ABI failure");};assert.equal(presentationRuntime.readSharedNpcRepairQuote(module,presentationRepairInput),null);
});

// NPC repair lifetime/authority fixtures load and exercise the real Page
// adapter. Quote results remain opaque Rust-shaped fixtures; no WASM or socket
// is created here. The AST assertions below bind the adapter contract to the
// production Page's final send gate and exact acknowledgement path.
const npcRepairServiceModule = loadTypeScriptModule(new URL("../lib/npc-repair-service.ts", import.meta.url), {
  "./equipment-gateway-adapter": equipmentGateway,
});
const { NpcRepairService, projectNpcRepairBag } = npcRepairServiceModule;
function npcRepairItem(uniqueId, slot, container = "bag1", quantity = 2, currentDura = 40, maxDura = 100) {
  return { uniqueId, slot, container, name: `Repair ${uniqueId}`, icon: uniqueId,
    quantity, durabilityCurrent: currentDura, durabilityMax: maxDura,
    tooltipSource: { info: { item_index: 77, price: 1000, durability: maxDura },
      userItem: { unique_id: uniqueId, item_index: 77, count: quantity, current_dura: currentDura,
        max_dura: maxDura, added_stats: [{ stat: 5, value: -3 }], rental_information: null } } };
}
function npcRepairSnapshot(overrides = {}) {
  return { playerObjectId: 17, mapFileName: "M001.map", gold: 500, inventoryCapacity: 54,
    inventoryItems: [npcRepairItem(900, 7), npcRepairItem(901, 3, "bag2", 1, 100, 100)],
    beltItems: [], equipmentItems: [{ uniqueId: 777, slot: "weapon" }], ...overrides };
}
function npcRepairOwner(socket = {}, changes = {}) {
  return { socket, connectionGeneration: 4, sessionGeneration: 8, ownerRevision: 3,
    playerObjectId: 17, sceneRevision: 12, mapFileName: "M001.map", ...changes };
}
function npcRepairQuote(input) {
  return { repairPrice: input.uniqueId === 901 ? 0 : 21, displayedTotal: input.uniqueId === 901 ? 0 : 21,
    totalPrice: input.uniqueId === 901 ? 0 : 21, affordable: true };
}

check("NPC repair projects complete raw Bag UID and physical bag1/bag2 slot, rejecting tooltip count and durability aliases", () => {
  const raw=npcRepairSnapshot(), bag=projectNpcRepairBag(raw);
  assert.deepEqual(bag.map(({uniqueId,slot})=>[uniqueId,slot]),[[900,7],[901,43]]);
  assert.equal(bag.some(row=>row.uniqueId===777),false); // equipment slot/index never becomes a Bag repair identity
  assert.deepEqual([bag[0].input.count,bag[0].input.currentDura,bag[0].input.maxDura],[2,40,100]);
  for(const alter of [
    item=>{item.tooltipSource.userItem.count=1;},
    item=>{item.tooltipSource.userItem.current_dura=39;},
    item=>{item.tooltipSource.userItem.max_dura=99;},
    item=>{delete item.tooltipSource.userItem.rental_information;},
    item=>{item.tooltipSource.userItem.added_stats=[{stat:5,value:-3,alias:true}];},
    item=>{item.tooltipSource.userItem.unique_id=7;},
  ]){
    const malformed=npcRepairSnapshot();alter(malformed.inventoryItems[0]);
    assert.equal(projectNpcRepairBag(malformed)[0].input,null);
  }
  const duplicate=npcRepairSnapshot();duplicate.inventoryItems[1].uniqueId=900;
  duplicate.inventoryItems[1].tooltipSource.userItem.unique_id=900;
  assert.equal(projectNpcRepairBag(duplicate),null);
  const duplicateZero=npcRepairSnapshot({inventoryItems:[npcRepairItem(0,4),npcRepairItem(0,5,"bag2",1,100,100)]});
  assert.equal(projectNpcRepairBag(duplicateZero),null);
  const nullIdentity=npcRepairSnapshot({inventoryItems:[npcRepairItem(null,6)]}), nullRow=projectNpcRepairBag(nullIdentity)[0];
  assert.equal(nullRow.uniqueId,null);assert.equal(nullRow.input,null);
  const nullOwner=npcRepairOwner(), nullService=new NpcRepairService();
  assert.equal(nullService.open(nullOwner,"repair",0.25,{}),true);
  assert.equal(nullService.observeSnapshot(nullOwner,nullIdentity),true);
  let nullQuotes=0;const nullView=nullService.view(nullOwner,()=>{nullQuotes++;return npcRepairQuote({uniqueId:0});},()=>true);
  assert.equal(nullQuotes,0);assert.equal(nullView.rows[0].reason,"unknown");
  assert.equal(nullService.select(nullView,null,nullView),null);
  const zeroIdentity=npcRepairSnapshot({inventoryItems:[npcRepairItem(0,6)]}), zeroRow=projectNpcRepairBag(zeroIdentity)[0];
  assert.equal(zeroRow.uniqueId,0);assert.equal(zeroRow.input.uniqueId,0);
});

check("NPC repair quote view binds exact owner rate mode live gold and opaque zero/full quote while unknown locked and unaffordable rows stay disabled", () => {
  const socket={}, owner=npcRepairOwner(socket), source=Object.freeze({npcObjectId:55,packet:"NPCRepair"});
  const service=new NpcRepairService();assert.equal(service.open(owner,"repair",0.25,source,"Repair NPC"),true);
  assert.equal(service.observeSnapshot(owner,npcRepairSnapshot()),true);
  const inputs=[];
  const view=service.view(owner,input=>{inputs.push(input);return npcRepairQuote(input);},()=>true);
  assert.equal(inputs.length,2);
  assert.deepEqual(inputs.map(({uniqueId,rate,special,gold,count,currentDura,maxDura})=>[uniqueId,rate,special,gold,count,currentDura,maxDura]),
    [[900,0.25,false,500,2,40,100],[901,0.25,false,500,1,100,100]]);
  assert.equal(view.source,source);assert.equal(view.mode,"repair");assert.equal(view.name,"Repair NPC");
  assert.deepEqual(view.rows.map(({uniqueId,reason,disabled,quote})=>[uniqueId,reason,disabled,quote?.totalPrice]),
    [[900,null,false,21],[901,null,false,0]]); // a known zero price is distinct from unknown
  assert.equal(service.select(view,900,view)?.uniqueId,900);
  assert.equal(service.select(view,901,view)?.uniqueId,901);
  const locked=service.view(owner,npcRepairQuote,id=>id!==901);
  assert.equal(locked.rows[1].reason,"locked");assert.equal(service.select(locked,901,locked),null);
  const unknown=service.view(owner,()=>null,()=>true);
  assert.equal(unknown.rows[0].reason,"unknown");assert.equal(service.select(unknown,900,unknown),null);
  const unaffordable=service.view(owner,input=>({...npcRepairQuote(input),affordable:false}),()=>true);
  assert.equal(unaffordable.rows[0].reason,"gold");assert.equal(unaffordable.rows[0].disabled,true);
});

check("NPC repair selection survives equivalent complete snapshots but explicit unknown and changed owner/source epochs retire old intent", () => {
  const socket={}, owner=npcRepairOwner(socket), source={}, service=new NpcRepairService();
  assert.equal(service.open(owner,"repair",0.5,source),true);assert.equal(service.observeSnapshot(owner,npcRepairSnapshot()),true);
  const first=service.view(owner,npcRepairQuote,()=>true), oldSelection=service.select(first,900,first);
  assert.ok(oldSelection);
  // Position/entities/chat can advance in full snapshots without changing the
  // current repair source. Selection can still be confirmed against that source.
  const equivalent={...npcRepairSnapshot(),entities:[{objectId:99,x:20,y:30}],chat:["new chat"],playerPosition:{x:8,y:9}};
  service.prepareSnapshot(owner,equivalent);
  assert.equal(service.observeSnapshot(owner,equivalent),true);
  const second=service.view(owner,npcRepairQuote,()=>true);
  const equivalentProof=service.reserve(oldSelection,second);assert.ok(equivalentProof);
  assert.equal(service.enter(equivalentProof,second,{...equivalentProof.command}),true);
  assert.equal(service.acknowledge(socket,"ItemRepaired",{uniqueId:900,currentDura:60,maxDura:100}),true);

  // An unknown interval explicitly retires the epoch. Seeing the byte-equivalent
  // snapshot later cannot revive a selection captured before that interval.
  const interrupted=new NpcRepairService();assert.equal(interrupted.open(owner,"repair",0.5,{}),true);
  const known=npcRepairSnapshot();assert.equal(interrupted.observeSnapshot(owner,known),true);
  const beforeUnknown=interrupted.view(owner,npcRepairQuote,()=>true);
  const invalidatedSelection=interrupted.select(beforeUnknown,900,beforeUnknown);assert.ok(invalidatedSelection);
  interrupted.prepareSnapshot(owner,null);
  assert.equal(interrupted.observeSnapshot(owner,known),true);
  const afterUnknown=interrupted.view(owner,npcRepairQuote,()=>true);
  assert.equal(interrupted.reserve(invalidatedSelection,afterUnknown),null);

  const nextSelection=service.select(second,900,second), nextProof=service.reserve(nextSelection,second);assert.ok(nextProof);
  const changedOwner=service.view(npcRepairOwner(socket,{sceneRevision:13}),npcRepairQuote,()=>true);
  assert.equal(changedOwner,null);
  assert.equal(service.enter(nextProof,changedOwner,{...nextProof?.command}),false);

  for(const changes of [
    {socket:{}},{connectionGeneration:5},{sessionGeneration:9},{ownerRevision:4},
    {playerObjectId:18},{sceneRevision:13},{mapFileName:"M002.map"},
  ]){
    const physical=npcRepairOwner(), ownerScoped=new NpcRepairService();
    assert.equal(ownerScoped.open(physical,"repair",0.5,{}),true);
    assert.equal(ownerScoped.observeSnapshot(physical,npcRepairSnapshot()),true);
    const captured=ownerScoped.view(physical,npcRepairQuote,()=>true);
    assert.equal(ownerScoped.view(npcRepairOwner(physical.socket,changes),npcRepairQuote,()=>true),null);
    assert.equal(ownerScoped.select(captured,900,null),null);
  }

  // A new repair dialog on the same physical server owner may begin under the
  // latest Bag renderer revision, but it needs a new full snapshot and intent.
  const physicalSocket={}, priorOwner=npcRepairOwner(physicalSocket), latestOwner=npcRepairOwner(physicalSocket,{ownerRevision:4});
  const incoming=new NpcRepairService();assert.equal(incoming.open(latestOwner,"repair",0.4,{npcObjectId:55,packet:"NPCRepair"}),true);
  assert.equal(incoming.observeSnapshot(latestOwner,npcRepairSnapshot()),true);
  const incomingView=incoming.view(latestOwner,npcRepairQuote,()=>true);
  assert.equal(incoming.select(incomingView,900,incomingView)?.uniqueId,900);
  assert.notEqual(priorOwner.ownerRevision,latestOwner.ownerRevision);

  const leaseService=new NpcRepairService(), leaseOwner=npcRepairOwner(), oldNpcSource={npcObjectId:55};
  assert.equal(leaseService.open(leaseOwner,"repair",0.5,oldNpcSource),true);
  assert.equal(leaseService.observeSnapshot(leaseOwner,npcRepairSnapshot()),true);
  const oldNpcView=leaseService.view(leaseOwner,npcRepairQuote,()=>true);
  const oldNpcSelection=leaseService.select(oldNpcView,900,oldNpcView);
  assert.equal(leaseService.open(leaseOwner,"special",0.75,{npcObjectId:56}),true);
  const newNpcView=leaseService.view(leaseOwner,npcRepairQuote,()=>true);
  assert.equal(leaseService.reserve(oldNpcSelection,newNpcView),null);

  const switched=new NpcRepairService();assert.equal(switched.open(owner,"special",0.75,{}),true);
  assert.equal(switched.observeSnapshot(owner,npcRepairSnapshot()),true);
  const special=switched.view(owner,npcRepairQuote,()=>true), specialSelection=switched.select(special,900,special);
  const specialProof=switched.reserve(specialSelection,special);
  assert.deepEqual(specialProof.command,{type:"specialRepairItem",uniqueId:900});
  assert.equal(switched.enter(specialProof,special,{...specialProof.command}),true);
});

check("NPC repair entered UID barrier survives transport throw close and service replacement until exact ItemRepaired on its socket", () => {
  const socket={}, owner=npcRepairOwner(socket), quoteReader=npcRepairQuote;
  const service=new NpcRepairService();assert.equal(service.open(owner,"repair",0.25,{}),true);
  assert.equal(service.observeSnapshot(owner,npcRepairSnapshot()),true);
  const view=service.view(owner,quoteReader,()=>true), selection=service.select(view,900,view), proof=service.reserve(selection,view);
  let sends=0;
  assert.equal(service.enter(proof,view,{...proof.command}),true); // ownership is committed before transport
  try { sends++; throw new Error("transport outcome unknown"); } catch {}
  assert.equal(sends,1);service.close();
  const remounted=new NpcRepairService();assert.equal(remounted.open(owner,"special",0.25,{}),true);
  assert.equal(remounted.observeSnapshot(owner,npcRepairSnapshot()),true);
  let current=remounted.view(owner,quoteReader,()=>true);
  assert.equal(current.rows.find(row=>row.uniqueId===900).reason,"busy");
  assert.equal(remounted.acknowledge(socket,"RepairItem",{uniqueId:900,currentDura:60,maxDura:100}),false);
  assert.equal(remounted.acknowledge(socket,"ItemRepaired",{uniqueId:900,currentDura:60,maxDura:100,extra:1}),false);
  assert.equal(remounted.acknowledge({},"ItemRepaired",{uniqueId:900,currentDura:60,maxDura:100}),false);
  assert.equal(remounted.acknowledge(socket,"ItemRepaired",{uniqueId:901,currentDura:60,maxDura:100}),false);
  assert.equal(remounted.acknowledge(socket,"ItemRepaired",{uniqueId:900,currentDura:101,maxDura:100}),false);
  assert.equal(remounted.acknowledge(socket,"ItemRepaired",{uniqueId:900,currentDura:60,maxDura:100}),true);
  current=remounted.view(owner,quoteReader,()=>true);
  assert.equal(current.rows.find(row=>row.uniqueId===900).reason,null);
  // A different physical socket has a separate ledger, but still requires a fresh user selection.
  const newSocket={}, newOwner=npcRepairOwner(newSocket), independent=new NpcRepairService();
  assert.equal(independent.open(newOwner,"repair",0.25,{}),true);
  assert.equal(independent.observeSnapshot(newOwner,npcRepairSnapshot()),true);
  const next=independent.view(newOwner,quoteReader,()=>true), nextSelection=independent.select(next,900,next);
  const nextProof=independent.reserve(nextSelection,next);assert.equal(independent.enter(nextProof,next,{...nextProof.command}),true);
  assert.equal(sends,1); // no automatic retransmit occurred during remount/ACK
});

check("NPC repair Page consumer captures NPC owner and rate, retires invalid sources, and enters exact proof before socket send", () => {
  const source=readFileSync(new URL("../app/page.tsx",import.meta.url),"utf8");
  const ast=ts.createSourceFile("page.tsx",source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  const declarations=new Map();
  function collect(node){if(ts.isFunctionDeclaration(node)&&node.name)declarations.set(node.name.text,node);ts.forEachChild(node,collect);}collect(ast);
  const fn=name=>{const node=declarations.get(name);assert.ok(node,`missing Page function ${name}`);return node;};
  const callNodes=node=>{const calls=[];function walk(current){if(ts.isCallExpression(current))calls.push(current);ts.forEachChild(current,walk);}walk(node);return calls;};
  const capture=fn("captureNpcRepairDialog");
  const captureText=capture.getText(ast);
  for(const field of ["activeNpcDialog","npcObjectId","sceneRevision","mapFileName"])
    assert.ok(captureText.includes(field),`dialog capture must bind ${field}`);
  assert.match(fn("readNpcRepairOwner").getText(ast),/ownerRevision:\s*equipmentBagOwnerRef\.current\.ownerRevision/);
  assert.match(fn("currentNpcRepairDialogSource").getText(ast),/source\.owner\.sceneRevision\s*===\s*owner\.sceneRevision/);
  const openCase=source.slice(source.indexOf('case "NPCRepair":'),source.indexOf('case "NPCResponse":'));
  assert.match(openCase,/payload\.rate/);assert.match(openCase,/currentNpcRepairDialogSource\(owner\)/);
  assert.match(openCase,/npcRepairAuthorityRef\.current\.open\(owner, mode, payload\.rate/);
  const confirm=fn("confirmNpcRepair"), confirmCalls=callNodes(confirm).map(call=>call.expression.getText(ast));
  assert.ok(confirmCalls.some(text=>text.includes("reserve")));
  assert.ok(confirmCalls.some(text=>text.startsWith("sendRaw")));
  assert.match(confirm.getText(ast),/npcRepairProof:\s*proof/);
  const send=fn("sendRaw"), calls=callNodes(send);
  const enter=calls.find(call=>call.expression.getText(ast).includes("npcRepairAuthorityRef.current.enter"));
  const socketSend=calls.find(call=>call.expression.getText(ast)==="socket.send");
  assert.ok(enter&&socketSend&&enter.pos<socketSend.pos,"opaque repair proof must enter its socket gate immediately before transport");
  const event=fn("handleGatewayEvent").getText(ast);
  assert.match(event,/event\.packet === "ItemRepaired"/);
  assert.match(event,/acknowledge\(source, event\.packet, event\.payload\)/);
  const handlerCalls=callNodes(fn("handleGatewayEvent"));
  const preflight=handlerCalls.find(call=>call.expression.getText(ast)==="invalidateNpcGoldBuyGatewayPacket");
  const diagnostic=handlerCalls.find(call=>call.expression.getText(ast)==="captureQuestMapGatewayEvent");
  assert.ok(preflight&&diagnostic&&preflight.pos<diagnostic.pos,"repair snapshot preflight must run before gateway diagnostics");
  const viewText=fn("readNpcRepairView").getText(ast);
  assert.match(viewText,/core\?\.readNpcRepairQuote\(input\)/);
  assert.match(viewText,/mailMutationAllowed\(wire, mailParcelRef\.current\?\.snapshot \?\? null,/);
  assert.match(viewText,/mailParcelRef\.current\?\.state\?\.blockedUniqueIds \?\? \[\]/);
  const snapshot=fn("applyNpcGoldBuyGatewaySnapshot").getText(ast);
  assert.match(snapshot,/npcRepairAuthorityRef\.current\.observeSnapshot\(owner, snapshot\)/);
  assert.match(snapshot,/captureNpcRepairDialog\(snapshot, owner\)/);
  assert.match(fn("invalidateNpcGoldBuyGatewayPacket").getText(ast),/prepareSnapshot\(readNpcRepairOwner\(\), event\.payload\)/);
  const scene=readFileSync(new URL("../app/components/original-client-game-ui-scene.tsx",import.meta.url),"utf8");
  const sceneAst=ts.createSourceFile("original-client-game-ui-scene.tsx",scene,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  const sceneText=sceneAst.getText();
  assert.match(sceneText,/id:\s*item\.uniqueId\s*\?\?\s*`unknown:\$\{item\.slot\}`/);
  assert.match(sceneText,/disabled:\s*item\.disabled/);
  const shop=readFileSync(new URL("../app/components/original-client-game-shop.tsx",import.meta.url),"utf8");
  const shopAst=ts.createSourceFile("original-client-game-shop.tsx",shop,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  const shopFns=[];function walkShop(node){if(ts.isFunctionDeclaration(node)&&node.name?.text==="NpcShopWindow")shopFns.push(node);ts.forEachChild(node,walkShop);}walkShop(shopAst);
  assert.ok(shopFns.length>0);
  const shopText=shopFns[0].getText(shopAst);
  // Repair returns its target UI before the generic Buy/Sell list. Check that
  // reachable branch, not the removed legacy list-row selection handlers.
  const repairBranches=shopFns[0].body.statements.filter(node=>ts.isIfStatement(node)&&node.expression.getText(shopAst)==="repairTab"
    &&ts.isBlock(node.thenStatement)&&node.thenStatement.statements.some(statement=>ts.isReturnStatement(statement)));
  assert.equal(repairBranches.length,1,"sole reachable early repair UI branch");
  const repairBranch=repairBranches[0],repairStatements=repairBranch.thenStatement.statements;
  const repairBindings=new Map(repairStatements.filter(ts.isVariableStatement).flatMap(statement=>[...statement.declarationList.declarations])
    .map(declaration=>[declaration.name.getText(shopAst),declaration.initializer]));
  assert.equal(repairBindings.get("targetSelection").getText(shopAst),"repairTargetSelection?.stamp === repairView?.stamp ? repairTargetSelection : null");
  assert.equal(repairBindings.get("target").getText(shopAst),"targetSelection ? repairView?.rows.find(row => row.uniqueId === targetSelection.uniqueId) : null");
  const genericReturn=shopFns[0].body.statements.find(node=>ts.isReturnStatement(node));
  assert.ok(genericReturn&&repairBranch.end<genericReturn.getStart(shopAst),"repair target branch cannot fall through to the generic list");
  let confirmControl=null;
  function findRepairControl(node){
    if(ts.isJsxElement(node)&&node.openingElement.attributes.properties.some(attr=>ts.isJsxAttribute(attr)
      &&attr.name.getText(shopAst)==="data-npc-repair-control"&&attr.initializer&&ts.isStringLiteral(attr.initializer)&&attr.initializer.text==="confirm")){
      assert.equal(confirmControl,null);confirmControl=node;
    }
    ts.forEachChild(node,findRepairControl);
  }
  findRepairControl(repairBranch);assert.ok(confirmControl,"reachable physical repair confirm control");
  const confirmButtons=confirmControl.children.filter(node=>ts.isJsxSelfClosingElement(node)&&node.tagName.getText(shopAst)==="SpriteButton");assert.equal(confirmButtons.length,1);
  const confirmAttributes=new Map(confirmButtons[0].attributes.properties.filter(ts.isJsxAttribute).map(attr=>[attr.name.getText(shopAst),attr.initializer]));
  assert.equal(confirmAttributes.get("disabled").expression.getText(shopAst),"!targetSelection || !target || target.disabled || blocked() || !onConfirmRepair");
  const confirmClick=confirmAttributes.get("onClick").expression;assert.ok(ts.isArrowFunction(confirmClick)&&ts.isBlock(confirmClick.body));
  assert.equal(confirmClick.body.statements.length,1);const confirmIf=confirmClick.body.statements[0];assert.ok(ts.isIfStatement(confirmIf));
  assert.equal(confirmIf.expression.getText(shopAst),"targetSelection && target && !target.disabled && !blocked()");
  assert.equal(confirmIf.thenStatement.getText(shopAst),"onConfirmRepair?.(targetSelection);");
  assert.match(shopText,/const blocked = \(\) => inputBlocked \|\| getInputBlocked\?\.\(\) === true/);
  assert.match(shopText,/if \(repairTab && repairView && node\) return onRegisterRepairTarget\?\.\(repairView, node\)/);
  assert.match(shopText,/\[repairTab, repairView\?\.stamp, onRegisterRepairTarget\]/);
});

check("NPC repair mail locks use Bag UID and optional blocked-cell checks; ordinary and special modes accept real UID zero", () => {
  const command={type:"repairItem",uniqueId:900};
  const snapshot={items:[{uniqueId:900,container:0,slot:7}]};
  assert.equal(mailParcelGateway.mailMutationAllowed(command,snapshot,[900]),false);
  assert.equal(mailParcelGateway.mailMutationAllowed(command,snapshot,[],[{container:0,slot:7}]),false);
  assert.equal(mailParcelGateway.mailMutationAllowed(command,snapshot,[],[{container:0,slot:8}]),true);
  assert.equal(mailParcelGateway.mailMutationAllowed(command,null,[900]),false);
  for(const mode of ["repair","special"]){
    const socket={},owner=npcRepairOwner(socket),service=new NpcRepairService();
    assert.equal(service.open(owner,mode,0.25,{}),true);
    assert.equal(service.observeSnapshot(owner,npcRepairSnapshot({inventoryItems:[npcRepairItem(0,9)]})),true);
    const view=service.view(owner,input=>({repairPrice:0,displayedTotal:0,totalPrice:0,affordable:true}),()=>true);
    const selection=service.select(view,0,view);assert.ok(selection);
    const proof=service.reserve(selection,view);assert.deepEqual(proof.command,
      {type:mode==="repair"?"repairItem":"specialRepairItem",uniqueId:0});
    assert.equal(service.enter(proof,view,{...proof.command}),true);
    assert.equal(service.acknowledge(socket,"ItemRepaired",{uniqueId:0,currentDura:0,maxDura:0}),true);
  }
});


// Source20 extends the existing real module and Page extraction. Quote answers
// stay opaque ABI-shaped data: these checks never calculate repair prices.
const repairDragGeometry = Object.freeze({ pointerId: 7, pointerType: "mouse", page: "bag1",
  stage: {}, target: {}, item: {}, stageRect: [10,20,800,600], targetRect: [110,120,80,60],
  itemRect: [30,40,32,32], virtualWidth: 800, virtualHeight: 600, scale: 1, devicePixelRatio: 2 });
check("NPC repair opaque drag accepts actual UID zero and Bag2 physical slot and spends every terminal once", () => {
  for (const [uid, slot, container] of [[0,7,"bag1"],[901,3,"bag2"]]) {
    const owner=npcRepairOwner(), service=new NpcRepairService(), raw=npcRepairSnapshot({inventoryItems:[npcRepairItem(uid,slot,container)]});
    assert.equal(service.open(owner,"repair",0.25,{}),true);assert.equal(service.observeSnapshot(owner,raw),true);
    const view=service.view(owner,npcRepairQuote,()=>true), geometry={...repairDragGeometry,page:container};
    const drag=service.beginDrag(view,uid,slot+(container==="bag2"?40:0),geometry,view);assert.ok(drag);assert.equal(Object.isFrozen(drag),true);
    assert.equal(service.drop({...drag},geometry,111,121,view),null,"copying public fields cannot mint a registered drag");
    const selection=service.drop(drag,geometry,111,121,view);assert.equal(selection.uniqueId,uid);
    assert.equal(service.drop(drag,geometry,111,121,view),null);assert.equal(service.beginDrag(view,uid,slot, {...geometry,page:container==="bag1"?"bag2":"bag1"},view),null);
    const cancelled=service.beginDrag(view,uid,drag.slot,geometry,view);assert.ok(cancelled);service.cancelDrag(cancelled);
    assert.equal(service.drop(cancelled,geometry,111,121,view),null);
    const outside=service.beginDrag(view,uid,drag.slot,geometry,view);assert.ok(outside);
    assert.equal(service.drop(outside,geometry,190,121,view),null,"right boundary is outside");
    assert.equal(service.drop(outside,geometry,111,121,view),null,"outside consumes the old terminal");
  }
});
check("NPC repair known low gold remains selectable by row or drag while confirmation stays disabled", () => {
  const owner=npcRepairOwner(),service=new NpcRepairService();assert.equal(service.open(owner,"special",0.25,{}),true);
  assert.equal(service.observeSnapshot(owner,npcRepairSnapshot()),true);
  const low=service.view(owner,input=>({...npcRepairQuote(input),affordable:false}),()=>true);
  assert.equal(low.rows[0].reason,"gold");assert.equal(low.rows[0].disabled,true);
  assert.ok(service.select(low,900,low));const drag=service.beginDrag(low,900,7,repairDragGeometry,low);assert.ok(drag);
  const selected=service.drop(drag,repairDragGeometry,111,121,low);assert.ok(selected);assert.equal(service.reserve(selected,low),null);
  const ready=service.view(owner,npcRepairQuote,()=>true);const staleSelection=service.select(ready,900,ready),proof=service.reserve(staleSelection,ready);assert.ok(proof);
  const poorer=service.view(owner,input=>({...npcRepairQuote(input),affordable:false}),()=>true);
  assert.equal(service.enter(proof,poorer,{...proof.command}),false);assert.equal(service.enter(proof,ready,{...proof.command}),false);
  for(const [reader,allowed,reason] of [[()=>null,()=>true,"unknown"],[npcRepairQuote,()=>false,"locked"]]){
    const blocked=service.view(owner,reader,allowed);assert.equal(blocked.rows[0].reason,reason);
    assert.equal(service.select(blocked,900,blocked),null);assert.equal(service.beginDrag(blocked,900,7,repairDragGeometry,blocked),null);
  }
});
check("NPC repair drag live geometry page pointer DPR and source changes cannot revive a spent token", () => {
  for(const delta of [{pointerId:8},{pointerType:"touch"},{page:"bag2"},{stage:{}},{target:{}},{item:{}},
    {stageRect:[11,20,800,600]},{targetRect:[111,120,80,60]},{itemRect:[31,40,32,32]},
    {virtualWidth:801},{virtualHeight:601},{scale:2},{devicePixelRatio:1},{targetRect:[110,120,0,60]}]){
    const owner=npcRepairOwner(),service=new NpcRepairService();service.open(owner,"repair",0.25,{});service.observeSnapshot(owner,npcRepairSnapshot());
    const view=service.view(owner,npcRepairQuote,()=>true),drag=service.beginDrag(view,900,7,repairDragGeometry,view);assert.ok(drag);
    assert.equal(service.drop(drag,{...repairDragGeometry,...delta},111,121,view),null);
    assert.equal(service.drop(drag,repairDragGeometry,111,121,view),null);
  }
  for(const delta of [{socket:{}},{connectionGeneration:99},{sessionGeneration:99},{ownerRevision:99},{playerObjectId:18},{sceneRevision:99},{mapFileName:"M002.map"}]){
    const owner=npcRepairOwner(),service=new NpcRepairService();service.open(owner,"repair",0.25,{});service.observeSnapshot(owner,npcRepairSnapshot());
    const view=service.view(owner,npcRepairQuote,()=>true),drag=service.beginDrag(view,900,7,repairDragGeometry,view);
    assert.equal(service.drop(drag,repairDragGeometry,111,121,service.view({...owner,...delta},npcRepairQuote,()=>true)),null);
  }
  for(const change of [raw=>{raw.gold++;},raw=>{raw.inventoryItems[0].quantity++;raw.inventoryItems[0].tooltipSource.userItem.count++;},
    raw=>{raw.inventoryItems[0].slot++;},raw=>{raw.inventoryItems[0].tooltipSource.info.price++;}]){
    const owner=npcRepairOwner(),service=new NpcRepairService();service.open(owner,"repair",0.25,{});service.observeSnapshot(owner,npcRepairSnapshot());
    const view=service.view(owner,npcRepairQuote,()=>true),drag=service.beginDrag(view,900,7,repairDragGeometry,view),next=npcRepairSnapshot();change(next);
    service.prepareSnapshot(owner,next);service.observeSnapshot(owner,next);
    assert.equal(service.drop(drag,repairDragGeometry,111,121,service.view(owner,npcRepairQuote,()=>true)),null);
  }

  // No raw retirement here: equal source/key restoration must still carry a
  // different stamp after quote or mutation authority became unknown/locked.
  for(const unavailable of ["quote","permission"]){
    const owner=npcRepairOwner(),service=new NpcRepairService();service.open(owner,"repair",0.25,{});service.observeSnapshot(owner,npcRepairSnapshot());
    const known=service.view(owner,npcRepairQuote,()=>true),continuous=service.view(owner,npcRepairQuote,()=>true);
    assert.equal(continuous.stamp,known.stamp,"consecutive fully known views preserve the same stamp");
    const positiveDrag=service.beginDrag(known,900,7,repairDragGeometry,continuous);assert.ok(positiveDrag);
    assert.ok(service.drop(positiveDrag,repairDragGeometry,111,121,continuous),"same-stamp refresh retains valid drag authority");
    assert.ok(service.select(known,900,continuous),"same-stamp refresh retains valid selection authority");
    const drag=service.beginDrag(known,900,7,repairDragGeometry,continuous),selection=service.select(known,900,continuous);
    const proof=service.reserve(service.select(known,900,continuous),continuous);assert.ok(drag);assert.ok(selection);assert.ok(proof);
    const blocked=service.view(owner,unavailable==="quote"?()=>null:npcRepairQuote,()=>unavailable!=="permission");
    assert.equal(blocked.rows[0].reason,unavailable==="quote"?"unknown":"locked");assert.notEqual(blocked.stamp,known.stamp);
    const restored=service.view(owner,npcRepairQuote,()=>true);assert.equal(restored.rows[0].reason,null);
    assert.deepEqual(restored.rows,known.rows,"restored rows and costs are completely identical");assert.notEqual(restored.stamp,known.stamp);
    assert.equal(service.drop(drag,repairDragGeometry,111,121,restored),null,"equal key cannot revive an older drag stamp");
    assert.equal(service.select(known,900,restored),null,"old display stamp cannot create a new selection");
    assert.equal(service.reserve(selection,restored),null,"equal key cannot revive an older selection stamp");
    assert.equal(service.enter(proof,restored,{...proof.command}),false,"equal key cannot revive an older proof stamp");
  }
});
check("NPC repair Hold preserves normal authoritative updates but unknown owner source and close discard it", () => {
  const owner=npcRepairOwner(),service=new NpcRepairService();service.open(owner,"repair",0.25,{});service.observeSnapshot(owner,npcRepairSnapshot());
  let view=service.view(owner,npcRepairQuote,()=>true);assert.equal(view.hold,false);assert.equal(service.toggleHold(view,view),true);
  view=service.view(owner,npcRepairQuote,()=>true);assert.equal(view.hold,true);
  const next=npcRepairSnapshot({gold:480});next.inventoryItems[0].durabilityCurrent=60;next.inventoryItems[0].tooltipSource.userItem.current_dura=60;
  service.prepareSnapshot(owner,next);service.observeSnapshot(owner,next);view=service.view(owner,npcRepairQuote,()=>true);assert.equal(view.hold,true);
  service.invalidateInventory();service.observeSnapshot(owner,next);view=service.view(owner,npcRepairQuote,()=>true);assert.equal(view.hold,true,"post-send source retire does not turn off explicit Hold");
  for(const bad of [null,{...next,inventoryItems:null}]){
    service.prepareSnapshot(owner,bad);assert.equal(service.observeSnapshot(owner,bad),false);service.observeSnapshot(owner,next);
    view=service.view(owner,npcRepairQuote,()=>true);assert.equal(view.hold,false);assert.equal(service.toggleHold(view,view),true);
  }
  service.prepareSnapshot({...owner,ownerRevision:9},next);service.observeSnapshot(owner,next);view=service.view(owner,npcRepairQuote,()=>true);assert.equal(view.hold,false);
  assert.equal(service.toggleHold(view,view),true);service.close();service.open(owner,"repair",0.25,{});
  view=service.view(owner,npcRepairQuote,()=>true);assert.equal(view.hold,false);
});
check("NPC repair fresh Hold drops keep entered UID barrier through cancel close and exact ACK without replay", () => {
  const owner=npcRepairOwner(),service=new NpcRepairService();service.open(owner,"special",0.25,{});service.observeSnapshot(owner,npcRepairSnapshot());
  let view=service.view(owner,npcRepairQuote,()=>true);service.toggleHold(view,view);view=service.view(owner,npcRepairQuote,()=>true);
  const first=service.beginDrag(view,900,7,repairDragGeometry,view),selection=service.drop(first,repairDragGeometry,111,121,view),proof=service.reserve(selection,view);
  assert.ok(proof);let sends=0;const entered=service.enter(proof,view,{...proof.command});assert.equal(entered,true);if(entered)sends++;
  assert.equal(service.enter(proof,view,{...proof.command}),false);service.cancelDrag(first);service.invalidateInventory();service.observeSnapshot(owner,npcRepairSnapshot());
  view=service.view(owner,npcRepairQuote,()=>true);assert.equal(view.hold,true);assert.equal(view.rows[0].reason,"busy");
  assert.equal(service.beginDrag(view,900,7,repairDragGeometry,view),null);service.close();service.open(owner,"special",0.25,{});
  view=service.view(owner,npcRepairQuote,()=>true);assert.equal(view.rows[0].reason,"busy");
  assert.equal(service.acknowledge({},"ItemRepaired",{uniqueId:900,currentDura:60,maxDura:100}),false);
  assert.equal(service.acknowledge(owner.socket,"ItemRepaired",{uniqueId:900,currentDura:60,maxDura:100}),true);
  view=service.view(owner,npcRepairQuote,()=>true);assert.equal(view.rows[0].reason,null);assert.equal(service.drop(first,repairDragGeometry,111,121,view),null);
  assert.equal(sends,1,"ACK and service changes never submit another command");
});

check("NPC repair actual Page Hold drop sends once through final proof and live owner source barriers", () => {
  for(const variant of ["manual","hold","lowGold","owner","source","pending"]){
    const owner=npcRepairOwner(),raw=npcRepairSnapshot(),page=parityPageFixture(owner,{...raw,connected:true,playerHp:100,entities:[{objectId:17,dead:false}]});
    const service=new NpcRepairService(),scope=page.scope;scope.npcRepairAuthorityRef.current=service;
    scope.equipmentBagOwnerRef.current.ownerRevision=owner.ownerRevision;scope.bagOpenRef.current=true;
    scope.npcRepairServiceRef.current="repair";scope.equipmentControllerRef.current={status:()=>({ready:true,pending:0})};
    scope.questCoreRuntimeRef.current={readNpcRepairQuote:input=>({...npcRepairQuote(input),affordable:variant!=="lowGold"})};
    page.api.captureNpcRepairDialog({...raw,activeNpcDialog:{npcObjectId:55}},owner);
    scope.npcRepairDialogBindingRef.current=scope.npcRepairDialogSourceRef.current;
    assert.equal(service.open(owner,"repair",0.25,scope.npcRepairDialogBindingRef.current),true);assert.equal(service.observeSnapshot(owner,raw),true);
    let listener=()=>{};scope.sendRawHandler.current=(command,options)=>page.api.parityFinal(command,options,page.socket,listener);
    let view=page.api.readNpcRepairView();assert.ok(view);assert.equal(view.hold,false);
    if(variant!=="manual"){page.api.toggleNpcRepairHold(view);view=page.api.readNpcRepairView();assert.equal(view.hold,true);}
    assert.equal(page.sent.length,0,"Hold toggle has no wire action");
    const row=scope.worldRef.current.inventoryItems[0];row.key="actual-bag-7";row.authoritativeUniqueId=900;
    const item={key:row.key,uniqueId:900,authoritativeUniqueId:900,container:"bag1",slot:7};
    const token=page.api.beginNpcRepairDrag(view,item,repairDragGeometry);assert.ok(token);
    if(variant==="owner")listener=()=>{scope.equipmentSessionGenerationRef.current++;};
    if(variant==="source")listener=()=>{service.prepareSnapshot(owner,null);};
    if(variant==="pending")listener=()=>{scope.pendingStorageRequestsRef.current.set("new-flight",{});};
    const result=page.api.dropNpcRepairDrag(token,repairDragGeometry,111,121);assert.ok(result);assert.equal(result.selection.uniqueId,900);
    assert.equal(result.submitted,variant==="hold");assert.equal(page.sent.length,variant==="hold"?1:0);
    assert.equal(page.api.dropNpcRepairDrag(token,repairDragGeometry,111,121),null);
    if(variant==="manual"){assert.equal(page.api.confirmNpcRepair(result.selection),true);assert.equal(page.sent.length,1);}
    if(variant==="lowGold"){assert.equal(page.api.confirmNpcRepair(result.selection),false);assert.equal(page.sent.length,0);}
    if(variant==="hold"||variant==="manual"){
      assert.deepEqual(page.sent,[{type:"repairItem",uniqueId:900}]);
      assert.equal(page.api.confirmNpcRepair(result.selection),false);assert.equal(page.sent.length,1);
      assert.equal(service.acknowledge(owner.socket,"ItemRepaired",{uniqueId:900,currentDura:60,maxDura:100}),true);
      assert.equal(page.sent.length,1,"exact ACK cannot replay the drop");
    }
  }
});

// Extract the actual Shell event routes and registered capture listeners. DOM
// objects below supply geometry/capture only; repair authority is the real module.
const repairShellSource=readFileSync(new URL("../app/original-client-shell.tsx",import.meta.url),"utf8");
const repairShellAst=ts.createSourceFile("original-client-shell.tsx",repairShellSource,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const repairShellNames=new Set(["repairGeometry","cancelNpcRepairPointer","beginNpcRepairPointer","handleNpcRepairPointer","fenceNpcRepairClick",
  "npcRepairControlTarget","rememberNpcRepairControlClick","handleSharedUiPointer","changeNpcRepairTarget","confirmNpcRepairTarget","openNpcRepairBagPage"]);
const repairShellDeclarations=new Map();let repairTargetRegistration=null,repairListenerEffect=null,repairLostCapture=null;
function visitRepairShell(node){
  if(ts.isFunctionDeclaration(node)&&node.name&&repairShellNames.has(node.name.text))repairShellDeclarations.set(node.name.text,node.getText(repairShellAst));
  if(ts.isVariableDeclaration(node)&&ts.isIdentifier(node.name)&&node.name.text==="registerNpcRepairTarget"){
    assert.ok(ts.isCallExpression(node.initializer)&&ts.isArrowFunction(node.initializer.arguments[0]));repairTargetRegistration=node.initializer.arguments[0].getText(repairShellAst);
  }
  if(ts.isCallExpression(node)&&ts.isIdentifier(node.expression)&&node.expression.text==="useEffect"&&ts.isArrowFunction(node.arguments[0])){
    const calls=[];function visitCalls(child){if(ts.isCallExpression(child)&&ts.isPropertyAccessExpression(child.expression)
      &&child.expression.name.text==="addEventListener"&&ts.isStringLiteral(child.arguments[0]))calls.push(child.arguments[0].text);ts.forEachChild(child,visitCalls);}
    visitCalls(node.arguments[0]);if(calls.includes("pointerdown")&&calls.includes("click")&&calls.includes("visibilitychange")){
      assert.equal(repairListenerEffect,null);repairListenerEffect=node.arguments[0].getText(repairShellAst);
    }
  }
  if(ts.isJsxAttribute(node)&&node.name.getText(repairShellAst)==="onLostPointerCapture"&&node.initializer&&ts.isJsxExpression(node.initializer)
    &&node.initializer.expression&&ts.isArrowFunction(node.initializer.expression)
    &&node.initializer.expression.body.getText(repairShellAst)==='handleSharedUiPointer(event, "cancel")')repairLostCapture=node.initializer.expression.getText(repairShellAst);
  ts.forEachChild(node,visitRepairShell);
}
visitRepairShell(repairShellAst);
assert.equal(repairShellDeclarations.size,repairShellNames.size);assert.ok(repairTargetRegistration);assert.ok(repairListenerEffect);assert.ok(repairLostCapture);
const repairShellJs=ts.transpileModule([...repairShellDeclarations.values()].join("\n")+
  "\nconst registerNpcRepairTarget="+repairTargetRegistration+";\nconst lostRepairCapture="+repairLostCapture+
  ";\nfunction installRepairPointerListeners(){return ("+repairListenerEffect+")();}",{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;


const repairInventorySource=readFileSync(new URL("../app/components/original-client-inventory-window.tsx",import.meta.url),"utf8");
const repairInventoryAst=ts.createSourceFile("original-client-inventory-window.tsx",repairInventorySource,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const repairInventoryDeclarations=new Map(),repairInventoryHandlers=new Map();let repairConsumedInitializer=null;
function visitRepairInventory(node){
  if(ts.isFunctionDeclaration(node)&&node.name&&["mailItemLocked","activateInventoryItem"].includes(node.name.text))repairInventoryDeclarations.set(node.name.text,node.getText(repairInventoryAst));
  if(ts.isVariableDeclaration(node)&&ts.isIdentifier(node.name)&&node.name.text==="consumedRepairPointerRef"){
    assert.ok(ts.isCallExpression(node.initializer));repairConsumedInitializer=node.initializer.arguments[0].getText(repairInventoryAst);
  }
  if(ts.isJsxOpeningElement(node)){
    const attrs=node.attributes.properties,className=attrs.find(attr=>ts.isJsxAttribute(attr)&&attr.name.getText(repairInventoryAst)==="className");
    if(className?.initializer&&ts.isStringLiteral(className.initializer)&&className.initializer.text==="inventory-item-card"){
      for(const name of ["onPointerDown","onMouseDown","onClick"]){const attr=attrs.find(attr=>ts.isJsxAttribute(attr)&&attr.name.getText(repairInventoryAst)===name);
        assert.ok(attr?.initializer&&ts.isJsxExpression(attr.initializer)&&ts.isArrowFunction(attr.initializer.expression));repairInventoryHandlers.set(name,attr.initializer.expression.getText(repairInventoryAst));}
    }
  }
  ts.forEachChild(node,visitRepairInventory);
}
visitRepairInventory(repairInventoryAst);assert.equal(repairInventoryDeclarations.size,2);assert.equal(repairInventoryHandlers.size,3);assert.ok(repairConsumedInitializer);
const repairInventoryJs=ts.transpileModule([...repairInventoryDeclarations.values()].join("\n")+
  "\nconst consumedRepairPointerRef={current:"+repairConsumedInitializer+"};\n"+
  [...repairInventoryHandlers].map(([name,text])=>"const "+name+"="+text+";").join("\n"),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
const repairInventoryEquipment=loadPureProductionDeclarations(new URL("../app/components/original-client-inventory-utils.ts",import.meta.url),["equipmentSlotForItemKey","equipmentSlotForItem"]);

check("NPC repair actual Shell captured drag terminal owns routing geometry and successor cleanup",()=>{
  for(const variant of ["normal","outside","cancel","page","rect","DPR","hidden","screen","source","pointerType","lostcapture","successor","reentrant"]){
    const owner=npcRepairOwner(),service=new NpcRepairService();service.open(owner,"repair",0.25,{});service.observeSnapshot(owner,npcRepairSnapshot());
    let view=service.view(owner,npcRepairQuote,()=>true),api,dropCount=0,cancelCount=0,fallthrough=0,stopCount=0;
    class MemoryNode{constructor(rect){this.rect=rect;this.isConnected=true;this.captures=new Set();}getBoundingClientRect(){const[left,top,width,height]=this.rect;return{left,top,width,height};}
      contains(node){return node===this||this.children?.some(child=>child.contains(node))===true;}closest(selector){return selector==="button"?(this.tagName==="BUTTON"?this:this.parentElement?.closest(selector)??null):null;}hasAttribute(name){return this.attributes?.has(name)===true;}setPointerCapture(id){this.captures.add(id);}hasPointerCapture(id){return this.captures.has(id);}releasePointerCapture(id){this.captures.delete(id);}}
    const stage=new MemoryNode([10,20,800,600]),item=new MemoryNode([30,40,32,32]),target=new MemoryNode([110,120,80,60]);stage.children=[item,target];
    const ref=current=>({current}),scope={Element:MemoryNode,Node:MemoryNode,HTMLElement:MemoryNode,window:{devicePixelRatio:2},
      stageFrameRef:ref(stage),stagePresentation:{virtualWidth:800,virtualHeight:600,scale:1},activeInventoryTab:"bag1",screen:"game",showInventory:true,
      npcRepairService:"repair",npcRepairView:view,npcRepairPointerRef:ref(null),npcRepairTargetRef:ref(null),npcRepairVisibleTargetRef:ref(null),
      npcRepairQuarantineRef:ref(new Map()),npcRepairClickFenceRef:ref(new Map()),heldScenePointerRef:ref({old:true}),heldQuestControlPointersRef:ref(new Set()),combatUiHoldRef:ref(new Map()),
      setNpcRepairTargetSelection:()=>{},onOpenInventoryTab:()=>{},onViewportDirectionStop:()=>{stopCount++;},
      beginCombatUiHold:(kind,id)=>scope.combatUiHoldRef.current.set(kind,{pointerId:id}),endCombatUiHold:(kind,hold)=>{if(scope.combatUiHoldRef.current.get(kind)===hold)scope.combatUiHoldRef.current.delete(kind);},
      onBeginNpcRepairDrag:(_view,_item,geometry)=>service.beginDrag(_view,900,7,geometry,view),
      onCancelNpcRepairDrag:drag=>{cancelCount++;service.cancelDrag(drag);},
      onDropNpcRepairDrag:(drag,geometry,x,y)=>{dropCount++;const selection=service.drop(drag,geometry,x,y,view);
        if(selection&&variant==="reentrant"){const newer=service.select(view,901,view);api.changeNpcRepairTarget(newer);
          api.beginNpcRepairPointer({pointerId:9,pointerType:"mouse",isPrimary:true,button:0,timeStamp:40,currentTarget:item,target:item,preventDefault(){},stopPropagation(){}},{});}
        return selection?{selection,submitted:false}:null;},
      handleSharedQuestWorldPointer:()=>{fallthrough++;},handleSharedNpcShopPointer:()=>{fallthrough++;return false;},handleSharedComposePointer:()=>false,
      handleSharedMailPointer:()=>false,handleSharedStoragePointer:()=>false,handleSharedSpellsPointer:()=>false,handleSharedBagPointer:()=>{fallthrough++;}};
    const keys=Object.keys(scope);api=new Function(...keys,repairShellJs+"\nreturn {beginNpcRepairPointer,handleSharedUiPointer,cancelNpcRepairPointer,registerNpcRepairTarget,changeNpcRepairTarget,lostRepairCapture,setView:value=>npcRepairView=value,setTab:value=>activeInventoryTab=value,setVisible:value=>showInventory=value,setScreen:value=>screen=value};")(...keys.map(key=>scope[key]));
    const cleanup=api.registerNpcRepairTarget(view,target),down={pointerId:7,pointerType:"mouse",isPrimary:true,button:0,timeStamp:10,currentTarget:item,target:item,clientX:111,clientY:121,
      prevented:0,preventDefault(){this.prevented++;},stopPropagation(){}};
    api.beginNpcRepairPointer(down,{});const first=scope.npcRepairPointerRef.current;assert.ok(first);assert.equal(item.hasPointerCapture(7),true);assert.equal(stopCount,1);assert.equal(scope.heldScenePointerRef.current,null);
    api.handleSharedUiPointer({...down,timeStamp:9},"up");assert.equal(scope.npcRepairPointerRef.current,first);assert.equal(dropCount,0,"older terminal cannot finish current gesture");
    if(variant==="page")api.setTab("bag2");if(variant==="rect")target.rect=[111,120,80,60];if(variant==="DPR")scope.window.devicePixelRatio=1;
    if(variant==="hidden")api.setVisible(false);if(variant==="screen")api.setScreen("select");
    if(variant==="source"){const changed=npcRepairSnapshot({gold:499});service.prepareSnapshot(owner,changed);service.observeSnapshot(owner,changed);view=service.view(owner,npcRepairQuote,()=>true);api.setView(view);}
    if(variant==="successor"){
      api.cancelNpcRepairPointer(first);api.handleSharedUiPointer({...down,timeStamp:20},"up");
      const successorTarget=new MemoryNode([110,120,80,60]);stage.children.push(successorTarget);api.registerNpcRepairTarget(view,successorTarget);
      api.beginNpcRepairPointer({...down,pointerId:9,timeStamp:30},{});const successor=scope.npcRepairPointerRef.current;assert.ok(successor);
      cleanup();api.cancelNpcRepairPointer(first);assert.equal(scope.npcRepairPointerRef.current,successor);assert.equal(scope.npcRepairTargetRef.current.node,successorTarget);
      api.cancelNpcRepairPointer(successor);assert.equal(service.drop(first.drag,first.geometry,111,121,view),null);continue;
    }
    const terminal={...down,timeStamp:20,clientX:variant==="outside"?190:111,pointerType:variant==="pointerType"?"touch":"mouse"};
    if(variant==="lostcapture")api.lostRepairCapture(terminal);else api.handleSharedUiPointer(terminal,variant==="cancel"?"cancel":"up");
    assert.equal(fallthrough,0,"repair terminals never fall through to shared/world routes");assert.equal(item.hasPointerCapture(7),false);
    if(variant==="normal")assert.equal(scope.npcRepairVisibleTargetRef.current.uniqueId,900);
    else if(variant==="reentrant"){assert.equal(scope.npcRepairVisibleTargetRef.current.uniqueId,901);assert.equal(scope.npcRepairPointerRef.current.geometry.pointerId,9);api.cancelNpcRepairPointer();}
    else assert.equal(scope.npcRepairVisibleTargetRef.current,null);
    assert.equal(service.drop(first.drag,first.geometry,111,121,view),null);assert.equal(scope.combatUiHoldRef.current.size,0);assert.equal(scope.npcRepairPointerRef.current,null);
    if(["cancel","hidden","screen","page","source","pointerType","lostcapture"].includes(variant))assert.equal(cancelCount,1);
    cleanup();
  }
});
check("NPC repair actual capture listeners quarantine second pointer through compatibility click while new gestures and keyboard remain usable",()=>{
  const owner=npcRepairOwner(),service=new NpcRepairService();service.open(owner,"repair",0.25,{});service.observeSnapshot(owner,npcRepairSnapshot());
  let view=service.view(owner,npcRepairQuote,()=>true);let api,sends=0,routeDown=0,fallthrough=0,stops=0;
  class MemoryNode{constructor(rect){this.rect=rect;this.isConnected=true;this.captures=new Set();}getBoundingClientRect(){const[left,top,width,height]=this.rect;return{left,top,width,height};}
    contains(node){return node===this||this.children?.some(child=>child.contains(node))===true;}closest(selector){return selector==="button"?(this.tagName==="BUTTON"?this:this.parentElement?.closest(selector)??null):null;}hasAttribute(name){return this.attributes?.has(name)===true;}setPointerCapture(id){this.captures.add(id);}hasPointerCapture(id){return this.captures.has(id);}releasePointerCapture(id){this.captures.delete(id);}}
  const stage=new MemoryNode([10,20,800,600]),item=new MemoryNode([30,40,32,32]),target=new MemoryNode([110,120,80,60]),confirm=new MemoryNode([210,120,80,30]);const panel=new MemoryNode([100,100,200,120]),wrapper=new MemoryNode([210,120,80,30]),holdWrapper=new MemoryNode([210,160,80,30]),holdControl=new MemoryNode([210,160,80,30]);
  confirm.tagName=holdControl.tagName="BUTTON";wrapper.attributes=holdWrapper.attributes=new Set(["data-npc-repair-control"]);
  target.parentElement=wrapper.parentElement=holdWrapper.parentElement=panel;confirm.parentElement=wrapper;holdControl.parentElement=holdWrapper;
  wrapper.children=[confirm];holdWrapper.children=[holdControl];panel.children=[target,wrapper,holdWrapper];stage.children=[item,panel];
  const windowHandlers=new Map(),documentHandlers=new Map(),registrations=[],ref=current=>({current});
  const scope={Element:MemoryNode,Node:MemoryNode,HTMLElement:MemoryNode,
    window:{devicePixelRatio:2,addEventListener:(name,handler,capture)=>{windowHandlers.set(name,handler);registrations.push([name,capture]);},removeEventListener:(name,handler)=>{assert.equal(windowHandlers.get(name),handler);windowHandlers.delete(name);}},
    document:{visibilityState:"visible",addEventListener:(name,handler)=>documentHandlers.set(name,handler),removeEventListener:(name,handler)=>{assert.equal(documentHandlers.get(name),handler);documentHandlers.delete(name);}},
    stageFrameRef:ref(stage),stagePresentation:{virtualWidth:800,virtualHeight:600,scale:1},activeInventoryTab:"bag1",screen:"game",showInventory:true,
    npcRepairService:"repair",npcRepairView:view,npcRepairPointerRef:ref(null),npcRepairTargetRef:ref(null),npcRepairVisibleTargetRef:ref(null),npcRepairQuarantineRef:ref(new Map()),npcRepairClickFenceRef:ref(new Map()),
    heldScenePointerRef:ref(null),heldQuestControlPointersRef:ref(new Set()),combatUiHoldRef:ref(new Map()),sharedBagPointerHandlerRef:ref(null),setNpcRepairTargetSelection:()=>{},onOpenInventoryTab:()=>{},onViewportDirectionStop:()=>{},
    beginCombatUiHold:(kind,id)=>scope.combatUiHoldRef.current.set(kind,{pointerId:id}),endCombatUiHold:(kind,hold)=>{if(scope.combatUiHoldRef.current.get(kind)===hold)scope.combatUiHoldRef.current.delete(kind);},
    onBeginNpcRepairDrag:(_view,_item,geometry)=>service.beginDrag(_view,901,43,{...geometry,page:"bag2"},view),
    onCancelNpcRepairDrag:drag=>service.cancelDrag(drag),onDropNpcRepairDrag:(drag,geometry,x,y)=>{const selection=service.drop(drag,geometry,x,y,view);return selection?{selection,submitted:false}:null;},
    onConfirmNpcRepair:selection=>{const proof=service.reserve(selection,view);if(!proof||!service.enter(proof,view,{...proof.command}))return false;sends++;return true;},
    handleSharedQuestWorldPointer:()=>{fallthrough++;},handleSharedNpcShopPointer:()=>{fallthrough++;return false;},handleSharedComposePointer:()=>false,handleSharedMailPointer:()=>false,
    handleSharedStoragePointer:()=>false,handleSharedSpellsPointer:()=>false,handleSharedBagPointer:()=>{fallthrough++;},
    cancelSharedNpcShopPointer:()=>{stops++;},cancelSharedStoragePointer:()=>{},cancelSharedComposePointer:()=>{},cancelSharedMailPointer:()=>{},cancelSharedSpellsPointer:()=>{},cancelSharedCharacterPointer:()=>{},cancelSharedBagPointer:()=>{},cancelSharedHudPointer:()=>{}};
  // The dragged B row is the actual Bag2 slot43; the previously selected A row
  // belongs to the same authoritative repair view and remains visible.
  scope.activeInventoryTab="bag2";
  const keys=Object.keys(scope);api=new Function(...keys,repairShellJs+"\nreturn {beginNpcRepairPointer,handleSharedUiPointer,registerNpcRepairTarget,changeNpcRepairTarget,confirmNpcRepairTarget,installRepairPointerListeners,setView:value=>npcRepairView=value};")(...keys.map(key=>scope[key]));
  scope.sharedBagPointerHandlerRef.current=(event,phase)=>{if(phase==="down")routeDown++;api.handleSharedUiPointer(event,phase);};
  api.registerNpcRepairTarget(view,target);const cleanup=api.installRepairPointerListeners();
  assert.deepEqual(registrations.filter(([name])=>["pointerdown","click","pointerup","pointercancel"].includes(name)),[["pointerdown",true],["click",true],["pointerup",true],["pointercancel",true]]);
  const event=(pointerId,timeStamp,eventTarget=item,detail=1)=>({pointerId,timeStamp,pointerType:"mouse",isPrimary:true,button:0,currentTarget:item,target:eventTarget,clientX:111,clientY:121,detail,
    prevented:false,stopped:false,preventDefault(){this.prevented=true;},stopPropagation(){this.stopped=true;},stopImmediatePropagation(){this.stopped=true;}});
  const selectedA=service.select(view,900,view);assert.ok(selectedA);api.changeNpcRepairTarget(selectedA);
  windowHandlers.get("pointerdown")(event(1,10));assert.equal(routeDown,0,"inactive canvas pointerdown is not double routed");
  api.beginNpcRepairPointer(event(1,10),{});const dragB=scope.npcRepairPointerRef.current;assert.ok(dragB);
  const second=event(2,11,confirm);windowHandlers.get("pointerdown")(second);assert.equal(second.prevented,true);assert.equal(scope.npcRepairPointerRef.current,null);assert.equal(scope.npcRepairQuarantineRef.current.has(2),true);
  const secondUp=event(2,12,confirm);windowHandlers.get("pointerup")(secondUp);assert.equal(secondUp.prevented,true);assert.equal(scope.npcRepairQuarantineRef.current.has(2),false);
  assert.equal(fallthrough,0);assert.equal(scope.npcRepairVisibleTargetRef.current,selectedA);
  const compat=event(2,13,confirm);windowHandlers.get("click")(compat);if(!compat.stopped)api.confirmNpcRepairTarget(selectedA);
  assert.equal(compat.prevented,true);assert.equal(compat.stopped,true);assert.equal(sends,0,"retired second gesture cannot Confirm the older selection");
  const spentClick=event(2,14,confirm);windowHandlers.get("click")(spentClick);assert.equal(spentClick.stopped,false,"matched compatibility click consumes its single-use fence");
  api.beginNpcRepairPointer(event(3,30),{});windowHandlers.get("pointerdown")(event(4,31,confirm));windowHandlers.get("pointerup")(event(4,32,confirm));
  const legacy=event(undefined,33,confirm);windowHandlers.get("click")(legacy);assert.equal(legacy.stopped,true,"legacy click matches the captured physical button");
  api.beginNpcRepairPointer(event(5,40),{});windowHandlers.get("pointerdown")(event(6,41,holdControl));windowHandlers.get("pointerup")(event(6,42,holdControl));
  const holdClick=event(6,43,holdControl);windowHandlers.get("click")(holdClick);if(!holdClick.stopped)service.toggleHold(view,view);
  assert.equal(holdClick.stopped,true);view=service.view(owner,npcRepairQuote,()=>true);api.setView(view);assert.equal(view.hold,false,"retired second gesture cannot toggle Hold");
  const keyboard=event(2,50,confirm,0);windowHandlers.get("click")(keyboard);assert.equal(keyboard.stopped,false);assert.equal(api.confirmNpcRepairTarget(selectedA),true);assert.equal(sends,1);
  assert.equal(service.drop(dragB.drag,dragB.geometry,111,121,view),null);
  assert.equal(service.acknowledge(owner.socket,"ItemRepaired",{uniqueId:900,currentDura:60,maxDura:100}),true);
  const newA=service.select(view,900,view);assert.ok(newA);api.changeNpcRepairTarget(newA);
  windowHandlers.get("pointerdown")(event(2,60,confirm));const freshClick=event(2,62,confirm);windowHandlers.get("click")(freshClick);
  assert.equal(freshClick.stopped,false);assert.equal(api.confirmNpcRepairTarget(newA),true);assert.equal(sends,2);
  for(const [offset,terminal] of ["blur","resize","visibilitychange","pagehide","pointercancel"].entries()){
    const id=30+offset;api.beginNpcRepairPointer(event(id,100+id),{});const lease=scope.npcRepairPointerRef.current;assert.ok(lease);
    if(terminal==="visibilitychange"){documentHandlers.get(terminal)();assert.equal(scope.npcRepairPointerRef.current,lease);scope.document.visibilityState="hidden";documentHandlers.get(terminal)();scope.document.visibilityState="visible";}
    else windowHandlers.get(terminal)(event(id,200+id));
    assert.equal(scope.npcRepairPointerRef.current,null);assert.equal(service.drop(lease.drag,lease.geometry,111,121,view),null);assert.equal(sends,2);
  }
  cleanup();assert.equal(windowHandlers.size,0);assert.equal(documentHandlers.size,0);assert.equal(scope.combatUiHoldRef.current.size,0);
  // Exercise the actual Inventory JSX handlers and activation function across a
  // synchronous repair close, rather than assuming preventDefault kills mouse.
  const activated=[],repairDowns=[],inventoryItem={key:"HealthPotion",name:"Health Potion",slot:7,container:"bag1",uniqueId:900,authoritativeUniqueId:900};
  const inventoryScope={repairMode:true,item:inventoryItem,window:{},mailLocks:[],storageMode:null,deleteMode:false,sellMode:false,pendingMoveItem:null,
    equipmentSlotForItem:repairInventoryEquipment.equipmentSlotForItem,onRepairPointerDown:event=>repairDowns.push(event.pointerId),onUseItem:value=>activated.push(value)};
  const inventoryKeys=Object.keys(inventoryScope),inventoryApi=new Function(...inventoryKeys,repairInventoryJs+
    "\nreturn {onPointerDown,onMouseDown,onClick,setRepair:value=>repairMode=value,consumedRepairPointerRef};")(...inventoryKeys.map(key=>inventoryScope[key]));
  const button={},otherButton={},inventoryEvent={pointerId:70,pointerType:"mouse",isPrimary:true,button:0,detail:1,currentTarget:button,defaultPrevented:false,preventDefault(){this.defaultPrevented=true;}};
  inventoryApi.onPointerDown({...inventoryEvent});assert.deepEqual(repairDowns,[70]);inventoryApi.setRepair(false);
  inventoryApi.onMouseDown({...inventoryEvent});assert.equal(activated.length,0,"repair down then close cannot use through compatibility mousedown");
  inventoryApi.onClick({...inventoryEvent});assert.equal(activated.length,0);
  for(const invalid of [{defaultPrevented:true},{isPrimary:false},{button:2},{pointerType:"pen"}]){
    inventoryApi.onPointerDown({...inventoryEvent,...invalid});inventoryApi.onMouseDown({...inventoryEvent});assert.equal(activated.length,0,"invalid/quarantined fresh edge cannot unlock activation");
  }
  inventoryApi.onPointerDown({...inventoryEvent,currentTarget:otherButton});inventoryApi.onMouseDown({...inventoryEvent});assert.equal(activated.length,0,"another physical button cannot unlock the old one");
  inventoryApi.onClick({...inventoryEvent,detail:0});assert.equal(activated.length,1,"independent keyboard activation is preserved after close");
  inventoryApi.onPointerDown({...inventoryEvent,pointerId:71});inventoryApi.onMouseDown({...inventoryEvent});assert.equal(activated.length,2,"a valid fresh pointer gesture restores ordinary activation");
  assert.deepEqual(activated[1],{key:"HealthPotion",uniqueId:900,authoritativeUniqueId:900,slot:7,container:"bag1"});
  inventoryApi.setRepair(true);inventoryApi.onClick({...inventoryEvent,detail:0});assert.equal(activated.length,2,"repair mode cannot borrow ordinary keyboard item use");
});

console.log(`stage5 adapter tests passed (${passed} groups)`);
