# Android 共享 Hero 窗口触控修复（2026-10-06）

本批真实 API31 模拟器上，Java/JNI 输入到原共享 Hero 模型与六个窗口场景可见，
四个标签切换、两种窗口关闭与关闭后的短时保持已逐张原图确认。
这是离线原生宿主/共享窗口的有界证据，不是全套手机 UI、联网玩家流程或真机通过。
**NI-16 PARTIAL；完整 AP-01–21 分母不变；Windows 对齐 goal Active。**

## 精确来源与范围

当前产品源码 `aef89f9426ab7b74d49bf8d61f05eac9f1126c24`；
直接父提交 `1b85d871483599326726ed18186ec7725ea4fa35`。
本批只改 Android 的 `src/mobile_ui.rs` 和新增 `src/touch_cursor_bridge_tests.rs`。
429 个输入哈希清单 SHA-256：
`6363f420e37773651d243df3312435c8ea8432494c00f6c551fee7a9a8b78144`；
427 个其他整文件相对直接父提交不变。
共享 UI/英雄模型/规则、Java/认证/网络、Windows 与服务器本批均未改。
原两个工作区的 HEAD/分支/status checkpoint 不变；不是递归未跟踪字节证明。

冻结 Windows 功能基线 `3d735745f1117d42a7859e87604a106351dca935`。
在 `2026-10-05T22:02:48.265Z` 只读查询，当前选定 Windows 分支
`codex/playtest-registration` 仍为 `56ee063fb4f9d6b581ec548659066ef9c5e05e9b`：
比冻结源 +1 提交、83 文档/证据路径、0 功能路径。
旧 `codex/windows-player-journey` 不是本轮选定来源；不修改/推送 Windows 分支。
PR #253 保持独立 Draft，base 不变；推送后另留只读核验，不把未推送记为已推送。

## 先失败，再修复

1. `068c6ee64c53fce21cbaed3efc7b9606aae37460` 新增六个仅 uiPreview 的完整
   Hero Java → 原 MainActivity/nativeEvent 场景。精确源码门通过，但首次实际 PID9725
   只收到10条 Java 输入、没有完整 Hero 窗口观察；GL506=5，失败原图/日志保留。
2. `1b85d871483599326726ed18186ec7725ea4fa35` 修正诊断把 Android
   “可使用网络”误当成“宿主已联网”的判断。仍保留真实 active-host 阻断、原强断言和
   enabled=true，未修改认证/socket状态。原六窗口实际 JNI/模型/画面通过；
   但相同坐标的标签切换/关闭全部未生效，追加180ms持按也失败。窗口可见不是点击通过。
3. 本批 `aef89f9426ab7b74d49bf8d61f05eac9f1126c24` 修复确认的宿主输入断点：
   Android 原先只发布 CursorMoved，原共享 Hero handler 实际读取 Window.cursor_position；
   故原生端无坐标，桌面/headless 旧分支不能替代实测。

已安装 Bevy0.19/Winit0.30 Android 后端不支持系统 cursor warp。
现在只让原 Window 在本 UI 帧看到触摸位置，并在 PostUpdate/UI layout 后恢复原 cursor，
先于原 Winit Last 同步。只恢复 cursor、不覆写其他窗口属性，替代/取消/重复样本均有回归。
不复制 Hero 的标签、物品、技能或战斗逻辑，不往诊断中塞模型/pending/ACK。

第一次测试采集是未编译 E0609（误读队列私有字段），明确不算语义 red。
修正采集后原生 helper 的已编译 red 0/2；修复后聚焦 green 6/6，
覆盖原共享 handler 同帧输入、清除、实际 schedule 注册与 Last 前恢复、取消和替代窗口。
旧断言未删改。前述失败文件与本次独立结论均保留。

## 同源完整门禁

| 此精确源码检查 | 结果 |
| --- | --- |
| 普通 Android host Rust | 484/484，0 ignored |
| ui-preview Android host Rust | 531/531，0 ignored |
| 强制新鲜 Java Debug / UiPreview | 各93/93，各六类，0失败/错误/skipped |
| API31 ARM64 ordinary / preview check | 各退出0 |

五门源码输入前后稳定；聚焦6项包含于全量，不累加。
此两个有界修复没有重跑共享/runtime/完整 Windows OS 门；上一批的结果仅为历史证据。

## 两个原生诊断 APK

Native Rust release 库在 Debug / uiPreview APK 中，不是商店/签名 release。
code35，name `0.1.32-gameshop-phone`，minAPI31，ARM64。
两包 Gateway URL 为空；普通包显示 “Test server not configured”，uiPreview 编译隔离且禁止联网。
不是 WebView 套壳，不加载远程网页，不能宣称真实登录/线上版本验收。

| APK | 字节 | SHA-256 |
| --- | ---: | --- |
| ordinary Debug | 533784343 | `ba3e83ee0893ed2d83065f08e9ce3c89fa25379b82e9e28d4838604f46c41000` |
| uiPreview | 542009887 | `0f5c8a912394df134b9520eb681a07accb8e0fbde9f752f12c51af4c02e84184` |

本机文件：
[ordinary APK](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/touch-cursor-20261006-1L1L30/final-apks/mir2-native-touch-cursor-debug-aef89f942.apk)；
[uiPreview APK](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/touch-cursor-20261006-1L1L30/final-apks/mir2-native-touch-cursor-preview-aef89f942.apk)。

每包所选6647 PNG +3 metadata 字节匹配；原32-bit UI470–473保持冻结源。
entity proof pack `android-archer-mount-bow-proof-20260912`：1atlas/5pages/7848rects，
manifest 2506377字节、SHA `929d146535bde9b61c75c33af0e98852dcc3aef83320869aac9f06f4bdd530d9`。
它仍不是获准完整正式资源包。当前变体 APK ELF 与该变体 nominal stripped-output 匹配；
原 strip 警告保留，字节相同不等于实际成功去符号或正式体积通过。

专用 `Mir2_API_31_ARM64`、`emulator-5554`，Android12/API31/arm64，
`sdk_gphone64_arm64`，2340×1080，physical0。
保留数据安装两包并回读安装 APK，字节数/SHA 与上表一致；未 wipe、卸载或清全局日志。

## 实际 JNI、窗口与触控

普通隔离 PID11069：没有诊断 Java/Hero 观察，原登录页。
六个 preview 场景分别 PID11152/11227/11302/11376/11450/11525：
每个从真实 Java 送10条原格式消息，等待原 observer 与 renderer；
入站能力=true、active host=false，wrong-owner 血量9999不覆盖，
Hero owner42/对象12/HP67/MP23/背包42/装备14/两技能/u64上限/201数量/
自动药35%与45%/原回执序号3，与玩家 HP80/MP20/Gold777/12件背包分离。
原六窗口图已查看；spawned 标志不是 Hero 在地图中的实体渲染证据。

相同原坐标实际触控在移除场景 PID11525 和新装备场景 PID11761，8张原图全部人工查看：

| 操作（原2340×1080像素坐标） | 观察结论 |
| --- | --- |
| Hero背包关闭 888,18 | 背包消失，玩家HUD/地图保持 |
| 装备页初始 | 原女英雄预览可见，只算基线窗口 |
| STATS I 1662,113 | 实际切页，HP67/180、MP23/90 |
| STATS II 1749,113 | 实际切页，bag7/wear3/hand2；fixture上限0不是完整属性验收 |
| SPELLS 1837,113 | JNI FireBall Lv2 Exp7/3、Healing Lv1 Exp0/2 |
| CHAR 1575,113 | 从Skills实际切回原女英雄装备页 |
| Hero角色关闭 1875,19 | 原窗口消失，HUD/地图保持 |
| 再等待3秒 | 仍关闭；仅短时观察，不是长时间恢复验收 |

六次tap +基线/等待两张图，不把8张图称为8种业务流程。
自动采集的 pending 视觉字段不改；另附 `touch-visual-verdict.json` 独立人工结论与哈希。
全部短样本 fatal0/缺失所选素材0/cursor-warp错误0。
七个冷启动独立 PID 仍共15条 GL0x0506（1/0/1/1/3/4/5），**零错误 GPU gate FAIL**。
触控新 PID11761为1条；同PID累计日志重复采样不能叠加为独立GPU错误或独立稳定性测试。

## 原始证据与未完项

本批本机原始目录：`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/touch-cursor-20261006-1L1L30`。
三个追加索引覆盖各98/77/77文件（不含 APK、索引自身、delivery及后续文档/推送记录），
精确哈希见 [source-evidence.json](source-evidence.json)；旧预提交54份索引不重写。
APK/原PNG/日志/缓存/密码/签名密钥不入 Git。

手机布局仍未接受：Hero背包遮左侧HUD、原Hero belt过小、原desktop窗口几何仍存在。
真实 login/list/StartGame/Zone/save，Hero地图实体/完整操作/权威回执、
全触屏/IME/多指、完整资源/光效/音频/更新/nativeResume、零错误GPU、
真机与最终人工验收均 OPEN。没有真实认证/业务发送/服务端ACK，没有生产或真实存档写入。
继续有界安全代码叶；联网需获准测试环境/版本/普通账号，凭据由用户本地输入。
