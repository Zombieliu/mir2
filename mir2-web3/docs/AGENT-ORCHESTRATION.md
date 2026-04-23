# Agent Orchestration

Last updated: 2026-04-23

Purpose: define the autonomous multi-agent workflow for driving `mir2-web3` to a full Crystal / Mir2 1:1 Candidate build without requiring routine human confirmation.

## Target State

The automation target is **100% Candidate**:

- Code, data, docs, tests, traces, and screenshots are complete against the current acceptance standard.
- Known gaps are either fixed, explicitly documented as blocked, or moved to a user acceptance decision.
- The user only needs final gameplay validation before the project is marked **100% Accepted**.

`100% Candidate` is not the same as `100% Accepted`. The final accepted state requires either human frontend gameplay acceptance or an explicit decision to accept remaining human-only visual/feel differences.

## Progress Tracks

| Track | Owner | Primary Evidence |
| --- | --- | --- |
| Backend/server parity | Backend agents | Rust tests, protocol tests, Crystal source references, parity docs |
| Frontend/client parity | Frontend agents | Playwright/CDP screenshots, UI smoke, manual QA script |
| Crystal assets and data | Data agents | generated manifests, asset smoke tests, map/API checks |
| Integration/live parity | QA agents | packet traces, local-vs-Crystal diffs, gateway smokes |
| Playability/operations | QA agents | soak/load tests, reconnect tests, player QA route |

## Roles

| Role | Typical Model | Effort | Responsibilities |
| --- | --- | --- | --- |
| Coordinator | current main Codex session | xhigh | select tasks, prevent conflicts, integrate patches, run final tests, update docs |
| Crystal Explorer | `gpt-5.3-codex-spark` or mini | medium/high | inspect `E:\mir2\Crystal`, extract exact source behavior and edge cases |
| Rust Explorer | `gpt-5.3-codex-spark` or mini | medium/high | inspect current Rust code/tests, locate minimal change points and risks |
| Backend Worker | `gpt-5.3-codex-spark` | high/xhigh | implement assigned backend behavior in a bounded write set |
| Frontend Worker | `gpt-5.3-codex-spark` | high | implement assigned frontend/UI parity in bounded files |
| Data Worker | `gpt-5.3-codex-spark` or mini | medium/high | update generators/manifests/assets in bounded files |
| QA/Docs Worker | mini or `gpt-5.3-codex-spark` | medium | prepare test matrix, screenshots, trace evidence, and docs updates |

## Current Quota Policy

Current observed account state on 2026-04-22:

- active model: `gpt-5.3-codex-spark`
- general 5h limit: 80% remaining
- general weekly limit: 58% remaining
- `GPT-5.3-Codex-Spark` 5h limit: 97% remaining
- `GPT-5.3-Codex-Spark` weekly limit: 78% remaining

Scheduling policy while this quota profile holds:

- Use `gpt-5.3-codex-spark` for backend/frontend workers and high-value explorers because Spark-specific quota is abundant.
- Use `xhigh` only for bounded implementation in high-risk files such as `apps/simulation/src/runtime.rs`, protocol serialization, or complex UI state.
- Use `high` for normal code workers.
- Use `medium` for read-only exploration and docs/QA planning.
- Avoid unsupported account models such as `gpt-5.2-codex` in this environment unless a later settings check proves availability.
- Keep concurrent workers to one code writer per high-conflict file; spend extra quota on explorers and QA instead of conflicting writers.

## Coordination Rules

- The Coordinator owns final integration and decides whether a task is complete.
- Explorers are read-only unless explicitly reassigned.
- A worker must receive a bounded write set before editing files.
- Do not assign two workers to edit the same file or tightly coupled module at the same time.
- `apps/simulation/src/runtime.rs` is high-conflict. Only one worker may edit it per round.
- Docs can be edited in parallel only when the code worker is not also editing docs.
- Every completed behavior change must update:
  - `docs/CRYSTAL-1TO1-ROADMAP.md`
  - `docs/BACKEND-1TO1-PROGRESS.md` when backend parity changes
  - `docs/CRYSTAL-SERVER-PARITY.md` when server parity changes
- A checkbox is marked only after a command, screenshot, packet trace, or source comparison supports it.

## Round Template

Each autonomous round should target one verified completion item.

1. Select the highest-value small unchecked task.
2. Start read-only explorers for Crystal behavior and local implementation context.
3. Start one bounded worker if the implementation scope is clear.
4. Coordinator performs non-overlapping docs/task-queue work while agents run.
5. Review worker changes and explorer findings.
6. Run focused tests first, then broader regression if the change touches shared behavior.
7. Update docs/checklists/run log.
8. Start the next round without asking for confirmation unless a stop condition is hit.

## Stop Conditions

Do not stop for normal implementation decisions, local test failures, local refactors needed to finish an assigned item, generated data refreshes, or documentation updates.

Stop and ask only when:

- destructive filesystem operations are required;
- credentials, private endpoints, or a live Crystal server address are required;
- required Crystal source/assets are unavailable;
- two acceptance standards conflict and cannot be inferred from Crystal behavior;
- human-only frontend acceptance is needed to move from Candidate to Accepted.

## Standard Verification Tiers

| Tier | When Used | Examples |
| --- | --- | --- |
| Focused | Every small task | `cargo test -p mir2-simulation drop_item_packet` |
| Adjacent | Shared behavior changed | `pickup`, `harvest`, `storage`, `packet_trace` tests |
| Workspace | Stage gates | `cargo test --workspace`, `npm.cmd run build` |
| UI/API | Frontend/data changes | Playwright/CDP smoke, map API smoke, screenshots |
| Live parity | Acceptance gates | local-vs-Crystal packet trace diff |

## Current Round Status

The authoritative current round is in `docs/AGENT-TASK-QUEUE.md`. If this file and the queue disagree, trust the queue and update this section.

Current checkpoint:

- Active round: `2026-04-23-R29`.
- Active task: select the next bounded parity bite after verified R28 bounded `CombineItem` target-type gating completion.
- Active round state: task-selection stage only; do not reopen R28 unless source inspection or tests show a regression.
- Last completed round: `2026-04-23-R28`, Crystal `CombineItem` target item-type gating across packet branches.
- Backend/server parity estimate: `77.16%`.
- Whole-project 1:1 estimate: roughly `61.7%`.
- Restart handoff file: `docs/AGENT-RESUME-HANDOFF.md`.

Latest completed rounds:

| Round | Result |
| --- | --- |
| R28 | Crystal `CombineItem` top-level target item-type gating across socket/seal/upgrade packet branches, including 466-test `mir2-simulation` regression green. |
| R27 | Crystal inventory-grid `CombineItem` shape-3/4 gem/orb upgrade parity, including `ItemUpgraded`, persisted `gem_count`, and 465-test `mir2-simulation` regression green. |
| R26 | Crystal inventory-grid `CombineItem` packet parity for current socket-growth and seal branches, including protocol ids/codecs, gateway JSON, runtime dispatch, and 461-test `mir2-simulation` regression green. |
| R25 | Crystal `StoreItem` / `TakeBackItem` active `@Storage` / `NPCStorage` gating, `DontStore`, password-lock/capacity/occupied-target no-swap, and ack-only failure semantics. |
| R18 | Crystal drop visibility and pickup rejection edges. |
| R19 | Crystal `HarvestMonster` pending drop transfer and full-bag retry semantics. |
| R20 | Crystal harvest owner / `EXPOwner` corpse scan rejection. |
| R21 | Crystal sell service gating, partial-stack gold-cap rejection, credit-shop mail delivery, and mail attachment capacity checks. |
| R22 | Crystal `BuyItem` silent no-mutation rejection for invalid panel/count, missing service, non-buy service pages, missing goods/metadata, insufficient gold, and full bags. |
| R23 | Crystal NPC `RepairItem` / `SRepairItem` active-page gating, backpack unique-id lookup, cost, max-dura, and rejection semantics. |
| R24 | Crystal NPC `SellItem` `DontSell`, script type, price, ack-only failure, and gold-cap semantics. |

Restart rule:

- Read `docs/AGENT-RESUME-HANDOFF.md` before continuing after a reboot or context loss.
- Relaunch read-only explorers for any subagent findings that were not written to docs.
- Continue from R29 task selection without asking for routine confirmation; choose the next bounded task from the queue before starting new code work.
- On this Mac verification environment, use `cargo +1.89.0` for Rust checks/tests unless the toolchain is explicitly pinned later; default `rustc 1.87.0` does not compile locked `bevy_* 0.17.3`.
