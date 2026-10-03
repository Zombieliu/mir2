# NI-15 Android 社交入站 — 源码与 APK 基线（2026-10-03）

本叶结论：NI-15 **PARTIAL**，完整 Windows-alignment goal **Active**。
只完成 Group/Guild/Trade 公共入站、角色绑定、原共享模型折叠和对应两份 APK。
没有新包安装、实际社交 JNI/原共享 overlay 消费、真实账号、在线操作/结算、
真机或完整手机 UI 验收。不得把旧安装包、旧截图或邮件 JNI 结果重绑到本源码。

## 精确源码与范围

- 产品源码：`d4dbb124c2a4fe63364571fe48bd5af8b6463bee`。
- 产品父提交：`49aa576755ff8891e93026f7bfc7a2bd9a244e4b`。
- 独立 worktree：`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync`。
- 分支：`codex/android-shared-sync`；[PR #253](https://github.com/Zombieliu/mir2/pull/253) 保持 Draft，不合并或推回 Windows。
- 完整冻结 Windows 分母：`3d735745f1117d42a7859e87604a106351dca935`，AP-01–21 不缩减。
- 本叶发布前重新 fetch/核验真正来源 `codex/playtest-registration`：
  `56ee063fb4f9d6b581ec548659066ef9c5e05e9b`；
  相对冻结分母多 1 提交、83 个文档/证据文件、0 功能源码。
  旧 Windows 分支指向 `6ae080711fc7b3aaf06dd9c6bcf63f121682dbe4`，不据此降低分母。
- 源码修改严格 5 文件：新增 `social_ingress.rs`；注册模块的 `lib.rs`；
  `shared_shell.rs`；Java `GatewaySession.java` 及其原测试文件。
  没有修改 Windows、共享 SocialModel、原 overlay、runtime、认证或服务端规则。

### 根因与产品修复

使用 investigate 的先复现/后修复流程；CodeGraph 未初始化，沿已定位文件检查。
Java 先前缺失全部 25 个公共社交名字，Rust Host 也没有 social 角色/FIFO producer。
大公告/完整读模型还会落入小宿主/场景解码器上限；复用既有 UI 并不能自动接好宿主。

本叶将这些公共名字送入原 `SocialModel::apply_network_packet`，再发布
`push_native_social_model`。Start 阶段仅暂存；经过既有 personal snapshot
核验的 owner 才能发布。hero/显式错 owner/角色拒绝，角色变化无 reset 拒绝；
启动失败、断开和 render-ready 失败清空，切图保留个人会员/邀请/交易状态。
候选 clone 后校验、序列化和入队成功才提交，不保留共享拒绝的半份修改。
队列保持 FIFO，背压不丢回执，不用本地“成功”结算。

25 名字与三个域均有合法输入回归；成员/等级/权限/邀请、稀疏 112 槽位、
u64 上限物品身份、物品 tooltip、对方交易报价、匹配 pending 处理依旧来自共享代码。
只有五类大型读模型可到 512 KiB：
`GroupMemberInfo`、`GuildMemberChange`、`GuildStorageList`、`TradeItem`、`GuildNoticeChange`。
其余社交回执保持 16 KiB；模型 512 KiB，暂存+待发合计 64 条/4 MiB。
宿主聚合仍 32 条/8 MiB。分类只选择解码器，不授予认证、bootstrap 或地图权限。
`ObjectGuildNameChanged` 保留对象层路线，未知/非法场景仍走原 fail-closed 检查。

### 保护项

61 输入文件 SHA 在每个干净提交回归/编译/打包前后稳定。
56 个非本叶整文件不变；53 项旧 Java 测试方法体/断言不变；
10 个认证/会话/邮件结果方法不变。
物品补全两函数和三数值 helper，经去注释/空白及既有共享委托别名归一后，
函数体 token 与冻结 Windows 相同。模板/玩家/插槽/real-info 规则仍委托既有
`native_inventory_ingress`，不复制交易、权限、背包或钱包规则。
两原工作区 HEAD/分支/status 保持；这是 Git checkpoint，不是未跟踪字节递归证明。

## 编译红与修复证据

原始目录：`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-ingress-20261003-P7uJBG`，APK/缓存/日志不进 Git。

| 原始检查 | 实际结果 | 解释与限制 |
| --- | --- | --- |
| Rust `red-rust-social-host-envelope` | 编译后 0 passed / 1 failed | 宿主容量的 opaque transport 探针；原 120 字符公告不是合法共享内容，不能据此声称合法公告已进入模型。最终探针是 32 字符行加有界忽略字段 |
| Java `red-java-social-whitelist` | Debug Javac 后 2 tests / 2 failed | 两项新社交转发测试真实失败；只跑 Debug。初版 runner 顺手复制的 UiPreview XML 是旧结果，不计入本轮 |
| Rust `green-rust-social-focused` | 编译后 11 passed / 1 failed | 测试样本错误地用了 128 字符名/公告；修正为原共享 32 字符限制，未放宽生产规则 |
| 定向 green2 | Rust 17 passed；本地 TLS Java 4 passed | 包括原 envelope receive/bind/native-queue producer，不是实际 Android JNI 或线上 Gateway |

初始 Java XML 的复制目录被定向 green 重用；原红日志和 result JSON 保留，
**原红 XML 未保留**，不宣称原始证据完整无覆盖。最终全量 Java XML 改为按检查名、
变体独立归档。编译红不是编译错误；本地 TLS fixture 登录不是实际获准账号登录。

## 精确提交上的最终七门

| 门 | 结果 |
| --- | --- |
| Android 普通 lib | 412 passed / 0 failed |
| Android ui-preview lib | 445 passed / 0 failed |
| 共享 native-player-ui | 1292 passed / 0 failed / 10 原有 ignored |
| runtime lib | 296 passed / 0 failed / 1 原有 ignored |
| API31 arm64 普通检查 | PASS |
| API31 arm64 preview 检查 | PASS |
| 强制重新运行 Java 两变体 | 各 81 tests / 0 failure / 0 error / 0 skipped；各 6 classes |

操作名、开始/结束时间、每门源绑定和 XML 分项见
[结构化证据](source-evidence.json)。旧 ignored 保留，不把它们当通过或删除。
完整七门没有运行 Windows 全套 gate，也不等于实际认证/在线 Group/Guild/Trade。

## 两份新构建 APK（未安装）

版本元数据仍 Code 35 / `0.1.32-gameshop-phone`，不是新的发布版本。
Android minSdk31 / compileSdk35；Gateway URL 空，不连生产。
原生 ELF 与各自变体构建的 stripped output 字节 SHA 一致，仅作归属核对，
不是剥离程度或包体优化验收。

| 变体 | APK bytes / SHA-256 | 原生 ELF bytes / SHA-256 |
| --- | --- | --- |
| 普通 `com.mir2.web3` | 533331663 / `14212d2e748d2647ab756cb4e2cdd50d3f48c4edbafaf11e299a44a928128d8e` | 145597448 / `f1fe170cd948352f85de3fbc2b118020bd3f480cb332795539ad0dd91f1f87b4` |
| 离线 preview `com.mir2.web3.uipreview` | 541337559 / `4978cbf2fa4167921a63f9b81f78c2aa9e0bebed4808793773ae40bb92fd031d` | 149572880 / `15e79e1192bbb46f4c646205a655a34f2b2650a0d82b58ea6c49f717c13d3de1` |

[普通 APK](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-ingress-20261003-P7uJBG/final-apks/mir2-native-social-ingress-debug-d4dbb124c.apk)、
[离线 preview APK](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-ingress-20261003-P7uJBG/final-apks/mir2-native-social-ingress-preview-d4dbb124c.apk)。

两包各选定 6647 UI PNG + 3 metadata 字节匹配实际源目录；
冻结 UI_32bit470–473 字节匹配冻结 Git。Items1003、StateItem5192、
MagIcon/MagIcon2各224、UI_32bit4，均保留原像素。
entity override pack `android-archer-mount-bow-proof-20260912`，
manifest SHA `929d146535bde9b61c75c33af0e98852dcc3aef83320869aac9f06f4bdd530d9`。
这不是完整资源/音频/可信更新或 release-size 验收。
本叶未改包装配置，未以大包体声称生产发布或性能优化。

当前只读设备清单是 `emulator-5554` / sdk_gphone64_arm64，0 个物理手机。
没有安装/启动新包，因此没有本源码的新截图、录屏、冷启动日志、GPU 或 JNI 通过证据。
旧 NI-14 源码 `2be65cc9` 的邮件实测保留为其自身证据；不重绑。
既有 GPU 零错误门 FAIL 保留，本叶没有复测或将它变绿。

## 剩余门与下一叶

| 项目 | 本叶状态 |
| --- | --- |
| Group/Guild/Trade 25 公共入站到共享模型 producer | SOURCE / local TLS / headless PASS |
| 自己的 trade offer/nonce/锁定 snapshot | OPEN：不能把 partnerItems/partnerGold 当自己报价 |
| 共享社交意图出站及写入失败/回执身份 | OPEN |
| 实际 Java→JNI→原 shared SocialModel/overlay 的当前包证据 | OPEN |
| 真实认证、在线会员/权限/仓库/邀请/报价/结算/重连 | OPEN；服务端权威，不使用 QA/admin 或真实存档 |
| 完整手机 UI、多指、IME、GPU、音频、资源和更新 | OPEN/既有 FAIL；UI 设计修补仍需人明确重新批准 |
| Android 真机与最终人工验收 | OPEN |
| 完整 AP-01–21 / Windows-alignment goal | Active，不标完成 |

下一叶先复用/抽取既有自己的报价投影与共享出站规则，再做当前包实际 JNI。
原冻结矩阵未缩减；也不把“只接三域入站”变成本轮完整目标。

## 原始哈希清单

截至建档：61 个 regular files / 1075829635 bytes。
[raw index](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/social-ingress-20261003-P7uJBG/raw-payload-index.json) SHA-256
`fd509c1b9befe65cb1854ce553d80c6e3c38ece7ee490f05898021cc7bf69417`；
清单自身及建档之后的交付报告排除在索引外，并显式声明，不称全目录最终快照。
