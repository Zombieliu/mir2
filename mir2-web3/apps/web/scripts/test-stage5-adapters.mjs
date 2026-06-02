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
  // onlineOnly always wins.
  assert.equal(rankingTabKey(3, true), "online");
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
  assert.equal(onlineResult.tab, "online", "onlineOnly state resolves to the online tab");
});

check("adaptActiveRankingPage falls back to overall/null for missing key", () => {
  assert.deepEqual(adaptActiveRankingPage(null, "1:all"), { tab: "overall", page: null });
  assert.deepEqual(adaptActiveRankingPage({}, null), { tab: "overall", page: null });
  assert.deepEqual(adaptActiveRankingPage({ "1:all": {} }, "nope"), {
    tab: "overall",
    page: null,
  });
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

console.log(`stage5 adapter tests passed (${passed} groups)`);
