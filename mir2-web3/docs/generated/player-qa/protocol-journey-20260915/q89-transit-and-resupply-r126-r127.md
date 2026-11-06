# q89 movement protection and Taoist resupply — R126/R127

Observed 2026-09-16. These are controller and ordinary-player functional results. Full three-class 30-level route, native UI/animation and paired Crystal visual acceptance remain open.

## Problem and change

R124 Wizard travel entered the D2031 entrance Shamans' six-tile attack area before its spawn-search protection ran. The recorded critical escape occurred at 22% HP with four nearby hostiles. MP exhaustion did not cause that escape. R126 checks both physical Run cells or the Walk cell after cadence and immediately before dispatch against the latest AOI. Only q89 Wizard travel enables this guard.

A intercepted live blocker is cleared from the existing collision-certified seven-to-nine-tile firing band, with every other Shaman kept beyond six tiles. The action-owned kiting navigator has the same fresh footprint check; an invalidated band becomes an unavailable-band result rather than a close cast or recursive blocker resolution. Clearing requires that exact blocker's same-map authoritative corpse/known zero HP or a fresh specific ObjectDied receipt. Deferred or budget-exhausted live blockers receive ordinary recovery; absence of authoritative physical progress then terminates with a typed error instead of replaying the same guarded step indefinitely. Recovery does not reset the total clearance/engagement budget.

R125 Taoist left with 32 Amulet, consumed 24 and returned through the normal town route at eight remaining. HP and MP were still sufficient at that return. R127 uses a q89-only minimum of 32 and normal-shop departure target of 100. Other class/quest policies, game prices, combat, quest rewards and saved completion records are unchanged. No held TownTeleport was proven at that return, so no shortcut inventory is assumed.

## Validation and review

Root reviewed the fresh dispatch ordering, intermediate Run-cell coverage, deferred/capped blocker branches, action-owned navigator and compatibility of the older spawn-search caller before restarting ordinary play. Two initial R126 gaps were corrected before live launch: an unresolved blocker could repeatedly replan, and action-owned repositioning had captured an unguarded navigator.

| Stable regression | Result | SHA-256 |
| --- | --- | --- |
| R126 movement/travel/combat/kiting focused | 237/237 | C6410AEDA14AC60FCC06E925130C05A40C73006ECC6289C2872DE4D5F55D0DE2 |
| R127 survival focused | 74/74 | A5245C37526F70E47AC3BAA94ECC57E3391E59D34F2592A5D6B5C52F27C27ADA |
| Joint R126/R127 controller full suite | 655/655; zero failures/skips | F6B8F394B962E2BB03E12977C7E79956EBC6549276E66F0C90A809E874C2A145 |

Raw logs are retained alongside this note without EOF normalization. Regressions cover a Shaman moving during action-owned cadence, both physical cells, blocker cap, deferred-live recovery without movement, and Taoist stock 31 requiring restock versus 32 sufficient with departure target 100.

## Live continuation

Primary natural R54 remains on localhost 17810 with the same ordinary accounts/store. R124 Wizard and R125 Taoist were stopped normally before loading the corrected controller. The resumed traces are:

- Wizard: `Wizard.2026-09-15T20-39-33-006Z.trace.jsonl`, PID 93988.
- Taoist: `Taoist.2026-09-15T20-40-27-269Z.trace.jsonl`, PID 94196.
- Warrior: `Warrior.2026-09-15T20-40-55-574Z.trace.jsonl`, normal completed-route relogin/LogOutSuccess; no new quest progress was fabricated.

At launch, Warrior retains Lv30, 55/55 mandatory and 4/4 milestones; both casters retain Lv25, 45/55 and 3/4. Thus completed units remain **155/177**. Subsequent blockers cleared, partial kills and regression passes do not increase that count. Three-class final proof is still pending.

Fresh Warrior final verification passes: owner snapshot651 → outbound logOut652 → received LogOutSuccess653 → empty postlogout snapshot654. Persisted revision7447 and prelogout state match Lv30, map0 `(334,270)`, HP419/MP116, 55 mandatory and four milestones. The older position discrepancy disappears under a fresh ordinary login/logout and is not asserted as a saving bug.

`verify-three-class-saved-route.mjs` selects the owner snapshot before outbound public logout and checks known map/position/vitals, live owner/report identity, level≥30, exactly55 derived mandatory IDs and four milestones against persisted state. Whole-route output correctly remains FAIL while either caster is incomplete. Synthetic proof fixtures pass baseline with an empty postlogout snapshot and reject wrong map, unknown level, missing prelogout owner, dead final owner and wrong identity. These fixture results do not replace natural route evidence. Helper SHA-256: `5615ED540D67C7CA7D4AD99DB8808D614EAC8C8132B8F75A95EDB63F82C9312D`.

The first R126 live review finds D2031 searches still unable to reach remaining Priest candidates and no entry into D2032. No transit guard firing is claimed from that trace. `wizard-q89-r126-live-diagnosis.md` retains the selected evidence; the next bounded correction is to enable the existing live-transfer fallback after a physically stalled full-spread search rather than repeating respawn waits.

The R125 Taoist's later revival was a real death: snapshot 16840 still showed HP4, but authoritative Death 16841 and ObjectDied 16846 preceded townRevive 16848. The live reducer had marked self dead and HP0 before sending revival. The selected sequence proof is retained separately.
