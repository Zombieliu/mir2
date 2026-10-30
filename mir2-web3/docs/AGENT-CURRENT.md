# Agent Current Context

Short routing summary, updated 2026-10-30 (Asia/Shanghai).
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
| Windows wallet / branch integration (2026-10-28) | Native Dubhe QR login follows the existing language preference. Historical `00ca0f93f` rollout retained119 accounts/35 characters and original data. The unified test rollout now uses Gatewaye824/wallet600; current data/readbacks are recorded separately. Phone signing and human frontend acceptance remain open; no Docker started. | [Integration](BRANCH-INTEGRATION-20261028.zh-CN.md), [wallet flow](DUBHE-WALLET-WINDOWS-LOGIN.zh-CN.md), [historical rollout](DUBHE-WALLET-PLAYTEST-ROLLOUT-20261028.zh-CN.md), [current rollout](UNIFIED-PLAYTEST-ROLLOUT-20261030.zh-CN.md) |
| Unified Windows integration (2026-10-30) | History-preserving merge of wallet/packed `b87f4071a` and SC/procurement/USDC `d7a34129b`; Game1875/Engine77 frozen releases, full Candidate, two r11 installers and isolated Windows display/navigation passed (255079 hashes/16 screenshots,9901 locale keys). Public TEST Gatewaye824/wallet600 and origin/CDN packed2026102912/five-class32/legacy33 are live;119 accounts/35 characters retained,45 original table digests exact, migrations/two background clocks recorded separately. CI7 release gates/public12GET/6CMS passed. Actual Windows Wallet28/C824→Engine77→packed passed two pre-login launches/normal exits,Healthy state and independent33-pack hashes; no login/payment/freshSC acceptance. Existing P2 language-footer fallthrough, phone signing, livePG chain delivery and human Accepted remain open; no new chain transactions. | [Integration](UNIFIED-WALLET-RECHARGE-20261029.zh-CN.md), [local evidence](generated/unified-wallet-recharge-20261029/README.md), [public rollout](UNIFIED-PLAYTEST-ROLLOUT-20261030.zh-CN.md) |
| Overall parity | Candidate100 remains false in the latest summarized records; final human acceptance remains open. Do not invent an overall percentage. | [Roadmap](CRYSTAL-1TO1-ROADMAP.md), relevant topic section |
| Simplified Chinese / recharge navigation (2026-10-29) | Windows zh-CN has 9769 keys, 873 versioned AI/source editorial overrides and a bundled complete SC font. Clean game commit `24216899a` / 209 maps / 255027-file Candidate, internal CMS checks, updater commit `81942dda7` r10 / nine-file bundle and both full Inno 7.1.1 installers passed. Actual isolated r10 install, uninstall/reinstall, no-argument Launcher→Updater→Game, saved language and three recharge pages passed; 100 Credits/10000 Gold/three historical orders unchanged. First install had full file hashes; reinstall used closure and key-file checks. The r9 startup failure is retained. This is the dated local record; subsequent unified public TEST deployment is recorded separately. Human review count is zero; final Accepted, publisher Authenticode and production charging remain open. Some original bitmap and upstream English remains. | [Finishing/install evidence](generated/native-zh-cn-finish-20261029/README.md), [prior SC UI](generated/native-zh-cn-20261029/README.md), [recharge UI](generated/billing-entry-ui-20261029/README.md), [subsequent TEST rollout](UNIFIED-PLAYTEST-ROLLOUT-20261030.zh-CN.md) |
| Cell preproduction P0 / P1 (2026-10-28) | P1 passed; P0 incomplete; P2 not started. One-hour work: spectator projection18 checks + fresh16/16; correlated Attack ACK-before-presentation39 checks + fresh16/16. V10 still timed out at25. V11 admitted25 in65.756s, recorded4434 launched/2821 positive/0 outcome timeouts across admission+measurement, then08:35UTC budget drain; witness latency failed and no complete300s stage. No25 capacity qualification;50/100/250/500 and2h unmeasured. Both owned processes shut down. | [One-hour source/results](generated/cell-p0-p1-completion-20261024/capacity-hour-20261028-spectator-ack-v1/README.md), [Prior Store optimization](generated/cell-p0-p1-completion-20261024/capacity-store-transaction-snapshot-work-v1/README.md), [500 contract](CELL-500-COMBAT-ACCEPTANCE.zh-CN.md) |
| 3000–5000 CCU baseline (2026-10-30) | User confirmed all3000–5000 continuously fighting across multiple maps. e824 two-player60s stages passed but both whole runs failed; resume/save relogin/combat/capacity ceiling remain unverified. Offline2GW/2host config preflight57 and mocked idempotent stop29 checks passed; no new cluster deployed. Subsequent reference-audited storage maintenance removed28 stale stage directories and bounded journals; free space recovered to about22.17GB. Public releases, backups and game processes retained; this does not qualify CCU or resolve shared CPU/RAM. Distributed fighter fixture/controller and independent environment remain open; automatic cluster/Cell handoff absent. | [Baseline and fixed-cluster preparation](CAPACITY-3000-5000-BASELINE-20261030.zh-CN.md), [Storage maintenance](SERVER-RETENTION-20261030.zh-CN.md), [Original failed runs](evidence/capacity-3000-5000-20261030/README.md), [New tools](evidence/capacity-readiness-20261030/TOOLING-VERIFICATION.json) |
| Personal procurement V1 (2026-10-29) | Historical real Windows → authenticated Gateway → File authority → Dubhe testnet fill/cancel/120s expiry delivered/refunded (18 snapshots/9 real transactions, both actors' final relogin). Rust54/Gateway13/schema3/PGlite44/native13 passed; offline GPU evidence remains separate. Unified public TEST configuration/readClock now passed without signing or new trades. NativePG chain delivery, production charging, multi-Gateway/long recovery and human Accepted remain open; parity/capacity unchanged. | [Feature record](PERSONAL-PROCUREMENT-V1.zh-CN.md), [File evidence](generated/personal-procurement-v1-20261028/README.md), [TEST rollout](UNIFIED-PLAYTEST-ROLLOUT-20261030.zh-CN.md) |
| Sui native USDC recharge V1 (2026-10-29) | Historical Windows → Gateway/File → Sui invoice → Circle Testnet USDC delivered100 Test Credits; successful order2transactions/0.007138270 testSUI actualGas, duplicates remain one receipt/100Credits. Isolated QA Wallet used. Public TEST routes/configuration and readonly configureValidate now passed;500mMIST is a lifetime reservation cap, not actualGas. Stock wallet/human signing, new nativePG chain delivery, multi-host, production charging and human Accepted remain open; parity/capacity unchanged. | [Feature record](SUI-USDC-RECHARGE-V1.zh-CN.md), [File evidence](generated/sui-usdc-recharge-v1-20261029/README.md), [TEST rollout](UNIFIED-PLAYTEST-ROLLOUT-20261030.zh-CN.md) |
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
