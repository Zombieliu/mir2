# NI-16 Hero 入站 — 源码、APK 与回归证据（2026-10-06）

本叶接受 **Android 公共 Hero 包转发、角色绑定后的 typed 宿主生产者及准确回执 FIFO 的源码回归**。
NI-16 **PARTIAL**，完整 Windows-alignment goal **Active**；完整 AP-01–21 不缩减。
本批没有实际 Hero JNI/界面操作或真实登录验收，不是“原生 Android 全部完成”。

## 来源与范围

- 产品提交：`0a3b24efa48272b9d1a1b33a63a116b9b5e626f7`，父提交 `7cf975c81932443b57c9a5af43c92611914960e5`。
- 独立 worktree：`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync`，分支 `codex/android-shared-sync`。
  [PR #253](https://github.com/Zombieliu/mir2/pull/253) 保持 Draft；不合并、不推 Windows。
- 冻结 Windows 完整分母：`3d735745f1117d42a7859e87604a106351dca935`。
  2026-10-05T16:10:27Z 只读来源仍为 `56ee063fb4f9d6b581ec548659066ef9c5e05e9b`；
  较冻结 +1 提交、83 文档/证据、0 功能源。本轮没有扩大或缩小分母。
- 仅五个 Android 功能文件：新增 `src/hero_ingress.rs`、模块注册 `src/lib.rs`、
  `src/shared_shell.rs`、Java `GatewaySession.java` 及其测试。
  共享 HeroModel/UI、认证方法、服务器权威和 Windows 功能未修改。
- 426 输入在十门、构建、安装、运行前后稳定；421 个非本叶整文件、
  57 个旧 Java 测试、10 个认证/会话方法、140 个旧 Host 函数保留。
  两原 worktree 的 HEAD/branch/status 保持；这是 Git checkpoint，不是未跟踪内容递归哈希证明。
- CodeGraph 未初始化，没有擅自初始化；按 investigate 在已定位文件中证实根因和回归。
  三轮失败并回退的手机布局仍需人明确重新批准，本轮没有重启该布局。

## 根因、修复与失败保留

1. Java 未转发 Hero 公共名字；独立 compiled JVM red 1 test / 1 failure。
2. 有效大 HeroInformation 被通用 65 KiB 宿主包上限误断线；compiled Rust red 0 passed / 1 failed。
   完整 HeroInformation 单独 512 KiB，其余 Hero 包保留 16 KiB，并从场景/实体小解码路径隔离。
3. 新 Host 边界回归发现 rejected Start 未清 Hero：24 passed / 1 failed。
   仅在原失败边界补 Hero reset，重跑 25/25；三份原始失败结果及日志均保留。
4. 冻结参考检查曾误把已有 skill epoch getter 提取、两处音频命名空间提取和
   rustfmt 参数尾逗号当成本叶改动。只为这三项增加精确映射，未知差异仍失败；
   这是检查准备修正，不是产品规则修改。记录是摘要，不冒充逐字 stderr。
5. APK 生成器沿用了上叶文件名后缀；产品源、输入与 ELF 绑定原本正确。
   仅改名为本源后缀，APK 字节未变，旧命名 JSON/日志和纠正记录保留。
   首次运行收集启动器的 shell 引号错误在 JS 执行前失败；修正引号后采集，
   不计入产品失败或成功，不声称有逐字文件归档。

接通原 HeroModel 消费的 24 个公共名字，不新增或复制 Hero AI/战斗/物品/技能规则。
元数据在 validated SelfPlayer owner/name 与原 skill epoch 绑定前有界暂存；
绑定 clone/replay/commit 原子提交，保留到达时钟。模型上限 2 MiB、保留队列 4 MiB、
暂存 64 包、准确回执 16 条；背压失败不丢队首，准确 item/skill 回执先于合并最新状态。
角色/连接失败退休；切图保留个人 Hero，与原 Windows SceneReset 行为一致。
物品展示复用现有 tooltip/metadata/geometry；自动喝药展示不是授予或托管物品。

原 Windows 四个 Hero 函数核对，三体直接相同，另一个仅既有 epoch getter 精确映射；
13 个共享 Hero 文件中 12 个原字节相同，另一个仅既有两处音频命名空间映射。
两份 test-only 冻结展示参考检查 sparse/null/缺 geometry/默认索引/游标覆写，
生产代码仍委托共享投影，不复制规则或另建手机端规则。
Host headless 回归止于 typed 生产者，不等同 Android 真实 JNI 或共享 UI 已消费。

## 干净产品十门

| 检查 | 结果 |
| --- | --- |
| Android 普通 / preview lib | 448 / 488 passed，0 failed |
| 共享 native-player-ui | 1304 passed，10 原有 ignored |
| runtime lib | 296 passed，1 原有 ignored |
| API31 arm64 普通 / preview | 两门通过 |
| 强制新编译执行 Java 普通 / preview | 各 90 tests，6 classes，0 failures/errors/skips |
| Mac host Windows Hero FIFO / skill FIFO / 协议子集 | 1 / 1 / 21 passed |

ignored 不算通过；Mac 子集不是完整 Windows OS gate。
通用结果中的 compiled 字段仅按 Rust 摘要识别，Java 执行以强制任务与 XML 为准。
精确命令、时间、输入列表摘要/完整列表索引及失败见 [结构化证据](source-evidence.json)。

## 两份可安装原生诊断 APK

Code35 / `0.1.32-gameshop-phone`，minSdk31 / compileSdk35，Rust release-profile；
分发仍是 Debug/uiPreview，不是签名商店 Release。Gateway 为空，没有远程 Web 页面，
没有连接生产或登录获准服务。版本号未变；识别产品以源码和 APK SHA 为准。

| 变体 | APK bytes / SHA-256 | ELF bytes / SHA-256 |
| --- | --- | --- |
| debug | 533715343 / `78b1a8648fa8b352b172952c7873ccd8a9bdedfd3f5d5688a7a5fb1235bcc8da` | 145981128 / `f8841ae7f24716921d78593692784ab7bf69216fd392d4a0d09fb82db6a8ba89` |
| preview | 541874335 / `7619c3ed9eb3420f0b1a31753e1ab704eeebf2b6e089af090e9e318ca7da5de1` | 150109656 / `d272a98f576550c34996663b83c356d9ea59568ce0156b251d7a2711b9481df9` |

[普通 APK](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/hero-ingress-20261005-q5sBAA/final-apks/mir2-native-hero-ingress-debug-0a3b24efa.apk) ·
[离线 preview APK](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/hero-ingress-20261005-q5sBAA/final-apks/mir2-native-hero-ingress-preview-0a3b24efa.apk)。

两变体 ELF 与各自实际 stripped 输出一致；选定 6647 PNG + 3 metadata 逐字节匹配，
UI_32bit470–473 冻结帧匹配；不是完整地图/全资源/音频/缓存/更新接受。
均保留数据安装并回读 base.apk SHA 匹配，没有 clear/wipe/uninstall、重建 AVD 或改原 Pixel5。

## 本源模拟器运行：仅既有路径回归

专用 Mir2_API_31_ARM64、Android12/API31、arm64-v8a，
1080×2340 / 440dpi；本轮库存 1 模拟器 / 0 真机。
沿用既有 AVD，不声称全新 AVD cold boot 或正常 nativeResume 验收。

- 正常包 PID7409：启动可见共享原登录，ID/PASS 为空，显示 Test server not configured；
  preview 参数没有注入离线社交场景。GL0x0506 = 1，fatal = 0。
- preview PID7493：五个原离线 Java→nativeEvent→共享 social 事件观测到；
  本人 125 与 guest17、u64 物品堆数 201/3、钱包777、原 Inventory+两交易窗可见。
  GL0x0506 = 0、fatal = 0 仅限此短采样，不能推翻历史 GPU 失败。
- 两张本源原图已实际查看；仍是 **既有 social JNI 回归，不是 Hero JNI 或 Hero UI**。
  model own_locked=true / UI ui_trade_locked=false 差异保留；没有触控、报价写入、
  权限邀请、pending、锁定画面或结算接受。零图形错误门仍 **FAIL**。

[普通包截图](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/hero-ingress-20261005-q5sBAA/runtime/normal-preview-isolation-pid-7409.png) ·
[既有交易 JNI 截图](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/hero-ingress-20261005-q5sBAA/runtime/trade-jni-pid-7493.png)。

## 仍需完成

实际 Hero Java/JNI→共享消费者、原 Hero 背包/装备/状态/技能/分配/设置窗口、
出站命令与准确 pending/回执、操作后恢复和真实线上托管均 OPEN。
完整手机布局重启批准、真实 HTTPS/WSS 登录→角色→StartGame、共享 Zone 移动/退出保存、
NI17–20、全资源音频/缓存/更新、零错误 GPU、真机与人工验收仍 OPEN。
完整 goal Active，不把本源构建/安装/社交回归提高为完整 Hero 或 Android 接受。

原始证据目录：`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/hero-ingress-20261005-q5sBAA`。
只读索引 94 文件 / 2160621102 bytes，SHA-256
`ec862bea0521404f2d485d5d4fb8509a2530fc822016f454aaea9cc9998e8c78`；
索引精确排除自身、后续文档/推送记录，APK/缓存/密码/签名密钥不入 Git。
此报告生成前 Android 远端仍 `8a307e16c665ad52fa6b2d17261b57c8b59332e0`，
PR open Draft；本报告不宣称后续提交或推送已完成，最终交付另行核验。
