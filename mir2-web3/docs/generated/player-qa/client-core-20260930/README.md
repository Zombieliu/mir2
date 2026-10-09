# Shared quest client: 2026-09-30 implementation checkpoint

## Progress74：本地 NPC 金币购买 owner 路由已通过有限检查（2026-10-06）

本批 Source03 已通过独立源代码与实际结果审查，0 blocker；现有串行 CargoGuard 下 **3/3 组、53/53 项执行通过**（购买 18、会话 34、商城 1），其中 **16 项新增测试**。两轮历史测试失败保留，不计入 53 项通过数；修正仅涉及新增测试夹具和预期，产品代码保持一致。

网关将本次 Committed / Rejected / Unknown 结果沿真实本地 owner 调用传递；身份、租约和能力检查在执行前完成。提交后的元数据、事件或协调异常保留本次已知结果，捕获前异常保持 Unknown。原 804 行共享通道可逐字节逆向恢复；现有 BuyItem 三字段、四键 JSON、GameShop 及共享尾部处理保持原契约。

本批有限检查记录已完成；测试数量表示实际执行数，不能用作项目整体完成率。旧生产包与历史界面证据仍限定在其原批次。

本批不计网络购买 ACK、幂等性、客户端恢复、新生产组合、UI/玩家/移动设备验收、Candidate 或总 goal 完成。详见 [本批 QA 及剩余工作](npc-gold-buy-owner-route/README.md)。

## Progress73：连接内未决购买保护已通过有限检查（2026-10-05）

Windows 与 Web 共用 Core 的 entered/flushed/unknown 保护：同一实际连接上的角色、完整模型、目录、界面关闭和 HMR 变化不能清除未决请求；严格更新的可信连接才可退役旧请求，仍要求新的完整模型和玩家新的明确意图，不自动重发或推断成交/拒绝。旧 holder 契约缺失会 poison 并拒绝；最后来源复核必须是真实完整 checkpoint。

本批 11/11 组完成：Rust 286 次跨配置执行、14 项新增 distinct；Node 256 项、13 项新增 distinct；严格 TSC 与 shared WASM 编译检查退出 0。994 项声明输入中 13 项既有授权变更、981 项保护、0 新源码文件，Source02/实际回执经独立只读复核 0 blocker；Source01 缺陷与原执行证据保留。Node Core 响应是夹具，完整 TSC 消费依赖图未冻结；未运行 WASM Instance。Progress72 的 155 次服务端执行仅限定承接，不算本批新执行。

下一项贯通 Gateway owner lease 的 typed NPC outcome，再接关联/幂等购买回执和完整投影到两端 Core，随后构建新的生产组合。本批不提高整体 parity 百分比；此前“整体 35%”缺少固定验收分母，不再使用。实际客户端、浏览器、移动真机、生产组合及 Candidate 仍未验收，整体 goal 未完成；用户“继续代码，暂不操作界面”的限制持续有效。详见 [Progress73 QA](npc-gold-buy-connection-barrier/README.md)。


## Progress72 · 2026-10-05 UTC · 普通金币购买 typed 本地处理结果

Session / InProcess Runtime 新入口要求 canonical 认证和 StartGame，默认 unsupported 不执行旧购买；普通 BUY 实际解析 Used 也被 typed 路径拒绝。每笔局部 capture 在真实校验或金币、库存两次提交后产生 Rejected / Committed，incoming_unique_id 仅为入包增量身份。已知结果经后处理 Err 或 unwind panic 仍保留；缺结果为 Unknown，不推断拒绝或自动重发。旧 Buy 内层四字段、binary、特殊经济、GameShop 与原完整收尾保持。

Source02 994 current / 994 after / 992 before，5既有变更＋2新增、987保护；Source01 panic审查问题原样保留且未执行。实际 Simulation153＋Gateway2＝155次测试执行通过，15项新增全执行，独立复核0 blocker；每次 fresh actual C≥50GiB 与原 Guard PolicyB 退出闭环齐全。Web243 / TSC / 共同客户端与WASM仅以未变有效输入限定承接71历史通过，不计新执行。

本批是领域接口与有限检查，未发行公开 processing receipt，未解决客户端 Unknown 恢复、未重建生产组合。下一项同连接未决屏障，再接精确请求关联与两端回执；实际界面按用户“继续代码，暂不操作界面”暂缓。玩家/保存重登/移动真机/最终 frontend / Candidate 未验收，goal active。

详见 [Progress72 QA](npc-gold-buy-processing-outcome/README.md)。


## Progress71 · 2026-10-05 UTC · 普通金币购买有效期与共同日期精度

服务端普通 Gold Trade fresh 物品按 Crystal 名称标签创建有效期：首个成功括号匹配、Int32 前缀及溢出归零，m/h/d 使用时长，M/y 按日历钳制月末，未知单位使用 Unspecified MaxValue。一次 UTC 捕获用于本笔购买；完整交付载体与 live sidecar 一致，日期超界、距离/金币不足或容量失败均在金币和库存提交前拒绝。带标签模板仅占格，不贡献客户端 fresh-compatible 容量证明。Pearl/BuyBack/Used 保持原路径。

公共 Rust 的 plain UserItemExpireInfo JSON 改为规范 signed i64 十进制字符串，同时读取旧整数；原 Crystal binary i64 不变。Windows/portable 共用同一编解码与严格完整载体门槛。Web 仅在 expiry 单字段形状下用原始 JSON source 恢复旧 unsafe 整数，无 source 时保留 unknown；新字符串原样透传，不放松 UID/count/dura。安全 legacy 数字仍保留既有 JSON 语义，Rental/Sealed 未在本批迁移。

Source01 联合992输入及完整 current/after/before核对：12既有改动＋2新增、978原文件保护。实际 Rust 138＋112＋73＋32＋19＋2＝376次跨配置测试通过，shared WASM 编译通过；17项新增 Rust 具名测试已执行，Native/portable 的5项重复不加成独立场景。Web Node 实际111＋84＋48＝243项通过，parser 的35内部断言组已含于一项文件测试，不另加总；9项新增 Node 与实际严格 TSC 通过。每次 Cargo 使用 fresh actual C≥50GiB、原 immutable Guard PolicyB，首个漏建收据目录的 pre-Cargo 拒绝保留且不计通过。

当前自然 Trade 目录没有 timed 商品，新增事务由 test-only World-local 目录经真实 Session BuyItem 入口验证。本批是源码与有限检查；Progress70 的 EXE/renderer/Next/独立包不能视为包含新代码。下一项先增加服务端直接返回的 typed 普通金币购买处理结果，再接精确请求关联与客户端恢复；完整 NPC 服务与后续组合重建继续排队。用户“继续代码，暂不操作界面”持续有效，实际玩家/保存重登/UI/移动真机/最终 frontend/Candidate 未验收，整体 goal active。

详见 [Progress71 QA](npc-gold-trade-expiry/README.md)。


## Progress69 · 2026-10-05 UTC · 共同NPC金币买入容量证据

服务端完整快照提供可空的 `npcGoldTradeCapacity` 两字段证据，Windows/Web 原样传入共同 Rust planner。None 保留旧严格身份与仅空格准入，false 禁用；true 只让 listed、身份无歧义且完整 raw Info/载体与 canonical fresh 相容的现代 Bag/Belt 堆叠贡献容量。Bag1/Bag2 同为 container0；Native 保留完整 u64，Web 限安全整数，旧跨 grid alias 仅在 NPC 专用证据门槛内允许。

局部物品/金币 mutation 在任何 handler 前同步撤销可用性，完整同 owner、当前 source stage 与库存指纹匹配才恢复；完整请求与发送 proof 保留证据，只有持久 Core authority 排除它，证据变化不释放 Entered/Unknown 屏障。没有新增购买 ACK、超时重试或 JS 容量算法。

Source02 的 990 当前源码/after 备份及 989 before 完整核对，25 旧文件变更＋1 新文件、964 旧文件保护。实际 Rust 五组 128＋107＋68＋32＋19＝354 次跨配置执行及 shared WASM 编译检查通过；Node 108＋83＋48＝239 项通过，新增 Rust28/Node10。TSC 仅限定承接 Source01 实际通过：Source02 只改四个 mjs 夹具，生产 TS/TSX 与编译有效输入未变，不称新 TSC 执行。Source01 的 19/1/1 失败及原日志保留，夹具精确修复且原断言未弱化。

下一项 Progress70 复用既有有限 builders，按原预算重建 Native、三 renderer、Next 与 Thin；Core24 输入/18 Rust 未改，仅限定承接。生产组合本轮尚未重建；name-tag expiry、无购买 ACK 恢复与完整 NPC 服务边界仍 open。用户“继续代码，暂不操作界面”持续有效，实际 UI/JS sink/HTTP/移动真机/最终 frontend/Candidate 未验收，goal active。详见 [Progress69 QA](shared-npc-gold-capacity/README.md)。


## Progress68 · 2026-10-05 UTC · 普通Gold Trade整笔购买修复

服务端普通Trade/Gold购买先在库存克隆中完成真实格位、自身数量上限、完整载体及身份检查；兼容的完整现代堆叠依合法Bag/Belt分区吸收数量，剩余量必须有真实空格。全部转换和容量规划成功后才一起写入金币与库存；fresh交付排除catalog UID与0，模板基础属性不写入AddedStats，失败不扣款、不保留部分合并。旧sidecarless根保留各packet grid内的编号，真实exact/nested/reserved冲突继续拒绝，旧物品只占格。Pearl/BuyBack/Used原路径保持。

最终Source04实际购买16/16、相邻NPC115/115通过（115已包含全部16，两个命令131次执行），独立复核0个确认blocker；两次真实失败原日志保留，旧runtime/tests.rs及全局allocator/codec/兼容谓词未改。813 Rust输入与备份匹配，前端553输入未改。

下一项接共同Rust客户端的兼容堆叠容量：由服务端完整快照提供可空roster/compatible-UID证据，Windows/Web原样透传，局部物品包同步撤销可用性，保持Core的Entered/Unknown屏障。客户端满包准入、name-tag expiry创建、无专用购买ACK恢复及生产组合仍需继续；用户“继续代码，暂不操作界面”持续有效，实际玩家/移动真机/最终frontend/Candidate与整体goal保持open/active。详见 [Progress68 QA](npc-gold-trade-capacity/README.md)。


## Progress67 · 普通NPC商店组合产物

Windows开发EXE、生产Core、三renderer、Next和359.68 MiB独立包完成有限构建复核，静态导出26/26，原体积预算保留。下一项服务端购买原子入包代码；实际界面/玩家链/真机和整体goal仍未验收。详见 [组合构建QA](shared-npc-shop-combined-build/README.md)。


## Progress60 · 普通仓库组合产物

当前Windows开发EXE、三renderer、Next01及359.05 MiB独立包的有限构建校验通过，Storage三包及复制产物接口闭合；实际界面/玩家链/真机和整体goal仍未验收。下一项普通NPC金币买入共享代码。详见 [组合构建QA](shared-storage-combined-build/README.md)。


Latest follow-up: [Shared ordinary Storage painter, portable/runtime ABI and actual finite integration regressions](shared-storage-ui/README.md).

Earlier follow-up: [Storage shared planner, exact Web transport identity and finite regressions](storage-transfers/README.md).

Earlier follow-up: [Shared Mail Windows/Web combination build, strict TSC and fixed-budget standalone](shared-mail-combined-build/README.md).

Earlier follow-up: [Native Mail exact quote ticket, socket-entry seam and focused regressions](native-mail-quote-ticket/README.md).

Earlier follow-up: [Native Mail connection stream, reset preservation and actual focused regressions](native-mail-stream/README.md).

Earlier follow-up: [shared Mail parcel source and actual Native/Web focused regressions](shared-mail-parcel/README.md).

Earlier follow-up: [shared Mail compose source and actual Native/Web focused regressions](shared-mail-compose/README.md).

Earlier follow-up: [shared Mail inbox/reader source, three WASM consumers and isolated-package HTTP](shared-mail-inbox/README.md).

## Progress70 · 2026-10-05 UTC · 共同NPC容量两端组合构建

本批将 Progress68/69 的普通金币购买与共同容量证据修复构建进 Windows 开发 EXE、三种 Web renderer、Next 与独立包，并完成独立实际产物复核。未改服务端购买规则、协议或 Zone，不据此增加 backend/server parity 百分比。Core 完整24输入及18 Rust成员逐字节未变，仅限定承接既有实际生产构建；不称新的 Core 构建。

Windows EXE 104599040B 已归档且未启动。GPU/GL/shared WASM 为30547434/21966635/31510131B，gzip 为5902438/5384473/6243678B，原 WASM/gzip/JS预算不变。每对当前及复制后的 Storage/NPC 各7静态导出检查通过；Module 检查不代表 Instance 或 JS sink运行。Next实际严格TSC与13页面通过，双编译manifest绑定 Core9191/Bevy1808。

独立包实际377189647B（359.72 MiB）/7299文件/775目录含根/零链接，原360 MiB上限余297713B。完整 Next与依赖43897文件实际流式哈希及最终成员稳定；553源和完整备份匹配，Native814输入、Core8/Bevy173历史保留。Next实际仅改tsconfig与next-env两项生成元数据；无未授权漂移。

下一项修 name-tag expiry创建及 Web JSON日期精度，再处理无专用购买ACK恢复与完整NPC服务边界。用户“继续代码，暂不操作界面”持续有效，实际登录/战斗/保存重登、UI/JS sink/HTTP、移动真机、最终frontend/Candidate未验收；整体goal active。详见 [Progress70 QA](shared-npc-gold-capacity-combined-build/README.md)。


Earlier follow-up: [Native command ownership and fresh EXE](native-command-ownership/README.md), [shared Spells/combat](shared-spells-combat/README.md), and [controlled GL2 weight pair, canonical browser regressions and package-selection scope](webgl2-weight-pair/README.md).

Previous follow-up: [compact600 shared Quest, real-scale repair and ordinary save/relogin](compact-quest/README.md),
[mobile More Diary entry, three locales and retained resize failures](mobile-diary-entry/README.md),
[opt-in WebGL2 shared UI and default-path regressions](webgl2-shared-ui/README.md),
[landscape touch Bag, explicit Inspect and retained failures](touch-bag/README.md),
[shared browser bag, DPI/focus evidence and retained failures](shared-bag/README.md),
[shared equipment and final Web save/relogin](shared-equipment/README.md),
[native equipment](shared-equipment-native/README.md), and
[portable reward hints plus rebuilt Windows repair](portable-hints/README.md).
Those records supersede the initial gate status below for their bounded scope.
The [actual native N1/N2 route](native-ui/README.md) is also recorded separately.
The historical first-round matrix and failures below are preserved.

CP-00 is reviewed. CP-01 has shared source, native integration and real browser
player evidence; its actual Windows GUI gate remains open. The overall
[cross-platform goal](../../../CROSS-PLATFORM-CLIENT-GOAL.md) is active.
This is not whole-game, deployed-release, Bevy browser-renderer or mobile-device acceptance.

## Changed behavior

- A dependency-free Rust quest policy owns explicit diary endpoints, profile,
  stage, current NPC offer, pending state and reward selection. Windows uses it
  directly; Web loads it through a small standalone `platform-web` WASM host.
- Missing or invalid metadata never becomes endpoint zero. NPC template indices
  are not compared with live object ids. V2 N2's explicit `0/0` endpoints allow
  normal diary accept/finish without an open NPC dialog.
- Finish requests for the same quest cannot bypass pending state by changing
  reward choice. Exact request id/operation/quest/reward receipts still govern
  release; NACK allows retry. ACK alone does not award items or complete a quest.
- The Web loader fails closed with visible localized feedback and retry. A retry
  uses a fresh JS module URL because browsers cache failed dynamic imports.
  Pending feedback reserves space above the buttons instead of covering details.
- N16 copy now says Zombie2, matching current authoritative content. All 22 V2
  titles and objective message projections have been checked.
- The JS/WASM package is 127,086 WASM bytes (about 124 KiB), independently of Bevy.
  Source fingerprint, each file's hash/length and combined immutable URL hash
  are checked before prebuilt use. This is the first host slice, not CP-02 UI.

## Verification matrix

| Gate | Result / scope |
| --- | --- |
| `client-core` Rust | 16 tests pass; no dependencies |
| `platform-web` native bridge | 4 tests pass |
| Actual generated WASM + Web adapter + prebuilt failure gates | 18 tests pass via `npm run test:client-core` |
| Native shared UI library | 1102 pass, 1 existing ignored, with `native-ui` |
| Windows quest metadata parser | 2 focused pass; adjacent `gameplay_bridge::tests` 98 pass |
| Windows executable | Latest working source builds successfully; GUI player route unexecuted |
| Web typecheck | Pass with external incremental cache |
| Next production compilation | `next build --webpack` passes using existing assets and external output directory; not the full asset-generation/publishing pipeline |
| Quest localization | Pass, 22 authoritative V2 definitions plus feedback in four languages |
| Existing quest menu entry | `test:system-menu-quest-button` passes |
| Real browser N1/N2 + logout/login | Pass against the isolated local Gateway described below |
| Browser blocked JS import → visible retry → enabled action | Pass; real network failure and UI retry, without page reload |
| Desktop CDP touch emulation | Quest selection and orientation gate observed; usability gaps remain |
| Actual Android/iOS | Unexecuted; no device/lifecycle claim |
| Production deployment / complete asset pipeline | Unexecuted |

The adjacent native checks preceded the final same-quest pending correction;
that correction was verified by the complete 1102-test native UI suite and a
fresh executable build. Existing whole-crate formatting issues were not swept
into this change; scoped whitespace and core/bridge formatting checks pass.

## Real player path

The normal password UI created isolated account `cpweb0930a` / Warrior
`CPWeb0930`. Credentials and signing keys remain outside this evidence folder.
The browser used `http://127.0.0.1:13100/?bevyRuntime=0` (compatibility renderer)
and WebSocket `ws://127.0.0.1:19110/ws`, with presentation/cadence `newcomer-v2`.
No QA reward grant, debug teleport, direct game-state edit or fabricated server
reply was used for the player path. Mouse and touch input acted on actual DOM
controls; state observations and receipt capture were read-only.

1. Registered, logged in, created a character and started normally.
2. Selected Assistant Jane; ordinary approach walked from `(288,616)` to
   `(284,607)`. Accepted and completed N1 through the quest window and current
   NPC offer. Exact successful receipts were observed.
3. With no NPC dialog open, selected N2 and pressed Accept. Sent
   `acceptQuest`, `npcIndex: 0`, quest `2110002`, request
   `qs-0000000000000003`; the matching server receipt succeeded.
4. Opened inventory and equipped WoodenSword normally. N2 became ready.
5. Without reopening the NPC dialog, pressed Complete. Sent `finishQuest`,
   `selectedItemIndex: -1`, request `qs-0000000000000004`; matching receipt
   succeeded. N2 became completed and N3 became available.
6. A development hot reload also exercised disconnect/save. Separately, normal
   Menu → Logout, password Login and Start Game preserved N2 completed, weapon
   equipped, gold 200 and position `(284,607)`. No second N2 reward was awarded.

[Receipts](browser-receipts.jsonl) · [relogin state](n2-normal-relogin-state.json)

![N2 available without NPC dialog](n2-available.png)
![N2 completed](n2-completed.png)
![N2 after normal logout and login](n2-normal-relogin.png)

## Failure and touch evidence

CDP blocked the actual versioned JS URL before reload. The character could enter
the game, but N3 Accept was disabled and the quest window showed Retry. After
unblocking, clicking Retry requested `mir2_platform_web.js?retry=1` and the
versioned WASM, removed the error and enabled Accept. N3 stayed available;
the recovery check did not submit it. [Recorded results](core-recovery.json).

![Module unavailable](core-unavailable.png)
![Recovered after retry](core-recovered.png)

In one continuous CDP session, `844×390`, five touch points and coarse pointer
were applied. Actual touch events opened/selected the diary. At `390×844` the
portrait gate covered the viewport. These are desktop emulation results only.
The diary was just 158×225 CSS pixels in landscape: text and action targets are
too small for mobile acceptance. The install prompt also stayed Chinese in an
English session. Both are retained for the CP-02/CP-03 layout/localization work.
[Emulation measurements](touch-smoke.json).

![Touch landscape](touch-landscape-quest.png)
![Portrait gate](touch-portrait-gate.png)

## Provenance and limits

- Local Gateway binary: 75,434,496 bytes, modified `2026-09-23T21:17:34Z`,
  SHA-256 `a79307fb7f7ed779771e5c6c49a3e47504196c59f2353d6b5c60b010ffa5d201`.
  It used isolated file storage and current V2 content. It is an existing binary,
  not a newly built Gateway from this working tree; this is compatibility
  evidence, not exact-source server or capacity acceptance.
- Newly built Windows development EXE: 103,401,984 bytes, SHA-256
  `04a4ceb4cb1a85f78977ec97ba0529f2da1a49dc4fe9fea60fe12116d4a36683`.
  It has not been packaged, published or used for this real browser route.
- Shared artifact content version:
  `43982b58248512934646d31795fa29f91bc601ff8fc75f4845a54314d51fdcfa`.
  Source and prebuilt assets must be included together in a publishing commit;
  this checkpoint itself makes no commit/deployment claim.
- Existing accepted saves, staged catalog transport/capacity work and shared
  server authority are preserved. No server gameplay rule changed here.
- Browser automation initially used agent-browser 0.27.0. Its daemon later
  returned EOF; the existing browser was exercised through direct local CDP.
  Early harness clicks before bootstrap completion were corrected to wait for
  ready controls. An external Next build cache needed its own node_modules
  junction; the initial missing-module development screen was not accepted.
- Screenshots retain development badges. Quest row/tab style shorthand warnings
  discovered by the real interaction were corrected. The intentionally blocked
  module produces an expected console error during recovery evidence.
- Actual Windows keyboard/mouse N2 → logout/login, public release settings,
  Bevy browser UI, mobile readable controls and actual device lifecycle remain
  unchecked. Passing shared tests does not close those gates.

## Reproduction

From the project root, use explicit external Cargo targets if disk space is low:

```text
cargo +1.95.0 test --manifest-path apps/game-client/client-core/Cargo.toml --target-dir <core-target>
cargo +1.95.0 test --manifest-path apps/game-client/platform-web/Cargo.toml --target-dir <web-core-target>
cargo +1.95.0 test --manifest-path apps/game-client/client-bevy/Cargo.toml --features native-ui --lib --target-dir <bevy-target>
cargo +1.95.0 test --locked --manifest-path apps/game-client/platform-windows/Cargo.toml --target-dir <windows-target> --bin mir2-platform-windows gameplay_bridge::tests
```

From `apps/web`: `npm run test:client-core`, `npm run test:quest-localization`,
and `npx tsc --noEmit --pretty false --tsBuildInfoFile <external-cache>`.
The build fixture copies only required source/package files to a clean temp
tree, uses deliberately nonexistent Rust tools, and verifies successful prebuilt
use plus rejection of stale source, corrupt WASM and a wrong immutable URL.

- [Native mail send-flight Source05 and focused acceptance](native-mail-send-flight/README.md) — Progress53; code/controlled fixtures only.
- [Web malformed MailSent ACK repair and fresh replays](web-mail-ack/README.md) — Progress54; code/controlled fixtures only.

- [Progress55 Web shared Core SendSlot](web-mail-send-slot/README.md)：63 Node 测试、三项显式 Rust 回放、1,480 whole-JSON 字符串对，以及未放宽预算的实际 259,237 B Core。仅源码/有限检查验收，C2、组合包和实际界面另验。

- [Progress56 shared Mail editor/painter](common-mail-editor/README.md)：Native/Web 共用 Rust 编辑与金额规则；Node86、92次跨配置 Rust 执行及1,508完整字节回放通过。仅源码/有限检查验收，组合构建与实际界面另验。
