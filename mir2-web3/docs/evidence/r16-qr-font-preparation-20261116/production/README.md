# 6ba exact-source paired production release

Application `6ba38557ddb298df72cb9e4a23c48caf542a598e` is live at
[numeron.obelisk.build/game](https://numeron.obelisk.build/game).
The Vercel game deployment is `dpl_G8iq4UBFibrWSEaSbwahtboeAom3`; the Gateway
reports the same application revision. Controller commit
`3a1d96c91abaadbad3801207562c925f8da63e90` and CI checkout fix
`f16f42c9eacde04b383dd2b25acadfde09a5f2be` are separate from application source.
No CDN asset, DNS or marketing-project release was performed in this round.

The matching [backend run](https://github.com/Zombieliu/mir2/actions/runs/43755627882)
passed 310 shared-world and 216 native PostgreSQL tests. Downloaded artifact
`14179605047` matched GitHub's archive SHA and all 1,841 packaged file hashes.
The guarded release used the reviewed [63-commit / 386-path source closure](../release/README.md).
[Stage](https://github.com/Zombieliu/mir2/actions/runs/43757458510),
[protected checks](https://github.com/Zombieliu/mir2/actions/runs/43758624049) and
[promotion](https://github.com/Zombieliu/mir2/actions/runs/43759114331) completed.

## Two independent public cohorts

Installed headless Chrome used ordinary system networking and the official
phone-sized Testnet wallet page. Each cohort verified actual QR pixels, one
signed login approval, character creation, ordinary StartGame and a real
authoritative adjacent walk. The unchanged original classifier accepted both
runs; the 8-second movement, 65-second walking-package and 120-second
runtime/Native limits and all six full fonts / 113 HUD checks were retained.

| Observation | R1 | R2 |
| --- | ---: | ---: |
| Input to authoritative walk ACK | 131 ms | 136 ms |
| First playable frame after StartGame intent | 6,950 ms | 7,086 ms |
| Runtime body after first frame | 21,386 ms | 37,194 ms |
| Native HUD observed after first frame | 89,688 ms | 86,942 ms |
| Complete original font SHA checks | 6/6 | 6/6 |
| HUD resource checks | 113/113 | 113/113 |
| Q window observed | Shared Native | React fallback |
| Original classifier | Accepted | Accepted |

The Q distinction is material: R2's HUD was ready while its Quest status was
still not ready, so Q opened the existing fallback window. After full font
verification and Worker restart, both native HUD and Quest reported ready.
The original classifier permits either Q implementation; its `sharedNativeQuest`
check name does **not** prove two native Quest observations. Cold-start ownership
handoff and pixel/language parity remain open. These are two repetitions of the
same public WebGPU package, not a WebGL2 public acceptance claim.

Both runs also passed complete original walking JSON delivery, localized Quest
content, Worker stop/restart with an unchanged cached HUD hash, normal Logout
and return to selection. This Worker restart is **not** a game/session restart.
An isolated cloned-database canary separately passed normal registration,
password login, walking, two Logouts and same-role reentry at the saved position.

Original reports: [R1](r1-original-acceptance.json), [R2](r2-original-acceptance.json).
Details, fresh public version/health and independent alias readback:
[verification](verification.json). Raw login/approval data and browser traces
remain private; their [audit hashes](private-evidence-pins.json) are retained.

## Preservation and limits

A transaction-bound production snapshot was backed up and restored into an
isolated database before rollout. After both public runs, all 312 preexisting
account rows, 224 character rows and 224 save rows matched their original bytes;
all 22 original configuration files matched their hashes. Two new QA accounts,
characters and saves remain, giving 314 / 226 / 226. No production database was
restored. All 29 recorded unrelated WIP hashes remain intact.

Simplified and Traditional Chinese public QR checks each created one real
matrix, reached pending, cancelled normally and observed no late polls. Both
owned browsers closed: [Simplified](qr-zh-cn-receipt.json),
[Traditional](qr-zh-tw-receipt.json). Physical phone camera scanning is untested.
All owned test browsers, isolated canary services and the SSH controller closed;
this round did not write credentials to files or Git.

Native HUD still takes roughly 87–90 seconds after the first playable frame in
these cold runs. Passing the recorded limit does not establish Windows-like
loading speed. Global Candidate/Accepted remain false: broader routes/maps,
three-class full progression, combat/economy/Alo, mixed language/visuals,
independent Windows UI, existing Rust format failures and human-only checks
remain tracked by the active R16 Goal. Earlier failed public runs retain their
original failed results and rollback records.

Screenshots: [R1 HUD](r1-public-hud-after-worker-restart.png),
[R1 Native Q](r1-quest-shared-zh-TW.png),
[R2 HUD](r2-public-hud-after-worker-restart.png),
[R2 fallback Q](r2-quest-shared-zh-TW.png).
