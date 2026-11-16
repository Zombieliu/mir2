# Ready-to-turn-in cofarm funding review

Trace sources (latest owner snapshots, not legacy class traces):

- Wizard: `C:\mir2-protocol-journey-20260911\Wizard.2026-09-15T22-04-24-777Z.trace.jsonl`, latest owner snapshot seq **8534**, `2026-09-15T22:21:53.055Z`, `JWdebac6d4`, Wizard level 25, map `0`, `(285,610)`, alive HP 100.
- Taoist: `C:\mir2-protocol-journey-20260911\Taoist.2026-09-15T22-01-35-089Z.trace.jsonl`, latest owner snapshot seq **11103**, `2026-09-15T22:19:56.537Z`, `JT3f4a4e1f`, Taoist level 25, map `1`, `(254,216)`, alive HP 177.

## Current ready state

Wizard has exactly one selected `readyToTurnIn` quest:

- q62 **Exterminate**, objective state `KekTal 1/1`, `VioletKekTal 1/1`, reward preview **EXP 3000, Gold 750**.

Taoist has no `readyToTurnIn` quest in its latest owner snapshot.

Neither current trace contains an AcceptQuest or CompleteQuest action for q62. The ready state is therefore inherited from persisted character state as far as these traces establish; this run cannot prove which earlier cofarm/acceptance sequence produced it.

## Generated route definition

Both `wizard-1-40-newcomer-v1.json` and `taoist-1-40-newcomer-v1.json` define q62 identically:

- finish NPC: `Master_Shok`, objectId 461, map `1`, coordinates `(312,73)`;
- eligibility classMask **31**, minimum level 18, prerequisite q61;
- reward: **750 gold**, 3000 EXP, fixed BronzeStrap itemIndex 1206 with requiredClass 31;
- source: `WoomyonWoods/Castle-GI/Town/3`.

## Assessment

`finishReadySupplyFundingQuest` filters the authoritative snapshot to `readyToTurnIn`, optionally restricts `questIds`, requires a generated finish NPC and positive gold, and then verifies completed stage plus exact gold increase. It does not accept or advance a new quest. Allowing an explicitly selected, already-ready q62 turn-in as ordinary positive-gold supply funding is consistent with that contract and does not alter the newcomer denominator.

The evidence supports q62 as a funding candidate for Wizard only. It does not establish that q62 was formed by the current run’s cofarm followers, and Taoist has no ready positive-gold candidate. Any decision to use q62 still needs the normal route/travel feasibility check to map 1 `(312,73)`; no live action was taken here.
