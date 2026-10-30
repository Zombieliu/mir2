# Agent Current Context

Short routing summary, updated 2026-10-31 (Asia/Shanghai).
This indexes recorded decisions and open work; it does not revalidate today's
working tree, services, release packages, source snapshots or parity percentages.

## Start with the user task

- A specific request defines the scope. Do not start unrelated gameplay or release
  work because a historical document says to continue a previous round.
- Small tasks read relevant source/tests only. For substantial implementation or
  general queue work, read [coordination](AGENT-ORCHESTRATION.md) and this summary,
  then only the relevant sections linked below.
- Inspect the current diff before editing; the shared workspace contains unrelated
  work. Confirm active configuration/code only where needed for the requested task.

## Last recorded decisions and open work

| Topic | Recorded state and next scope | Detailed reference |
| --- | --- | --- |
| Windows wallet / branch integration (2026-10-28) | Native Dubhe QR login follows the existing nine-language preference. Full-history integration preserves local gameplay and durable authority domains. Test Gateway and isolated wallet login deployed at `00ca0f93f`; 119 accounts/35 characters and original data digests retained, no Docker started. Phone signing and frontend acceptance remain open. | [Integration](BRANCH-INTEGRATION-20261028.zh-CN.md), [wallet flow](DUBHE-WALLET-WINDOWS-LOGIN.zh-CN.md), [test rollout](DUBHE-WALLET-PLAYTEST-ROLLOUT-20261028.zh-CN.md) |
| Windows promotion / recharge bridge (2026-10-30) | Original test Gateway updated to `050950f5b`; original120 accounts/36 characters/saves and historical billing row hashes preserved, with two empty system actors separately added. Native-PG CI49 passed without skips or Docker. Website source `806e8be6d` is public on mir2.obelisk.build; new Windows test campaign recruitment/commission on, payout off, three legacy campaigns isolated. Clone synthetic proof maps to original account; fresh phone signing, real recharge/commission display and human acceptance remain open. Native origin patch is separate; no updater published. | [Topic and rollout evidence](PROMOTION-WINDOWS-BRIDGE-20261030.zh-CN.md) |
| Dubhe native USDC transfer accounting (2026-10-31) | Gateway and matching worker `868ef2d2` deployed after native-PG CI54 and isolated-clone acceptance. Old real 1-USDC payment restored: Applied, +100 credits; clone replay grants0. Original125 accounts/38 characters/saves retained; exact public assets verified. Public r12 installer still contains game `1875c6c20`, which rejects the separate billing origin; client patch `5022a2423` plus explicit packaged billing origin must ship together. Windows click-to-browser/mobile payment and human acceptance remain open. No Docker or updater publication by this chat. | [Implementation, client handoff and evidence](USDC-NATIVE-TRANSFER-20261031.zh-CN.md), [earlier UI candidate](USDC-DUBHE-QR-PAYMENT-20261030.zh-CN.md) |
| Reviewed legacy invitation carry-over (2026-10-31) | User authorized preserving the original leader/member relationship across the two named test campaigns. Native-PG migration14 and CLI/adjacent21 passed, no skips or Docker; source evidence and actual current-terms acceptance remain distinct. Original leader's current Windows enrollment verified read-only. Exact CI, clone preservation, same-source GW/worker rollout and member confirmation remain open; no real relation or money moved. | [Policy, verification and current conditions](USDC-NATIVE-TRANSFER-20261031.zh-CN.md#已授权的旧邀请迁移实现与待完成条件) |
| Unified Windows integration (2026-10-30) | History-preserving merge of wallet/packed `b87f4071a` and SC/procurement/USDC `d7a34129b`; original tracked work preserved. Game1875/Engine77 clean releases, full Candidate, two complete r11 installers and actual isolated Windows display/navigation passed; installed255079 hashes and16 screenshots recorded. SC procurement now has9901 total locale keys. Signed33-pack generation passed real local production-library apply/PID-ready/10s health. Existing P2 language-footer pointer fallthrough remains; language input isolation, phone signing and human Accepted not passed. Public feeds/server unchanged; no new chain transactions. | [Current integration](UNIFIED-WALLET-RECHARGE-20261029.zh-CN.md), [evidence](generated/unified-wallet-recharge-20261029/README.md) |
| Overall parity | Candidate100 remains false in the latest summarized records; final human acceptance remains open. Do not invent an overall percentage. | [Roadmap](CRYSTAL-1TO1-ROADMAP.md), relevant topic section |
| Simplified Chinese / recharge navigation (2026-10-29) | Windows zh-CN has 9769 keys, 873 versioned AI/source editorial overrides and a bundled complete SC font. Clean game commit `24216899a` / 209 maps / 255027-file Candidate, internal CMS checks, updater commit `81942dda7` r10 / nine-file bundle and both full Inno 7.1.1 installers passed. Actual isolated r10 install, uninstall/reinstall, no-argument Launcher→Updater→Game, saved language and three recharge pages passed; 100 Credits/10000 Gold/three historical orders unchanged. First install had full file hashes; reinstall used closure and key-file checks. The r9 startup failure is retained. Human review count is zero; final Accepted, publisher Authenticode, remote deployment and production charging remain open. Some original bitmap and upstream English remains. | [Latest finishing/install evidence](generated/native-zh-cn-finish-20261029/README.md), [prior SC UI](generated/native-zh-cn-20261029/README.md), [recharge UI](generated/billing-entry-ui-20261029/README.md) |
| Cell preproduction P0 / P1 (2026-10-28) | P1 passed; P0 incomplete; P2 not started. One-hour work: spectator projection18 checks + fresh16/16; correlated Attack ACK-before-presentation39 checks + fresh16/16. V10 still timed out at25. V11 admitted25 in65.756s, recorded4434 launched/2821 positive/0 outcome timeouts across admission+measurement, then08:35UTC budget drain; witness latency failed and no complete300s stage. No25 capacity qualification;50/100/250/500 and2h unmeasured. Both owned processes shut down. | [One-hour source/results](generated/cell-p0-p1-completion-20261024/capacity-hour-20261028-spectator-ack-v1/README.md), [Prior Store optimization](generated/cell-p0-p1-completion-20261024/capacity-store-transaction-snapshot-work-v1/README.md), [500 contract](CELL-500-COMBAT-ACCEPTANCE.zh-CN.md) |
| Personal procurement V1 (2026-10-29) | Custom feature: real Windows → authenticated Gateway → File game authority → Dubhe testnet fill, cancellation and 120s expiry all delivered/refunded; independent audit passed across 18 snapshots/9 real transactions, including both actors' final post-settlement relogin. Rust 54, final Gateway 13, schema 3, PGlite 44 and native logic 13 passed; historical offline GPU captures remain separate. Production charging, native PostgreSQL, multi-Gateway/long-running recovery and human Accepted remain open; this does not change overall parity/capacity status. | [Single topic record](PERSONAL-PROCUREMENT-V1.zh-CN.md), [public evidence](generated/personal-procurement-v1-20261028/README.md) |
| Sui native USDC recharge V1 (2026-10-29) | Custom feature alongside Stripe: real Windows → Gateway/File → independent Sui Move invoice → Circle Testnet USDC payment → 100 Test Credits delivered. One successful order uses two transactions, 0.007138270 test SUI actual Gas; separate expired attempts/setup retained. Duplicate actual payment notifications preserve one receipt and 100 Credits. Isolated QA Wallet adapter was used; stock extension wallet, native PostgreSQL/multi-host, public deployment, production charging and human Accepted remain open. Final restart/relogin evidence is recorded in the topic. This does not change parity/capacity status. | [Single topic record](SUI-USDC-RECHARGE-V1.zh-CN.md), [public evidence](generated/sui-usdc-recharge-v1-20261029/README.md) |
| Level 100 / SLG (2026-10-19) | User direction is a level-100 cap with new SLG after max level. Resource audit/planning is recorded; runtime cap, level 71-100 content, balancing, ordinary acquisition and server entry gates still require implementation/verification. Keep custom rules separate from Crystal's original baseline. | [Growth rules](LEVEL100-SLG-PROGRESSION.zh-CN.md), [equipment audit](EQUIPMENT-LEVEL100-SLG-AUDIT-20261019.zh-CN.md), [queue](AGENT-TASK-QUEUE.md) topic heading |
| Cross-chat integration (2026-10-14) | Code/push/validation scope was reconciled. Signed R26 paired release, event-site re-publication and player acceptance remained open in that record; confirm current status only for related tasks. | [Code progress](CODE-PROGRESS-20261014.zh-CN.md) |
| Web delivery (2026-10-12) | Source78 records a verified empty Spells bootstrap/reentry branch. It does not establish nonempty skills, every panel, combat/quest, mobile or final acceptance. Do not rebuild/retest that historical package for unrelated tasks. | [Frontend gaps](FRONTEND-1TO1-GAPS.md), [Windows QA](NATIVE-WINDOWS-PLAYER-QA.md), queue Source78 section |
| Shared world / authentication | Session owns personal state; Zone owns shared world authority. Enforce authentication, safe routing and final transform before save. Inspect present implementation rather than assuming an old MVP description is current. | [Zone rules](SHARED-ZONE-MVP-RULES.md), relevant runtime/gateway source |

## Read by task

| Task | Start here; widen only if needed |
| --- | --- |
| Simulation behavior | `apps/simulation/src/runtime/` and `apps/simulation/tests/`; matching backend/parity topic sections |
| Gateway/auth/session/routing | `apps/gateway/src/`, `apps/simulation/src/world_runtime.rs`; Zone rules when lifecycle/movement is involved |
| Player Web/UI | `apps/web/app/`, `apps/web/lib/`, `apps/web/scripts/`, `apps/web/package.json`; relevant gaps/QA section |
| Native/shared client | `apps/game-client/`; relevant native QA section |
| Data/assets | `packages/game-data/`, `packages/tooling/`, the relevant generator and manifest; use explicit asset/evidence paths |
| Architecture/integration | [Architecture reference](ARCHITECTURE-CURRENT.md) plus current source at affected boundaries; dated implementation claims require checking |
| Resume after context loss | This file and coordination, then task-specific saved evidence; [old handoff](AGENT-RESUME-HANDOFF.md) only for the relevant historical topic |

All paths above are relative to `mir2-web3`. Source/configuration manifests remain
visible in default search; binary media, `work/`, packaged copies and generated QA
evidence are excluded by root `.ignore` and remain available by explicit path.

## Bounded lookup examples

From the repository root:

```text
rg -n "<symbol>" mir2-web3/apps/simulation/src mir2-web3/apps/simulation/tests
rg -n "^##|<topic>" mir2-web3/docs/AGENT-TASK-QUEUE.md
rg --no-ignore -n "<topic>" mir2-web3/docs/generated/<relevant-evidence-directory>
rg --files --no-ignore mir2-web3/apps/web/public/<relevant-library> -g '*.png'
```

Read only the matching section. For queue selection, prefer the newest relevant
user decision over stale completed-round tables. Keep this summary below 120
lines; record a detailed round once in its topic document and link to it here.
