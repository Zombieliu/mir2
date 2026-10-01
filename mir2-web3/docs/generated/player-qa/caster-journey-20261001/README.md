# Wizard / Taoist newcomer QA — 2026-10-01

This checks the user-requested caster journeys and medicine/supply UI. It does
not replace the user's accepted Warrior save or claim whole-game acceptance.

## Actual journey evidence

The first independent ordinary cohort ran source `b39cf9b38` with the Gateway
hash in [source.json](source.json). Normal NewAccount/Login/NewCharacter,
Walk/Run/Magic/Attack/NPC/item/quest packets were used; no grants, admin commands,
save edits, accelerated damage, reward rates, movement or respawn were used.
Its read-only [saved audit](caster-audit-first.json) verifies **33/52** nodes:
Wizard 21/26 at level 26, Taoist 12/26 at level 19. Both normal logout saves
match their final authoritative frames. Neither class completed the route.

The ordinary clocks and the separately labelled functional recheck clock
expired. The latter is retained in [functional-recheck-clock.json](functional-recheck-clock.json).
Neither clock nor death/revival ledger was reset. Complete raw traces and
private stores remain local and are deliberately excluded from Git.

A second independent fresh cohort is running against a copied, hash-bound
candidate Gateway after the corrections below. It has separate accounts,
stores and ordinary clocks. Completion, level 30 and normal saved logout are
still pending; the first cohort is not reclassified as successful.

## Corrections supported by the first run

- The native skills panel's F-key ground/directional/self-summon cast lost its
  map coordinate. Native input now retains cursor aim inside the map viewport
  through ordinary panels, including 1×/1.5×/2× stage transforms. Prompts/NPC
  capture, cast admission and out-of-stage guards remain enforced.
- Periodic shared snapshots exposed buried Zombie2/private plants before a
  real Crystal reveal, permitting a visible/selectable target the server
  correctly rejected. Gateway projection now consults authoritative Zone
  visibility before copying actors into a player's AOI. Corpses, stone Zuma,
  owned summons, removal ledgers and unknown-actor compatibility are retained.
- Old-map Struck/Poisoned/health notifications could reach a player after a
  TownTeleport bootstrap. The map boundary discards queued old viewport
  presentations while preserving personal settlement receipts. Online transfer
  carries finite control-poison deadlines for the same account/character/life
  and calibrates destination poison, including zero, without extending time.
- Healing's advertised cost was 6 while actual level-zero shared casting
  consumed 3 MP. Canonical BaseCost + LevelCost × Level now drives both UI
  affordability and personal preflight/debit. Non-Crystal generic fallback and
  unknown/negative-cost rejection remain explicit.

Controller-only corrections also retain all gameplay limits: refresh spell
readiness after authoritative movement; avoid buried targets until an actual
reveal; recognize all three authored training actors; retreat from unrelated
search aggressors; cap optional top-ups by real free weight after mandatory
minimum purchases; and use bounded, learned Taoist self-Healing after a safe
retreat. Two group neighbors are admissible for the existing three-actor
Temple training group, while adjacent-hostile, HP, 20-action, 48-engagement,
navigation, deadline and revival gates remain unchanged.

## Native UI and regression evidence

The [visual manifest](visual/caster-shop-layouts.json) checks seven caster supply
services at levels 9/28, Buy and Sell modes, zh-TW/en/pt-BR: **42 real offscreen
GPU captures**, with no layout failures. Service-open, imported item selection,
quantity, single purchase intent, tab switch, close/reopen and saved bag-layout
restoration run through production UI/ECS bindings. These are synthetic service
fixtures, not authenticated human gameplay acceptance.

| Validation | Result and scope |
| --- | --- |
| Shared client, complete serial run | 1187 passed / 9 ignored |
| Native client, complete final serial run | 806 passed / 5 ignored |
| Native panel aim reproduction | 9 passed / 1 failed before; 10/10 after |
| Native buried-monster replay | 3/3; production adapter/input/render selection, CPU geometry |
| Caster shop behavior | 5/5, including seven imported service cases |
| Controller and strict saved-evidence regressions | 386/386 after final group/readiness changes |
| Private visibility | 0/4 before; 4/4 after |
| Gateway map-boundary status | 0/3 before; 3/3 after |
| Shared finite control handoff | 0/2 before; 5/5 after; counted-boundary 1/1 |
| Canonical mana | Metadata 2/3 and projection 1/5 before; 3/3 + 5/5 after |
| Final Gateway adjacent validation | 33/33, plus 7/7 isolated V2 bridge checks |
| Earlier complete Gateway run | 857 passed / 18 ignored; predates the late status/mana slice |
| Complete Simulation run | 1906 passed / 5 failed / 22 ignored; five old fixtures corrected below |
| Simulation fixture correction | Shinsu 2/2, line attack 1/1, town/bind 3/3 after |

The complete Simulation failures are preserved. Three old seeded Shinsu tests
provided only Scout's 35 MP for a level-three 40-MP summon; their fixture now
prepares canonical mana and verifies cast/MP debit. Two old TownTeleport tests
expected the spawn tile despite the imported safe-area binding center; they
now check that center, as TownRevive already did. Production mana/binding rules
were not rolled back. The clean full run is not relabelled as passing; these
focused results resolve its five failures.

## Second-cohort controller wire correction

The r2 ordinary traces exposed a QA-only gap: Wizard N8 and Taoist N11 sent
GreatFireBall/SoulFireBall 42–45 ms after the last movement, without a fresh
shared readiness frame. No Magic ACK or mana debit followed; both targets
remained stationary and unharmed. [Selected real receipts](practice-owner-wire-receipts.json)
retain the unextended pauses.

Crystal owner-only `UserLocation` serializes position and direction, without
an `objectId` (`Shared/ServerPackets.cs:870`). The earlier test invented that
field and therefore missed the live branch. The corrected real-wire test
fails before the controller fix; the 386-case suite passes afterward. Owner
receipts without an id now invalidate stale readiness; an explicitly foreign
id remains excluded. No server cooldown or projectile rule changed. Both
same accounts resumed through normal login within their original r2 clock.

## Limits

- The native buried replay binds original packet/snapshot sequence numbers.
  Its filtered frame and remove/map controls are explicitly synthetic; real
  reveal belongs to a different actor. It is not a GPU/human acceptance test.
- Only finite online control-poison handoff is corrected here. Counted or
  periodic poison leases remain a separate parity gap. Fresh Zone-login clock
  isolation is not a claim that Crystal logout clears saved poison.
- Canonical base/level mana is covered; original dynamic mana penalties and
  special teleport surcharges are not claimed complete.
- Native full-route visual play, laptop DPI and production soak remain open.
  Public playtest, installed r8, updater feed and human stores were not changed
  during this local caster round. Capacity work remains paused.
