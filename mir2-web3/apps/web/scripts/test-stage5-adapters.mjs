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
const stageNpcRaw = loadTypeScriptModule(new URL("../lib/npc-purchase-client.ts", import.meta.url));
const extendedPackets = loadTypeScriptModule(new URL("../lib/extended-server-packets.ts", import.meta.url), {"./npc-purchase-client":stageNpcRaw});
const heroUi = loadTypeScriptModule(new URL("../lib/hero-player-ui.ts", import.meta.url));
const guildBuffUi = loadTypeScriptModule(new URL("../lib/guild-buff-ui.ts", import.meta.url));
const crystalItemSource = loadTypeScriptModule(new URL("../lib/crystal-item-source.ts", import.meta.url));
const sharedTooltip = loadTypeScriptModule(new URL("../lib/shared-item-tooltip.ts", import.meta.url), {"./crystal-item-source":crystalItemSource});
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
  "./social-incoming-replies": socialReplies,"./npc-purchase-client":stageNpcRaw,
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
const bagBeltDispatcherModule = loadTypeScriptModule(new URL("../lib/bag-belt-move-dispatcher.ts", import.meta.url), {"./mail-parcel-gateway-adapter":mailParcelGateway});
const bagBeltGeometryModule = loadTypeScriptModule(new URL("../lib/bag-belt-gesture.ts", import.meta.url));
const parityPageUrl = new URL("../app/page.tsx", import.meta.url);
const parityPageText = readFileSync(parityPageUrl, "utf8");
const parityPageAst = ts.createSourceFile(fileURLToPath(parityPageUrl), parityPageText, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const parityPageNames = ["currentSpellsOwner", "currentSocialReplyOwner", "currentSocialReceiveOwner", "sameSocialPhysicalOwner",
  "currentHeroModel", "parityItemMutationAllowed", "heroProofCurrent", "heroInputAllowed", "otherPlayerUiBlocksInput", "heroUiBlocksGameplay", "heroBasisWindows",
  "referenceWindowsBlockGameplay", "heroUiLeaseCurrent", "advanceHeroWindowEpochs", "changeHeroWindows", "syncHeroWindowActor", "scheduleHeroRestock", "syncObservePreference", "currentCreatureSource", "currentCashGameShopSource",
  "currentQuestWorldIdentity", "currentCombatModeOwner", "sameCombatModeOwner", "nextCombatModeRevision",
  "captureCombatModeSnapshot", "captureCombatModeReceipt", "readCombatModeSource", "retireWorldFishingGesture", "cancelWorldFishingGesture",
  "currentGuildBuffSource", "captureParityPacket", "captureParitySnapshot", "ownedItemTooltipRequest", "readPlayerItemTooltip", "readHeroItemTooltip",
  "readCashGameShopItemTooltip", "guildStorageTooltipItem", "readGuildStorageItemTooltip", "readSocialItemSurface", "readNpcRepairOwner", "captureNpcRepairDialog", "currentNpcRepairDialogSource", "readNpcRepairView", "selectNpcRepair", "toggleNpcRepairHold", "beginNpcRepairDrag", "cancelNpcRepairDrag", "dropNpcRepairDrag", "confirmNpcRepair"];
const parityDeclarations = new Map();
const ownedTooltipCaptures = new Map(); let ownedTooltipMemo, ownedTooltipCommit;
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
  if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name)) {
    if (["ownedTooltipOwner","ownedTooltipCore","ownedTooltipOwnerRevision","ownedTooltipBagEpoch","ownedTooltipCharacterEpoch"].includes(node.name.text)) {
      assert.equal(ownedTooltipCaptures.has(node.name.text),false); ownedTooltipCaptures.set(node.name.text,"const "+node.getText(parityPageAst)+";");
    }
    if (node.name.text === "ownedItemTooltipReaders") {
      assert.ok(ts.isCallExpression(node.initializer) && node.initializer.expression.getText(parityPageAst)==="useMemo");
      assert.ok(ts.isArrowFunction(node.initializer.arguments[0])); ownedTooltipMemo=node.initializer;
    }
  }
  if (ts.isCallExpression(node) && node.expression.getText(parityPageAst)==="useLayoutEffect" && ts.isArrowFunction(node.arguments[0])
    && node.arguments[0].body.getText(parityPageAst).includes("ownedTooltipReaderRef.current = ownedItemTooltipReaders")) {
    assert.equal(ownedTooltipCommit,undefined); ownedTooltipCommit=node.arguments[0].body;
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
assert.equal(ownedTooltipCaptures.size,5); assert.ok(ownedTooltipMemo && ownedTooltipCommit);
const ownedTooltipCaptureText=[...ownedTooltipCaptures.values()].join("\n")
  + "\nconst ownedItemTooltipReaders=("+ownedTooltipMemo.arguments[0].getText(parityPageAst)+")();\nreturn ownedItemTooltipReaders;";
const parityPageJs = ts.transpileModule([...parityDeclarations.values()].join("\n")
  + "\nfunction captureOwnedItemTooltipReaders(presentationWorld){"+ownedTooltipCaptureText+"\n}"
  + "\nfunction commitOwnedItemTooltipReaders(ownedItemTooltipReaders)"+ownedTooltipCommit.getText(parityPageAst)
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
    rankingInspectOpenRef:ref(false),
    bevyHpLocalOverlayOpenRef:ref(false), questReactModalOpenRef:ref(false), bagOpenRef:ref(false), characterOpenRef:ref(false), questLogOpenRef:ref(false),
    socialReplyWindowsRef:ref({group:false,bonds:false}), socialRosterWindowsRef:ref({friends:false}), rankingWindowRef:ref(false),
    marketOpenRef:ref(false), conquestOpenRef:ref(false), buffsOpenRef:ref(false), mailUiOpenRef:ref(false), worldMapOpenRef:ref(false),
    chatSettingsOpenRef:ref(false), tutorialOpenRef:ref(false), npcShopServiceRef:ref(null), npcRepairServiceRef:ref(null),
    storageServiceActiveRef:ref(false), npcShopUiIngressRef:ref(null), npcRepairAuthorityRef:ref({invalidateInventory:()=>{}}), storageUiIngressRef:ref(null), combatIngressRef:ref(null), spellsIngressRef:ref(null),
    skillBarPointerHeldRef:ref(false), heroUiProofLeasesRef:ref(new WeakMap()), heroProofHighWaterRef:ref(new WeakMap()),
    heroSharedUiIngressRef:ref(null),heroSharedProofsRef:ref(new WeakMap()),heroProofRendererTokensRef:ref(new WeakMap()),
    heroRendererTokenRef:ref("hero-react:0"),heroActionBasisMatches:heroUi.heroActionBasisMatches,
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
    bagBeltMovesRef:ref(new bagBeltDispatcherModule.BagBeltMoveDispatcher()), bagBeltInventoryReadyRef:ref(null),
    worldFishingGestureRegistryRef:ref(new WeakMap()),worldFishingActiveGestureRef:ref(null),worldFishingQueuedRef:ref(null),
    renderParityServices:()=>{}, appendLog:()=>{}, t:(_key,_args,fallback)=>fallback,
    isMailItemMutation:mailParcelGateway.isMailItemMutation,
    makeCreaturePlayerSource:creatureUi.makeCreaturePlayerSource, upsertCreaturePlayerRecord:creatureUi.upsertCreaturePlayerRecord,
    creaturePlayerCommandJson:creatureUi.creaturePlayerCommandJson,
    makeCashGameShopSource:cashShopUi.makeCashGameShopSource, emptyCashGameShopCatalog:cashShopUi.emptyCashGameShopCatalog,
    upsertCashGameShopInfo:cashShopUi.upsertCashGameShopInfo, applyCashGameShopStock:cashShopUi.applyCashGameShopStock,
    parseMailList:extendedPackets.parseMailList,
    ownedTooltipReaderRef:ref(null), ownedTooltipEpochRef:ref({bag:1,character:1}), activeCharacterTabRef:ref("char"),
    runtimeRef:ref(null), readSharedItemTooltip:sharedTooltip.readSharedItemTooltip, readSharedItemCatalogInfo:sharedTooltip.readSharedItemCatalogInfo,
    ...socialItems, socialCatalogRef:ref(new Map()), guildStorageRawRef:ref(null), guildPermissionsRef:ref(null),
    socialOwnTradeRef:ref(null), tradeLifecycleRef:ref({state:"closed",partner:""}), tradeIncarnationRef:ref(1),
  };
  const keys = Object.keys(scope), api = new Function(...keys, parityPageJs + "\nreturn {currentCombatModeOwner,readCombatModeSource,captureCombatModeSnapshot,captureCombatModeReceipt,parityIngress,parityFinal,parityReceipt,parityCollectClaim,heroProofCurrent,heroInputAllowed,referenceWindowsBlockGameplay,heroUiLeaseCurrent,changeHeroWindows,parityItemMutationAllowed,currentHeroModel,currentCreatureSource,currentCashGameShopSource,currentGuildBuffSource,captureParityPacket,captureParitySnapshot,ownedItemTooltipRequest,captureOwnedItemTooltipReaders,commitOwnedItemTooltipReaders,readPlayerItemTooltip,readHeroItemTooltip,readCashGameShopItemTooltip,guildStorageTooltipItem,readGuildStorageItemTooltip,readSocialItemSurface,readNpcRepairView,captureNpcRepairDialog,toggleNpcRepairHold,beginNpcRepairDrag,dropNpcRepairDrag,confirmNpcRepair};")(...keys.map(k=>scope[k]));
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
check("Shared tooltip JS memory getter caches exact formatter clock and request and never caches malformed ABI responses", () => {
  let calls=0; const document={broken:false,sourceComplete:true,sections:[{kind:"name",lines:[{text:"Actual",colour:"white"}]}]};
  const clocks=[]; const runtime={getMir2ItemTooltipDocument(json,clock){calls++; clocks.push(clock); assert.equal(JSON.parse(json).version,1); return JSON.stringify({version:1,ok:true,document});}};
  const item={uniqueId:501,itemIndex:10,name:"Actual",icon:1,count:1,tooltipSource:{userItem:heroItemFixture(501)}}, player={level:20};
  const first=sharedTooltip.readSharedItemTooltip(runtime,item,player,1000); assert.ok(first);
  assert.strictEqual(sharedTooltip.readSharedItemTooltip(runtime,item,player,1000),first); assert.equal(calls,1);
  const later=sharedTooltip.readSharedItemTooltip(runtime,item,player,1999); assert.deepEqual(later,first); assert.notStrictEqual(later,first); assert.equal(calls,2);
  assert.deepEqual(clocks,["621355968010000000","621355968019990000"]);
  sharedTooltip.readSharedItemTooltip(runtime,{...item,count:2},player,1999); sharedTooltip.readSharedItemTooltip(runtime,item,player,2000); assert.equal(calls,4);
  assert.deepEqual(clocks,["621355968010000000","621355968019990000","621355968019990000","621355968020000000"]);
  for (const result of ["not json",JSON.stringify({version:2,ok:true,document}),JSON.stringify({version:1,ok:false,document}),
    JSON.stringify({version:1,ok:true,document:{...document,broken:"false"}})]) {
    let reads=0; const bad={getMir2ItemTooltipDocument:()=>{reads++; return result;}};
    assert.equal(sharedTooltip.readSharedItemTooltip(bad,item,player,1000),null);
    assert.equal(sharedTooltip.readSharedItemTooltip(bad,item,player,1000),null); assert.equal(reads,2);
  }
  assert.equal(sharedTooltip.readSharedItemTooltip({getMir2ItemTooltipDocument:()=>{throw Error("fake getter");}},item,player,1000),null);
  assert.equal(sharedTooltip.readSharedItemTooltip(runtime,{...item,uniqueId:Number.MAX_SAFE_INTEGER+1},player,1000),null);
  assert.equal(sharedTooltip.readSharedItemTooltip(runtime,item,{level:NaN},1000),null); assert.equal(calls,4);
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
  for (const index of [-2147483649,1.5,2147483648,NaN,"6"]) assert.equal(sharedTooltip.readSharedItemCatalogInfo(runtime,index),null);
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
const fishingClickAdapter=loadTypeScriptModule(new URL("../lib/shared-fishing-click.ts",import.meta.url));
const rankingInspectAdapter=loadTypeScriptModule(new URL("../lib/shared-ranking-inspect.ts",import.meta.url),{"./crystal-item-source":crystalItemSource});
const pearlSourceModule=loadTypeScriptModule(new URL("../lib/npc-pearl-buy-source.ts",import.meta.url),{"./crystal-item-source":crystalItemSource});
const pearlBuyModule=loadTypeScriptModule(new URL("../lib/npc-pearl-buy.ts",import.meta.url),{"./npc-pearl-buy-source":pearlSourceModule});
const presentationRuntime = loadTypeScriptModule(new URL("../lib/client-presentation-runtime.ts", import.meta.url), {"./shared-item-tooltip":sharedTooltip,"./shared-fishing-click":fishingClickAdapter,"./npc-pearl-buy-source":pearlSourceModule,"./npc-pearl-buy":pearlBuyModule,"./shared-ranking-inspect":rankingInspectAdapter});
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
// Hero is absent in these repair fixtures. Retain its actual cleanup function
// with null gesture/hover refs so the shared blur/resize listeners stay real.
const repairShellSource=readFileSync(new URL("../app/original-client-shell.tsx",import.meta.url),"utf8");
const repairShellAst=ts.createSourceFile("original-client-shell.tsx",repairShellSource,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const repairShellNames=new Set(["repairGeometry","cancelNpcRepairPointer","beginNpcRepairPointer","handleNpcRepairPointer","fenceNpcRepairClick",
  "npcRepairControlTarget","rememberNpcRepairControlClick","handleSharedUiPointer","changeNpcRepairTarget","confirmNpcRepairTarget","openNpcRepairBagPage",
  "cancelSharedHeroPointer"]);
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
// Existing repair fixtures have no Bag-to-Belt gesture. Supply inactive custody
// records while retaining the real newly shared event routes and click fences.
const inactiveBagBeltNames=new Set(["rememberBagBeltClick","fenceBagBeltMouse","cancelBagBeltPointer","handleBagBeltPointer","finishBagBeltPointer"]),inactiveBagBeltDeclarations=[];
(function visit(node){if(ts.isFunctionDeclaration(node)&&inactiveBagBeltNames.has(node.name?.text))inactiveBagBeltDeclarations.push(node.getText(repairShellAst));ts.forEachChild(node,visit);})(repairShellAst);
assert.equal(inactiveBagBeltDeclarations.length,inactiveBagBeltNames.size);

const inactiveWorldFishingNames=new Set(["cancelWorldFishingHeldPointer","retireWorldFishingPhysical"]),inactiveWorldFishingDeclarations=[],inactiveWorldFishingRefs=new Map();
(function visit(node){if(ts.isFunctionDeclaration(node)&&inactiveWorldFishingNames.has(node.name?.text))inactiveWorldFishingDeclarations.push(node.getText(repairShellAst));
  if(ts.isVariableDeclaration(node)&&["worldFishingPhysicalRef","worldFishingTerminalRef"].includes(node.name.getText(repairShellAst))){assert.ok(ts.isCallExpression(node.initializer)&&node.initializer.expression.getText(repairShellAst)==="useRef");inactiveWorldFishingRefs.set(node.name.getText(repairShellAst),node.initializer.arguments[0].getText(repairShellAst));}ts.forEachChild(node,visit);})(repairShellAst);
assert.equal(inactiveWorldFishingDeclarations.length,inactiveWorldFishingNames.size);assert.equal(inactiveWorldFishingRefs.size,2);
const inactiveWorldFishingText=inactiveWorldFishingDeclarations.join("\n")+"\n"+[...inactiveWorldFishingRefs].map(([name,initializer])=>"const "+name+"={current:"+initializer+"};").join("\n");

const repairShellJs=ts.transpileModule([...repairShellDeclarations.values(),...inactiveBagBeltDeclarations,inactiveWorldFishingText].join("\n")+
  "\nconst bagBeltPointerRef={current:null},bagBeltArmedSharedRef={current:null},bagBeltQuarantineRef={current:new Map()},bagBeltClickFenceRef={current:new Map()},bagBeltGeometryRef={current:null},bagBeltButtonsRef={current:null},bagBeltCallbacksRef={current:{}};\n"+
  "\nconst registerNpcRepairTarget="+repairTargetRegistration+";\nconst lostRepairCapture="+repairLostCapture+
  ";\nfunction installRepairPointerListeners(){return ("+repairListenerEffect+")();}",{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;


const tooltipPanelsUrl=new URL("../app/components/original-client-panels.tsx",import.meta.url);
const tooltipPanelsText=readFileSync(tooltipPanelsUrl,"utf8");
const tooltipPanelsAst=ts.createSourceFile(fileURLToPath(tooltipPanelsUrl),tooltipPanelsText,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const tooltipHookNodes=tooltipPanelsAst.statements.filter(node=>ts.isFunctionDeclaration(node)&&node.name?.text==="useActiveItemTooltip");
assert.equal(tooltipHookNodes.length,1,"sole production active tooltip hook");
const tooltipHookJs=ts.transpileModule(tooltipHookNodes[0].getText(tooltipPanelsAst).replace(/^export /,""),
  {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;

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
      handleSharedMailPointer:()=>false,handleSharedStoragePointer:()=>false,handleSharedSpellsPointer:()=>false,heroPointerLeaseRef:{current:null},heroHoverContextRef:{current:null},handleSharedHeroPointer:()=>false,handleSharedBagPointer:()=>{fallthrough++;}};
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
    handleSharedStoragePointer:()=>false,handleSharedSpellsPointer:()=>false,heroPointerLeaseRef:{current:null},heroHoverContextRef:{current:null},handleSharedHeroPointer:()=>false,handleSharedBagPointer:()=>{fallthrough++;},
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
  // The existing action fixture has no optional tooltip reader. Install the
  // genuine production hook, so its new touch branch never borrows a stub gate.
  const inventoryTooltipScope={useRef:current=>({current}),useState:initial=>[initial,()=>{}],useEffect:callback=>{assert.equal(callback(),undefined);},window:{},document:{visibilityState:"visible"}};
  const inventoryTooltipKeys=Object.keys(inventoryTooltipScope),inventoryTooltip=new Function(...inventoryTooltipKeys,tooltipHookJs+"\nreturn useActiveItemTooltip;")(...inventoryTooltipKeys.map(key=>inventoryTooltipScope[key]))([inventoryItem]);
  const inventoryScope={repairMode:true,item:inventoryItem,tooltip:inventoryTooltip,window:{},mailLocks:[],storageMode:null,deleteMode:false,sellMode:false,pendingMoveItem:null,
    activeTab:"bag1",bagBeltActivationGenerationRef:{current:1},pendingSplitItem:null,pendingGoldDrop:null,contextMenu:null,onBagBeltPointerDown:undefined,
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



// Source21 optional tooltip ABI: actual production modules and actual Page/DOM
// functions, with memory getters and event/timer records only. No WASM instance.
const tooltipDocumentFixture={broken:false,sourceComplete:true,sections:[{kind:"name",lines:[{text:"Owned",colour:"white"}]}]};
const tooltipCoreUrl=new URL("../lib/client-core-runtime.ts",import.meta.url);
const tooltipCoreAst=ts.createSourceFile(fileURLToPath(tooltipCoreUrl),readFileSync(tooltipCoreUrl,"utf8"),ts.ScriptTarget.Latest,true);
const tooltipFacadeNodes=[];
(function visit(node){if(ts.isMethodDeclaration(node)&&node.name.getText(tooltipCoreAst)==="readItemTooltip")tooltipFacadeNodes.push(node);ts.forEachChild(node,visit);})(tooltipCoreAst);
assert.equal(tooltipFacadeNodes.length,1,"sole actual Core presentation reader proxy");
const tooltipFacadeJs=ts.transpileModule("const facade={"+tooltipFacadeNodes[0].getText(tooltipCoreAst)+"};",
  {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;

check("PUI optional item formatter preserves capability independence exact clock cache and ABI getter custody",()=>{
  const item={uniqueId:0,itemIndex:-2147483648,name:"Owned",icon:1,count:2,tooltipSource:{userItem:heroItemFixture(0,-2147483648,2)}},player={level:20};
  let calls=0;const args=[];
  const module={item_tooltip_abi_version:()=>1,item_tooltip_document(json,clock){calls++;args.push([JSON.parse(json),clock]);return JSON.stringify({version:1,ok:true,document:tooltipDocumentFixture});}};
  for(const absent of [{},{item_tooltip_abi_version:()=>1},{item_tooltip_document:module.item_tooltip_document},{...module,item_tooltip_abi_version:()=>0}])
    assert.equal(presentationRuntime.readSharedPresentationItemTooltip(absent,item,player,1000),null);
  assert.equal(calls,0);
  assert.deepEqual(presentationRuntime.searchSharedMapRoute({getMir2MapRouteVersion:()=>1,getMir2MapRoutePlan:()=>Int32Array.of(0)},
    {width:1,height:1,origin:{x:0,y:0},goal:{x:0,y:0},edges:new Uint8Array(1)}),{status:"ok",steps:[]},"an older module without item formatting still exposes its existing map capability");
  const first=presentationRuntime.readSharedPresentationItemTooltip(module,item,player,1000);assert.ok(first);
  assert.strictEqual(presentationRuntime.readSharedPresentationItemTooltip(module,item,player,1000),first);assert.equal(calls,1,"stable module adapter preserves shared cache");
  assert.deepEqual(Object.keys(args[0][0]).sort(),["item","player","version"]);assert.equal(args[0][0].item.uniqueId,0);assert.equal(args[0][0].item.itemIndex,-2147483648);
  assert.equal(args[0][1],"621355968010000000");
  presentationRuntime.readSharedPresentationItemTooltip(module,item,player,1001);assert.equal(calls,2);assert.equal(args[1][1],"621355968010010000");
  for(const sourceKind of ["catalogInstance","catalogPreview","preview",null])assert.equal(presentationRuntime.readSharedPresentationItemTooltip(module,{...item,sourceKind},player,1000),null);
  assert.equal(calls,2);
  assert.ok(presentationRuntime.readSharedPresentationItemTooltip(module,{...item,sourceKind:"instance"},player,1000));assert.equal(calls,3);
  const getter=module.item_tooltip_document,abi=module.item_tooltip_abi_version;
  module.item_tooltip_document=function(json,clock){calls++;return JSON.stringify({version:1,ok:true,document:{...tooltipDocumentFixture,sourceComplete:false}});};
  assert.equal(presentationRuntime.readSharedPresentationItemTooltip(module,item,player,1000).sourceComplete,false);assert.equal(calls,4);
  module.item_tooltip_abi_version=()=>1;assert.ok(presentationRuntime.readSharedPresentationItemTooltip(module,item,player,1000));assert.equal(calls,5,"ABI function identity also retires adapter cache");
  assert.ok(presentationRuntime.readSharedPresentationItemTooltip({...module},item,player,1000));assert.equal(calls,6,"a different module never inherits another module cache");
  module.item_tooltip_document=function(){module.item_tooltip_document=getter;return JSON.stringify({version:1,ok:true,document:tooltipDocumentFixture});};
  assert.equal(presentationRuntime.readSharedPresentationItemTooltip(module,item,player,1000),null,"getter reentry cannot publish through changed module");
  module.item_tooltip_abi_version=abi;module.item_tooltip_document=function(){module.item_tooltip_abi_version=()=>0;return JSON.stringify({version:1,ok:true,document:tooltipDocumentFixture});};
  assert.equal(presentationRuntime.readSharedPresentationItemTooltip(module,item,player,1000),null);
});
check("Shared item clock uses exact canonical i64 ticks and bounded caches while signed catalog indexes remain real",()=>{
  assert.equal(sharedTooltip.crystalTooltipDotnetTicks(0),"621355968000000000");assert.equal(sharedTooltip.crystalTooltipDotnetTicks(-1),"621355967999990000");
  for(const clock of [NaN,Infinity,0.5,Number.MAX_SAFE_INTEGER,Number.MIN_SAFE_INTEGER])assert.equal(sharedTooltip.crystalTooltipDotnetTicks(clock),null);
  const item={uniqueId:0,itemIndex:-1,name:"Owned",icon:1,count:1},player={level:1};let reads=0;
  const runtime={getMir2ItemTooltipDocument(json,clock){reads++;assert.match(clock,/^-?(0|[1-9][0-9]*)$/);return JSON.stringify({version:1,ok:true,document:tooltipDocumentFixture});}};
  for(let clock=0;clock<17;clock++)assert.ok(sharedTooltip.readSharedItemTooltip(runtime,item,player,clock));
  assert.equal(reads,17);sharedTooltip.readSharedItemTooltip(runtime,item,player,16);assert.equal(reads,17);sharedTooltip.readSharedItemTooltip(runtime,item,player,0);assert.equal(reads,18,"only sixteen exact clocks are retained");
  for(const index of [-2147483649,2147483648,NaN,0.5])assert.equal(sharedTooltip.readSharedItemTooltip(runtime,{...item,itemIndex:index},player,0),null);
  let catalogReads=0;const catalog={getMir2ItemCatalogInfo(json){catalogReads++;const request=JSON.parse(json);assert.equal(request.itemIndex,-1);return JSON.stringify({version:1,ok:true,itemInfo:heroInfoFixture(-1)});}};
  assert.equal(sharedTooltip.readSharedItemCatalogInfo(catalog,-1).itemIndex,-1);assert.equal(catalogReads,1);
  for(const length of [257,512])assert.ok(sharedTooltip.readSharedItemTooltip(runtime,{...item,name:"a".repeat(length)},player,40+length));
  assert.equal(sharedTooltip.readSharedItemTooltip(runtime,{...item,name:"a".repeat(513)},player,1000),null);
  const changed={...item};const reentrant={getMir2ItemTooltipDocument(){changed.count=2;return JSON.stringify({version:1,ok:true,document:tooltipDocumentFixture});}};
  assert.equal(sharedTooltip.readSharedItemTooltip(reentrant,changed,player,0),null,"mutable input is checked after getter entry");
});
check("Core actual item reader facade proxies only presentation and leaves missing optional capabilities usable",()=>{
  let calls=0;const presentation={item_tooltip_abi_version:()=>1,item_tooltip_document(json,clock){calls++;assert.equal(clock,"621355968010000000");assert.equal(JSON.parse(json).item.itemIndex,-2);return JSON.stringify({version:1,ok:true,document:tooltipDocumentFixture});}};
  const facade=new Function("presentation","readSharedPresentationItemTooltip",tooltipFacadeJs+"\nreturn facade;")(presentation,presentationRuntime.readSharedPresentationItemTooltip);
  assert.ok(facade.readItemTooltip({uniqueId:0,itemIndex:-2,name:"Owned",icon:0,count:1},{level:1},1000));assert.equal(calls,1);
  delete presentation.item_tooltip_document;assert.equal(facade.readItemTooltip({uniqueId:0,itemIndex:-2,name:"Owned",icon:0,count:1},{level:1},1000),null);assert.equal(calls,1);
  const missing=new Function("presentation","readSharedPresentationItemTooltip",tooltipFacadeJs+"\nreturn facade;")({},presentationRuntime.readSharedPresentationItemTooltip);
  assert.equal(missing.readItemTooltip({uniqueId:0,itemIndex:0,name:"Owned",icon:0,count:1},{level:1},1000),null);
});
check("Page owned instance request preserves UID zero signed index unit sale value count and independent raw carrier",()=>{
  const owner=npcRepairOwner(),page=parityPageFixture(owner);
  const raw={...heroWorldRowFixture(heroItemFixture(0,-1,3),2),authoritativeUniqueId:0,key:"Owned",grade:"rare",description:"Raw",sellValue:7,
    durabilityCurrent:10,durabilityMax:10,attack:2,defence:3,addedAttack:4,addedDefence:5,addedLuck:6,socketSlots:1};
  const request=page.api.ownedItemTooltipRequest(raw);assert.ok(request);assert.equal(request.uniqueId,0);assert.equal(request.itemIndex,-1);assert.equal(request.count,3);
  assert.equal(request.legacy.sellValue,7,"unit sale value is not multiplied in TS");assert.equal(request.legacy.addedLuck,6);assert.equal(request.legacy.socketSlots,1);
  assert.deepEqual(request.tooltipSource,raw.tooltipSource);assert.notStrictEqual(request.tooltipSource,raw.tooltipSource);assert.notStrictEqual(request.tooltipSource.userItem,raw.tooltipSource.userItem);
  for(const alter of [row=>delete row.authoritativeUniqueId,row=>row.authoritativeUniqueId=Number.MAX_SAFE_INTEGER+1,row=>row.uniqueId=9,
    row=>{delete row.tooltipSource.userItem.item_index;},row=>row.tooltipSource.userItem.unique_id=9,row=>{delete row.tooltipSource.userItem.unique_id;},row=>row.tooltipSource.info.item_index=2,
    row=>row.quantity=0,row=>row.quantity=65536,row=>row.quantity=2,row=>row.durabilityCurrent=9,row=>row.durabilityMax=11,
    row=>row.tooltipSource.userItem=[],row=>row.tooltipSource.info=[]]){
    const changed=structuredClone(raw);alter(changed);assert.equal(page.api.ownedItemTooltipRequest(changed),null);
  }
  const legacy=structuredClone(raw);delete legacy.sellValue;assert.equal(Object.hasOwn(page.api.ownedItemTooltipRequest(legacy).legacy,"sellValue"),false);
  raw.tooltipSource.userItem.count=4;assert.equal(request.tooltipSource.userItem.count,3,"formatter request owns an independent source copy");
});
check("Page actual owned readers bind displayed Bag Belt Equipment objects and commit source before any format read",()=>{
  const owner=npcRepairOwner(),bag={...heroWorldRowFixture(heroItemFixture(0,-1,3),2),authoritativeUniqueId:0,sellValue:7,durabilityCurrent:10,durabilityMax:10},
    belt={...heroWorldRowFixture(heroItemFixture(4,5),0,"belt"),authoritativeUniqueId:4},equipment={...heroWorldRowFixture(heroItemFixture(5,6),"weapon"),authoritativeUniqueId:5};delete equipment.container;
  const page=parityPageFixture(owner,{inventoryItems:[bag],beltItems:[belt],equipmentItems:[equipment],entities:[{objectId:owner.playerObjectId,level:33,classKey:"Wizard",genderKey:"Female"}],playerCrystalStats:[{stat:5,value:9}],playerWeights:[1,2,3],currentWeight:1,maxWeight:2});
  page.scope.bagOpenRef.current=true;page.scope.characterOpenRef.current=true;let calls=0;const requests=[];
  page.scope.questCoreRuntimeRef.current={readItemTooltip(item,player){calls++;requests.push({item,player});return tooltipDocumentFixture;}};
  const readers=page.api.captureOwnedItemTooltipReaders(page.scope.worldRef.current);
  assert.equal(readers.readItem(bag),null,"uncommitted render callback cannot read");const cleanup=page.api.commitOwnedItemTooltipReaders(readers);
  assert.strictEqual(readers.readItem(bag),tooltipDocumentFixture);assert.strictEqual(readers.readItem(belt),tooltipDocumentFixture);assert.strictEqual(readers.readEquipment(equipment),tooltipDocumentFixture);assert.equal(calls,3);
  assert.equal(requests[0].item.uniqueId,0);assert.equal(requests[0].item.itemIndex,-1);assert.equal(requests[0].item.count,3);assert.equal(requests[0].item.legacy.sellValue,7);
  assert.deepEqual(requests[0].player,{level:33,className:"Wizard",gender:"Female",crystalStats:[{stat:5,value:9}],weights:[1,2,3],currentWeightKnown:true,currentWeight:1,maxWeight:2});
  assert.equal(readers.readItem({...bag}),null);assert.equal(readers.readEquipment({...equipment}),null);assert.equal(readers.readEquipment(bag),null);
  page.scope.worldRef.current.position=[9,9];assert.ok(readers.readItem(bag),"movement alone does not retire an unchanged source");
  page.scope.bagOpenRef.current=false;assert.equal(readers.readItem(bag),null);assert.ok(readers.readItem(belt),"belt reader is independent of Bag visibility");
  page.scope.bagOpenRef.current=true;page.scope.ownedTooltipEpochRef.current.bag++;assert.equal(readers.readItem(bag),null,"close reopen never revives old window callback");
  const successor=page.api.captureOwnedItemTooltipReaders(page.scope.worldRef.current),successorCleanup=page.api.commitOwnedItemTooltipReaders(successor);
  cleanup();assert.strictEqual(page.scope.ownedTooltipReaderRef.current,successor,"old layout cleanup cannot erase successor commit");assert.ok(successor.readItem(bag));assert.equal(readers.readItem(belt),null);
  successorCleanup();assert.equal(successor.readItem(bag),null);assert.equal(page.sent.length,0,"tooltip paths never send actions");
});
check("Page actual owned readers reject physical owner source visibility and reentrant changes before and after Core",()=>{
  const variants=["socket","connection","session","player","scene","map","screen","hidden","suspend","core","bagOwner","bagEpoch","characterEpoch","bagClosed","characterClosed","bagTab","characterTab","rawCount","rawDura","rawSource","rawSlot","duplicateUid","duplicateSlot","liveReplacement","viewer","committed"];
  for(const timing of ["before","during"])for(const variant of variants){
    const owner=npcRepairOwner(),bag={...heroWorldRowFixture(heroItemFixture(0,-3,2),2),authoritativeUniqueId:0,durabilityCurrent:10,durabilityMax:10},
      equipment={...heroWorldRowFixture(heroItemFixture(5,6),"weapon"),authoritativeUniqueId:5};delete equipment.container;
    const page=parityPageFixture(owner,{inventoryItems:[bag],beltItems:[],equipmentItems:[equipment],entities:[{objectId:owner.playerObjectId,level:30,classKey:"Warrior",genderKey:"Male"}]});
    page.scope.bagOpenRef.current=true;page.scope.characterOpenRef.current=true;let calls=0;
    const equipped=["characterEpoch","characterClosed","characterTab"].includes(variant),row=equipped?equipment:bag;
    const mutate=()=>{
      if(variant==="socket")page.scope.socketRef.current={readyState:1};if(variant==="connection")page.scope.equipmentConnectionGenerationRef.current++;
      if(variant==="session")page.scope.equipmentSessionGenerationRef.current++;if(variant==="player")page.scope.worldRef.current.playerObjectId++;
      if(variant==="scene")page.scope.socialSceneRevisionRef.current++;if(variant==="map")page.scope.worldRef.current.mapFileName="other";
      if(variant==="screen")page.scope.screenRef.current="login";if(variant==="hidden")page.scope.document.visibilityState="hidden";
      if(variant==="suspend")page.scope.equipmentHostSuspendReasonRef.current="blur";
      if(variant==="core")page.scope.questCoreRuntimeRef.current={readItemTooltip:()=>tooltipDocumentFixture};if(variant==="bagOwner")page.scope.equipmentBagOwnerRef.current.ownerRevision++;
      if(variant==="bagEpoch")page.scope.ownedTooltipEpochRef.current.bag++;if(variant==="characterEpoch")page.scope.ownedTooltipEpochRef.current.character++;
      if(variant==="bagClosed")page.scope.bagOpenRef.current=false;if(variant==="characterClosed")page.scope.characterOpenRef.current=false;
      if(variant==="bagTab")page.scope.activeInventoryTabRef.current="bag2";if(variant==="characterTab")page.scope.activeCharacterTabRef.current="state";
      if(variant==="rawCount")row.quantity++;if(variant==="rawDura")row.durabilityCurrent=9;if(variant==="rawSource")row.tooltipSource.userItem.current_dura=9;
      if(variant==="rawSlot")row.slot=7;if(variant==="duplicateUid")page.scope.worldRef.current.beltItems.push({...bag,container:"belt",slot:0});
      if(variant==="duplicateSlot")page.scope.worldRef.current.inventoryItems.push({...bag,authoritativeUniqueId:9});
      if(variant==="liveReplacement")page.scope.worldRef.current.inventoryItems=[{...bag}];if(variant==="viewer")page.scope.worldRef.current.entities[0].level++;
      if(variant==="committed")page.scope.ownedTooltipReaderRef.current={};
    };
    page.scope.questCoreRuntimeRef.current={readItemTooltip(){calls++;if(timing==="during")mutate();return tooltipDocumentFixture;}};
    const readers=page.api.captureOwnedItemTooltipReaders(page.scope.worldRef.current);page.api.commitOwnedItemTooltipReaders(readers);
    // Viewer values are intentionally captured at each click, rather than bound
    // to render. Only a change during Core entry invalidates their exact request.
    if(timing==="before"&&variant==="viewer"){mutate();assert.ok(readers.readItem(bag));assert.equal(calls,1);continue;}
    if(timing==="before")mutate();assert.equal((equipped?readers.readEquipment:readers.readItem)(row),null,variant+" "+timing);
    assert.equal(calls,timing==="before"?0:1,variant+" exact entry count");assert.equal(page.sent.length,0);
  }
});

const tooltipSurfaceNodes=new Map();
for(const [surface,url,componentName,className] of [
  ["bag",new URL("../app/components/original-client-inventory-window.tsx",import.meta.url),"InventoryWindow","inventory-item-card"],
  ["belt",tooltipPanelsUrl,"BeltDialog","belt-item"],
  ["equipment",new URL("../app/components/original-client-character-window.tsx",import.meta.url),"CharacterWindow","character-slot-card"]]){
  const text=readFileSync(url,"utf8"),ast=ts.createSourceFile(fileURLToPath(url),text,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),cards=[];
  const components=ast.statements.filter(node=>ts.isFunctionDeclaration(node)&&node.name?.text===componentName);
  assert.equal(components.length,1,surface+" sole actual component declaration");
  assert.ok(components[0].body);
  (function visit(node){if(ts.isJsxOpeningElement(node)&&node.tagName.getText(ast)==="button"){
    const attr=node.attributes.properties.find(a=>ts.isJsxAttribute(a)&&a.name.getText(ast)==="className");
    const initializer=attr?.initializer;
    const expression=initializer&&ts.isJsxExpression(initializer)?initializer.expression:initializer;
    const classPrefix=expression&&(ts.isStringLiteral(expression)||ts.isNoSubstitutionTemplateLiteral(expression))?expression.text
      :expression&&ts.isTemplateExpression(expression)?expression.head.text:null;
    if(classPrefix?.trim().split(/\s+/).includes(className))cards.push(node);
  }ts.forEachChild(node,visit);})(components[0].body);
  assert.equal(cards.length,1,surface+" sole actual item card");const handlers=new Map();
  for(const name of ["onPointerEnter","onPointerLeave","onFocus","onBlur","onPointerCancel","onPointerDown"]){
    const attr=cards[0].attributes.properties.find(a=>ts.isJsxAttribute(a)&&a.name.getText(ast)===name);
    assert.ok(attr?.initializer&&ts.isJsxExpression(attr.initializer)&&ts.isArrowFunction(attr.initializer.expression),surface+" "+name);
    handlers.set(name,attr.initializer.expression.getText(ast));
  }
  const js=ts.transpileModule([...handlers].map(([name,text])=>"const "+name+"="+text+";").join("\n"),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
  tooltipSurfaceNodes.set(surface,{ast,cards,handlers,js});
}
check("Actual Bag Belt Equipment tooltip events are read only and active timer cleans stale source and successor safely",()=>{
  let currentItems=[],reader,visible=true,clock=1000,reads=0,actions=0,nowApi;
  const slots=[],effects=[],pendingEffects=[],timers=new Map(),windowHandlers=new Map(),documentHandlers=new Map();let cursor=0,nextTimer=0;
  const scope={
    useRef(initial){const index=cursor++;return slots[index]??(slots[index]={current:initial});},
    useState(initial){const index=cursor++;if(!(index in slots))slots[index]=initial;return[slots[index],value=>{slots[index]=typeof value==="function"?value(slots[index]):value;}];},
    useEffect(callback,deps){const index=cursor++,old=effects[index];if(!old||deps.some((value,i)=>!Object.is(value,old.deps[i])))pendingEffects.push({index,callback,deps});},
    window:{setInterval(callback,delay){assert.equal(delay,1000);const id=++nextTimer;timers.set(id,callback);return id;},clearInterval:id=>timers.delete(id),
      addEventListener:(name,fn)=>windowHandlers.set(name,fn),removeEventListener:(name,fn)=>{if(windowHandlers.get(name)===fn)windowHandlers.delete(name);}},
    document:{visibilityState:"visible",activeElement:null,addEventListener:(name,fn)=>documentHandlers.set(name,fn),removeEventListener:(name,fn)=>{if(documentHandlers.get(name)===fn)documentHandlers.delete(name);}},
  };
  const keys=Object.keys(scope),hook=new Function(...keys,tooltipHookJs+"\nreturn useActiveItemTooltip;")(...keys.map(key=>scope[key]));
  const render=()=>{cursor=0;nowApi=hook(currentItems,reader,visible);for(const effect of pendingEffects.splice(0)){effects[effect.index]?.cleanup?.();effects[effect.index]={deps:effect.deps,cleanup:effect.callback()};}return nowApi;};
  const module={item_tooltip_abi_version:()=>1,item_tooltip_document(_json,ticks){reads++;assert.equal(ticks,sharedTooltip.crystalTooltipDotnetTicks(clock));return JSON.stringify({version:1,ok:true,document:tooltipDocumentFixture});}};
  reader=item=>presentationRuntime.readSharedPresentationItemTooltip(module,item,{level:1},clock);
  const originalReader=reader;
  for(const surface of ["bag","belt","equipment"]){
    const item={uniqueId:0,itemIndex:-1,name:"Owned",icon:1,count:1},node={isConnected:true,matches:()=>false};currentItems=[item];visible=true;reader=originalReader;render();
    const eventScope={item,tooltip:nowApi,document:scope.document,repairMode:false,consumedRepairPointerRef:{current:new WeakSet()},onRepairPointerDown:()=>{actions++;},
      activeTab:"bag1",bagBeltActivationGenerationRef:{current:1},deleteMode:false,sellMode:false,storageMode:null,pendingMoveItem:null,pendingSplitItem:null,pendingGoldDrop:null,contextMenu:null,
      mailItemLocked:new Function("mailLocks",ts.transpileModule(repairInventoryDeclarations.get("mailItemLocked"),{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText+"\nreturn mailItemLocked;")([]),onBagBeltPointerDown:undefined};
    const eventKeys=Object.keys(eventScope),handlers=new Function(...eventKeys,tooltipSurfaceNodes.get(surface).js+"\nreturn {onPointerEnter,onPointerLeave,onFocus,onBlur,onPointerCancel,onPointerDown};")(...eventKeys.map(key=>eventScope[key]));
    const event={pointerType:"mouse",isPrimary:true,button:0,currentTarget:node,defaultPrevented:false,stopped:false,stopPropagation(){this.stopped=true;},preventDefault(){this.defaultPrevented=true;}};
    const before=reads;handlers.onPointerEnter({...event,pointerType:"touch"});render();assert.equal(reads,before,"touch enter alone is not an item read");
    handlers.onPointerEnter(event);render();assert.equal(reads,before+1);assert.ok(nowApi.document(item));assert.equal(timers.size,1);
    clock++;[...timers.values()][0]();render();assert.equal(reads,before+2);assert.ok(nowApi.document(item));
    scope.document.activeElement=node;handlers.onPointerLeave(event);render();assert.ok(nowApi.document(item),"focus keeps current item after mouse leave");
    handlers.onFocus(event);render();assert.equal(timers.size,1,"successor activity keeps one timer");
    const staleTimer=[...timers.values()][0];scope.document.activeElement=null;handlers.onBlur(event);render();assert.equal(timers.size,0);assert.equal(nowApi.document(item),null);
    staleTimer();render();assert.equal(nowApi.document(item),null,"retired timer cannot recreate a document");
    const touch={...event,pointerType:"touch"};handlers.onPointerDown(touch);render();assert.equal(touch.stopped,true);assert.ok(nowApi.document(item));assert.equal(actions,0,"hover focus touch tooltip path does not use equip repair or send");
    handlers.onPointerLeave(touch);render();assert.equal(timers.size,0);assert.equal(nowApi.document(item),null);
    handlers.onFocus(event);render();handlers.onPointerCancel(event);render();assert.equal(nowApi.document(item),null);
    for(const terminal of ["blur","resize","pointercancel","pagehide","hidden"]){
      handlers.onFocus(event);render();assert.equal(timers.size,1);
      if(terminal==="hidden"){scope.document.visibilityState="hidden";documentHandlers.get("visibilitychange")();}
      else windowHandlers.get(terminal)();render();assert.equal(timers.size,0);assert.equal(nowApi.document(item),null);scope.document.visibilityState="visible";
    }
    handlers.onFocus(event);render();const oldTimer=[...timers.values()][0];reader=value=>originalReader(value);render();
    assert.equal(nowApi.document(item),null);assert.equal(timers.size,0);const retiredReads=reads;oldTimer();render();assert.equal(reads,retiredReads,"old captured reader cannot borrow new callback");
    nowApi.activate(item,node);render();assert.ok(nowApi.document(item));currentItems=[{...item}];render();assert.equal(nowApi.document(item),null);assert.equal(timers.size,0);
    currentItems=[item];render();assert.equal(nowApi.document(item),null,"same item returned after retirement does not resume itself");
    nowApi.activate(item,node);render();node.isConnected=false;clock++;const disconnectedReads=reads;[...timers.values()][0]();render();assert.equal(reads,disconnectedReads);assert.equal(nowApi.document(item),null);
    visible=false;render();assert.equal(timers.size,0);assert.equal(windowHandlers.size,0);assert.equal(documentHandlers.size,0);
  }
  for(const effect of effects)effect?.cleanup?.();assert.equal(timers.size,0);assert.equal(actions,0);
});
check("Three actual tooltip mounts carry exact reader props document and original fallback without raw bulk actions",()=>{
  const sceneUrl=new URL("../app/components/original-client-game-ui-scene.tsx",import.meta.url),sceneAst=ts.createSourceFile(fileURLToPath(sceneUrl),readFileSync(sceneUrl,"utf8"),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),mounts=[];
  (function visit(node){if(ts.isJsxSelfClosingElement(node)&&["InventoryWindow","BeltDialog","CharacterWindow"].includes(node.tagName.getText(sceneAst)))mounts.push(node);ts.forEachChild(node,visit);})(sceneAst);
  const expected=new Map([["InventoryWindow",["onReadItemTooltip","onReadItemTooltip"]],["BeltDialog",["onReadItemTooltip","onReadItemTooltip"]],["CharacterWindow",["onReadEquipmentItemTooltip","onReadEquipmentItemTooltip"]]]);
  for(const [component,[prop,reader]]of expected){
    const matches=mounts.filter(node=>node.tagName.getText(sceneAst)===component);assert.equal(matches.length,1,component+" actual mount");
    const attr=matches[0].attributes.properties.find(a=>ts.isJsxAttribute(a)&&a.name.getText(sceneAst)===prop);
    assert.ok(attr?.initializer&&ts.isJsxExpression(attr.initializer));assert.equal(attr.initializer.expression.getText(sceneAst),reader);
  }
  const shellTooltipUrl=new URL("../app/original-client-shell.tsx",import.meta.url),shellAst=ts.createSourceFile(fileURLToPath(shellTooltipUrl),readFileSync(shellTooltipUrl,"utf8"),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),sceneMounts=[];let sharedProps;
  (function visit(node){if(ts.isJsxSelfClosingElement(node)&&["GameUiScene","GameUiSceneStoreBound"].includes(node.tagName.getText(shellAst)))sceneMounts.push(node);
    if(ts.isVariableDeclaration(node)&&node.name.getText(shellAst)==="gameUiSharedProps")sharedProps=node.initializer;ts.forEachChild(node,visit);})(shellAst);
  assert.equal(sceneMounts.length,2);assert.ok(sharedProps&&ts.isObjectLiteralExpression(sharedProps));
  for(const mount of sceneMounts){const spread=mount.attributes.properties.find(ts.isJsxSpreadAttribute);assert.ok(spread);assert.equal(spread.expression.getText(shellAst),"gameUiSharedProps");}
  for(const prop of ["onReadItemTooltip","onReadEquipmentItemTooltip"]){const member=sharedProps.properties.find(p=>p.name?.getText(shellAst)===prop);assert.ok(member&&ts.isShorthandPropertyAssignment(member));}
  for(const {ast,cards}of tooltipSurfaceNodes.values()){
    const card=cards[0].parent;assert.ok(ts.isJsxElement(card));const renderer=[],fallback=[];
    (function visit(node){if(ts.isJsxSelfClosingElement(node)){if(node.tagName.getText(ast)==="OriginalCrystalItemTooltip")renderer.push(node);if(node.tagName.getText(ast)==="OriginalItemTooltip")fallback.push(node);}ts.forEachChild(node,visit);})(card);
    assert.equal(renderer.length,1);assert.equal(fallback.length,1,"original basic label remains available");
    const documentProp=renderer[0].attributes.properties.find(a=>ts.isJsxAttribute(a)&&a.name.getText(ast)==="document");assert.equal(documentProp.initializer.expression.getText(ast),"tooltipDocument");
  }
  const pageReaderProps=[];(function visit(node){if(ts.isJsxAttribute(node)&&["onReadItemTooltip","onReadEquipmentItemTooltip"].includes(node.name.getText(parityPageAst)))pageReaderProps.push(node);ts.forEachChild(node,visit);})(parityPageAst);
  for(const [prop,reader]of [["onReadItemTooltip","ownedItemTooltipReaders.readItem"],["onReadEquipmentItemTooltip","ownedItemTooltipReaders.readEquipment"]]){
    const matches=pageReaderProps.filter(node=>node.name.getText(parityPageAst)===prop&&node.initializer?.expression?.getText(parityPageAst)===reader);assert.equal(matches.length,1,prop+" actual Page binding");
  }
  assert.ok(ts.isArrayLiteralExpression(ownedTooltipMemo.arguments[1]));const dependencies=ownedTooltipMemo.arguments[1].elements.map(node=>node.getText(parityPageAst));
  for(const dep of ["presentationWorld.inventoryItems","presentationWorld.beltItems","presentationWorld.equipmentItems","ownedTooltipOwner?.socket","ownedTooltipOwner?.connectionGeneration","ownedTooltipOwner?.sessionGeneration","ownedTooltipOwner?.playerObjectId","ownedTooltipOwner?.sceneRevision","ownedTooltipOwner?.mapFileName","ownedTooltipCore","ownedTooltipOwnerRevision","ownedTooltipBagEpoch","ownedTooltipCharacterEpoch","showInventory","activeInventoryTab","showCharacter","activeCharacterTab"])assert.ok(dependencies.includes(dep),"memo retires "+dep);
});

const ownedItemProjection=loadTypeScriptModule(new URL("../lib/world-model/item-presentation.ts",import.meta.url));
const tooltipMovementDeclarations=new Map();
(function visit(node){if(ts.isVariableDeclaration(node)&&["sameProjectedList","itemIdentity"].includes(node.name.getText(parityPageAst)))tooltipMovementDeclarations.set(node.name.getText(parityPageAst),"const "+node.getText(parityPageAst)+";");ts.forEachChild(node,visit);})(parityPageAst);
assert.equal(tooltipMovementDeclarations.size,2);
const tooltipMovementJs=ts.transpileModule([...tooltipMovementDeclarations.values()].join("\n"),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
check("Actual movement snapshot item projection keeps sale value changes out of confirmed movement fast path",()=>{
  const {sameProjectedList,itemIdentity}=new Function("projectItemPresentation",tooltipMovementJs+"\nreturn {sameProjectedList,itemIdentity};")(ownedItemProjection.projectItemPresentation);
  const row={...heroWorldRowFixture(heroItemFixture(0,-1,3),2),sellValue:7};
  assert.equal(sameProjectedList([row],[{...row,sellValue:8}],itemIdentity),false,"same UID/source with different unit sale value must replace tooltip source");
  assert.equal(sameProjectedList([{...row,sellValue:undefined}],[{...row,sellValue:0}],itemIdentity),true,"omitted and zero retain the actual projection default");
  assert.equal(sameProjectedList([row],[{...row,position:[8,9]}],itemIdentity),true,"unchanged price and item metadata survive unrelated movement");
  assert.equal(sameProjectedList([row],[{...row,tooltipSource:{...row.tooltipSource,userItem:{...row.tooltipSource.userItem,current_dura:9}}}],itemIdentity),false);
  const snapshots=[];(function visit(node){if(ts.isVariableDeclaration(node)&&node.name.getText(parityPageAst)==="staticStateMatches")snapshots.push(node);ts.forEachChild(node,visit);})(parityPageAst);
  assert.equal(snapshots.length,1);assert.ok(ts.isBinaryExpression(snapshots[0].initializer));
  const comparisonCalls=[];(function visit(node){if(ts.isCallExpression(node)&&node.expression.getText(parityPageAst)==="sameProjectedList")comparisonCalls.push(node.arguments.map(arg=>arg.getText(parityPageAst)));ts.forEachChild(node,visit);})(snapshots[0]);
  for(const source of ["inventoryItems","beltItems","storageItems"])assert.ok(comparisonCalls.some(args=>args[0]==="snapshot."+source&&args[1]==="currentWorldFast."+source&&args[2]==="itemIdentity"),source+" actual fast-path predicate");
});

// Source22: actual TS modules and whole production boundary declarations. The
// planner replies below are Rust ABI-shaped fixtures, never WASM execution or a
// JavaScript implementation of Crystal's unified-index planner.
check("Bag-to-Belt optional PUI forwards UID zero and Bag2 capacity without JS index planning",()=>{
  const input={inventoryCapacity:54,source:{container:0,slot:40,uniqueId:0},targetSlot:5};
  let reads=0;const requests=[],module={bag_to_belt_move_abi_version:()=>1,bag_to_belt_move_plan(json){reads++;requests.push(JSON.parse(json));
    return JSON.stringify({version:1,ok:true,plan:{uniqueId:0,from:46,to:5}});}};
  const plan=presentationRuntime.readSharedBagToBeltMovePlan(module,input);
  assert.deepEqual(plan,{uniqueId:0,from:46,to:5});assert.equal(Object.isFrozen(plan),true);
  assert.deepEqual(requests,[{version:1,...input}]);assert.equal(reads,1);
  for(const missing of [{},{bag_to_belt_move_abi_version:()=>1},{bag_to_belt_move_plan:module.bag_to_belt_move_plan},{...module,bag_to_belt_move_abi_version:()=>2}])
    assert.equal(presentationRuntime.readSharedBagToBeltMovePlan(missing,input),null);
  for(const change of [{inventoryCapacity:NaN},{inventoryCapacity:-1},{source:{...input.source,uniqueId:Number.MAX_SAFE_INTEGER+1}},
    {source:{...input.source,container:256}},{source:{...input.source,slot:0.5}},{targetSlot:-1}])
    assert.equal(presentationRuntime.readSharedBagToBeltMovePlan(module,{...input,...change}),null);
  assert.equal(reads,1,"invalid adapter inputs never enter the optional planner");
  const methodNodes=[];(function visit(node){if(ts.isMethodDeclaration(node)&&node.name.getText(tooltipCoreAst)==="planBagToBeltMove")methodNodes.push(node);ts.forEachChild(node,visit);})(tooltipCoreAst);
  assert.equal(methodNodes.length,1);const js=ts.transpileModule("const facade={"+methodNodes[0].getText(tooltipCoreAst)+"};",{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
  const facade=new Function("presentation","readSharedBagToBeltMovePlan",js+"\nreturn facade;")(module,presentationRuntime.readSharedBagToBeltMovePlan);
  assert.deepEqual(facade.planBagToBeltMove(input),plan);assert.equal(reads,2);
});
check("Bag-to-Belt optional PUI rejects malformed results and reentrant ABI getter replacement",()=>{
  const input={inventoryCapacity:54,source:{container:0,slot:40,uniqueId:0},targetSlot:0},reply={version:1,ok:true,plan:{uniqueId:0,from:46,to:0}};
  for(const result of [{version:1,ok:false},{...reply,extra:1},{...reply,version:2},{...reply,plan:{...reply.plan,extra:1}},
    {...reply,plan:{...reply.plan,uniqueId:1}},{...reply,plan:{...reply.plan,to:1}},{...reply,plan:{...reply.plan,from:0.5}},
    {...reply,plan:{...reply.plan,to:6}},"x".repeat(1025),"{bad"]){
    const module={bag_to_belt_move_abi_version:()=>1,bag_to_belt_move_plan:()=>typeof result==="string"?result:JSON.stringify(result)};
    assert.equal(presentationRuntime.readSharedBagToBeltMovePlan(module,input),null);
  }
  for(const edge of ["abi-before","planner-before","abi-during","planner-during","throw"]){
    let calls=0,abiCalls=0;const module={bag_to_belt_move_abi_version(){abiCalls++;if(abiCalls===1&&edge==="abi-before")module.bag_to_belt_move_abi_version=()=>1;
      if(abiCalls===1&&edge==="planner-before")module.bag_to_belt_move_plan=()=>JSON.stringify(reply);return 1;},
      bag_to_belt_move_plan(){calls++;if(edge==="abi-during")module.bag_to_belt_move_abi_version=()=>1;if(edge==="planner-during")module.bag_to_belt_move_plan=()=>JSON.stringify(reply);
        if(edge==="throw")throw Error("fixture getter");return JSON.stringify(reply);}};
    assert.equal(presentationRuntime.readSharedBagToBeltMovePlan(module,input),null,edge);assert.equal(calls,edge.endsWith("before")?0:1);
  }
});

function bagBeltLedgerFixture(occupied=false) {
  const owner={socket:{},connectionGeneration:1,sessionGeneration:1,playerObjectId:17},dispatcher=new bagBeltDispatcherModule.BagBeltMoveDispatcher();
  assert.equal(dispatcher.retireConnection(owner.socket,1),true);
  const command={type:"moveItem",grid:"belt",from:46,to:0},layout={capacity:54,placements:[{container:0,slot:40,uniqueId:0},
    {container:0,slot:1,uniqueId:10},...(occupied?[{container:1,slot:0,uniqueId:11}]:[])]};
  const proof=dispatcher.reserve(owner,command,40,0,occupied?11:null,4,layout);assert.ok(proof);
  const moved={...layout,placements:[{container:1,slot:0,uniqueId:0},{container:0,slot:1,uniqueId:10},...(occupied?[{container:0,slot:40,uniqueId:11}]:[])]};
  return {owner,dispatcher,command,layout,proof,moved,ack:{grid:"belt",from:46,to:0,success:true}};
}
check("Bag-to-Belt dispatcher protects UID zero both identities and empty destination cells",()=>{
  for(const occupied of [false,true]){
    const f=bagBeltLedgerFixture(occupied),d=f.dispatcher;assert.deepEqual(d.blockedUniqueIds(),occupied?[0,11]:[0]);
    assert.equal(d.isCellReserved(0,40),true);assert.equal(d.isCellReserved(1,0),true);assert.equal(d.isCellReserved(1,1),false);
    assert.equal(d.mutationAllowed(f.command,f.layout),false);assert.equal(d.mutationAllowed(f.command,f.layout,f.proof),true);
    assert.equal(d.mutationAllowed({type:"moveItem",grid:"belt",from:7,to:0},f.layout),false,"empty target cell is still reserved");
    assert.equal(d.mutationAllowed({type:"moveItem",grid:"belt",from:7,to:1},f.layout),true);
    assert.equal(d.mutationAllowed({type:"moveItem",grid:"belt",from:7,to:1},null),false);
    assert.equal(d.mutationAllowed({type:"chat",message:"unrelated"},null),true);
    assert.equal(d.allows({...f.proof},f.owner,f.command),false);assert.equal(d.allows(f.proof,f.owner,{...f.command,uniqueId:0}),false);
    assert.equal(d.enter(f.proof,f.owner,{...f.command,to:1}),false);assert.equal(d.cancelDefinitelyUnsent(f.proof),true);assert.equal(d.pending,0);
  }
});
check("Bag-to-Belt dispatcher entered transport throw stays blocked without owner layout or timeout release",()=>{
  const f=bagBeltLedgerFixture(true),d=f.dispatcher;assert.equal(d.enter(f.proof,f.owner,f.command),true);
  assert.throws(()=>{throw Error("socket transport fixture");},/transport fixture/);
  assert.equal(d.cancelDefinitelyUnsent(f.proof),false);assert.equal(d.enter(f.proof,f.owner,f.command),false);
  for(const owner of [f.owner,{...f.owner,sessionGeneration:2},{...f.owner,playerObjectId:18}]){
    d.observe(owner,f.layout,Number.MAX_SAFE_INTEGER);assert.equal(d.pending,1);
  }
  assert.equal(d.retireConnection(f.owner.socket,2),false,"a newer logical generation on the same physical socket cannot retire");
  assert.equal(d.retireConnection({},1),false);assert.equal(d.retireConnection({},0),false);assert.equal(d.pending,1);
  const next={};assert.equal(d.retireConnection(next,2),true);assert.equal(d.pending,0);assert.equal(d.allows(f.proof,{...f.owner,socket:next,connectionGeneration:2},f.command),false);
});
check("Bag-to-Belt exact ACK ignores wrong tuple session and physical connection",()=>{
  const f=bagBeltLedgerFixture(),d=f.dispatcher;assert.equal(d.acknowledge(f.owner,f.ack),false,"unentered flight cannot be ACKed");
  assert.equal(d.enter(f.proof,f.owner,f.command),true);
  for(const ack of [{...f.ack,grid:"inventory"},{...f.ack,from:40},{...f.ack,to:1},{...f.ack,success:1},{uniqueId:0,success:true}])
    assert.equal(d.acknowledge(f.owner,ack),false);
  for(const change of [{socket:{}},{connectionGeneration:2},{sessionGeneration:2},{playerObjectId:18}])assert.equal(d.acknowledge({...f.owner,...change},f.ack),false);
  assert.equal(d.pending,1);assert.equal(d.acknowledge(f.owner,{...f.ack,success:false}),true);assert.equal(d.pending,0);
  assert.equal(d.acknowledge(f.owner,f.ack),false,"released ACK cannot replay a command");
});
check("Bag-to-Belt success requires exact ACK and newer complete empty or occupied exchange in either order",()=>{
  for(const occupied of [false,true])for(const ackFirst of [false,true]){
    const f=bagBeltLedgerFixture(occupied),d=f.dispatcher;assert.equal(d.enter(f.proof,f.owner,f.command),true);
    if(ackFirst)assert.equal(d.acknowledge(f.owner,f.ack),true);
    assert.equal(d.observe(f.owner,f.moved,4),false,"same version cannot prove movement");assert.equal(d.pending,1);
    for(const placements of [f.layout.placements,[],[...f.moved.placements,{container:0,slot:2,uniqueId:0}],
      occupied?f.moved.placements.filter(p=>p.uniqueId!==11):[...f.moved.placements,{container:0,slot:40,uniqueId:12}]]){
      assert.equal(d.observe(f.owner,{...f.moved,placements},5),false);assert.equal(d.pending,1);
    }
    assert.equal(d.observe(f.owner,f.moved,5),ackFirst);assert.equal(d.pending,ackFirst?0:1);
    if(!ackFirst){assert.equal(d.acknowledge(f.owner,f.ack),true);assert.equal(d.pending,0);}
  }
});
check("Mail actual Belt decoder resolves from six and global Bag2 forty and protects both touched cells",()=>{
  const snapshot={bagCapacity:48,items:[{container:0,slot:0,uniqueId:7},{container:0,slot:40,uniqueId:8},{container:1,slot:0,uniqueId:9}]};
  const allowed=(command,ids=[],cells=[],snap=snapshot)=>mailParcelGateway.mailMutationAllowed(command,snap,ids,cells),a={type:"moveItem",grid:"belt",from:6,to:0},b={...a,from:46,to:1};
  assert.equal(allowed(a,[7]),false);assert.equal(allowed(a,[9]),false);assert.equal(allowed(a,[8]),true);
  assert.equal(allowed(b,[8]),false);assert.equal(allowed(b,[7]),true);assert.equal(allowed(b,[],[{container:1,slot:1}]),false);
  assert.equal(allowed(b,[],[{container:0,slot:40}]),false);assert.equal(allowed(b,[],[{container:0,slot:0}]),true);
  for(const from of [40,54,Number.MAX_SAFE_INTEGER,-1,6.5])assert.equal(allowed({...b,from},[9]),false);
  for(const snap of [null,{...snapshot,items:[]},{...snapshot,items:[...snapshot.items,{container:0,slot:40,uniqueId:99}]},
    {...snapshot,items:snapshot.items.map(p=>p.slot===40?{...p,uniqueId:null}:p)}])assert.equal(allowed(b,[9],[],snap),false);
  assert.equal(allowed({type:"chat",message:"unrelated"},[9],[],null),true);
});

const bagBeltGeometryJs=ts.transpileModule(readFileSync(new URL("../lib/bag-belt-gesture.ts",import.meta.url),"utf8"),
  {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
function bagBeltGeometryFixture() {
  class MemoryNode {
    constructor(rect){this.rect=rect;this.isConnected=true;this.hidden=false;this.disabled=false;this.tagName="BUTTON";this.captures=new Set();}
    getBoundingClientRect(){const [left,top,width,height]=this.rect;return {left,top,width,height};}
    contains(node){return node===this||this.children?.some(child=>child.contains(node))===true;}
    closest(selector){return selector==="button"?this:null;}
    setPointerCapture(id){this.captures.add(id);}hasPointerCapture(id){return this.captures.has(id);}releasePointerCapture(id){this.captures.delete(id);}
  }
  const stage=new MemoryNode([10,20,800,600]),buttons=Array.from({length:6},(_,slot)=>new MemoryNode([30+slot*50,120,40,40]));stage.children=[...buttons];
  const targets=buttons.map((button,slot)=>({slot,uniqueId:slot===0?0:null,left:10+slot*25,top:50,width:20,height:20}));
  const geometry={revision:1,targets,stage,stageRect:[10,20,800,600],buttons,scale:2,virtualWidth:400,virtualHeight:300,devicePixelRatio:2,page:"bag2"};
  const state={hit:buttons[0],style:{display:"block",visibility:"visible",pointerEvents:"auto",opacity:"1"},focus:true},
    document={visibilityState:"visible",hasFocus:()=>state.focus,elementFromPoint:()=>state.hit},window={devicePixelRatio:2,getComputedStyle:()=>state.style};
  const exports={};new Function("exports","document","window",bagBeltGeometryJs)(exports,document,window);
  return {MemoryNode,stage,buttons,geometry,state,document,window,api:exports};
}
check("Bag-to-Belt geometry requires six actual painted unobscured targets and exact release point",()=>{
  const f=bagBeltGeometryFixture(),g=f.geometry,a=f.api,point={clientX:31,clientY:121},target={slot:0,uniqueId:0};
  assert.equal(a.validBagBeltDropGeometry(g),true);assert.equal(a.bagBeltGeometryIsCurrent(g),true);
  assert.deepEqual(a.bagBeltTargetAtClientPoint(g,31,121),target);assert.equal(a.bagBeltReleasePointIsCurrent(g,target,point),true);
  f.state.hit={};assert.equal(a.bagBeltTargetAtClientPoint(g,31,121),null);f.state.hit=null;assert.equal(a.bagBeltTargetAtClientPoint(g,31,121),null);
  const child=new f.MemoryNode([31,121,1,1]);f.buttons[0].children=[child];f.state.hit=child;assert.deepEqual(a.bagBeltTargetAtClientPoint(g,31,121),target);
  for(const style of [{display:"none"},{visibility:"hidden"},{visibility:"collapse"},{pointerEvents:"none"},{opacity:"0"}]){
    const old=f.state.style;f.state.style={...old,...style};assert.equal(a.bagBeltTargetAtClientPoint(g,31,121),null);f.state.style=old;
  }
  for(const flag of ["disabled","hidden","isConnected"]){const old=f.buttons[0][flag];f.buttons[0][flag]=flag!=="isConnected";
    assert.equal(a.bagBeltTargetAtClientPoint(g,31,121),null);f.buttons[0][flag]=old;}
  assert.equal(a.bagBeltReleasePointIsCurrent(g,target,{clientX:70,clientY:121}),false,"half-open painted edge cannot drift");
  assert.equal(a.bagBeltReleasePointIsCurrent(g,{slot:0,uniqueId:null},point),false);
  for(const invalid of [{targets:g.targets.slice(0,5)},{targets:g.targets.map((t,i)=>i===1?{...t,slot:0}:t)},
    {targets:g.targets.map((t,i)=>i===1?{...t,uniqueId:0}:t)},{targets:g.targets.map((t,i)=>i===1?{...t,left:10}:t)}])assert.equal(a.validBagBeltDropGeometry({...g,...invalid}),false);
});
check("Bag-to-Belt geometry rejects DPR hidden node identity and measured rect changes",()=>{
  const f=bagBeltGeometryFixture(),a=f.api,g=f.geometry;
  f.window.devicePixelRatio=1;assert.equal(a.bagBeltGeometryIsCurrent(g),false);f.window.devicePixelRatio=2;
  f.document.visibilityState="hidden";assert.equal(a.bagBeltGeometryIsCurrent(g),false);f.document.visibilityState="visible";
  f.state.focus=false;assert.equal(a.bagBeltGeometryIsCurrent(g),false);f.state.focus=true;
  f.buttons[1].rect[0]++;assert.equal(a.bagBeltGeometryIsCurrent(g),false);f.buttons[1].rect[0]--;
  f.stage.rect[0]++;assert.equal(a.bagBeltGeometryIsCurrent(g),false);f.stage.rect[0]--;
  const replacement=new f.MemoryNode([...f.buttons[0].rect]);assert.equal(a.sameBagBeltGeometry(g,{...g,buttons:[replacement,...g.buttons.slice(1)]}),false);
  assert.equal(a.bagBeltGeometryIsCurrent({...g,buttons:[replacement,...g.buttons.slice(1)]}),false,"replacement is not inside measured stage");
  assert.equal(a.sameBagBeltGeometry(g,{...g,targets:g.targets.map(t=>({...t}))}),true,"equivalent values preserve actual node binding");
});

check("Belt actual layout effect retains equivalent bindings and registers all six empty or occupied buttons",()=>{
  const beltNodes=tooltipPanelsAst.statements.filter(n=>ts.isFunctionDeclaration(n)&&n.name?.text==="BeltDialog");assert.equal(beltNodes.length,1);
  const selected=new Map();let effect,button;
  (function visit(node){if(ts.isVariableDeclaration(node)&&["itemBySlot","beltBindingKey"].includes(node.name.getText(tooltipPanelsAst)))selected.set(node.name.getText(tooltipPanelsAst),"const "+node.getText(tooltipPanelsAst)+";");
    if(ts.isCallExpression(node)&&node.expression.getText(tooltipPanelsAst)==="useLayoutEffect"&&node.arguments[0].getText(tooltipPanelsAst).includes("onRegisterBeltTargets"))effect=node;
    if(ts.isJsxOpeningElement(node)&&node.tagName.getText(tooltipPanelsAst)==="button"&&node.attributes.getText(tooltipPanelsAst).includes('className={`belt-item '))button=node;
    ts.forEachChild(node,visit);})(beltNodes[0]);
  assert.equal(selected.size,2);assert.ok(effect&&button);assert.deepEqual(effect.arguments[1].elements.map(n=>n.getText(tooltipPanelsAst)),["beltBindingKey","vertical","onRegisterBeltTargets"]);
  assert.equal(ts.isJsxElement(button.parent),true,"actual button is unconditional within each mapped Belt slot");
  const refAttr=button.attributes.properties.find(p=>ts.isJsxAttribute(p)&&p.name.getText(tooltipPanelsAst)==="ref");assert.ok(refAttr.initializer.expression.getText(tooltipPanelsAst).includes("beltButtons.current[index] = node"));
  const js=ts.transpileModule([...selected.values()].join("\n")+"\n"+effect.getText(tooltipPanelsAst)+";",{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
  const f=bagBeltGeometryFixture(),beltButtons={current:f.buttons};let deps,cleanup,registrations=0,retires=0,latest;
  const register=targets=>{registrations++;latest=targets;return()=>retires++;};
  const useLayoutEffect=(callback,next)=>{if(deps&&next.every((value,i)=>Object.is(value,deps[i])))return;cleanup?.();deps=next;cleanup=callback();};
  const render=items=>new Function("items","vertical","beltButtons","onRegisterBeltTargets","useLayoutEffect",js)(items,false,beltButtons,register,useLayoutEffect);
  render([{slot:0,authoritativeUniqueId:0}]);assert.equal(registrations,1);assert.equal(latest.length,6);assert.equal(latest[0].item.authoritativeUniqueId,0);assert.equal(latest[1].item,null);
  render([{slot:0,authoritativeUniqueId:0,name:"equivalent replaced object"}]);assert.equal(registrations,1);assert.equal(retires,0);
  render([{slot:0,authoritativeUniqueId:9}]);assert.equal(registrations,2);assert.equal(retires,1);assert.equal(latest[0].item.authoritativeUniqueId,9);
  render([]);assert.equal(registrations,3);assert.ok(latest.every(target=>target.item===null));cleanup();assert.equal(retires,3);
});

const bagBeltShellNames=new Set(["rememberBagBeltClick","fenceBagBeltMouse","cancelBagBeltPointer","refreshBagBeltGeometry","bagBeltCallbacksMatch",
  "beginBagBeltPointer","beginCompatBagBeltPointer","finishBagBeltPointer","handleBagBeltPointer"]),bagBeltShellDeclarations=new Map();let bagBeltShellRect,bagBeltShellRegistration;
(function visit(node){if(ts.isFunctionDeclaration(node)&&bagBeltShellNames.has(node.name?.text))bagBeltShellDeclarations.set(node.name.text,node.getText(repairShellAst));
  if(ts.isVariableDeclaration(node)&&node.name.getText(repairShellAst)==="bagBeltRect")bagBeltShellRect="const "+node.getText(repairShellAst)+";";
  if(ts.isVariableDeclaration(node)&&node.name.getText(repairShellAst)==="registerBeltTargets")bagBeltShellRegistration=node.initializer.arguments[0].getText(repairShellAst);
  ts.forEachChild(node,visit);})(repairShellAst);
assert.equal(bagBeltShellDeclarations.size,bagBeltShellNames.size);assert.ok(bagBeltShellRect&&bagBeltShellRegistration);
const bagBeltShellJs=ts.transpileModule([...bagBeltShellDeclarations.values(),bagBeltShellRect,inactiveWorldFishingText,"const registerBeltTargets="+bagBeltShellRegistration+";"].join("\n"),
  {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
check("Bag-to-Belt actual Shell terminal burns before callback keeps four pixel threshold and fences compatibility clicks",()=>{
  for(const variant of ["drop","click","four","obscured","cancel","reentrant","bevy"]){
    // The threshold case starts inside its own source immediately beside Belt:
    // physical x23 -> x31 at scale2 is exactly four logical pixels.
    const f=bagBeltGeometryFixture(),node=new f.MemoryNode(variant==="four"?[20,120,10,40]:[350,300,40,40]);f.stage.children.push(node);
    const ref=current=>({current}),events=[],scope={...f.api,Element:f.MemoryNode,HTMLElement:f.MemoryNode,document:f.document,window:f.window,
      stageFrameRef:ref(f.stage),bagBeltButtonsRef:ref(null),bagBeltGeometryRef:ref(null),bagBeltGeometryRevisionRef:ref(0),
      bagBeltContextRef:ref({page:"bag2",open:true,screen:"game",repair:false,scale:2,virtualWidth:400,virtualHeight:300}),
      bagBeltPointerRef:ref(null),bagBeltArmedSharedRef:ref(null),bagBeltQuarantineRef:ref(new Map()),bagBeltRejectedTerminalRef:ref(null),bagBeltClickFenceRef:ref(new Map()),
      bagBeltCallbacksRef:ref({}),bagPointerCallbacksRef:ref({}),heldScenePointerRef:ref(null),combatUiHoldRef:ref(new Map()),
      onViewportDirectionStop:()=>{},beginCombatUiHold:(kind,id)=>scope.combatUiHoldRef.current.set(kind,{pointerId:id}),
      endCombatUiHold:(kind,hold)=>{if(scope.combatUiHoldRef.current.get(kind)===hold)scope.combatUiHoldRef.current.delete(kind);},cancelSharedBagPointer:()=>events.push("shared-cancel"),
      npcRepairService:null,bevyBagUiActive:false,showInventory:true};let api;
    const source={key:"Owned",container:"bag2",slot:0,authoritativeUniqueId:900},event=(id,time,x=351,y=301)=>({pointerId:id,timeStamp:time,clientX:x,clientY:y,
      pointerType:"mouse",isPrimary:true,button:0,currentTarget:node,target:node,preventDefault(){this.prevented=true;},stopPropagation(){this.stopped=true;}});
    scope.bagBeltCallbacksRef.current={onBeginBagToBeltGesture:()=>({token:{}}),onCancelBagToBeltGesture:()=>events.push("cancel"),
      onArmBagToBeltGesture:()=>{assert.equal(scope.bagBeltPointerRef.current,null,"physical terminal is spent before Page callback");events.push("arm");
        if(variant==="reentrant")assert.equal(api.beginBagBeltPointer(event(2,30),node,source,"react",()=>events.push("activate")),true);return true;},
      onBagToBeltMove:()=>{assert.equal(scope.bagBeltPointerRef.current,null);events.push("move");return true;}};
    const keys=Object.keys(scope);api=new Function(...keys,bagBeltShellJs+"\nreturn {registerBeltTargets,beginBagBeltPointer,beginCompatBagBeltPointer,handleBagBeltPointer,cancelBagBeltPointer,fenceBagBeltMouse};")(...keys.map(k=>scope[k]));
    const cleanup=api.registerBeltTargets(f.buttons.map((button,slot)=>({slot,node:button,item:slot===0?{authoritativeUniqueId:0}:null})));
    assert.ok(scope.bagBeltGeometryRef.current,"actual registered geometry before "+variant);
    assert.equal(scope.bagBeltGeometryRef.current.stage.contains(node),true,"actual source node containment before "+variant);
    assert.equal(typeof scope.bagBeltCallbacksRef.current.onBeginBagToBeltGesture,"function","committed begin callback before "+variant);
    const bevy=variant==="bevy",down=variant==="four"?event(1,10,23,121):event(1,10);
    assert.equal(api.beginBagBeltPointer(down,node,bevy?null:source,bevy?"bevy":"react",()=>events.push("activate")),true,"actual begin variant "+variant);
    const lease=scope.bagBeltPointerRef.current;assert.ok(lease);
    if(variant==="obscured")f.state.hit={};
    const up=variant==="click"?event(1,20,352,301):event(1,20,31,121);
    assert.equal(api.handleBagBeltPointer(up,variant==="cancel"?"cancel":"up"),!bevy);
    if(variant==="click")assert.deepEqual(events,["activate","cancel"]);
    else if(["drop","four"].includes(variant))assert.deepEqual(events,["arm","move","cancel"]);
    else if(variant==="bevy"){assert.deepEqual(events,["arm"]);assert.equal(scope.bagBeltArmedSharedRef.current,lease);api.cancelBagBeltPointer(lease);}
    else if(variant==="reentrant"){assert.deepEqual(events,["arm","cancel"]);assert.equal(scope.bagBeltPointerRef.current.pointerId,2);api.cancelBagBeltPointer();}
    else assert.deepEqual(events,["cancel"]);
    const mouse={...event(1,40),type:"mousedown"};assert.equal(api.fenceBagBeltMouse(mouse),true);assert.equal(mouse.prevented,true);
    assert.equal(api.fenceBagBeltMouse({...event(1,41),type:"click",detail:0}),false,"keyboard activation is independent");
    assert.equal(api.fenceBagBeltMouse({...event(1,42),type:"click",detail:1}),true);assert.equal(api.fenceBagBeltMouse({...event(1,43),type:"click",detail:1}),false);
    cleanup();assert.equal(scope.bagBeltGeometryRef.current,null);assert.equal(scope.combatUiHoldRef.current.size,0);
  }
});
check("Bag-to-Belt actual Shell second pointer quarantines through terminal without successor cleanup erasure",()=>{
  const f=bagBeltGeometryFixture(),node=new f.MemoryNode([350,300,40,40]);f.stage.children.push(node);const ref=current=>({current});let cancels=0;
  const scope={...f.api,Element:f.MemoryNode,HTMLElement:f.MemoryNode,document:f.document,window:f.window,stageFrameRef:ref(f.stage),
    bagBeltButtonsRef:ref(null),bagBeltGeometryRef:ref(null),bagBeltGeometryRevisionRef:ref(0),bagBeltContextRef:ref({page:"bag2",open:true,screen:"game",repair:false,scale:2,virtualWidth:400,virtualHeight:300}),
    bagBeltPointerRef:ref(null),bagBeltArmedSharedRef:ref(null),bagBeltQuarantineRef:ref(new Map()),bagBeltRejectedTerminalRef:ref(null),bagBeltClickFenceRef:ref(new Map()),
    bagBeltCallbacksRef:ref({onBeginBagToBeltGesture:()=>({token:{}}),onCancelBagToBeltGesture:()=>cancels++}),bagPointerCallbacksRef:ref({}),heldScenePointerRef:ref(null),combatUiHoldRef:ref(new Map()),
    onViewportDirectionStop:()=>{},beginCombatUiHold:(kind,id)=>scope.combatUiHoldRef.current.set(kind,{pointerId:id}),endCombatUiHold:kind=>scope.combatUiHoldRef.current.delete(kind),cancelSharedBagPointer:()=>{},
    npcRepairService:null,bevyBagUiActive:false,showInventory:true};
  const keys=Object.keys(scope),api=new Function(...keys,bagBeltShellJs+"\nreturn {registerBeltTargets,beginBagBeltPointer,handleBagBeltPointer,cancelBagBeltPointer,fenceBagBeltMouse};")(...keys.map(k=>scope[k])),
    bindings=f.buttons.map((button,slot)=>({slot,node:button,item:null})),event=(id,time)=>({pointerId:id,timeStamp:time,clientX:351,clientY:301,pointerType:"mouse",button:0,currentTarget:node,target:node,preventDefault(){this.prevented=true;},stopPropagation(){}});
  const oldCleanup=api.registerBeltTargets(bindings);assert.equal(api.beginBagBeltPointer(event(1,10),node,{},"react"),true);const oldLease=scope.bagBeltPointerRef.current;
  assert.equal(api.handleBagBeltPointer(event(2,11),"down"),true);assert.equal(scope.bagBeltPointerRef.current,null);assert.equal(cancels,1);
  assert.equal(scope.bagBeltQuarantineRef.current.has(1),true);assert.equal(scope.bagBeltQuarantineRef.current.has(2),true);
  assert.equal(api.handleBagBeltPointer(event(2,10),"up"),true);assert.equal(scope.bagBeltQuarantineRef.current.has(2),true);
  assert.equal(api.handleBagBeltPointer(event(2,12),"up"),true);assert.equal(scope.bagBeltQuarantineRef.current.has(2),false);
  assert.equal(api.fenceBagBeltMouse({...event(2,13),type:"click",detail:1}),true);
  api.handleBagBeltPointer(event(1,14),"up");const nextCleanup=api.registerBeltTargets(bindings);
  assert.equal(api.beginBagBeltPointer(event(3,20),node,{},"react"),true);const successor=scope.bagBeltPointerRef.current;
  oldCleanup();api.cancelBagBeltPointer(oldLease);assert.equal(scope.bagBeltPointerRef.current,successor);assert.ok(scope.bagBeltGeometryRef.current);
  nextCleanup();assert.equal(scope.bagBeltPointerRef.current,null);assert.equal(cancels,2);
});

const bagBeltPageNames=new Set(["currentEquipmentOwner","itemCommandRequiresOwner","socialItemMutationAllowed","bagBeltPhysicalOwner","cancelBagToBeltGesture",
  "invalidateBagBeltInventory","observeBagBeltInventorySnapshot","receiveBagBeltDropGeometry","readBagBeltContext","resolveBagBeltSource","bagBeltTargetCurrent",
  "beginBagToBeltGesture","bagBeltRecordCurrent","armBagToBeltGesture","bagBeltFinalCurrent","submitBagToBeltMove","retireWorldFishingGesture","cancelWorldFishingGesture"]),bagBeltPageDeclarations=new Map();
(function visit(node){if(ts.isFunctionDeclaration(node)&&bagBeltPageNames.has(node.name?.text))bagBeltPageDeclarations.set(node.name.text,node.getText(parityPageAst));ts.forEachChild(node,visit);})(parityPageAst);
assert.equal(bagBeltPageDeclarations.size,bagBeltPageNames.size);
const bagBeltEntryNodes=paritySendStatements.filter(node=>ts.isIfStatement(node)&&node.expression.getText(parityPageAst)==="options?.bagBeltProof");
assert.equal(bagBeltEntryNodes.length,1,"sole actual Bag-to-Belt irreversible entry");
const bagBeltFollowingFishingEntry=paritySendStatements[paritySendStatements.indexOf(bagBeltEntryNodes[0])+1];
assert.ok(ts.isIfStatement(bagBeltFollowingFishingEntry));
assert.equal(bagBeltFollowingFishingEntry.expression.getText(parityPageAst),"options?.worldFishingProof && !enterWorldFishingSend(options.worldFishingProof, wireCommand, socket)");
const bagBeltFollowingPearlEntry=paritySendStatements[paritySendStatements.indexOf(bagBeltEntryNodes[0])+2];
assert.ok(ts.isIfStatement(bagBeltFollowingPearlEntry));
assert.equal(bagBeltFollowingPearlEntry.expression.getText(parityPageAst).replace(/\s+/g," "),"options?.npcPearlBuyProof && !npcBuyDispatcher?.claimPearl(options.npcPearlBuyProof,wireCommand,serialized,socket, () => npcPearlSendCurrent(options.npcPearlBuyProof!,wireCommand,socket))");
const bagBeltFollowingRankingInspectEntry=paritySendStatements[paritySendStatements.indexOf(bagBeltEntryNodes[0])+3];
assert.ok(ts.isIfStatement(bagBeltFollowingRankingInspectEntry));
assert.equal(bagBeltFollowingRankingInspectEntry.expression.getText(parityPageAst).replace(/\s+/g,""),
  'wireCommand.type==="inspect"&&(!options?.rankingInspectProof||socketRef.current!==socket||socket.readyState!==WebSocket.OPEN||options.rankingInspectProof.owner.socket!==socket||!rankingInspectRequestsRef.current.claim(options.rankingInspectProof,currentRankingInspectSource(),wireCommand,Math.floor(performance.now())))',
  "actual Ranking Inspect claim follows the unchanged Bag Fishing and Pearl guards");
assert.equal(paritySendStatements.indexOf(bagBeltFollowingRankingInspectEntry)+1,paritySocketIndex,"actual Ranking Inspect guard adjoins the socket try");
assert.equal(paritySendStatements.indexOf(bagBeltEntryNodes[0])+4,paritySocketIndex,"actual Bag entry retains Fishing then Pearl entry immediately before socket.send");
// Retain whole actual serialization/DTO statements and the complete Bag-specific
// entry through socket.send. All Bag-specific owner/source/mail/storage/social
// guards execute in the actual bagBeltFinalCurrent; no gate is replaced by true.
const bagBeltPageJs=ts.transpileModule([...bagBeltPageDeclarations.values()].join("\n")+
  "\nfunction bagBeltSocketEntry(command,options,socket,beforeFinal){"+
  paritySendStatements.slice(paritySerializeIndex,paritySerializeIndex+2).map(n=>n.getText(parityPageAst)).join("\n")+
  "\nif(beforeFinal)beforeFinal();\n"+
  paritySendStatements.slice(parityDtoIndex,parityDtoIndex+2).map(n=>n.getText(parityPageAst)).join("\n")+
  "\n"+bagBeltEntryNodes[0].getText(parityPageAst)+"\n"+bagBeltFollowingFishingEntry.getText(parityPageAst)+"\n"+bagBeltFollowingPearlEntry.getText(parityPageAst)+"\n"+bagBeltFollowingRankingInspectEntry.getText(parityPageAst)+"\n"+paritySendStatements[paritySocketIndex].getText(parityPageAst)+"\nreturn true;}",
  {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
const bagBeltOwnerModule=loadTypeScriptModule(new URL("../lib/bevy-bag-ui.ts",import.meta.url),{
  "./bag-belt-gesture":bagBeltGeometryModule,"./world-model/item-identity":itemIdentity});
const bagBeltStorageModule=loadTypeScriptModule(new URL("../lib/storage-gateway-adapter.ts",import.meta.url),{
  "./equipment-gateway-adapter":equipmentGateway,"./world-model/item-identity":itemIdentity,"./mail-parcel-gateway-adapter":mailParcelGateway});
function bagBeltPageFixture(occupied=false,extraInventory=[]) {
  const geometryFixture=bagBeltGeometryFixture(),g=geometryFixture.geometry;g.targets[0].uniqueId=occupied?11:null;
  const owner=npcRepairOwner(),row={...npcRepairItem(0,0,"bag2"),authoritativeUniqueId:0,key:"Owned"},
    belt=occupied?[{...npcRepairItem(11,0,"belt"),authoritativeUniqueId:11}]:[],raw=npcRepairSnapshot({inventoryItems:[row,...extraInventory],beltItems:belt,equipmentItems:[],storageItems:[],maxBagSlots:48});
  const page=parityPageFixture(owner,raw),ref=current=>({current}),layout=equipmentGateway.projectEquipmentGatewaySnapshot(raw);assert.ok(layout);
  const bagOwner={owner:"react",runGeneration:2,ownerRevision:3};page.scope.equipmentBagOwnerRef.current=bagOwner;
  page.scope.equipmentSnapshotRef.current={...owner,snapshot:layout};page.scope.bagOpenRef.current=true;
  const plannerRequests=[],plannerModule={bag_to_belt_move_abi_version:()=>1,bag_to_belt_move_plan(json){plannerRequests.push(JSON.parse(json));
    return JSON.stringify({version:1,ok:true,plan:{uniqueId:0,from:46,to:0}});}};
  // Mock only the Core ABI-shaped pending metadata. The complete Page guards,
  // transport dispatcher, owner comparison, source lookup and geometry are real.
  let pendingRead=null,beforeFinal=null,throwTransport=false,pendingReads=0;const errors=[];
  page.scope.questCoreRuntimeRef.current={planBagToBeltMove:input=>presentationRuntime.readSharedBagToBeltMovePlan(plannerModule,input)};
  page.scope.equipmentControllerRef.current={status:()=>({ready:true,pending:0}),hasPendingInstance:id=>{pendingReads++;pendingRead?.(id);return {ok:true,reserved:false};}};
  page.scope.bagBeltInventoryReadyRef.current={layout,raw,signature:JSON.stringify([raw.inventoryCapacity,raw.inventoryItems,raw.beltItems]),epoch:1,snapshotVersion:1};
  const scope={...page.scope,...geometryFixture.api,document:geometryFixture.document,window:geometryFixture.window,
    sameBagOwner:bagBeltOwnerModule.sameBagOwner,...itemIdentity,storageMutationAllowed:bagBeltStorageModule.storageMutationAllowed,
    projectMailItemMutationSnapshot:mailParcelGateway.projectMailItemMutationSnapshot,
    parityItemMutationAllowed:page.api.parityItemMutationAllowed,equipmentRenderOwnerToken:{...bagOwner,connectionGeneration:owner.connectionGeneration,sessionGeneration:owner.sessionGeneration},
    bagBeltGeometryRef:ref(g),bagBeltGestureRegistryRef:ref(new WeakMap()),bagBeltArmedGestureRef:ref(null),bagBeltInventoryEpochRef:ref(1),
    bagBeltLastInventorySignatureRef:ref(null),renderBagOwner:()=>{},console:{error:(...args)=>errors.push(args)},
    sendRaw:(command,options)=>api.bagBeltSocketEntry(command,options,page.socket,beforeFinal)};
  const keys=Object.keys(scope),api=new Function(...keys,bagBeltPageJs+"\nreturn {beginBagToBeltGesture,armBagToBeltGesture,submitBagToBeltMove,bagBeltSocketEntry,bagBeltFinalCurrent,observeBagBeltInventorySnapshot,receiveBagBeltDropGeometry,invalidateBagBeltInventory};")(...keys.map(k=>scope[k]));
  page.socket.send=body=>{if(throwTransport)throw Error("fixture socket throw");page.sentBodies.push(body);page.sent.push(JSON.parse(body));};
  const item={key:"Owned",uniqueId:0,authoritativeUniqueId:0,container:"bag2",slot:0},target={slot:0,uniqueId:occupied?11:null},point={clientX:31,clientY:121};
  const arm=()=>{const gesture=api.beginBagToBeltGesture(item,g,"react");assert.ok(gesture);assert.equal(api.armBagToBeltGesture(gesture,g,target,point),true);return gesture;};
  return {...page,...geometryFixture,scope,api,item,target,layout,raw,owner,plannerModule,plannerRequests,errors,arm,
    setPendingRead:value=>pendingRead=value,setBeforeFinal:value=>beforeFinal=value,setThrow:value=>throwTransport=value,getPendingReads:()=>pendingReads};
}
check("Bag-to-Belt Page actual UID zero Bag2 gesture sends exactly four keys once and retains transport throw barrier",()=>{
  for(const throws of [false,true]){
    const f=bagBeltPageFixture(),proof=f.arm();f.setThrow(throws);
    assert.equal(f.api.submitBagToBeltMove(f.item,f.target,proof,f.geometry),!throws);
    assert.deepEqual(f.plannerRequests,[{version:1,inventoryCapacity:54,source:{container:0,slot:40,uniqueId:0},targetSlot:0}]);
    assert.equal(f.api.submitBagToBeltMove(f.item,f.target,proof,f.geometry),false,"spent gesture cannot retry");
    assert.equal(f.plannerRequests.length,1);assert.equal(f.scope.bagBeltMovesRef.current.pending,1);
    if(throws){assert.equal(f.sent.length,0);assert.equal(f.errors.length,1);}
    else {assert.deepEqual(f.sent,[{type:"moveItem",grid:"belt",from:46,to:0}]);assert.equal(Object.keys(f.sent[0]).length,4);}
    assert.equal(f.scope.bagBeltMovesRef.current.mutationAllowed({type:"moveItem",grid:"belt",from:46,to:0},f.layout),false);
  }
});
check("Bag-to-Belt Page final entry rechecks reentrant owner source geometry mail and actual mutation gates",()=>{
  for(const edge of ["planner","pending","listener"])for(const change of ["socket","session","owner","source","inventory","epoch","mail","geometry","obscured","hidden","core","storage","social"]){
    const f=bagBeltPageFixture(),proof=f.arm(),mutate=()=>{
      if(change==="socket")f.scope.socketRef.current={readyState:1};if(change==="session")f.scope.equipmentSessionGenerationRef.current++;
      if(change==="owner")f.scope.equipmentBagOwnerRef.current={...f.scope.equipmentBagOwnerRef.current,ownerRevision:99};
      if(change==="source")f.scope.worldRef.current.inventoryItems[0].authoritativeUniqueId=2;
      if(change==="inventory")f.scope.bagBeltInventoryReadyRef.current={...f.scope.bagBeltInventoryReadyRef.current,layout:{...f.layout}};
      if(change==="epoch")f.scope.bagBeltInventoryEpochRef.current++;
      if(change==="mail")f.scope.mailParcelRef.current={snapshot:{bagCapacity:48,items:f.layout.placements},state:{blockedUniqueIds:[0]}};
      if(change==="geometry")f.buttons[0].rect[0]++;if(change==="obscured")f.state.hit={};if(change==="hidden")f.document.visibilityState="hidden";
      if(change==="core")f.scope.questCoreRuntimeRef.current={planBagToBeltMove:()=>null};
      if(change==="storage")f.scope.pendingStorageRequestsRef.current.set("flight",{proof:{type:"storeItemV2",from:40,to:0,source:{uniqueId:0},target:{uniqueId:null}}});
      if(change==="social")Object.defineProperty(f.scope.socialItemOperationsRef.current,"pending",{value:{entered:true,proof:{}},configurable:true});
    };
    if(edge==="planner"){const original=f.plannerModule.bag_to_belt_move_plan;f.plannerModule.bag_to_belt_move_plan=json=>{const reply=original(json);mutate();return reply;};}
    if(edge==="pending"){let once=false;f.setPendingRead(()=>{if(!once){once=true;mutate();}});}
    if(edge==="listener")f.setBeforeFinal(mutate);
    assert.equal(f.api.submitBagToBeltMove(f.item,f.target,proof,f.geometry),false,edge+" "+change);
    assert.equal(f.sent.length,0,edge+" "+change);assert.equal(f.scope.bagBeltMovesRef.current.pending,0,"definitely unentered proof released");
    assert.equal(f.api.submitBagToBeltMove(f.item,f.target,proof,f.geometry),false,"failed terminal remains spent");
  }
});
check("Bag-to-Belt Page final Core getter reentry cannot add mail social or storage ownership after last gate",()=>{
  for(const change of ["mail","social","storage","source"]){
    const f=bagBeltPageFixture(),proof=f.arm(),core=f.scope.questCoreRuntimeRef.current,planner=core.planBagToBeltMove;let reads=0;
    f.setBeforeFinal(()=>Object.defineProperty(core,"planBagToBeltMove",{configurable:true,get(){
      reads++;if(reads===2){
        if(change==="mail")f.scope.mailParcelRef.current={snapshot:{bagCapacity:48,items:f.layout.placements},state:{blockedUniqueIds:[0]}};
        if(change==="social")Object.defineProperty(f.scope.socialItemOperationsRef.current,"pending",{value:{entered:true,proof:{}},configurable:true});
        if(change==="storage")f.scope.pendingStorageRequestsRef.current.set("flight",{proof:{type:"storeItemV2",from:40,to:0,source:{uniqueId:0},target:{uniqueId:null}}});
        if(change==="source")f.scope.worldRef.current.inventoryItems[0].authoritativeUniqueId=7;
      }return planner;}}));
    assert.equal(f.api.submitBagToBeltMove(f.item,f.target,proof,f.geometry),false,change);assert.ok(reads>=2);
    assert.equal(f.sent.length,0);assert.equal(f.scope.bagBeltMovesRef.current.pending,0);
  }
});
check("Bag-to-Belt Page actual complete snapshot observation and exact ACK release occupied exchange in both orders",()=>{
  for(const ackFirst of [false,true]){
    const f=bagBeltPageFixture(true),proof=f.arm();assert.equal(f.api.submitBagToBeltMove(f.item,f.target,proof,f.geometry),true);
    const d=f.scope.bagBeltMovesRef.current,ack={grid:"belt",from:46,to:0,success:true},physical={socket:f.socket,connectionGeneration:f.owner.connectionGeneration,sessionGeneration:f.owner.sessionGeneration,playerObjectId:17};
    if(ackFirst)assert.equal(d.acknowledge(physical,ack),true);assert.equal(d.pending,1);
    f.api.observeBagBeltInventorySnapshot(f.raw,f.owner.connectionGeneration,f.scope.equipmentSnapshotRef.current);
    assert.equal(d.pending,1,"partial or unreplaced baseline cannot release entered flight");
    const moved=npcRepairSnapshot({inventoryItems:[{...npcRepairItem(11,0,"bag2"),authoritativeUniqueId:11}],beltItems:[{...npcRepairItem(0,0,"belt"),authoritativeUniqueId:0}],equipmentItems:[]}),
      old=f.scope.equipmentSnapshotRef.current,next=equipmentGateway.projectEquipmentGatewaySnapshot(moved);assert.ok(next);
    f.scope.worldRef.current={...f.scope.worldRef.current,...moved};f.scope.worldSnapshotVersionRef.current=2;f.scope.equipmentSnapshotRef.current={...f.owner,snapshot:next};
    f.api.observeBagBeltInventorySnapshot(moved,f.owner.connectionGeneration,old);assert.equal(d.pending,ackFirst?0:1);
    if(!ackFirst){assert.equal(d.acknowledge(physical,ack),true);assert.equal(d.pending,0);}
    assert.equal(f.sent.length,1,"observation and ACK never retry the spent terminal");
    f.api.invalidateBagBeltInventory();assert.equal(f.scope.bagBeltInventoryReadyRef.current,null);
  }
});

check("Mail actual item mutation projection preserves known UID zero while parcel attachment sentinel remains null",()=>{
  const raw=npcRepairSnapshot({inventoryItems:[npcRepairItem(0,0,"bag2"),npcRepairItem(7,1)],beltItems:[npcRepairItem(11,0,"belt")],equipmentItems:[]}),
    command={type:"moveItem",grid:"belt",from:46,to:0},mutation=mailParcelGateway.projectMailItemMutationSnapshot(raw),parcel=mailParcelGateway.projectMailParcelSnapshot(raw);
  assert.ok(mutation&&parcel);assert.equal(mutation.bagCapacity,48);assert.equal(mutation.items[0].uniqueId,0);assert.equal(mutation.items[0].slot,40);
  assert.equal(parcel.items[0].uniqueId,null,"original positive parcel attachment identity remains unchanged");
  assert.equal(mailParcelGateway.mailMutationAllowed(command,mutation,[7]),true,"unrelated locked mail UID does not prevent known zero exchange");
  assert.equal(mailParcelGateway.mailMutationAllowed(command,mutation,[0]),false);assert.equal(mailParcelGateway.mailMutationAllowed(command,mutation,[11]),false);
  assert.equal(mailParcelGateway.mailMutationAllowed(command,mutation,[],[{container:0,slot:40}]),false);
  assert.equal(mailParcelGateway.mailMutationAllowed(command,mutation,[],[{container:1,slot:0}]),false);
  assert.equal(mailParcelGateway.mailMutationAllowed(command,parcel,[7]),false,"parcel's zero sentinel cannot authorize a move");
  const empty={...raw,beltItems:[]},emptyMutation=mailParcelGateway.projectMailItemMutationSnapshot(empty);
  assert.equal(mailParcelGateway.mailMutationAllowed(command,emptyMutation,[7]),true);
  assert.equal(mailParcelGateway.mailMutationAllowed(command,emptyMutation,[],[{container:1,slot:0}]),false,"known empty target remains reservable");
  for(const changed of [{...raw,inventoryItems:raw.inventoryItems.slice(1)},
    {...raw,inventoryItems:[{...raw.inventoryItems[0],uniqueId:null},raw.inventoryItems[1]]},
    {...raw,inventoryItems:[{...raw.inventoryItems[0],uniqueId:undefined},raw.inventoryItems[1]]},
    {...raw,inventoryItems:[...raw.inventoryItems,{...raw.inventoryItems[0]}]},
    {...raw,beltItems:[...raw.beltItems,{...raw.beltItems[0]}]}]){
    const projected=mailParcelGateway.projectMailItemMutationSnapshot(changed);
    assert.equal(mailParcelGateway.mailMutationAllowed(command,projected,[7]),false,"unknown missing or ambiguous touched row cannot become permission");
  }
  for(const changed of [{...raw,inventoryItems:undefined},{...raw,beltItems:undefined},{...raw,inventoryCapacity:47}])assert.equal(mailParcelGateway.projectMailItemMutationSnapshot(changed),null);
});
check("Bag-to-Belt Page uses actual zero preserving projection under unrelated mail locks for empty and occupied Belt",()=>{
  for(const occupied of [false,true]){
    const f=bagBeltPageFixture(occupied,[{...npcRepairItem(7,1),authoritativeUniqueId:7}]),parcel=mailParcelGateway.projectMailParcelSnapshot(f.raw);
    assert.equal(parcel.items[0].uniqueId,null);f.scope.mailParcelRef.current={snapshot:parcel,state:{blockedUniqueIds:[7]}};
    const gesture=f.arm();assert.equal(f.api.submitBagToBeltMove(f.item,f.target,gesture,f.geometry),true);
    assert.deepEqual(f.sent,[{type:"moveItem",grid:"belt",from:46,to:0}]);assert.equal(f.scope.bagBeltMovesRef.current.pending,1);
  }
  for(const blocked of [[0],[11]]){
    const f=bagBeltPageFixture(true),gesture=f.arm();f.scope.mailParcelRef.current={snapshot:mailParcelGateway.projectMailParcelSnapshot(f.raw),state:{blockedUniqueIds:blocked}};
    assert.equal(f.api.submitBagToBeltMove(f.item,f.target,gesture,f.geometry),false);assert.equal(f.sent.length,0);assert.equal(f.scope.bagBeltMovesRef.current.pending,0);
  }
});
check("Bag-to-Belt Page rejects duplicate global UID zero and missing raw source despite display identity",()=>{
  for(const change of ["duplicateUid","duplicateCell","missingUid","missingGroup"]){
    const f=bagBeltPageFixture();
    if(change==="duplicateUid")f.raw.inventoryItems.push({...npcRepairItem(0,1),authoritativeUniqueId:0});
    if(change==="duplicateCell")f.raw.inventoryItems.push({...npcRepairItem(8,0,"bag2"),authoritativeUniqueId:8});
    if(change==="missingUid")delete f.raw.inventoryItems[0].uniqueId;
    if(change==="missingGroup")f.raw.inventoryItems=undefined;
    const layout=equipmentGateway.projectEquipmentGatewaySnapshot(f.raw);f.scope.worldRef.current={...f.scope.worldRef.current,...f.raw};
    if(layout){f.scope.equipmentSnapshotRef.current={...f.owner,snapshot:layout};f.scope.bagBeltInventoryReadyRef.current={...f.scope.bagBeltInventoryReadyRef.current,layout,raw:f.raw};}
    else f.scope.bagBeltInventoryReadyRef.current=null;
    assert.equal(f.api.beginBagToBeltGesture(f.item,f.geometry,"react"),null,change);assert.equal(f.sent.length,0);assert.equal(f.plannerRequests.length,0);
  }
});


// Source23 optional fishing and owned animation boundaries. These exercise the
// product adapters with bounded JSON getters; they do not instantiate WASM.
const sharedFishingClick=loadTypeScriptModule(new URL("../lib/shared-fishing-click.ts",import.meta.url));
const worldFishingSource=loadTypeScriptModule(new URL("../lib/world-fishing-source.ts",import.meta.url));
const fishingAnimation=loadTypeScriptModule(new URL("../app/components/original-client-entity-animation-runtime.ts",import.meta.url));
const fishingTargetsInput=()=>({origin:{x:-2147483648,y:2147483647},direction:2});
const fishingTargetsReply=()=>({version:1,ok:true,walkCandidates:[{direction:2,cell:{x:1,y:2}},
  {direction:1,cell:{x:3,y:4}},{direction:3,cell:{x:5,y:6}}],waterTarget:{x:7,y:8}});
const fishingDecisionInput=()=>({origin:{x:0,y:0},direction:2,requestedWalk:false,autoRoute:false,
  walkBlocked:[false,null,true],rodPresent:null,water:{cell:{x:4,y:0},light:255},facingMatches:null,
  standing:null,fishing:null,transformType:-32768,nowMs:Number.MAX_SAFE_INTEGER,lastCastMs:0});
check("Fishing optional ABI forwards exact raw geometry clocks and nullable facts without host arithmetic",()=>{
  const requests=[],module={fishing_click_abi_version:()=>1,
    fishing_click_targets(json){requests.push(JSON.parse(json));return JSON.stringify(fishingTargetsReply());},
    fishing_click_decision(json){const input=JSON.parse(json);requests.push(input);return JSON.stringify({version:1,ok:true,decision:{type:"cast",lastCastMs:input.nowMs}});}};
  const targets=presentationRuntime.readSharedFishingClickTargets(module,fishingTargetsInput());
  assert.deepEqual(requests[0],{version:1,...fishingTargetsInput()});assert.deepEqual(targets,{walkCandidates:fishingTargetsReply().walkCandidates,waterTarget:{x:7,y:8}});
  assert.ok(Object.isFrozen(targets)&&Object.isFrozen(targets.walkCandidates)&&Object.isFrozen(targets.walkCandidates[0].cell));
  const decision=presentationRuntime.readSharedFishingClickDecision(module,fishingDecisionInput());
  assert.deepEqual(requests[1],{version:1,...fishingDecisionInput()});assert.deepEqual(decision,{type:"cast",lastCastMs:Number.MAX_SAFE_INTEGER});assert.ok(Object.isFrozen(decision));
  for(const absent of [{},{fishing_click_abi_version:()=>0},{fishing_click_abi_version:()=>1}]){
    assert.equal(presentationRuntime.readSharedFishingClickTargets(absent,fishingTargetsInput()),null);
    assert.equal(presentationRuntime.readSharedFishingClickDecision(absent,fishingDecisionInput()),null);
  }
});
check("Fishing adapters reject coerced raw coordinates clocks directions and malformed outputs",()=>{
  let calls=0,reply=fishingTargetsReply();const module={fishing_click_abi_version:()=>1,
    fishing_click_targets(){calls++;return JSON.stringify(reply);},fishing_click_decision(){calls++;return JSON.stringify({version:1,ok:true,decision:{type:"none"}});}};
  for(const delta of [{origin:{x:"0",y:0}},{origin:{x:2147483648,y:0}},{origin:{x:0,y:-2147483649}},
    {origin:{x:0,y:0,extra:1}},{direction:-1},{direction:256},{direction:2.5},{extra:true}])
    assert.equal(sharedFishingClick.readSharedFishingClickTargets(module,{...fishingTargetsInput(),...delta}),null);
  for(const delta of [{nowMs:-1},{nowMs:1.5},{nowMs:Infinity},{nowMs:"1"},{lastCastMs:Number.MAX_SAFE_INTEGER+1},
    {requestedWalk:1},{autoRoute:null},{walkBlocked:[false,false]},{walkBlocked:[false,0,true]},
    {rodPresent:0},{standing:"standing"},{fishing:undefined},{transformType:32768},
    {water:{cell:{x:0,y:0},light:256}},{water:{cell:{x:0,y:0},light:1,extra:true}}])
    assert.equal(sharedFishingClick.readSharedFishingClickDecision(module,{...fishingDecisionInput(),...delta}),null);
  assert.equal(calls,0,"invalid raw facts never enter optional Rust getter");
  for(const invalid of [null,[],{...fishingTargetsReply(),extra:1},{...fishingTargetsReply(),waterTarget:{x:0,y:0,extra:1}},
    {...fishingTargetsReply(),walkCandidates:[]},{...fishingTargetsReply(),walkCandidates:[{direction:1,cell:{x:0,y:0}},...fishingTargetsReply().walkCandidates.slice(1)]}]){
    reply=invalid;assert.equal(sharedFishingClick.readSharedFishingClickTargets(module,fishingTargetsInput()),null);
  }
  for(const decision of [{type:"cast",lastCastMs:1},{type:"turn",direction:1,delayMs:200},{type:"turn",direction:2,delayMs:199},
    {type:"none",extra:true},{type:"cast",lastCastMs:"1"}]){
    module.fishing_click_decision=()=>JSON.stringify({version:1,ok:true,decision});assert.equal(sharedFishingClick.readSharedFishingClickDecision(module,fishingDecisionInput()),null);
  }
  module.fishing_click_decision=()=>JSON.stringify({version:1,ok:true,decision:{type:"turn",direction:2,delayMs:200}});
  assert.deepEqual(sharedFishingClick.readSharedFishingClickDecision(module,fishingDecisionInput()),{type:"turn",direction:2,delayMs:200});
  module.fishing_click_targets=()=>"{";assert.equal(sharedFishingClick.readSharedFishingClickTargets(module,fishingTargetsInput()),null);
  module.fishing_click_targets=()=>"x".repeat(4097);assert.equal(sharedFishingClick.readSharedFishingClickTargets(module,fishingTargetsInput()),null);
});
check("Fishing optional ABI custody rejects getter replacement and reentrant input mutation before and after calls",()=>{
  for(const reader of ["targets","decision"])for(const edge of ["abi-before","abi-after","getter","input-before","input-after"]){
    const input=reader==="targets"?fishingTargetsInput():fishingDecisionInput(),key="fishing_click_"+reader;
    let abiCalls=0,calls=0;const mutate=()=>{input.origin.x++;};
    const module={fishing_click_abi_version(){abiCalls++;if(edge==="abi-before"&&abiCalls===1)module[key]=()=>"{}";
      if(edge==="abi-after"&&abiCalls===3)module.fishing_click_abi_version=()=>1;
      if(edge==="input-before"&&abiCalls===1)mutate();return 1;},[key](){calls++;
      if(edge==="getter")module[key]=()=>"{}";if(edge==="input-after")mutate();
      return JSON.stringify(reader==="targets"?fishingTargetsReply():{version:1,ok:true,decision:{type:"cast",lastCastMs:input.nowMs}});}};
    assert.equal(reader==="targets"?sharedFishingClick.readSharedFishingClickTargets(module,input):sharedFishingClick.readSharedFishingClickDecision(module,input),null,reader+" "+edge);
    assert.equal(calls,edge.endsWith("before")?0:1,reader+" "+edge);
  }
});
const fishingOwner=()=>({socket:{},connectionGeneration:1,sessionGeneration:2,sceneRevision:3,playerObjectId:17,mapFileName:"0"});
const fishingSnapshot=(owner,delta={})=>({playerObjectId:owner.playerObjectId,mapFileName:owner.mapFileName,
  entities:[{kind:"selfPlayer",objectId:owner.playerObjectId,x:0,y:1,direction:"Right",dead:false,fishing:false,transformType:0,...delta}]});
check("WorldFishingSource preserves raw own facts and rejects foreign or stale owner packets",()=>{
  const owner=fishingOwner(),source=new worldFishingSource.WorldFishingSource(),first=source.observeSnapshot(owner,fishingSnapshot(owner,{x:-2147483648,y:2147483647,transformType:-32768}));
  assert.ok(first&&first.animationKnown&&first.bootstrapKnown);assert.deepEqual(first.self,{x:-2147483648,y:2147483647,direction:"Right",dead:false});
  assert.equal(first.transformType,-32768);assert.equal(first.fishing,false);assert.ok(Object.isFrozen(first)&&Object.isFrozen(first.owner)&&Object.isFrozen(first.self));assert.equal(Object.isFrozen(owner.socket),false);
  for(const foreign of [{...owner,socket:{}},{...owner,sessionGeneration:3},{...owner,sceneRevision:4}]){
    source.observePacket(foreign,"Disconnect",{});assert.strictEqual(source.current(owner),first);assert.equal(source.current(foreign),null);
  }
  source.observePacket(owner,"ObjectTurn",{objectId:18,x:4,y:5,direction:"Down"});assert.strictEqual(source.current(owner),first);
  const newer={...owner,connectionGeneration:2},next=source.observeSnapshot(newer,fishingSnapshot(newer));assert.notEqual(next.sourceKey,first.sourceKey);assert.equal(source.current(owner),null);
  source.observePacket(owner,"MapChanged",{});assert.strictEqual(source.current(newer),next);
});
check("WorldFishingSource unknown intervals cannot revive from ordinary snapshots or partial actions",()=>{
  const owner=fishingOwner(),source=new worldFishingSource.WorldFishingSource(),first=source.observeSnapshot(owner,fishingSnapshot(owner));
  source.observePacket(owner,"ObjectAttack",{objectId:17,direction:"Right"});const unknown=source.current(owner);
  assert.equal(unknown.animationKnown,false);assert.equal(unknown.bootstrapKnown,false);assert.ok(unknown.continuityRevision>first.continuityRevision);
  const restored=source.observeSnapshot(owner,fishingSnapshot(owner));assert.equal(restored.animationKnown,false);assert.equal(restored.bootstrapKnown,false);assert.equal(restored.sourceKey,first.sourceKey);
  source.observePacket(owner,"FishingUpdate",{objectId:17,fishing:true});assert.equal(source.current(owner).animationKnown,false);
  source.observePacket(owner,"ObjectTurn",{objectId:17,x:2,y:3,location:{x:9,y:3},direction:"Down"});assert.equal(source.current(owner).animationKnown,false);
  assert.equal(source.current(owner).self,null,"conflicting own raw position withdraws complete self facts");
  const reacquired=source.observeSnapshot(owner,fishingSnapshot(owner,{x:2,y:3,direction:"Down",fishing:true}));
  assert.equal(reacquired.animationKnown,false,"complete raw snapshot reacquires dead and transform facts without healing animation");assert.equal(reacquired.bootstrapKnown,false);
  assert.deepEqual(reacquired.self,{x:2,y:3,direction:"Down",dead:false});
  const before=source.current(owner);source.observePacket(owner,"ObjectTurn",{objectId:17,location:{x:2,y:3},direction:"Down"},1000);const known=source.current(owner);
  assert.equal(known.animationKnown,true);assert.equal(known.bootstrapKnown,false);assert.ok(known.continuityRevision>before.continuityRevision);assert.deepEqual(known.self,{x:2,y:3,direction:"Down",dead:false});
  source.observePacket(owner,"ObjectWalk",{objectId:17,x:3,y:3,direction:"Right"},1001);const walking=source.current(owner);
  assert.equal(walking.continuityRevision,known.continuityRevision,"complete ordinary actions retain the same known interval");assert.ok(walking.revision>known.revision);assert.equal(walking.bootstrapKnown,false);
});
check("WorldFishingSource initial unknown bootstrap is once while malformed raw facts stay unknown",()=>{
  const owner=fishingOwner(),source=new worldFishingSource.WorldFishingSource();source.observeSnapshot(owner,{playerObjectId:17,mapFileName:"0",entities:[]});
  const first=source.observeSnapshot(owner,fishingSnapshot(owner));assert.equal(first.bootstrapKnown,true);assert.equal(first.animationKnown,true);
  for(const delta of [{x:"0"},{dead:undefined},{direction:"right"},{transformType:32768},{fishing:0}]){
    const record=source.observeSnapshot(owner,fishingSnapshot(owner,delta));assert.equal(record.animationKnown,false);assert.equal(record.bootstrapKnown,false);
    assert.equal(source.observeSnapshot(owner,fishingSnapshot(owner)).animationKnown,false,"valid plain snapshot cannot restore retired evidence");
  }
  const duplicate=fishingSnapshot(owner);duplicate.entities.push({...duplicate.entities[0]});assert.equal(source.observeSnapshot(owner,duplicate).self,null);
  source.observePacket(owner,"UserLocation",{x:4,y:5,direction:"Down"});assert.equal(source.current(owner).animationKnown,false,"unknown dead fact requires complete own facts");
});
check("WorldFishingSource complete normal actions preserve bootstrap continuity and raw transform zero",()=>{
  const owner=fishingOwner(),source=new worldFishingSource.WorldFishingSource(),first=source.observeSnapshot(owner,fishingSnapshot(owner));
  source.observePacket(owner,"ObjectAttack",{objectId:17,x:1,y:1,direction:"Right"});const action=source.current(owner);
  assert.equal(action.bootstrapKnown,true);assert.equal(action.continuityRevision,first.continuityRevision);assert.ok(action.revision>first.revision);
  source.observePacket(owner,"TransformUpdate",{objectId:17,transformType:0});assert.equal(source.current(owner).transformType,0);
  source.observePacket(owner,"FishingUpdate",{objectId:17,fishing:false});assert.equal(source.current(owner).fishing,false);
  source.observePacket(owner,"ObjectDied",{objectId:17,x:1,y:1,direction:"Right"});assert.equal(source.current(owner).self.dead,true);
  source.observePacket(owner,"ObjectRevived",{objectId:17,x:1,y:1,direction:"Right"});assert.equal(source.current(owner).self.dead,false);
  source.retire();assert.equal(source.current(owner).animationKnown,false);assert.equal(source.observeSnapshot(owner,fishingSnapshot(owner)).bootstrapKnown,false);
});


check("WorldFishingSource equivalent full snapshots retain exact immutable authority and revision",()=>{
  const owner=fishingOwner(),source=new worldFishingSource.WorldFishingSource(),raw=fishingSnapshot(owner),first=source.observeSnapshot(owner,raw);
  assert.strictEqual(source.observeSnapshot(owner,structuredClone(raw)),first);assert.equal(source.current(owner).revision,first.revision);
  const changed=source.observeSnapshot(owner,fishingSnapshot(owner,{x:1}));assert.notStrictEqual(changed,first);assert.ok(changed.revision>first.revision);
  assert.equal(changed.continuityRevision,first.continuityRevision);assert.equal(changed.bootstrapKnown,true);
  assert.strictEqual(source.observeSnapshot(owner,fishingSnapshot(owner,{x:1})),changed);
  for(const delta of [{fishing:true},{transformType:1},{direction:"Down"},{dead:true}]){
    const before=source.current(owner),next=source.observeSnapshot(owner,fishingSnapshot(owner,delta));assert.notStrictEqual(next,before);assert.ok(next.revision>before.revision);
  }
});
check("WorldFishingSource own Harvest retires authority until a fresh complete canonical action",()=>{
  for(const name of ["ObjectHarvest","ObjectHarvested","Harvest"]){
    const owner=fishingOwner(),source=new worldFishingSource.WorldFishingSource(),first=source.observeSnapshot(owner,fishingSnapshot(owner));
    source.observePacket(owner,name,{objectId:18});assert.strictEqual(source.current(owner),first,"foreign harvest cannot retire own evidence");
    source.observePacket({...owner,socket:{}},name,{objectId:17});assert.strictEqual(source.current(owner),first,"old physical owner cannot retire current evidence");
    source.observePacket(owner,name,{objectId:17,x:0,y:1,direction:"Right"});const unknown=source.current(owner);
    assert.equal(unknown.animationKnown,false,name);assert.equal(unknown.bootstrapKnown,false,name);assert.ok(unknown.continuityRevision>first.continuityRevision,name);
    const snapshot=source.observeSnapshot(owner,fishingSnapshot(owner));assert.equal(snapshot.animationKnown,false);assert.equal(snapshot.bootstrapKnown,false);
    source.observePacket(owner,"ObjectTurn",{objectId:17,x:1,y:1,direction:"Down"});const restored=source.current(owner);
    assert.equal(restored.animationKnown,true);assert.equal(restored.bootstrapKnown,false);assert.ok(restored.continuityRevision>unknown.continuityRevision);
  }
});

function ownedAnimationModule(){
  const calls=[],module={entity_animation_abi_version:()=>1};let constructions=0;
  class Bridge { constructor(){constructions++;} resolveMir2EntityAnimationPoses(json){calls.push(["resolve",this,json]);return "{}";}
    getMir2EntityActionPose(json){calls.push(["peek",this,json]);return "{}";} resetMir2EntityAnimations(){calls.push(["reset",this]);} }
  module.EntityAnimationBridge=Bridge;return {module,calls,Bridge,getConstructions:()=>constructions};
}
check("Owned animation optional facade lazily owns one bridge and preserves actual method receiver",()=>{
  for(const absent of [{},{entity_animation_abi_version:()=>0},{entity_animation_abi_version:()=>1}])assert.equal(presentationRuntime.createSharedEntityAnimationAccessor(absent)(),null);
  const f=ownedAnimationModule(),get=presentationRuntime.createSharedEntityAnimationAccessor(f.module);assert.equal(f.getConstructions(),0);
  const runtime=get();assert.ok(runtime);assert.equal(f.getConstructions(),1);assert.strictEqual(get(),runtime);assert.ok(Object.isFrozen(runtime));
  runtime.resolveMir2EntityAnimationPoses("{}");runtime.getMir2EntityActionPose("{}");runtime.resetMir2EntityAnimations();
  assert.deepEqual(f.calls.map(call=>call[0]),["resolve","peek","reset"]);for(const call of f.calls)assert.strictEqual(call[1],runtime.source);
  const other=presentationRuntime.createSharedEntityAnimationAccessor(f.module)();assert.notStrictEqual(other,runtime);assert.notStrictEqual(other.source,runtime.source);assert.equal(f.getConstructions(),2);
});
check("Owned animation optional facade rejects changed ABI bridge methods and bounded payloads",()=>{
  for(const edge of ["abi","bridge","resolve","peek","reset"]){
    const f=ownedAnimationModule(),get=presentationRuntime.createSharedEntityAnimationAccessor(f.module),runtime=get();
    if(edge==="abi")f.module.entity_animation_abi_version=()=>1;if(edge==="bridge")f.module.EntityAnimationBridge=class extends f.Bridge{};
    if(edge==="resolve")runtime.source.resolveMir2EntityAnimationPoses=()=>"{}";if(edge==="peek")runtime.source.getMir2EntityActionPose=()=>"{}";
    if(edge==="reset")runtime.source.resetMir2EntityAnimations=()=>{};
    assert.equal(get(),null,edge);assert.throws(()=>runtime.getMir2EntityActionPose("{}"),/unavailable/,edge);assert.equal(f.calls.length,0);
  }
  const f=ownedAnimationModule(),runtime=presentationRuntime.createSharedEntityAnimationAccessor(f.module)();
  assert.throws(()=>runtime.getMir2EntityActionPose("x".repeat(2049)),/unavailable/);
  assert.throws(()=>runtime.getMir2EntityActionPose("界".repeat(683)),/unavailable/,"UTF8 budget applies independently of character count");
  assert.throws(()=>runtime.resolveMir2EntityAnimationPoses("x".repeat(2097153)),/unavailable/);assert.equal(f.calls.length,0);
  const bad=ownedAnimationModule();bad.Bridge.prototype.getMir2EntityActionPose=()=>"x".repeat(2049);
  assert.throws(()=>presentationRuntime.createSharedEntityAnimationAccessor(bad.module)().getMir2EntityActionPose("{}"),/unavailable/);
});
check("Owned animation optional facade burns construction attempt and rejects ABI and method reentry",()=>{
  const f=ownedAnimationModule();let get;f.module.EntityAnimationBridge=class extends f.Bridge{constructor(){super();assert.equal(get(),null);}};
  get=presentationRuntime.createSharedEntityAnimationAccessor(f.module);const runtime=get();assert.ok(runtime);assert.strictEqual(get(),runtime);assert.equal(f.getConstructions(),1);
  const failed=ownedAnimationModule();failed.module.EntityAnimationBridge=class{constructor(){throw Error("constructor");}};
  const noRetry=presentationRuntime.createSharedEntityAnimationAccessor(failed.module);assert.equal(noRetry(),null);failed.module.EntityAnimationBridge=failed.Bridge;assert.equal(noRetry(),null);assert.equal(failed.getConstructions(),0);
  const nested=ownedAnimationModule();let facade,access;const events=[];
  nested.Bridge.prototype.getMir2EntityActionPose=function(json){events.push(json);assert.equal(access(),null);
    assert.throws(()=>facade.getMir2EntityActionPose("{}"),/unavailable/);assert.throws(()=>facade.resetMir2EntityAnimations(),/unavailable/);return "{}";};
  access=presentationRuntime.createSharedEntityAnimationAccessor(nested.module);facade=access();assert.equal(facade.getMir2EntityActionPose("{}"),"{}");assert.deepEqual(events,["{}"]);assert.strictEqual(access(),facade);
  const changed=ownedAnimationModule();changed.Bridge.prototype.getMir2EntityActionPose=function(){changed.module.entity_animation_abi_version=()=>1;return "{}";};
  const changedAccess=presentationRuntime.createSharedEntityAnimationAccessor(changed.module);assert.throws(()=>changedAccess().getMir2EntityActionPose("{}"),/unavailable/);assert.equal(changedAccess(),null);
});
const fishingCoreAst=ts.createSourceFile("client-core-runtime.ts",readFileSync(tooltipCoreUrl,"utf8"),ts.ScriptTarget.Latest,true);
const fishingCoreMethods=[],fishingAccessorDeclarations=[];
(function visit(node){if(ts.isMethodDeclaration(node)&&["getEntityAnimationRuntime","fishingClickTargets","decideFishingClick"].includes(node.name.getText(fishingCoreAst)))fishingCoreMethods.push(node);
  if(ts.isVariableDeclaration(node)&&node.name.getText(fishingCoreAst)==="getEntityAnimationRuntime")fishingAccessorDeclarations.push(node);ts.forEachChild(node,visit);})(fishingCoreAst);
assert.equal(fishingCoreMethods.length,3,"three actual optional Core fishing and animation proxies");assert.equal(fishingAccessorDeclarations.length,1,"sole actual lazy owned animation accessor");
const fishingCoreJs=ts.transpileModule("const "+fishingAccessorDeclarations[0].getText(fishingCoreAst)+";const facade={"+fishingCoreMethods.map(node=>node.getText(fishingCoreAst)).join(",")+"};",
  {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
check("Core actual fishing and animation facade forwards optional presentation capabilities independently",()=>{
  const f=ownedAnimationModule(),requests=[];Object.assign(f.module,{fishing_click_abi_version:()=>1,
    fishing_click_targets:json=>{requests.push(JSON.parse(json));return JSON.stringify(fishingTargetsReply());},
    fishing_click_decision:json=>{requests.push(JSON.parse(json));return JSON.stringify({version:1,ok:true,decision:{type:"none"}});}});
  const create=module=>new Function("presentation","createSharedEntityAnimationAccessor","readSharedFishingClickTargets","readSharedFishingClickDecision",fishingCoreJs+"\nreturn facade;")(
    module,presentationRuntime.createSharedEntityAnimationAccessor,presentationRuntime.readSharedFishingClickTargets,presentationRuntime.readSharedFishingClickDecision);
  const core=create(f.module);assert.ok(core.getEntityAnimationRuntime());assert.deepEqual(core.fishingClickTargets(fishingTargetsInput()).waterTarget,{x:7,y:8});assert.deepEqual(core.decideFishingClick(fishingDecisionInput()),{type:"none"});
  assert.deepEqual(requests,[{version:1,...fishingTargetsInput()},{version:1,...fishingDecisionInput()}]);
  const missing=create({});assert.equal(missing.getEntityAnimationRuntime(),null);assert.equal(missing.fishingClickTargets(fishingTargetsInput()),null);assert.equal(missing.decideFishingClick(fishingDecisionInput()),null);
});
function fishingPoseFixture(){
  const owner=fishingOwner(),source=new worldFishingSource.WorldFishingSource().observeSnapshot(owner,fishingSnapshot(owner)),requests=[];
  let reply={version:1,known:true,worldKey:"world:0",objectId:"17",actionSourceKey:source.sourceKey,sourceRevision:source.revision,
    continuityRevision:source.continuityRevision,bridgeEpoch:1,incarnation:1,lastNowMs:1000,action:"standing",direction:"Right"};
  const runtime={getMir2EntityActionPose(json){requests.push(JSON.parse(json));return JSON.stringify(reply);}},
    commit={context:{runtime,record:source},worldKey:"world:0",worldSeed:7,atMs:1000,token:{}};
  return {runtime,commit,source,requests,getReply:()=>reply,setReply:value=>reply=value};
}
check("Animation action peek is read only exact authority revision and fresh shared pose without standing fallback",()=>{
  const f=fishingPoseFixture(),read=now=>fishingAnimation.readCrystalEntityActionPose(f.runtime,f.commit,f.source,now);
  const pose=read(1250);assert.ok(pose);assert.ok(Object.isFrozen(pose));assert.equal(pose.action,"standing");
  assert.deepEqual(f.requests,[{version:1,worldKey:"world:0",worldSeed:7,objectId:"17",actionSourceKey:f.source.sourceKey}]);
  assert.equal(read(1251),null,"250ms inclusive freshness boundary");assert.equal(read(999),null,"future pose cannot become current");
  for(const now of [-1,1.5,NaN,"1000"])assert.equal(read(now),null);
  assert.equal(fishingAnimation.readCrystalEntityActionPose({},f.commit,f.source,1000),null);
  assert.equal(fishingAnimation.readCrystalEntityActionPose({...f.runtime},f.commit,f.source,1000),null,"different current runtime cannot borrow pose");
  assert.equal(fishingAnimation.readCrystalEntityActionPose(f.runtime,f.commit,{...f.source,revision:f.source.revision+1},1000),null);
  for(const reply of [null,[],{version:1,known:false},{...f.getReply(),action:"unknown"},{...f.getReply(),direction:"right"},
    {...f.getReply(),lastNowMs:1.5},{...f.getReply(),sourceRevision:0},{...f.getReply(),continuityRevision:0},
    {...f.getReply(),sourceRevision:f.source.revision+1},{...f.getReply(),objectId:"18"},{...f.getReply(),extra:true}]){
    f.setReply(reply);assert.equal(read(1000),null,"unknown or malformed peek cannot default to standing");
  }
  assert.ok(f.requests.length>0);
});
check("Animation action peek rejects getter replacement and does not advance or resolve an animation world",()=>{
  const f=fishingPoseFixture();let resolves=0,resets=0;f.runtime.resolveMir2EntityAnimationPoses=()=>{resolves++;return "{}";};f.runtime.resetMir2EntityAnimations=()=>{resets++;};
  const original=f.runtime.getMir2EntityActionPose;f.runtime.getMir2EntityActionPose=function(json){const raw=original(json);this.getMir2EntityActionPose=()=>raw;return raw;};
  assert.equal(fishingAnimation.readCrystalEntityActionPose(f.runtime,f.commit,f.source,1000),null);assert.equal(resolves,0);assert.equal(resets,0);assert.equal(f.requests.length,1);
});
check("Animation renderer forwards exact clock and canonical action tokens while missing event facts remain unknown",()=>{
  const owner=fishingOwner(),source=new worldFishingSource.WorldFishingSource().observeSnapshot(owner,fishingSnapshot(owner)),requests=[],
    runtime={resolveMir2EntityAnimationPoses(json){const raw=JSON.parse(json);requests.push(raw);return JSON.stringify({worldKey:raw.worldKey,nowMs:raw.nowMs,poses:[]});}},
    entity={objectId:"17",kind:"selfPlayer",x:0,y:1,direction:"Right",movementAnimation:"walking",movementStartedAt:123,attackStartedAt:234,attackAnimation:"melee2"};
  fishingAnimation.resolveCrystalEntityAnimationPoses({runtime,worldKey:"world:0",worldSeed:7,now:1000,selfAuthority:source,entities:[{entity,state:"walking"}]});
  assert.deepEqual(requests[0],{worldKey:"world:0",worldSeed:7,nowMs:1000,entities:[{objectId:"17",kind:"selfPlayer",direction:"Right",action:"walking",actionToken:"move:123:walking",
    actionSourceKey:source.sourceKey,actionSourceRevision:source.revision,bootstrapKnown:true,actionKnown:true}]});
  assert.deepEqual(fishingAnimation.animationEventForEntity(entity,"attackMelee"),{action:"attack2",actionToken:"attack:234:attack2"});
  assert.deepEqual(fishingAnimation.animationEventForEntity({...entity,movementStartedAt:0},"running"),{action:"running",actionToken:"move:0:running"});
  for(const attackAnimation of ["melee1","melee2","melee3","melee4"]){const action="attack"+(Number(attackAnimation.slice(-1)));assert.deepEqual(fishingAnimation.animationEventForEntity({...entity,attackAnimation},"attackMelee"),{action,actionToken:"attack:234:"+action});}
  assert.deepEqual(fishingAnimation.animationEventForEntity({...entity,struckStartedAt:456},"struck"),{action:"struck",actionToken:"struck:456"});
  fishingAnimation.resolveCrystalEntityAnimationPoses({runtime,worldKey:"world:0",worldSeed:7,now:1001,selfAuthority:source,entities:[{entity:{...entity,movementStartedAt:undefined},state:"walking"}]});
  assert.equal(requests[1].entities[0].actionKnown,false);assert.equal(Object.hasOwn(requests[1].entities[0],"actionToken"),false);
  const foreign={...entity,objectId:"18"};fishingAnimation.resolveCrystalEntityAnimationPoses({runtime,worldKey:"world:0",worldSeed:7,now:1002,selfAuthority:source,entities:[{entity:foreign,state:"standing"}]});
  assert.equal(Object.hasOwn(requests[2].entities[0],"actionKnown"),false,"own source never promotes another entity");
  fishingAnimation.resolveCrystalEntityAnimationPoses({runtime,worldKey:"world:0",worldSeed:7,now:1003,selfAuthority:{...source,animationKnown:false,bootstrapKnown:false},entities:[{entity,state:"standing"}]});
  assert.equal(requests[3].entities[0].actionKnown,false);assert.equal(requests[3].entities[0].bootstrapKnown,false,"legacy default standing cannot bootstrap retired own authority");
  runtime.resolveMir2EntityAnimationPoses=()=>JSON.stringify({worldKey:"world:0",nowMs:999,poses:[{objectId:"17",incarnation:1,animationState:"standing",action:"standing",direction:"Right",logicalFrameIndex:0,queueDepth:0}]});
  assert.deepEqual(fishingAnimation.resolveCrystalEntityAnimationPoses({runtime,worldKey:"world:0",worldSeed:7,now:1002,entities:[{entity,state:"standing"}]}),{},"wrong animation clock cannot publish a pose");
});


check("WorldFishingSource captures immutable canonical action evidence without invented receive clocks",()=>{
  const owner=fishingOwner(),source=new worldFishingSource.WorldFishingSource();source.observeSnapshot(owner,fishingSnapshot(owner));
  source.observePacket(owner,"ObjectWalk",{objectId:17,x:1,y:1,direction:"Right"},1000);const first=source.current(owner);
  assert.equal(first.actionEvidence.family,"walking");assert.equal(first.actionEvidence.receivedAtMs,1000);assert.ok(Object.isFrozen(first.actionEvidence));
  assert.equal(first.actionEvidence.token,first.sourceKey+":action:"+first.actionEvidence.id);
  source.observePacket(owner,"ObjectRun",{objectId:17,x:2,y:1,direction:"Right"});const second=source.current(owner);
  assert.equal(second.actionEvidence.receivedAtMs,null);assert.equal(second.actionEvidence.family,"running");assert.ok(second.actionEvidence.id>first.actionEvidence.id);assert.notEqual(second.actionEvidence.token,first.actionEvidence.token);
  for(const clock of [-1,1.5,"1000",Number.MAX_SAFE_INTEGER+1]){
    source.observePacket(owner,"ObjectAttack",{objectId:17,x:2,y:1,direction:"Right"},clock);assert.equal(source.current(owner).actionEvidence.receivedAtMs,null);
  }
});
check("WorldFishingSource UserLocation updates transform but cannot heal unknown or fabricate standing",()=>{
  const owner=fishingOwner(),source=new worldFishingSource.WorldFishingSource();source.observeSnapshot(owner,fishingSnapshot(owner));
  source.observePacket(owner,"ObjectWalk",{objectId:17,x:1,y:1,direction:"Right"},1000);const walking=source.current(owner);
  source.observePacket(owner,"UserLocation",{x:2,y:1,direction:"Down"},1001);const ack=source.current(owner);
  assert.strictEqual(ack.actionEvidence,walking.actionEvidence);assert.equal(ack.animationKnown,true);assert.equal(ack.actionEvidence.family,"walking");assert.deepEqual(ack.self,{x:2,y:1,direction:"Down",dead:false});
  source.observePacket(owner,"Harvest",{objectId:17});const unknown=source.current(owner);
  source.observePacket(owner,"UserLocation",{x:3,y:1,direction:"Right"},1002);const corrected=source.current(owner);
  assert.equal(corrected.animationKnown,false);assert.equal(corrected.bootstrapKnown,false);assert.equal(corrected.actionEvidence,null);assert.equal(corrected.continuityRevision,unknown.continuityRevision);
  source.observeSnapshot(owner,fishingSnapshot(owner,{x:3}));assert.equal(source.current(owner).animationKnown,false);
  source.observePacket(owner,"ObjectTurn",{objectId:17,x:3,y:1,direction:"Right"},1003);assert.equal(source.current(owner).animationKnown,true);assert.equal(source.current(owner).actionEvidence.family,"standing");
});
function fishingAnimationEventFixture(){
  const owner=fishingOwner(),source=new worldFishingSource.WorldFishingSource();source.observeSnapshot(owner,fishingSnapshot(owner));const requests=[];
  const runtime={resolveMir2EntityAnimationPoses(json){const raw=JSON.parse(json);requests.push(raw);return JSON.stringify({worldKey:raw.worldKey,nowMs:raw.nowMs,poses:[]});}};
  const render=(record,entity,state,motionSnapshot)=>{fishingAnimation.resolveCrystalEntityAnimationPoses({runtime,worldKey:"source23:events",worldSeed:1,now:1000,selfAuthority:record,entities:[{entity,state,motionSnapshot}]});return requests.at(-1).entities[0];};
  const entity={objectId:"17",kind:"selfPlayer",x:0,y:1,direction:"Right",movementAnimation:"walking",movementStartedAt:1000};
  return {owner,source,entity,requests,render};
}
check("Animation own transient authority rejects missing receive time old timestamps and metadata mismatch",()=>{
  for(const edge of ["missingReceive","missingTimestamp","oldTimestamp","wrongAnimation","wrongFamily","wrongTransform"]){
    const f=fishingAnimationEventFixture();f.source.observePacket(f.owner,"ObjectWalk",{objectId:17,x:0,y:1,direction:"Right"},edge==="missingReceive"?undefined:1000);
    const record=f.source.current(f.owner),entity={...f.entity};if(edge==="missingTimestamp")delete entity.movementStartedAt;
    if(edge==="oldTimestamp")entity.movementStartedAt=999;if(edge==="wrongAnimation")entity.movementAnimation="running";
    if(edge==="wrongTransform")entity.x=9;const output=f.render(record,entity,edge==="wrongFamily"?"attackMelee":"walking");
    assert.equal(output.actionKnown,false,edge);assert.equal(output.bootstrapKnown,false,edge);
    assert.equal(f.render(record,entity,"standing").actionKnown,false,"failed transient cannot later become standing authority");
  }
  const f=fishingAnimationEventFixture();f.source.observePacket(f.owner,"ObjectWalk",{objectId:17,x:0,y:1,direction:"Right"},1000);
  const record=f.source.current(f.owner),output=f.render(record,{...f.entity,movementStartedAt:999},"walking",{animationState:"walking",startedAt:1001});
  assert.equal(output.actionKnown,false,"matching actual entity metadata at old clock cannot borrow motion to heal itself");
  const motion=f.render(record,{...f.entity,movementAnimation:undefined,movementStartedAt:999},"walking",{animationState:"walking",startedAt:1001});
  assert.equal(motion.actionKnown,true);assert.equal(motion.actionToken,record.actionEvidence.token+":move:1001:walking","canonical motion timestamp wins when entity has no matching animation metadata");
});
check("Animation own descriptor cache preserves canonical token after selector standing and rejects reused old event",()=>{
  const f=fishingAnimationEventFixture();f.source.observePacket(f.owner,"ObjectWalk",{objectId:17,x:0,y:1,direction:"Right"},1000);
  const first=f.source.current(f.owner),walking=f.render(first,f.entity,"walking"),token=first.actionEvidence.token+":move:1000:walking";
  assert.equal(walking.actionKnown,true);assert.equal(walking.actionToken,token);assert.equal(walking.bootstrapKnown,false);
  const completed=f.render(first,{...f.entity,movementAnimation:undefined,movementStartedAt:undefined},"standing");
  assert.equal(completed.action,"walking");assert.equal(completed.actionKnown,true);assert.equal(completed.actionToken,token,"Rust owns natural completion while host repeats its consumed event");
  f.source.observePacket(f.owner,"ObjectWalk",{objectId:17,x:0,y:1,direction:"Right"},1000);const next=f.source.current(f.owner);
  assert.notEqual(next.actionEvidence.token,first.actionEvidence.token);assert.equal(f.render(next,f.entity,"walking").actionKnown,false,"next descriptor cannot reuse consumed timestamp");
  const fresh=f.render(next,{...f.entity,movementStartedAt:1001},"walking");assert.equal(fresh.actionKnown,true);assert.equal(fresh.actionToken,next.actionEvidence.token+":move:1001:walking");
  assert.equal(f.render(next,f.entity,"standing").actionToken,fresh.actionToken);
});
check("Animation action peek binds exact captured layout clock and independent bridge authority counters",()=>{
  const f=fishingPoseFixture(),base={...f.getReply(),continuityRevision:19};f.setReply(base);
  const pose=fishingAnimation.readCrystalEntityActionPose(f.runtime,f.commit,f.source,1250);assert.ok(pose);assert.equal(pose.continuityRevision,19);assert.notEqual(pose.continuityRevision,f.source.continuityRevision);
  for(const lastNowMs of [999,1001]){f.setReply({...base,lastNowMs});assert.equal(fishingAnimation.readCrystalEntityActionPose(f.runtime,f.commit,f.source,1250),null,"resolve ahead or behind captured commit cannot borrow its authority");}
  f.setReply(base);assert.equal(fishingAnimation.readCrystalEntityActionPose(f.runtime,{...f.commit,context:{...f.commit.context,record:{...f.source}}},f.source,1000),null,"equivalent record copy cannot replace committed identity");
});
check("Animation action peek rejects reentrant commit context owner source and getter mutation",()=>{
  for(const edge of ["context","runtime","record","worldKey","worldSeed","atMs","token","sourceKey","revision","owner","playerId","getter"]){
    const f=fishingPoseFixture(),source={...f.source,owner:{...f.source.owner}},context={runtime:f.runtime,record:source},commit={...f.commit,context},original=f.runtime.getMir2EntityActionPose;
    f.runtime.getMir2EntityActionPose=function(json){const reply=original(json);
      if(edge==="context")commit.context={...context};if(edge==="runtime")context.runtime={...f.runtime};if(edge==="record")context.record={...source};
      if(edge==="worldKey")commit.worldKey="successor";if(edge==="worldSeed")commit.worldSeed++;if(edge==="atMs")commit.atMs++;if(edge==="token")commit.token={};
      if(edge==="sourceKey")source.sourceKey+="changed";if(edge==="revision")source.revision++;if(edge==="owner")source.owner={...source.owner};if(edge==="playerId")source.owner.playerObjectId++;
      if(edge==="getter")f.runtime.getMir2EntityActionPose=()=>reply;return reply;};
    assert.equal(fishingAnimation.readCrystalEntityActionPose(f.runtime,commit,source,1000),null,edge);assert.equal(f.requests.length,1,edge);
  }
});


// Actual Page custody and socket entry with bounded in-memory ABI data and
// physical-owner records. No network connection or animation engine is created.
const fishingPageSource=readFileSync(parityPageUrl,"utf8"),fishingPageAst=ts.createSourceFile("page.tsx",fishingPageSource,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const fishingPageNames=new Set(["worldFishingOwner","cancelWorldFishingGesture","retireWorldFishingGesture","publishWorldFishingSource",
  "captureWorldFishingGatewayEvent","captureWorldFishingSnapshot","commitWorldFishingAnimation","readWorldFishingRod","worldFishingInputAllowed","worldFishingPureUiBlocked",
  "worldFishingLeaseCurrent","beginWorldFishingGesture","worldFishingMapCell","worldFishingFinalCurrent","enterWorldFishingSend","tryWorldFishingBlockedClick",
  "currentSpellsOwner","otherPlayerUiBlocksInput","heroUiBlocksGameplay","referenceWindowsBlockGameplay","mapRouteInputBlocked","sceneInputDeferredForInitialAssets",
  "originalMapRegionContainsTile","originalMapCellBlocksMovement"]),fishingPageDeclarations=new Map();
(function visit(node){if(ts.isFunctionDeclaration(node)&&fishingPageNames.has(node.name?.text))fishingPageDeclarations.set(node.name.text,node.getText(fishingPageAst));ts.forEachChild(node,visit);})(fishingPageAst);
assert.equal(fishingPageDeclarations.size,fishingPageNames.size,"all actual fishing Page and input authority declarations");
const fishingSendNode=[];(function visit(node){if(ts.isFunctionDeclaration(node)&&node.name?.text==="sendRaw")fishingSendNode.push(node);ts.forEachChild(node,visit);})(fishingPageAst);
assert.equal(fishingSendNode.length,1);const fishingSendStatements=fishingSendNode[0].body.statements;
const fishingSendEntry=fishingSendStatements.filter(node=>ts.isIfStatement(node)&&node.expression.getText(fishingPageAst)==="options?.worldFishingProof && !enterWorldFishingSend(options.worldFishingProof, wireCommand, socket)");
assert.equal(fishingSendEntry.length,1,"sole actual final WorldFishing irreversible entry");
const fishingSocketIndex=fishingSendStatements.findIndex(node=>ts.isTryStatement(node)&&node.tryBlock.statements.some(child=>ts.isExpressionStatement(child)&&child.expression.getText(fishingPageAst)==="socket.send(serialized)"));
const fishingFollowingPearlEntry=fishingSendStatements[fishingSendStatements.indexOf(fishingSendEntry[0])+1];
assert.ok(ts.isIfStatement(fishingFollowingPearlEntry));
assert.equal(fishingFollowingPearlEntry.expression.getText(fishingPageAst),bagBeltFollowingPearlEntry.expression.getText(parityPageAst),"same actual Pearl claim guard after Fishing and Bag custody");
const fishingFollowingRankingInspectEntry=fishingSendStatements[fishingSendStatements.indexOf(fishingSendEntry[0])+2];
assert.ok(ts.isIfStatement(fishingFollowingRankingInspectEntry));
assert.equal(fishingFollowingRankingInspectEntry.expression.getText(fishingPageAst),bagBeltFollowingRankingInspectEntry.expression.getText(parityPageAst),
  "same exact Ranking Inspect guard after the unchanged Fishing and Pearl claims");
assert.equal(fishingSendStatements.indexOf(fishingFollowingRankingInspectEntry)+1,fishingSocketIndex,"Fishing fixture Ranking Inspect guard adjoins actual socket try");
assert.equal(fishingSendStatements.indexOf(fishingSendEntry[0])+3,fishingSocketIndex,"WorldFishing retains exact Pearl guard immediately before actual socket send");
const fishingSerializeIndex=fishingSendStatements.findIndex(node=>ts.isVariableStatement(node)&&node.declarationList.declarations.some(decl=>decl.name.getText(fishingPageAst)==="serialized"));
const fishingDtoIndex=fishingSendStatements.findIndex(node=>ts.isVariableStatement(node)&&node.declarationList.declarations.some(decl=>decl.name.getText(fishingPageAst)==="wireCommand"));
const fishingPreflightGuards=fishingSendStatements.filter(node=>ts.isIfStatement(node)&&['command.type === "fishingCast" && command.castOut === true && !options?.worldFishingProof','options?.worldFishingProof && !worldFishingFinalCurrent(options.worldFishingProof, command)'].includes(node.expression.getText(fishingPageAst)));
assert.equal(fishingPreflightGuards.length,2,"actual raw fishing cast requires its opaque source proof");
const fishingPageJs=ts.transpileModule([...fishingPageDeclarations.values()].join("\n")+"\nfunction fishingSocketEntry(command,options,socket,beforeFinal){"+
  fishingPreflightGuards.map(node=>node.getText(fishingPageAst)).join("\n")+"\n"+fishingSendStatements.slice(fishingSerializeIndex,fishingSerializeIndex+2).map(node=>node.getText(fishingPageAst)).join("\n")+
  "\nif(beforeFinal)beforeFinal();\n"+fishingSendStatements.slice(fishingDtoIndex,fishingDtoIndex+2).map(node=>node.getText(fishingPageAst)).join("\n")+
  "\n"+fishingSendEntry[0].getText(fishingPageAst)+"\n"+fishingFollowingPearlEntry.getText(fishingPageAst)+"\n"+fishingFollowingRankingInspectEntry.getText(fishingPageAst)+"\n"+fishingSendStatements[fishingSocketIndex].getText(fishingPageAst)+"\nreturn true;}",
  {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
function worldFishingPageFixture(){
  const owner=npcRepairOwner(),rod={...npcRepairItem(0,"weapon"),authoritativeUniqueId:0};delete rod.container;
  rod.tooltipSource.info={...rod.tooltipSource.info,item_type:1,shape:49};rod.tooltipSource.userItem.slots=[null,null,null,null,null];
  const raw=npcRepairSnapshot({inventoryItems:[],beltItems:[],equipmentItems:[rod]}),page=parityPageFixture(owner,raw),ref=current=>({current}),
    region={mapFileName:"M001.map",regionBounds:{minX:0,minY:0,maxX:10,maxY:10},cells:[{x:1,y:1,blocked:true},{x:1,y:0,blocked:true},{x:1,y:2,blocked:true},{x:4,y:1,light:105}]},
    self={objectId:17,kind:"selfPlayer",x:0,y:1,direction:"Right",dead:false,fishing:false,transformType:0},
    layout=equipmentGateway.projectEquipmentGatewaySnapshot(raw);assert.ok(layout);
  page.scope.worldRef.current={...page.scope.worldRef.current,entities:[self],originalMapRegion:region};page.scope.equipmentSnapshotRef.current={...owner,snapshot:layout};
  page.scope.bagBeltInventoryReadyRef.current={layout,raw,signature:JSON.stringify([raw.inventoryCapacity,raw.inventoryItems,raw.beltItems]),epoch:1,snapshotVersion:1};
  for(const name of ["heroManagementOpenRef","heroPetOpenRef","cashShopOpenRef"])page.scope[name].current=false;
  page.scope.socialItemWindowsRef.current={guild:false,trade:false};page.scope.equipmentControllerRef.current={status:()=>({ready:true,pending:0})};
  let now=1000,beforeFinal=null,throwTransport=false,abiEdge=null,peekEdge=null;const requests=[],peekRequests=[],errors=[],entryProofs=[];
  const fishingModule={fishing_click_abi_version:()=>1,fishing_click_targets(json){const input=JSON.parse(json);requests.push(["targets",input]);abiEdge?.("targets");
    return JSON.stringify({version:1,ok:true,walkCandidates:[{direction:2,cell:{x:1,y:1}},{direction:1,cell:{x:1,y:0}},{direction:3,cell:{x:1,y:2}}],waterTarget:{x:4,y:1}});},
    fishing_click_decision(json){const input=JSON.parse(json);requests.push(["decision",input]);abiEdge?.("decision");return JSON.stringify({version:1,ok:true,decision:
      input.standing===true&&input.facingMatches===true&&input.fishing===false&&input.transformType===0&&input.water?.light>=100&&input.water?.light<=119?{type:"cast",lastCastMs:input.nowMs}:{type:"none"}});}};
  const runtime={getMir2EntityActionPose(json){const query=JSON.parse(json);peekRequests.push(query);const source=scope.worldFishingSourceRef.current.current(api.worldFishingOwner());
    const reply={version:1,known:true,worldKey:"world:m001",objectId:"17",actionSourceKey:source?.sourceKey,sourceRevision:source?.revision,
      continuityRevision:19,bridgeEpoch:2,incarnation:3,lastNowMs:scope.worldFishingCommitRef.current?.atMs,action:"standing",direction:"Right"};peekEdge?.(reply);return JSON.stringify(reply);}},
    core={fishingClickTargets:input=>presentationRuntime.readSharedFishingClickTargets(fishingModule,input),decideFishingClick:input=>presentationRuntime.readSharedFishingClickDecision(fishingModule,input)};
  page.scope.questCoreRuntimeRef.current=core;
  const scope={...page.scope,Date:{now:()=>now},WorldFishingSource:worldFishingSource.WorldFishingSource,sameWorldFishingOwner:worldFishingSource.sameOwner,
    readCrystalEntityActionPose:fishingAnimation.readCrystalEntityActionPose,ownedItemTooltipRequest:page.api.ownedItemTooltipRequest,
    // No Hero surface is mounted here; execute the real shared/fallback gate.
    heroSharedUiIngressRef:ref(null),
    mapRoutePageBlockedRef:ref(false),questWindowOpenRef:ref(false),firstPlayableFrameMarkedRef:ref(true),
    worldFishingSourceRef:ref(new worldFishingSource.WorldFishingSource()),worldFishingRuntimeRef:ref({core,runtime}),worldFishingCommitRef:ref(null),
    worldFishingGestureRegistryRef:ref(new WeakMap()),worldFishingActiveGestureRef:ref(null),worldFishingSendProofsRef:ref(new WeakMap()),worldFishingQueuedRef:ref(null),
    worldFishingCastClockRef:ref(null),queuedMoveIntentRef:ref(null),questRouteRunRef:ref(null),nextMoveSendAtRef:ref(0),
    setWorldFishingRecord:()=>{},console:{error:(...args)=>errors.push(args)},
    sendRaw:(command,options)=>{entryProofs.push(options.worldFishingProof);return api.fishingSocketEntry(command,options,page.socket,beforeFinal);},
    sendCrystalTurn:(direction,proof)=>{entryProofs.push(proof);return api.fishingSocketEntry({type:"turn",direction},{worldFishingProof:proof},page.socket,beforeFinal);}};
  const keys=Object.keys(scope),api=new Function(...keys,fishingPageJs+"\nreturn {worldFishingOwner,captureWorldFishingSnapshot,captureWorldFishingGatewayEvent,commitWorldFishingAnimation,beginWorldFishingGesture,cancelWorldFishingGesture,retireWorldFishingGesture,worldFishingLeaseCurrent,worldFishingMapCell,worldFishingFinalCurrent,tryWorldFishingBlockedClick,fishingSocketEntry,readWorldFishingRod,worldFishingPureUiBlocked};")(...keys.map(key=>scope[key]));
  page.socket.send=body=>{if(throwTransport)throw Error("fixture unknown transport");page.sentBodies.push(body);page.sent.push(JSON.parse(body));};
  api.captureWorldFishingSnapshot({...raw,entities:[self]},owner.connectionGeneration);
  const source=scope.worldFishingSourceRef.current.current(api.worldFishingOwner());assert.ok(source?.animationKnown,"actual raw M001.map snapshot must establish normalized map authority");assert.ok(api.readWorldFishingRod(),"actual raw UID-zero rod source must pass current equipment custody");
  const commit=Object.freeze({context:Object.freeze({runtime,record:source}),worldKey:"world:m001",worldSeed:7,atMs:now,token:Object.freeze({})});api.commitWorldFishingAnimation(commit);assert.strictEqual(scope.worldFishingCommitRef.current,commit);
  const queued={kind:"direction",direction:"Right",requestedMode:"walk",requestedAt:now};let pointerCurrent=true,pointerEdge=null;
  const pointer={source:{},stage:{},pointerId:1,pointerType:"mouse",startedAt:now,current:()=>{pointerEdge?.();return pointerCurrent;}};
  const begin=()=>{const gesture=api.beginWorldFishingGesture(pointer);assert.ok(gesture);scope.queuedMoveIntentRef.current=queued;return gesture;};
  return {...page,scope,api,raw,rod,region,self,core,runtime,fishingModule,source,commit,queued,pointer,requests,peekRequests,errors,entryProofs,begin,
    setNow:value=>now=value,setPointerCurrent:value=>pointerCurrent=value,setPointerEdge:value=>pointerEdge=value,setBeforeFinal:value=>beforeFinal=value,setThrow:value=>throwTransport=value,
    setAbiEdge:value=>abiEdge=value,setPeekEdge:value=>peekEdge=value};
}


check("Fishing Page actual raw UID-zero rod and normalized map sends one exact cast through consumed socket proof",()=>{
  for(const throws of [false,true]){
    const f=worldFishingPageFixture(),gesture=f.begin();f.setThrow(throws);
    assert.equal(f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000),true);
    assert.deepEqual(f.requests,[ ["targets",{version:1,origin:{x:0,y:1},direction:2}],
      ["decision",{version:1,origin:{x:0,y:1},direction:2,requestedWalk:true,autoRoute:false,walkBlocked:[true,true,true],rodPresent:true,
        water:{cell:{x:4,y:1},light:105},facingMatches:true,standing:true,fishing:false,transformType:0,nowMs:1000,lastCastMs:0}] ]);
    assert.equal(f.scope.worldFishingCastClockRef.current.lastCastMs,1000,"entered outcome owns the shared supplied cast clock even on transport throw");
    assert.equal(f.entryProofs.length,1);assert.equal(f.scope.worldFishingSendProofsRef.current.has(f.entryProofs[0]),false);
    assert.equal(f.api.fishingSocketEntry({type:"fishingCast",castOut:true},{worldFishingProof:f.entryProofs[0]},f.socket),false,"consumed final proof cannot replay");
    assert.equal(f.api.fishingSocketEntry({type:"fishingCast",castOut:true},{},f.socket),false,"unproved raw cast cannot use ordinary send");
    if(throws){assert.equal(f.sent.length,0);assert.equal(f.errors.length,1);}else{assert.deepEqual(f.sent,[{type:"fishingCast",castOut:true}]);assert.equal(Object.keys(f.sent[0]).length,2);}
    f.api.cancelWorldFishingGesture(gesture);assert.equal(f.scope.worldFishingCastClockRef.current.lastCastMs,1000,"pointer retirement cannot unconsume unknown transport clock");
  }
});
check("Fishing Page actual all-three blockers typed water and readonly standing gates never invent cast facts",()=>{
  for(const edge of ["run","autoQuest","autoMap","walkable","unknownCell","duplicateCell","light5","lightString","noCommit","staleCommit","wrongPose","oldPose","missingRod"]){
    const f=worldFishingPageFixture(),gesture=f.begin();
    if(edge==="run")f.queued.requestedMode="run";if(edge==="autoQuest")f.scope.questRouteRunRef.current={queued:f.queued};if(edge==="autoMap")f.scope.mapImageRouteRef.current={queued:f.queued};
    if(edge==="walkable")f.region.cells[0].blocked=false;if(edge==="unknownCell")f.region.cells[0].blocked=1;if(edge==="duplicateCell")f.region.cells.push({...f.region.cells[0]});
    if(edge==="light5")f.region.cells[3].light=5;if(edge==="lightString")f.region.cells[3].light="105";if(edge==="noCommit")f.scope.worldFishingCommitRef.current=null;
    if(edge==="staleCommit")f.setNow(1251);if(edge==="wrongPose")f.setPeekEdge(reply=>{reply.action="walking";});if(edge==="oldPose")f.setPeekEdge(reply=>{reply.lastNowMs=999;});
    if(edge==="missingRod")f.scope.worldRef.current.equipmentItems=[];
    f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,edge==="staleCommit"?1251:1000);
    assert.equal(f.sent.length,0,edge);assert.equal(f.scope.worldFishingCastClockRef.current.lastCastMs,0,edge);
    if(["run","autoQuest","autoMap"].includes(edge))assert.equal(f.requests.length,0,"excluded movement owner never invokes fishing Core");
  }
});
check("Fishing Page actual final getter and synchronous listener reentry reject changed source owner gates and bridge counters",()=>{
  for(const at of ["targets","decision","finalPeek","listener"])for(const edge of ["socket","session","source","runtime","core","rod","layout","modal","hidden","storage","social","mail",...(["finalPeek","listener"].includes(at)?["epoch","incarnation","continuity"]:[])]){
    const f=worldFishingPageFixture(),gesture=f.begin();let peeks=0,changed=false;
    const mutate=reply=>{if(changed)return;changed=true;
      if(edge==="socket")f.scope.socketRef.current={readyState:1};if(edge==="session")f.scope.equipmentSessionGenerationRef.current++;
      if(edge==="source")f.scope.worldFishingSourceRef.current.observePacket(f.api.worldFishingOwner(),"Harvest",{objectId:17},1000);
      if(edge==="runtime")f.scope.worldFishingRuntimeRef.current={...f.scope.worldFishingRuntimeRef.current,runtime:{...f.runtime}};
      if(edge==="core")f.scope.questCoreRuntimeRef.current={...f.core};if(edge==="rod")f.rod.tooltipSource.info.shape=1;
      if(edge==="layout")f.scope.equipmentSnapshotRef.current={...f.scope.equipmentSnapshotRef.current,snapshot:{...f.scope.equipmentSnapshotRef.current.snapshot}};
      if(edge==="modal")f.scope.mapRouteLocalModalRef.current=true;if(edge==="hidden")f.scope.document.visibilityState="hidden";
      if(edge==="storage")f.scope.pendingStorageRequestsRef.current.set("new",{});
      if(edge==="social")Object.defineProperty(f.scope.socialItemOperationsRef.current,"pending",{value:{},configurable:true});
      if(edge==="mail")f.scope.mailParcelRef.current={state:{blockedUniqueIds:[0]}};
      if(["epoch","incarnation","continuity"].includes(edge)){const key=edge==="epoch"?"bridgeEpoch":edge==="continuity"?"continuityRevision":"incarnation";
        if(reply)reply[key]++;else f.setPeekEdge(value=>{value[key]++;});}
    };
    if(at==="targets"||at==="decision")f.setAbiEdge(kind=>{if(kind===at)mutate();});
    if(at==="finalPeek")f.setPeekEdge(reply=>{if(++peeks===2)mutate(reply);});
    if(at==="listener")f.setBeforeFinal(()=>mutate());
    f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000);
    assert.equal(f.sent.length,0,at+" "+edge);assert.equal(f.scope.worldFishingCastClockRef.current.lastCastMs,0,at+" "+edge);
  }
});
check("Fishing Page equivalent full snapshot preserves current gesture while Harvest and old commit cleanup retire only their owner",()=>{
  const f=worldFishingPageFixture(),gesture=f.begin(),lease=f.scope.worldFishingActiveGestureRef.current;
  f.api.captureWorldFishingSnapshot({...f.raw,entities:[{...f.self}]},f.scope.equipmentConnectionGenerationRef.current);
  assert.strictEqual(f.scope.worldFishingActiveGestureRef.current,lease);assert.strictEqual(f.scope.worldFishingSourceRef.current.current(f.api.worldFishingOwner()),f.source);
  f.api.commitWorldFishingAnimation(null,{});assert.strictEqual(f.scope.worldFishingCommitRef.current,f.commit,"foreign old cleanup cannot clear committed successor");
  f.api.captureWorldFishingGatewayEvent({type:"packet",packet:"Harvest",payload:{objectId:17}});assert.equal(f.scope.worldFishingActiveGestureRef.current,null);
  f.api.captureWorldFishingSnapshot({...f.raw,entities:[{...f.self}]},f.scope.equipmentConnectionGenerationRef.current);
  assert.equal(f.api.beginWorldFishingGesture(f.pointer),null,"ordinary snapshot cannot revive harvested unknown action interval");
  assert.equal(f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000),false);assert.equal(f.sent.length,0);
});


const fishingShellSource=readFileSync(new URL("../app/original-client-shell.tsx",import.meta.url),"utf8"),fishingShellAst=ts.createSourceFile("original-client-shell.tsx",fishingShellSource,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const fishingShellNames=new Set(["retireWorldFishingPhysical","cancelWorldFishingHeldPointer","worldFishingStageGeometry","worldFishingPointCurrent",
  "updateWorldFishingPointerPosition","worldFishingPhysicalCurrent","attachWorldFishingPhysical","handleSceneWorldPointerDown","finishWorldFishingPhysical",
  "scenePointFromMouseEvent","committedViewportPlayerPosition","tileFromScenePoint","dispatchSceneClickInput","dispatchSceneMoveInput","npcShopBlocksWorldInput","stopNpcShopWorldInput","heroBlocksWorldInput"]),fishingShellDeclarations=new Map(),fishingShellRefInitializers=new Map();
let fishingListenerEffect=null,fishingContextEffect=null,fishingLayoutEffect=null,fishingIntervalCallback=null;
(function visit(node){if(ts.isFunctionDeclaration(node)&&fishingShellNames.has(node.name?.text))fishingShellDeclarations.set(node.name.text,node.getText(fishingShellAst));
  if(ts.isVariableDeclaration(node)&&["worldFishingPhysicalRef","worldFishingTerminalRef","worldFishingPointersRef","worldFishingQuarantineRef","worldFishingShellRef","heldScenePointerRef"].includes(node.name.getText(fishingShellAst))){assert.ok(ts.isCallExpression(node.initializer)&&node.initializer.expression.getText(fishingShellAst)==="useRef");fishingShellRefInitializers.set(node.name.getText(fishingShellAst),node.initializer.arguments[0].getText(fishingShellAst));}
  if(ts.isCallExpression(node)&&node.expression.getText(fishingShellAst)==="useEffect"&&ts.isArrowFunction(node.arguments[0])&&node.arguments[0].body.getText(fishingShellAst).includes("const pointers = worldFishingPointersRef.current")){assert.equal(fishingListenerEffect,null);fishingListenerEffect=node.arguments[0].getText(fishingShellAst);}
  if(ts.isCallExpression(node)&&node.expression.getText(fishingShellAst)==="useLayoutEffect"&&ts.isArrowFunction(node.arguments[0])){const text=node.arguments[0].body.getText(fishingShellAst);
    if(text.includes("worldFishingShellRef.current = context")){assert.equal(fishingContextEffect,null);fishingContextEffect=node.arguments[0].getText(fishingShellAst);}
    if(text.includes("committedEntityAnimationRef.current = { worldKey: animationWorldKey, poses }")){assert.equal(fishingLayoutEffect,null);fishingLayoutEffect=node.arguments[0].getText(fishingShellAst);}}
  if(ts.isCallExpression(node)&&node.expression.getText(fishingShellAst)==="window.setInterval"&&ts.isArrowFunction(node.arguments[0])&&node.arguments[0].body.getText(fishingShellAst).includes("dispatchSceneMoveInput(held)")){assert.equal(fishingIntervalCallback,null);fishingIntervalCallback=node.arguments[0].getText(fishingShellAst);}
  ts.forEachChild(node,visit);})(fishingShellAst);
assert.equal(fishingShellDeclarations.size,fishingShellNames.size);assert.equal(fishingShellRefInitializers.size,6);assert.ok(fishingListenerEffect&&fishingContextEffect&&fishingLayoutEffect&&fishingIntervalCallback);
const fishingShellInitializersJs=ts.transpileModule([...fishingShellRefInitializers].map(([name,initializer])=>"const "+name+"={current:"+initializer+"};").join("\n")+"\nreturn {"+[...fishingShellRefInitializers.keys()].join(",")+"};",
  {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
const fishingShellJs=ts.transpileModule([...fishingShellDeclarations.values()].join("\n")+"\nfunction installWorldFishingListeners(){return ("+fishingListenerEffect+")();}"+
  "\nfunction commitWorldFishingShellContext(){return ("+fishingContextEffect+")();}\nfunction commitWorldFishingLayout(){return ("+fishingLayoutEffect+")();}\nconst worldFishingHeldTick="+fishingIntervalCallback+";",
  {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
const fishingCanvasMode=loadTypeScriptModule(new URL("../lib/bevy-shared-canvas-mode.ts",import.meta.url));
const fishingSceneLayout=loadTypeScriptModule(new URL("../app/components/original-client-scene-layout.ts",import.meta.url),{"../../lib/original-ui":loadTypeScriptModule(new URL("../lib/original-ui.ts",import.meta.url))});
function worldFishingShellFixture(){
  const page=worldFishingPageFixture(),handlers=new Map(),documentHandlers=new Map(),sinks=[],cancels=[],resolves=[];let now=1000,api,sinkEdge=null;
  class MemoryStageNode{constructor(id,rect){this.id=id;this.rect=rect;this.isConnected=true;this.dataset={};this.style={display:"block",visibility:"visible",opacity:"1"};this.children=[];}
    getBoundingClientRect(){const[left,top,width,height]=this.rect;return{left,top,width,height,right:left+width,bottom:top+height};}getClientRects(){return[this.getBoundingClientRect()];}
    contains(node){return node===this||this.children.includes(node);}closest(selector){if(selector==="[hidden]")return this.hidden?this:null;return this.interactive&&selector.includes("data-ui-interactive")?this:null;}}
  const stage=new MemoryStageNode("world-stage",[10,20,800,600]),canvas=new MemoryStageNode("world-bevy-canvas",[10,20,800,600]);stage.children=[canvas];stage.dataset={viewportPlayerX:"0",viewportPlayerY:"1"};
  const state={hit:canvas},document={visibilityState:"visible",hasFocus:()=>true,elementFromPoint:()=>state.hit,
    addEventListener:(name,fn)=>documentHandlers.set(name,fn),removeEventListener:(name,fn)=>{if(documentHandlers.get(name)===fn)documentHandlers.delete(name);}},
    window={devicePixelRatio:2,visualViewport:{scale:1},getComputedStyle:node=>node.style,addEventListener:(name,fn)=>handlers.set(name,fn),removeEventListener:(name,fn)=>{if(handlers.get(name)===fn)handlers.delete(name);}},ref=current=>({current});
  const actualRefs=new Function(fishingShellInitializersJs)();assert.equal(actualRefs.worldFishingPhysicalRef.current,null);assert.equal(actualRefs.worldFishingTerminalRef.current,null);assert.equal(actualRefs.worldFishingPointersRef.current.size,0);
  page.runtime.resolveMir2EntityAnimationPoses=json=>{const input=JSON.parse(json);resolves.push(input);return JSON.stringify({worldKey:input.worldKey,nowMs:input.nowMs,poses:[]});};
  const scope={...actualRefs,HTMLElement:MemoryStageNode,document,window,Date:{now:()=>now},stageFrameRef:ref(stage),stagePresentation:{virtualWidth:800,virtualHeight:600,scale:1},
    bagPointerCallbacksRef:ref({}),npcShopPointerCallbacksRef:ref({}),parityUiBlocksGameplay:undefined,heroPointerLeaseRef:ref(null),heroPointerCallbacksRef:ref({}),
    heldKeyboardMoveKeysRef:ref(new Set()),heldKeyboardRunModeRef:ref(false),onViewportDirectionStop:()=>{},updateSceneCombatPointer:()=>{},
    latestMoveInputRef:ref({screen:"game",player:{x:0,y:1}}),viewportLayout:{offsetX:0,offsetY:0},...fishingSceneLayout,
    screen:"game",sceneInteractionReady:true,bevyQuestUiCapturesPointer:false,questLocalModalOpen:false,mobileMoreOpen:false,bevyMailComposeReady:false,bevyMailComposePending:false,
    worldFishingAnimation:{runtime:page.runtime,record:page.source},onBeginWorldFishingGesture:pointer=>page.api.beginWorldFishingGesture(pointer),
    onCancelWorldFishingGesture:gesture=>{cancels.push(gesture);page.api.cancelWorldFishingGesture(gesture);},webGl2SharedCanvasPrototype:false,sharedUiCanvasId:fishingCanvasMode.sharedUiCanvasId,
    player:page.self,animationWorldKey:page.source.sourceKey,entityAnimationWorldSeed:7,committedEntityAnimationRef:ref(null),
    entityAnimationInputs:{entities:[{entity:{...page.self,objectId:"17"},legacyAnimationState:"standing"}]},resolveCrystalEntityAnimationPoses:fishingAnimation.resolveCrystalEntityAnimationPoses,
    entityAnimationRuntimeFromWindow:fishingAnimation.entityAnimationRuntimeFromWindow,onWorldFishingAnimationCommit:(commit,previous)=>page.api.commitWorldFishingAnimation(commit,previous),
    onViewportTileSecondaryAction:()=>{throw Error("primary fixture cannot send secondary action");},playerUiPreferences:undefined,
    onViewportTileClick:(x,y,gesture)=>{sinks.push({kind:"click",x,y,gesture,burned:scope.worldFishingPhysicalRef.current===null&&scope.worldFishingTerminalRef.current?.phase==="terminal"});sinkEdge?.();if(gesture)page.api.tryWorldFishingBlockedClick(page.queued,gesture,page.self,now);},
    onViewportDirectionStep:(x,y,mode,gesture)=>{sinks.push({kind:"held",x,y,mode,gesture});if(gesture)page.api.tryWorldFishingBlockedClick(page.queued,gesture,page.self,now);}};
  const keys=Object.keys(scope);api=new Function(...keys,fishingShellJs+"\nreturn {attachWorldFishingPhysical,handleSceneWorldPointerDown,worldFishingPhysicalCurrent,worldFishingStageGeometry,finishWorldFishingPhysical,retireWorldFishingPhysical,installWorldFishingListeners,commitWorldFishingShellContext,commitWorldFishingLayout,worldFishingHeldTick,setContextInputs:value=>{if(Object.hasOwn(value,'onBeginWorldFishingGesture'))onBeginWorldFishingGesture=value.onBeginWorldFishingGesture;if(Object.hasOwn(value,'worldFishingAnimation'))worldFishingAnimation=value.worldFishingAnimation;if(Object.hasOwn(value,'mobileMoreOpen'))mobileMoreOpen=value.mobileMoreOpen;}};")(...keys.map(key=>scope[key]));
  api.commitWorldFishingShellContext();const cleanup=api.installWorldFishingListeners();
  const event=(id,time,type="mouse",delta={})=>({pointerId:id,pointerType:type,button:0,isPrimary:true,timeStamp:time,clientX:58,clientY:21,target:canvas,preventDefault(){this.prevented=true;},...delta});
  const down=(id=1,time=10,type="mouse")=>{const nativeEvent=event(id,time,type);handlers.get("pointerdown")(nativeEvent);const before=scope.heldScenePointerRef.current;
    if(type==="mouse")scope.heldScenePointerRef.current={button:0,sceneX:48,sceneY:1,tileX:1,tileY:1,startedAt:now,dispatched:false};
    api.handleSceneWorldPointerDown({...nativeEvent,nativeEvent,currentTarget:stage},before);return nativeEvent;};
  return {...page,shellScope:scope,shellApi:api,stage,canvas,state,handlers,documentHandlers,sinks,cancels,resolves,cleanup,event,down,
    setNow:value=>{now=value;page.setNow(value);},setSinkEdge:value=>sinkEdge=value};
}
check("Fishing Shell actual short primary pointer terminal burns custody before real Page sink and one cast",()=>{
  for(const pointerType of ["mouse","touch"]){const f=worldFishingShellFixture();f.down(1,10,pointerType);const lease=f.shellScope.worldFishingPhysicalRef.current;assert.ok(lease?.gesture);
    f.setSinkEdge(()=>f.handlers.get("pointerup")(f.event(1,12,pointerType)));f.handlers.get("pointerup")(f.event(1,11,pointerType));
    assert.equal(f.sinks.length,1);assert.equal(f.sinks[0].kind,"click");assert.equal(f.sinks[0].burned,true);assert.equal(f.sinks[0].gesture,lease.gesture);assert.deepEqual([f.sinks[0].x,f.sinks[0].y],[1,1]);
    assert.deepEqual(f.sent,[{type:"fishingCast",castOut:true}]);assert.equal(f.shellScope.worldFishingPhysicalRef.current,null);assert.equal(f.shellScope.worldFishingTerminalRef.current,null);
    f.handlers.get("pointerup")(f.event(1,13,pointerType));assert.equal(f.sinks.length,1);assert.equal(f.sent.length,1);f.cleanup();assert.equal(f.handlers.size,0);assert.equal(f.documentHandlers.size,0);
  }
  const f=worldFishingShellFixture();f.down();f.shellApi.worldFishingHeldTick();assert.equal(f.sinks.length,0,"actual 100ms hold callback does not promote short click");
  f.setNow(1100);f.shellApi.worldFishingHeldTick();assert.equal(f.sinks[0].kind,"held");assert.equal(f.sinks[0].mode,"walk");assert.ok(f.sinks[0].gesture);assert.equal(f.sent.length,1);
  f.handlers.get("pointerup")(f.event(1,11));assert.equal(f.sinks.length,1,"held dispatch cannot duplicate short terminal click");f.cleanup();
});
check("Fishing Shell actual geometry terminal quarantine and lifecycle retirement keep old pointer proof spent",()=>{
  for(const field of ["dpr","scale"])for(const value of [0,-1,NaN,Infinity,"2"]){const f=worldFishingShellFixture();
    if(field==="dpr")f.shellScope.window.devicePixelRatio=value;else f.shellScope.window.visualViewport.scale=value;
    assert.equal(f.shellApi.worldFishingStageGeometry(f.stage),null,"actual "+field+" must be a positive finite raw number");
    f.down();assert.equal(f.shellScope.worldFishingPhysicalRef.current,null);f.handlers.get("pointerup")(f.event(1,11));assert.equal(f.sent.length,0);f.cleanup();
  }
  for(const edge of ["DPR","rect","hidden","obscured","ui","bag","callback","runtime","modal","source","wrongType","staleUp"]){
    const f=worldFishingShellFixture();f.down();const lease=f.shellScope.worldFishingPhysicalRef.current;assert.ok(lease);
    if(edge==="DPR")f.shellScope.window.devicePixelRatio=3;if(edge==="rect")f.stage.rect[0]++;
    if(edge==="hidden")f.stage.style.visibility="hidden";if(edge==="obscured")f.state.hit={};if(edge==="ui")f.canvas.interactive=true;
    if(edge==="bag")f.shellScope.bagPointerCallbacksRef.current={getBevyBagPointerContext:()=>({inputRegions:[{left:0,top:0,width:800,height:600}]})};
    if(edge==="callback")f.shellScope.onBeginWorldFishingGesture=()=>null;if(edge==="runtime")f.shellScope.worldFishingAnimation={...f.shellScope.worldFishingAnimation,runtime:{}};
    if(edge==="modal")f.shellScope.mobileMoreOpen=true;if(edge==="source")f.shellScope.worldFishingAnimation={...f.shellScope.worldFishingAnimation,record:{...f.source,sourceKey:"successor"}};
    if(["callback","runtime","modal","source"].includes(edge)){f.shellApi.setContextInputs({onBeginWorldFishingGesture:f.shellScope.onBeginWorldFishingGesture,worldFishingAnimation:f.shellScope.worldFishingAnimation,mobileMoreOpen:f.shellScope.mobileMoreOpen});f.shellApi.commitWorldFishingShellContext();}
    f.handlers.get("pointerup")(f.event(1,edge==="staleUp"?9:11,edge==="wrongType"?"touch":"mouse"));assert.equal(f.sent.length,0,edge);
    if(edge==="wrongType"||edge==="staleUp"){assert.strictEqual(f.shellScope.worldFishingPhysicalRef.current,lease);f.handlers.get("pointerup")(f.event(1,12));assert.equal(f.sent.length,1,"only matching newer actual terminal can spend physical source");}
    else{assert.equal(f.shellScope.worldFishingPhysicalRef.current,null);assert.equal(lease.phase,"retired");assert.equal(f.shellApi.worldFishingPhysicalCurrent(lease),false);}
    f.cleanup();
  }
  for(const edge of ["blur","resize","pagehide","visibilitychange","pointercancel"]){const f=worldFishingShellFixture();f.down();const lease=f.shellScope.worldFishingPhysicalRef.current;
    if(edge==="visibilitychange"){f.shellScope.document.visibilityState="hidden";f.documentHandlers.get(edge)();f.shellScope.document.visibilityState="visible";}
    else if(edge==="pointercancel")f.handlers.get(edge)(f.event(1,11));else f.handlers.get(edge)();
    assert.equal(lease.phase,"retired");f.handlers.get("pointerup")(f.event(1,12));assert.equal(f.sent.length,0);f.cleanup();
  }
  const f=worldFishingShellFixture();f.down(1,10);const old=f.shellScope.worldFishingPhysicalRef.current;f.handlers.get("pointerdown")(f.event(2,11));
  assert.equal(old.phase,"retired");assert.equal(f.shellScope.worldFishingQuarantineRef.current.size,2);f.handlers.get("pointerup")(f.event(1,12));
  assert.equal(f.shellScope.worldFishingQuarantineRef.current.size,1);f.handlers.get("pointerup")(f.event(2,13));assert.equal(f.shellScope.worldFishingQuarantineRef.current.size,0);assert.equal(f.sent.length,0);
  f.down(3,20);const successor=f.shellScope.worldFishingPhysicalRef.current;assert.ok(successor);f.shellApi.retireWorldFishingPhysical(old);assert.strictEqual(f.shellScope.worldFishingPhysicalRef.current,successor);
  f.handlers.get("pointerup")(f.event(3,21));assert.equal(f.sent.length,1);f.cleanup();
});
check("Fishing Shell actual shared animation advances only in committed layout with captured clock and successor cleanup",()=>{
  const f=worldFishingShellFixture();assert.equal(f.resolves.length,0);const firstCleanup=f.shellApi.commitWorldFishingLayout(),first=f.scope.worldFishingCommitRef.current;
  assert.equal(f.resolves.length,1);assert.equal(f.resolves[0].nowMs,1000);assert.equal(first.atMs,1000);assert.strictEqual(first.context,f.shellScope.worldFishingAnimation);assert.ok(Object.isFrozen(first)&&Object.isFrozen(first.token));
  f.setNow(1001);const nextCleanup=f.shellApi.commitWorldFishingLayout(),next=f.scope.worldFishingCommitRef.current;assert.notStrictEqual(next,first);assert.equal(next.atMs,1001);
  firstCleanup();assert.strictEqual(f.scope.worldFishingCommitRef.current,next,"old layout cleanup cannot clear committed successor");nextCleanup();assert.equal(f.scope.worldFishingCommitRef.current,null);f.cleanup();
});


check("Fishing Page final physical callback sees new equipment NPC and composer pending before irreversible entry",()=>{
  for(const edge of ["equipment","npc","composer"]){const f=worldFishingPageFixture(),gesture=f.begin();let pending=false,reads=0;
    if(edge==="equipment")f.scope.equipmentControllerRef.current={status:()=>({ready:true,pending:pending?1:0})};
    if(edge==="npc")f.scope.npcBuyDispatcherRef.current={status:()=>({flight:pending?{}:null})};
    if(edge==="composer")f.scope.mailDispatcherRef.current={composer:{pending:()=>pending}};
    f.setBeforeFinal(()=>f.setPointerEdge(()=>{if(++reads===2)pending=true;}));
    f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000);assert.equal(pending,true,edge);assert.ok(reads>=2,edge);
    assert.equal(f.sent.length,0,edge);assert.equal(f.scope.worldFishingCastClockRef.current.lastCastMs,0,edge);
  }
});
check("Fishing Page final actual ingress getter reentry cannot replace owner source or create pending after a captured gate",()=>{
  for(const ingress of ["npcUi","combat","spells","storageActive","storageTransitioning"])for(const edge of ["owner","source","equipment","npc","composer"]){
    const f=worldFishingPageFixture(),gesture=f.begin();let reads=0,pending=false;
    f.scope.equipmentControllerRef.current={status:()=>({ready:true,pending:edge==="equipment"&&pending?1:0})};
    f.scope.npcBuyDispatcherRef.current={status:()=>({flight:edge==="npc"&&pending?{}:null})};
    f.scope.mailDispatcherRef.current={composer:{pending:()=>edge==="composer"&&pending}};
    const mutate=()=>{if(edge==="owner")f.scope.socketRef.current={readyState:1};if(edge==="source")f.scope.worldFishingSourceRef.current.observePacket(f.api.worldFishingOwner(),"Harvest",{objectId:17},1000);
      if(["equipment","npc","composer"].includes(edge)){pending=true;f.api.retireWorldFishingGesture();}};
    f.setBeforeFinal(()=>{const invoke=()=>{if(++reads===2)mutate();};
      if(ingress==="npcUi")f.scope.npcShopUiIngressRef.current={blocksInput:()=>{invoke();return false;}};
      if(ingress==="combat")f.scope.combatIngressRef.current={hasUiHeld:()=>{invoke();return false;}};
      if(ingress==="spells")f.scope.spellsIngressRef.current={pointerContext:()=>{invoke();return {modal:false};}};
      if(ingress==="storageActive")f.scope.storageUiIngressRef.current={get active(){invoke();return false;},transitioning:false};
      if(ingress==="storageTransitioning")f.scope.storageUiIngressRef.current={active:false,get transitioning(){invoke();return false;}};
    });
    f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000);assert.ok(reads>=2,ingress+" "+edge);assert.equal(f.sent.length,0,ingress+" "+edge);assert.equal(f.scope.worldFishingCastClockRef.current.lastCastMs,0);
  }
});
const fishingReservePrefixes=new Map();
(function visit(node){if(ts.isBlock(node)){const statements=node.statements;for(let index=1;index<statements.length;index++){
  const declaration=statements[index];if(!ts.isVariableStatement(declaration))continue;const initializers=declaration.declarationList.declarations.map(item=>item.initializer).filter(Boolean);
  for(const call of initializers){if(!ts.isCallExpression(call))continue;const expression=call.expression.getText(fishingPageAst),kind=expression==="controller.reserve"?"equipment":expression==="dispatcher.prepare"&&call.arguments.length===2&&call.arguments[0].getText(fishingPageAst)==="captured"?"sharedNpc":expression==="dispatcher?.prepare"&&call.arguments.length===2&&call.arguments[0].getText(fishingPageAst)==="captured"?"compatNpc":expression==="sender.reserve"?"composer":null;
    if(!kind)continue;assert.equal(statements[index-1].getText(fishingPageAst),"retireWorldFishingGesture();","actual "+kind+" reserve immediately retires old world authority");
    assert.equal(fishingReservePrefixes.has(kind),false,"sole actual "+kind+" reserve prefix");fishingReservePrefixes.set(kind,statements[index-1].getText(fishingPageAst)+"\n"+declaration.getText(fishingPageAst));}
  }}ts.forEachChild(node,visit);})(fishingPageAst);
assert.equal(fishingReservePrefixes.size,4,"equipment both NPC reserve paths and composer reserve remain covered");
check("Fishing Page actual equipment both NPC and composer reserve prefixes retire gesture before reentrant Core call",()=>{
  for(const [kind,text] of fishingReservePrefixes){const f=worldFishingPageFixture(),gesture=f.begin(),lease=f.scope.worldFishingActiveGestureRef.current,calls=[];
    const reserve=(...args)=>{assert.equal(f.scope.worldFishingActiveGestureRef.current,null,kind);assert.equal(lease.valid,false,kind);assert.equal(f.scope.worldFishingGestureRegistryRef.current.has(gesture),false,kind);calls.push(args);return null;};
    const scope={retireWorldFishingGesture:f.api.retireWorldFishingGesture,controller:{reserve},session:{connectionGeneration:4,sessionGeneration:8},
      options:{ownerToken:{ownerRevision:3}},operation:{kind:"equip",uniqueId:0,grid:"bag"},dispatcher:{prepare:reserve},captured:{typed:true},intent:{quantity:1},quantity:1,
      sender:{reserve},owner:f.scope.equipmentStartGameRef.current,mailPresentation:{key:"composer:1"},decision:{payload:{name:"Owned",message:"",gold:0,itemsIdx:[0,0,0,0,0],stamped:false}}};
    const keys=Object.keys(scope),js=ts.transpileModule(text,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
    new Function(...keys,js)(...keys.map(key=>scope[key]));assert.equal(calls.length,1,kind);assert.equal(f.sent.length,0,kind);
  }
});


check("Fishing Page required facing pose unknown permanently spends gesture despite exact getter and commit recovery",()=>{
  for(const edge of ["missingCommit","missingGetter","unknownReply","nullReply"]){const f=worldFishingPageFixture(),gesture=f.begin(),lease=f.scope.worldFishingActiveGestureRef.current;
    const getter=f.runtime.getMir2EntityActionPose,commit=f.scope.worldFishingCommitRef.current;
    if(edge==="missingCommit")f.scope.worldFishingCommitRef.current=null;if(edge==="missingGetter")delete f.runtime.getMir2EntityActionPose;
    if(edge==="unknownReply")f.setPeekEdge(reply=>{reply.known=false;});if(edge==="nullReply")f.runtime.getMir2EntityActionPose=()=>"null";
    f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000);assert.equal(f.sent.length,0,edge);assert.equal(lease.valid,false,edge);
    assert.equal(f.scope.worldFishingActiveGestureRef.current,null,edge);assert.equal(f.scope.worldFishingGestureRegistryRef.current.has(gesture),false,edge);
    f.scope.worldFishingCommitRef.current=commit;f.runtime.getMir2EntityActionPose=getter;f.setPeekEdge(null);
    assert.strictEqual(f.scope.worldFishingSourceRef.current.current(f.api.worldFishingOwner()),f.source);
    assert.equal(f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000),false,"same physical gesture cannot revive after "+edge);assert.equal(f.sent.length,0);
    const fresh=f.begin();assert.notStrictEqual(fresh,gesture);f.api.tryWorldFishingBlockedClick(f.queued,fresh,f.self,1000);assert.deepEqual(f.sent,[{type:"fishingCast",castOut:true}],edge);
  }
});
check("Fishing Page optional targets and decision ABI loss permanently spends gesture after exact facade recovery",()=>{
  for(const edge of ["missingAbi","missingTargets","nullTargets","missingDecision","nullDecision"]){const f=worldFishingPageFixture(),gesture=f.begin(),lease=f.scope.worldFishingActiveGestureRef.current;
    const original={...f.fishingModule};if(edge==="missingAbi")delete f.fishingModule.fishing_click_abi_version;
    if(edge==="missingTargets")delete f.fishingModule.fishing_click_targets;if(edge==="nullTargets")f.fishingModule.fishing_click_targets=()=>"null";
    if(edge==="missingDecision")delete f.fishingModule.fishing_click_decision;if(edge==="nullDecision")f.fishingModule.fishing_click_decision=()=>JSON.stringify({version:1,ok:false});
    f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000);assert.equal(f.sent.length,0,edge);assert.equal(lease.valid,false,edge);
    Object.assign(f.fishingModule,original);assert.strictEqual(f.scope.worldFishingSourceRef.current.current(f.api.worldFishingOwner()),f.source);
    assert.equal(f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000),false,edge);assert.equal(f.sent.length,0);assert.equal(f.scope.worldFishingCastClockRef.current.lastCastMs,0);
    const fresh=f.begin();f.api.tryWorldFishingBlockedClick(f.queued,fresh,f.self,1000);assert.deepEqual(f.sent,[{type:"fishingCast",castOut:true}],edge);
  }
});
check("Fishing Page final cast peek loss permanently spends held gesture before socket entry and survives getter restoration",()=>{
  for(const edge of ["firstFinal","socketPreflight","lastEntry"]){const f=worldFishingPageFixture(),gesture=f.begin(),lease=f.scope.worldFishingActiveGestureRef.current,getter=f.runtime.getMir2EntityActionPose;let reads=0;
    if(edge==="lastEntry")f.setBeforeFinal(()=>{f.runtime.getMir2EntityActionPose=()=>"null";});
    else f.runtime.getMir2EntityActionPose=function(json){reads++;return reads===(edge==="firstFinal"?2:3)?"null":getter.call(this,json);};
    f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000);assert.equal(f.sent.length,0,edge);assert.equal(lease.valid,false,edge);assert.equal(f.scope.worldFishingCastClockRef.current.lastCastMs,0);
    f.runtime.getMir2EntityActionPose=getter;f.setBeforeFinal(null);assert.strictEqual(f.scope.worldFishingSourceRef.current.current(f.api.worldFishingOwner()),f.source);
    assert.equal(f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000),false,edge);assert.equal(f.sent.length,0);
    const fresh=f.begin();f.api.tryWorldFishingBlockedClick(f.queued,fresh,f.self,1000);assert.deepEqual(f.sent,[{type:"fishingCast",castOut:true}],edge);
  }
});
check("Fishing Page known nonstanding and valid cooldown none preserve held retry without new physical gesture",()=>{
  for(const edge of ["nonstanding","cooldown"]){const f=worldFishingPageFixture(),gesture=f.begin(),lease=f.scope.worldFishingActiveGestureRef.current;
    if(edge==="nonstanding")f.setPeekEdge(reply=>{reply.action="walking";});
    if(edge==="cooldown"){f.scope.worldFishingCastClockRef.current.lastCastMs=500;f.fishingModule.fishing_click_decision=json=>{const input=JSON.parse(json);return JSON.stringify({version:1,ok:true,decision:input.nowMs-input.lastCastMs<1000?{type:"none"}:{type:"cast",lastCastMs:input.nowMs}});};}
    assert.equal(f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000),true);assert.equal(f.sent.length,0);assert.equal(lease.valid,true);assert.strictEqual(f.scope.worldFishingActiveGestureRef.current,lease);
    assert.strictEqual(f.scope.worldFishingGestureRegistryRef.current.get(gesture),lease);
    f.setPeekEdge(null);if(edge==="cooldown"){f.setNow(1500);const commit=Object.freeze({...f.commit,atMs:1500,token:Object.freeze({})});f.api.commitWorldFishingAnimation(commit);}
    f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,edge==="cooldown"?1500:1000);assert.deepEqual(f.sent,[{type:"fishingCast",castOut:true}],edge);
    assert.equal(f.scope.worldFishingCastClockRef.current.lastCastMs,edge==="cooldown"?1500:1000);
  }
});
check("Fishing Page wrong facing uses actual turn branch and socket proof with known water and unavailable owned pose",()=>{
  const f=worldFishingPageFixture();f.self.direction="Up";f.api.captureWorldFishingSnapshot({...f.raw,entities:[f.self]},f.api.worldFishingOwner().connectionGeneration);
  const source=f.scope.worldFishingSourceRef.current.current(f.api.worldFishingOwner());assert.ok(source?.animationKnown);assert.equal(source.self.direction,"Up");
  f.scope.worldFishingCommitRef.current=null;delete f.runtime.getMir2EntityActionPose;
  const decisions=[];f.fishingModule.fishing_click_decision=json=>{const input=JSON.parse(json);decisions.push(input);return JSON.stringify({version:1,ok:true,decision:{type:"turn",direction:input.direction,delayMs:200}});};
  const gesture=f.begin(),lease=f.scope.worldFishingActiveGestureRef.current;f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000);
  assert.deepEqual(f.sent,[{type:"turn",direction:"Right"}]);assert.equal(decisions.length,1);assert.equal(decisions[0].facingMatches,false);assert.equal(decisions[0].standing,null);assert.deepEqual(decisions[0].water,{cell:{x:4,y:1},light:105});
  assert.equal(f.peekRequests.length,0);assert.equal(lease.valid,true);assert.equal(f.scope.worldFishingCastClockRef.current.lastCastMs,0);assert.equal(f.scope.nextMoveSendAtRef.current,1200);
});
check("Fishing Shell unknown physical geometry cannot revive old pointer after repaint but fresh down owns a new cast",()=>{
  const f=worldFishingShellFixture();f.down(1,10);const lease=f.shellScope.worldFishingPhysicalRef.current;assert.ok(lease);
  f.stage.style.visibility="hidden";assert.equal(f.shellApi.worldFishingPhysicalCurrent(lease),false);assert.equal(lease.phase,"retired");assert.equal(f.shellScope.worldFishingPhysicalRef.current,null);
  f.stage.style.visibility="visible";assert.equal(f.shellApi.worldFishingPhysicalCurrent(lease),false);f.handlers.get("pointerup")(f.event(1,12));assert.equal(f.sent.length,0);
  f.down(2,20);assert.notStrictEqual(f.shellScope.worldFishingPhysicalRef.current,lease);f.handlers.get("pointerup")(f.event(2,21));assert.deepEqual(f.sent,[{type:"fishingCast",castOut:true}]);f.cleanup();
});


check("Fishing Page unknown water spends either facing gesture while known nonwater retains valid none",()=>{
  for(const facing of [true,false]){const f=worldFishingPageFixture();
    if(!facing){f.self.direction="Up";f.api.captureWorldFishingSnapshot({...f.raw,entities:[f.self]},f.api.worldFishingOwner().connectionGeneration);}
    const source=f.scope.worldFishingSourceRef.current.current(f.api.worldFishingOwner());
    f.fishingModule.fishing_click_decision=json=>{const input=JSON.parse(json);return JSON.stringify({version:1,ok:true,decision:input.water?.light===105?(input.facingMatches?{type:"cast",lastCastMs:input.nowMs}:{type:"turn",direction:input.direction,delayMs:200}):{type:"none"}});};
    const gesture=f.begin(),lease=f.scope.worldFishingActiveGestureRef.current;f.region.cells[3].light="105";
    f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000);assert.equal(lease.valid,false);assert.equal(f.sent.length,0);
    f.region.cells[3].light=105;assert.strictEqual(f.scope.worldFishingSourceRef.current.current(f.api.worldFishingOwner()),source);
    assert.equal(f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000),false);assert.equal(f.sent.length,0);
    const fresh=f.begin();if(facing)f.api.commitWorldFishingAnimation(f.commit);f.api.tryWorldFishingBlockedClick(f.queued,fresh,f.self,1000);
    assert.deepEqual(f.sent,[facing?{type:"fishingCast",castOut:true}:{type:"turn",direction:"Right"}]);
  }
  const f=worldFishingPageFixture(),gesture=f.begin(),lease=f.scope.worldFishingActiveGestureRef.current;f.region.cells[3].light=0;
  assert.equal(f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000),true);assert.equal(lease.valid,true);assert.equal(f.sent.length,0);
  f.region.cells[3].light=105;f.api.tryWorldFishingBlockedClick(f.queued,gesture,f.self,1000);assert.deepEqual(f.sent,[{type:"fishingCast",castOut:true}]);
});


// Source24 Pearl pure source / DTO / PUI host fixtures. Fixed PUI responses
// inspect the real host adapter; they do not execute WASM or prove Rust pricing.
// Full canonical ItemInfo32/UserItem22; nested added stats verify raw custody.
// Oracle only admits structurally valid test sources; Rust price tests run separately.
const pearlStructureOracle=()=>true;
function pearlRawGood(uid = 0) {
  const index=-7, info={item_index:index,name:"Same visible name",item_type:1,grade:1,required_type:0,required_class:31,required_gender:3,item_set:0,shape:0,weight:4,light:0,required_amount:5,image:0,durability:4000,stack_size:99,price:900,start_item:false,effect:0,need_identify:false,show_group_pickup:false,class_based:false,level_based:false,can_mine:false,global_drop_notify:false,bind:0,unique:0,random_stats_id:0,can_fast_run:false,can_awakening:false,slots:0,stats:[{stat:4,value:2},{stat:5,value:4}],tooltip:null},
    userItem={unique_id:uid,item_index:index,current_dura:0,max_dura:0,count:1,soul_bound_id:-1,identified:true,cursed:false,slots:[null],gem_count:0,added_stats:[{stat:5,value:3}],awake_type:0,awake_values:[],refined_value:0,refine_added:0,refine_success_chance:0,wedding_ring:-1,expire_info:null,rental_information:null,is_shop_item:true,sealed_info:null,gm_made:false};
  return {...JSON.parse(JSON.stringify(userItem)),id:uid,uniqueId:uid,itemIndex:index,
    name:"Same visible name",price:17,icon:2,grade:1,description:"Raw Pearl offer",
    tooltipSource:{info,realInfo:null,userItem,socketInfos:[null],realSocketInfos:[]}};
}
function pearlRawCatalog() { return {panelType:0,rate:1.25,list:[pearlRawGood(0),pearlRawGood(42)]}; }
function pearlPhysicalOwner(delta = {}) {
  return {socket:{},connectionGeneration:4,sessionGeneration:8,playerObjectId:17,sceneRevision:3,mapFileName:"0",...delta};
}
function pearlPureFixture() {
  const source = new pearlSourceModule.NpcPearlShopSource(), physical = pearlPhysicalOwner(), payload = pearlRawCatalog();
  const catalog = source.observeCatalog(physical,"NPCPearlGoods",payload,pearlStructureOracle);
  assert.ok(catalog);
  const wallet = source.observeWallet(physical,200);
  assert.ok(wallet);
  const current = {currency:"pearls",owner:{connectionGeneration:4,sessionGeneration:8,ownerRevision:3,playerObjectId:17},
    serviceRevision:5,catalogRevision:catalog.revision,presentation:{sceneRevision:3,mapFileName:"0",windowEpoch:9},
    catalog,wallet,selectedId:0,inventory:{capacity:86,gold:999999,items:[]},allowsBuy:true,blocked:false};
  return {source,physical,payload,catalog,wallet,current};
}
const pearlValidOutput = {version:1,ok:true,maxQuantity:99,quote:34,admittedCount:2,denial:0};
const pearlScalarInput = {allowsBuy:true,selected:true,usePearls:true,uniqueId:0,unitPrice:17,stock:-1,quantity:2,
  walletKnown:true,pearls:200,occupied:0,infoPrice:900,rate:1.25};

check("Pearl raw whole catalogue preserves UID0 and same-name different-UID entries",()=>{
  const f=pearlPureFixture();
  assert.deepEqual(f.catalog.goods.map(row=>row.uniqueId),[0,42]);
  assert.equal(f.catalog.goods[0].name,f.catalog.goods[1].name);
  assert.equal(f.catalog.goods[0].itemIndex,-7);
  assert.deepEqual(f.catalog.goods.map(row=>[row.unitPrice,row.stock]),[[17,-1],[17,-1]]);
  assert.equal(f.catalog.rate,1.25);
  assert.equal(f.catalog.panelType,0);
  assert.strictEqual(f.source.currentCatalog(f.physical),f.catalog);
});
check("Pearl missing raw aliases duplicate UID0 and bad partial rows retire the complete catalogue",()=>{
  const mutations=[...['id','uniqueId','unique_id','itemIndex','item_index'].map(key=>raw=>{delete raw.list[0][key];}),
    raw=>{raw.list[1]=pearlRawGood(0);},raw=>{raw.list[1].count=0;},raw=>{raw.list[1].tooltipSource.userItem.item_index=9;},
    raw=>{raw.list[0].id=42;},raw=>{raw.list[0].uniqueId=42;}];
  for(const mutate of mutations){const f=pearlPureFixture(),raw=pearlRawCatalog();mutate(raw);
    assert.equal(f.source.observeCatalog(f.physical,"NPCPearlGoods",raw,pearlStructureOracle),null);
    assert.equal(f.source.currentCatalog(f.physical),null,"a bad second row must not leave the valid first offer");
    assert.strictEqual(f.source.currentWallet(f.physical),f.wallet);
  }
});
check("Pearl zero display fallback without the raw template is not a free offer",()=>{
  for(const mutate of [raw=>{delete raw.list[0].tooltipSource;},raw=>{delete raw.list[0].tooltipSource.info;},
    raw=>{delete raw.list[0].tooltipSource.info.price;},raw=>{delete raw.list[0].tooltipSource.userItem;},
    raw=>{raw.list[0].tooltipSource.info.item_index=8;}]){
    const f=pearlPureFixture(),raw=pearlRawCatalog();raw.list[0].price=0;raw.list[0].icon=0;mutate(raw);
    assert.equal(f.source.observeCatalog(f.physical,"NPCPearlGoods",raw,pearlStructureOracle),null);
    assert.equal(f.source.currentCatalog(f.physical),null);
  }
  const f=pearlPureFixture(),raw=pearlRawCatalog();raw.list[0].price=0;raw.list[0].tooltipSource.info.price=0;
  assert.equal(f.source.observeCatalog(f.physical,"NPCPearlGoods",raw,pearlStructureOracle).goods[0].unitPrice,0,
    "a known complete zero-price row remains distinct from missing-template fallback");
});
check("Pearl invalid rates panels packet kinds and raw numeric source ranges fail closed",()=>{
  for(const rate of [-1,NaN,Infinity,1e40,"1",null]){const f=pearlPureFixture(),raw=pearlRawCatalog();raw.rate=rate;
    assert.equal(f.source.observeCatalog(f.physical,"NPCPearlGoods",raw,pearlStructureOracle),null);assert.equal(f.source.currentCatalog(f.physical),null);}
  for(const panel of [1,-1,"0",null,undefined]){const f=pearlPureFixture(),raw=pearlRawCatalog();if(panel===undefined)delete raw.panelType;else raw.panelType=panel;
    assert.equal(f.source.observeCatalog(f.physical,"NPCPearlGoods",raw,pearlStructureOracle),null);}
  for(const packet of ["NPCGoods","NPCSell","NewIntelligentCreature",""]){const f=pearlPureFixture();
    assert.equal(f.source.observeCatalog(f.physical,packet,f.payload,pearlStructureOracle),null);assert.equal(f.source.currentCatalog(f.physical),null);}
  for(const [key,value] of [["unique_id",-1],["price",4294967296],["price",0.5],["count",65536],["icon",-1],["item_index",-2147483649]]){
    const f=pearlPureFixture(),raw=pearlRawCatalog();raw.list[0][key]=value;
    assert.equal(f.source.observeCatalog(f.physical,"NPCPearlGoods",raw,pearlStructureOracle),null,key);
  }
});
check("Pearl strict source JSON rejects getter toJSON sparse symbols cycles and non-data fields without coercion",()=>{
  let calls=0;const getter={};Object.defineProperty(getter,"amount",{enumerable:true,get(){calls++;return 1;}});
  const toJSON={toJSON(){calls++;return {};}};const sparse=[,1],symbol={};symbol[Symbol("raw")]=1;
  const hidden={};Object.defineProperty(hidden,"amount",{value:1,enumerable:false});const cycle={};cycle.self=cycle;
  const extraArray=[1];extraArray.extra=2;
  for(const value of [getter,toJSON,sparse,symbol,hidden,cycle,extraArray,new Date(0),undefined,NaN,Infinity,1n,{missing:undefined}]){
    assert.throws(()=>pearlSourceModule.npcPearlJson(value));
  }
  assert.equal(calls,0,"strict source inspection must not invoke an accessor or toJSON");
  const shared={raw:1};assert.deepEqual(JSON.parse(pearlSourceModule.npcPearlJson({a:shared,b:shared})),{a:{raw:1},b:{raw:1}});
});
check("Pearl complete raw catalogue clone and nested read body survive original payload mutation",()=>{
  const f=pearlPureFixture(),rawPayload=f.catalog.rawPayload;
  f.payload.list[0].unique_id=999;f.payload.list[0].price=0;
  f.payload.list[0].tooltipSource.info.price=0;
  f.payload.list[0].tooltipSource.userItem.added_stats[0].value=999;
  f.payload.list[0].tooltipSource.userItem.slots.push({forged:true});f.payload.list.push(pearlRawGood(77));
  assert.strictEqual(f.catalog.rawPayload,rawPayload);assert.deepEqual(rawPayload.list.map(row=>row.unique_id),[0,42]);
  assert.equal(f.catalog.goods[0].uniqueId,0);assert.equal(f.catalog.goods[0].unitPrice,17);
  assert.equal(f.catalog.goods[0].tooltipSource.info.price,900);
  assert.equal(f.catalog.goods[0].tooltipSource.userItem.added_stats[0].value,3);
  assert.deepEqual(f.catalog.goods[0].tooltipSource.userItem.slots,[null]);
  assert.ok(Object.isFrozen(f.catalog)&&Object.isFrozen(f.catalog.goods)&&Object.isFrozen(f.catalog.goods[0]));
});
check("Pearl signed i32 wallet preserves negatives and rejects invalid observations without a free-wallet default",()=>{
  for(const amount of [-2147483648,-1,0,2147483647]){const f=pearlPureFixture();
    const wallet=f.source.observeWallet(f.physical,amount);assert.ok(wallet);assert.equal(wallet.amount,amount);assert.ok(wallet.revision>f.wallet.revision);}
  for(const amount of [undefined,null,"0",0.5,NaN,Infinity,-2147483649,2147483648,{amount:0}]){
    const f=pearlPureFixture();assert.equal(f.source.observeWallet(f.physical,amount),null);
    assert.equal(f.source.currentWallet(f.physical),null);
    const capture=pearlBuyModule.captureNpcPearlBuy({...f.current,wallet:null},1);assert.ok(capture);
    const input=pearlBuyModule.npcPearlBuyInput(capture.requestJson);assert.equal(input.walletKnown,false);assert.equal(input.pearls,0);
  }
});
check("Pearl personal wallet survives catalogue-only retirement and scene change while new personal owner is unknown",()=>{
  const f=pearlPureFixture(),nextScene={...f.physical,sceneRevision:4,mapFileName:"1"};
  assert.equal(f.source.currentCatalog(nextScene),null);assert.strictEqual(f.source.currentWallet(nextScene),f.wallet);
  f.source.retireCatalog();assert.equal(f.source.currentCatalog(f.physical),null);assert.strictEqual(f.source.currentWallet(f.physical),f.wallet);
  // The no-amount creature packet must not be treated as an observeWallet(0).
  const before=f.source.currentWallet(nextScene);assert.strictEqual(before,f.wallet);assert.equal(before.amount,200);
  for(const delta of [{socket:{}},{connectionGeneration:5},{sessionGeneration:9},{playerObjectId:18}]){
    assert.equal(f.source.currentWallet({...nextScene,...delta}),null);
  }
  f.source.retire();assert.equal(f.source.currentWallet(f.physical),null);
});
check("Pearl explicit boundary retirement and successor observations prevent old socket session or scene ABA authority",()=>{
  for(const delta of [{socket:{}},{connectionGeneration:5},{sessionGeneration:9},{sceneRevision:4},{mapFileName:"1"}]){
    const f=pearlPureFixture(),successor={...f.physical,...delta};f.source.retireCatalog();
    assert.equal(f.source.currentCatalog(f.physical),null);
    const next=f.source.observeCatalog(successor,"NPCPearlGoods",pearlRawCatalog(),pearlStructureOracle);assert.ok(next);
    assert.ok(next.revision>f.catalog.revision);assert.equal(f.source.currentCatalog(f.physical),null);
    assert.strictEqual(f.source.currentCatalog(successor),next);
  }
  for(const delta of [{socket:{}},{connectionGeneration:5},{sessionGeneration:9}]){
    const f=pearlPureFixture(),successor={...f.physical,...delta};f.source.retireWallet();
    const next=f.source.observeWallet(successor,1);assert.ok(next);assert.equal(f.source.currentWallet(f.physical),null);
    assert.strictEqual(f.source.currentWallet(successor),next);
  }
});
check("Pearl capture forwards raw UID display price quantity and only container0 entry count without JS planning",()=>{
  const f=pearlPureFixture();f.current.inventory.items=[{container:0,slot:999},{container:0,slot:999},
    {container:1,slot:0},{container:2,slot:0},{container:3,slot:0},{container:"0",slot:0}];
  for(const quantity of [0,1,99,100,65535]){
    const captured=pearlBuyModule.captureNpcPearlBuy(f.current,quantity);assert.ok(captured);
    assert.deepEqual(pearlBuyModule.npcPearlBuyInput(captured.requestJson),{...pearlScalarInput,quantity,occupied:2});
    assert.equal(captured.selectedId,0);assert.equal(captured.quantity,quantity);
    assert.deepEqual(JSON.parse(captured.authorityJson).catalog.rawPayload,f.catalog.rawPayload);
    assert.equal(Object.hasOwn(JSON.parse(captured.requestJson),"quote"),false);
  }
  const other=pearlBuyModule.captureNpcPearlBuy({...f.current,selectedId:42},2);assert.equal(JSON.parse(other.requestJson).uniqueId,42);
  assert.equal(pearlBuyModule.captureNpcPearlBuy({...f.current,selectedId:77},2),null);
});
check("Pearl capture keeps unknown selected and wallet flags distinct from UID0 known negative balance",()=>{
  const f=pearlPureFixture();
  const unknown=pearlBuyModule.captureNpcPearlBuy({...f.current,selectedId:null,wallet:null,allowsBuy:false,blocked:true},0);
  assert.ok(unknown);assert.equal(unknown.blocked,true);assert.equal(unknown.selectedId,null);
  assert.deepEqual(JSON.parse(unknown.requestJson),{...pearlScalarInput,allowsBuy:false,selected:false,unitPrice:0,quantity:0,walletKnown:false,pearls:0,infoPrice:0});
  const negative=pearlBuyModule.captureNpcPearlBuy({...f.current,wallet:{revision:9,amount:-1}},1);
  assert.ok(negative);assert.equal(JSON.parse(negative.requestJson).walletKnown,true);assert.equal(JSON.parse(negative.requestJson).pearls,-1);
});
check("Pearl capture rejects malformed current authority getters sparse data and bad scalar quantities",()=>{
  for(const quantity of [-1,65536,0.5,"1",NaN,Infinity])assert.equal(pearlBuyModule.captureNpcPearlBuy(pearlPureFixture().current,quantity),null);
  for(const mutate of [c=>{c.currency="gold";},c=>{c.owner.extra=1;},c=>{c.owner.playerObjectId=0;},c=>{c.catalogRevision++;},
    c=>{c.serviceRevision=0;},c=>{c.wallet={revision:1,amount:2147483648};},c=>{c.allowsBuy=1;},c=>{c.inventory.items=[,{}];},
    c=>{c.inventory.toJSON=()=>({items:[]});}]){
    const current=JSON.parse(JSON.stringify(pearlPureFixture().current));mutate(current);
    assert.equal(pearlBuyModule.captureNpcPearlBuy(current,1),null);
  }
  const current=JSON.parse(JSON.stringify(pearlPureFixture().current));let calls=0;
  Object.defineProperty(current.inventory,"items",{enumerable:true,get(){calls++;return [];}});
  assert.equal(pearlBuyModule.captureNpcPearlBuy(current,1),null);assert.equal(calls,0);
});
check("Pearl capture seals full authority context and request strings against later source mutation",()=>{
  const f=pearlPureFixture(),captured=pearlBuyModule.captureNpcPearlBuy(f.current,2);assert.ok(captured);
  const sealed={...captured};f.current.inventory.items.push({container:0,slot:0});f.current.selectedId=42;
  f.current.owner.ownerRevision++;f.current.presentation.windowEpoch++;f.payload.list[0].price=0;
  assert.deepEqual(captured,sealed);
  assert.equal(JSON.parse(captured.requestJson).uniqueId,0);assert.equal(JSON.parse(captured.requestJson).occupied,0);
  assert.equal(JSON.parse(captured.contextJson).owner.ownerRevision,3);
  assert.equal(JSON.parse(captured.authorityJson).catalog.goods[0].tooltipSource.userItem.added_stats[0].value,3);
});
check("Pearl strict scalar DTO rejects coercion missing extra and malformed JSON before a PUI call",()=>{
  for(const text of ["","{","null","[]","true","0",JSON.stringify({})])assert.equal(pearlBuyModule.npcPearlBuyInput(text),null);
  for(const key of Object.keys(pearlScalarInput)){const missing={...pearlScalarInput};delete missing[key];assert.equal(pearlBuyModule.npcPearlBuyInput(JSON.stringify(missing)),null,key);}
  assert.equal(pearlBuyModule.npcPearlBuyInput(JSON.stringify({...pearlScalarInput,extra:true})),null);
  for(const [key,value] of [["allowsBuy",1],["selected","true"],["usePearls",0],["walletKnown",null],
    ["uniqueId",-1],["uniqueId",Number.MAX_SAFE_INTEGER+1],["unitPrice",4294967296],["stock",2147483648],
    ["quantity",65536],["pearls",-2147483649],["occupied",-1],["occupied",0.5],["quantity","2"],["infoPrice",4294967296],["infoPrice","900"],["rate",-1],["rate","1.25"],["rate",1e100]]){
    assert.equal(pearlBuyModule.npcPearlBuyInput(JSON.stringify({...pearlScalarInput,[key]:value})),null,key);
  }
  const read=pearlBuyModule.npcPearlBuyInput(JSON.stringify(pearlScalarInput));assert.deepEqual(read,pearlScalarInput);assert.ok(Object.isFrozen(read));
});
check("Pearl plan DTO validates every fixed denial field and denies incoherent admitted output",()=>{
  assert.deepEqual(pearlBuyModule.parseNpcPearlBuyPlan(pearlValidOutput),{maxQuantity:99,quote:34,admittedCount:2,denial:0});
  for(let denial=1;denial<=7;denial++)assert.deepEqual(pearlBuyModule.parseNpcPearlBuyPlan({...pearlValidOutput,admittedCount:null,denial}),
    {maxQuantity:99,quote:34,admittedCount:null,denial});
  for(const value of [null,[],{version:1,ok:false}, {...pearlValidOutput,extra:1}, {...pearlValidOutput,version:2},
    {...pearlValidOutput,ok:1},{...pearlValidOutput,maxQuantity:98},{...pearlValidOutput,quote:null},
    {...pearlValidOutput,quote:-1},{...pearlValidOutput,quote:4294967296},{...pearlValidOutput,admittedCount:0},
    {...pearlValidOutput,admittedCount:100},{...pearlValidOutput,admittedCount:null},{...pearlValidOutput,denial:1},
    {...pearlValidOutput,denial:8},{...pearlValidOutput,denial:"0"}])assert.equal(pearlBuyModule.parseNpcPearlBuyPlan(value),null);
});
check("Pearl invoke consumes shared admitted count raw UID0 and the existing four-key BuyItem wire",()=>{
  const captured=pearlBuyModule.captureNpcPearlBuy(pearlPureFixture().current,100),runtime={},calls=[];
  const quote=pearlBuyModule.invokeNpcPearlBuy(runtime,function(json){assert.strictEqual(this,runtime);calls.push(json);
    return JSON.stringify({...pearlValidOutput,quote:1683,admittedCount:99});},captured);
  assert.ok(quote);assert.deepEqual(calls,[captured.requestJson]);assert.equal(JSON.parse(calls[0]).quantity,100);
  assert.deepEqual(quote.command,{type:"buyItem",itemIndex:0,count:99,panelType:0});
  assert.deepEqual(Object.keys(quote.command).sort(),["count","itemIndex","panelType","type"]);
  assert.equal(quote.totalGold,null);assert.equal(quote.totalPearls,1683);assert.equal(quote.currency,"pearls");
  assert.ok(Object.isFrozen(quote)&&Object.isFrozen(quote.command));
});
check("Pearl invoke preserves unknown-wallet denial and rejects planner exceptions bad JSON and oversized replies",()=>{
  const captured=pearlBuyModule.captureNpcPearlBuy({...pearlPureFixture().current,wallet:null},1);
  const quote=pearlBuyModule.invokeNpcPearlBuy({},()=>JSON.stringify({...pearlValidOutput,quote:0,admittedCount:null,denial:4}),captured);
  assert.equal(quote.canBuy,false);assert.equal(quote.blockReason,"unknownWallet");assert.equal(quote.command,null);
  for(const reply of [undefined,{},"{","null","x".repeat(1025),JSON.stringify({version:1,ok:false})]){
    assert.equal(pearlBuyModule.invokeNpcPearlBuy({},()=>reply,captured),null);
  }
  assert.equal(pearlBuyModule.invokeNpcPearlBuy({},()=>{throw Error("PUI");},captured),null);
});
check("Pearl optional PUI absence bad ABI and getter exceptions do not affect ordinary presentation capability",()=>{
  const module={cash_preview_abi_version:()=>1,cash_preview_turn:(direction,right)=>right?direction%8+1:(direction+6)%8+1};
  assert.equal(presentationRuntime.readSharedNpcPearlBuyPlan(module,pearlScalarInput),null);
  assert.equal(presentationRuntime.turnSharedCashPreview(module,8,true),1);
  let calls=0;
  for(const version of [0,1,3,"2",null]){assert.equal(presentationRuntime.readSharedNpcPearlBuyPlan({npc_pearl_buy_abi_version:()=>version,
    npc_pearl_buy_plan:()=>{calls++;return JSON.stringify(pearlValidOutput);}},pearlScalarInput),null);}
  assert.equal(calls,0);
  for(const module of [{npc_pearl_buy_abi_version:()=>2},{npc_pearl_buy_plan:()=>JSON.stringify(pearlValidOutput)},
    {get npc_pearl_buy_abi_version(){throw Error("getter");}},{npc_pearl_buy_abi_version:()=>{throw Error("abi");},npc_pearl_buy_plan:()=>JSON.stringify(pearlValidOutput)}]){
    assert.equal(presentationRuntime.readSharedNpcPearlBuyPlan(module,pearlScalarInput),null);
  }
});
check("Pearl PUI scalar host forwards exact signed facts and requires stable ABI getter and planner identity",()=>{
  const calls=[],module={npc_pearl_buy_abi_version(){assert.strictEqual(this,module);return 2;},
    npc_pearl_buy_plan(...args){assert.strictEqual(this,module);calls.push(args);return JSON.stringify(pearlValidOutput);}};
  const input={...pearlScalarInput,pearls:-2147483648,quantity:65535,occupied:45};
  assert.deepEqual(presentationRuntime.readSharedNpcPearlBuyPlan(module,input),{maxQuantity:99,quote:34,admittedCount:2,denial:0});
  assert.deepEqual(calls,[[true,true,true,0,17,-1,65535,true,-2147483648,45,900,1.25]]);
  for(const edge of ["abiReplacesPlanner","plannerReplacesAbi","plannerReplacesPlanner","finalVersion"]){
    let reads=0,entered=0;const m={npc_pearl_buy_abi_version(){reads++;if(edge==="abiReplacesPlanner")m.npc_pearl_buy_plan=()=>JSON.stringify(pearlValidOutput);return edge==="finalVersion"&&reads===2?1:2;},
      npc_pearl_buy_plan(){entered++;if(edge==="plannerReplacesAbi")m.npc_pearl_buy_abi_version=()=>2;
        if(edge==="plannerReplacesPlanner")m.npc_pearl_buy_plan=()=>JSON.stringify(pearlValidOutput);return JSON.stringify(pearlValidOutput);}};
    assert.equal(presentationRuntime.readSharedNpcPearlBuyPlan(m,pearlScalarInput),null,edge);
    assert.equal(entered,edge==="abiReplacesPlanner"?0:1,edge);
  }
  let getterReads=0;const first=()=>2,second=()=>2;
  const getterModule={get npc_pearl_buy_abi_version(){return ++getterReads===1?first:second;},npc_pearl_buy_plan:()=>{throw Error("changed getter must never enter");}};
  assert.equal(presentationRuntime.readSharedNpcPearlBuyPlan(getterModule,pearlScalarInput),null);
});
check("Pearl PUI input coercion getters and toJSON cannot enter a scalar planner",()=>{
  let calls=0;const module={npc_pearl_buy_abi_version:()=>2,npc_pearl_buy_plan:()=>{calls++;return JSON.stringify(pearlValidOutput);}};
  for(const [key,value] of [["allowsBuy",1],["walletKnown","true"],["pearls","0"],["stock",2147483648],["quantity",0.5],["occupied",NaN]]){
    assert.equal(presentationRuntime.readSharedNpcPearlBuyPlan(module,{...pearlScalarInput,[key]:value}),null,key);
  }
  const getter={...pearlScalarInput};let getterCalls=0;Object.defineProperty(getter,"quantity",{enumerable:true,get(){getterCalls++;return 2;}});
  assert.equal(presentationRuntime.readSharedNpcPearlBuyPlan(module,getter),null);
  assert.equal(presentationRuntime.readSharedNpcPearlBuyPlan(module,{...pearlScalarInput,toJSON(){throw Error("must not execute");}}),null);
  assert.equal(calls,0);assert.equal(getterCalls,0);
});
check("Pearl PUI bad fixed outputs fail closed and valid shared denial fields remain available",()=>{
  for(const reply of ["{","null",JSON.stringify({version:1,ok:false}),JSON.stringify({...pearlValidOutput,admittedCount:100}),
    JSON.stringify({...pearlValidOutput,denial:1}),JSON.stringify({...pearlValidOutput,extra:true}),"x".repeat(1025),null]){
    const module={npc_pearl_buy_abi_version:()=>2,npc_pearl_buy_plan:()=>reply};
    assert.equal(presentationRuntime.readSharedNpcPearlBuyPlan(module,pearlScalarInput),null);
  }
  for(let denial=1;denial<=7;denial++){
    const module={npc_pearl_buy_abi_version:()=>2,npc_pearl_buy_plan:()=>JSON.stringify({...pearlValidOutput,admittedCount:null,denial})};
    assert.deepEqual(presentationRuntime.readSharedNpcPearlBuyPlan(module,pearlScalarInput),{maxQuantity:99,quote:34,admittedCount:null,denial});
  }
});


// Source24 real NPC DOM declarations evaluated only as memory element records.
const pearlShopDomSource=readFileSync(new URL("../app/components/original-client-game-shop.tsx",import.meta.url),"utf8");
const pearlShopDomAst=ts.createSourceFile("pearl-npc-shop-dom.tsx",pearlShopDomSource,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const pearlShopDomNames=new Set(["NpcShopWindow","npcShopTabLabel","formatGold","confirmEnabledFor","NPC_SHOP_GRADE_COLOUR","shopStyle"]);
const pearlShopDomDeclarations=new Map();
for(const node of pearlShopDomAst.statements){
  const name=ts.isFunctionDeclaration(node)?node.name?.text:ts.isVariableStatement(node)&&node.declarationList.declarations.length===1
    ?node.declarationList.declarations[0].name.getText(pearlShopDomAst):null;
  if(pearlShopDomNames.has(name)){assert.equal(pearlShopDomDeclarations.has(name),false);pearlShopDomDeclarations.set(name,node.getText(pearlShopDomAst));}
}
assert.deepEqual([...pearlShopDomDeclarations.keys()].sort(),[...pearlShopDomNames].sort());
const pearlShopDomJs=ts.transpileModule([...pearlShopDomDeclarations.values()].join("\n"),{
  compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022,jsx:ts.JsxEmit.ReactJSX}}).outputText;
function pearlDomElements(value,result=[]){
  if(Array.isArray(value))for(const child of value)pearlDomElements(child,result);
  else if(value&&typeof value==="object"&&Object.hasOwn(value,"props")){result.push(value);pearlDomElements(value.props.children,result);}
  return result;
}
function pearlNpcDomFixture(delta={}){
  const state=[],refs=[],effects=[],layoutEffects=[],buys=[],quotes=[];let stateIndex=0,refIndex=0,effectIndex=0,layoutIndex=0;
  let answer={currency:"pearls",maxQuantity:99,totalGold:null,totalPearls:34,canBuy:true,blockReason:null,
    command:{type:"buyItem",itemIndex:0,count:2,panelType:0}},quoteEdge=null,inputBlocked=false;
  const pending=[];
  const observeEffect=(records,index,fn,deps)=>{const old=records[index];if(!old||!deps||deps.some((value,i)=>value!==old.deps?.[i])){
    records[index]={deps:[...deps],fn};pending.push(()=>{old?.cleanup?.();records[index].cleanup=fn();});}};
  const hooks={useState(initial){const index=stateIndex++;if(!(index in state))state[index]=typeof initial==="function"?initial():initial;
      return[state[index],next=>{state[index]=typeof next==="function"?next(state[index]):next;}];},
    useRef(initial){const index=refIndex++;return refs[index]??(refs[index]={current:initial});},useMemo:fn=>fn(),
    useEffect:(fn,deps)=>observeEffect(effects,effectIndex++,fn,deps),useLayoutEffect:(fn,deps)=>observeEffect(layoutEffects,layoutIndex++,fn,deps)};
  const jsx=(type,props)=>({type,props});const exports={};
  // Asset strings and JSX records are inert. Every selection/quote/confirm handler
  // comes from the complete actual NpcShopWindow function above.
  new Function("exports","require",...Object.keys(hooks),"ORIGINAL_UI","originalAssetPath","originalItemIconPath","SpriteButton",pearlShopDomJs)(
    exports,name=>{assert.equal(name,"react/jsx-runtime");return{jsx,jsxs:jsx,Fragment:"Fragment"};},...Object.values(hooks),
    {inventory:{closeButton:{}}},value=>value,icon=>"memory-icon:"+icon,"SpriteButton");
  const props={t:(_key,_args,fallback)=>fallback??_key,npcName:"Pearl trader",gold:0,currency:"pearls",pearlBalance:200,
    availableTabs:["buy"],buyItems:[{id:0,name:"Same name",price:17,icon:2,count:1,requiresPearlBuyPlan:true},
      {id:42,name:"Same name",price:17,icon:2,count:1,requiresPearlBuyPlan:true}],onQuoteBuy:(id,quantity)=>{
      quotes.push({id,quantity});quoteEdge?.();return answer;},onBuy:(id,quantity)=>buys.push({id,quantity}),
    getInputBlocked:()=>inputBlocked,onClose:()=>{},...delta};
  const render=()=>{stateIndex=0;refIndex=0;effectIndex=0;layoutIndex=0;const tree=exports.NpcShopWindow(props);while(pending.length)pending.shift()();return tree;};
  const find=(tree,predicate)=>pearlDomElements(tree).find(predicate);
  return{props,buys,quotes,render,find,state,setAnswer:value=>{answer=value;},setQuoteEdge:value=>{quoteEdge=value;},
    setBlocked:value=>{inputBlocked=value;},row:(tree,id)=>find(tree,node=>node.props.className==="npc-shop-row"&&node.props["data-item-id"]===String(id)),
    confirm:tree=>find(tree,node=>node.props.className==="npc-shop-confirm")};
}
check("Pearl actual NPC DOM keeps UID0 same-name rows and a shared Pearl quote despite zero Gold",()=>{
  const f=pearlNpcDomFixture();let tree=f.render();assert.ok(f.row(tree,0)&&f.row(tree,42));
  f.row(tree,0).props.onClick();tree=f.render();
  assert.equal(f.row(tree,0).props["aria-pressed"],true);assert.equal(f.row(tree,42).props["aria-pressed"],false);
  assert.equal(f.confirm(tree).props.disabled,false);assert.ok(f.quotes.some(call=>call.id===0));
  f.confirm(tree).props.onClick();assert.deepEqual(f.buys,[{id:0,quantity:1}]);
});
check("Pearl actual NPC DOM confirm obtains a fresh quote and rejects changed denial or Gold branding",()=>{
  for(const edge of ["denied","gold","unknown"]){const f=pearlNpcDomFixture();let tree=f.render();f.row(tree,0).props.onClick();tree=f.render();
    assert.equal(f.confirm(tree).props.disabled,false);const before=f.quotes.length;
    if(edge==="denied")f.setAnswer({currency:"pearls",maxQuantity:99,totalGold:null,totalPearls:34,canBuy:false,blockReason:"pearls",command:null});
    if(edge==="gold")f.setAnswer({maxQuantity:99,totalGold:34,canBuy:true,blockReason:null,command:{type:"buyItem",itemIndex:0,count:1,panelType:0}});
    if(edge==="unknown")f.setAnswer(null);
    f.confirm(tree).props.onClick();assert.equal(f.quotes.length,before+1,edge);assert.deepEqual(f.quotes.at(-1),{id:0,quantity:1});assert.equal(f.buys.length,0,edge);
  }
  const f=pearlNpcDomFixture();let tree=f.render();f.row(tree,0).props.onClick();tree=f.render();
  const input=f.find(tree,node=>node.type==="input"&&node.props.type==="number");input.props.onChange({target:{value:"100"}});tree=f.render();
  assert.equal(f.find(tree,node=>node.type==="input"&&node.props.type==="number").props.value,99);
  f.confirm(tree).props.onClick();assert.deepEqual(f.quotes.at(-1),{id:0,quantity:99});assert.deepEqual(f.buys,[{id:0,quantity:99}]);
});
check("Pearl actual NPC DOM double-click freshly quotes that raw row and never trusts a selected other UID",()=>{
  const f=pearlNpcDomFixture();let tree=f.render();f.row(tree,42).props.onClick();tree=f.render();
  const before=f.quotes.length;f.row(tree,0).props.onDoubleClick();
  assert.equal(f.quotes.length,before+1);assert.deepEqual(f.quotes.at(-1),{id:0,quantity:1});assert.deepEqual(f.buys,[{id:0,quantity:1}]);
  f.setAnswer({maxQuantity:99,totalGold:0,canBuy:true,blockReason:null,command:{type:"buyItem",itemIndex:0,count:1,panelType:0}});
  f.row(tree,0).props.onDoubleClick();assert.equal(f.buys.length,1,"foreign Gold quote cannot authorize Pearl double-click");
});
check("Pearl actual NPC DOM unknown wallet rejects free quotes while a known negative wallet displays zero",()=>{
  for(const balance of [null,undefined,"0",0.5,NaN,Infinity]){const f=pearlNpcDomFixture({pearlBalance:balance});
    f.setAnswer({currency:"pearls",maxQuantity:99,totalGold:null,totalPearls:0,canBuy:true,blockReason:null,command:{type:"buyItem",itemIndex:0,count:1,panelType:0}});
    let tree=f.render();f.row(tree,0).props.onClick();tree=f.render();assert.equal(f.confirm(tree).props.disabled,true);
    f.confirm(tree).props.onClick();f.row(tree,0).props.onDoubleClick();assert.equal(f.buys.length,0);
    assert.ok(pearlDomElements(tree).some(node=>node.type==="span"&&node.props.children==="—"));
  }
  const f=pearlNpcDomFixture({pearlBalance:-1});f.setAnswer({currency:"pearls",maxQuantity:99,totalGold:null,totalPearls:0,canBuy:true,blockReason:null,command:{type:"buyItem",itemIndex:0,count:1,panelType:0}});
  let tree=f.render();f.row(tree,0).props.onClick();tree=f.render();assert.equal(f.confirm(tree).props.disabled,false);
  assert.ok(pearlDomElements(tree).some(node=>node.type==="span"&&node.props.children==="0"));
});
check("Pearl actual NPC DOM reentrant quote input blocker stops both confirmation and double-click",()=>{
  for(const action of ["confirm","double"]){const f=pearlNpcDomFixture();let tree=f.render();f.row(tree,0).props.onClick();tree=f.render();
    f.setQuoteEdge(()=>f.setBlocked(true));if(action==="confirm")f.confirm(tree).props.onClick();else f.row(tree,0).props.onDoubleClick();assert.equal(f.buys.length,0,action);
  }
});
check("Pearl actual Scene binds currency wallet raw UID0 and existing Page quote and buy callbacks",()=>{
  const source=readFileSync(new URL("../app/components/original-client-game-ui-scene.tsx",import.meta.url),"utf8");
  const ast=ts.createSourceFile("pearl-scene.tsx",source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),matches=[];
  (function visit(node){if(ts.isJsxSelfClosingElement(node)&&node.tagName.getText(ast)==="NpcShopWindow"
    &&node.attributes.properties.some(attr=>ts.isJsxAttribute(attr)&&attr.name.text==="currency"))matches.push(node);ts.forEachChild(node,visit);})(ast);
  assert.equal(matches.length,1);const fields=new Map(matches[0].attributes.properties.filter(ts.isJsxAttribute).map(attr=>[attr.name.text,attr.initializer]));
  const names=["currency","pearlBalance","onQuoteBuy","onBuy"],js=ts.transpileModule("return {"+names.map(name=>name+":"+fields.get(name).expression.getText(ast)).join(",")+"};",
    {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
  const calls=[],binding=new Function("npcShopService","onQuoteNpcShopItem","onBuyNpcShopItem",js)(
    {currency:"pearls",pearlBalance:-1,panelType:0},(id,quantity)=>{calls.push({kind:"quote",id,quantity});return null;},(id,quantity,panelType)=>calls.push({kind:"buy",id,quantity,panelType}));
  assert.equal(binding.currency,"pearls");assert.equal(binding.pearlBalance,-1);binding.onQuoteBuy(0,100);binding.onBuy(0,100);
  assert.deepEqual(calls,[{kind:"quote",id:0,quantity:100},{kind:"buy",id:0,quantity:100,panelType:0}]);
});
check("Pearl actual Core facade delegates exact input to optional PUI and returns fixed fail-closed data",()=>{
  const source=readFileSync(new URL("../lib/client-core-runtime.ts",import.meta.url),"utf8"),ast=ts.createSourceFile("pearl-core-facade.ts",source,ts.ScriptTarget.Latest,true),methods=[];
  (function visit(node){if(ts.isMethodDeclaration(node)&&node.name.getText(ast)==="getMir2NpcPearlBuyPlan")methods.push(node);ts.forEachChild(node,visit);})(ast);
  assert.equal(methods.length,1);const js=ts.transpileModule("return {"+methods[0].getText(ast)+"};",{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
  const calls=[],module={npc_pearl_buy_abi_version:()=>2,npc_pearl_buy_plan(...args){calls.push(args);return JSON.stringify(pearlValidOutput);}};
  const facade=new Function("npcPearlBuyInput","readSharedNpcPearlBuyPlan","presentation",js)(pearlBuyModule.npcPearlBuyInput,presentationRuntime.readSharedNpcPearlBuyPlan,module);
  assert.deepEqual(JSON.parse(facade.getMir2NpcPearlBuyPlan(JSON.stringify(pearlScalarInput))),pearlValidOutput);
  assert.deepEqual(calls,[[true,true,true,0,17,-1,2,true,200,0,900,1.25]]);
  assert.deepEqual(JSON.parse(facade.getMir2NpcPearlBuyPlan("{bad json")),{version:1,ok:false});assert.equal(calls.length,1);
  delete module.npc_pearl_buy_abi_version;assert.deepEqual(JSON.parse(facade.getMir2NpcPearlBuyPlan(JSON.stringify(pearlScalarInput))),{version:1,ok:false});
});


check("Pearl complete source requires a per-row explicit price Oracle and keeps no partial admitted catalog",()=>{
  for(const oracle of [undefined,()=>false,()=>1,()=>{throw Error("price capability missing");}]){
    const f=pearlPureFixture();
    assert.equal(f.source.observeCatalog(f.physical,"NPCPearlGoods",pearlRawCatalog(),oracle),null);
    assert.equal(f.source.currentCatalog(f.physical),null);
  }
  const f=pearlPureFixture(),calls=[];
  const value=f.source.observeCatalog(f.physical,"NPCPearlGoods",pearlRawCatalog(),(...args)=>{calls.push(args);return calls.length===1;});
  assert.equal(value,null);assert.equal(f.source.currentCatalog(f.physical),null);
  assert.deepEqual(calls,[[900,1.25,17],[900,1.25,17]]);
});
check("Pearl raw source deeply freezes canonical tooltip nested stats while owning an independent row clone",()=>{
  const f=pearlPureFixture(),row=f.catalog.goods[0];
  assert(Object.isFrozen(row.tooltipSource));assert(Object.isFrozen(row.tooltipSource.info.stats));
  assert(Object.isFrozen(row.tooltipSource.userItem.added_stats));assert(Object.isFrozen(row.tooltipSource.userItem.added_stats[0]));
  assert.throws(()=>{row.tooltipSource.userItem.added_stats[0].value=99;},TypeError);
  assert.throws(()=>row.tooltipSource.info.stats.push({stat:7,value:9}),TypeError);
  f.payload.list[0].tooltipSource.userItem.added_stats[0].value=99;
  assert.equal(row.tooltipSource.userItem.added_stats[0].value,3);
  const raw=f.catalog.rawPayload;assert.equal(raw.list[0].added_stats[0].value,3);
  assert.equal(row.infoPrice,900);assert.equal(row.unitPrice,17);
});
check("Pearl recursive price Oracle retirement never overwrites the successor catalog with outer observations",()=>{
  for(const kind of ["retire","successor"]){
    const f=pearlPureFixture();let entered=false,successor=null;
    const outer=f.source.observeCatalog(f.physical,"NPCPearlGoods",pearlRawCatalog(),()=>{
      if(!entered){entered=true;if(kind==="retire")f.source.retireCatalog();
        else successor=f.source.observeCatalog(f.physical,"NPCPearlGoods",pearlRawCatalog(),pearlStructureOracle);}
      return true;
    });
    assert.equal(entered,true);assert.equal(outer,null);
    assert.strictEqual(f.source.currentCatalog(f.physical),successor);
    if(successor)assert(successor.revision>f.catalog.revision);
  }
});


check("Pearl every complete Info User and tooltip carrier field is required before the source price Oracle",()=>{
  for(const [carrier,keys] of [["info",Object.keys(pearlRawGood().tooltipSource.info)],
    ["userItem",crystalItemSource.crystalUserItemFields],["tooltipSource",["info","realInfo","userItem","socketInfos","realSocketInfos"]],
    ["row",crystalItemSource.crystalUserItemFields]]){
    for(const key of keys){const f=pearlPureFixture(),raw=pearlRawCatalog();let calls=0;
      const target=carrier==="row"?raw.list[0]:carrier==="tooltipSource"?raw.list[0].tooltipSource:raw.list[0].tooltipSource[carrier];
      delete target[key];assert.equal(f.source.observeCatalog(f.physical,"NPCPearlGoods",raw,()=>{calls++;return true;}),null,carrier+"."+key);
      assert.equal(calls,0,carrier+"."+key);assert.equal(f.source.currentCatalog(f.physical),null);
    }
  }
  const f=pearlPureFixture(),raw=pearlRawCatalog();raw.list[0].tooltipSource.userItem.awake_values=[1];
  assert.equal(f.source.observeCatalog(f.physical,"NPCPearlGoods",raw,pearlStructureOracle),null,"top-level User22 must equal the complete raw user body");
});


check("Pearl root rate negative zero normalizes once while nested price identity stat and date negative zero reject",()=>{
  const f=pearlPureFixture(),raw=pearlRawCatalog();raw.rate=-0;raw.list.forEach(row=>{row.price=0;});const calls=[];
  const catalog=f.source.observeCatalog(f.physical,"NPCPearlGoods",raw,(...args)=>{calls.push(args);return true;});assert(catalog);
  assert.equal(Object.is(catalog.rate,-0),false);assert.equal(catalog.rate,0);
  assert.deepEqual(calls,[[900,0,0],[900,0,0]]);assert.equal(catalog.rawPayload.rate,0);
  assert.throws(()=>pearlSourceModule.npcPearlJson({nested:{rate:-0}}));
  for(const mutate of [row=>{row.price=-0;},row=>{row.unique_id=-0;},row=>{row.added_stats[0].value=-0;},
    row=>{row.tooltipSource.userItem.expire_info={expiry_binary_datetime:-0};},
    row=>{row.tooltipSource.userItem.expire_info={expiry_binary_datetime:"-0"};}]){
    const next=pearlRawCatalog();mutate(next.list[0]);assert.equal(f.source.observeCatalog(f.physical,"NPCPearlGoods",next,pearlStructureOracle),null);
  }
});


function pearlExpandedRawCatalog(size=8,descriptionSize=4096){
  return {panelType:0,rate:1.25,list:Array.from({length:size},(_,uid)=>{
    const row=pearlRawGood(uid);row.description="d".repeat(descriptionSize);return row;
  })};
}
check("Pearl whole rawPayload above 16KiB remains structured frozen custody through source capture and fixed Core quote",()=>{
  const f=pearlPureFixture(),raw=pearlExpandedRawCatalog(),before=JSON.parse(JSON.stringify(raw));
  const bytes=Buffer.byteLength(JSON.stringify(raw));assert(bytes>16384&&bytes<2097152);
  const catalog=f.source.observeCatalog(f.physical,"NPCPearlGoods",raw,pearlStructureOracle);assert(catalog);
  assert.equal(Object.hasOwn(catalog,"rawJson"),false);assert.deepEqual(catalog.rawPayload,before);
  assert(Object.isFrozen(catalog.rawPayload));assert(Object.isFrozen(catalog.rawPayload.list));
  assert(Object.isFrozen(catalog.rawPayload.list[0]));assert(Object.isFrozen(catalog.rawPayload.list[0].tooltipSource.userItem.added_stats[0]));
  assert.throws(()=>{catalog.rawPayload.list[0].price=0;},TypeError);
  const current={...f.current,catalog,catalogRevision:catalog.revision};
  const captured=pearlBuyModule.captureNpcPearlBuy(current,2);assert(captured);
  assert.deepEqual(JSON.parse(captured.authorityJson).catalog.rawPayload,before);
  const calls=[],quote=pearlBuyModule.invokeNpcPearlBuy({},json=>{calls.push(JSON.parse(json));return JSON.stringify(pearlValidOutput);},captured);
  assert.equal(quote.currency,"pearls");assert.equal(quote.totalPearls,34);
  assert.deepEqual(quote.command,{type:"buyItem",itemIndex:0,count:2,panelType:0});assert.equal(calls.length,1);
  raw.list[0].unique_id=999;raw.list[0].tooltipSource.userItem.added_stats[0].value=999;raw.list.pop();
  assert.deepEqual(catalog.rawPayload,before);assert.deepEqual(JSON.parse(captured.authorityJson).catalog.rawPayload,before);
  assert.equal(catalog.goods.length,8);assert.equal(catalog.goods[0].uniqueId,0);
});
check("Pearl complete raw catalog beyond 2MiB rejects the entire source before every price Oracle",()=>{
  const f=pearlPureFixture(),raw=pearlExpandedRawCatalog(256,8192);assert(Buffer.byteLength(JSON.stringify(raw))>2097152);
  let calls=0;assert.equal(f.source.observeCatalog(f.physical,"NPCPearlGoods",raw,()=>{calls++;return true;}),null);
  assert.equal(calls,0);assert.equal(f.source.currentCatalog(f.physical),null);
  assert.throws(()=>pearlSourceModule.npcPearlJson(raw));
  assert.throws(()=>pearlSourceModule.npcPearlJson({field:"s".repeat(16385)}),"ordinary source string budget remains strict");
});

// Source25 optional ABI fixtures use fixed JS responses, never WASM execution.
// They check data custody and actual source presentation, not gesture/send rights.
function rankingAdmissionInput(delta={}) { return {version:1,opened:true,rankingsReady:true,pending:false,playerId:0,nowMs:1001,nextReadyMs:1000,...delta}; }
function rankingAdmissionModule(reply={version:1,ok:true,objectId:0,nextReadyMs:1501}) {
  const calls=[];
  return {calls,module:{ranking_inspect_abi_version:()=>1,ranking_inspect_admission(json){calls.push(JSON.parse(json));return JSON.stringify(reply);}}};
}
check("Ranking Inspect optional ABI delegates exact seven fields and preserves ID0",()=>{
  const f=rankingAdmissionModule(),input=rankingAdmissionInput();
  const result=presentationRuntime.readSharedRankingInspectAdmission(f.module,input);
  assert.deepEqual(result,{version:1,ok:true,objectId:0,nextReadyMs:1501});assert(Object.isFrozen(result));
  assert.deepEqual(f.calls,[input]);assert.equal(Object.keys(f.calls[0]).length,7);
  assert.strictEqual(presentationRuntime.parseRankingPlayerInspect,rankingInspectAdapter.parseRankingPlayerInspect);
});
check("Ranking Inspect absent optional capability never affects ordinary map admission",()=>{
  for(const module of [{},{ranking_inspect_abi_version:()=>1},{ranking_inspect_admission:()=>"{}"},
    {ranking_inspect_abi_version:()=>2,ranking_inspect_admission:()=>{throw Error("must not call");}}])
    assert.equal(presentationRuntime.readSharedRankingInspectAdmission(module,rankingAdmissionInput()),null);
  assert.deepEqual(presentationRuntime.searchSharedMapRoute({getMir2MapRouteVersion:()=>1,getMir2MapRoutePlan:()=>Int32Array.of(0)},
    {width:1,height:1,origin:{x:0,y:0},goal:{x:0,y:0},edges:new Uint8Array(1)}),{status:"ok",steps:[]});
});
check("Ranking Inspect strict input bounds booleans exact keys and non-data descriptors reject before getter",()=>{
  const f=rankingAdmissionModule();
  for(const delta of [{version:2},{opened:1},{rankingsReady:null},{pending:"false"},{playerId:-1},{playerId:4294967296},
    {playerId:0.5},{playerId:-0},{nowMs:-1},{nowMs:Number.MAX_SAFE_INTEGER+1},{nextReadyMs:NaN},{nextReadyMs:1.5},{extra:true}])
    assert.equal(presentationRuntime.readSharedRankingInspectAdmission(f.module,rankingAdmissionInput(delta)),null);
  const missing=rankingAdmissionInput();delete missing.pending;
  assert.equal(presentationRuntime.readSharedRankingInspectAdmission(f.module,missing),null);
  let touched=0;const accessor=rankingAdmissionInput();Object.defineProperty(accessor,"opened",{enumerable:true,get(){touched++;return true;}});
  assert.equal(presentationRuntime.readSharedRankingInspectAdmission(f.module,accessor),null);assert.equal(touched,0);assert.equal(f.calls.length,0);
});
check("Ranking Inspect JS does not duplicate shared readiness or cooldown planning",()=>{
  const f=rankingAdmissionModule({version:1,ok:false});
  for(const delta of [{opened:false},{rankingsReady:false},{pending:true},{nowMs:1000},{nowMs:0,nextReadyMs:9999}]) {
    const input=rankingAdmissionInput(delta);
    assert.deepEqual(presentationRuntime.readSharedRankingInspectAdmission(f.module,input),{version:1,ok:false});
    assert.deepEqual(f.calls.at(-1),input);
  }
  assert.equal(f.calls.length,5);
});
check("Ranking Inspect strict rejection and success output shapes reject foreign fields or unsafe integers",()=>{
  for(const reply of [{version:1,ok:false,objectId:0},{version:1,ok:true},{version:2,ok:false},{version:1,ok:0},
    {version:1,ok:true,objectId:1,nextReadyMs:1501},{version:1,ok:true,objectId:4294967296,nextReadyMs:1501},
    {version:1,ok:true,objectId:0,nextReadyMs:Number.MAX_SAFE_INTEGER+1},
    {version:1,ok:true,objectId:0,nextReadyMs:-1},{version:1,ok:true,objectId:0,nextReadyMs:1501,extra:true}]) {
    const f=rankingAdmissionModule(reply);assert.equal(presentationRuntime.readSharedRankingInspectAdmission(f.module,rankingAdmissionInput()),null);
  }
  for(const raw of ["{", "x".repeat(1025), '{"version":1,"ok":true,"objectId":0,"nextReadyMs":-0}']) {
    const f=rankingAdmissionModule();f.module.ranking_inspect_admission=()=>raw;
    assert.equal(presentationRuntime.readSharedRankingInspectAdmission(f.module,rankingAdmissionInput()),null);
  }
});
check("Ranking Inspect whole canonical ABI response rejects raw and escaped duplicate keys",()=>{
  const success='{"version":1,"ok":true,"objectId":0,"nextReadyMs":1501}',rejected='{"version":1,"ok":false}';
  for(const [canonical,fields] of [[success,[["version",1],["ok",true],["objectId",0],["nextReadyMs",1501]]],
    [rejected,[["version",1],["ok",false]]]]){
    for(const [key,value] of fields){
      const original=JSON.stringify(key)+":"+JSON.stringify(value);
      const escaped='"\\u'+key.charCodeAt(0).toString(16).padStart(4,"0")+key.slice(1)+'":'+JSON.stringify(value);
      for(const duplicate of [original,escaped]){
        const raw=canonical.replace(original,original+","+duplicate);assert.notEqual(raw,canonical);
        const f=rankingAdmissionModule();f.module.ranking_inspect_admission=()=>raw;
        assert.equal(presentationRuntime.readSharedRankingInspectAdmission(f.module,rankingAdmissionInput()),null,key+":"+duplicate);
      }
    }
  }
});
check("Ranking Inspect only fixed Rust canonical ordering and number forms publish ABI plans",()=>{
  const canonical='{"version":1,"ok":true,"objectId":0,"nextReadyMs":1501}';
  for(const raw of ['{"ok":true,"version":1,"objectId":0,"nextReadyMs":1501}',canonical+"\n"," "+canonical,
    canonical.replace('"version"','"vers\\u0069on"'),canonical.replace('"objectId":0','"objectId":0e0'),
    canonical.replace('"nextReadyMs":1501','"nextReadyMs":1501.0'),'{"ok":false,"version":1}',
    '{"version":1,"ok":false}\n']){
    const f=rankingAdmissionModule();f.module.ranking_inspect_admission=()=>raw;
    assert.equal(presentationRuntime.readSharedRankingInspectAdmission(f.module,rankingAdmissionInput()),null,raw);
  }
  for(const [raw,input,expected] of [[canonical,rankingAdmissionInput(),{version:1,ok:true,objectId:0,nextReadyMs:1501}],
    ['{"version":1,"ok":false}',rankingAdmissionInput(),{version:1,ok:false}],
    ['{"version":1,"ok":true,"objectId":0,"nextReadyMs":9007199254740991}',
      rankingAdmissionInput({nowMs:Number.MAX_SAFE_INTEGER-500,nextReadyMs:Number.MAX_SAFE_INTEGER-501}),
      {version:1,ok:true,objectId:0,nextReadyMs:Number.MAX_SAFE_INTEGER}]]){
    const f=rankingAdmissionModule();f.module.ranking_inspect_admission=()=>raw;
    assert.deepEqual(presentationRuntime.readSharedRankingInspectAdmission(f.module,input),expected);
  }
});
check("Ranking Inspect maximum u32 and safe JS clocks remain literal without u64 coercion",()=>{
  const input=rankingAdmissionInput({playerId:4294967295,nowMs:Number.MAX_SAFE_INTEGER,nextReadyMs:0});
  const f=rankingAdmissionModule({version:1,ok:true,objectId:4294967295,nextReadyMs:Number.MAX_SAFE_INTEGER});
  assert.deepEqual(presentationRuntime.readSharedRankingInspectAdmission(f.module,input),
    {version:1,ok:true,objectId:4294967295,nextReadyMs:Number.MAX_SAFE_INTEGER});assert.deepEqual(f.calls,[input]);
});
check("Ranking Inspect changed optional ABI identities and mutated input reject publication",()=>{
  for(const edge of ["abi","getter","input","throw"]){
    const input=rankingAdmissionInput(),f=rankingAdmissionModule(),getter=f.module.ranking_inspect_admission;
    f.module.ranking_inspect_admission=function(json){const answer=getter(json);
      if(edge==="abi")f.module.ranking_inspect_abi_version=()=>1;
      if(edge==="getter")f.module.ranking_inspect_admission=()=>answer;
      if(edge==="input")input.playerId=1;
      if(edge==="throw")throw Error("optional getter unavailable");return answer;};
    assert.equal(presentationRuntime.readSharedRankingInspectAdmission(f.module,input),null,edge);
  }
});
check("Ranking Inspect reentrant ABI property and function reads invalidate outer publication",()=>{
  for(const edge of ["property","abi","getter"]){
    const input=rankingAdmissionInput(),f=rankingAdmissionModule(),base=f.module.ranking_inspect_admission;let entered=false,nested="unset";
    const reenter=()=>{if(!entered){entered=true;nested=presentationRuntime.readSharedRankingInspectAdmission(f.module,input);}};
    if(edge==="property")Object.defineProperty(f.module,"ranking_inspect_admission",{get(){reenter();return base;}});
    if(edge==="abi")f.module.ranking_inspect_abi_version=()=>{reenter();return 1;};
    if(edge==="getter")f.module.ranking_inspect_admission=json=>{reenter();return base(json);};
    assert.equal(presentationRuntime.readSharedRankingInspectAdmission(f.module,input),null,edge);assert.equal(nested,null,edge);
  }
});
function rankingInspectRawRow(uid=0) {
  const pearl=pearlRawGood(uid),source=pearl.tooltipSource;
  return {...JSON.parse(JSON.stringify(source.userItem)),name:source.info.name,icon:source.info.image,tooltipSource:JSON.parse(JSON.stringify(source))};
}
function rankingInspectPayload(delta={}) {
  const equipment=Array(14).fill(null);equipment[0]=rankingInspectRawRow();equipment[13]=rankingInspectRawRow(42);
  return {name:"Actual target",guildName:"Target guild",guildRank:"Officer",equipment,class:"Archer",gender:"Female",hair:7,
    level:45,loverName:"Target lover",allowObserve:false,isHero:false,...delta};
}
check("Ranking PlayerInspect projects actual target facts and fourteen stable positional holes",()=>{
  const input=rankingInspectPayload(),info=rankingInspectAdapter.parseRankingPlayerInspect(input,"Actual target");assert(info);
  assert.equal(info.name,"Actual target");assert.equal(info.class,"Archer");assert.equal(info.gender,"Female");assert.equal(info.hair,7);
  assert.equal(info.level,45);assert.equal(info.guildName,"Target guild");assert.equal(info.guildRank,"Officer");assert.equal(info.loverName,"Target lover");
  assert.equal(info.allowObserve,false);assert.equal(info.isHero,false);assert.equal(info.equipment.length,14);
  assert.equal(info.equipment[0].uniqueId,0);assert.equal(info.equipment[0].slotIndex,0);assert.equal(info.equipment[13].slotIndex,13);
  assert(info.equipment.slice(1,13).every(item=>item===null));assert.equal(info.equipment[0].icon,input.equipment[0].tooltipSource.info.image);
  assert.equal(info.equipment[0].sourceKind,"instance");assert(Object.isFrozen(info));assert(Object.isFrozen(info.equipment));
});
check("Ranking PlayerInspect rejects requester fallback wrong target Hero and missing identity facts",()=>{
  assert.equal(rankingInspectAdapter.parseRankingPlayerInspect(rankingInspectPayload(),"Requester"),null);
  for(const delta of [{name:"Requester"},{isHero:true},{hero:false,isHero:undefined},{class:"archer"},{class:4},{gender:1},
    {hair:256},{level:65536},{level:"45"},{allowObserve:0},{guildName:null},{guildRank:undefined},{loverName:null}])
    assert.equal(rankingInspectAdapter.parseRankingPlayerInspect(rankingInspectPayload(delta),"Actual target"),null);
  for(const key of ["name","guildName","guildRank","class","gender","hair","level","loverName","allowObserve","isHero","equipment"]){
    const input=rankingInspectPayload();delete input[key];assert.equal(rankingInspectAdapter.parseRankingPlayerInspect(input,"Actual target"),null,key);
  }
});
check("Ranking PlayerInspect rejects short long sparse or fabricated equipment",()=>{
  for(const equipment of [[],Array(13).fill(null),Array(15).fill(null),Array(14),[...Array(13).fill(null),{}]])
    assert.equal(rankingInspectAdapter.parseRankingPlayerInspect(rankingInspectPayload({equipment}),"Actual target"),null);
  const empty=rankingInspectAdapter.parseRankingPlayerInspect(rankingInspectPayload({equipment:Array(14).fill(null)}),"Actual target");
  assert(empty);assert.deepEqual(empty.equipment,Array(14).fill(null));
});
check("Ranking PlayerInspect requires all raw User22 and actual template carrier facts",()=>{
  for(const key of crystalItemSource.crystalUserItemFields){const input=rankingInspectPayload();delete input.equipment[0][key];
    assert.equal(rankingInspectAdapter.parseRankingPlayerInspect(input,"Actual target"),null,key);}
  for(const mutate of [row=>delete row.tooltipSource,row=>delete row.tooltipSource.info,row=>{row.tooltipSource.info=null;},
    row=>{row.tooltipSource.userItem.count=2;},row=>{row.tooltipSource.info.item_index=99;},row=>{row.unique_id=Number.MAX_SAFE_INTEGER+1;},
    row=>delete row.name,row=>delete row.icon,row=>{row.name="Requester item";},row=>{row.icon=65536;},row=>{row.icon=-1;},
    row=>{row.unique_id="18446744073709551615";},row=>{row.tooltipSource.userItem.unique_id=Number.MAX_SAFE_INTEGER+1;}]){
    const input=rankingInspectPayload();mutate(input.equipment[0]);assert.equal(rankingInspectAdapter.parseRankingPlayerInspect(input,"Actual target"),null);
  }
});
check("Ranking PlayerInspect independently freezes actual raw carriers added stats and socket null holes",()=>{
  const input=rankingInspectPayload(),before=JSON.parse(JSON.stringify(input.equipment[0]));
  const info=rankingInspectAdapter.parseRankingPlayerInspect(input,"Actual target"),item=info.equipment[0];
  assert.deepEqual(item.raw,before);assert.deepEqual(item.tooltipSource,before.tooltipSource);assert.deepEqual(item.tooltipSource.userItem.slots,[null]);
  assert(Object.isFrozen(item.raw));assert(Object.isFrozen(item.raw.added_stats[0]));assert(Object.isFrozen(item.tooltipSource.userItem.slots));
  input.equipment[0].added_stats[0].value=99;input.equipment[0].tooltipSource.userItem.slots[0]={invented:true};
  assert.deepEqual(item.raw,before);assert.throws(()=>{item.raw.added_stats[0].value=99;},TypeError);
});
check("Ranking PlayerInspect keeps actual Gateway concrete stack icon distinct from catalogue image",()=>{
  const input=rankingInspectPayload(),row=input.equipment[0];
  row.count=50;row.tooltipSource.userItem.count=50;
  Object.assign(row.tooltipSource.info,{item_type:8,shape:1,image:11});row.icon=3674;
  const info=rankingInspectAdapter.parseRankingPlayerInspect(input,"Actual target");assert(info);
  assert.equal(info.equipment[0].icon,3674);assert.equal(info.equipment[0].tooltipSource.info.image,11);
});
check("Ranking PlayerInspect duplicate root or socket UID identities reject",()=>{
  const duplicate=rankingInspectPayload();duplicate.equipment[13]=rankingInspectRawRow(0);
  assert.equal(rankingInspectAdapter.parseRankingPlayerInspect(duplicate,"Actual target"),null);
  const input=rankingInspectPayload(),row=input.equipment[0],child=rankingInspectRawRow(42).tooltipSource.userItem;
  row.slots=[child];row.tooltipSource.userItem.slots=JSON.parse(JSON.stringify(row.slots));
  row.tooltipSource.socketInfos=[JSON.parse(JSON.stringify(row.tooltipSource.info))];
  assert.equal(rankingInspectAdapter.parseRankingPlayerInspect(input,"Actual target"),null);
});
check("Ranking PlayerInspect descriptor getters and unsafe source date facts never synthesize values",()=>{
  let calls=0;const input=rankingInspectPayload();Object.defineProperty(input,"name",{enumerable:true,get(){calls++;return "Actual target";}});
  assert.equal(rankingInspectAdapter.parseRankingPlayerInspect(input,"Actual target"),null);assert.equal(calls,0);
  const date=rankingInspectPayload();date.equipment[0].expire_info={expiry_binary_datetime:Number.MAX_SAFE_INTEGER+1};
  date.equipment[0].tooltipSource.userItem.expire_info={...date.equipment[0].expire_info};
  assert.equal(rankingInspectAdapter.parseRankingPlayerInspect(date,"Actual target"),null);
});

function rankingInspectComponentFixture() {
  const source=readFileSync(new URL("../app/components/original-client-inspect-window.tsx",import.meta.url),"utf8");
  const compiled=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022,jsx:ts.JsxEmit.ReactJSX}}).outputText;
  const originalUi=loadTypeScriptModule(new URL("../lib/original-ui.ts",import.meta.url));
  const jsx=(type,props)=>({type,props}),events=[],reads=[],exports={};
  const requires={"react":{useMemo:fn=>fn()},"react/jsx-runtime":{jsx,jsxs:jsx,Fragment:"Fragment"},
    "../../lib/original-ui":originalUi,"./original-client-inventory-utils":{originalItemIconPath:icon=>"memory-icon:"+icon},
    "./original-client-crystal-item-tooltip":{OriginalCrystalItemTooltip:"SharedTooltip"},
    "./original-client-item-tooltip":{OriginalItemTooltip:"BasicTooltip"},"./original-client-overlays":{SpriteButton:"CloseButton"},
    "./original-client-panels":{useActiveItemTooltip(items,reader){return {activate:(item,node)=>events.push({kind:"activate",item,node}),
      release:(item,node)=>events.push({kind:"release",item,node}),document(item){if(!reader)return null;reads.push(item);return reader(item);}};}}};
  new Function("exports","require",compiled)(exports,name=>{assert(Object.hasOwn(requires,name),name);return requires[name];});
  return {render:props=>exports.OriginalInspectWindow(props),events,reads};
}
check("Ranking actual Inspect component paints fourteen protocol indexes actual target and Original assets",()=>{
  const f=rankingInspectComponentFixture(),info=rankingInspectAdapter.parseRankingPlayerInspect(rankingInspectPayload(),"Actual target");
  const tree=f.render({t:(_key,_args,fallback)=>fallback,info,onClose:()=>{}}),nodes=pearlDomElements(tree);
  const slots=nodes.filter(node=>Object.hasOwn(node.props,"data-inspect-slot"));assert.equal(slots.length,14);
  assert.deepEqual(slots.map(node=>node.props["data-inspect-slot"]),[0,1,2,13,4,3,5,6,7,8,9,11,10,12]);
  assert.equal(slots.filter(node=>node.props.children!==null).length,2);
  assert.equal(slots[0].props.children.props["aria-label"],info.equipment[0].name);
  assert.equal(nodes.filter(node=>node.type==="img"&&node.props.src==="/original-ui/Prguse/430.png").length,1);
  assert.equal(nodes.filter(node=>node.type==="img"&&node.props.src==="/original-ui/Prguse/341.png").length,1);
  assert.equal(nodes.find(node=>node.props.className==="character-name").props.children,"Actual target");
  assert.equal(nodes.find(node=>Object.hasOwn(node.props,"data-inspect-guild")).props.children,"Target guild");
  assert.equal(nodes.find(node=>Object.hasOwn(node.props,"data-inspect-rank")).props.children,"Officer");
  assert(nodes.some(node=>Object.hasOwn(node.props,"data-inspect-lover")));
  assert.equal(tree.props["data-inspect-gender"],"Female");assert.equal(tree.props["data-inspect-hair"],7);
});
check("Ranking actual Inspect component shares actual carriers through tooltip reader and exposes no item action",()=>{
  const f=rankingInspectComponentFixture(),info=rankingInspectAdapter.parseRankingPlayerInspect(rankingInspectPayload(),"Actual target");
  let closed=0;const document={sections:[],broken:false,sourceComplete:true};
  const tree=f.render({t:(_key,_args,fallback)=>fallback,info,onClose:()=>closed++,onReadItemTooltip:item=>{
    assert.strictEqual(item,info.equipment[item.slotIndex]);return document;}}),nodes=pearlDomElements(tree);
  assert.deepEqual(f.reads,[info.equipment[0],info.equipment[13]]);
  const buttons=nodes.filter(node=>node.type==="button");assert.equal(buttons.length,2);
  for(const button of buttons){assert.equal(button.props.onClick,undefined);assert.equal(button.props.onDoubleClick,undefined);
    for(const event of ["onPointerEnter","onPointerLeave","onFocus","onBlur","onPointerCancel","onPointerDown"])assert.equal(typeof button.props[event],"function",event);}
  assert.equal(nodes.filter(node=>node.type==="SharedTooltip"&&node.props.document===document).length,2);
  const node={},button=buttons[0];button.props.onPointerEnter({pointerType:"mouse",currentTarget:node});
  button.props.onPointerCancel({currentTarget:node});assert.deepEqual(f.events.map(event=>event.kind),["activate","release"]);
  assert.strictEqual(f.events[0].item,info.equipment[0]);nodes.find(node=>node.type==="CloseButton").props.onClick();assert.equal(closed,1);
});
check("Ranking actual ExtraWindows forwards onInspect unchanged and guards empty or closed inspect info",()=>{
  const source=readFileSync(new URL("../app/components/original-client-extra-windows.tsx",import.meta.url),"utf8");
  const ast=ts.createSourceFile("ranking-extra-windows.tsx",source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  const ranking=[],inspect=[];
  (function visit(node){if(ts.isJsxSelfClosingElement(node)){
    if(node.tagName.getText(ast)==="RankingWindow")ranking.push(node);
    if(node.tagName.getText(ast)==="OriginalInspectWindow")inspect.push(node);
  }ts.forEachChild(node,visit);})(ast);
  assert.equal(ranking.length,1);assert.equal(inspect.length,1);
  const attr=ranking[0].attributes.properties.find(node=>ts.isJsxAttribute(node)&&node.name.text==="onInspect");
  assert.equal(attr.initializer.expression.getText(ast),"ranking.onInspect");
  let expression=inspect[0].parent;while(expression&&!ts.isConditionalExpression(expression))expression=expression.parent;
  assert(expression);const guard=expression.condition;
  const js=ts.transpileModule("return "+guard.getText(ast)+";",{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
  const admitted=new Function("inspect",js);assert.equal(admitted(undefined),undefined);
  assert.equal(admitted({open:false,info:{}}),false);assert.equal(admitted({open:true,info:null}),null);
  const info={};assert.strictEqual(admitted({open:true,info}),info);
});

// Source25 actual request controller and bounded Page statements. All transports
// below are inert JS sinks; fixed admission replies do not implement Rust policy.
const rankingRequestsModule=loadTypeScriptModule(new URL("../lib/ranking-inspect-requests.ts",import.meta.url));
function rankingRequestFixture() {
  const controller=new rankingRequestsModule.RankingInspectRequests(),calls=[];
  const owner={socket:{},connectionGeneration:4,sessionGeneration:8,sceneRevision:3,playerObjectId:17,mapFileName:"0"};
  let reply={version:1,ok:true,objectId:0,nextReadyMs:1501},edge=null;
  const module={ranking_inspect_abi_version:()=>1,ranking_inspect_admission(json){calls.push(JSON.parse(json));edge?.();return JSON.stringify(reply);}};
  const core={readRankingInspectAdmission(input){return presentationRuntime.readSharedRankingInspectAdmission(module,input);}};
  let source={owner,core,readAdmission:core.readRankingInspectAdmission,page:{rankType:0,rankIndex:0,onlineOnly:false,
    entries:[{playerId:0,name:"Actual target"},{playerId:7,name:"Other target"}]},query:{rankType:0,rankIndex:0,onlineOnly:false},opened:true,pending:false};
  const read=()=>source,wire={type:"inspect",objectId:0,ranking:true,hero:false};
  return {controller,calls,module,core,owner,wire,read,set:delta=>{source={...source,...delta};return source;},
    setReply:value=>{reply=value;},setEdge:value=>{edge=value;},reserve:(id=0,name="Actual target",now=1001)=>controller.reserve(read,id,name,now),
    claim:(proof,command=wire,now=1001)=>controller.claim(proof,read(),command,now),
    info:()=>rankingInspectAdapter.parseRankingPlayerInspect(rankingInspectPayload(),"Actual target")};
}
check("Ranking actual request controller reserves unique raw row ID0 and calls exact seven-field shared admission",()=>{
  const f=rankingRequestFixture(),proof=f.reserve();assert(proof);assert(Object.isFrozen(proof));assert(Object.isFrozen(proof.owner));
  assert.equal(proof.objectId,0);assert.equal(proof.expectedName,"Actual target");assert.strictEqual(proof.owner.socket,f.owner.socket);
  assert.deepEqual(f.calls,[{version:1,opened:true,rankingsReady:true,pending:false,playerId:0,nowMs:1001,nextReadyMs:0}]);
  assert.equal(f.controller.pending,true);
});
check("Ranking actual request controller missing mismatched or duplicated raw ID rows never invoke admission",()=>{
  for(const entries of [[{playerId:7,name:"Other target"}],[{playerId:0,name:"Other target"}],
    [{playerId:0,name:"Actual target"},{playerId:0,name:"Other target"}],
    [{playerId:0,name:"Actual target"},{playerId:0,name:"Actual target"}]]){
    const f=rankingRequestFixture();f.set({page:{...f.read().page,entries}});assert.equal(f.reserve(),null);assert.equal(f.calls.length,0);
  }
  const f=rankingRequestFixture();assert.equal(f.reserve(99),null);assert.equal(f.reserve(0,""),null);assert.equal(f.calls.length,0);
});
check("Ranking actual request controller delegates opened pending and matching-page facts without a JS readiness planner",()=>{
  for(const [delta,flags] of [[{opened:false},{opened:false,rankingsReady:true,pending:false}],
    [{pending:true},{opened:true,rankingsReady:true,pending:true}],
    [{query:{rankType:1,rankIndex:0,onlineOnly:false}},{opened:true,rankingsReady:false,pending:false}]]){
    const f=rankingRequestFixture();f.set(delta);f.setReply({version:1,ok:false});assert.equal(f.reserve(),null);
    assert.equal(f.calls.length,1);assert.deepEqual(f.calls[0],{version:1,...flags,playerId:0,nowMs:1001,nextReadyMs:0});
    assert.equal(f.controller.pending,false);
  }
});
check("Ranking actual request controller refuses missing malformed or foreign checked admission plans",()=>{
  for(const reply of [null,{version:1,ok:false},{version:1,ok:true,objectId:7,nextReadyMs:1501},
    {version:1,ok:true,objectId:0,nextReadyMs:Number.MAX_SAFE_INTEGER+1},{version:1,ok:true,objectId:0,nextReadyMs:-1}]){
    const f=rankingRequestFixture();f.setReply(reply);assert.equal(f.reserve(),null);assert.equal(f.controller.pending,false);
  }
});
check("Ranking actual request controller proofs cannot be copied changed or consumed twice",()=>{
  const f=rankingRequestFixture(),proof=f.reserve();assert(proof);
  assert.equal(f.claim({...proof}),false);
  for(const delta of [{objectId:7},{ranking:false},{hero:true},{extra:true},{type:"getRanking"}])assert.equal(f.claim(proof,{...f.wire,...delta}),false);
  assert.equal(f.claim(proof),true);assert.equal(f.claim(proof),false);
  f.controller.cancelDefinitelyUnsent(proof);assert.equal(f.controller.pending,true);
});
check("Ranking actual request controller getter source changes and recursive reservations cannot publish stale proofs",()=>{
  for(const delta of [{opened:false},{pending:true},{core:{}},{readAdmission:()=>null}]){
    const f=rankingRequestFixture();f.setEdge(()=>f.set(delta));assert.equal(f.reserve(),null);assert.equal(f.controller.pending,false);
  }
  const f=rankingRequestFixture();let recursive="unset";f.setEdge(()=>{recursive=f.reserve();});
  assert(f.reserve());assert.equal(recursive,null);assert.equal(f.calls.length,1);
});
check("Ranking actual request controller final claim rejects physical owner Core and method replacement",()=>{
  for(const change of [f=>({owner:{...f.owner,socket:{}}}),f=>({owner:{...f.owner,connectionGeneration:5}}),
    f=>({owner:{...f.owner,sessionGeneration:9}}),f=>({owner:{...f.owner,sceneRevision:4}}),
    f=>({owner:{...f.owner,mapFileName:"1"}}),()=>({core:{}}),()=>({readAdmission:()=>null})]){
    const f=rankingRequestFixture(),proof=f.reserve();f.set(change(f));assert.equal(f.claim(proof),false);
  }
});
check("Ranking actual request controller observed close reopen and page ABA cannot resurrect queued proofs",()=>{
  for(const kind of ["window","page","query"]){
    const f=rankingRequestFixture(),original=f.read(),proof=f.reserve();
    f.set(kind==="window"?{opened:false}:kind==="page"?{page:{...original.page}}:{query:{...original.query,rankIndex:1}});
    f.controller.observe(f.read());f.set(original);f.controller.observe(f.read());assert.equal(f.claim(proof),false,kind);
  }
});
check("Ranking actual request controller clock rollback invalidates queued proof without replay",()=>{
  const f=rankingRequestFixture(),proof=f.reserve();assert.equal(f.claim(proof,f.wire,1000),false);assert.equal(f.controller.pending,false);
  assert.equal(f.claim(proof,f.wire,1001),false);
});
check("Ranking actual request controller entered flight survives cancellation time rollback and a new query until actual reply",()=>{
  const f=rankingRequestFixture(),proof=f.reserve();assert.equal(f.claim(proof),true);f.controller.cancelDefinitelyUnsent(proof);
  assert.equal(f.reserve(0,"Actual target",100000),null);assert.equal(f.controller.pending,true);
  f.set({query:{rankType:1,rankIndex:0,onlineOnly:false}});f.controller.observe(f.read());assert.equal(f.controller.pending,true);
  assert.equal(f.reserve(0,"Actual target",99999),null);assert.equal(f.controller.pending,true);
  assert.equal(f.controller.receive(f.info(),f.read()),false);assert.equal(f.controller.pending,false);
});
check("Ranking actual request controller closed late valid reply drains without display or automatic retry",()=>{
  const f=rankingRequestFixture(),proof=f.reserve();f.claim(proof);f.controller.closeDisplay();
  assert.equal(f.controller.pending,true);assert.equal(f.controller.expectedName(f.owner),"Actual target");
  assert.equal(f.controller.receive(f.info(),f.read()),false);assert.equal(f.controller.pending,false);assert.equal(f.calls.length,1);
  assert.equal(f.claim(proof),false);
});
check("Ranking actual request controller wrong-name Hero and oldsocket lookup do not release entered flight",()=>{
  const f=rankingRequestFixture(),proof=f.reserve();f.claim(proof);
  assert.equal(f.controller.receive({...f.info(),name:"Requester"},f.read()),false);assert.equal(f.controller.pending,true);
  assert.equal(f.controller.receive({...f.info(),isHero:true},f.read()),false);assert.equal(f.controller.pending,true);
  assert.equal(f.controller.expectedName({...f.owner,socket:{}}),null);assert.equal(f.controller.pending,true);
  assert.equal(f.controller.receive(f.info(),f.read()),true);assert.equal(f.controller.pending,false);
});
check("Ranking actual request controller only physical replacement or explicit retirement clears entered unknown",()=>{
  for(const kind of ["newOwner","retire"]){const f=rankingRequestFixture(),proof=f.reserve();f.claim(proof);
    if(kind==="newOwner"){f.set({owner:{...f.owner,socket:{},connectionGeneration:5}});f.controller.observe(f.read());}
    else f.controller.retireConnection();
    assert.equal(f.controller.pending,false);assert.equal(f.claim(proof),false);
  }
});
check("Ranking legacy name correlation carries no nonce and admits only one entered flight",()=>{
  const f=rankingRequestFixture(),proof=f.reserve();assert.deepEqual(Object.keys(proof).sort(),["expectedName","objectId","owner"]);
  f.claim(proof);assert.equal(f.reserve(),null);assert.equal(f.calls.length,1);
  // A same-name reply is the legacy protocol correlation. It is not proof of a
  // fresh request nonce, independently owned wire ACK, or unique reply epoch.
  assert.equal(f.controller.receive(f.info(),f.read()),true);
});

const rankingPageFunctions=new Map();let rankingPlayerInspectCase;
(function visit(node){if(ts.isFunctionDeclaration(node)&&node.name&&["currentRankingInspectSource","inspectRankingPlayer","closeRankingInspect",
  "readRankingInspectTooltip","applyRankingPacket","sameSocialPhysicalOwner"].includes(node.name.text))rankingPageFunctions.set(node.name.text,node);
  if(ts.isCaseClause(node)&&ts.isStringLiteral(node.expression)&&node.expression.text==="PlayerInspect")rankingPlayerInspectCase=node;
  ts.forEachChild(node,visit);})(parityPageAst);
assert.equal(rankingPageFunctions.size,6);assert(rankingPlayerInspectCase);
const rankingRawGuard=paritySendStatements.find(node=>ts.isIfStatement(node)&&node.expression.getText(parityPageAst).includes('command.type === "inspect"'));
const rankingFinalGuard=paritySendStatements.find(node=>ts.isIfStatement(node)&&node.expression.getText(parityPageAst).includes('wireCommand.type === "inspect"'));
assert(rankingRawGuard&&rankingFinalGuard);
const rankingPageFixtureJs=ts.transpileModule([...rankingPageFunctions.values()].map(node=>node.getText(parityPageAst)).join("\n")+
  "\nfunction receiveInspect(payload){switch('PlayerInspect'){"+rankingPlayerInspectCase.getText(parityPageAst)+"}}\n"+
  "function finalRawSend(command,options){"+rankingRawGuard.getText(parityPageAst)+
  "\nconst socket=socketRef.current,serialized=JSON.stringify(command),wireCommand=JSON.parse(serialized);beforeFinal?.();\n"+
  rankingFinalGuard.getText(parityPageAst)+"\n"+paritySendStatements[paritySocketIndex].getText(parityPageAst)+"\nreturn true;}"+
  "\nreturn {currentRankingInspectSource,inspectRankingPlayer,closeRankingInspect,readRankingInspectTooltip,applyRankingPacket,receiveInspect,finalRawSend};",
  {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
function rankingPageFixture() {
  const f=rankingRequestFixture(),received=[],sent=[],tooltips=[],ref=current=>({current});let api,clock=1001,beforeFinal=null,throwSend=false,spectator=false,owner=f.owner;
  const socket={readyState:1,send(body){sent.push(JSON.parse(body));if(throwSend)throw Error("mock unknown send");}};owner={...owner,socket};
  const core={...f.core,readItemTooltip(item,viewer,now){tooltips.push({item,viewer,now});return tooltipDocumentFixture;}};
  const query={rankType:0,rankIndex:0,onlineOnly:false},page=f.read().page,world={rankings:{"0:false":page},playerObjectId:17,
    entities:[{objectId:17,level:33,classKey:"Wizard",genderKey:"Male"}],playerCrystalStats:[{stat:4,value:7}],playerWeights:[1,2,3],currentWeight:2,maxWeight:9};
  const scope={rankingInspectRequestsRef:ref(f.controller),rankingInspectBindingRef:ref(null),rankingInspectOpenRef:ref(false),
    questCoreRuntimeRef:ref(core),rankingQueriesRef:ref({desired:query,pending:false,receive(type,actualOwner){received.push({type,owner:actualOwner});return query;},wantsAnother:()=>false}),
    rankingWindowRef:ref(true),worldRef:ref(world),socketRef:ref(socket),WebSocket:{OPEN:1},document:{visibilityState:"visible"},performance:{now:()=>clock},
    isSpectatorBrowserMode:()=>spectator,currentSocialReplyOwner:()=>owner,currentSocialReceiveOwner:()=>owner,
    rankingPageKey:(type,online)=>`${type}:${online}`,setRankingInspectInfo:value=>{scope.display=value;},cancelPlayerUiWorldIntent:()=>{},
    parseRankingPlayerInspect:rankingInspectAdapter.parseRankingPlayerInspect,console:{error:()=>{}},Date:{now:()=>1234567},
    updateWorld:fn=>{scope.worldRef.current=fn(scope.worldRef.current);},numberOrUndefined:value=>typeof value==="number"?value:undefined,
    mapClassKey:value=>value,renderRankingRequests:()=>{},issueRankingRequest:()=>{},get beforeFinal(){return beforeFinal;},
    send:(command,options)=>api.finalRawSend(command,options)};
  api=new Function("scope","with(scope){"+rankingPageFixtureJs+"}")(scope);
  return {f,scope,api,core,owner,socket,sent,received,tooltips,setClock:value=>{clock=value;},setBeforeFinal:value=>{beforeFinal=value;},
    setOwner:value=>{owner=value;},setSpectator:value=>{spectator=value;},setThrowSend:value=>{throwSend=value;}};
}
check("Ranking actual Core facade forwards identical input to optional presentation ABI without planning",()=>{
  const source=readFileSync(new URL("../lib/client-core-runtime.ts",import.meta.url),"utf8"),ast=ts.createSourceFile("ranking-core.ts",source,ts.ScriptTarget.Latest,true),methods=[];
  (function visit(node){if(ts.isMethodDeclaration(node)&&node.name.getText(ast)==="readRankingInspectAdmission")methods.push(node);ts.forEachChild(node,visit);})(ast);
  assert.equal(methods.length,1);const input=rankingAdmissionInput(),module=rankingAdmissionModule();
  const js=ts.transpileModule("return {"+methods[0].getText(ast)+"};",{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
  const facade=new Function("presentation","readSharedRankingInspectAdmission",js)(module.module,presentationRuntime.readSharedRankingInspectAdmission);
  assert.deepEqual(facade.readRankingInspectAdmission(input),{version:1,ok:true,objectId:0,nextReadyMs:1501});assert.deepEqual(module.calls,[input]);
});
check("Ranking actual Page source captures current Core method physical owner query page and known UI readiness",()=>{
  const page=rankingPageFixture(),source=page.api.currentRankingInspectSource();
  assert.strictEqual(source.core,page.core);assert.strictEqual(source.readAdmission,page.core.readRankingInspectAdmission);
  assert.strictEqual(source.owner,page.owner);assert.strictEqual(source.page,page.scope.worldRef.current.rankings["0:false"]);
  page.scope.document.visibilityState="hidden";assert.equal(page.api.currentRankingInspectSource().opened,false);
  page.setSpectator(true);assert.equal(page.api.currentRankingInspectSource().owner,null);
});
check("Ranking actual Page inspect gesture sends exact four raw fields UID0 through one final proof claim",()=>{
  const page=rankingPageFixture();page.api.inspectRankingPlayer(0,"Actual target");
  assert.deepEqual(page.sent,[{type:"inspect",objectId:0,ranking:true,hero:false}]);assert.equal(page.f.controller.pending,true);
  page.api.inspectRankingPlayer(0,"Actual target");assert.equal(page.sent.length,1);
});
check("Ranking actual Page raw Inspect requires proof and final claim directly precedes actual socket send",()=>{
  const page=rankingPageFixture();assert.equal(page.api.finalRawSend({type:"inspect",objectId:0,ranking:true,hero:false}),false);
  const finalIndex=paritySendStatements.indexOf(rankingFinalGuard);assert.equal(finalIndex+1,paritySocketIndex);
  const sends=[];(function visit(node){if(ts.isCallExpression(node)&&node.expression.getText(parityPageAst)==="socket.send")sends.push(node);ts.forEachChild(node,visit);})(paritySendStatements[paritySocketIndex]);
  assert.equal(sends.length,1);assert.equal(sends[0].arguments[0].getText(parityPageAst),"serialized");
  const listeners=paritySendStatements.map((node,index)=>({node,index})).filter(({node})=>node.getText(parityPageAst).includes("window.dispatchEvent"));
  assert(listeners.length>0);assert(listeners.every(({index})=>index<finalIndex));assert.equal(page.sent.length,0);
});
check("Ranking actual Page final callback owner Core and admission method changes block inspect sink",()=>{
  for(const change of [page=>page.setOwner({...page.owner,sessionGeneration:9}),page=>{page.scope.questCoreRuntimeRef.current={...page.core};},
    page=>{page.core.readRankingInspectAdmission=()=>null;},page=>{page.scope.rankingWindowRef.current=false;}]){
    const page=rankingPageFixture();page.setBeforeFinal(()=>change(page));page.api.inspectRankingPlayer(0,"Actual target");assert.equal(page.sent.length,0);
  }
});
check("Ranking actual Page unknown socket outcome retains entered flight and does not resend",()=>{
  const page=rankingPageFixture();page.setThrowSend(true);page.api.inspectRankingPlayer(0,"Actual target");assert.equal(page.sent.length,1);
  assert.equal(page.f.controller.pending,true);page.setThrowSend(false);page.setClock(99999);page.api.inspectRankingPlayer(0,"Actual target");
  assert.equal(page.sent.length,1);assert.equal(page.f.controller.pending,true);
});
check("Ranking actual Page receiver rejects wrong target Hero and oldsocket before draining actual pending",()=>{
  const page=rankingPageFixture();page.api.inspectRankingPlayer(0,"Actual target");
  for(const info of [rankingInspectPayload({name:"Requester"}),rankingInspectPayload({isHero:true})]){
    page.api.receiveInspect({info});assert.equal(page.f.controller.pending,true);assert.equal(page.scope.rankingInspectOpenRef.current,false);
  }
  page.setOwner({...page.owner,socket:{}});page.api.receiveInspect({info:rankingInspectPayload()});assert.equal(page.f.controller.pending,true);
  page.setOwner(page.owner);page.api.receiveInspect({info:rankingInspectPayload()});assert.equal(page.f.controller.pending,false);
  assert.equal(page.scope.rankingInspectOpenRef.current,true);assert.equal(page.scope.display.name,"Actual target");
});
check("Ranking actual Page closed late valid PlayerInspect drains without mounting a window",()=>{
  const page=rankingPageFixture();page.api.inspectRankingPlayer(0,"Actual target");page.api.closeRankingInspect();
  page.api.receiveInspect({info:rankingInspectPayload()});assert.equal(page.f.controller.pending,false);
  assert.equal(page.scope.rankingInspectOpenRef.current,false);assert.equal(page.scope.rankingInspectBindingRef.current,null);assert.equal(page.scope.display,null);
});
check("Ranking actual applyRankingPacket rejects raw missing malformed ID or name before query receive",()=>{
  for(const entry of [{name:"Actual target"},{playerId:null,name:"Actual target"},{playerId:"0",name:"Actual target"},
    {playerId:-1,name:"Actual target"},{playerId:2147483648,name:"Actual target"},{playerId:0.5,name:"Actual target"},
    {playerId:0},{playerId:0,name:""},{playerId:0,name:"bad\0name"},{playerId:0,name:7}]){
    const page=rankingPageFixture();page.api.applyRankingPacket({rankType:0,count:1,listingDetails:[entry]});assert.equal(page.received.length,0);
  }
  const page=rankingPageFixture();page.api.applyRankingPacket({rankType:0,count:1,listingDetails:[{playerId:0,name:"Actual target",level:45,class:"Archer"}]});
  assert.equal(page.received.length,1);assert.equal(page.scope.worldRef.current.rankings["0:false"].entries[0].playerId,0);
  assert.equal(page.scope.worldRef.current.rankings["0:false"].entries[0].name,"Actual target");
});
check("Ranking actual Page tooltip uses actual requester viewer and inspected carrier references without template stats",()=>{
  const page=rankingPageFixture();page.api.inspectRankingPlayer(0,"Actual target");page.api.receiveInspect({info:rankingInspectPayload()});
  const info=page.scope.display,item=info.equipment[0];assert.strictEqual(page.api.readRankingInspectTooltip(item),tooltipDocumentFixture);
  assert.strictEqual(page.tooltips[0].item,item);assert.deepEqual(page.tooltips[0].viewer,{level:33,className:"Wizard",gender:"Male",
    crystalStats:[{stat:4,value:7}],weights:[1,2,3],currentWeightKnown:true,currentWeight:2,maxWeight:9});
  assert.equal(page.tooltips[0].now,1234567);assert.equal(page.api.readRankingInspectTooltip({...item}),null);
});
check("Ranking actual Page tooltip physical session current Core method and equipment reference fences reject stale readers",()=>{
  for(const change of [page=>page.setOwner({...page.owner,socket:{}}),page=>page.setOwner({...page.owner,sessionGeneration:9}),
    page=>page.setOwner({...page.owner,sceneRevision:4}),page=>{page.scope.questCoreRuntimeRef.current={...page.core};},
    page=>{page.core.readItemTooltip=()=>tooltipDocumentFixture;},page=>{page.scope.rankingInspectOpenRef.current=false;}]){
    const page=rankingPageFixture();page.api.inspectRankingPlayer(0,"Actual target");page.api.receiveInspect({info:rankingInspectPayload()});
    const item=page.scope.display.equipment[0];change(page);assert.equal(page.api.readRankingInspectTooltip(item),null);assert.equal(page.tooltips.length,0);
  }
  const page=rankingPageFixture();page.api.inspectRankingPlayer(0,"Actual target");page.api.receiveInspect({info:rankingInspectPayload()});
  const binding=page.scope.rankingInspectBindingRef.current,item=binding.info.equipment[0];
  binding.tooltip=page.core.readItemTooltip=function(){page.setOwner({...page.owner,sessionGeneration:9});return tooltipDocumentFixture;};
  assert.equal(page.api.readRankingInspectTooltip(item),null);
});
check("Ranking actual Page binds Ranking onInspect receiver and readonly fourteen-position Inspect window",()=>{
  const extra=[];(function visit(node){if(ts.isJsxSelfClosingElement(node)&&node.tagName.getText(parityPageAst)==="ExtraWindows")extra.push(node);ts.forEachChild(node,visit);})(parityPageAst);
  assert.equal(extra.length,1);const attrs=new Map(extra[0].attributes.properties.filter(ts.isJsxAttribute).map(node=>[node.name.text,node.initializer?.expression]));
  assert(attrs.get("ranking").getText(parityPageAst).includes("onInspect:inspectRankingPlayer"));
  assert(attrs.get("inspect").getText(parityPageAst).includes("info:rankingInspectInfo"));
  assert(attrs.get("inspect").getText(parityPageAst).includes("onReadItemTooltip:readRankingInspectTooltip"));
  const inspected=rankingInspectAdapter.parseRankingPlayerInspect(rankingInspectPayload(),"Actual target"),f=rankingInspectComponentFixture();
  const tree=f.render({t:(_key,_args,fallback)=>fallback,info:inspected,onClose:()=>{}}),nodes=pearlDomElements(tree);
  assert.equal(nodes.filter(node=>Object.hasOwn(node.props,"data-inspect-slot")).length,14);
  assert(nodes.filter(node=>node.type==="button").every(node=>!node.props.onClick&&!node.props.onDoubleClick));
});

check("Ranking active Inspect blocks actual older Hero and Fishing Page paths through shared modal ref",()=>{
  const hero=heroPageFixture();assert.equal(hero.api.parityIngress({...hero.proof.wire},{heroProof:hero.proof}),true);
  hero.scope.rankingInspectOpenRef.current=true;
  assert.equal(hero.api.referenceWindowsBlockGameplay(),true);
  assert.equal(hero.api.heroInputAllowed(),false);assert.equal(hero.api.heroProofCurrent(hero.proof,hero.proof.wire),false);
  assert.equal(hero.api.parityFinal({...hero.proof.wire},{heroProof:hero.proof},hero.socket),false);
  assert.equal(hero.sent.length,0);assert.equal(hero.scope.heroOperationsRef.current.pending.state,"reserved");
  const fishing=worldFishingPageFixture();assert.equal(fishing.scope.rankingInspectOpenRef.current,false);
  fishing.scope.rankingInspectOpenRef.current=true;
  assert.equal(fishing.api.worldFishingPureUiBlocked(),true);
  assert.equal(fishing.api.beginWorldFishingGesture(fishing.pointer),null);assert.equal(fishing.sent.length,0);assert.equal(fishing.requests.length,0);
});


// Source41 exercises the Page-owned custody with finite runtime ports, never WASM.
const heroIngressDocument=loadTypeScriptModule(new URL("../lib/bevy-hero-ui.ts",import.meta.url),{"./hero-player-ui":heroUi});
function heroRawFixture({bound=true,checkpoint=true}={}){
  const physical={socket:{},connectionGeneration:1,sessionGeneration:2};
  const scene={sceneRevision:1,playerObjectId:1,mapFileName:"D000"};
  const ingress=new heroIngressDocument.HeroRawIngress(),checkpoints=new Map(),trace=[];
  let serial=0,now=10;
  function renderer({checkpointAvailable=checkpoint}={}){
    const calls=[],options={checkpointAvailable,throwPush:false,rejectPush:false},state={scope:null,cursor:0,hero:false,firstClock:null};
    const runtime={getMir2HeroUiAbiVersion:()=>1,
      activateMir2HeroIngress(raw){state.scope=JSON.parse(raw);calls.push({kind:"activate",scope:state.scope});return true;},
      pushMir2HeroRawFrame(scope,sequence,raw,receivedAtMs){return push("raw",scope,sequence,raw,receivedAtMs,null);},
      pushMir2HeroVerifiedOwnerFrame(scope,sequence,raw,receivedAtMs,expected){return push("verified",scope,sequence,raw,receivedAtMs,expected);},
      withdrawMir2HeroIngress(scope){calls.push({kind:"withdraw",scope});return true;},
      getMir2HeroIngressCheckpoint(){if(!options.checkpointAvailable)return null;
        const raw='{ "opaque" : 18446744073709551615, "cursor" : '+state.cursor+', "serial" : '+(++serial)+' }';
        checkpoints.set(raw,{cursor:state.cursor,hero:state.hero,firstClock:state.firstClock});calls.push({kind:"checkpoint",raw});return raw;},
      restoreMir2HeroIngressCheckpoint(scope,raw){calls.push({kind:"restore",scope:JSON.parse(scope),raw});
        const held=checkpoints.get(raw);assert(held,"original held string must exist");Object.assign(state,held);return true;},
      getMir2HeroUiStatus(){return JSON.stringify({version:1,source:state.hero?{scope:state.scope,frameSequence:state.cursor,
        heroObjectId:2,heroGeneration:1,rustModelRevision:1}:null,acceptedFrameSequence:state.cursor,closed:false,
        control:null,ready:false,frameSequence:0});}};
    function push(kind,scope,sequence,raw,receivedAtMs,expected){
      calls.push({kind,scope:JSON.parse(scope),sequence,raw,receivedAtMs,expected});trace.push({kind,sequence,raw,receivedAtMs});
      if(options.throwPush)throw Error("controlled renderer failure");if(options.rejectPush)return false;
      assert(sequence>state.cursor,"a restored receiver must not get its prefix twice");state.cursor=sequence;
      if(raw.includes("HeroInformation")){state.hero=true;state.firstClock??=receivedAtMs;}return true;}
    return {runtime,calls,options,state};
  }
  const first=renderer();ingress.sync(bound?first.runtime:null,physical,bound?scene:null);
  function capture(raw,at=now++,identity=physical){const delivery=ingress.capture(raw,identity,at);assert(delivery);return delivery;}
  function deliver(raw,expected=null){const delivery=capture(raw);assert(ingress.offer(delivery,expected));return delivery;}
  return {physical,scene,ingress,renderer,first,trace,capture,deliver};
}
const heroOpaque={requestId:"18446744073709551615",actor:"a".repeat(64),producerScope:"b".repeat(64),serverRevision:"0"};
const heroWideOriginal=' {"type":"packet","packet":"HeroInformation","payload":{"uniqueId":18446744073709551615,"xp":9007199254740993}} ';
check("Hero ingress preserves original wide raw floating first clock and verified metadata",()=>{
  const f=heroRawFixture({checkpoint:false}),d=f.capture(heroWideOriginal,10.375);assert(f.ingress.offer(d));
  const owner=' {"type":"npcPurchaseOwner","protocolVersion":1,"requestId":"18446744073709551615","reply":{},"snapshot":{"uniqueId":18446744073709551615},"authority":{"actor":"'+heroOpaque.actor+'","producerScope":"'+heroOpaque.producerScope+'","serverRevision":"0"}} ';
  const expected=heroIngressDocument.verifiedHeroOwnerFromFrame(owner);assert.deepEqual(expected,heroOpaque);
  const receipt=f.capture(owner,10.875);assert(f.ingress.offer(receipt,expected));
  assert.equal(f.trace[0].raw,heroWideOriginal);assert.equal(f.trace[0].receivedAtMs,10.375);assert.equal(f.trace[1].raw,owner);
  assert.equal(f.trace[1].receivedAtMs,10.875);assert.deepEqual(JSON.parse(f.first.calls.find(c=>c.kind==="verified").expected),heroOpaque);
  assert(f.ingress.caughtUp());assert.equal(f.ingress.offer(receipt),false);
});
check("Hero bootstrap source can precede control but never manufactures UI Ready",()=>{
  const f=heroRawFixture({checkpoint:false});f.deliver(heroWideOriginal);const bootstrap=f.ingress.bootstrap();
  assert.equal(bootstrap.source.heroObjectId,2);assert.equal(bootstrap.acceptedFrameSequence,f.trace[0].sequence);
  assert.equal(JSON.parse(f.first.runtime.getMir2HeroUiStatus()).ready,false);assert.equal(JSON.parse(f.first.runtime.getMir2HeroUiStatus()).control,null);
  for(const bad of [{version:2,source:null,acceptedFrameSequence:1,closed:false},{version:1,source:null,acceptedFrameSequence:0.5,closed:false},
    {version:1,source:{...bootstrap.source,scope:{...bootstrap.source.scope,unknown:1}},acceptedFrameSequence:bootstrap.acceptedFrameSequence,closed:false},
    {version:1,source:{...bootstrap.source,frameSequence:bootstrap.acceptedFrameSequence+1},acceptedFrameSequence:bootstrap.acceptedFrameSequence,closed:false}]){
    assert.equal(heroIngressDocument.readHeroBootstrap({getMir2HeroUiStatus:()=>JSON.stringify(bad)}),null);}
});
check("Hero ingress captures before bootstrap without dropping ignored original frames",()=>{
  const f=heroRawFixture({bound:false,checkpoint:false});const ignored=f.capture('{"type":"notice","text":"first"}',10.25);
  assert(f.ingress.offer(ignored));const info=f.capture(heroWideOriginal,10.75);assert(f.ingress.offer(info));assert.equal(f.trace.length,0);
  f.ingress.sync(f.first.runtime,f.physical,f.scene);assert.deepEqual(f.trace.map(p=>p.sequence),[ignored.sequence,info.sequence]);
  assert.deepEqual(f.trace.map(p=>p.receivedAtMs),[10.25,10.75]);assert.equal(f.first.state.firstClock,10.75);assert(f.ingress.caughtUp());
});
check("Hero captured high water blocks until classification and delivery finish",()=>{
  const f=heroRawFixture({checkpoint:false});f.deliver(heroWideOriginal);assert(f.ingress.caughtUp());
  const d=f.capture('{"type":"notice"}');assert.equal(f.ingress.caughtUp(),false);assert(f.ingress.offer(d));assert(f.ingress.caughtUp());
});
check("Hero reentrant offer cannot overtake earlier captured raw or report caught up",()=>{
  const f=heroRawFixture({checkpoint:false}),a=f.capture(heroWideOriginal,10.125),b=f.capture('{"type":"notice"}',10.625);
  assert(f.ingress.offer(b));assert.equal(f.trace.length,0);assert.equal(f.ingress.caughtUp(),false);assert(f.ingress.offer(a));
  assert.deepEqual(f.trace.map(p=>p.sequence),[a.sequence,b.sequence]);assert.deepEqual(f.trace.map(p=>p.receivedAtMs),[10.125,10.625]);
  assert(f.ingress.caughtUp());assert.equal(f.ingress.offer(a),false);
});
check("Hero tail retains every accepted frame when renderer cannot checkpoint",()=>{
  const f=heroRawFixture({checkpoint:false}),a=f.deliver(heroWideOriginal),b=f.deliver('{"type":"notice"}');
  assert.equal(f.ingress.status().tailFrames,2);const next=f.renderer({checkpointAvailable:false});f.ingress.detachRenderer(f.first.runtime);
  f.ingress.sync(next.runtime,f.physical,f.scene);assert.equal(next.calls.filter(c=>c.kind==="restore").length,0);
  assert.deepEqual(next.calls.filter(c=>c.kind==="raw").map(c=>c.sequence),[a.sequence,b.sequence]);assert(f.ingress.caughtUp());
});
check("Hero handoff restores exact held string then each retained tail once with new run",()=>{
  const f=heroRawFixture(),a=f.deliver(heroWideOriginal);f.ingress.sync(f.first.runtime,f.physical,f.scene);
  const held=f.first.calls.filter(c=>c.kind==="checkpoint").at(-1).raw,oldRun=f.ingress.status().scope.runGeneration;
  f.first.options.checkpointAvailable=false;const b=f.deliver('{"type":"notice"}'),next=f.renderer({checkpointAvailable:false});
  f.ingress.detachRenderer(f.first.runtime);f.ingress.sync(next.runtime,f.physical,f.scene);
  assert.equal(next.calls.find(c=>c.kind==="restore").raw,held);assert(next.calls[0].scope.runGeneration>oldRun);
  assert.deepEqual(next.calls.filter(c=>c.kind==="raw").map(c=>c.sequence),[b.sequence]);assert.equal(next.state.firstClock,a.receivedAtMs);
  f.ingress.sync(next.runtime,f.physical,f.scene);assert.equal(next.calls.filter(c=>c.kind==="raw").length,1);assert(f.ingress.caughtUp());
});
check("Hero no renderer cross map restores old scene and tail before activating current scene",()=>{
  const f=heroRawFixture();f.deliver(heroWideOriginal);f.ingress.sync(f.first.runtime,f.physical,f.scene);
  f.first.options.checkpointAvailable=false;f.ingress.sync(null,f.physical,f.scene);const oldTail=f.deliver('{"type":"notice","map":"old"}');
  const changed={...f.scene,sceneRevision:2,mapFileName:"D001"};f.ingress.sync(null,f.physical,changed);
  const mapTail=f.deliver('{"type":"packet","packet":"MapChanged","payload":{"mapFileName":"D001"}}'),next=f.renderer({checkpointAvailable:false});
  f.ingress.sync(next.runtime,f.physical,changed);assert.equal(next.calls[0].scope.mapFileName,"D000");
  assert.equal(next.calls.find(c=>c.kind==="restore").scope.mapFileName,"D000");
  const raw=next.calls.filter(c=>c.kind==="raw");assert.deepEqual(raw.map(c=>c.sequence),[oldTail.sequence,mapTail.sequence]);
  assert.deepEqual(raw.map(c=>c.scope.mapFileName),["D000","D001"]);assert.equal(f.ingress.status().scope.mapFileName,"D001");assert(f.ingress.caughtUp());
});
check("Hero failed checkpoint keeps last successful original plus full newer tail",()=>{
  const f=heroRawFixture();f.deliver(heroWideOriginal);f.ingress.sync(f.first.runtime,f.physical,f.scene);
  const held=f.first.calls.filter(c=>c.kind==="checkpoint").at(-1).raw;
  f.first.runtime.getMir2HeroIngressCheckpoint=()=>{throw Error("controlled checkpoint refusal");};
  const b=f.deliver('{"type":"notice","n":1}'),c=f.deliver('{"type":"notice","n":2}');f.ingress.sync(f.first.runtime,f.physical,f.scene);
  const next=f.renderer({checkpointAvailable:false});f.ingress.sync(next.runtime,f.physical,f.scene);
  assert.equal(next.calls.find(c=>c.kind==="restore").raw,held);assert.deepEqual(next.calls.filter(c=>c.kind==="raw").map(c=>c.sequence),[b.sequence,c.sequence]);
});
check("Hero renderer refusal permits a new runtime to recover the unchanged held and tail",()=>{
  const f=heroRawFixture();f.deliver(heroWideOriginal);f.ingress.sync(f.first.runtime,f.physical,f.scene);
  f.first.options.throwPush=true;const b=f.capture('{"type":"notice","n":1}');assert(f.ingress.offer(b));assert.equal(f.ingress.status().closed,true);
  assert.equal(f.ingress.caughtUp(),false);const c=f.capture('{"type":"notice","n":2}');assert(f.ingress.offer(c));
  const next=f.renderer({checkpointAvailable:false});f.ingress.sync(next.runtime,f.physical,f.scene);
  assert.equal(f.ingress.status().closed,false);assert.deepEqual(next.calls.filter(c=>c.kind==="raw").map(c=>c.sequence),[b.sequence,c.sequence]);assert(f.ingress.caughtUp());
});
check("Hero physical session and socket replacement never restore previous private held",()=>{
  for(const change of [p=>({...p,sessionGeneration:p.sessionGeneration+1}),p=>({...p,socket:{}})]){
    const f=heroRawFixture();f.deliver(heroWideOriginal);f.ingress.sync(f.first.runtime,f.physical,f.scene);
    const physical=change(f.physical),next=f.renderer({checkpointAvailable:false});f.ingress.sync(next.runtime,physical,f.scene);
    assert.equal(next.calls.filter(c=>c.kind==="restore").length,0);assert.equal(next.calls.filter(c=>c.kind==="raw").length,0);
    assert.equal(f.ingress.offer(f.capture('{"type":"notice"}',100,physical)),true);assert.equal(next.calls.filter(c=>c.kind==="raw").length,1);
  }
});
check("Hero same runtime scene activation and stale cleanup preserve the live receiver",()=>{
  const f=heroRawFixture({checkpoint:false});f.deliver(heroWideOriginal);const changed={...f.scene,sceneRevision:2,mapFileName:"D001"};
  f.ingress.sync(f.first.runtime,f.physical,changed);assert.equal(f.first.calls.filter(c=>c.kind==="withdraw").length,0);
  const next=f.renderer({checkpointAvailable:false});f.ingress.sync(next.runtime,f.physical,changed);const before=next.calls.length;
  f.ingress.detachRenderer(f.first.runtime);assert.equal(next.calls.length,before);assert.equal(f.ingress.status().scope.mapFileName,"D001");
});
check("Hero raw custody overflow and rejection remain closed across renderer replacement",()=>{
  const f=heroRawFixture({bound:false,checkpoint:false});for(let i=0;i<4096;i++)f.capture('{"type":"notice"}',i+10);
  assert.equal(f.ingress.capture('{}',f.physical,5000),null);assert.equal(f.ingress.status().closed,true);
  const next=f.renderer({checkpointAvailable:false});f.ingress.sync(next.runtime,f.physical,f.scene);assert.equal(next.calls.filter(c=>c.kind==="raw").length,0);
  assert.equal(f.ingress.caughtUp(),false);
  const rejected=heroRawFixture({checkpoint:false}),d=rejected.capture(heroWideOriginal);rejected.ingress.reject(d);
  assert.equal(rejected.ingress.offer(d),false);assert.equal(rejected.ingress.caughtUp(),false);
});
check("Hero unsupported ABI and unsafe owner metadata cannot grant raw authority",()=>{
  const f=heroRawFixture({checkpoint:false});for(const runtime of [null,{...f.first.runtime,getMir2HeroUiAbiVersion:()=>0},
    {...f.first.runtime,pushMir2HeroVerifiedOwnerFrame:undefined},{...f.first.runtime,getMir2HeroUiAbiVersion:()=>{throw Error("ABI");}}]){
    assert.equal(heroIngressDocument.supportsHeroIngress(runtime),false);}
  for(const bad of [{...heroOpaque,requestId:"0"},{...heroOpaque,requestId:"01"},{...heroOpaque,requestId:"18446744073709551616"},
    {...heroOpaque,actor:"1"},{...heroOpaque,actor:"0".repeat(64)},{...heroOpaque,producerScope:"B".repeat(64)},
    {...heroOpaque,serverRevision:"18446744073709551615"},{...heroOpaque,unknown:true}])assert.equal(heroIngressDocument.validVerifiedHeroOwner(bad),false);
  assert.equal(heroIngressDocument.verifiedHeroOwnerFromFrame('{"type":"npcPurchaseOwner"}'),null);
});
check("Hero original Page message captures first clock before synchronous ingress work",()=>{
  const handlers=[];(function visit(node){if(ts.isCallExpression(node)&&node.expression.getText(parityPageAst)==='socket.addEventListener'
    &&node.arguments[0]?.getText(parityPageAst)==='"message"'&&node.arguments[1]?.getText(parityPageAst).includes('heroRawIngressRef.current?.capture'))handlers.push(node.arguments[1]);
    ts.forEachChild(node,visit);})(parityPageAst);assert.equal(handlers.length,1);
  const compiled=ts.transpileModule('const originalHandler='+handlers[0].getText(parityPageAst)+';',
    {compilerOptions:{module:ts.ModuleKind.None,target:ts.ScriptTarget.ES2022}}).outputText;
  const socket={},physical={socket,connectionGeneration:1,sessionGeneration:2},calls=[];let now=10.375;
  const scope={socket,socketRef:{current:socket},connectionGeneration:1,equipmentConnectionGenerationRef:{current:1},
    performance:{now(){calls.push("clock");return now;}},syncHeroRawIngress(){calls.push("sync");now=999;},
    currentHeroRawPhysical:()=>physical,heroRawIngressRef:{current:{capture(raw,p,at){calls.push({raw,p,at});return {raw,at};},reject(){throw Error("unexpected reject");}}},
    receiveNpcPurchaseGatewayFrame(event,generation,s,d){calls.push({received:d});},appendLog(){throw Error("unexpected Page log");},t:()=>"log"};
  const handler=new Function('scope','with(scope){'+compiled+'return originalHandler;}')(scope);handler({data:heroWideOriginal});
  assert.equal(calls[0],"clock");assert.equal(calls[1],"sync");assert.equal(calls[2].at,10.375);assert.equal(calls[3].received.at,10.375);
  scope.socketRef.current={};const before=calls.length;handler({data:heroWideOriginal});assert.equal(calls.length,before);
});


check("Hero nontext invalid clock and oversized source close custody before stale readiness",()=>{
  for(const [raw,clock] of [[{},100],[null,100],[new Uint8Array(1),100],["{}",NaN],["{}",Infinity],["{}",-1],["{}",Number.MAX_SAFE_INTEGER+1]]){
    const f=heroRawFixture({checkpoint:false});f.deliver(heroWideOriginal);assert(f.ingress.caughtUp());
    assert.equal(f.ingress.capture(raw,f.physical,clock),null);assert.equal(f.ingress.status().closed,true);assert.equal(f.ingress.caughtUp(),false);
  }
  const f=heroRawFixture({checkpoint:false});f.deliver(heroWideOriginal);assert.equal(f.ingress.capture("a".repeat(16*1024*1024+1),f.physical,100),null);
  assert.equal(f.ingress.status().closed,true);assert.equal(f.ingress.caughtUp(),false);
});


check("Hero cross scene reentry closes instead of relabelling earlier unfinished raw",()=>{
  for(const bound of [true,false]){
  const f=heroRawFixture({bound,checkpoint:false}),a=f.capture('{"type":"worldSnapshot","payload":{"mapFileName":"D000"}}',10.25),
    b=f.capture('{"type":"packet","packet":"MapChanged","payload":{"mapFileName":"D001"}}',10.75);
  f.ingress.sync(f.first.runtime,f.physical,{...f.scene,sceneRevision:2,mapFileName:"D001"});
  assert.equal(f.ingress.offer(b),false);assert.equal(f.ingress.offer(a),false);assert.equal(f.trace.length,0);
  assert.equal(f.ingress.status().closed,true);assert.equal(f.ingress.caughtUp(),false);
  const next=f.renderer({checkpointAvailable:false});f.ingress.sync(next.runtime,f.physical,{...f.scene,sceneRevision:2,mapFileName:"D001"});
  assert.equal(next.calls.filter(c=>c.kind==="raw").length,0);assert.equal(f.ingress.caughtUp(),false);
  }
});


const heroBasisWindows42={inventoryOpen:true,characterOpen:true,characterPage:"equipment",beltVisible:true,beltVertical:false};
check("Hero basis binds occupied destinations and exact canonical IDs independently",()=>{
  const f=heroFixture(),action={kind:"move",from:2,to:0},basis=heroUi.captureHeroActionBasis(f.model,action,heroBasisWindows42);
  assert(basis);assert.deepEqual(basis.cells.map(c=>[c.slot,c.item?.uid]),[[0,"501"],[2,"502"]]);
  assert.equal(basis.resolvedWire.type,"moveItem");assert.equal(heroUi.heroActionBasisMatches(basis,f.model,action,heroBasisWindows42),true);
  f.info.inventory[0].count=2;f.world.heroInventoryItems[0].quantity=2;f.authority.receiveInformation(f.info,f.owner);f.authority.receiveSnapshot(f.world,f.owner);
  assert.equal(heroUi.heroActionBasisMatches(basis,f.authority.read(f.owner),action,heroBasisWindows42),false);
  assert.equal(heroUi.heroActionBasisMatches({...basis,untrusted:true},f.model,action,heroBasisWindows42),false);
  assert.equal(Object.isFrozen(basis.facts.policies),true);assert.equal(Object.isFrozen(f.owner.socket),false);
});
check("Hero basis automatic remove preserves semantic omission and original bag-first target",()=>{
  const f=heroFixture(),gear=heroItemFixture(503,12),info=heroInfoFixture(12,{item_type:12});
  f.info.equipment[3]=gear;f.world.heroEquipmentItems=[heroWorldRowFixture(gear,3,"bag1",info)];
  f.authority.receiveInformation(f.info,f.owner);f.authority.receiveSnapshot(f.world,f.owner);const model=f.authority.read(f.owner),action={kind:"remove",from:3};
  assert(model);const basis=heroUi.captureHeroActionBasis(model,action,heroBasisWindows42);assert(basis);
  assert.equal("to" in action,false);assert.deepEqual(basis.facts.target,{mode:"automatic",slot:3});assert.equal(basis.resolvedWire.to,3);
  assert.equal(basis.cells.length,11);assert.equal(basis.cells.filter(c=>c.grid==="HeroInventory").length,10);
  const explicit=heroUi.captureHeroActionBasis(model,{...action,to:1},heroBasisWindows42);assert(explicit);
  assert.equal(explicit.cells.length,2);assert.deepEqual(explicit.facts.target,{mode:"explicit",slot:1});
});
check("Hero basis retains both base and effective use policy and refuses differing Rust wire",()=>{
  const f=heroFixture();f.world.heroInventoryItems[1].tooltipSource.realInfo=heroInfoFixture(777,{item_type:8,stack_size:30});
  f.authority.receiveSnapshot(f.world,f.owner);const model=f.authority.read(f.owner),action={kind:"use",slot:2};
  const basis=heroUi.captureHeroActionBasis(model,action,heroBasisWindows42);assert(basis);
  const source=basis.facts.policies.find(p=>p.grid==="HeroInventory"&&p.slot===2);
  assert.equal(source.base.itemIndex,10);assert.equal(source.effective.itemIndex,777);assert.equal(source.effective.stackSize,30);
  assert.equal(basis.resolvedWire.type,"equipItem");assert.equal(basis.resolvedWire.uniqueId,"502");
  const different={...basis,resolvedWire:{type:"mergeItem",gridFrom:"HeroInventory",gridTo:"HeroEquipment",idFrom:"502",idTo:"503"}};
  assert.equal(heroUi.heroActionBasisMatches(different,model,action,heroBasisWindows42),false);
  assert.equal(heroUi.planHeroAction(model,action).wire.type,"equipItem","witness rejection cannot rewrite the existing Web planner");
});
check("Hero basis carries belt restock authority and leaves request allocation in the sole ledger",()=>{
  const f=heroFixture(),basis=heroUi.captureHeroActionBasis(f.model,{kind:"use",slot:0},heroBasisWindows42);assert(basis);
  assert.equal(basis.cells.length,10);assert.deepEqual(basis.facts.restock,{belt:0,from:2,uid:"502",itemIndex:10});
  const magic=heroUi.captureHeroActionBasis(f.model,{kind:"magicKey",spell:"FireBall",key:18},heroBasisWindows42);assert(magic);
  assert.deepEqual(magic.facts.keys,[{spell:"FireBall",key:17}]);assert.equal(magic.resolvedWire.oldKey,17);assert.equal("requestId" in magic.resolvedWire,false);
  const ledger=new heroUi.HeroPlayerOperations(),proof=ledger.reserve(f.model,{kind:"magicKey",spell:"FireBall",key:18});assert(proof);
  assert(Number.isSafeInteger(proof.wire.requestId));assert.equal(proof.wire.oldKey,17);
});
check("Hero basis displays exact wide UID but does not widen legacy mutation wire",()=>{
  const f=heroFixture(),id="9007199254740993";f.info.inventory[2].unique_id=id;f.world.heroInventoryItems[1].uniqueId=id;
  f.authority.receiveInformation(f.info,f.owner);f.authority.receiveSnapshot(f.world,f.owner);const model=f.authority.read(f.owner);assert(model);
  const basis=heroUi.captureHeroActionBasis(model,{kind:"move",from:2,to:3},heroBasisWindows42);assert(basis);
  assert.equal(basis.cells[0].item.uid,id);assert.equal(heroUi.captureHeroActionBasis(model,{kind:"use",slot:2},heroBasisWindows42),null);
});
check("Hero captured high-water changes even when identical raw has already been fully consumed",()=>{
  const f=heroRawFixture();f.deliver(heroWideOriginal);const before=f.ingress.highWater();assert(before);assert.equal(f.ingress.caughtUp(),true);
  const next=f.capture(heroWideOriginal);assert.equal(heroIngressDocument.sameHeroHighWater(before,f.ingress.highWater()),false);
  assert.equal(f.ingress.caughtUp(),false);assert(f.ingress.offer(next));assert.equal(f.ingress.caughtUp(),true);
  assert.equal(heroIngressDocument.sameHeroHighWater(before,f.ingress.highWater()),false);
  assert.equal(heroIngressDocument.sameHeroHighWater(before,{...before,socket:{}}),false);
});
const heroSubmitFunctions42=[];(function visit(node){if(ts.isFunctionDeclaration(node)&&["submitHeroAction","heroProofCurrent"].includes(node.name?.text))heroSubmitFunctions42.push(node.getText(parityPageAst));ts.forEachChild(node,visit);})(parityPageAst);
assert.equal(heroSubmitFunctions42.length,2);const heroSubmitJs42=ts.transpileModule(heroSubmitFunctions42.join("\n"),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
function heroSubmitFixture42(mode="accepted"){
  const f=heroFixture(),raw=heroRawFixture(),ledger=new heroUi.HeroPlayerOperations();raw.deliver(heroWideOriginal);
  let sends=0,enabled=true;const scope={currentHeroModel:()=>f.model,heroInputAllowed:()=>enabled,heroUiLeaseCurrent:()=>true,
    heroOperationsRef:{current:ledger},heroUiProofLeasesRef:{current:new WeakMap()},heroBeltProofRef:{current:null},heroProofHighWaterRef:{current:new WeakMap()},
    heroSharedUiIngressRef:{current:null},heroSharedProofsRef:{current:new WeakMap()},heroProofRendererTokensRef:{current:new WeakMap()},
    heroRendererTokenRef:{current:"hero-react:0"},heroWindowsRef:{current:heroBasisWindows42},heroBasisWindows:()=>heroBasisWindows42,heroActionBasisMatches:heroUi.heroActionBasisMatches,
    heroRawIngressRef:{current:raw.ingress},sameHeroHighWater:heroIngressDocument.sameHeroHighWater,renderParityServices(){},console:{error(){}},
    sendRaw(wire,{heroProof}){
      if(mode==="prethrow")throw Error("controlled preclaim exception");
      if(mode==="captured"||mode==="consumed"){const d=raw.capture(heroWideOriginal);if(mode==="consumed")raw.ingress.offer(d);}
      if(!api.current(heroProof,wire))return false;
      if(mode==="preclaim")return false;
      assert(ledger.claim(heroProof,f.model,wire));if(mode==="postclaim")return false;if(mode==="throw")throw Error("controlled send exception");sends++;return true;
    }};
  const api=new Function(...Object.keys(scope),heroSubmitJs42+"\nreturn {submit:submitHeroAction,current:heroProofCurrent};")(...Object.values(scope));
  return {f,raw,ledger,api,scope,get sends(){return sends;},disable(){enabled=false;}};
}
check("Hero actual Page submit returns true only after accepted send and cannot clear entered custody",()=>{
  for(const mode of ["accepted","preclaim","prethrow","postclaim","throw"]){const f=heroSubmitFixture42(mode),dto=heroUi.captureHeroAction(f.f.model,{kind:"move",from:2,to:3});assert(dto);
    assert.equal(f.api.submit(dto,false,{origin:"inventory",epoch:1}),mode==="accepted");assert.equal(f.sends,mode==="accepted"?1:0);
    assert.equal(f.ledger.pending?.state??null,["preclaim","prethrow"].includes(mode)?null:mode==="accepted"?"entered":"unknown");
    if(f.ledger.pending)assert.equal(f.ledger.cancelDefinitelyUnsent(f.ledger.pending.proof),false);
  }
  const blocked=heroSubmitFixture42();blocked.disable();assert.equal(blocked.api.submit(heroUi.captureHeroAction(blocked.f.model,{kind:"move",from:2,to:3}),false,{origin:"inventory",epoch:1}),false);assert.equal(blocked.ledger.pending,null);
});
check("Hero actual Page final gate rejects captured and already consumed same-content listener frames",()=>{
  for(const mode of ["captured","consumed"]){const f=heroSubmitFixture42(mode),dto=heroUi.captureHeroAction(f.f.model,{kind:"move",from:2,to:3});
    assert.equal(f.api.submit(dto,false,{origin:"inventory",epoch:1}),false);assert.equal(f.sends,0);assert.equal(f.ledger.pending,null);
  }
});


check("Hero maximum 42-cell bag keeps valid use witness within the unchanged action bound",()=>{
  const f=heroFixture(),info=heroInfoFixture(2147483647);f.info.inventory=Array.from({length:42},(_,slot)=>heroItemFixture(Number.MAX_SAFE_INTEGER-slot,2147483647,slot===0?1:3));
  f.world.heroInventoryCapacity=42;f.world.heroInventoryItems=f.info.inventory.map((item,slot)=>heroWorldRowFixture(item,slot,"bag1",info));
  f.authority.receiveInformation(f.info,f.owner);f.authority.receiveSnapshot(f.world,f.owner);const model=f.authority.read(f.owner);assert(model);
  const basis=heroUi.captureHeroActionBasis(model,{kind:"use",slot:0},heroBasisWindows42);assert(basis);assert.equal(basis.cells.length,42);
  assert.equal(basis.facts.policies.length,1,"scan-only occupancy must not duplicate unused item policy");assert(new TextEncoder().encode(JSON.stringify(basis)).byteLength<=16384);
  assert.equal(basis.facts.restock.from,2);assert.equal(basis.facts.restock.itemIndex,2147483647);
});

// Source43 compares independent real projectors through finite ports, never WASM.
const heroWitnessGolden43="{\"actor\":{\"class\":\"Warrior\",\"gender\":\"Male\",\"name\":\"Hero\",\"objectId\":12,\"spawned\":true},\"config\":{\"autoPot\":false,\"hpItemIndex\":0,\"hpPercent\":30,\"mpItemIndex\":0,\"mpPercent\":40},\"display\":{\"experience\":\"77\",\"hair\":3,\"hp\":20,\"level\":2,\"maxExperience\":\"200\",\"maxHp\":30,\"maxMp\":15,\"mp\":10},\"equipment\":[null,null,null,null,null,null,null,null,null,null,null,null,null,null],\"inventory\":[null,null,null,null,null,null,null,null,null,null],\"inventoryCapacity\":10,\"keys\":[],\"personalCapacity\":40,\"personalInventory\":[null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null],\"planner\":{\"experience\":\"77\",\"hp\":20,\"level\":2,\"maxExperience\":\"200\",\"mp\":10},\"riding\":null,\"skills\":[],\"stats\":[],\"version\":1,\"weights\":{\"bag\":1,\"hand\":3,\"wear\":2}}";
function heroWitnessFixture43(){
  const base=heroFixture(),owner={...base.owner,playerObjectId:42,mapFileName:"TestMap"};
  const info={...base.info,object_id:12,level:2,hair:3,hp:20,mp:10,experience:77,max_experience:200,
    inventory:Array(10).fill(null),equipment:Array(14).fill(null),magics:[],auto_pot:false,
    auto_hp_percent:30,auto_mp_percent:40,hp_item_index:0,mp_item_index:0};
  const world={...base.world,playerObjectId:42,mapFileName:"TestMap",inventoryItems:[],beltItems:[],heroInventoryItems:[],heroEquipmentItems:[],
    heroMaxExperience:200,heroStats:[],heroWeights:{bag:1,wear:2,hand:3},heroVitals:{hp:20,maxHp:30,mp:10,maxMp:15},
    stage5Systems:{hero:{name:"Hero",class:"Warrior",gender:"Male",level:2,experience:77,behaviour:0,spawned:true,
      autoPot:false,autoHpPercent:30,autoMpPercent:40,hpItemIndex:0,mpItemIndex:0},heroLearnedMagics:[]}};
  const authority=new heroUi.HeroPlayerAuthority();assert(authority.receiveInformation({info},owner));assert(authority.receiveSnapshot(world,owner));
  return {owner,info,world,authority,get model(){const model=authority.read(owner);assert(model);return model;}};
}
function freezeHeroWitnessFixture43(value){
  if(value&&typeof value==="object"){for(const v of Object.values(value))freezeHeroWitnessFixture43(v);Object.freeze(value);}return value;
}
function heroWitnessPort43(f=heroWitnessFixture43()){
  const owner=f.owner,state={source:{scope:{runGeneration:1,connectionGeneration:owner.connectionGeneration,sessionGeneration:owner.sessionGeneration,
    sceneRevision:owner.sceneRevision,playerObjectId:owner.playerObjectId,mapFileName:owner.mapFileName},frameSequence:2,heroObjectId:f.model.actor.objectId,
    heroGeneration:1,rustModelRevision:1},witness:heroUi.createHeroSourceWitness(f.model),cursor:2,closed:false,control:null,ready:false,
    appliedSource:null,appliedWitness:null,sinkGeneration:2,frame:0};assert(state.witness);
  const controls=[],runtime={getMir2HeroUiAbiVersion:()=>1,getMir2HeroActionBasisVersion:()=>1,getMir2HeroSourceWitnessVersion:()=>1,
    activateMir2HeroIngress:()=>true,pushMir2HeroRawFrame:()=>true,pushMir2HeroVerifiedOwnerFrame:()=>true,withdrawMir2HeroIngress:()=>true,
    getMir2HeroIngressCheckpoint:()=>null,restoreMir2HeroIngressCheckpoint:()=>true,
    getMir2HeroSourceWitness:()=>JSON.stringify({version:1,source:state.source,witness:state.witness}),
    getMir2HeroUiStatus:()=>JSON.stringify({version:1,source:state.source,acceptedFrameSequence:state.cursor,closed:state.closed,
      appliedSource:state.appliedSource,appliedWitness:state.appliedWitness,controlRevision:state.control?.controlRevision??0,
      webLeaseToken:state.control?.webLeaseToken??"",sinkGeneration:state.sinkGeneration,frame:state.frame,modal:false,
      ready:state.ready,prepared:state.prepared??false,capturesEscape:state.capturesEscape??false,acceptedInputSequence:state.acceptedInputSequence??0,
      inputEnabled:state.ready,inputRegions:state.ready?[{left:0,top:0,width:316,height:236}]:[],receiptFrames:[]}),
    setMir2HeroUiControlWithWitness(scope,raw,witness){controls.push({scope,raw,witness});state.control=JSON.parse(raw);return true;},
    setMir2HeroUiInputEdge:()=>true,setMir2HeroUiIntentSink:()=>state.sinkGeneration,clearMir2HeroUiIntentSink:()=>true};
  const control={source:state.source,controlRevision:1,webLeaseToken:"web-authority:43:hero:1",hudGeneration:8,webModelRevision:9,
    presentationRevision:10,windowEpochs:[1,2,3],windows:{inventoryOpen:true,characterOpen:false,characterPage:"equipment",beltVisible:false,beltVertical:false},
    presentation:{logicalWidth:1024,logicalHeight:768,stageCssScale:1,touch:false},inGame:true,hostVisible:true,inputEnabled:true,pending:false};
  return {f,state,controls,runtime,control,publish(){state.appliedSource=state.source;state.appliedWitness=state.witness;state.frame++;state.prepared=true;state.ready=true;}};
}
check("Hero independent Authority witness matches actual Rust golden bytes without adopting runtime data",()=>{
  const f=heroWitnessFixture43(),witness=heroUi.createHeroSourceWitness(f.model);assert.equal(witness,heroWitnessGolden43);
  assert(heroUi.heroSourceWitnessMatches(witness,f.model));assert.equal(Object.isFrozen(f.model.plannerSource),true);
  assert.equal(Object.isFrozen(f.model.displaySource),true);assert.equal(heroUi.createHeroSourceWitness(heroFixture().model),null);
  const changed=Object.freeze({...f.model,authoritySerial:900,informationSerial:900,snapshotSerial:900,personalSerial:900,skillSnapshotSerial:900,
    actor:Object.freeze({...f.model.actor,generation:900})});assert.equal(heroUi.createHeroSourceWitness(changed),witness);
});
check("Hero live HP keeps actual owner inventory and all full-source ACK barriers",()=>{
  const f=heroWitnessFixture43();f.world.heroInventoryItems=[heroWorldRowFixture(heroItemFixture(501),2)];
  f.world.stage5Systems.hero.autoHpPercent=55;assert(f.authority.receiveSnapshot(f.world,f.owner));const before=f.model;
  const ledger=new heroUi.HeroPlayerOperations(),proof=ledger.reserve(before,{kind:"move",from:2,to:3});assert(proof);assert(ledger.claim(proof,before,proof.wire));
  const witness=heroUi.createHeroSourceWitness(before);assert(witness);
  assert(f.authority.receiveHealthChanged({hp:7,mp:2},f.owner));const after=f.model;
  for(const name of ["informationSerial","snapshotSerial","personalSerial","skillSnapshotSerial"])assert.equal(after[name],before[name]);
  assert(after.authoritySerial>before.authoritySerial);assert.equal(after.inventory[2].uniqueId,501);assert.equal(after.hpPercent,55);
  assert.equal(after.plannerSource.hp,7);assert.equal(after.displaySource.hp,7);assert.equal(after.displaySource.maxHp,30);
  assert.notEqual(heroUi.createHeroSourceWitness(after),witness);assert.equal(after.experience,77);
  assert.equal(ledger.receipt("HeroHealthChanged",{hp:7,mp:2},f.owner,after.authoritySerial),false);ledger.observe(after);
  assert.equal(ledger.pending.state,"entered");assert.equal(ledger.cancelDefinitelyUnsent(proof),false);
  assert(ledger.receipt("MoveItem",{grid:"HeroInventory",from:2,to:3,success:true},f.owner,after.authoritySerial));
  assert(f.authority.receiveHealthChanged({hp:6,mp:1},f.owner));const afterAck=f.model;
  assert.equal(afterAck.snapshotSerial,before.snapshotSerial);assert.equal(ledger.observe(afterAck),false);
  assert.equal(ledger.pending.state,"acknowledged");assert.equal(ledger.cancelDefinitelyUnsent(proof),false);
});
check("Hero owner packet and new Information keep planner and painter level XP domains distinct",()=>{
  const f=heroWitnessFixture43();assert(f.authority.receiveHealthChanged({hp:7,mp:2},f.owner));
  const info={...f.info,level:9,hair:7,hp:5,mp:1,experience:88,max_experience:300};
  assert(f.authority.receiveInformation({info},f.owner));const newer=f.model,witness=JSON.parse(heroUi.createHeroSourceWitness(newer));
  assert.equal(witness.planner.level,9);assert.equal(witness.planner.experience,"88");assert.equal(witness.planner.maxExperience,"300");
  assert.equal(witness.display.level,2);assert.equal(witness.display.experience,"77");assert.equal(witness.display.maxExperience,"200");
  assert.equal(witness.display.hp,5);assert.equal(witness.display.maxHp,30);assert.equal(witness.display.hair,7);
  assert(f.authority.receiveHealthChanged({hp:0,mp:-2},f.owner));const hp=f.model;
  assert.equal(hp.plannerSource.level,9);assert.equal(hp.displaySource.level,2);assert.equal(hp.plannerSource.hp,0);
  assert.equal(hp.informationSerial,newer.informationSerial);assert.equal(hp.snapshotSerial,newer.snapshotSerial);
  f.world.stage5Systems.hero.level=3;f.world.stage5Systems.hero.experience=90;f.world.heroMaxExperience=250;f.world.heroVitals.hp=12;
  assert(f.authority.receiveSnapshot(f.world,f.owner));const owner=f.model;
  assert.equal(owner.plannerSource.hp,12);assert.equal(owner.plannerSource.level,3);assert.equal(owner.plannerSource.experience,90);
  assert.equal(owner.displaySource.level,3);assert.equal(owner.displaySource.maxExperience,250);assert.equal(owner.displaySource.hair,7);
});
check("Hero invalid or foreign health cannot alter model clocks counters or physical source",()=>{
  const f=heroWitnessFixture43(),before=f.model,witness=heroUi.createHeroSourceWitness(before);
  for(const payload of [{hp:7},{hp:1.5,mp:2},{hp:2147483648,mp:2},{hp:NaN,mp:2},{hp:7,mp:Infinity}])assert.equal(f.authority.receiveHealthChanged(payload,f.owner),false);
  for(const owner of [{...f.owner,socket:{}},{...f.owner,sessionGeneration:f.owner.sessionGeneration+1},
    {...f.owner,sceneRevision:f.owner.sceneRevision+1},{...f.owner,mapFileName:"OtherMap"}])assert.equal(f.authority.receiveHealthChanged({hp:7,mp:2},owner),false);
  assert.equal(f.model.authoritySerial,before.authoritySerial);assert.equal(heroUi.createHeroSourceWitness(f.model),witness);
});
check("Hero source witness preserves exact wide signed and zero UID data without mutation custody",()=>{
  const f=heroWitnessFixture43(),wide="18446744073709551615";
  f.info.inventory[2]=heroItemFixture(wide,2147483647,65535);f.info.inventory[3]=heroItemFixture(0);
  f.world.heroInventoryItems=[heroWorldRowFixture(f.info.inventory[2],2),heroWorldRowFixture(f.info.inventory[3],3)];
  f.info.experience="-9223372036854775808";f.info.max_experience="9223372036854775807";
  f.world.stage5Systems.hero.experience=f.info.experience;f.world.heroMaxExperience=f.info.max_experience;
  assert(f.authority.receiveInformation(f.info,f.owner));assert(f.authority.receiveSnapshot(f.world,f.owner));const model=f.model;
  const witness=JSON.parse(heroUi.createHeroSourceWitness(model));assert.equal(witness.inventory[2].uid,wide);
  assert.equal(witness.inventory[2].count,65535);assert.equal(witness.inventory[3].uid,"0");assert.equal(witness.planner.experience,f.info.experience);
  assert.equal(heroUi.planHeroAction(model,{kind:"move",from:3,to:4}),null);assert.equal(heroUi.planHeroAction(model,{kind:"move",from:2,to:3}),null);
  assert.equal(heroUi.planHeroAction(model,{kind:"use",slot:2}),null);assert.equal(heroUi.captureHeroActionBasis(model,{kind:"move",from:3,to:4},heroBasisWindows42),null);
});
check("Hero full 42 plus14 plus80 cells and all actual protocol skills fit the 64KiB source cap",()=>{
  const f=heroWitnessFixture43(),uid=18446744073709551615n;
  f.info.inventory=Array.from({length:42},(_,slot)=>heroItemFixture(String(uid-BigInt(slot)),2147483647,65535));
  f.info.equipment=Array.from({length:14},(_,slot)=>heroItemFixture(String(uid-42n-BigInt(slot)),2147483647,65535));
  f.world.heroInventoryCapacity=42;f.world.heroInventoryItems=f.info.inventory.map((item,slot)=>heroWorldRowFixture(item,slot));
  f.world.heroEquipmentItems=f.info.equipment.map((item,slot)=>heroWorldRowFixture(item,slot));
  f.world.inventoryCapacity=86;f.world.maxBagSlots=80;
  f.world.inventoryItems=Array.from({length:80},(_,slot)=>heroWorldRowFixture(heroItemFixture(String(uid-56n-BigInt(slot)),2147483647,65535),slot%40,slot<40?"bag1":"bag2"));
  const protocol=readFileSync(new URL("../../../packages/protocol/src/types.rs",import.meta.url),"utf8");
  const spellBody=protocol.match(/pub enum Spell\s*\{([\s\S]*?)\n\}/)?.[1];assert(spellBody);
  const spells=Array.from(spellBody.matchAll(/^\s*([A-Za-z][A-Za-z0-9_]*)\s*=\s*\d+\s*,/gm),match=>match[1]);assert(spells.length>1);
  const seed=heroFixture().info.magics[0];f.info.magics=spells.map(spell=>({...seed,spell,name:spell,key:0}));
  f.world.stage5Systems.heroLearnedMagics=f.info.magics.map(m=>({spell:m.spell,level:m.level,key:m.key,experience:m.experience}));
  assert(f.authority.receiveInformation(f.info,f.owner));assert(f.authority.receiveSnapshot(f.world,f.owner));
  const raw=heroUi.createHeroSourceWitness(f.model);assert(raw);assert(new TextEncoder().encode(raw).byteLength<=65536);const value=JSON.parse(raw);
  assert.equal(value.inventory.length,42);assert.equal(value.equipment.length,14);assert.equal(value.personalInventory.length,80);assert.equal(value.skills.length,spells.length);
});
check("Hero canonical source cap counts UTF8 and rejects malformed canonical input",()=>{
  const f=heroWitnessFixture43(),seed=heroFixture().info.magics[0];f.info.magics=[seed];f.world.stage5Systems.heroLearnedMagics=[{spell:seed.spell,level:1,key:17,experience:0}];
  assert(f.authority.receiveInformation(f.info,f.owner));assert(f.authority.receiveSnapshot(f.world,f.owner));const model=f.model;
  const base=heroUi.createHeroSourceWitness(model);assert(base);const make=name=>{
    const raw=Object.freeze({...model.magics[0].raw,name}),magic=Object.freeze({...model.magics[0],name,raw});return Object.freeze({...model,magics:Object.freeze([magic])});};
  const overhead=new TextEncoder().encode(base).byteLength-seed.name.length;
  const exact=heroUi.createHeroSourceWitness(make("a".repeat(65536-overhead)));assert(exact);assert.equal(new TextEncoder().encode(exact).byteLength,65536);
  assert.equal(heroUi.createHeroSourceWitness(make("a".repeat(65537-overhead))),null);
  assert.equal(heroUi.createHeroSourceWitness(make("界".repeat(22000))),null);assert.equal(heroUi.createHeroSourceWitness(make("\ud800")),null);
  assert.equal(heroUi.heroSourceWitnessMatches(base+" ",model),false);assert.equal(heroUi.heroSourceWitnessMatches(JSON.stringify({...JSON.parse(base),extra:true}),model),false);
  assert.equal(heroUi.createHeroSourceWitness({...model}),null);assert.equal(heroUi.createHeroSourceWitness(Object.freeze({...model,plannerSource:undefined})),null);
});
check("Hero bridge requires independent model data and exact owner scope before requesting control",()=>{
  const f=heroWitnessPort43(),binding=heroIngressDocument.readHeroSourceWitness(f.runtime,f.f.model);assert(binding);assert(heroIngressDocument.supportsHeroSharedUi(f.runtime));
  assert.equal(heroIngressDocument.bindHeroUiControl(f.runtime,f.control,f.f.model)?.witness,binding.witness);assert.equal(f.controls.length,1);
  assert.equal(f.state.ready,false);assert.equal(f.state.appliedWitness,null);
  assert.equal(heroIngressDocument.readHeroUiReady(f.runtime,binding,f.f.model,1,f.control.webLeaseToken,2),null);
  for(const name of ["connectionGeneration","sessionGeneration","playerObjectId","sceneRevision","mapFileName"]){
    const port=heroWitnessPort43(),scope=port.state.source.scope;port.state.source={...port.state.source,scope:{...scope,[name]:typeof scope[name]==="string"?"OtherMap":scope[name]+1}};
    assert.equal(heroIngressDocument.readHeroSourceWitness(port.runtime,port.f.model),null);assert.equal(port.controls.length,0);
  }
  const wrongActor=heroWitnessPort43();wrongActor.state.source={...wrongActor.state.source,heroObjectId:13};
  assert.equal(heroIngressDocument.readHeroSourceWitness(wrongActor.runtime,wrongActor.f.model),null);
  f.state.witness=heroWitnessGolden43.replace('"hp":20','"hp":19');assert.equal(heroIngressDocument.readHeroSourceWitness(f.runtime,f.f.model),null);
});
check("Hero source reads reject cursor reentry unsupported capabilities and unbounded bootstrap status",()=>{
  const f=heroWitnessPort43(),getter=f.runtime.getMir2HeroSourceWitness;
  f.runtime.getMir2HeroSourceWitness=()=>{const value=getter();f.state.cursor++;return value;};assert.equal(heroIngressDocument.readHeroSourceWitness(f.runtime,f.f.model),null);
  const g=heroWitnessPort43();for(const runtime of [{...g.runtime,getMir2HeroSourceWitnessVersion:()=>0},
    {...g.runtime,setMir2HeroUiControlWithWitness:undefined},{...g.runtime,getMir2HeroActionBasisVersion:()=>{throw Error("capability");}}])assert.equal(heroIngressDocument.supportsHeroSharedUi(runtime),false);
  g.runtime.getMir2HeroUiStatus=()=>" ".repeat(196609);assert.equal(heroIngressDocument.readHeroBootstrap(g.runtime),null);
  assert.equal(heroIngressDocument.readHeroSourceWitness(g.runtime,g.f.model),null);
});
check("Hero UI Ready requires actual applied source witness control lease sink and paint frame",()=>{
  const f=heroWitnessPort43(),binding=heroIngressDocument.bindHeroUiControl(f.runtime,f.control,f.f.model);assert(binding);f.publish();
  assert(heroIngressDocument.readHeroUiReady(f.runtime,binding,f.f.model,1,f.control.webLeaseToken,2));
  const raw=f.runtime.getMir2HeroUiStatus(),status=JSON.parse(raw);
  for(const delta of [{appliedWitness:null},{appliedWitness:binding.witness+" "},{appliedSource:{...binding.source,rustModelRevision:2}},
    {controlRevision:2},{webLeaseToken:"other"},{sinkGeneration:3},{frame:0},{ready:false},{closed:true},
    {prepared:false},{capturesEscape:"true"},{acceptedInputSequence:-1},
    {inputRegions:[{left:0,top:0,width:-1,height:1}]},{receiptFrames:[1.5]}]){
    f.runtime.getMir2HeroUiStatus=()=>JSON.stringify({...status,...delta});assert.equal(heroIngressDocument.readHeroUiReady(f.runtime,binding,f.f.model,1,f.control.webLeaseToken,2),null);
  }
});
check("Hero actual Page routes health into independent Authority without settling entered operations",()=>{
  const f=heroWitnessFixture43();f.world.heroInventoryItems=[heroWorldRowFixture(heroItemFixture(501),2)];assert(f.authority.receiveSnapshot(f.world,f.owner));
  const before=f.model,page=parityPageFixture(f.owner,f.world);page.scope.heroAuthorityRef.current=f.authority;
  const proof=page.scope.heroOperationsRef.current.reserve(before,{kind:"move",from:2,to:3});assert(proof);
  assert(page.scope.heroOperationsRef.current.claim(proof,before,proof.wire));page.api.captureParityPacket("HeroHealthChanged",{hp:7,mp:2});
  const after=f.model;assert.equal(after.plannerSource.hp,7);assert.equal(after.displaySource.hp,7);assert.equal(after.inventory[2].uniqueId,501);
  for(const name of ["informationSerial","snapshotSerial","personalSerial","skillSnapshotSerial"])assert.equal(after[name],before[name]);
  assert.equal(page.scope.heroOperationsRef.current.pending.state,"entered");assert.equal(page.sent.length,0);
});


// Source44 exercises the real ownership host through finite ports. Publication is
// separate from control acceptance; this is not a renderer, browser or WASM test.
const heroHostDocument44=loadTypeScriptModule(new URL("../lib/bevy-hero-host.ts",import.meta.url),{
  "./bevy-hero-ui":heroIngressDocument,"./hero-player-ui":heroUi,
});
function heroHostFixture44(){
  const base=heroFixture(),gearInfo=heroInfoFixture(12,{item_type:12}),bagGear=heroItemFixture(504,12),worn=heroItemFixture(505,12);
  base.info.inventory[4]=bagGear;base.info.equipment[3]=worn;
  base.world.heroInventoryItems.push(heroWorldRowFixture(bagGear,4,"bag1",gearInfo));
  base.world.heroEquipmentItems=[heroWorldRowFixture(worn,3,"bag1",gearInfo)];
  base.world.heroVitals={hp:100,maxHp:120,mp:50,maxMp:90};base.world.heroMaxExperience=100;base.world.stage5Systems.hero.experience=1;
  assert(base.authority.receiveInformation(base.info,base.owner));assert(base.authority.receiveSnapshot(base.world,base.owner));
  let model=base.authority.read(base.owner);assert(model);assert(heroUi.createHeroSourceWitness(model));
  const physical={socket:base.owner.socket,connectionGeneration:base.owner.connectionGeneration,sessionGeneration:base.owner.sessionGeneration};
  const scene={sceneRevision:base.owner.sceneRevision,playerObjectId:base.owner.playerObjectId,mapFileName:base.owner.mapFileName};
  const ingress=new heroIngressDocument.HeroRawIngress(),trace=[],intents=[],ownerEvents=[],states=[];
  const calls={read:0,now:0,status:0,witness:0,withdrawRaw:0},controls=[],edges=[],clears=[],sinks=[];
  const state={scope:null,source:null,cursor:0,witness:heroUi.createHeroSourceWitness(model),frame:1,controlHigh:0,control:null,
    publishedControl:null,appliedSource:null,appliedWitness:null,publishedSink:0,prepared:false,ready:false,
    sinkGeneration:0,sink:null,acceptedInputSequence:0,closed:false,modal:false,capturesEscape:false,queued:[]};
  const view={windows:{...heroBasisWindows42},windowEpochs:{inventory:1,character:2,belt:3},hudGeneration:8,
    presentation:{logicalWidth:1024,logicalHeight:768,stageCssScale:1,touch:false},pending:false,inputAllowed:true,eligible:true};
  let time=0,receivedAt=10.375,intentSequence=0,onIntent=intent=>{intents.push(intent);return true;},onOwner=()=>{};
  const sameSource=(a,b)=>heroIngressDocument.sameHeroSource(a,b),sameScope=raw=>heroIngressDocument.sameHeroScope(JSON.parse(raw),state.scope);
  const runtime={getMir2HeroUiAbiVersion:()=>1,getMir2HeroActionBasisVersion:()=>1,getMir2HeroSourceWitnessVersion:()=>1,
    activateMir2HeroIngress(raw){state.scope=JSON.parse(raw);return true;},
    pushMir2HeroRawFrame(scope,sequence,raw,firstClock){assert(sameScope(scope));assert(sequence>state.cursor);
      trace.push({scope,sequence,raw,firstClock});state.cursor=sequence;
      state.source={scope:{...state.scope},frameSequence:sequence,heroObjectId:model.actor.objectId,heroGeneration:1,rustModelRevision:(state.source?.rustModelRevision??0)+1};
      state.witness=heroUi.createHeroSourceWitness(model);assert(state.witness);return true;},
    pushMir2HeroVerifiedOwnerFrame(scope,sequence,raw,firstClock){return runtime.pushMir2HeroRawFrame(scope,sequence,raw,firstClock);},
    withdrawMir2HeroIngress(){calls.withdrawRaw++;return true;},getMir2HeroIngressCheckpoint:()=>null,restoreMir2HeroIngressCheckpoint:()=>false,
    getMir2HeroSourceWitness(){calls.witness++;return JSON.stringify({version:1,source:state.source,witness:state.witness});},
    getMir2HeroUiStatus(){calls.status++;const p=state.publishedControl,c=state.control;
      const matching=!!p&&!!c&&!!state.sink&&p.controlRevision===c.controlRevision&&p.webLeaseToken===c.webLeaseToken
        &&state.publishedSink===state.sinkGeneration&&sameSource(state.appliedSource,state.source)&&state.appliedWitness===state.witness;
      const ready=matching&&state.ready;
      return JSON.stringify({version:1,source:state.source,acceptedFrameSequence:state.cursor,acceptedInputSequence:state.acceptedInputSequence,closed:state.closed,
        appliedSource:state.appliedSource,appliedWitness:state.appliedWitness,controlRevision:p?.controlRevision??0,webLeaseToken:p?.webLeaseToken??"",
        sinkGeneration:state.publishedSink,frame:state.frame,modal:state.modal,prepared:matching&&state.prepared,ready,
        capturesEscape:ready&&state.capturesEscape,inputEnabled:ready&&p.inputEnabled,
        inputRegions:ready?[{left:10,top:20,width:316,height:236}]:[],receiptFrames:[]});},
    setMir2HeroUiControlWithWitness(scope,raw,witness){const control=JSON.parse(raw);
      if(!sameScope(scope)||!sameSource(control.source,state.source)||witness!==state.witness||control.controlRevision<=state.controlHigh)return false;
      controls.push(control);state.controlHigh=control.controlRevision;state.control=control;state.queued=[];return true;},
    setMir2HeroUiIntentSink(scope,sink){assert(sameScope(scope));state.sinkGeneration++;state.sink=sink;
      sinks.push({generation:state.sinkGeneration,sink});return state.sinkGeneration;},
    clearMir2HeroUiIntentSink(scope,expected){const current=sameScope(scope)&&expected===state.sinkGeneration&&!!state.sink;
      clears.push({expected,current});if(current){state.sinkGeneration++;state.sink=null;state.queued=[];}return current;},
    setMir2HeroUiInputEdge(scope,raw){const envelope=JSON.parse(raw),c=state.control;
      if(!sameScope(scope)||!state.sink||!c?.inputEnabled||envelope.sinkGeneration!==state.sinkGeneration
        ||envelope.webLeaseToken!==c.webLeaseToken||envelope.controlRevision!==c.controlRevision||envelope.edge.sequence<=state.acceptedInputSequence)return false;
      assert.deepEqual(envelope.edge.stamp,stamp(c));state.acceptedInputSequence=envelope.edge.sequence;edges.push(envelope);state.queued.push(envelope);return true;},
  };
  function stamp(c){return {scope:c.source.scope,heroObjectId:c.source.heroObjectId,heroGeneration:c.source.heroGeneration,hudGeneration:c.hudGeneration,
    modelRevision:c.webModelRevision,presentationRevision:c.presentationRevision,windowEpochs:c.windowEpochs};}
  function capture(raw=heroWideOriginal){const delivery=ingress.capture(raw,physical,receivedAt++);assert(delivery);return delivery;}
  function deliver(raw=heroWideOriginal){const delivery=capture(raw);assert(ingress.offer(delivery));return delivery;}
  ingress.sync(runtime,physical,scene);deliver();
  const options={runtime,now(){calls.now++;return time;},read(){calls.read++;return {...view,ingress,model};},
    onOwner(token){ownerEvents.push({token,control:state.control,projection:currentHost?.peek()});onOwner(token);},
    onState(next){states.push(next);},onIntent(intent){return onIntent(intent);}};
  let currentHost=new heroHostDocument44.BevyHeroHost(options);
  function publish({prepared=true}={}){time+=16;state.frame++;state.publishedControl=state.control;state.appliedSource=state.source;
    state.appliedWitness=state.witness;state.publishedSink=state.sinkGeneration;state.prepared=prepared;
    state.ready=prepared&&!!state.control?.inputEnabled&&!!state.sink;state.queued=[];}
  function ready(host=currentHost){host.tick();assert.equal(host.peek().owned,false);publish();host.tick();
    assert.equal(host.peek().owned,true);assert.equal(host.peek().active,false);publish();host.tick();assert.equal(host.peek().active,true);return host;}
  function envelope(action={kind:"move",from:2,to:3},origin="inventory",basisOverride){
    const c=state.control;assert(c);const normalized=heroHostDocument44.parseHeroSemanticAction(action);assert(normalized);
    const basis=basisOverride??heroUi.captureHeroActionBasis(model,normalized,c.windows);assert(basis);
    return {version:1,source:c.source,controlRevision:c.controlRevision,webLeaseToken:c.webLeaseToken,
      intent:{...stamp(c),intentSequence:++intentSequence,type:"action",origin,action,oldKey:normalized.kind==="magicKey"?model.magics.find(m=>m.spell===normalized.spell)?.key:null,basis}};
  }
  function navigation(windows){const c=state.control;return {version:1,source:c.source,controlRevision:c.controlRevision,webLeaseToken:c.webLeaseToken,
    intent:{...stamp(c),intentSequence:++intentSequence,type:"windows",windows}};}
  function dispatch(value,sink=state.sink){assert.equal(typeof sink,"function");return sink(typeof value==="string"?value:JSON.stringify(value));}
  function authority({information=true}={}){if(information)assert(base.authority.receiveInformation(base.info,base.owner));
    assert(base.authority.receiveSnapshot(base.world,base.owner));model=base.authority.read(base.owner);assert(model);return model;}
  return {base,state,view,options,calls,controls,edges,clears,sinks,trace,intents,ownerEvents,states,runtime,ingress,physical,scene,
    get host(){return currentHost;},get model(){return model;},setModel(next){model=next;},setOnIntent(fn){onIntent=fn;},setOnOwner(fn){onOwner=fn;},
    capture,deliver,publish,ready,envelope,navigation,dispatch,authority,
    restart(){currentHost=new heroHostDocument44.BevyHeroHost(options);return currentHost;},
    advanceTime(ms){time+=ms;},advanceFrame(){time+=16;state.frame++;},
    sameContent(){authority({information:false});return deliver();},
  };
}
check("Hero shared host publishes a pure owned projection between actual preparation and enabled Ready",()=>{
  const f=heroHostFixture44(),h=f.host;
  f.setOnOwner(token=>{assert(token);assert.equal(h.peek().owned,true);assert.equal(h.peek().active,false);
    assert.equal(f.state.control.inputEnabled,false,"DOM retirement precedes enabling control");});
  h.tick();assert.equal(h.peek().owned,false);assert.equal(f.controls.length,1);const prepareRevision=f.state.control.controlRevision;
  for(let i=0;i<2;i++){f.advanceFrame();h.tick();assert.equal(f.controls.length,1);assert.equal(h.peek().owned,false);}
  f.publish();h.tick();assert.equal(f.ownerEvents.length,1);assert.equal(f.state.control.inputEnabled,true);
  assert(f.state.control.controlRevision>prepareRevision);assert.equal(h.peek().active,false);
  const enabledRevision=f.state.control.controlRevision;
  for(let i=0;i<2;i++){f.advanceFrame();h.tick();assert.equal(f.state.control.controlRevision,enabledRevision);assert.equal(h.peek().owned,true);}
  f.publish();h.tick();assert.equal(h.peek().active,true);assert.equal(h.peek().worldBlocked,false);
  const counts={...f.calls},projection=h.peek();for(let i=0;i<4;i++)assert.strictEqual(h.peek(),projection);
  assert.deepEqual(f.calls,counts,"render projection samples no clock, source or runtime port");
});
check("Hero shared host never promotes a control or getter echo without exact applied World proof",()=>{
  for(const damage of ["appliedSource","appliedWitness","sink","prepared"]){
    const f=heroHostFixture44(),h=f.host;h.tick();f.publish();h.tick();assert.equal(h.peek().owned,true);f.publish();
    if(damage==="appliedSource")f.state.appliedSource={...f.state.source,rustModelRevision:f.state.source.rustModelRevision+1};
    if(damage==="appliedWitness")f.state.appliedWitness=null;
    if(damage==="sink")f.state.publishedSink++;
    if(damage==="prepared"){f.state.prepared=false;f.state.ready=false;}
    h.tick();assert.equal(h.peek().active,false);assert.equal(h.pointerContext(),null);assert.equal(f.intents.length,0);
  }
});
check("Hero shared sink admits all ten exact Rust semantic actions through the actual TS planner",()=>{
  const actions=[
    [{kind:"use",slot:0,confirmed:false},"belt"],[{kind:"move",from:2,to:3},"inventory"],
    [{kind:"equip",from:4,to:3},"inventory"],[{kind:"remove",from:3,to:null},"character"],
    [{kind:"merge",from:{grid:"HeroInventory",slot:2},to:{grid:"HeroInventory",slot:0}},"inventory"],
    [{kind:"transfer",from:3,to:3},"inventory"],[{kind:"takeBack",from:2,to:4},"inventory"],
    [{kind:"autoPotValue",stat:12,value:55},"inventory"],[{kind:"autoPotItem",grid:"HeroHpItem",slot:2},"inventory"],
    [{kind:"magicKey",spell:"FireBall",key:18},"character"],
  ];
  for(const [action,origin] of actions){const f=heroHostFixture44();f.ready();const raw=f.envelope(action,origin);
    assert.equal(f.dispatch(raw),true,action.kind);assert.equal(f.intents.length,1);const intent=f.intents[0];
    assert.equal(intent.type,"action");assert.equal(intent.action.kind,action.kind);assert.equal(intent.origin,origin);
    assert(heroUi.heroActionBasisMatches(intent.actionBasis,f.model,intent.action,heroBasisWindows42));
    assert(Object.isFrozen(intent));assert(Object.isFrozen(intent.model));assert(Object.isFrozen(intent.model.inventory));
    assert(Object.isFrozen(intent.actionBasis));assert.strictEqual(intent.model.owner.socket,f.base.owner.socket);
    assert.equal(Object.isFrozen(f.base.owner.socket),false);assert.equal(f.host.claimCurrent(intent),true);assert.equal(f.host.claimCurrent(intent),true);
    if(action.kind==="remove")assert.equal(Object.hasOwn(intent.action,"to"),false,"only Rust null maps to TS automatic placement");
    if(action.kind==="magicKey")assert.equal(intent.oldKey,17);
    assert.equal(f.dispatch(raw),false,"same serialized intent cannot replay");assert.equal(f.intents.length,1);
  }
});
check("Hero shared sink rejects malformed semantic fields stale basis oldKey and repeated denied envelopes",()=>{
  const f=heroHostFixture44();f.ready();
  for(const mutate of [v=>v.extra=true,v=>v.source.extra=true,v=>v.intent.extra=true,
    v=>v.intent.action.from="2",v=>v.intent.action.extra=true,v=>v.intent.basis.cells[0].item.uid="999",
    v=>v.intent.basis.resolvedWire.to=9,v=>v.intent.oldKey=17,v=>v.intent.windowEpochs[0]++,
    v=>v.intent.origin="belt",v=>v.intent.basis.extra=true]){
    const valid=f.envelope(),bad=JSON.parse(JSON.stringify(valid));mutate(bad);assert.equal(f.dispatch(bad),false);
  }
  const magic=f.envelope({kind:"magicKey",spell:"FireBall",key:18},"character");magic.intent.oldKey=18;assert.equal(f.dispatch(magic),false);
  const wrongBasis=f.envelope();wrongBasis.intent.basis={};assert.equal(f.dispatch(wrongBasis),false);
  wrongBasis.intent.basis=heroUi.captureHeroActionBasis(f.model,{kind:"move",from:2,to:3},heroBasisWindows42);
  assert.equal(f.dispatch(wrongBasis),false,"denied admitted sequence is still burned");assert.equal(f.intents.length,0);
  for(const action of [{kind:"remove",from:3},{kind:"use",slot:0},{kind:"use",slot:"0",confirmed:false},
    {kind:"autoPotValue",stat:14,value:10},{kind:"autoPotValue",stat:12,value:100},
    {kind:"autoPotItem",grid:"Inventory",slot:null},{kind:"magicKey",spell:"FireBall",key:1},
    {kind:"merge",from:{grid:"HeroInventory",slot:2,extra:true},to:{grid:"HeroInventory",slot:0}}])
    assert.equal(heroHostDocument44.parseHeroSemanticAction(action),null);
});
check("Hero shared UID0 remains display-only while wide UID cell moves do not widen legacy use",()=>{
  for(const uid of [0,"9007199254740993"]){
    const f=heroHostFixture44(),priorBasis=heroUi.captureHeroActionBasis(f.model,{kind:"use",slot:2,confirmed:false},heroBasisWindows42);
    f.base.info.inventory[2].unique_id=uid;const source=f.base.world.heroInventoryItems.find(item=>item.slot===2);
    source.uniqueId=uid;source.tooltipSource.userItem.unique_id=uid;f.authority();f.deliver();f.ready();
    assert.equal(JSON.parse(heroUi.createHeroSourceWitness(f.model)).inventory[2].uid,String(uid));
    const use=f.envelope({kind:"use",slot:2,confirmed:false},"inventory",priorBasis);assert.equal(f.dispatch(use),false);
    const move={kind:"move",from:2,to:3};
    if(uid===0){assert.equal(heroUi.captureHeroActionBasis(f.model,move,heroBasisWindows42),null);
      const oldMove=heroUi.captureHeroActionBasis(heroHostFixture44().model,move,heroBasisWindows42);
      assert.equal(f.dispatch(f.envelope(move,"inventory",oldMove)),false);
    }else{assert.equal(f.dispatch(f.envelope(move)),true);assert.equal(f.intents[0].actionBasis.cells[0].item.uid,uid);}
  }
});
check("Hero shared live pointer sink and claim reject a frozen frame at 501ms without waiting for tick",()=>{
  for(const boundary of ["pointer","sink","claim"]){
    const f=heroHostFixture44();f.ready();const context=f.host.pointerContext(),raw=f.envelope();assert(context);
    let accepted;if(boundary==="claim"){assert(f.dispatch(raw));accepted=f.intents[0];assert(f.host.claimCurrent(accepted));}
    f.advanceTime(500);if(boundary==="claim")assert.equal(f.host.claimCurrent(accepted),true);
    f.advanceTime(1);
    if(boundary==="pointer")assert.equal(f.host.pointer({context,pointerId:9,phase:"down",x:30,y:40,button:0}),false);
    if(boundary==="sink")assert.equal(f.dispatch(raw),false);
    if(boundary==="claim")assert.equal(f.host.claimCurrent(accepted),false);
    assert.equal(f.edges.length,0);assert.equal(f.intents.length,boundary==="claim"?1:0);
    assert.equal(f.host.isSharedOwner(),false);assert.equal(f.host.peek().owned,false);
  }
});
check("Hero shared late cleanup is generation checked and same-runtime restarts keep control and input monotonic",()=>{
  const f=heroHostFixture44(),old=f.ready(),context=old.pointerContext();assert(context);
  assert(old.pointer({context,pointerId:9,phase:"down",x:30,y:40,button:0}));
  assert(old.pointer({context,pointerId:9,phase:"up",x:30,y:40,button:0}));const sequence=f.edges.at(-1).edge.sequence,revision=f.state.control.controlRevision;
  const oldGeneration=f.state.sinkGeneration,replacement=f.restart();replacement.tick();const replacementSink=f.state.sink,replacementGeneration=f.state.sinkGeneration;
  assert(replacementGeneration>oldGeneration);assert(f.state.control.controlRevision>revision);const replacementControl=f.state.control;
  old.stop();assert.strictEqual(f.state.sink,replacementSink);assert.strictEqual(f.state.control,replacementControl);
  assert.deepEqual(f.clears.at(-1),{expected:oldGeneration,current:false});
  f.publish();replacement.tick();f.publish();replacement.tick();assert.equal(replacement.peek().active,true);
  const next=replacement.pointerContext();assert(next);assert(replacement.pointer({context:next,pointerId:9,phase:"down",x:30,y:40,button:0}));
  assert(f.edges.at(-1).edge.sequence>sequence);assert.equal(old.pointer({context,pointerId:9,phase:"up",x:30,y:40,button:0}),false);
  assert.equal(f.state.sinkGeneration,replacementGeneration);assert.equal(f.calls.withdrawRaw,0);
});
check("Hero shared identical full frame preserves fresh physical drag edges but never rebases an old queued intent",()=>{
  const f=heroHostFixture44(),h=f.ready(),context=h.pointerContext(),old=f.envelope();assert(context);
  assert(h.pointer({context,pointerId:9,phase:"down",x:30,y:40,button:0}));f.publish();h.tick();
  const before=f.state.control,oldModel=f.model;f.sameContent();assert(f.model.authoritySerial>oldModel.authoritySerial);assert.equal(f.model.sourceKey,oldModel.sourceKey);
  h.tick();const during=h.pointerContext();assert(during);assert.equal(during.ready,false);assert.equal(h.peek().owned,true);assert.equal(h.peek().active,false);
  assert.equal(f.state.control.webModelRevision,before.webModelRevision);assert.equal(f.state.control.presentationRevision,before.presentationRevision);
  assert.equal(f.state.control.inputEnabled,true);assert(f.state.control.controlRevision>before.controlRevision);
  assert.equal(h.pointer({context:during,pointerId:10,phase:"down",x:40,y:50,button:0}),false);
  assert(h.pointer({context,pointerId:9,phase:"move",x:80,y:90,button:0}));
  assert(h.pointer({context,pointerId:9,phase:"up",x:80,y:90,button:0}));
  assert.deepEqual(f.edges.slice(-2).map(edge=>edge.edge.phase),["move","up"]);
  assert.equal(f.edges.at(-1).controlRevision,f.state.control.controlRevision);assert.deepEqual(f.edges.at(-1).edge.stamp,context.stamp);
  assert.equal(f.dispatch(old),false);f.publish();h.tick();assert.equal(h.peek().active,true);assert.equal(f.dispatch(old),false);
  assert.equal(f.dispatch(f.envelope()),true);
});
check("Hero shared material pending layout and window changes revoke old pointer lineage before accepting a new click",()=>{
  for(const change of ["model","pending","presentation","window"]){
    const f=heroHostFixture44(),h=f.ready(),context=h.pointerContext();assert(context);
    assert(h.pointer({context,pointerId:9,phase:"down",x:30,y:40,button:0}));f.publish();h.tick();
    if(change==="model"){assert(f.base.authority.receiveHealthChanged({hp:90,mp:40},f.base.owner));f.setModel(f.base.authority.read(f.base.owner));f.deliver();}
    if(change==="pending")f.view.pending=true;
    if(change==="presentation")f.view.presentation={...f.view.presentation,logicalWidth:1280};
    if(change==="window"){f.view.windows={...f.view.windows,characterPage:"skills"};f.view.windowEpochs={...f.view.windowEpochs,character:f.view.windowEpochs.character+1};}
    h.tick();assert.equal(h.pointer({context,pointerId:9,phase:"up",x:30,y:40,button:0}),false);
    assert.equal(h.pointer({context,pointerId:9,phase:"cancel",x:30,y:40,button:0}),false);
    f.publish();h.tick();const fresh=h.pointerContext();assert(fresh?.ready);
    assert(h.pointer({context:fresh,pointerId:9,phase:"down",x:30,y:40,button:0}));
    assert(h.pointer({context:fresh,pointerId:9,phase:"up",x:30,y:40,button:0}));
  }
});
check("Hero shared own reservation stays claimable and only the original exact ACK plus new full snapshot settles it",()=>{
  const f=heroHostFixture44(),h=f.ready(),ledger=new heroUi.HeroPlayerOperations();let proof,accepted;
  f.setOnIntent(intent=>{accepted=intent;proof=ledger.reserve(f.model,heroUi.captureHeroAction(intent.model,intent.action));assert(proof);f.view.pending=true;
    assert(h.allows(intent));assert(h.claimCurrent(intent));assert(h.claimCurrent(intent));assert(ledger.claim(proof,f.model,proof.wire));return true;});
  assert.equal(f.dispatch(f.envelope()),true);assert.equal(ledger.pending.state,"entered");
  assert.equal(ledger.receipt("MoveItem",{grid:"HeroInventory",from:2,to:4,success:true},f.base.owner,f.model.authoritySerial),false);
  assert(ledger.receipt("MoveItem",{grid:"HeroInventory",from:2,to:3,success:true},f.base.owner,f.model.authoritySerial));assert.equal(ledger.pending.state,"acknowledged");
  h.withdraw();assert.equal(h.claimCurrent(accepted),false);assert.equal(ledger.pending.state,"acknowledged");assert.equal(ledger.observe(f.model),false);
  f.base.world.heroInventoryItems.find(item=>item.slot===2).slot=3;f.authority({information:false});
  assert.equal(f.model.inventory[2],null);assert.equal(f.model.inventory[3].uniqueId,502);assert.equal(ledger.observe(f.model),true);assert.equal(ledger.pending,null);
  assert.equal(f.calls.withdrawRaw,0);assert(f.ingress.highWater());
});
check("Hero shared final claim refuses synchronous captured source window and owner changes without releasing entered custody",()=>{
  for(const mode of ["captured","consumed","window","owner","entered"]){
    const f=heroHostFixture44(),h=f.ready(),ledger=new heroUi.HeroPlayerOperations();let proof;
    f.setOnIntent(intent=>{proof=ledger.reserve(f.model,heroUi.captureHeroAction(intent.model,intent.action));assert(proof);f.view.pending=true;
      if(mode==="entered")assert(ledger.claim(proof,f.model,proof.wire));
      if(mode==="captured")f.capture();
      if(mode==="consumed"||mode==="entered")f.sameContent();
      if(mode==="window"){f.view.windows={...f.view.windows,inventoryOpen:false};f.view.windowEpochs={...f.view.windowEpochs,inventory:f.view.windowEpochs.inventory+1};}
      if(mode==="owner")f.setModel(Object.freeze({...f.model,owner:Object.freeze({...f.model.owner,socket:{}})}));
      assert.equal(h.claimCurrent(intent),false);assert.equal(h.allows(intent),false);
      assert.equal(ledger.pending.state,mode==="entered"?"entered":"reserved");return false;});
    assert.equal(f.dispatch(f.envelope()),false);assert(proof);
    assert.equal(ledger.cancelDefinitelyUnsent(proof),mode!=="entered");
    if(mode==="entered"){assert.equal(ledger.pending.state,"entered");assert(ledger.outcomeUnknown(proof));assert.equal(ledger.pending.state,"unknown");}
    assert.equal(f.calls.withdrawRaw,0);
  }
});
check("Hero shared lifecycle gates fall back without retiring original raw custody and do not fake touch geometry",()=>{
  for(const mode of ["focus","hidden","resize","touch"]){
    const f=heroHostFixture44(),h=f.ready(),before=f.ingress.highWater(),first=f.trace[0];
    if(mode==="focus")f.view.inputAllowed=false;if(mode==="hidden")f.view.eligible=false;
    if(mode==="resize")f.view.presentation=null;if(mode==="touch")f.view.presentation={logicalWidth:1024,logicalHeight:768,stageCssScale:1,touch:true};
    h.tick();assert.equal(h.peek().owned,false);assert.equal(h.peek().active,false);assert.equal(f.calls.withdrawRaw,0);
    assert(heroIngressDocument.sameHeroHighWater(before,f.ingress.highWater()));assert(f.ingress.caughtUp());
    assert.strictEqual(f.trace[0],first);assert.equal(first.raw,heroWideOriginal);assert.equal(first.firstClock,10.375);
    assert.equal(h.pointerContext(),null);
  }
  assert.equal(heroHostDocument44.heroPresentationFits({logicalWidth:1024,logicalHeight:768,stageCssScale:1,touch:true}),false);
  assert.equal(heroHostDocument44.heroPresentationFits({logicalWidth:1024,logicalHeight:768,stageCssScale:4,touch:true}),true);
});
check("Hero shared keys leave world movement alone and preserve modal text Ctrl+A and Escape order",()=>{
  const f=heroHostFixture44(),h=f.ready();
  assert.equal(h.key({key:"ArrowUp",code:"ArrowUp"}),false);assert.equal(h.key({key:"a",code:"KeyA"}),false);assert.equal(f.edges.length,0);
  f.state.modal=true;f.state.capturesEscape=true;f.publish();h.tick();
  assert(h.key({key:"a",code:"KeyA",control:true}));assert.equal(f.edges.at(-1).edge.key,"KeyA");assert.equal(f.edges.at(-1).edge.text,"");
  assert(h.key({key:"5",code:"Digit5"}));assert.equal(f.edges.at(-1).edge.text,"5");
  assert(h.key({key:"Escape",code:"Escape"}));assert.equal(f.edges.at(-1).edge.key,"Escape");
  f.state.modal=false;f.state.capturesEscape=false;f.publish();h.tick();assert.equal(h.key({key:"ArrowUp"}),false);
  const context=h.pointerContext();assert(context);assert(h.pointer({context,pointerId:9,phase:"down",x:30,y:40,button:0}));
  assert(h.pointer({context,pointerId:9,phase:"up",x:30,y:40,button:0}));
  assert(h.key({key:"Escape"}),"same-frame Escape follows an admitted UI click instead of leaking to world");
});


// Source44: execute actual Page and Shell declarations in the original finite harness.
const heroShellNames44=new Set(["heroBlocksWorldInput","cancelSharedHeroPointer","handleSharedHeroPointer"]),heroShellDeclarations44=new Map();
(function visit(node){if(ts.isFunctionDeclaration(node)&&heroShellNames44.has(node.name?.text))heroShellDeclarations44.set(node.name.text,node.getText(fishingShellAst));ts.forEachChild(node,visit);})(fishingShellAst);
assert.equal(heroShellDeclarations44.size,3,"actual Hero Shell authority functions");
const heroShellJs44=ts.transpileModule([...heroShellDeclarations44.values()].join("\n"),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
function heroShellFixture44(){
  const ref=current=>({current}),edges=[];
  class HeroNode44 {id="";captured=new Set();focus(){}setPointerCapture(id){this.captured.add(id);}hasPointerCapture(id){return this.captured.has(id);}}
  const frame=new HeroNode44(),canvas=new HeroNode44();canvas.id="shared-hero-test-canvas";
  let context={webLeaseToken:"hero:44",sinkGeneration:2,stamp:{scope:{runGeneration:1,connectionGeneration:1,sessionGeneration:1,sceneRevision:1,playerObjectId:42,mapFileName:"TestMap"},
    heroObjectId:12,heroGeneration:1,hudGeneration:8,modelRevision:9,presentationRevision:10,windowEpochs:[1,2,3]},
    presentation:{logicalWidth:1024,logicalHeight:768,stageCssScale:1,touch:false},ready:true,inputRegions:[{left:0,top:0,width:40,height:40}]};
  let stops=0,accepted=true;
  const scope={HTMLElement:HeroNode44,heroPointerLeaseRef:ref(null),heroHoverContextRef:ref(null),
    heroPointerCallbacksRef:ref({getBevyHeroInputBlocked:()=>false,getBevyHeroPointerContext:()=>context,
      onBevyHeroPointer:edge=>{edges.push(edge);return accepted;}}),
    stageFrameRef:ref(frame),sceneInteractionReady:true,questLocalModalOpen:false,mobileMoreOpen:false,bevyQuestUiCapturesPointer:false,
    webGl2SharedCanvasPrototype:false,sharedUiCanvasId:()=>canvas.id,scenePointFromMouseEvent:({clientX,clientY})=>({sceneX:clientX,sceneY:clientY}),
    stopNpcShopWorldInput:()=>stops++,heldScenePointerRef:ref(null),worldFishingPhysicalRef:ref(null),combatUiHoldRef:ref(new Map()),
    bagBeltPointerRef:ref(null),bagBeltArmedSharedRef:ref(null),npcRepairPointerRef:ref(null),bagBeltQuarantineRef:ref(new Map()),npcRepairQuarantineRef:ref(new Map()),
    heldQuestControlPointersRef:ref(new Set()),hudPointerRouterRef:ref({held:null}),bagPointerRouterRef:ref({held:null}),
    characterPointerRouterRef:ref({held:null}),storagePointerRouterRef:ref({held:null}),npcShopPointerRouterRef:ref({held:null}),
    spellsPointerRouterRef:ref({held:null}),mailPointerRouterRef:ref({held:null}),bevyMailComposeReady:false,bevyMailComposePending:false};
  const api=new Function(...Object.keys(scope),heroShellJs44+"\nreturn {handle:handleSharedHeroPointer,cancel:cancelSharedHeroPointer,blocked:heroBlocksWorldInput};")(...Object.values(scope));
  const event=(time=10,extra={})=>({pointerId:1,pointerType:"mouse",timeStamp:time,type:"pointermove",button:0,buttons:0,clientX:10,clientY:10,target:canvas,
    preventDefault(){this.prevented=true;},...extra});
  return {api,scope,edges,frame,canvas,event,get context(){return context;},setContext:value=>{context=value;},setAccepted:value=>{accepted=value;},get stops(){return stops;}};
}
check("Hero actual Shell hover clears once through its original context and never captures world origin",()=>{
  const f=heroShellFixture44(),original=f.context;
  assert.equal(f.api.handle(f.event(),"move"),true);assert.equal(f.edges.length,1);assert.equal(f.scope.heroPointerLeaseRef.current,null);assert.equal(f.stops,0);assert.equal(f.canvas.captured.size,0);
  f.setContext(structuredClone(original));
  const outside=f.event(11,{clientX:100});assert.equal(f.api.handle(outside,"move"),false);assert.equal(outside.prevented,undefined);
  assert.equal(f.edges.length,2);assert.equal(f.edges[1].context,original);assert.equal(f.edges[1].x,100);assert.equal(f.scope.heroHoverContextRef.current,null);
  assert.equal(f.api.handle(f.event(12,{clientX:101}),"move"),false);assert.equal(f.edges.length,2);
  for(const name of ["heldScenePointerRef","worldFishingPhysicalRef","bagBeltPointerRef","bagBeltArmedSharedRef","npcRepairPointerRef"]){
    const guarded=heroShellFixture44();guarded.scope[name].current={};assert.equal(guarded.api.handle(guarded.event(),"move"),false);assert.equal(guarded.edges.length,0);
  }
  for(const name of ["hudPointerRouterRef","bagPointerRouterRef","characterPointerRouterRef","storagePointerRouterRef","npcShopPointerRouterRef","spellsPointerRouterRef","mailPointerRouterRef"]){
    const guarded=heroShellFixture44();guarded.scope[name].current.held={};assert.equal(guarded.api.handle(guarded.event(),"move"),false);assert.equal(guarded.edges.length,0);
  }
  const pressed=heroShellFixture44();assert.equal(pressed.api.handle(pressed.event(10,{buttons:1}),"move"),false);assert.equal(pressed.edges.length,0);
});
check("Hero actual Shell rejects old terminals and wrong capture without clearing the current physical lease",()=>{
  const f=heroShellFixture44();assert.equal(f.api.handle(f.event(10,{type:"pointerdown",buttons:1}),"down"),true);
  const old=f.scope.heroPointerLeaseRef.current;assert(old);assert.equal(f.edges[0].context,old.context);
  for(const extra of [{timeStamp:9,type:"pointerup"},{pointerType:"touch",type:"pointerup"},
    {type:"lostpointercapture",target:f.frame},{type:"lostpointercapture",target:f.canvas}]){
    assert.equal(f.api.handle(f.event(11,extra),extra.type==="lostpointercapture"?"cancel":"up"),true);assert.equal(f.scope.heroPointerLeaseRef.current,old);assert.equal(f.edges.length,1);
  }
  assert.equal(f.api.handle(f.event(12,{type:"pointercancel"}),"cancel"),true);assert.equal(f.scope.heroPointerLeaseRef.current,null);
  assert.equal(f.api.handle(f.event(20,{type:"pointerdown",buttons:1}),"down"),true);const next=f.scope.heroPointerLeaseRef.current;
  f.api.cancel(old);assert.equal(f.scope.heroPointerLeaseRef.current,next);
  assert.equal(f.api.handle(f.event(15,{type:"pointerup"}),"up"),true);assert.equal(f.scope.heroPointerLeaseRef.current,next);
  assert.equal(f.api.handle(f.event(21,{type:"pointerup"}),"up"),true);assert.equal(f.scope.heroPointerLeaseRef.current,null);
  assert.equal(f.edges.at(-1).context,next.context);assert.equal(f.edges.at(-1).phase,"up");
  const wrong=heroShellFixture44();wrong.api.handle(wrong.event(10,{type:"pointerdown",buttons:1}),"down");
  assert.equal(wrong.api.handle(wrong.event(11,{type:"pointerup",button:2}),"up"),true);assert.deepEqual(wrong.edges.map(e=>e.phase),["down","cancel"]);
  const replaced=heroShellFixture44();replaced.api.handle(replaced.event(10,{type:"pointerdown",buttons:1}),"down");const newEdges=[];
  replaced.scope.heroPointerCallbacksRef.current={...replaced.scope.heroPointerCallbacksRef.current,onBevyHeroPointer:e=>{newEdges.push(e);return true;}};
  assert.equal(replaced.api.handle(replaced.event(11,{type:"pointerup"}),"up"),true);assert.deepEqual(replaced.edges.map(e=>e.phase),["down","cancel"]);assert.equal(newEdges.length,0);
});
function heroPageSharedFixture44(mode="accepted"){
  const f=heroSubmitFixture42(mode),action={kind:"move",from:2,to:3},dto=heroUi.captureHeroAction(f.f.model,action),basis=heroUi.captureHeroActionBasis(f.f.model,action,heroBasisWindows42);assert(dto&&basis);
  let current=true,claims=0;
  const intent={type:"action",origin:"inventory",action,actionBasis:basis,oldKey:null,model:f.f.model,sourceWindows:heroBasisWindows42,
    webLeaseToken:"hero-shared:44",windowEpochs:{inventory:1,character:2,belt:3},intentSequence:1};
  f.scope.heroRendererTokenRef.current=intent.webLeaseToken;
  f.scope.heroSharedUiIngressRef.current={isSharedOwner:()=>true,allows:()=>current,claimCurrent:value=>{assert.equal(value,intent);claims++;return current;}};
  return {...f,intent,dto,get claims(){return claims;},invalidate:()=>{current=false;}};
}
check("Hero actual Page shared action uses its sole ledger and preserves definitely unsent versus entered unknown",()=>{
  for(const mode of ["accepted","preclaim","prethrow","postclaim","throw"]){
    const f=heroPageSharedFixture44(mode);assert.equal(f.api.submit(f.dto,false,{origin:"inventory",epoch:1},f.intent.webLeaseToken,f.intent),mode==="accepted");
    assert.equal(f.ledger.pending?.state??null,["preclaim","prethrow"].includes(mode)?null:mode==="accepted"?"entered":"unknown");
    if(f.ledger.pending){assert.equal(f.scope.heroSharedProofsRef.current.get(f.ledger.pending.proof),f.intent);assert.equal(f.ledger.cancelDefinitelyUnsent(f.ledger.pending.proof),false);}
  }
  const legacy=heroPageSharedFixture44();assert.equal(legacy.api.submit(legacy.dto,false,{origin:"inventory",epoch:1},"hero-react:0"),false);assert.equal(legacy.ledger.pending,null);
});
check("Hero actual Page final claim rejects synchronous source or renderer changes and never retags the shared proof",()=>{
  for(const mutation of ["captured","consumed","renderer","host","basis"]){
    const f=heroPageSharedFixture44(),wireProof=f.ledger.reserve(f.f.model,f.dto);assert(wireProof);
    f.scope.heroUiProofLeasesRef.current.set(wireProof,{kind:"window",lease:{origin:"inventory",epoch:1}});
    f.scope.heroProofRendererTokensRef.current.set(wireProof,f.intent.webLeaseToken);f.scope.heroSharedProofsRef.current.set(wireProof,f.intent);
    f.scope.heroProofHighWaterRef.current.set(wireProof,f.raw.ingress.highWater());
    if(mutation==="captured"||mutation==="consumed"){const delivery=f.raw.capture(heroWideOriginal);if(mutation==="consumed")assert(f.raw.ingress.offer(delivery));}
    if(mutation==="renderer")f.scope.heroRendererTokenRef.current="hero-react:1";
    if(mutation==="host")f.invalidate();
    if(mutation==="basis")f.intent.actionBasis={...f.intent.actionBasis,version:99};
    assert.equal(f.api.current(wireProof,wireProof.wire),false);assert.equal(f.ledger.pending.state,"reserved");
    assert.equal(f.ledger.cancelDefinitelyUnsent(wireProof),true);assert.equal(f.ledger.pending,null);
  }
});

console.log(`stage5 adapter tests passed (${passed} groups)`);
