import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import { SPELL_EFFECTS, WORLD_SPELL_EFFECTS } from "./export-crystal-magic-effects.mjs";

const WEB_ROOT = path.resolve(import.meta.dirname, "..");
const REPO_ROOT = path.resolve(WEB_ROOT, "..", "..");
const DATA_ROOT = path.join(REPO_ROOT, "packages", "game-data", "data");
const OUTPUT_ROOT = path.join(REPO_ROOT, "docs", "generated", "player-qa", "skills");
const PUBLIC_ROOT = path.join(WEB_ROOT, "public");

const PROFILE_PATH = path.join(DATA_ROOT, "content_profiles", "platinum_176.json");
const MAGIC_PATH = path.join(DATA_ROOT, "generated", "crystal_magic_manifest.json");
const ITEM_PATH = path.join(DATA_ROOT, "generated", "crystal_item_manifest.json");
const DROP_PATH = path.join(DATA_ROOT, "generated", "crystal_drop_manifest.json");
const PROTOCOL_PATH = path.join(REPO_ROOT, "packages", "protocol", "src", "types.rs");
const RUNTIME_PATH = path.join(REPO_ROOT, "apps", "simulation", "src", "runtime", "skills.rs");
const RUNTIME_TEST_PATH = path.join(
  REPO_ROOT,
  "apps",
  "simulation",
  "src",
  "runtime",
  "tests.rs",
);
const COMBAT_RUNTIME_PATH = path.join(
  REPO_ROOT,
  "apps",
  "simulation",
  "src",
  "runtime",
  "combat.rs",
);
const ZONE_RUNTIME_PATH = path.join(
  REPO_ROOT,
  "apps",
  "simulation",
  "src",
  "runtime",
  "zone",
  "runtime.rs",
);
const SHARED_ZONE_TEST_PATH = path.join(
  REPO_ROOT,
  "apps",
  "simulation",
  "tests",
  "shared_zone.rs",
);
const GATEWAY_ROUTING_PATH = path.join(REPO_ROOT, "apps", "gateway", "src", "routing.rs");

const CLASS_BY_REQUIRED = new Map([
  [1, "Warrior"],
  [2, "Wizard"],
  [4, "Taoist"],
]);
const CLASS_ORDER = new Map([["Warrior", 0], ["Wizard", 1], ["Taoist", 2]]);
const EXPECTED_STRICT_COUNTS = { Warrior: 15, Wizard: 23, Taoist: 25 };
const ACTOR_ACTION_SKILLS = new Set([
  "Fencing",
  "Slaying",
  "Thrusting",
  "HalfMoon",
  "ShoulderDash",
  "FlamingSword",
  "CrossHalfMoon",
  "SpiritSword",
]);
const SOURCE_WITHOUT_EXTRA_MAGIC_SPRITE = new Set(["EnergyShield"]);
const PASSIVE_STAT_SKILLS = new Set(["Fencing", "SpiritSword"]);
const MELEE_ATTACK_SKILLS = new Set([
  "Slaying",
  "Thrusting",
  "HalfMoon",
  "TwinDrakeBlade",
  "FlamingSword",
  "CrossHalfMoon",
]);

function readJson(filePath) {
  return JSON.parse(readFileSync(filePath, "utf8"));
}

function protocolSpellNames(source) {
  const body = source.match(/pub enum Spell\s*\{([\s\S]*?)\n\}/)?.[1];
  assert.ok(body, "protocol Spell enum is missing");
  return new Set([...body.matchAll(/^\s*([A-Za-z][A-Za-z0-9_]*)\s*=\s*\d+,/gm)].map((match) => match[1]));
}

function frameIndices(spec) {
  const indices = [];
  for (let value = 0; value < (spec.valueCount ?? 1); value += 1) {
    for (let direction = 0; direction < (spec.directionCount ?? 1); direction += 1) {
      const base = spec.base
        + value * (spec.valueStride ?? 0)
        + direction * (spec.directionStride ?? 0);
      for (let frame = 0; frame < spec.count; frame += 1) indices.push(base + frame);
    }
  }
  return indices;
}

function phaseSpecs(spec, label) {
  if (!spec) return [];
  return [
    { label: spec.kind ?? label, spec },
    ...["projectile", "impact", "returnEffect"]
      .filter((phase) => spec[phase])
      .map((phase) => ({ label: phase === "returnEffect" ? "return" : phase, spec: spec[phase] })),
  ];
}

function loadEffectMeta(library, cache) {
  if (!cache.has(library)) {
    cache.set(
      library,
      readJson(path.join(PUBLIC_ROOT, "original-effects", library, "meta.json")),
    );
  }
  return cache.get(library);
}

function phaseHasAllAssets(phaseSpec, metaCache) {
  const meta = loadEffectMeta(phaseSpec.library, metaCache);
  return frameIndices(phaseSpec).every((index) => {
    const frame = meta.frames?.[String(index)];
    if (!frame?.path) return false;
    return existsSync(path.join(PUBLIC_ROOT, frame.path.replace(/^\/+/, "")));
  });
}

function accessibleDropSources(profile, dropManifest) {
  const monsters = new Set(profile.monsterWhitelist.map((name) => name.toLowerCase()));
  const byItem = new Map();
  const add = (item, source) => {
    const key = item.toLowerCase();
    if (!byItem.has(key)) byItem.set(key, new Set());
    byItem.get(key).add(source);
  };
  for (const table of dropManifest.tables) {
    const monster = table.table_key.split("/").at(-1);
    if (!monster || !monsters.has(monster.toLowerCase())) continue;
    for (const section of table.sections) {
      for (const entry of section.entries) {
        if (entry.item_name) add(entry.item_name, `${monster} (${table.table_key})`);
      }
    }
  }
  for (const override of profile.dropOverrides) {
    add(override.item, `${override.monster} (profile override)`);
  }
  return byItem;
}

function markdownTable(rows) {
  const lines = [
    "| Scope | Class | Book Lv | Spell | Data/profile | Personal behavior | Shared authority | World book source | Original visual route | Phases |",
    "|---|---|---:|---|---|---|---|---|---|---|",
  ];
  for (const row of rows) {
    lines.push(
      `| ${row.scope === "strict" ? "<=50" : "adjacent"} | ${row.class} | ${row.requiredLevel} | \`${row.spell}\` | ${row.dataComplete ? "PASS" : "FAIL"} | ${row.behaviorCovered ? "PASS" : "FAIL"} | ${row.scope === "strict" ? `${row.sharedAuthorityComplete ? `PASS (${row.gatewayRoute})` : "FAIL"}` : "N/A (outside profile)"} | ${row.scope === "strict" ? `${row.dropSources.length > 0 ? "PASS" : "FAIL"} (${row.dropSources.length})` : "N/A (outside profile)"} | ${row.visualRoute} | ${row.visualPhases.join(", ") || "none"} |`,
    );
  }
  return lines.join("\n");
}

function normalizedTestCorpus(...sources) {
  return sources
    .flatMap((source) => source.split(/(?=#\[(?:tokio::)?test\])/g))
    .filter((chunk) => /^#\[(?:tokio::)?test\]/.test(chunk.trimStart()))
    .join("\n")
    .toLowerCase()
    .replaceAll(/[^a-z0-9]/g, "");
}

function gatewayRouteFor(spell) {
  if (PASSIVE_STAT_SKILLS.has(spell)) return "stat-snapshot";
  if (MELEE_ATTACK_SKILLS.has(spell)) return "melee-attack";
  return "zone-magic";
}

async function main() {
  const profile = readJson(PROFILE_PATH);
  const magicManifest = readJson(MAGIC_PATH);
  const itemManifest = readJson(ITEM_PATH);
  const dropManifest = readJson(DROP_PATH);
  const protocolSource = readFileSync(PROTOCOL_PATH, "utf8");
  const runtimeSource = readFileSync(RUNTIME_PATH, "utf8");
  const runtimeTestSource = readFileSync(RUNTIME_TEST_PATH, "utf8");
  const combatRuntimeSource = readFileSync(COMBAT_RUNTIME_PATH, "utf8");
  const zoneRuntimeSource = readFileSync(ZONE_RUNTIME_PATH, "utf8");
  const sharedZoneTestSource = readFileSync(SHARED_ZONE_TEST_PATH, "utf8");
  const gatewayRoutingSource = readFileSync(GATEWAY_ROUTING_PATH, "utf8");
  const behaviorTestCorpus = runtimeTestSource
    .split(/(?=#\[test\])/g)
    .filter((chunk) => /fn (?:magic_packet_|casting_|attack_packet_|friendly_)/.test(chunk))
    .join("\n")
    .toLowerCase()
    .replaceAll(/[^a-z0-9]/g, "");
  const sharedBehaviorTestCorpus = normalizedTestCorpus(
    combatRuntimeSource,
    zoneRuntimeSource,
    sharedZoneTestSource,
    gatewayRoutingSource,
  );
  const protocolSpells = protocolSpellNames(protocolSource);
  const magicSpells = new Set(magicManifest.magics.map((magic) => magic.spell));
  const profileBySpell = new Map(profile.skills.map((rule) => [rule.spell, rule]));
  const whitelist = new Set(profile.itemWhitelist.map((item) => item.toLowerCase()));
  const dropsByItem = accessibleDropSources(profile, dropManifest);
  const casterBySpell = new Map(SPELL_EFFECTS.map((effect) => [effect.spell, effect]));
  const worldBySpell = new Map(WORLD_SPELL_EFFECTS.map((effect) => [effect.spell, effect]));
  const metaCache = new Map();

  const books = itemManifest.items
    .filter((item) => item.item_type === 20 && item.required_type === 0 && item.required_amount > 0)
    .map((item) => ({ ...item, class: CLASS_BY_REQUIRED.get(item.required_class) }))
    .filter((item) => item.class && item.required_amount <= 53 && magicSpells.has(item.name))
    .sort((left, right) =>
      left.required_amount - right.required_amount
      || CLASS_ORDER.get(left.class) - CLASS_ORDER.get(right.class)
      || left.name.localeCompare(right.name),
    );

  const rows = books.map((book) => {
    const strict = book.required_amount <= profile.acceptanceLevel;
    const rule = profileBySpell.get(book.name);
    const caster = casterBySpell.get(book.name);
    const world = worldBySpell.get(book.name);
    const phases = [...phaseSpecs(caster, "cast"), ...phaseSpecs(world, "world")];
    const visualPhases = [...new Set(phases.map((entry) => entry.label))];
    const atlasComplete = phases.length > 0 && phases.every((entry) => phaseHasAllAssets(entry.spec, metaCache));
    const actorAction = ACTOR_ACTION_SKILLS.has(book.name);
    const noExtraSpriteInSource = SOURCE_WITHOUT_EXTRA_MAGIC_SPRITE.has(book.name);
    const visualRoute = atlasComplete
      ? "source-atlas"
      : actorAction
        ? "actor-action"
        : noExtraSpriteInSource
          ? "source-no-extra-sprite"
          : "missing";
    const dropSources = [...(dropsByItem.get(book.name.toLowerCase()) ?? [])].sort();
    const runtimeReferenced = runtimeSource.includes(`Spell::${book.name}`)
      || runtimeSource.includes(`"${book.name}"`);
    const behaviorCovered = behaviorTestCorpus.includes(book.name.toLowerCase());
    const gatewayRoute = gatewayRouteFor(book.name);
    const sharedRuntimeReferenced = zoneRuntimeSource.includes(`Spell::${book.name}`)
      || ((PASSIVE_STAT_SKILLS.has(book.name) || MELEE_ATTACK_SKILLS.has(book.name))
        && combatRuntimeSource.includes(`Spell::${book.name}`));
    const gatewayRouteCovered = gatewayRoute === "stat-snapshot"
      ? gatewayRoutingSource.includes("ZoneCommand::UpdatePlayerCombatStats")
      : gatewayRoute === "melee-attack"
        ? gatewayRoutingSource.includes("ZoneCommand::PlayerAttackObject")
        : gatewayRoutingSource.includes("ZoneCommand::PlayerCastMagicWithItem");
    const sharedBehaviorCovered = sharedBehaviorTestCorpus.includes(book.name.toLowerCase());
    const sharedAuthorityComplete = sharedRuntimeReferenced
      && gatewayRouteCovered
      && sharedBehaviorCovered;
    const profileComplete = strict
      ? Boolean(
          rule
          && rule.class === book.class
          && rule.requiredLevel === book.required_amount
          && whitelist.has(book.name.toLowerCase()),
        )
      : !rule;
    return {
      scope: strict ? "strict" : "adjacent",
      spell: book.name,
      class: book.class,
      requiredLevel: book.required_amount,
      itemIndex: book.item_index,
      spellId: book.shape,
      magicInfo: magicSpells.has(book.name),
      protocol: protocolSpells.has(book.name),
      runtimeReferenced,
      behaviorCovered,
      gatewayRoute,
      gatewayRouteCovered,
      sharedRuntimeReferenced,
      sharedBehaviorCovered,
      sharedAuthorityComplete,
      profileComplete,
      dataComplete:
        magicSpells.has(book.name)
        && protocolSpells.has(book.name)
        && runtimeReferenced
        && profileComplete,
      functionalComplete:
        magicSpells.has(book.name)
        && protocolSpells.has(book.name)
        && runtimeReferenced
        && behaviorCovered
        && profileComplete
        && (strict ? sharedAuthorityComplete : true)
        && (strict ? dropSources.length > 0 : true),
      dropSources,
      visualRoute,
      visualPhases,
      visualComplete: atlasComplete || actorAction || noExtraSpriteInSource,
    };
  });

  const strictRows = rows.filter((row) => row.scope === "strict");
  const adjacentRows = rows.filter((row) => row.scope === "adjacent");
  const strictCounts = Object.fromEntries(
    Object.keys(EXPECTED_STRICT_COUNTS).map((className) => [
      className,
      strictRows.filter((row) => row.class === className).length,
    ]),
  );
  assert.deepEqual(strictCounts, EXPECTED_STRICT_COUNTS, "source three-class <=50 counts drifted");
  assert.equal(strictRows.length, 63, "strict <=50 source skill count drifted");
  assert.deepEqual(adjacentRows.map((row) => row.spell).sort(), ["IceThrust", "SlashingBurst"]);
  const incompleteStrict = strictRows
    .filter((row) => !row.functionalComplete)
    .map((row) => ({
      spell: row.spell,
      dataComplete: row.dataComplete,
      behaviorCovered: row.behaviorCovered,
      sharedRuntimeReferenced: row.sharedRuntimeReferenced,
      sharedBehaviorCovered: row.sharedBehaviorCovered,
      gatewayRouteCovered: row.gatewayRouteCovered,
      dropSources: row.dropSources.length,
    }));
  assert.deepEqual(incompleteStrict, [], "one or more <=50 skills are not functionally complete");
  assert.ok(strictRows.every((row) => row.visualComplete), "one or more <=50 skills lack a source-faithful visual route");
  assert.ok(adjacentRows.every((row) => row.functionalComplete && row.visualComplete), "level-53 boundary coverage is incomplete");

  const fastMoveBook = itemManifest.items.find((item) => item.name === "FastMove" && item.item_type === 20);
  const summary = {
    strictLevelCap: profile.acceptanceLevel,
    strictTotal: strictRows.length,
    strictByClass: strictCounts,
    adjacentLevel53Total: adjacentRows.length,
    functionalComplete: strictRows.filter((row) => row.functionalComplete).length,
    behaviorCovered: strictRows.filter((row) => row.behaviorCovered).length,
    sharedAuthorityComplete: strictRows.filter((row) => row.sharedAuthorityComplete).length,
    visualRoutes: {
      sourceAtlas: strictRows.filter((row) => row.visualRoute === "source-atlas").length,
      actorAction: strictRows.filter((row) => row.visualRoute === "actor-action").length,
      sourceWithoutExtraSprite: strictRows.filter((row) => row.visualRoute === "source-no-extra-sprite").length,
      missing: strictRows.filter((row) => row.visualRoute === "missing").length,
    },
  };
  const report = {
    schemaVersion: 1,
    generatedAt: null,
    profile: { id: profile.profileId, version: profile.version, acceptanceLevel: profile.acceptanceLevel },
    source: {
      magic: path.relative(REPO_ROOT, MAGIC_PATH),
      items: path.relative(REPO_ROOT, ITEM_PATH),
      drops: path.relative(REPO_ROOT, DROP_PATH),
      protocol: path.relative(REPO_ROOT, PROTOCOL_PATH),
      runtime: path.relative(REPO_ROOT, RUNTIME_PATH),
      runtimeTests: path.relative(REPO_ROOT, RUNTIME_TEST_PATH),
      combatRuntime: path.relative(REPO_ROOT, COMBAT_RUNTIME_PATH),
      zoneRuntime: path.relative(REPO_ROOT, ZONE_RUNTIME_PATH),
      sharedZoneTests: path.relative(REPO_ROOT, SHARED_ZONE_TEST_PATH),
      gatewayRouting: path.relative(REPO_ROOT, GATEWAY_ROUTING_PATH),
      crystalClient: "Crystal/Client/MirObjects/PlayerObject.cs + SpellObject.cs",
    },
    summary,
    sourceDisabled: [
      {
        spell: "FastMove",
        bookLevel: fastMoveBook?.required_amount ?? null,
        reason: "The Crystal MagicInfo initializer is commented out and is not an active source spell.",
      },
    ],
    rows,
  };

  const markdown = `# Three-class original skill audit through level 50\n\n`
    + `Deterministic audit of the active Crystal/Jev three-class books, the \`${profile.profileId}\` v${profile.version} profile, protocol/runtime coverage, accessible world book sources, and source-derived visual assets.\n\n`
    + `## Result\n\n`
    + `- Strict <=${profile.acceptanceLevel}: **${summary.functionalComplete}/${summary.strictTotal} automated implementation gates pass** (${strictCounts.Warrior} Warrior / ${strictCounts.Wizard} Wizard / ${strictCounts.Taoist} Taoist).\n`
    + `- Personal-session behavior regressions: **${summary.behaviorCovered}/${summary.strictTotal}**.\n`
    + `- Shared-Zone authority + gateway route regressions: **${summary.sharedAuthorityComplete}/${summary.strictTotal}**.\n`
    + `- Visual routes: **${summary.visualRoutes.sourceAtlas} source-atlas**, **${summary.visualRoutes.actorAction} actor-action**, **${summary.visualRoutes.sourceWithoutExtraSprite} source-without-extra-sprite**, **${summary.visualRoutes.missing} missing**.\n`
    + `- Level-53 boundary: **${summary.adjacentLevel53Total}/2** (SlashingBurst and IceThrust) retained outside the level-50 profile but covered by protocol/runtime/source visuals.\n`
    + `- FastMove is excluded: its Crystal MagicInfo initializer is commented out; treating its placeholder as active would fabricate source content.\n\n`
    + `An "automated implementation gate" requires source book + MagicInfo + protocol id + personal-session behavior + shared-Zone implementation and regression + gateway route + exact profile gate + an accessible profile-world book source. It does not replace physical-device or human combat-feel acceptance.\n\n`
    + `## Per-skill matrix\n\n${markdownTable(rows)}\n`;

  await mkdir(OUTPUT_ROOT, { recursive: true });
  const jsonPath = path.join(OUTPUT_ROOT, "three-class-level-50-audit.json");
  const markdownPath = path.join(OUTPUT_ROOT, "three-class-level-50-audit.md");
  await writeFile(jsonPath, `${JSON.stringify(report, null, 2)}\n`);
  await writeFile(markdownPath, markdown);
  console.log(`three-class skill audit passed: ${summary.strictTotal}/63 <=50; ${summary.adjacentLevel53Total}/2 adjacent; visual missing=${summary.visualRoutes.missing}`);
  console.log(path.relative(REPO_ROOT, markdownPath));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
