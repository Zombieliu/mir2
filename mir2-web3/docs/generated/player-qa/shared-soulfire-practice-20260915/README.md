# Shared SoulFireBall practice — 2026-09-15

Implementation was uncommitted at worker completion. Coordinator HEAD at final
verification was `bb65057de477854a4f2c8bd23119c3e21d070ba4`.

This change closes the shared-Zone SoulFireBall practice omission. It is limited
to one positive resolved native-monster hit per accepted SoulFireBall cast. It
does not grant historical XP, adjust account files, raise SC, or alter the
existing personal-session practice trigger. Live journey confirmation awaits
the coordinator's R53 deployment at a safe Taoist checkpoint.

## Source and observed defect

Local Crystal reference: `../Crystal/Server/MirObjects/HumanObject.cs` delayed
SoulFireBall branch around line 5919 and `LevelMagic` around line 6903. Crystal
validates the target's attack eligibility, map/node, and distance within two
tiles of the launch target position, then calls `LevelMagic` only when
`target.Attacked(...) > 0`. The delayed-hit branch does not reject a caster
merely because the caster died after lawful launch.

`LevelMagic` gives 1–3 practice XP times the skill-gain multiplier, applies the
character-level gates and practice thresholds, and sends `MagicLeveled`. Current
SoulFireBall manifest gates are levels 18/21/24 and thresholds 1300/2700/4000.
Rust's existing `advance_magic_progression` supplies the same bounded gain,
multiplier, gates, threshold carry, delay update, and packet behavior.

Before the fix, shared casting called `apply_zone_player_magic_spend` for MP and
cooldown but never forwarded successful resolved hits to personal progression.
Snapshots and saved `skill_states_json` both explicitly showed SoulFireBall
level 0, experience 0, so this was not an omitted snapshot field.

Read-only journey evidence:

- `C:/mir2-protocol-journey-20260911/Taoist.2026-09-15T04-32-21-719Z.trace.jsonl`
  (R99): 42 accepted SoulFireBall casts and 42 matched positive damage events;
  no `MagicLeveled`. First accepted cast seq 978 → damage seq 985; last accepted
  cast seq 2483 → damage seq 2502. All matched damage values were 7.
- `C:/mir2-protocol-journey-20260911/Taoist.2026-09-15T05-00-34-648Z.trace.jsonl`
  (R100 audit interval): eight accepted casts and eight matched positive hits;
  damage 7 for seven hits and lethal remainder 5 for one. First cast seq 550 →
  damage seq 564; last matched damage seq 768. Skill snapshots seq 546 and
  seq 6941 explicitly retained level 0 / experience 0; no `MagicLeveled`.
- `C:/mir2-protocol-journey-20260911/gateway/accounts.json`, account
  `jp3f4a4e1f`, character index 10, revision 8162 at the final read-only audit:
  durable SoulFireBall level 0 / experience 0 and MinorHeal level 3 / experience
  0. No account writes were performed during this work.

Fifty legitimate positive hits would give 50–150 XP at multiplier 1. They would
not reach the 1300 XP threshold. Damage 7 against WoomaSoldier follows the
existing level-0 spell power, SC 4, and MAC 2; this fix does not immediately
increase that damage.

## Implementation boundary

- `apps/simulation/src/runtime/zone/types.rs`: typed
  `ZoneSoulFirePracticeReceipt` / `ZoneOutbound::SoulFirePractice`.
- `apps/simulation/src/runtime/zone/runtime.rs`: stamp accepted primary
  SoulFireBall cast metadata, validate the original caster presence and target,
  and emit the receipt only after positive actual HP loss. Lethal remainder
  counts; zero damage and invalid, dead, or displaced targets do not.
- `apps/simulation/src/runtime/session.rs` and `world_runtime.rs`: trusted
  authenticated-character bridge to existing `advance_magic_progression`.
  No additional MP charge, launch reward, caster-dead restriction, or XP boost.
- `apps/gateway/src/routing.rs`: owner queue, source session/account/character/
  object/map/life validation, cast dedupe, owner-only `MagicLeveled`, and drain
  before the normal teardown checkpoint. Reconnect, map change, or revival
  invalidates old receipts. A logout fence rejects new side effects while
  allowing previously resolved receipts to drain into the final save.
- Internal shared checkpoints preserve pending receipts and dedupe progress
  with serde defaults for old images; account schema remains unchanged.
- Mechanical public exports in `lib.rs`, `runtime/mod.rs`, and `zone/mod.rs`;
  `soulfire_practice: None` in existing pet-special and Shinsu hit constructors
  keeps their behavior unchanged.

Cast identity is original online object/life plus `cast_at_ms`; authoritative
SoulFireBall cooldown permits only one accepted cast at a timestamp. The
gateway rejects duplicate queued receipts and later replay. It never routes
the caster's progression to observers.

## Verification

Commands run from the repository root with Rust 1.95.0:

```powershell
cargo +1.95.0 check -p mir2-simulation -p mir2-gateway --lib
cargo +1.95.0 test -p mir2-simulation soulfire_practice --lib
cargo +1.95.0 test -p mir2-gateway zone_soulfire_practice --lib
cargo +1.95.0 test -p mir2-simulation magic_packet_crystal_skill_gain_multiplier_scales_practice_experience --lib
rustfmt +1.95.0 --edition 2021 apps/simulation/src/runtime/zone/runtime/soulfire_practice_tests.rs apps/gateway/src/routing/tests/zone_soulfire_practice_tests.rs
```

Exact logs in this directory:

- `check.log`: passed simulation and gateway library check.
- `simulation-tests.log`: 6/6 passed, covering resolved positive/lethal hits,
  zero/MAC-absorbed/missing/dead/moved targets, changed object/life, lawful
  caster death, departed owner, no extension to FireBall, and pending cast
  metadata surviving a strict Zone checkpoint.
- `gateway-tests.log`: 8/8 passed, including shared positive hit → owner
  `MagicLeveled` level 1 → normal logout/reload → public `Magic` and
  `ObjectMagic` level 1; owner isolation, invalid/stale/deduplicated receipts,
  checkpoint replay protection, character-level gate, and final save drain.
- `progression-multiplier-tests.log`: existing 3× skill-gain multiplier
  regression passed (1/1).
- `fmt-check.log`: focused rustfmt check passed for both added test files.

The added strict Zone checkpoint test initially used an unbounded test collision
for map `0`. Strict restore reconstructs authoritative map collision and correctly
rejected that different root. The failure is preserved in
`simulation-tests-fixture-collision-failure.log`; the fixture now uses the same
authoritative collision as restore. No checkpoint verification policy was
weakened or production checkpoint code changed.

Gateway tests use isolated in-memory account fixtures and cover owner-only
progression, duplicate queue/replay, invalid identity, stale queued reconnect/
revival, shared checkpoint replay protection, level gate, logout fence and
normal teardown checkpoint, real shared positive hit, normal logout/reload,
and subsequent public `Magic` / `ObjectMagic` at level 1.

Only newly added tests and inserted method blocks were formatted, avoiding
unrelated formatting of existing compact modules. No gateway restart, native
input, production account modification, commit, or push was performed.

## Remaining acceptance

R53 must show a natural positive SoulFireBall hit followed by owner
`MagicLeveled` and durable nonzero experience, with the same value after a safe
checkpoint/restart. Subsequent casts must use any legitimately earned new
skill level. Other shared spells and native visual/sustained-run acceptance
are not certified by this bounded change. The coordinator owns integration
and the required roadmap/backend-progress/server-parity document updates.
