# V2 ordinary practice navigation budget repair

The separate fresh Wizard paused on N4 at 2026-09-17T20:36:47.967Z with
`Navigation attempt budget exceeded (30)`. Its normal logout and matching save
retain level 7 and three completed nodes. Accepted owner locations show steady
progress; no static no-path, rejected movement or position loop was observed.
The first Oma field edge was about 70 tiles away, while dividing 180 attempts
equally across six candidates allowed only 30 attempts for that first leg.

The controller now reserves enough attempts for the actual waypoint distance,
within the same shared 180-attempt / 120-successful-step field-search caps.
It deducts actual integer counters from successful navigation and precisely
matched no-path/attempt-cap errors. Missing, null, string and out-of-range
counters fail closed. A fresh target entering AOI on the last allowed movement
is selected after settlement rather than lost when the budget reaches zero.
The existing final target approach remains separately bounded.

The implementation worker ran the affected Navigator and V2 suites: 87/87 pass.
New cases cover a distant first field, cumulative spending, invalid accounting,
attempt exhaustion and a fresh target appearing at the exact cap. Root reviewed
the source and checked the scoped diff. Server rules, collision, spawn density,
attack/recovery caps and the original 120-minute timing ledger are unchanged.

This repair is not an ordinary N4, full-route, timing or visual acceptance.
The same fresh Wizard account will resume through normal login from its save;
the original deadline remains 2026-09-17T22:35:16.486Z.
