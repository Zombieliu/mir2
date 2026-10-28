# Poisoning carried-poison rule — 2026-10-02

The user explicitly requested that **Poisoning / 施毒术** consume poison from
the bag or belt without equipping it. This is a project-specific rule that
differs from the previous equipped-only behavior; it is not a new Crystal 1:1
parity claim.

The implementation starts from native-shop-resources source
`9cf50711ad3810fbb7c3fd6cdafa69c2cd44e8d6` in the separate
`codex/poisoning-carried-items` worktree.

## Selection and consumption

For Poisoning only, one selector supplies admission, the economy component
key, the shared Zone `item_param` colour and the actual inventory debit. Each
accepted cast consumes one unit from the selected stack and sends that stack's
`DeleteItem` reference. Equipped references keep their previous protocol slot
identity; bag/belt references use the item's actual UID.

| Priority | Eligible source and deterministic order |
| --- | --- |
| 1 | Equipped poison, preserving the existing equipment order and admission |
| 2 | Bag1, then Bag2, ordered by slot, UID and backing index |
| 3 | Belt, ordered by slot, UID and backing index |

Shape 1 is green poison; shape 2 is red poison. When both colours are carried,
the priority above determines the colour. Bag/belt candidates must have a
canonical poison template of Amulet type and shape 1 or 2, positive quantity,
unbroken durability and satisfied template class/gender/stat requirements.
Storage and quest items are not candidate sources. Exhausting one stack moves
the next accepted cast to the next eligible source.

Tagged seal/expiry/rental/shop-instance metadata retains the existing server
handling. This bounded change adds no wall-clock eligibility policy; repeated
freshness checks could select a different colour and debit at an expiry
boundary. The supply UI remains conservative about expired/sealed/shop-marked
instances. The cast evidence here covers ordinary poison stacks, not tagged
instance freshness. Any future time-dependent admission must bind selection
and debit within the same transaction.

`PoisonSword`, `PoisonShot`, `CrippleShot`, `PoisonCloud`, `Plague`, Soul
Fireball and summoning keep their previous material rules. Existing target,
range, MP and cooldown admission is unchanged. Rejected casts do not consume
carried poison.

## Client and quest guidance

The native client input path has no poison-equipment preflight to relax.
Practice and supply guidance now explains direct bag/belt Poisoning use and
equipped-poison priority. Soul Fireball and summons still require equipped
Amulets. The supply-plan flag describes material guidance rather than an
equipment swap. All nine registered languages now explain the same rule,
including the six extra locale packs' key overrides. The quest description,
practice instruction, material requirement and compact supply card agree.

Newcomer state checks use the actual Poisoning admission instead of a second
bag approximation. The authored condition string containing "re-equip between
casts" remains an internal compatibility key for existing quest definitions
and saves; the displayed instruction no longer requires a swap.

## Verification

Focused and adjacent tests use isolated in-memory accounts/scenes. They do not
use player saves, a running service or native UI. Rust 1.95.0 compiles the
changed source with `--locked`; existing debug caches are reused only as build
caches.

| Command scope | Result |
| --- | --- |
| `mir2-simulation --test poisoning_carried_items` | 7 pass: red/green bag and belt selection; actual Zone colour and exact receipt debit; equipped priority; exhaustion; missing/broken supply; ordinary personal casts; rejected target/MP/cooldown; other material rules |
| `mir2-simulation --test shared_zone_self_magic` | 4 pass: existing self, summon, directional/ground and rejection semantics |
| `mir2-gateway --lib routing::tests::poisoning_carried_tests::` | 3 pass: ordinary Magic through the actual in-process shared inventory service; green/red Bag2 exact UID and colour; belt exhaustion; equipped priority; cooldown, missing supply and unknown-target rejection without debit |
| `mir2-client-bevy --features native-ui --lib quest_practice::tests::` | 5 pass, including positive carried/equipped claims in all nine languages and all authored class guidance |
| `mir2-client-bevy --features native-ui --lib quest_supplies::tests::` | 9 pass, including source IDs, quantities, storage exclusion, seals/expiry and existing material requirements |

The working pre-change regression run has 5 expected failures and 2 guard
passes. Three earlier simulation fixture-construction attempts and three
Gateway fixture/compile attempts failed and are retained separately; they are
not feature regression evidence. The expanded-language test also retained
its first assertion failure: the Traditional Chinese compact card abbreviates
equipped Amulets as 裝備符. The corrected semantic check passed in all nine
languages. Raw logs and attempt receipts are retained under
`C:/mir2-client-delivery-20261002/poisoning-carried-items-20261002-231419-90cb51ab/`.

The seven simulation cases compose personal Session and Zone APIs; they are
not Gateway or live evidence. The three Gateway cases independently enter the
ordinary packet dispatcher and commit its actual shared inventory receipt.
Fixtures synchronize their trusted Zone transform and vitals explicitly;
no listener, external inventory service or database is used. Bag2 coverage
uses the existing expanded inventory capacity rather than bypassing slots.
There are 28 distinct focused/adjacent checks, not 28 live casts.

Implementation and tests are ready for source review. No game has been built,
launched, activated, deployed or restarted for this change. The frozen
`ab970` game source, R13 artifacts, resource-offset work and paused goals are
unchanged. Native visual acceptance remains separate.
