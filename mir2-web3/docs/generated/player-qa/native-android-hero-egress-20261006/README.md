# Android NI-16 Hero 出站 — 2026-10-06

本页分别保存预提交快照和后续精确源码/安装包证据；完整 Windows 对齐 goal 保持 Active、AP-01–21 不缩减、NI-16 为 PARTIAL。
本页记录本批源码、编译和宿主回归。APK、实际Android JNI/UI、真实网络与真机分别验收，不能由本页测试代替。

## 精确提交、APK及安装交付

产品源码：`b5b6f4c6893cc62f6f9a9c0a1bf58887cf257605`。干净精确提交重跑十门通过，
427输入摘要仍为 `80ff854b7126d4d82b936ab0f72d685f356aeeaa21eae08d40c217e513dace41`。
源码已普通非强制推送并核验PR #253 open Draft，base不改；后续文档提交不是APK的源码SHA。

| 包 | 本机APK | SHA-256 | 字节 |
| --- | --- | --- | --- |
| debug | `/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/hero-egress-native-20261006-5oUXMA/final-apks/mir2-native-hero-egress-debug-b5b6f4c68.apk` | `da053ff928ac9e62b69786b5537cfdc2d965dd658db8b39524c316ac96d5dd97` | 533809759 |
| preview | `/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/hero-egress-native-20261006-5oUXMA/final-apks/mir2-native-hero-egress-preview-b5b6f4c68.apk` | `bd3176f16d9e5d617c068d59662678e09a466b6912c1118bda2f263dd7167ffa` | 541898095 |

两包Code35/name0.1.32-gameshop-phone/min31/compile35；Rust release、诊断Debug/uiPreview，不是商店release。
两Gateway为空；普通UIPREVIEW=false、preview=true且禁止联网。内嵌当前variant ELF匹配：
- debug：146075544字节，`cdd02c464fdc2372d44f5bc141bfc9ec48d36baed2cc4982a27bc0028f3e8cba`
- preview：150133416字节，`7d1ba2e5473dc0ed77a8b354c7eb91cb93ced410327cf6426fccd2d1a0cd2247`
Gradle的native strip warning保留；“匹配strip任务输出”不表示真的成功去符号，更不是release体积通过。

实体proof pack仍为android-archer-mount-bow-proof-20260912，
manifest `929d146535bde9b61c75c33af0e98852dcc3aef83320869aac9f06f4bdd530d9`；
两包选定6647PNG/3metadata匹配批准本地来源和冻结权重帧，仅为选定资源字节验收，非正式全资源包。

设备：既有Mir2_API_31_ARM64、Android12/API31/arm64、emulator-5554；0真机。
安装-r保留数据，分别回读安装base.apk并与发布前hash一致。未wipe、未重启或扩容原AVD。

两个新原图已查看，2340×1080横屏：
[普通登录页](</Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/hero-egress-native-20261006-5oUXMA/startup-debug-hero-egress.png>)，
[默认preview HUD](</Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/hero-egress-native-20261006-5oUXMA/startup-preview-hero-egress.png>)。
普通登录页显示Test server not configured；preview为OFFLINE UI PREVIEW — hud/NOT LIVE GAMEPLAY，
背景黑场是该hud-only诊断默认场景，不能称为实际地图或Hero显示。
冷启动PID8803/8866均存活、fatal0，约6秒样本各GL506=0；
这不关闭既有GL0x0506/完整world/反复切场/持续GPU门FAIL，不是Hero Java→JNI或业务操作实测。

原始交付索引：96文件/2,160,856,601字节，
`raw-delivery-evidence-index.json` SHA-256
`33c86dcbf21bdc7c3cfb868d2d015e67bd9da5b2497b4098a2ca1c692bb04bf1`。
包含当前bound十门、双APK/回读包、资源核验、安装、两个PID日志/原图和源码推送证据；
不含索引自身及其后的summary/文档/最终guard/文档推送产物。
APK、原始PNG/日志、缓存、密钥均不入Git。

已关闭：本批源码/实际Android编译、诊断包构建、选定资源、保留数据安装、默认冷启动。
未关闭：实际Hero Android Java→JNI→共享model/UI/窗口/按钮/pending；真实登录和在线流程；
完整手机布局、九语言/IME、多指触控、全资源/音频/cache/update、resume、Zone保存、真机与人工接受。
历史GPU FAIL和交易锁定UI差异保留。NI-16仍PARTIAL，完整AP21项与goal Active不变。
下一批直接补实际Hero Android JNI/UI独立证据，不借用这里的默认HUD或Mac C ABI当作完成。

## 根因与修补

原 Android 宿主缺少 Hero 出站注册与共享操作上下文接线；本批将12种闭合意图接到原认证传输队列。
共享pending、owner/name、epoch、Hero generation/actor、技能requestId和物品结果代次均精确匹配；
只有确定未发送的原草稿可以恢复，socket Sent不是权威ACK，不复制英雄AI/战斗/物品/技能/交易/存档规则。
冻结Windows命令oracle及12种wire schema一致，玩家技能键1–8与Hero键17–24保持分离。

进一步编译复现：普通/preview生产目标都报E0599，既有只读model() getter仅测试可见。
第六文件仅删除该getter的 `#[cfg(test)]`，整个函数体和其他入站字节保持不变。
此前额外索要批准过严：用户项目指令明确不因正常局部编译修补停工，本次按原授权执行。
没有新的第六文件问题人类答复，也不把自动续跑或默认选项当作授权来源。

未轮询FFI复现为1通过/3失败，修补后同组4通过：
尚未被Java复制的旧Hero请求按精确序列退役并释放lease，暂停时保留序列与FIFO，
满256项仍先清理；短缓冲查询不消费，其他域可继续发送。
已经复制/可能写出的请求不伪造撤回或Failed，晚结果只退役原lease，不污染新身份/等值新草稿。

## 来源与范围

基点HEAD `0ebd5c66a705cd4691b92287cb10b5d7ade9126f`，
分支 `codex/android-shared-sync`；下述是预提交工作副本，而非该基点已包含新功能。
产品提交SHA由后续source-commit-binding记录给出，测试清单 `bound=false` 不冒充干净提交验收。

六个代码文件：

- `platform-android/src/hero_egress.rs`（新增）
- `platform-android/src/lib.rs`
- `platform-android/src/gateway_bridge.rs`
- `platform-android/src/shared_shell.rs`
- `platform-android/src/hero_ingress.rs`（仅上述一行可见性修补）
- `client-bevy/src/crystal_ui/overlays.rs`（加法式别名/域意图提取，无布局修改）

427项输入摘要 `80ff854b7126d4d82b936ab0f72d685f356aeeaa21eae08d40c217e513dace41`；
最终聚焦、八项宿主及两项实际Android目标检查前后均稳定。
421受保护整文件、858旧函数/测试体、六文件以外源码、共享规则/Windows/Java/认证都不变；
两个原工作区HEAD/分支/Git状态一致，不宣称未跟踪目录递归字节证明。
六份状态文档更新，AP21项/NI20项矩阵字节不改；旧WIP报告作为独立历史来源保留。

## 验证

| 门槛 | 实际结果 | 不能据此宣称 |
| --- | --- | --- |
| 最终Hero出站/FFI聚焦 | 31通过 | Android JNI/触控 |
| Android普通宿主lib | 478通过 | Android真实设备 |
| Android preview宿主lib | 519通过 | 正常认证/网络 |
| 共享native-player-ui lib | 1304通过，10既有ignored | ignored通过/完整手机布局 |
| runtime lib | 296通过，1既有ignored | 真实Zone/保存 |
| 强制Java普通/preview | 90+90通过；各6类，0fail/error/skip | 实际JNI/WSS |
| API31 arm64普通check | PASS（旧E0599 FAIL已保留） | APK已打包/安装 |
| API31 arm64 preview check | PASS（旧E0599 FAIL已保留） | APK已打包/安装 |
| Mac-host Windows Hero FIFO | 1通过 | Windows OS全量gate |
| Mac-host Windows技能FIFO | 1通过 | Windows OS全量gate |
| Mac-host Windows协议 | 21通过 | Windows OS全量gate |

十门是十条回归/检查命令（Java一命令含两variant）；聚焦包含在全量、变体有重叠，不能相加报完整度百分比。
七个FFI新增测试覆盖owner/Hero/退出、暂停/render/focus、满队列、已轮询晚结果、
新等值技能草稿、缺模型/消费者、短缓冲与其他域FIFO。

## 证据、远端与未完成项

本机原始证据：`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/hero-egress-native-20261006-5oUXMA`。
预提交索引 `precommit-raw-evidence-index.json`：37文件/2,934,578字节，
SHA-256 `10011c8c91adb1d1a462c98fc250641dd6e973b49e12ed47590657d762662e7b`。
索引不包含自身以及后续collector/文档/提交绑定/干净源码重跑/APK或运行时产物。
[精确命令、计数、权限依据、历史失败与验收状态](source-evidence.json)。

2026-10-05T18:43:10.051Z只读远端核验：Android/PR #253 HEAD仍0eb，PR open Draft、base不改；
Windows来源 `56ee063fb4f9d6b581ec548659066ef9c5e05e9b` 相对冻结 `3d735745f1117d42a7859e87604a106351dca935`
仅+1提交/83文档证据路径/0功能源差异，不减分母、不推Windows、不合并、不部署或改真实存档。

[历史五文件WIP](../native-android-hero-egress-wip-20261006/README.md)保留6f17…输入来源、31项聚焦、
两项真实E0599 FAIL及41文件原始索引；不重绑为本批80ff…源码。
上批0a Hero入站APK/截图也不重绑。本批预提交时没有新APK、安装、截图、实际Hero JNI/UI、
真实登录/在线流程或物理设备。旧GL0x0506/零错误GPU FAIL、交易锁定显示差异、
完整手机窗口/IME/九语言、NI17–20、正式全资源/音频/更新、Zone保存、真机和人工接受仍OPEN/EXTERNAL。

下一步：提交验证过的源码和报告，在干净精确提交上重跑十门，再生成双APK并核对hash/ELF/资源，
保留数据安装后单独验实际Android Hero JNI/UI；不能把离线或Mac C ABI算作完整联网/真机完成。
