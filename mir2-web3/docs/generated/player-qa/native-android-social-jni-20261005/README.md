# NI-15 离线社交 JNI — 安装与共享模型证据（2026-10-05）

本叶仅接受四个 **离线 Java→实际 nativeEvent JNI→原共享模型** 观测场景，以及选定原版窗口显示。
NI-15 **PARTIAL**，完整 Windows-alignment goal **Active**；AP-01–21 不缩减。
真实登录、HTTPS/WSS、社交操作/pending、交易结算、完整手机布局、零错误GPU、
完整资源/音频/更新、真机及人工验收未通过。不是WebView或“原生版全部完成”。

## 精确来源与隔离

- 当前产品：`3f27e8e9e9a41679eaacebaa3891daf0e5a05b56`；本叶父提交 `8a307e16c665ad52fa6b2d17261b57c8b59332e0`。
- 先前产品 `73909efa4a069fbd6b5bdf0d7761f53fdc20d2dc`、`17aebd06fbb317151e8349f9127cffc1ba80fb20` 的构建/失败/截图保持原绑定，不重写。
- 独立 worktree：`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync`，分支 `codex/android-shared-sync`；[PR #253](https://github.com/Zombieliu/mir2/pull/253) 保持独立 Draft，不合并、不推 Windows。
- 冻结完整 Windows 分母：`3d735745f1117d42a7859e87604a106351dca935`。
- 2026-10-05只读刷新来源 `codex/playtest-registration` 仍为 `56ee063fb4f9d6b581ec548659066ef9c5e05e9b`：
  较冻结+1提交、83文档/证据、0功能源，API与已fetch对象一致。SSH失败不算核验成功。
- 本叶累计4个功能文件：Android `OfflinePersonalIngressPreview.java`、其Java测试、`ui_preview.rs`、
  新增 `social_jni_preview_tests.rs`。没有修改原 MainActivity、GatewaySession、shared shell、
  SocialModel、共享trade UI、Windows或服务器规则。
- CodeGraph未初始化，未擅自初始化；按investigate在已定位文件中证实根因、compiled red/green、干净源回归。
  三轮失败并回退的手机布局需人明确重新批准；本轮无答案、没有重启布局修改。

## 真实失败与两处诊断根因

1. 新场景原先被当本地specimen，Java也没有事件流。新回归实际compiled red；
   初次Rust观察器访问私有Host字段导致E0616编译失败，已保留，没有扩大Host可见性。
2. 第一个干净产品全量preview回归468 passed/1 failed：原严格场景计数51应为55。
   仅更新计数，旧51项顺序和唯一性断言保留；当前源完整471通过，未删测试或忽略失败。
3. `17aebd06f` 已安装并实测：组队/行会/取消观测成功，但交易NOT_OBSERVED，己方物品格为空。
   原共享 `trade_dialog::sync` 收TradeAccept会打开 **Inventory加两张交易窗**；
   原诊断却等UiPanel::Trade。新compiled red0/1证实，修正诊断呈现条件，没有改共享规则。
4. 原己方Java样本只有bag icon，无TradeRenderer所需tooltipSource；新的JVM回归85 tests/1 failure，
   只在两种离线交易场景为两件物品附显式Info/UserItem资料。新Rust检查实例、201/3计数、
   item_index及原图标算法；不能用显示名或默认图标假装齐备。
   Java red在Debug任务失败后停止，通用runner复制的UiPreview旧84-test XML不是该red实跑证据；
   generic compiled字段仅为Rust摘要启发式，Java编译/执行以任务和XML为准。

当前定向Rust6及fresh Java85+85通过；最终干净源另跑十门并重新打包、安装、实际JNI。
CRLF/date/正则转义及旧目录EEXIST属于诊断准备错误，发生在对应动作前且未覆盖旧证据；
与上述产品测试失败分开记录，不冒充逐字stderr归档。

## 最终干净源与保护核对

69输入逐SHA在每门、构建、安装/JNI前后稳定；65非本叶整文件未改，
57旧Gateway Java断言、10认证/会话方法、5冻结物品helper、8旧preview测试、
52旧preview函数字节未改；唯一旧计数断言只将51改55。两原工作区HEAD/branch/status保持，
是Git checkpoint，不是未跟踪内容递归哈希证明。

| 检查 | 准确产品结果 |
| --- | --- |
| Android普通 / preview lib | 431 / 471 passed，0 failed |
| 共享 native-player-ui | 1304 passed，0 failed，10原有ignored |
| runtime lib | 296 passed，0 failed，1原有ignored |
| API31 arm64普通 / preview | 两门PASS |
| 强制重跑Java普通 / preview | 各85 tests / 6 classes，0 failures/errors/skips |
| Mac host Windows本人报价 / guest-tooltip / 协议子集 | 5 / 1 / 21 passed |

不是完整Windows OS gate，ignored不算通过，编译及无法strip提示保留。
精确命令、输入、日志哈希、起止时间、旧失败及设备信息见[结构化证据](source-evidence.json)。

## 当前两份可安装原生诊断APK

Code35 / `0.1.32-gameshop-phone`，minSdk31 / compileSdk35，Rust release-profile；
Gateway为空，不连接生产，没有远程Web页面版本。仍是Debug/preview，不是签名发行或release-size接受。
ELF归属与本变体stripped输出一致；normal ELF与上叶相同，因为新Rust观察器只在preview启用。
两份均保留数据安装到专用AVD并pull回base.apk，SHA与本地APK一致，没有clear/wipe/uninstall。

| 变体 | APK bytes / SHA-256 | ELF bytes / SHA-256 |
| --- | --- | --- |
| debug | 533340431 / `25450fcbc4eff0f7484e6857d9ab0e75d32961af7eafe006fdcde5af5db18842` | 145606216 / `e38f61ce5fc285f30b96f38e897d986ee638859eb564d564d576c90929aa1cac` |
| preview | 541445919 / `5029492813c970309a1c0e94cfec57dc42816d0e2d6afddf0bea4a62d7295c5e` | 149681240 / `9eb3fae3671d41f91f1af4bbd486ab761697e3f69825a704e485134d06a182a9` |

[普通APK](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-jni-20261005-lSQucc/correction/final-apks/mir2-native-social-jni-debug-3f27e8e9e.apk) · [离线preview APK](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-jni-20261005-lSQucc/correction/final-apks/mir2-native-social-jni-preview-3f27e8e9e.apk)。
两包选定6647PNG+3metadata逐字节匹配；UI_32bit470–473与冻结Git一致。
实体pack `android-archer-mount-bow-proof-20260912`，manifest
`929d146535bde9b61c75c33af0e98852dcc3aef83320869aac9f06f4bdd530d9`。
这不接受完整角色/地图/技能源帧、音频、按需资源发布或可信更新。

## API31实际JNI与画面

已有专用Mir2_API_31_ARM64，Android12 / sdk_gphone64_arm64 / arm64-v8a，
SwiftShader，使用原default_boot snapshot、未wipe或保存snapshot。每项force-stop新PID，
不是AVD冷启动；1模拟器、0物理设备。每项同PID/设备时间边界日志，不清logcat。

| 场景 | 新PID / 模型观测 | GL0x0506行数 | 已查看画面 |
| --- | --- | --- | --- |
| normal-preview-isolation | 6256 / DIAGNOSTIC_ABSENT | 0 | [截图](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-jni-20261005-lSQucc/correction/runtime/normal-preview-isolation-pid-6256.png) |
| group-jni | 6336 / OBSERVED | 1 | [截图](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-jni-20261005-lSQucc/correction/runtime/group-jni-pid-6336.png) |
| guild-jni | 6422 / OBSERVED | 0 | [截图](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-jni-20261005-lSQucc/correction/runtime/guild-jni-pid-6422.png) |
| trade-jni | 6506 / OBSERVED | 0 | [截图](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-jni-20261005-lSQucc/correction/runtime/trade-jni-pid-6506.png) |
| trade-closed-jni | 6590 / OBSERVED | 1 | [截图](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-jni-20261005-lSQucc/correction/runtime/trade-closed-jni-pid-6590.png) |

每项进程存活、无fatal记录。原model marker在Android logger及RustStdout镜像出现，
不是两次输入或两份独立证明；结构化摘要去除镜像重复，原log保留。

Java前置STARTING并先发公共packet，再发owner42的worldSnapshot，经原owner staging/reducer：
组队15人/邀请代次；行会200成员/200公告/112嵌套仓库槽/4105金币；
交易本人125与guest17分离、10槽/u64实例、本人1/8槽201/3数量、nonce/locked；
取消后本人/guest报价清空且钱包777、12背包项不变。不是登录、操作ACK、物品发放或结算。

画面实见原组队邀请、行会公告/统计、背包+两独立交易窗及己方图标计数，
取消后原消息框和背包。没有证明全公告/成员/仓库滚动、软键盘、按钮触控或权限操作。
**锁定完整呈现仍OPEN**：模型own_locked=true但原共享ui_trade_locked=false；
原local unlock优先级未改，不能据model marker把锁定UI记为通过。
**零错误GPU门FAIL**：新6336/6590各一条0x0506，旧5990失败保留；
其余短跑0条不是GPU修复或持续稳定性验收。

[同PID6590切后台返回截图](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-jni-20261005-lSQucc/correction/offline-background-pid-6590.png) 已查看：
HOT返回、同PID、原Inventory/取消消息和wallet777/201栈仍可见，片段无fixture重放/GL/fatal行。
preview策略明确跳过normal disconnect/relogin，因此不是正常认证、WSS重连、token或nativeResumeV1验收。

## 原始证据与后续

忽略目录 `/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-jni-20261005-lSQucc` 包含222个普通文件/4,317,078,847 bytes（含原/新APK和安装回读副本），
[raw-index.json](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-jni-20261005-lSQucc/raw-index.json) SHA-256：
`b536ea5934627a0da097a46d68f6a5082cac329c034f2083a047b3711518ec9d`。清单排除自身及随后文档提交/远端核验记录，
不把事后文件伪称已在快照。APK、缓存、许可资源、密钥和密码不入Git。

继续NI-16 Hero完整状态/物品/技能/回执宿主审计与共享接线，
NI-17–20和完整AP-01–21仍在分母。真实账号和获准测试环境、在线玩家闭环/Zone/保存、
完整触控手机UI、物理设备与人工验收分别补证；不把离线preview或Mac测试替代它们。
