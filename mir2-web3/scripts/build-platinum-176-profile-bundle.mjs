import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputPath = path.resolve(
  repoRoot,
  process.env.MIR2_PROFILE_BUNDLE_OUT ??
    "packages/game-data/data/generated/platinum_176_bundle.json",
);
const checkMode = process.argv.includes("--check");
const dependencyPaths = [
  "packages/game-data/data/content_profiles/platinum_176.json",
  "packages/game-data/data/generated/crystal_respawn_manifest.json",
  "packages/game-data/data/generated/crystal_monster_manifest.json",
  "packages/game-data/data/generated/crystal_monster_ai_summary.json",
  "packages/game-data/data/generated/crystal_item_manifest.json",
  "packages/game-data/data/generated/crystal_random_item_stats_manifest.json",
  "packages/game-data/data/generated/crystal_magic_manifest.json",
  "packages/game-data/data/generated/crystal_buff_manifest.json",
  "packages/game-data/data/generated/crystal_drop_manifest.json",
  "packages/game-data/data/generated/crystal_npc_manifest.json",
  "packages/game-data/data/generated/crystal_npc_info_manifest.json",
  "packages/game-data/data/generated/crystal_npc_command_summary.json",
  "packages/game-data/data/generated/crystal_base_stats_packet_manifest.json",
];

const files = [];
for (const relativePath of dependencyPaths) {
  const bytes = await fs.readFile(path.join(repoRoot, relativePath));
  files.push({
    path: relativePath,
    bytes: bytes.length,
    sha256: sha256(bytes),
  });
}

const contentHashInput = files
  .map((file) => `${file.path}\0${file.sha256}\n`)
  .join("");
const profile = JSON.parse(
  await fs.readFile(path.join(repoRoot, dependencyPaths[0]), "utf8"),
);
const respawns = JSON.parse(
  await fs.readFile(
    path.join(
      repoRoot,
      "packages/game-data/data/generated/crystal_respawn_manifest.json",
    ),
    "utf8",
  ),
);
const builtAt = buildTimestamp();
const bundle = {
  schema: "mir2-content-profile-bundle/1",
  profileId: profile.profileId,
  profileVersion: profile.version,
  acceptanceLevel: profile.acceptanceLevel,
  source: profile.source,
  sourceData: {
    crystalDatabaseVersion: respawns.crystal_db_version,
    crystalDatabaseCustomVersion: respawns.crystal_db_custom_version,
  },
  builtAt,
  hashAlgorithm: "sha256",
  contentHash: sha256(Buffer.from(contentHashInput)),
  files,
  summary: {
    maps: profile.mapWhitelist.length,
    monsters: profile.monsterWhitelist.length,
    items: profile.itemWhitelist.length,
    skills: profile.skills.length,
    npcScripts: profile.npcScriptWhitelist.length,
    dropOverrides: profile.dropOverrides.length,
  },
};

if (checkMode) {
  const existing = JSON.parse(await fs.readFile(outputPath, "utf8"));
  const expected = { ...bundle, builtAt: existing.builtAt };
  if (JSON.stringify(existing) !== JSON.stringify(expected)) {
    throw new Error(
      `Profile bundle is stale: run node scripts/build-platinum-176-profile-bundle.mjs`,
    );
  }
  console.log(
    JSON.stringify(
      {
        ok: true,
        outputPath,
        profileId: bundle.profileId,
        profileVersion: bundle.profileVersion,
        files: bundle.files.length,
        contentHash: bundle.contentHash,
      },
      null,
      2,
    ),
  );
} else {
  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  await fs.writeFile(outputPath, `${JSON.stringify(bundle, null, 2)}\n`);
  console.log(JSON.stringify(bundle, null, 2));
}

function sha256(value) {
  return crypto.createHash("sha256").update(value).digest("hex");
}

function buildTimestamp() {
  const explicit = process.env.MIR2_PROFILE_BUNDLE_BUILT_AT;
  if (explicit) {
    const date = new Date(explicit);
    if (Number.isNaN(date.valueOf())) {
      throw new Error("MIR2_PROFILE_BUNDLE_BUILT_AT must be an ISO-8601 timestamp");
    }
    return date.toISOString();
  }
  const epoch = process.env.SOURCE_DATE_EPOCH;
  if (epoch) {
    const seconds = Number(epoch);
    if (!Number.isFinite(seconds) || seconds < 0) {
      throw new Error("SOURCE_DATE_EPOCH must be a non-negative number");
    }
    return new Date(seconds * 1000).toISOString();
  }
  return new Date().toISOString();
}
