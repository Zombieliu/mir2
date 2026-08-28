# Windows 原生可玩闭环验收清单

> 2026-08-28 VIS-01 exact-head motion Candidate evidence: clean revision
> `94f8e4f032643fdb826d6ef0ae82b360b4dcc83d` produced attested Candidate
> `WN-CANDIDATE-VIS01-MOTION-20260828`. The 67,435,520-byte Release EXE has
> SHA-256 `E40C5216A29DE870DA7898F0ACABE331E7310C583D249C2F66DC3210692050F4`;
> all 32,594 Candidate files are covered by the package identity chain and the
> payload aggregate is
> `167EB82528CD5ADEDA5621B170233FA2B8314540F11A95E65EB812ECA8D5B726`.
> Detached-CMS and final nonvisual verification pass. This closes exact-head
> build/package evidence for the two motion leaves, not visible play. The EXE
> was not launched, the internal certificate is not publisher Authenticode,
> and the keyed-map manifest still records 2,508 missing source entries.
> Authenticated same-EXE WSS, live mounted input, UI/chat, mouse combat,
> skills/VFX, complete monster/map denominators, 100/125/150% DPI, native
> 30-minute soak and human visual/audio/feel acceptance remain open;
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-08-28 VIS-01 mounted motion cadence/packet-clock checkpoint: revision
> `eb174e94eecde4a6e24f63d16616e2dfb9a03589` drives the native motion
> window from the current player movement descriptor instead of a global six-
> frame constant. Mounted Walk is now 8 x 100ms with matching right-direction
> sprite phases and camera cancellation; mounted Run stays 6 x 100ms. A live
> authoritative packet time window is consumed at its already-elapsed phase,
> while missing, future, expired, inverted or overflowing metadata safely
> falls back to the descriptor duration. Explicit movement frame counts are
> bounded to the Web-compatible 1..8 range. Focused 5/5 and full Windows
> 446/446 pass, and independent review is P0=0/P1=0. No exact-revision EXE,
> package, live mounted input route, screenshot or human-feel evidence exists
> for this head. The wider player/action/UI/VFX/monster denominators and final
> authenticated same-EXE WSS, DPI, soak, human and signing gates remain open;
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-08-28 VIS-01 continuous player-locomotion checkpoint: revision
> `532ddc6be0a0c38313fdd39fe9e0af82b883371b` makes overlapping native
> player Walk/Run segments continue from the prior segment's current
> fractional coordinate, restarts the visible phase with the matching
> authoritative sequence, and rejects the bounded self stale-source echo that
> previously could move both actor and camera back to the old tile. Only stale
> locomotion is replaced; queued combat remains FIFO, and monster/NPC paths do
> not use the player-only frame override. Runtime passes 194/194 and Windows
> passes 443/443, with independent P0=0/P1=0 player and Web/Crystal reviews.
> This head has not produced or launched an exact-revision EXE and has no new
> screenshot or human feel evidence. Mounted Walk eight-phase cadence,
> packet-carried timing, complete player actions, UI/chat, mouse combat,
> skills/VFX and final authenticated same-EXE live WSS, real DPI, 30-minute
> native soak, human acceptance and formal publisher signing remain open.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-08-28 VIS-01 player-sprite geometry/package checkpoint: native Windows
> no longer invents a 48x64 rectangle when a player frame is absent from the
> entity atlas. The ten packaged player families use exact per-frame Crystal
> metadata as a verified fallback, including original offsets and transparent
> pixel hit/highlight behavior; other atlas misses still produce no layer.
> Source, staging and final-package closure all pass at 10 families / 34
> libraries / 22,944 declared frames. Full Windows tests pass 441/441, both
> Candidate self-tests pass, and exact revision
> `ef25aec83b8023003ae648b4a2955a4e9ec76362` produced nonvisually verified
> Candidate `WN-CANDIDATE-VIS01-PLAYER-GEOMETRY-20260828`. Its Release EXE is
> 67,398,144 bytes, SHA-256
> `1550B512930C54BA5356100B63976919A146E904F9A397D4EDE4CF653200FC3A`.
> The package carries a detached CMS statement verified with the internal
> certificate, while the EXE itself is not publisher-signed. At the
> 2026-08-28 observation, PID 225736 was a current-source debug/loopback
> inspection window, not exact-Release same-EXE evidence.
> `visualAccepted=false`, `accepted=false`
> and `globalParityPercent=null`: complete actions, UI/VFX, authenticated live
> WSS, 100/125/150% DPI, native 30-minute soak, human visual/feel acceptance
> and formal publisher signing remain open.

> 2026-08-27 VIS-00 code gate: the independent visual-parity branch now has a
> source-bound Phase-A ledger plus tested repairs for Arial routing,
> 8pt chat/nameplates, outlined ordinary NameView labels, remote
> normal/Transform body routing, Harvest/CWeapon-01/Skeleton transitions and
> Hidden/corpse opacity. HUD/damage typography, hover corpse names and additive
> weapon/wing layers remain open. This entry has no same-EXE
> capture and does not supersede the currently running frozen Candidate.
> Focused gates pass; the full suite remains FAIL because the frozen asset pack
> is missing the Archer walk and Mount frames required by two of 318 Windows
> tests. Treat VIS-00 as an
> implementation checkpoint only, with `visualAccepted=false`.

> 2026-08-19 alternate-class/combat-overlay update: the native resolver now
> mirrors the Web alternate libraries for Archer walk/run/range actions
> (`ARArmour` / `ARHair` / `ARWeapon`) and Assassin body/hair plus directional
> dual-weapon actions (`AArmour` / `AHair` / `AWeapon*`). Combat presentation
> now treats Crystal `ObjectStruck` as the pose packet and the separate
> `DamageIndicator` packet as the authoritative numeric hit/miss/crit/heal
> event. Floating events are deduplicated, bounded and animated. Native F-key
> targeting also ranks active-quest match, then live distance, and only uses a
> stale `selectedObjectId` as a same-distance tie-break. A live Deer changed
> `10/25 -> 9/25 -> 8/25`; the renderer-owned F12 frame visibly contains the
> red `1` damage floater at
> `generated/player-qa/native-windows-combat-overlay-round4/windows-native-combat-authoritative-round4-in-game-1787122701585-1.png`
> (SHA-256 `0E24F11B963382F02C82F0DAEEE745F51794A01460175E92523AABE7DCBD49AA`).
> Windows tests pass 104/104, shared runtime 133/133, the full Simulation lib
> passes 1183/1183, Web typecheck passes, and Release SHA-256 is
> `B6A7078173865DF3415B089DE4119EAA438886EF518AFD9DEC69054B445773D9`.
> Starter entity metadata contains no usable `maskPath` or nonzero shadow data,
> so shadows/effect masks require an exporter/asset-pipeline slice rather than
> a renderer flag. Spell/projectile effects, lighting, exact text and final
> same-scene/Gemini/human acceptance remain open.

> 2026-08-19 entity-composition update: Windows now loads 697 usable Crystal
> per-library frame-set catalogs from `original-ui/frame-sets.generated.json`
> and applies the source `start/count/skip/interval/reverse` data with explicit
> action fallbacks. Player rendering is a stable native composite of body,
> hair, front/rear weapon and mount layers; mounted actors suppress weapon
> layers. Native-only entity overlays now render Crystal-positioned NPC/monster/
> player labels, dead-state lines and the self HP bar from the authoritative
> payload. Live keyboard movement moved the same player from `288,615` to
> `287,613` while the camera, minimap, actor and label followed. Evidence is
> `generated/player-qa/native-windows-overlay-round1/windows-native-overlay-round1-in-game-1787118632558-2.png`
> (SHA-256 `343B4B05A9E67EF7B687F0DFA9B5D8D2F34E222A7AD95BC03AD6D1E4569E8DC4`).
> Windows tests pass 98/98, shared runtime 133/133, Release SHA-256 is
> `34B5053B58C2808FF5B7DD7ACE4FEE1721817BDE5977E7DE363EDA76F9B6A738`,
> and Web typecheck passes. This closes the per-library frame-set and basic
> composite/nameplate/HP subgates, not full entity parity: alternate class
> libraries, shadows/effect masks, combat effects, exact GDI/bitmap text,
> lighting, same-scene review and final human acceptance remain open.

> 2026-08-19 entity-animation update: native entity rendering now has a
> Windows-main-thread Crystal animation clock instead of selecting one static
> frame only when a Gateway message arrives. Authoritative packet hints cover
> self walk/run/death/revive and ObjectWalk/ObjectRun/ObjectAttack/
> ObjectRangeAttack/ObjectMagic/ObjectSpell/ObjectStruck/ObjectDied/
> ObjectRevived. Monotonic action sequences prevent repeated snapshots from
> restarting a cycle; monster run/range/spell normalize only to actions its
> current audited default catalog supports. Two F12 captures 254 ms apart are
> `native-windows-animation-round1/...-1787116494338-2.png` and
> `...-1787116494592-3.png`; they differ in 5,087 pixels, with the RGB difference
> bounded to world rows 60..532 and no HUD change. Live TownRevive then restored
> `18/18` at `288,616`, and `W` moved the authoritative map/HUD to `288,615`;
> evidence `...-1787116615991-4.png` has SHA-256
> `853B4E9502EB789EEDF6AC6EC3D2CD78C32A2351E668C7A98DBE252227E769FB`.
> Windows tests pass 90/90, shared runtime 133/133 after aligning its stale
> Windows dependency lock, Release builds, and Web typecheck passes. This closes
> only the per-frame/action-clock subgate: per-library frameSet metadata,
> class/equipment composite layers, overlays and final Crystal visual acceptance
> remain open.

> 2026-08-19 map/entity/vitals update: the native packet-first path now moves
> the authoritative terrain camera, minimap coordinates and HUD together; the
> entity producer reads the schema-v2 multi-page atlas with per-page routing and
> Crystal source offsets; and the retained map renderer rebuilds a page layout
> when camera movement exposes rects not present in the first viewport. This
> removes the 96x64/192x128 black terrain holes. More importantly, shared-Zone
> `ObjectHealth`/`Death` packets now override the lagging personal snapshot for
> both the native HUD and self entity. A persisted dead character visibly opened
> at `HP 0/18`, `V` issued the real `TownRevive`, and the client returned to
> `0 @ 288,616` at `HP 18/18`; subsequent field damage was immediately visible
> as `18 -> 17 -> 16`. Evidence is in
> `generated/player-qa/native-windows-map-layout-round1/` and
> `generated/player-qa/native-windows-vitals-round1/`; the near-reference frame
> is `windows-native-vitals-round1-in-game-1787114471473-3.png` (SHA-256
> `E3E0AEF851A90ECD1FC31FEE794DBB34A7BAA2B7A5B34465F3B53667CD56B68C`).
> Gates pass at Windows 83/83, shared runtime 133/133, Release build, Web
> typecheck, and Git Bash package-script syntax. A full offline asset staging
> run also passed: 8,325 files / 269.91 MiB were copied and every required
> manifest, map pack and Crystal UI sentinel was verified. Final same-coordinate
> scene, complete entity action/equipment layers, effects, DPI execution and
> human acceptance remain open. This local staging result does not yet prove a
> clean-checkout package: native-keyed and late ChrSel inputs must be generated
> or tracked by CI, and the EXE must still launch from outside the repository
> with `MIR2_NATIVE_ASSET_ROOT` unset.

> 2026-08-19 visual-parity update: the exact Crystal login screen is captured at
> `generated/player-qa/native-windows-visual-parity-round2/visual-parity-round2-login-1787100667931-2.png`
> (SHA-256 `A108BE789722A619BAFF0473DD3131F15AAC21EC8F5AD0108FA05D93BBC1D9CC`).
> The final structured report is
> `generated/player-qa/ai-visual-review/antigravity-native-login-final-20260819/review.md`:
> Accepted, 100/100, `sameScene=true`, zero visible issues. This supersedes the
> old login frame for visual acceptance only. Exact Crystal character select is
> now captured at `generated/player-qa/native-windows-select-round1/windows-native-character-select-character-select-1787102742999-1.png`
> (SHA-256 `1536F6C37759B85C1B705B746A2ED6E805B5DF078A8BD9F3B7B8B1889A898A85`)
> and reviewed at `generated/player-qa/ai-visual-review/antigravity-native-select-round1-20260819/review.md`:
> Accepted, 100/100, `sameScene=true`, zero visible issues. An occupied-roster
> capture at `generated/player-qa/native-windows-select-occupied-final/windows-native-character-select-occupied-final-character-select-1787103615727-1.png`
> (SHA-256 `F62C413AE599A12F662EAF99CA644947ACDA96B4A90F698FB9B891AB42778FBF`)
> verifies the animated class preview and selected row. The first exact Crystal
> in-game HUD Candidate is captured at
> `generated/player-qa/native-windows-hud-round2/windows-native-crystal-hud-round2-in-game-1787106442919-2.png`
> (SHA-256 `D7A6EBC21EBB56A52EF20A3F5C76A1CF441F1D6C33C1058CF4F8C83B641C578B`).
> The HUD-only evidence pair is reviewed at
> `generated/player-qa/ai-visual-review/antigravity-native-hud-round2-20260819/review.md`:
> Accepted, 88/100, `sameScene=true`, no P0/P1. This clears the first-Candidate
> threshold of 85, but not the final 92 target or final human acceptance. The
> original six-frame table still records the completed functional q1-to-q2 flow.

状态：Windows 原生可玩 Candidate 的实现、自动回归和本机证据齐全；等待最终 Crystal 1:1 人工前端接受。
目标程序：`mir2-platform-windows.exe`（Bevy/Winit/WGPU 原生窗口，不使用 WebView、Tauri 或浏览器 DOM）。
验收视口：1024×768。
权威状态：Gateway + Simulation；客户端只发送意图并渲染服务端回包。

## 1. 固定截图清单

下列文件均已验证为 1024×768 PNG。目录 `docs/generated/player-qa/` 按仓库策略被忽略，属于本机验收产物，不作为源码提交内容。

| 阶段 | 文件 | 可见验收点 | SHA-256 |
|---|---|---|---|
| 登录 | `generated/player-qa/native-windows-candidate/01-login-login-1787076463613-1.png` | 账号、掩码密码、Login/New Account、原生背景 | `4FAECEA3333295CB83FCE7F619451954DD6E499122AD9507168CEE52440CDE2C` |
| 选角 | `generated/player-qa/native-windows-candidate/02-character-select-character-select-1787076504781-1.png` | 服务端角色、职业/性别/等级、创建、开始、退出 | `3A32D8E2F384555953C78AE4DA6C67E1C78669E2BFC68E846C08132FA018F8AD` |
| 进图 | `generated/player-qa/native-windows-candidate/03-in-game-in-game-1787079477140-1.png` | 比奇省地图、实体、HP/MP、目标、任务、背包、原生控制提示 | `02A39AA420C5BFC1A1FB0822759D26862123D7BEAD1CB91C89DAAC1A` |
| 已接任务 | `generated/player-qa/native-windows-candidate/04-quest-accepted-quest-accepted-1787079731634-1.png` | 任务 2 `In Progress`、GingerTea 0/1、CraftsLady 邻近提示 | `F9253A68A819F847CC2B1241AF8A7DD0537DD4CAF4F5FA2F7B13FB79888C2103` |
| 战斗/进度 | `generated/player-qa/native-windows-candidate/05-combat-combat-1787080889965-1.png` | 目标 12/25 HP、任务 `Ready to Turn In`、GingerTea 1/1、背包物品 | `F725A2C00A999DC0074E5574B7B39B3B88F8DC94A894CDA15FADC63479CB0445` |
| 已完成 | `generated/player-qa/native-windows-candidate/06-quest-complete-quest-complete-1787081418998-1.png` | 任务 2 `Completed`、200 Gold、30 EXP 与奖励说明、奖励物品 | `B3CD2F848D84F52775AC135A9FA6E27538259E9936A2C014D9EDF4804B74A60A` |

## 2. 人工 E2E 操作清单

人工执行时不要设置自动截图状态变量，也不要使用 `demo`、GM、QA、传送或直接修改存档。账号和密码应由玩家在原生登录页输入。

| 步骤 | 玩家输入 | 预期权威证据 | 预期可见结果 | 对应截图 |
|---|---|---|---|---|
| 1. 启动 | 双击/运行原生 EXE | WebSocket 连接成功；尚未发送 Login | 先出现原生窗口和登录页，空凭据也不会卡住启动 | 01 |
| 2. 登录 | 输入账号、密码并按 Enter/点击 Login | `LoginSuccess` 携带角色数组 | 密码只显示掩码；失败可重试；成功进入选角 | 01、02 |
| 3. 创建/选择角色 | 创建 Warrior/Male 或选择已有角色，点击 Start | `NewCharacterSuccess`；随后 `StartGame(result=4)`、`UserInformation` | 使用服务端 `characterIndex`；收到玩家世界初始化后才进入游戏 | 02、03 |
| 4. 原生移动 | WASD/方向键，Shift+移动测试跑步 | `UserLocation`；AOI 对其他对象发 `ObjectWalk/ObjectRun` | 玩家和摄像机移动，地图碰撞有效；失败移动得到校正 | 03 |
| 5. 完成任务 1 | 靠近 Jane，按 T 交互，接取；移动到 CraftsLady 并交付 | `NPCResponse`、`NewQuestInfo/ChangeQuest`、`CompleteQuest` | 任务 1 完成并解锁任务 2，经验增加 10 | 03、04 |
| 6. 接取任务 2 | 与 CraftsLady 交互并接受 | `ChangeQuest` 显示任务 2 `InProgress` | 任务追踪显示 `Collect GingerTea (0/1)` | 04 |
| 7. 战斗 | 移动到 Scarecrow，选中目标，按 F/Attack | 每次有效命中有 `ObjectHealth`；死亡有 `ObjectDied`；击杀奖励有 `GainExperience` | 目标 HP 只随服务端回包下降，死亡/失败攻击不由客户端伪造 | 05 |
| 8. 任务物品 | 等待合法 Q-drop 结算；普通地面物品可按 R 拾取 | Quest 2 的 Crystal `Q` 项通过 `GainedItem(item_index=1112)` 直接进入任务包；普通地面物品使用 `ObjectItem` + `PickUp` | GingerTea 变为 1/1，任务进入 `Ready to Turn In`；普通地面物品出现时 R 可拾取 | 05 |
| 9. 死亡恢复 | 若途中死亡，按 V | `TownRevive` 后收到 `Revived/ObjectRevived`、`UserLocation` 和正 HP 快照 | 返回城镇复活点，HP 恢复，继续任务，不保存死亡前陈旧坐标 | 05 |
| 10. 交付任务 2 | 回到 Jane，按 T，选择完成任务 | `CompleteQuest` 包含任务 2；交付增量为 30 EXP、200 Gold、GoldenPendant、CopperRing | 任务显示 `Completed`，奖励和金币可见，GingerTea 被消费 | 06 |
| 11. 重登 | Logout，再次登录并进入同一角色 | 新会话的 `UserInformation/worldSnapshot` 与保存结果一致 | 坐标、HP、经验、200 Gold、奖励物品和任务完成状态保持；不能重复领奖 | 06 |

## 3. 自动权威闭环

脚本：`apps/game-client/platform-windows/scripts/smoke-native-flow.mjs`

2026-08-19 使用全新账号和全新角色的一次执行结果为 `ok: true`、退出码 0，覆盖：

- 新账号、登录、建角、StartGame 与真实世界初始化；
- 任务 1 接取、移动、NPC 交付；
- 任务 2 接取、27 个权威步走到战斗区；
- 同一 Scarecrow 从 100% 到 0% 的 20 次有效攻击回包；
- `ObjectDied`、30 击杀经验与 `GainedItem(item_index=1112, count=1)`；
- 玩家死亡后的真实 `TownRevive` 生命周期；
- Jane 交付、30 任务经验、200 Gold、GoldenPendant、CopperRing；
- Logout/Login/StartGame 后位置、经验、金币、背包和两个任务完成状态一致。

注意：Crystal `Q` 标记物品不是普通地面掉落。共享 Zone 的生产实现会把它直接送入合资格玩家的任务包；验收脚本因此要求真实 `GainedItem`，不能错误要求 GingerTea 一定先成为 `ObjectItem`。普通地面拾取仍由 `PickUp/PickUpTile`、Gateway 事务和 Simulation 测试单独保护。

## 4. 当前视觉限制

此证据包证明“Windows 原生且可玩”的垂直闭环，不代表 Crystal 画面已经 1:1：

- 多页 entity atlas、source offset、697 个逐库 frameSet、基础 body/hair/weapon/mount 复合、Archer/Assassin alternate class library、名字/自体血条和权威伤害飘字已接入；源导出仍没有可用的 `maskPath`/shadow 元数据，技能/投射物特效、精确 GDI/位图文字和高优先级动作打断仍未完整收敛；
- HUD 已迁移为 Crystal MainDialog/ChatDialog/快捷栏/小地图，首轮结构化审阅 88/100；最终 92 分门仍需位图字体和同场景整屏复核；
- 靠近 `0 @ 257,594` 的实走证据已到 `261,595`，但动态实体/控制状态阻断了最后 5 格；不能把该帧伪称为精确同坐标，需先固化可重复的实体布局或安全验收会话；
- 任务文本来自原始数据，部分源数据中的空占位或英文拼写问题仍会原样出现；
- Windows 125%/150% DPI、长时间运行、断网重连和发布包离线资源仍需最终人工矩阵确认。

这些限制应进入 `FRONTEND-1TO1-GAPS.md`，不能用本次“可玩闭环通过”替代最终 Crystal 1:1 人工接受。
