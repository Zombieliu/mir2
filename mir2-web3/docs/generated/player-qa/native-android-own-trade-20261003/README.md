# NI-15 本人交易报价共享投影 — 源码与 APK 基线（2026-10-03）

本叶结论：本人报价 snapshot **SOURCE / headless PASS**；NI-15 **PARTIAL**，
完整 Windows-alignment goal **Active**，AP-01–21 不缩减。
未安装新包，未证明实际 Android JNI/原共享 overlay 消费、真实账号、
在线社交/交易结算、完整手机 UI 或真机。旧截图与邮件 JNI 证据不重绑。

## 精确源码与修改范围

- 产品源码：`a9698682322d27bab6be8bab69327841f540f89c`；父提交：`48caf6d477c188597560f8dbc5ad0a7dd32f8e82`。
- 独立 worktree：`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync`，分支 `codex/android-shared-sync`。
- [PR #253](https://github.com/Zombieliu/mir2/pull/253) 保持 Draft，不合并或推回 Windows。
- 完整冻结 Windows 分母：`3d735745f1117d42a7859e87604a106351dca935`。
- 发布前真正来源 `codex/playtest-registration` 仍为
  `56ee063fb4f9d6b581ec548659066ef9c5e05e9b`；较冻结分母 +1 提交、83 文档/证据文件、0 功能源。
  旧 Windows 分支不是降低分母的依据。
- 功能范围严格 5 文件：共享库新增 `native_trade_ingress.rs` 并在 `lib.rs` 导出；
  Windows `trade_projection.rs` 变薄委托；Android `social_ingress.rs` 与
  `shared_shell.rs` 补 accepted-owner snapshot 接入。
- 没有修改原 Windows checkout、共享 SocialModel/overlay、runtime、Java、
  认证、服务端交易、背包、钱包或存档规则。已有 25 个社交公共入站保持。

## 根因、复现与纯展示修复

遵循 investigate 的先编译复现、再修复、按源码绑定回归流程；
CodeGraph 未初始化，沿已定位源码检查，未重复初始化或委派探索。
Windows 从 personal world snapshot 的 `stage5Systems.trade` 读取本人的保留报价；
Android 此前只消费 guest 的 TradeGold/TradeItem，不能把它们当 own ACK。

原始 compiled red `red-rust-own-trade-host` 为 0 passed / 1 failed / exit101：
实际 headless Host.receive/bind fixture 后自己的 125 显示为 0。
这是父 HEAD 加新回归的未提交输入，不冒称干净基线、Android JNI 或真实网络。
同一阶段 Windows 原有 5 项报价测试通过。
修复后未提交/格式化前定向 green 为 Android9、共享5、Mac Windows5；最终干净提交另跑九门。

将冻结 Windows 的 OwnOffer 字段与生产函数抽成两端共用纯投影；
去注释/空白后生产函数 SHA
`9c1bc3a2a1f153c622d314e3fd2670e949c4cd8f41a18dda431cbf3f2f5a40c5`，schema SHA
`2636e2a8908647af8495c45040903e30bd2fe0bdf7e84992b36230ddf123c9dd`，均与冻结 Windows 相同。
Windows 的完整原测试模块（含 fixture）字节不变；
共享复制测试仅归一格式化尾逗号/空白/注释后相同，fixture 使用既有共享 inventory projector。

Android 仅在既有角色/world 校验和社交 owner 绑定后调用：
root owner、唯一 selfPlayer、object ID 与名字必须一致。
交易已 open、partner/nonce/currency/完成标记有效，物品 canonical bag 槽位、
u64 实例、数量与 tooltip 身份匹配，才显示本人报价。
10 槽位及空洞、同名不同实例、上限 u64、201 数量/堆叠图片、guest 金额与物品均有回归。
缺失、无效、相同、完成或不同 nonce 不覆盖；显式清空仅改本人，不能开启、
复活或完成交易。只产生内部只读事件，`success=None`，不授予物品/货币。

候选 clone 经 typed serialization 和队列容量检查后才原子提交。
snapshot raw 1 MiB / model512 KiB；既有总 64 条/4 MiB FIFO 保留，
count/byte 背压不提交新报价、不丢回执；切图保留交易，断开/新 session 清空。
headless Host 仍留在 StartingGame/待 render-ready，不绕过地图渲染屏障。

## 保护核对与最终九门

65 输入 SHA 在每门/构建前后稳定；60 非本叶整文件、57 旧 Java 测试方法体、
10 认证/会话方法、5 冻结物品/数值 helper 未改。
两原 worktree HEAD/branch/status 保持，这是 Git checkpoint，
不是未跟踪文件递归字节证明。

| 门 | 精确提交结果 |
| --- | --- |
| Android 普通 lib | 421 passed / 0 failed |
| Android ui-preview lib | 454 passed / 0 failed |
| 共享 native-player-ui | 1297 passed / 0 failed / 10 原有 ignored |
| runtime lib | 296 passed / 0 failed / 1 原有 ignored |
| API31 arm64 普通与 preview | 两门 PASS |
| 强制重新运行 Java 两变体 | 各81 tests / 6 classes / 0 failure / error / skipped |
| Mac host 上 Windows 原报价回归 | 5 passed |
| Mac host 上 Windows guest-tooltip 回归 | 1 passed |

不是 Windows 全量串行 gate 或 Windows OS 实测。旧 ignored 不计通过。
Java XML 按本次检查名、变体分开归档；各门时间/命令/源绑定见
[结构化证据](source-evidence.json)。现存编译 warning 未作零警告声明。

## 两份新原生 APK（未安装）

版本仍 Code35 / `0.1.32-gameshop-phone`，minSdk31 / compileSdk35；
Gateway URL 空，不连接或部署生产，不是正式 release-size/性能验收。
ELF 与本变体 stripped output SHA 相同，仅证明构建归属，不证明剥离程度。

| 变体 | APK bytes / SHA-256 | 原生 ELF bytes / SHA-256 |
| --- | --- | --- |
| debug | 533340599 / `1c47a16d8df2b1b34d0ced2bcf892d4ae893743a06e9bb73d4da8e15b71547c0` | 145606384 / `e6720d984b94cf7b57b5282f112fe98cf329cd97cb7b9adb4fa40457aea01348` |
| preview | 541394679 / `0124a3a9c844348fdbd99c640b518afb84b6e65a8bdcd34d6658b1684178b489` | 149630000 / `7345de229afcefbdce0aefed333a09a82a5b38879a29adf9708f50480daccb0a` |

[普通 APK](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/own-trade-20261003-Ogipdk/final-apks/mir2-native-own-trade-debug-a96986823.apk)、[离线 preview APK](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/own-trade-20261003-Ogipdk/final-apks/mir2-native-own-trade-preview-a96986823.apk)。

各包选定6647 UI PNG +3 metadata 全字节匹配实际源目录；
Items1003、StateItem5192、MagIcon/MagIcon2各224、UI_32bit4。
冻结 UI_32bit470–473 原像素匹配；entity override pack
`android-archer-mount-bow-proof-20260912`，
manifest SHA `929d146535bde9b61c75c33af0e98852dcc3aef83320869aac9f06f4bdd530d9`。
不是完整资源/音频/可信更新验收；APK、资源缓存和日志均不进 Git。

只读设备清单为1个 API31模拟器、0物理手机。没有本源新安装/截图/录屏，
实际社交 Java→JNI→原共享消费者和 GPU 未复测；
旧 `2be65cc9` 邮件结果只属于其原源码，旧 GPU 零错误 FAIL 保留。

## 剩余门与下一叶

| 项目 | 当前状态 |
| --- | --- |
| 25 公共社交入站 + own offer/nonce/锁定 snapshot producer | SOURCE / headless PASS |
| 共享社交意图出站、写入失败、精确 pending 身份与回执 | OPEN |
| 当前包实际 Android JNI / 原 SocialModel / overlay 消费 | OPEN |
| 真实登录、会员/权限/仓库/邀请/报价/结算、断网/重连 | OPEN；服务端权威，不绕过认证或操作真实存档 |
| 完整手机窗口/多指/IME/GPU/音频/资源/更新 | OPEN/既有 FAIL；UI 修补需明确重新批准 |
| 真机、最终人工验收、完整 AP-01–21 | OPEN；完整 goal Active，NI-15 PARTIAL |

下一叶先复用共享社交出站规则，再取得当前源码实际 JNI；
完整玩家目标不降为“几个模型能构建”。

原始目录：`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/own-trade-20261003-Ogipdk`。
截至建档58 regular files / 1076024606 bytes；
[raw index](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/own-trade-20261003-Ogipdk/raw-payload-index.json) SHA-256
`bf4dd6382381e8e726f6cefe20ac59a0e2e24d998943cd8b37b48e7ae4f554c6`。索引自身及建档后的报告不在索引内，
不称全目录最终快照。编译红/定向 green/最终门分别保留。
