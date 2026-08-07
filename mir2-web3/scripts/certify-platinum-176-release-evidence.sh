#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"
report_path="${1:-${repo_root}/docs/generated/player-qa/platinum-176/latest-release-evidence.json}"
started_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

case "${report_path}" in
  /*) ;;
  *) report_path="${repo_root}/${report_path}" ;;
esac

cd "${repo_root}"

run_lib_case() {
  local test_name="$1"
  local full_name="runtime::session::tests::${test_name}"
  local output
  if ! output="$(
    cargo +1.89.0 test \
      -p mir2-simulation \
      --lib \
      "${full_name}" \
      -- \
      --exact \
      --nocapture 2>&1
  )"; then
    printf '%s\n' "${output}"
    return 1
  fi
  printf '%s\n' "${output}"
  if [[ "${output}" != *"test ${full_name} ... ok"* ]]; then
    echo "expected exactly named lib test did not run: ${full_name}" >&2
    return 1
  fi
}

run_zone_case() {
  local test_name="$1"
  local output
  if ! output="$(
    cargo +1.89.0 test \
      -p mir2-simulation \
      --test shared_zone \
      "${test_name}" \
      -- \
      --exact \
      --nocapture 2>&1
  )"; then
    printf '%s\n' "${output}"
    return 1
  fi
  printf '%s\n' "${output}"
  if [[ "${output}" != *"test ${test_name} ... ok"* ]]; then
    echo "expected exactly named shared-zone test did not run: ${test_name}" >&2
    return 1
  fi
}

# Real shared-zone combat: the poison case runs through the final tick, death,
# kill award and ground drop. The summon cases verify owned pets attack hostile
# monsters without damaging players.
combat_cases=(
  "zone_native_player_poison_shot_ticks_green_damage_and_awards_kill"
  "zone_native_summon_attacks_hostile_monster_for_owner_without_hitting_players"
  "zone_native_summon_shinsu_spawns_owned_pet_and_attacks_hostile_monster"
  "zone_native_monster_combat_kill_and_drop_are_authoritative"
)
for test_name in "${combat_cases[@]}"; do
  run_zone_case "${test_name}"
done

# Multi-session visibility and classic endgame/social state. These are real
# server commands and state transitions, though still deterministic automation
# rather than a scheduled human event.
social_cases=(
  "zone_guild_chat_routes_same_guild"
  "conquest_archer_guard_ignores_defender_guild_and_attacks_enemy_guild"
  "zone_ground_drop_claim_blocks_non_owner_until_owner_window_expires"
)
for test_name in "${social_cases[@]}"; do
  run_zone_case "${test_name}"
done

social_lib_cases=(
  "stage5_social_group_guild_mail_persist_across_reload"
  "guild_war_return_starts_two_guild_war_after_cost_and_blocks_duplicate_rollback"
  "stage5_conquest_campaign_closes_registration_captures_settles_and_rewards_once"
  "deeply_red_player_death_drops_two_eligible_items_and_recalculates_equipment"
  "pk_decay_accumulator_persists_and_reconnect_cannot_accelerate_decay"
)
for test_name in "${social_lib_cases[@]}"; do
  run_lib_case "${test_name}"
done

# Gold/item conservation, potion recovery and both field-oil/NPC durability
# sinks are kept as separate gates so a green trade test cannot hide a broken
# consumable or repair loop.
economy_cases=(
  "stage5_trade_shop_and_auction_are_transactional"
  "use_item_packet_dynamic_crystal_sun_potion_applies_template_hp_and_mp"
  "use_item_packet_dynamic_crystal_normal_potion_queues_timed_restore"
  "repair_and_war_god_oil_emit_item_repaired_for_weapon"
  "repair_item_packet_repairs_inventory_unique_id_with_cost_and_max_dura_loss"
)
for test_name in "${economy_cases[@]}"; do
  run_lib_case "${test_name}"
done

social_economy_output="$(
  cargo +1.89.0 test \
    -p mir2-simulation \
    --test social_economy_integration \
    -- \
    --nocapture 2>&1
)"
printf '%s\n' "${social_economy_output}"
if [[ "${social_economy_output}" != *"test result: ok. 3 passed; 0 failed"* ]]; then
  echo "social/economy integration suite did not run all three expected cases" >&2
  exit 1
fi

mkdir -p "$(dirname -- "${report_path}")"
node - "${report_path}" "${started_at}" <<'NODE'
const fs = require("node:fs");

const [reportPath, startedAt] = process.argv.slice(2);
const categories = [
  {
    id: "combat.native-dot-kill",
    cases: ["zone_native_player_poison_shot_ticks_green_damage_and_awards_kill"],
    claim: "Poison reaches authoritative death, kill award and ground drop.",
  },
  {
    id: "combat.native-summons",
    cases: [
      "zone_native_summon_attacks_hostile_monster_for_owner_without_hitting_players",
      "zone_native_summon_shinsu_spawns_owned_pet_and_attacks_hostile_monster",
    ],
    claim: "Skeleton and Shinsu attack hostile monsters without friendly-fire damage.",
  },
  {
    id: "combat.authoritative-kill-drop",
    cases: ["zone_native_monster_combat_kill_and_drop_are_authoritative"],
    claim: "Shared-zone combat owns monster death and drop generation.",
  },
  {
    id: "multiplayer.social-and-endgame",
    cases: [
      "zone_guild_chat_routes_same_guild",
      "conquest_archer_guard_ignores_defender_guild_and_attacks_enemy_guild",
      "stage5_social_group_guild_mail_persist_across_reload",
      "guild_war_return_starts_two_guild_war_after_cost_and_blocks_duplicate_rollback",
      "stage5_conquest_campaign_closes_registration_captures_settles_and_rewards_once",
    ],
    claim: "Multi-session guild visibility, guild war and Sabuk lifecycle remain coherent.",
  },
  {
    id: "pk-and-loot-ownership",
    cases: [
      "zone_ground_drop_claim_blocks_non_owner_until_owner_window_expires",
      "deeply_red_player_death_drops_two_eligible_items_and_recalculates_equipment",
      "pk_decay_accumulator_persists_and_reconnect_cannot_accelerate_decay",
    ],
    claim: "Loot ownership, red-name death loss and reconnect-safe PK decay are enforced.",
  },
  {
    id: "economy.transaction-and-mail",
    cases: [
      "stage5_trade_shop_and_auction_are_transactional",
      "social_economy_integration",
    ],
    claim: "Fixture trade/shop/auction and exact mail parcel operations conserve state.",
  },
  {
    id: "economy.consumables-and-repair",
    cases: [
      "use_item_packet_dynamic_crystal_sun_potion_applies_template_hp_and_mp",
      "use_item_packet_dynamic_crystal_normal_potion_queues_timed_restore",
      "repair_and_war_god_oil_emit_item_repaired_for_weapon",
      "repair_item_packet_repairs_inventory_unique_id_with_cost_and_max_dura_loss",
    ],
    claim: "Instant/timed recovery and both repair paths consume the expected resources.",
  },
].map((entry) => ({ ...entry, passed: true }));

const report = {
  schema: "mir2-platinum-176-release-evidence/1",
  generatedAt: new Date().toISOString(),
  startedAt,
  passed: categories.every((entry) => entry.passed),
  totalCases: categories.reduce((sum, entry) => sum + entry.cases.length, 0),
  categories,
  caveats: [
    "This is deterministic server evidence, not a substitute for human multiplayer acceptance.",
    "The summon cases prove owned-pet attacks and safety; representative live party TTK is still a separate balance gate.",
    "Potion, repair and transaction correctness do not by themselves lock seven-day inflation or live-player consumption rates.",
    "Levels 22-50 natural browser acquisition remains a separate release gate and is not claimed by this report.",
  ],
};

fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
console.log(
  JSON.stringify(
    {
      ok: report.passed,
      reportPath,
      categories: report.categories.length,
      totalCases: report.totalCases,
    },
    null,
    2,
  ),
);
NODE
