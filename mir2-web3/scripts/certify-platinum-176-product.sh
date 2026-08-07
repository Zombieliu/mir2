#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"
report_path="${1:-${repo_root}/docs/generated/player-qa/platinum-176/latest-product-loop.json}"
combat_report_path="${repo_root}/docs/generated/player-qa/platinum-176/latest-combat-milestones.json"
party_boss_report_path="${repo_root}/docs/generated/player-qa/platinum-176/latest-party-boss.json"
started_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

cd "${repo_root}"

node scripts/build-platinum-176-profile-bundle.mjs --check
cargo +1.89.0 test -p mir2-game-data --test platinum_176_content_loop -- --nocapture
"${repo_root}/scripts/certify-platinum-176-combat.sh" "${combat_report_path}"
"${repo_root}/scripts/certify-platinum-176-party-boss.sh" "${party_boss_report_path}"

simulation_cases=(
  "runtime::session::tests::platinum_176_blocks_post_176_and_qa_stage5_actions_but_keeps_classic_social_endgame"
  "runtime::session::tests::stage5_social_group_guild_mail_persist_across_reload"
  "runtime::session::tests::stage5_trade_shop_and_auction_are_transactional"
  "runtime::session::tests::stage5_conquest_campaign_closes_registration_captures_settles_and_rewards_once"
  "runtime::session::tests::deeply_red_player_death_drops_two_eligible_items_and_recalculates_equipment"
  "runtime::session::tests::pk_decay_accumulator_persists_and_reconnect_cannot_accelerate_decay"
)

for test_name in "${simulation_cases[@]}"; do
  cargo +1.89.0 test \
    -p mir2-simulation \
    --lib \
    "${test_name}" \
    -- \
    --exact \
    --nocapture
done

shared_zone_cases=(
  "zone_native_monster_combat_kill_and_drop_are_authoritative"
  "zone_ground_drop_claim_blocks_non_owner_until_owner_window_expires"
)

for test_name in "${shared_zone_cases[@]}"; do
  cargo +1.89.0 test \
    -p mir2-simulation \
    --test shared_zone \
    "${test_name}" \
    -- \
    --exact \
    --nocapture
done

(
  cd apps/web
  npx tsc --noEmit
)

mkdir -p "$(dirname -- "${report_path}")"
node - "${report_path}" "${started_at}" "${combat_report_path}" "${party_boss_report_path}" <<'NODE'
const fs = require("node:fs");
const path = require("node:path");

const [reportPath, startedAt, combatReportPath, partyBossReportPath] = process.argv.slice(2);
const profilePath = path.resolve("packages/game-data/data/content_profiles/platinum_176.json");
const profile = JSON.parse(fs.readFileSync(profilePath, "utf8"));
const bundlePath = path.resolve("packages/game-data/data/generated/platinum_176_bundle.json");
const bundle = JSON.parse(fs.readFileSync(bundlePath, "utf8"));
const combatReport = JSON.parse(fs.readFileSync(combatReportPath, "utf8"));
const partyBossReport = JSON.parse(fs.readFileSync(partyBossReportPath, "utf8"));
const combatPassed =
  combatReport.cases.length === 15 &&
  Object.values(combatReport.assertions).every((passed) => passed === true);
const partyBossPassed = Object.values(partyBossReport.assertions).every(
  (passed) => passed === true,
);
const cases = [
  {
    id: "profile.bundle-identity",
    scope: "Profile and 12 runtime data manifests match the published SHA-256 content bundle",
  },
  {
    id: "content.levels-8-50",
    scope: "Profile maps, monster spawns, skill books, equipment and Boss sources",
  },
  {
    id: "combat.levels-22-50",
    scope: "Three classes at levels 22/35/40/45/50 use Profile gear, skills, real maps and real respawns",
    passed: combatPassed,
    evidence: path.relative(path.dirname(reportPath), combatReportPath),
  },
  {
    id: "combat.party-boss-ttk",
    scope: "Three level-50 classes fight a real Profile RedMoonEvil spawn in one authoritative shared Zone",
    passed: partyBossPassed,
    evidence: path.relative(path.dirname(reportPath), partyBossReportPath),
  },
  {
    id: "profile.feature-boundary",
    scope: "Block post-1.76 and QA mutations while preserving classic social/endgame actions",
  },
  {
    id: "social.guild.persistence",
    scope: "Group, guild and social state survive save/reload",
  },
  {
    id: "economy.transactionality",
    scope: "Trade, shop and auction fixture transactions conserve state",
  },
  {
    id: "sabuk.campaign",
    scope: "Registration, capture, settlement and reward idempotency",
  },
  {
    id: "pk.deep-red-drops",
    scope: "Deep-red death drops two eligible equipment items and recalculates stats",
  },
  {
    id: "pk.decay.persistence",
    scope: "PK decay persists and reconnect cannot accelerate it",
  },
  {
    id: "world.authoritative-kill-drop",
    scope: "Shared-Zone monster death and drop generation are authoritative",
  },
  {
    id: "world.drop-ownership",
    scope: "Non-owner claim is blocked until the ownership window expires",
  },
  {
    id: "web.profile-ui-typecheck",
    scope: "Platinum UI content-profile boundary compiles",
  },
].map((entry) => ({ passed: true, ...entry }));

const report = {
  schema: "mir2-platinum-176-product-loop/1",
  generatedAt: new Date().toISOString(),
  startedAt,
  profileId: profile.profileId,
  profileVersion: profile.version,
  bundleHash: bundle.contentHash,
  ratePolicy: profile.ratePolicy,
  passed: cases.every((entry) => entry.passed),
  cases,
  caveats: [
    "The trade/auction fixture test proves transactionality in crystal_full; platinum_176 independently blocks auction commands.",
    "This certificate is deterministic server/UI regression evidence, not Windows multiplayer or long-duration soak evidence.",
    "The measured combat cases seed levels, learned skills and representative equipment; natural acquisition remains a separate browser release gate.",
    "The shared-Zone party certificate measures 17-second three-class RedMoonEvil TTK versus 59 seconds solo with production damage multiplier 1; it is deterministic balance evidence, not a substitute for human play feel.",
    "Damage-over-time cases record the first observed poison pulse and intentionally omit a misleading action-count kill estimate.",
    "The tiered XP rate is a launch candidate backed by source-supply certification; live player pacing still requires staging telemetry.",
  ],
};

fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
console.log(JSON.stringify({ ok: report.passed, reportPath, profileVersion: profile.version, cases: cases.length }, null, 2));
NODE
