# NI-15 共享社交出站 — 源码与 APK 基线（2026-10-05）

本叶结论：17种 Group/Guild/Trade 操作 **SOURCE / headless PASS**，
NI-15 **PARTIAL**，完整 Windows-alignment goal **Active**，AP-01–21 不缩减。
未安装新包；未验收实际触控生产者、Android JNI、真实socket写入、
在线组队/行会/交易结算、手机布局或真机。旧截图不重绑到本提交。

## 精确源码与范围

- 产品提交：`0665a4f4ecc5449ca100d613561f78b40c4a9e76`；父提交：`414139f41a474928cf2541eee6529b0d58ba8916`。
- 独立 worktree：`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync`，分支 `codex/android-shared-sync`。
- [PR #253](https://github.com/Zombieliu/mir2/pull/253) 保持独立 Draft，不合并或推回 Windows。
- 完整冻结 Windows 分母：`3d735745f1117d42a7859e87604a106351dca935`。
- 2026-10-05 只读核验真正来源 `codex/playtest-registration`：
  `56ee063fb4f9d6b581ec548659066ef9c5e05e9b`，较冻结 +1提交、83文档/证据、0功能源。
  首次fetch成功，随后SSH ls-remote中断；已改用GitHub只读API，且API来源与fetch对象一致。
- 功能范围5文件：共享新增 `native_social_egress.rs`、`lib.rs`导出、
  `crystal_ui/overlays.rs`追加社交专用有界取出方法；
  Android `shared_shell.rs`注册/转发，`gateway_bridge.rs`追加闭合typed入队。
- 四个既有文件仅新增代码，原规则/方法/测试没有删除重写。Windows、
  Java认证/发送pump、SocialModel、runtime、服务端、钱包、战斗、存档规则未改。

原始证据目录于2026-10-03建立；测试继续并于2026-10-05完成，
保留每项真实起止时间，不把目录名当统一执行日期。

## 根因与红绿证据

遵循 investigate：先证明生产断点，再修改、绑定源码回归。
CodeGraph未初始化，沿已定位文件检查，未擅自初始化或委派探索。

共享按钮已有 NativePlayerUiIntent 与 pending，但原 AndroidSharedShellPlugin
没有社交转发器。编译后的实际 PostUpdate 注册回归 `red-social-egress-actual-registration`
0 passed / 1 failed / exit101，列出了真实调度但未找到转发器。
这是父HEAD加新回归的未提交输入，不称干净基线或Android运行证据。

修复后定向共享7、Android10、preview11通过；最终干净产品提交另跑完整十门。
注册回归只初始化真实Plugin调度，不执行完整startup/GPU；
生产转发/租约测试是headless手动就绪夹具，不证明手机按钮或正常StartGame成功。

## 传输边界与恢复

闭合17种wire类型：switchGroup / addMember / delMember / groupInvite /
requestGuildInfo / editGuildMember / editGuildNotice / guildInvite /
guildStorageGoldChange / guildStorageItemChange /
tradeRequest / tradeReply / tradeGold / depositTradeItem / retrieveTradeItem /
tradeConfirm / tradeCancel。serde字段逐项与冻结Windows一致；
既有UiCore行会仓库坐标校验复用，不另造权限、余额或结算规则。

pending操作通过原共享队列规则推导，不维护第二份Android规则表。
邀请捕获原共享UI的名字/代次，旧邀请不回答新邀请；缺少关联时使用原断线/
DataReset边界，不能猜测身份或假造ACK。确定未发送的full/invalid/stale操作
只释放相应pending，调用原草稿/邀请恢复，保留新草稿和权威数据。

仅InGame、接受的owner、无待切图/render请求、前台、联网、焦点窗口时转发。
StartingGame/加载/后台/断网/失焦保留有界队列；非游戏屏只丢弃自己的未发送域。
社交选择和其他域各自FIFO，每帧最多16条，不偷取邮件/装备/hero/buff动作。
原Gateway容量、序列和租约沿用；满时拒绝新项，不覆盖老项。
typed命令上限48KiB，为原64KiB JNI对象包装预留空间；
最大测试公告超过16KiB，不代表真实JNI字节边界已在设备上验收。

成功入队或Sent写结果**不**清除社交pending、不更改本人/guest金额、锁定、
成员、物品或结算；只等服务端权威读回。实际socket失败继续由未修改Java发送pump
触发断线/会话隔离，本叶未在Android实际网络上验证此路径。

## 保护核对与最终十门

68输入在每门/两构建前后SHA稳定；63非本叶整文件、
57旧Java测试、10认证/会话方法、5冻结物品helper未变。
17种公共协议schema与冻结Windows归一一致。两原worktree HEAD/branch/status保留，
仅为Git checkpoint，不声称未跟踪字节递归证明。

| 门 | 精确提交结果 |
| --- | --- |
| Android普通 lib | 431 passed / 0 failed |
| Android ui-preview lib | 465 passed / 0 failed |
| 共享 native-player-ui | 1304 passed / 0 failed / 10原有ignored |
| runtime lib | 296 passed / 0 failed / 1原有ignored |
| API31 arm64普通与preview | 两门PASS |
| 强制重新运行Java两变体 | 各81 tests / 6 classes / 0 failure / error / skipped |
| Mac host上Windows本人报价 | 5 passed |
| Mac host上Windowsguest-tooltip | 1 passed |
| Mac host上Windows协议子集 | 21 passed |

不是完整Windows OS gate。ignored不计通过；原编译warning保留，
Java XML按本次检查名和变体独立归档。精确命令、时间与源绑定见
[结构化证据](source-evidence.json)。

## 两份新原生 APK（未安装）

Code35 / `0.1.32-gameshop-phone`、minSdk31 / compileSdk35；
Gateway空、没有远程网页版本，不连接生产。不是WebView套壳或正式release-size验收。
本次日志确认两变体重新编译Rust后打包，不只复用旧so；
ELF与本变体stripped输出匹配只证明构建归属，Gradle仍提示不能剥离此库。

| 变体 | APK bytes / SHA-256 | ELF bytes / SHA-256 |
| --- | --- | --- |
| debug | 533340431 / `7aee6124e699aa669e74740b153f5d6efdab520eedf1cc0746c7ccfe251643ee` | 145606216 / `e38f61ce5fc285f30b96f38e897d986ee638859eb564d564d576c90929aa1cac` |
| preview | 541492383 / `0309a4f61aadee7254450070f2e8d29647f78a44b9b9e568e6cffcd7e945d3b8` | 149727704 / `a5a6358baef9dcca09059a0078010eff673d451f0534675e0380d46f16763fbe` |

[普通APK](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-egress-20261003-PEFxKS/final-apks/mir2-native-social-egress-debug-0665a4f4e.apk)、[离线preview APK](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-egress-20261003-PEFxKS/final-apks/mir2-native-social-egress-preview-0665a4f4e.apk)。

每包选定6647 PNG +3 metadata全字节匹配：Items1003、StateItem5192、
MagIcon/MagIcon2各224、UI_32bit4。冻结UI_32bit470–473原像素匹配。
entity override pack `android-archer-mount-bow-proof-20260912`，manifest SHA
`929d146535bde9b61c75c33af0e98852dcc3aef83320869aac9f06f4bdd530d9`。
不等于完整地图/实体/音频/可信更新验收；APK、日志、资源缓存和密钥不进Git。

2026-10-05只读设备清单：0在线模拟器、0物理设备。本版无安装/截图/录屏，
JNI、触控、GPU未复测；历史2be65cc9邮件JNI只属于原源码，
历史GPU零错误门FAIL仍保留，不据包体变化声称渲染优化。

## 剩余验收与下一叶

| 项目 | 当前状态 |
| --- | --- |
| 25公共社交入站、own-offer snapshot、17种出站 | SOURCE / headless PASS |
| 实际Android Java→JNI→原SocialModel/overlay消费 | OPEN |
| 实际UI生产者→原JNI租约/pump、网络写失败 | OPEN |
| 真实登录、成员/权限/仓库/邀请/报价/权威结算与重连 | OPEN |
| 完整手机UI、多指、IME、资源/音频/更新与GPU稳定 | OPEN/既有FAIL；布局修补待明确重新批准 |
| NI16–20、完整AP-01–21、真机与人工验收 | OPEN；goal Active / NI-15 PARTIAL |

下一叶补当前源码离线实际JNI/共享消费者证据，随后核验实际生产者与发送边界；
离线诊断不冒充认证网络，不改真实存档或部署生产。没有真机不妨碍其余安全代码工作。

原始目录：`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-egress-20261003-PEFxKS`。
截至建档59 regular files / 1076139539 bytes；
[raw index](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-egress-20261003-PEFxKS/raw-payload-index.json) SHA-256
`1a1893559400f691d6a0c66e274b835a44ecc08d7facf328bb51b4b642a68354`。索引自身及之后创建的报告未纳入，
不称最终全目录快照；原始编译红、定向绿、最终门和APK分别保留。
