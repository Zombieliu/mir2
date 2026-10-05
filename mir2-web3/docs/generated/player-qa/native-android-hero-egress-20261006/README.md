# Android NI-16 Hero 出站 — 2026-10-06

预提交源码验证快照；完整 Windows 对齐 goal 保持 Active、AP-01–21 不缩减、NI-16 为 PARTIAL。
本页记录本批源码、编译和宿主回归。APK、实际Android JNI/UI、真实网络与真机分别验收，不能由本页测试代替。

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
