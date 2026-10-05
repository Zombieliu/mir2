# Android NI-16 Hero Java/JNI 诊断（2026-10-06）

这是完整 Windows 对齐 goal 的一个有界验证叶，不是英雄玩法、在线闭环或手机 UI 完成。
NI-16 仍 **PARTIAL**，goal **Active**，AP-01–21 的完整分母保持不变。
冻结 Windows 功能基线 `3d735745f1117d42a7859e87604a106351dca935`。

## 本次预提交快照

基点 `275b6fc5b768a1f20696cdba9f4daec49c9f5806`，此段记录提交前对应字节；后续提交/包证据须独立绑定，不重写旧失败。
428 个源码输入哈希清单 SHA-256 `bf1907cb739e4a163c89e066938a9bc4d0889262a38c929a937168afe26e99db`。
原始本地证据：`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/hero-jni-20261006-rz55ei`。
54 份/5,628,209 字节索引 SHA-256 `e86c918fe613ec2bc317159d7e0ef6b76258b28a5fe8182320fc42ec043bf024`；
索引不包括自身和其后产生的提交、构建、运行与文档证据。

四个 Android 文件：Java 离线输入、其测试、Rust 预览接线/观察器、独立 Rust 测试。
424 个其他输入整文件不变；旧 Java 输入函数和测试精确保留；
旧预览函数只做申明的新增接线以及场景计数 55→61，原 55 场景顺序保留。
原两 worktree 的 Git HEAD/分支/status checkpoint 保留，不声称递归未跟踪字节证明。

## 做了什么

新增六个 **仅独立 uiPreview 包** 的 Java → MainActivity/nativeEvent 诊断场景：

- `hero-inventory-jni`、`hero-equipment-jni`
- `hero-status-jni`、`hero-state-jni`
- `hero-skills-jni`、`hero-removed-jni`

先送完整 HeroInformation，再绑定原 owner snapshot，之后送真实格式增量：
召出状态、HP/MP、自动药设置和技能等级；额外错误 owner=43 的血量不得覆盖 owner=42。
42 英雄背包槽、14 装备槽、两个技能、上限 u64 实例/201 数量/嵌套 socket；
玩家金币777、credit33、HP80/MP20和12件背包必须保持独立。

Rust 诊断不填充 HeroModel、不复制规则、不造 pending/ACK。
等正常入站和原共享 observer 已产生一致模型后，只调用原窗口 toggle 一次；
再等原 renderer 创建窗口节点，记录所收到的模型与窗口状态。
窗口节点/单测不等于 GPU 画面；关闭后不会被诊断反复打开。
普通 APK 不生成输入；认证、网络 pump、Windows、服务器、共享英雄/物品/技能规则不变。

## 源码门禁与失败保留

五项门均使用上述相同428输入；不是再次宣称全部 Windows OS 回归。

| 本批检查 | 结果 |
| --- | --- |
| 普通 Android host Rust | 478/478 |
| ui-preview Android host Rust | 525/525 |
| 强制重跑 Java Debug / UiPreview | 各93/93，各六类；0失败/错误/skipped |
| API31 arm64 普通 check | PASS，退出0 |
| API31 arm64 preview check | PASS，退出0 |

聚焦 Rust 6/6 包含于全量525，不另累加。
Java 初版新增3项失败（仅新鲜 Debug OfflinePersonalIngressPreviewTest 15项/3失败）→修复后两变体聚焦15/15。
第一次失败没有执行到 UiPreview；当时复制的其他 XML 是旧文件，明确排除，不伪装为本次结果。
Rust 注册红已编译0通过/2失败→6/6绿；旧断言未删改。

本批没有改共享源码；上一批 b5 的共享/runtime/Windows-host子集测试是历史证据，不重绑为本批新跑。

## 旧安装包实测与采集错误

旧 uiPreview 包确认为源码 `b5b6f4c6893cc62f6f9a9c0a1bf58887cf257605`；
安装 SHA-256 `bd3176f16d9e5d617c068d59662678e09a466b6912c1118bda2f263dd7167ffa`。
专用 API31 ARM64 emulator-5554，PID9281；
请求新增英雄场景未生成 Java 英雄输入或共享英雄观察记录，原图已看，实际停在未配置 Gateway 的登录页。
这不是崩溃；本次记录fatal0、短样本GL506=0，不关闭旧 GPU FAIL。

两个采集脚本错误分开保留：
误以为未知场景必回退 HUD；实际没有该合同，按原日志纠正。
PNG 超过 Node 默认缓冲，采集报ENOBUFS；改采集上限后完整保存原PNG，未改应用或图片。
不把采集修补计为产品功能修复。

## 尚未通过

此预提交版本尚无本源新APK/安装/实际英雄JNI/新窗口截图。
接下来绑定精确源码，保留数据安装后验证六种实际 Java/JNI 与原共享窗口。
离线诊断不运行认证 socket，不证明 UI 意图经过真实 Java发送或服务器 ACK、技能分配/物品操作/AI/战斗结算。
真实登录、共享Zone/保存、nativeResume、完整手机窗口/触屏/IME、多语言、
完整资源/音频/更新、旧零错误GPU gate、真机与人工接受继续 OPEN。
不部署生产、不改真实存档、不绕过认证、不推Windows、不合并PR。
