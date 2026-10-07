# Player-QA playthrough loop

## Source33：三包体积达标，完整 Web 交付与实际可玩验收仍待验证（2026-10-08）

Core / Presentation / NPC Purchase 已按同一 Rust crate 的三个互斥生产 feature 拆分；原严格 JSON visitor 逐字迁移、Core/PUI 已发布 JS 保持字节一致，永久购买 facade 绑定独立 NPC 包版本且不因重试重建 ledger。原 optimizer、预算与 publication gates 保持；三包全部校验后才允许 immutable leaves 和 manifest-last。

实际原工具链编译及静态优化尺寸：Core 252883 B、Presentation 261542 B、NPC 167607 B；JS 24393/24721/8332 B，全部满足原 WASM<262144 / JS≤204800。原 Guard 六次 Cargo 实际 C≥50GiB、freshness 上界最大65ms、PolicyB全部关闭，30份 nonce 文件。选定 Rust 17＋9＋17=43次通过（26独立名称，两个原 Mail capture ignored 不计），Node20＋5＋1=26独立名称，共69次/52独立通过；新增10项Node测试。严格非增量 TypeScript5.9.3实际exit0、零诊断。895声明产品输入8改/887保护；17720声明Web输入在各验证调用前后匹配，后续文档变更只作限定承接。

尺寸通过不等于完整交付：原 full-module validation / name / normalized imports-exports 未运行，三 renderer、源码匹配 Next/Thin、完整当前 prebuilt 套件及 WASM/资源初始化均待验证。旧 Source25 prebuilt success 测试因真实双包manifest缺NPC而失败的日志、首次 Guard 审核配置绑定错误均保留；未改原断言或绕过门槛。Web交付仍Source25 Next08/Thin08，不含Source26–33。用户“继续代码，暂不操作界面”保持；实际登录→战斗→保存→重登、移动真机、玩家验收 not-run，无可信试玩日期。Matrix11保持309/317≈97.5%有界功能记录，含206 legacy、103 shared、8 common；overallPercentage=null、Candidate100=false、goal active。

Source32已实际提交推送并核验1c786cf494a7dd9394b3c5de65a9c6b7944491d5，本轮携带其事后publication；Source33 Git发布另以Root随后实际结果为准。[Source33实际结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source33-finite-result01.json)。下面Source32及更旧段落保留为历史，当前以上述Source33为准。


## Source32：严格编解码92项通过，最新 Web 包仍超过原体积预算（2026-10-08）

Root 在两个原产品文件收缩 JSON 边界：保留原 typed serde 与 Native/Gateway encoders、校验和购买 host ledger；Web 改用严格 Value 解码、经校验的 Value 输出及逐字节等价的请求编码，并复用现有递归 StrictMailValue walker。外层按原MAX_INPUT（6×2MiB＋64KiB）限长，内层frame独立按原2MiB限长；两个raw walker仍拒绝重复/escaped duplicate、过深与尾随JSON。完整 snapshot 仍是原始 carrier，不能作为真实 Applied 或 live authority 证明。

最终 Rust04 实际 Wire26＋Web ABI17＋Native36＋Gateway13＝92/92、92唯一具名测试；Source32新增14（Wire10＋bridge4）全部具名执行。864组 exact-byte specimens 是一个测试中的断言，不另加测试总数。893声明的普通 repository 输入中2改/891保护，冻结前后零漂移；不是全 workspace 或工具链覆盖。最终四次测试与一次Core编译均由原 immutable CargoGuard 串行运行，actual GetVolume C≥53687091200 B，sampleEnd→childStarted 截断UTC上界最大82ms<2000ms，PolicyB completed/exited/disposed，共25份原 nonce 文件。Rust03历史87通过、首轮 ABI14/15失败及空对象 unit enum 的原Serde兼容修复均保留，不累加为当前通过数或放宽断言。

原 Rust1.95.0 Cargo、wasm-bindgen0.2.118 和 Binaryen131 CLI 参数实际完成 Core 编译与静态优化，未使用 WASM API/instance。optimized Core 从460011降到365241再降到351501 B，累计减少108510 B（约23.6%）；生成 JS 与两个DTS保持原字节一致。351501仍不满足原严格<262144 B，JS26545≤204800通过。PUI首轮261472只通过旧输入的体积检查，不称最终源码重建；name/normalized imports-exports/full-module validation、三renderer、Next/Thin与最新Web组合交付仍未完成。预算、原helper/probe/Guard与publication gates均未改，原manifest/公开资产未换成未通过的产物。Windows/Gateway生产EXE仍是Source31只编译证据；本轮Native/Gateway运行的是有限测试binary，没有新EXE或产品启动。

下一轮Source33按独立只读调查落实第三个NPC policy feature/bundle。保护已发布Source25 Core/PUI全部导出，保留尚未交付的NPC Receipt ABI1及永久document facade；明确其raw export的包位置将改变。提取原无导出的StrictMailValue供两个模块复用，避免拉入Mail WASM exports；三包继续使用相同原预算与metadata gates，全部通过后才immutable publication、manifest-last。现有loader须绑定NPC包版本且不因重挂载/重连/加载重试重建ledger。该方案尚未实现或实测尺寸，不声明新包通过。

最新Web交付仍Source25（7b1afe2706a4eec037f085d1f599d2ea987f031f），Next08/Thin08不含Source26–32。Matrix11原字节保持103 shared/206 legacy/0 raw open/8 common：309/317≈97.5%只是有界记录，含legacy实现，overallPercentage=null、Candidate100=false、goal active。用户“继续代码，暂不操作界面”保持有效；资源/WASM初始化、实际登录→战斗→保存→重登、移动真机及玩家验收全部not-run，当前没有可承诺试玩日期。

Source31已实际提交推送并核验远端06da30011e4a98599bfee568e652f6d0839e894d，本轮携带事后publication。Source32 Git提交推送由Root随后实际操作另记，本文和测试不证明已推送。下面Source31及更旧段落均保留为历史，当前以上述Source32为准。

证据：[最终有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-finite-result01.json)、[独立结果复核](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-independent-review01.json)、[最新Core实际构建与体积失败](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-rust04-core-build-result01.json)、[初轮构建](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-initial-build-result01.json)、[中间构建](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-intermediate-build-result01.json)、[历史87项](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-rust03-finite-result01.json)、[保留的ABI失败](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-abi-test01-failure.json)、[结构方案](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-next-structure01.json)、[冻结源码](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-rust04-snapshot.json)、[独立源码复核](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-rust04-review.json)、[选定75份原始字节](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-raw-evidence01.json)、[Source31发布](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source31-publication01.json)。原始容器仅归档选定实际证据，不是全历史；源码SHA、未改的manifest/matrix与真实输入同实际结果记录对应。


## Source31：Native 完整购买接线与 Web 无损投影通过，最新 Web 交付包待重建（2026-10-08）

Native 永久 client 已接实际 gateway receiver、同源 projector、主线程整体经济应用和有类型的 Applied 回传；Quote、poll_ready、最终 Enter 使用原 UI inventory/catalog/parent proof，单次 Purchase、重连只查原 operation。每个 BuyItem 在同步发布时保存原完整来源；普通移动、钱包同值、有效同源更新保留该来源，修改目录或 owner 会退役旧手势。新普通 Hero/NPC 来源驱动真实经验/maxXP、装备/邮件/tooltip 和 Trade/BuyBack/Used 显示，Used 显示1、请求0。共享 game-data goods 投影复用原真实 Info/raw 数据；个人 Used checkpoint 仍不证明 shared-zone 全局 stock 原子所有权。

Web 在普通 decoder 前保留原 raw；声明的 u64/i64 叶子保留 canonical decimal string，safe Number 保持兼容。真实 Page 同步投影普通完整经济来源，并先于 movement-only 快路径替换变化内容；普通显示不能 settle 未决购买或循环 Begin/Query。宽 UID sidecar 保真，但既有 numeric selector/action/tooltip ABI 继续拒绝 unsafe 值。Storage 日期 DOM 从严格 i64 binary/ticks 转为可验证日历；Bevy numeric ABI 不接宽字符串或 unsafe Number。Hero 使用 fresh XP/maxXP。

Root 实际最终 Rust91（Native36＋Runtime39＋Hero3＋Core2＋Bevy1＋Simulation8＋GameData2），原3个 Web 脚本201/201（NPC155＋Storage25＋integration21；198唯一名称，跨脚本重复3），共292次选定通过、289唯一名称；新增47 Rust＋27 NPC＋2 Storage＝76项 distinct。Native03 仅按依赖不编译后续 cfg(test) 夹具修正限定承接；不称新 Native 测试执行。881/367 为声明的 Rust 输入快照，不称全 workspace/工具链覆盖。Web07 的15504声明输入前后零漂移。首轮失败、注册中止、旧不安全日期夹具及后续 AST/纯依赖夹具修复均保留；所有业务/exact-one 断言保留，未跳过。

原锁匹配的104公开 npm 包从本机 cache 按 SHA512 离线恢复，15209普通文件/0链接或额外文件，无安装器、生命周期或网络；原 TypeScript5.9.3 完整132文件已恢复。严格非增量 TSC02 实际 exit0/零诊断；其后仅声明 superset 中两个 MJS 测试夹具改变，有效 TS/JSON/依赖未变而限定承接，不称16827当前零漂移或重跑。用户 tsconfig 保持原状且不纳入本轮提交。

Windows EXE106137600 B、Gateway108321280 B 实际编译通过，未启动。最终 Native02/Gateway01 用原命令、串行原 CargoGuard 和886普通 repository 输入，加入真实 MMap/MagIcon include_str、Cargo配置与两个 workspace 解析 manifest；输入前后零漂移。实际 GetVolume C≥50GiB、sampleEnd→childStarted 截断 UTC 上界64.8447ms/62.3821ms<2000ms，PolicyB completed/exited/disposed。Native02 复用 Cargo 已有产物0.33s，不能称全量重新链接；前一 Native01 真实3m56s产物与之 SHA 相同。完整 Web/Windows 组合仍未完成：Core/PUI、三 renderer、Next/Thin 未匹配 Source31；原 optimizer 必须使用 WebAssembly.Module API，按本轮禁止 WASM API 边界尚未调用，原 helper 与 metadata/体积门槛未改或绕过。现有 Web 包仍 Source25 Next08/Thin08，不含 Source26–31。

Matrix11 原字节保持103 shared/206 legacy/0 raw open/8 common；309/317≈97.5%只是固定清单中的有界代码记录，overallPercentage=null、Candidate100=false、goal active。SkillModel 同数量行不等于逐行内容校验，poll_flush 无 deadline，controlled Sink/bare World/提取 Page 声明不等于 live UI/socket。用户“继续代码，暂不操作界面”保持有效；资源/WASM初始化、实际登录→战斗→保存→重登、移动真机和玩家验收全部 not-run，没有可承诺试玩日期。下一项先交付匹配 Web 包，再继续未穷尽的数值 ABI/技能内容/运输时限和共同缺陷；不以原始缺口分类关闭替代可玩验收。

Source30 已实际提交推送并核验远端 cb30715f53322f31304ed5c449b0347bdcfd95dd；本轮携带其事后 publication。Source31 发布另由 Root 实际 Git 操作核验，测试、构建和本文不证明已推送。以下 Source30 与更旧段落，包括旧 Current Round 状态，均为历史；当前以上述 Source31 为准。

证据：[本轮有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source31-finite-result01.json)、[Web实际结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source31-web-finite-result01.json)、[严格类型检查](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source31-tsc-finite-result01.json)、[静态构建](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source31-static-build-result01.json)、[独立源码范围复核](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source31-independent-review01.json)、[选定原始字节](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source31-raw-evidence01.json)、[Source30发布](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source30-publication01.json)。原始字节容器仅归档选定执行证据，完整历史/cache恢复台账仍在原外部QA路径；可读JSON副本与原SHA对应，CRLF归一不替代精确原始收据。

`apps/web/scripts/qa-playthrough.mjs` drives the **real web client over Chrome
DevTools Protocol** through a full "real player" journey and records every
problem it sees into a structured report. It is the browser-level complement to
the protocol-level bots (`load-gateway-ws.mjs`, `smoke-two-client-zone.mjs`):
those are blind to rendering, this one sees the actual Bevy canvas.

## The journey (beats)

1. open client → login screen
2. register a fresh account
3. log in → character select
4. create a character → start game
5. enter world → **map renders** (scene-ready + canvas not black + no stuck "Loading map…")
6. move → **render keeps up** (server moved us but canvas didn't = render bug)
7. **camera update-rate probe** → quantify the scroll content rate during a sustained walk (judder = low `cameraUpdateHz` vs `rafHz`)
8. find an NPC → walk to it → open dialog (+ assert `.npc-dialog-panel` rendered)
9. accept a quest through the dialog (`questLog` grows)
10. **cross-map** → transfer to a SECOND map and assert it renders (interaction-ready + canvas not black; flags stuck "Loading map…" / black)
11. **combat** → find a `kind:"monster"` near the player (hopping to a hunting field — Woomyon "1" / Serpent Valley "2" — if the current map is empty), walk melee-adjacent, and attack the way the client does (click its tile → `activateEntity` → `attackTarget` → `send({type:"attack"})`). Verify its HP drops and/or it dies, and that damage indicators (`.scene-damage-floater`) appear; flags "attacking produced no damage/death/indicators"
12. **inventory** → click `.hud-button.inventory button` and assert the inventory window (`.inventory-window`) rendered
13. travel several legs → render stays stable
14. wrap → dump diagnostics

Each beat is best-effort: a failure is recorded and the loop keeps going.

### Camera A/B mode (`--cameraAb true`)

A separate flow (instead of the journey above) that A/Bs the Bevy self-camera fix
(PR #125): it runs the camera update-rate probe **with and without
`?bevySelfCamera=1`** across 2–3 maps (Bichon "0", Woomyon "1", Serpent Valley
"2") and reports `cameraUpdateHz` for each into a comparison table in `report.md`
(and `camera-ab.json`) — the evidence for a default-on decision. Each variant
reloads the page (the flag is read once at load), re-logs in, and re-enters.

## What it records

Output lands in `apps/web/docs/generated/player-qa/playthrough-<runId>/`:

| File | Contents |
|---|---|
| `report.md` | human-readable: issue table by severity, per-beat journey, evidence |
| `report.json` | machine-readable issues + beats |
| `summary.json` | counts by severity/category |
| `frames/NN-<beat>.png` | screenshot after each beat (the visual timeline) |
| `console.json` | console errors/warnings |
| `network-failures.json` | every ≥400 / `net::ERR_*` request (sprite/atlas 404s) |
| `ws-timeline.json` | last WS frames sent/received (server truth) |
| `camera-ab.json` | (`--cameraAb` only) per-map `cameraUpdateHz` with/without `?bevySelfCamera=1` |

### Issue categories

- **render** — black/blank canvas (luma), stuck "Loading map…", scene never ready, dialog open in state but not in DOM, render frozen during movement, cross-map transfer that never lands/renders
- **movement** — server moved but client didn't (desync), teleport/jump, or low camera scroll rate vs frame rate (judder). Note: the time between *logical tile changes* is the walk/run cadence (movement speed), NOT jank, so it is recorded (`tileCadenceMs`) but never flagged. Fine-grained movement-feel analysis (prediction staleness, command-queue latency, camera continuity) is `capture-web-movement-jitter.mjs`.
- **combat** — attacking a monster produced no damage/death/damage-indicators, or no monster could be found/spawned to fight
- **quest** — NPC click opened no dialog, no quest added after clicking dialog links, no NPC on map
- **ui** — a HUD window (e.g. inventory) did not open/render when its button was clicked
- **network** — failed sprite/atlas/UI requests, grouped by kind (a wall of identical 404s = one issue). _If files exist in git, the R2 release is likely stale — see `ASSET-RELEASE-RUNBOOK.md`._
- **console** — critical console errors/exceptions (deduped)
- **flow** — a beat threw (blocks progression)

## Run it

Prereqs: gateway on `:7110` + simulation running, and a web client served
somewhere (a running `next dev`, e.g. `:3001`).

```bash
cd apps/web

# watch it play (headed; real GPU — best for render fidelity)
npm run qa:playthrough -- --headed --baseUrl http://127.0.0.1:3001

# headless (CI-style)
npm run qa:playthrough -- --baseUrl http://127.0.0.1:3001

# camera A/B (compare ?bevySelfCamera=1 on/off across maps) instead of the journey
npm run qa:playthrough -- --headed --baseUrl http://127.0.0.1:3001 --cameraAb true

# against this worktree's own dev server
npm run dev            # in another terminal; note the port it prints
npm run qa:playthrough -- --headed --baseUrl http://127.0.0.1:<port>
```

Useful flags: `--account NAME --password PW`, `--createAccount false` (reuse an
existing account), `--runId my-run`, `--startMap 0 --startX 330 --startY 330`
(force a spawn point), `--moveWindowMs`, `--combatWindowMs`, `--sceneReadyTimeoutMs`,
`--cameraAb true` (run the camera A/B comparison instead of the normal journey).

> **One run at a time:** a playthrough drives the live stack (gateway + simulation
> + client) exclusively — don't start a second loop against the same stack while
> one is running.

## Gotchas

- **Headed uses real GPU**; headless falls back to SwiftShader which can produce
  a falsely "black" canvas — prefer `--headed` when judging render health.
- Background-throttling is disabled via Chrome flags, so rAF keeps running even
  when the window is not focused.
- The harness launches its **own** fresh Chrome (separate profile + debug port);
  it does not touch your normal browser.
- It registers a **new account each run** (unique id) so runs are reproducible
  from scratch; pass `--account`/`--createAccount false` to reuse one.

## Fix loop

The report is the input to the fix phase: triage by severity, locate with
codegraph, fix, then **re-run the same loop** (`--runId`) to confirm the issue is
gone (regression).

---

# Soak loop (`qa-soak.mjs`)

`apps/web/scripts/qa-soak.mjs` is the **long-running stability** complement to the
journey loops above. The playthrough/combat/social loops each run a short scripted
journey and exit; the soak answers the one thing they can't — does a **sustained
"play all day" session stay healthy, or slowly leak / degrade / desync?** It keeps
the real client **continuously busy** and samples a **time-series** of memory / GPU /
DOM / FPS / WebSocket / error health every ~10–15s for the whole run (minutes →
hours), then fits trends and emits a **PASS / LEAK / DEGRADED** verdict.

## Activity driver (active soak, not idle)

An idle client never allocates, so a leak that only grows under play would never
show. On a fast cycle (`--activityIntervalMs`, default 2.5s) the harness alternates
real player activity, keeping the client busy the whole run:

1. **click-to-move** — click an in-viewport tile a few steps away (production move path)
2. **held-keyboard** — sustain a real held **arrow key** via CDP `Input.dispatchKeyEvent`
   (also the only loop that exercises the keyboard SEND pipeline the click loops miss)
3. **combat** — find a `kind:"monster"`, walk adjacent, swing
4. **cross-map travel** — `transferMap` across a ring of maps (Bichon "0" / Woomyon "1" / Serpent Valley "2")
5. **inventory churn** — open then close the inventory HUD (a window mount/unmount → DOM-leak probe)

## What it samples (the time-series)

| Group | Source | Fields |
|---|---|---|
| JS heap | `performance.memory` (**GC-forced** each sample → retained heap) | `usedJSHeapSize`, `totalJSHeapSize` |
| Runtime / GPU | `window.__mir2BevyEntityRendererDebug` + `sceneAssetRuntime` | `atlasPixelBytes`, `atlasCount`, `alphaKeyedBlobBytes` |
| Cache | `window.__mir2CacheMetrics.snapshot().summary` | `transferBytes`, `cacheStorageEntryCount`, `domImageCount` |
| DOM | `document` | `querySelectorAll("*").length`, `images.length` |
| FPS / cadence | in-page rAF recorder (the `rafGaps` approach from `qa-load-stress.mjs`) | median fps, p95 frame-time, worst no-frame gap |
| WS health | CDP `Network.*` | reconnect count, frames-received rate, `wsState` |
| Errors | CDP `Runtime.consoleAPICalled` / `exceptionThrown` + `Network` ≥400 / `net::ERR_*` | cumulative + per-window delta |

> Heap is read **after a forced `HeapProfiler.collectGarbage`**, so the series is
> *retained* memory — a monotonic climb is a real leak, not uncollected garbage.

## Detectors / verdict

- **LEAK** — retained JS heap **or** bevy `atlasPixelBytes` **or** DOM node count trends
  monotonically up (least-squares slope over the **steady-state** windows, past warm-up)
  beyond a threshold GC doesn't reclaim.
- **FPS DEGRADED** — last-window median fps materially below the first window.
- **ERROR ACCUM** — console/network error rate growing across the run.
- **RECONNECT STORM** — repeated gateway reconnects (or sustained non-`open` `wsState`).
- **FREEZE** — a long no-frame gap (rAF starved) / zero-frame window / renderer crash.

Verdict = **LEAK** (any leak detector) → else **DEGRADED** (any other) → else **PASS**.
Exit code is `1` on LEAK / FREEZE / RECONNECT-STORM (hard failures), else `0`.

## What it writes

Output lands in `apps/web/docs/generated/soak-qa/run-<runId>/`:

| File | Contents |
|---|---|
| `timeseries.json` | the full per-window sample series (written incrementally — survives a mid-run crash) |
| `report.md` | verdict, detector table, trend table with **sparklines**, per-window numbers, issues |
| `report.json` / `summary.json` | machine-readable verdict + detectors + trends |
| `console.json` / `network-failures.json` | accumulated errors |
| `frames/00-start.png`, `frames/99-end.png` | first/last screenshots |

## Run it

> **Prefer an ISOLATED gateway.** The shared `:7110` node-proxy sim **depletes** over
> long runs (project memory), so a soak pointed at it measures shared-sim exhaustion,
> not the client. Spin up a private gateway on fresh ports with a temp account store
> and point the client at it via the localhost-only `?gatewayWs=` override:

```bash
cd apps/web

# 1) isolated gateway + sim (fresh ports + temp account store)
MIR2_GATEWAY_WEB_ADDR=127.0.0.1:7311 MIR2_ACCOUNT_STORE=$(mktemp -d)/acct.json \
  cargo +1.89.0 run -p mir2-gateway --bin mir2-gateway

# 2) reuse a running `next dev` (note its port) and point the soak at the isolated gateway
npm run qa:soak -- --headed --durationMin 120 \
  --baseUrl http://127.0.0.1:3001 --gatewayWs ws://127.0.0.1:7311/ws

# quick smoke (5 min) — proves the time-series + verdict + report pipeline
npm run qa:soak -- --headed --durationMin 5 --sampleMs 8000 \
  --baseUrl http://127.0.0.1:3001 --gatewayWs ws://127.0.0.1:7111/ws
```

If you run against the **shared** `:7110` stack the report flags it (`gateway.shared`)
so a leak/degrade there isn't mistaken for a client bug.

Useful flags: `--durationMin` (default 20; supports multi-hour), `--headed`
(default true — real GPU for true render memory), `--baseUrl`, `--account`/`--password`,
`--sampleMs` (default 12000), `--activityIntervalMs` (default 2500), and detector
thresholds (`--leakHeapSlope`, `--leakDomSlope`, `--fpsDegradeRatio`, `--reconnectMax`,
`--freezeGapMs`, `--warmupFraction`). `Ctrl-C` flushes a partial report.
