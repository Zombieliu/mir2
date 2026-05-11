Original prompt: Continue autonomous Crystal/Mir2 1:1 parity work until the current frontend input and NPC marker issues are landed and verified.

2026-05-11:

- Fixed/verified the Crystal input loop follow-up for held-run plus repeated target-click movement.
- Key evidence: `docs/generated/player-qa/movement-jitter/r-input-queue-held-run-spam-click-crystal-input-final-090527.json` is green with no visual jumps, no logical rollback, no direction lag, no stale prediction, no command queue warnings, and no residual movement plan.
- Re-smoked click target, route spam obstacle, blocked target, and NPC click paths against local Web/Gateway.
- Verified quest marker placement with an isolated temporary Gateway/account-store fixture so the main `.mir2-data/accounts.json` was not modified.
- Next useful follow-up: run one manual browser feel pass on the user's current page, then continue the queued deeper skill-system and late-gameplay packet-perfect parity slices.

2026-05-11 backend continuation:

- Reconciled two 5.5 xhigh worker slices plus local skill-system work.
- Hero learned magic now gains and levels from successful keyed Hero AI casts with Crystal `MagicLeveled` / `MagicDelay` packet evidence.
- `BackStep`, `ShoulderDash`, and `FlashDash` now advance practice only on Crystal success gates instead of generic cast completion.
- Mail exact parcel claim now preflights all serialized attachments and consumes payload only after successful claim.
- Verification: `magic_packet_crystal_` 73/73, Hero AI 28/28, focused Hero progression 2/2, Mail 9+2, Simulation fmt/check, and targeted diff checks passed.
