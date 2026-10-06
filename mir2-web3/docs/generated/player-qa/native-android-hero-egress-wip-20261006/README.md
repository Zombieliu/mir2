# Android NI-16 Hero 出站 WIP — 2026-10-06

本页为第六文件编译修补之前的历史WIP快照；当时的未提交/待确认/E0599状态和输入摘要保留。
当前六文件源码验证见[后续证据](../native-android-hero-egress-20261006/README.md)，不覆盖或重绑本页旧结果。

本页只记录未提交工作副本及宿主证据，不是新 Android 发布或完整度验收。
goal 仍 Active，NI-16 为 PARTIAL，完整 AP-01–21 分母及各验收层不变。
此前已提交的 Hero 入站、APK 和截图见[上一批证据](../native-android-hero-ingress-20261006/README.md)，不得重绑到本 WIP。

## 来源与写集

来源 HEAD：`0ebd5c66a705cd4691b92287cb10b5d7ade9126f`，分支 `codex/android-shared-sync`。
本目录的新README/source-evidence受既有generated忽略规则影响，仅留本机且未暂存；
六份状态文档也未提交，未修改忽略规则。编译门通过后再按交付流程加入版本记录。

这五个功能文件均为本地 WIP，未提交、未推送，没有可称为本批产品来源的新提交 SHA：

- `apps/game-client/platform-android/src/hero_egress.rs`（新增）
- `apps/game-client/platform-android/src/lib.rs`
- `apps/game-client/platform-android/src/gateway_bridge.rs`
- `apps/game-client/platform-android/src/shared_shell.rs`
- `apps/game-client/client-bevy/src/crystal_ui/overlays.rs`（加法式别名与按域提取意图；不改窗口布局）

输入清单427项的最终摘要：`6f17e699cde7c5e1c87fe7e3f0812ab4622d922090bfa0bd1733a0f70a21c443`。
最终聚焦、八门和两项实际 Android 检查前后均稳定；报告中的 `source=0eb…` 是基点，
`bound=false` 明确表示不是干净提交绑定。红/首轮绿有各自不同输入摘要，不重绑。

2026-10-05T17:55:35.007Z 只读远端核验：Android/PR #253 HEAD仍为上述0eb，
PR为open Draft，base仍 `codex/playtest-registration`。
Windows来源 `56ee063fb4f9d6b581ec548659066ef9c5e05e9b` 较冻结
`3d735745f1117d42a7859e87604a106351dca935` 仅+1提交/83文档证据路径/0功能源变化；
不是降低功能分母。未写Windows、Java、认证、服务器、资源包或真实存档。

## 根因、修补与范围限制

英雄出站复用原共享模型/操作草稿，12种闭合协议与冻结Windows完全相同；
捕获精确 owner、epoch、Hero generation/actor、技能 requestId 和物品结果代次。
仅恢复确定未发送的对应草稿，入队和 socket Sent 都不是服务端 ACK；
不复制英雄AI、战斗、交易、技能、物品或存档规则。

复现的新缺口是：请求已进入 Rust C ABI 出站队列、但尚未被 Java 复制时，
原检查只覆盖待发送 Rust 队列。旧英雄/账号请求可能跨身份屏障；
暂停时仍能被取走；宿主队列满256项时提前返回还会跳过清理。
本次在相同五文件写集补齐未轮询队列隔离：

- 尚未交给Java：准确旧序列退役，adapter lease容量释放；暂停项保留原序列与FIFO。
- 其他域可继续传输，短缓冲查询不消费延期Hero；缺共享模型/消费者时延期而非误发。
- 已复制/可能已写出的请求不伪造撤回或Failed；晚到实际结果只退役原lease，
  不污染新身份/新等值草稿，不确认任何权威模型或重放未知结果命令。
- 源码guard保留422个整文件和858个原函数/测试体；只允许明确列出的原宿主函数/注册点
  中的必要接线变化，原共享规则、Windows命令oracle和12种wire schema受保护。
  两原工作区的HEAD/分支/Git状态检查点一致；这不是未跟踪目录递归字节证明。

尚存独立编译阻塞：`AndroidHeroIngress::model()` 现有只读getter受 `#[cfg(test)]` 限制。
宿主测试可见，Android生产目标不可见。普通/preview已分别实际运行并报E0599。
按 investigate 的超过五文件写集限制，仅移除该getter的测试条件会成为第六代码文件
`platform-android/src/hero_ingress.rs`，仍待人明确批准；未进行模块搬迁或绕开检查。

## 实际验证

下面八项为八条宿主回归命令，Java同一命令含两variant；不是八项Android实机验收。
聚焦测试包含在全量测试中，普通/preview也重叠，不相加生成虚假的独立覆盖率。

| 门槛 | 结果 | 边界 |
| --- | --- | --- |
| 初始未轮询FFI复现（已编译） | 1通过 / 3失败 | 保留失败日志，非通过 |
| 对应首轮修补 | 4通过 / 0失败 | 同组测试红转绿 |
| 最终Hero出站+FFI聚焦 | 31通过 | Mac Rust/C ABI，非Android JNI |
| Android crate普通宿主lib | 478通过 | 宿主测试，不等于Android目标编译 |
| Android crate preview宿主lib | 519通过 | 宿主测试 |
| 共享 native-player-ui lib | 1304通过 / 10既有ignored | ignored不算通过 |
| runtime lib | 296通过 / 1既有ignored | ignored不算通过 |
| 强制重跑Java普通/preview | 90 + 90通过 | 两套各6类，0fail/error/skip，无build cache |
| Mac-host Windows Hero FIFO | 1通过 | 非完整Windows OS gate |
| Mac-host Windows技能 FIFO | 1通过 | 非完整Windows OS gate |
| Mac-host Windows协议 | 21通过 | 非完整Windows OS gate |
| API31 arm64普通目标check | FAIL，退出101/E0599 | 测试专用getter不可见 |
| API31 arm64 preview目标check | FAIL，退出101/E0599 | 同一阻塞，非未运行 |

七条新增FFI测试还覆盖：owner/Hero/退出、focus/render/pause、
256满队列、已轮询晚结果、新等值技能草稿、共享模型/消费者缺失、
短缓冲与其他域FIFO。每项均用原共享状态，未伪造在线ACK。

## 证据与验收缺项

原始本机证据：
`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/hero-egress-20261006-ktzy2n/ffi-fences-1P0J8H`。

`raw-evidence-index.json` 的SHA-256：
`06f74215f88d0fbed6212bf09dfa5384bcd856a7783e28944e8be5949f2a2548`。
41文件/3,354,329字节，包括红/绿/最终聚焦、八门、两项FAIL、来源guard及只读远端核验。
索引不含自身、后续collector/文档/最终guard输出；本叶无APK。
[精确命令、计数、来源与验收状态](source-evidence.json)保留紧凑可追溯摘要；
完整427项输入及原始stdout/stderr只在忽略的本机证据目录。

当前设备清单仅1个API31 emulator（`emulator-5554`）、0真机；
本 WIP 没有打包、安装、实际Android JNI、截图、GPU重测、真实登录或在线玩家闭环。
上批0a安装包仍是历史来源；历史GL0x0506/零错误GPU门FAIL、交易UI锁定差异未关闭。
手机全屏布局三轮失败后的重启批准、NI17–20、完整资源/音频/更新、真实Zone移动保存、
真机触控/软键盘/后台/断网与人工接受均保持OPEN或EXTERNAL，不记完成。

下一步需要明确批准第六代码文件的getter可见性修补；批准后保持getter函数体不变，
重新跑两项Android目标检查和全部相关回归，通过后才做精确源码提交/双APK/哈希和安装。
实际Hero Android JNI/UI验收另立证据，不复用这批Mac C ABI结果。
本WIP未提交/未推送；不合并PR、不自动改base、不部署生产、不搜凭据、不重放未知结果。
