# NI-14：Android 无损 u64 JSON 宿主边界

源码 `f895302be399529df3163cbf3c44499772acd8c2`，分支 `codex/android-shared-sync`。
完整 Windows 对齐 goal 仍 Active，NI-14 仍 PARTIAL。
冻结功能分母 `3d735745f1117d42a7859e87604a106351dca935` 不缩减。
本轮重新核验真正 Windows 来源 `56ee063fb4f9d6b581ec548659066ef9c5e05e9b`：
其后只有1提交/83份文档证据、0功能代码差异；旧 Windows 分支是冻结分母的祖先。
[57输入、七门、两包 SHA、设备探针及148份本机原始载荷索引](source-evidence.json)。

## DEBUG REPORT — DONE_WITH_CONCERNS

Symptom：Android 默认解析器把邮件 ID `18446744073709551615` 转为
`Double 1.8446744073709552E19`；原宿主为避免错认 ID 又拒绝合法的上限 u64。
本机 JVM 测试的 org.json 能保留 BigInteger，不能替代手机平台证明。

Root cause：Gateway 网络入站、权威快照克隆和 MainActivity 原生出站使用默认
JSONObject 解析，Android 发生 Long→Double 回退后已无法恢复原始整数。
[AOSP JSONTokener 源码](https://android.googlesource.com/platform/prebuilts/fullsdk/sources/%2B/refs/heads/androidx-constraintlayout-release/android-35/org/json/JSONTokener.java)
用于定位；API31 实际平台探针的产品红是本轮直接证据，未把其他 API 版本当作设备实测。

Fix：[WireJson.java:21](https://github.com/Zombieliu/mir2/blob/f895302be399529df3163cbf3c44499772acd8c2/mir2-web3/apps/game-client/platform-android/android/app/src/main/java/com/mir2/web3/WireJson.java#L21)
复用标准 JSONTokener 的对象、数组和字符串读取，仅对规范数值 literal 保留精度。
有符号 Long 范围保留 Integer/Long，上限 u64 用有界 BigInteger，序列化仍为 JSON 数字，
不是改协议为字符串。限制1MiB UTF-8/64层，整数超范围、非法 literal、尾随内容拒绝。
这是规范十进制数值保真适配，不是重写完整 JSON 语法；其余标准 Tokener 的宽松
读取语法未宣称为严格 RFC JSON 或全面安全等价。三个宿主通路使用此解码器；
collectParcel 接受精确的正整数 u64，仍拒绝字符串、
浮点、零、负数和溢出。原角色/连接代次/实际 socket 写入成功才记录操作的限制保留。

Evidence：旧实现完成编译后的 TLS 产品红1测试/1失败；修复后保留该断言，并新增
嵌套/克隆、完整原生 JSON 出站→实际本地 TLS 写入→自有 ACK、网络入站、整数边界、
无效身份和资源上限回归。实际 API31 候选 DEX40项通过，再分别对两个已安装、
SHA 核验一致的 APK 反射包内 WireJson：各40项通过。探针 DEX 只有自身一类，
不携带产品解析器。这里证明的是安装版宿主数值边界，不是实际 Activity pump、
邮件 JNI、真实 Gateway 账号或在线收发/结算。

Regression test：
[旧实现拒绝完整 u64 的保留断言:504](https://github.com/Zombieliu/mir2/blob/f895302be399529df3163cbf3c44499772acd8c2/mir2-web3/apps/game-client/platform-android/android/app/src/test/java/com/mir2/web3/GatewaySessionTest.java#L504)，
[原生出站 JSON 与自有 ACK:531](https://github.com/Zombieliu/mir2/blob/f895302be399529df3163cbf3c44499772acd8c2/mir2-web3/apps/game-client/platform-android/android/app/src/test/java/com/mir2/web3/GatewaySessionTest.java#L531)。
四个 Android 源/测试文件；53个选定旧输入、11个受保护整文件、7个认证方法字节不变；
connect 只有解码器替换，MainActivity 只有两处解码器替换。旧 Java 断言未删除或更改。
runtime 整文件未变，34个已有测试规范化一致；冻结邮件附件 helper/七命令字段仍一致。
没有修改共享规则、布局、Windows、服务端、签名或版本文件。

Related：不再将 Android 上限 u64 解析笼统记为缺失，但实际邮件 Java→JNI、获准账号、
线上资源/版本、结算与真机仍 OPEN。JNI 序列的 jlong ABI 边界是另一问题，
本轮没有扩展或声称所有64位宿主字段都获得无限范围。失败 v36 已回退；
本轮不恢复失败 UI，设计修补仍待人明确确认。

Status：本叶数值修复 DONE_WITH_CONCERNS；完整 goal 与 NI-14 未完成。

## 精确干净源码的七门与 APK

57个选定输入在各门前后哈希一致。Rust1.95/锁定离线依赖/NDK26/JDK17/API31；
Java 强制重跑且禁用构建缓存，六类/每变体。既有 ignored 不删除或计为通过。

| 门 | 结果 |
| --- | --- |
| Android 普通 / ui-preview Rust | 394 / 422通过，0失败 |
| shared native-player-ui | 1292通过，10个原 ignored |
| runtime | 296通过，1个原 ignored |
| Java 普通 / ui-preview | 各74通过，0失败/错误/跳过 |
| API31 arm64 普通 / ui-preview | 两项 check 与两份实际 APK 构建通过 |
| 安装包 WireJson 设备探针 | 两包各40项通过；非 JNI/在线流程验收 |

版本Code仍35、版本名仍 `0.1.32-gameshop-phone`，以源码/SHA识别，不称新版本号。
普通包网关为空并显示测试服未配置；preview 独立包只用于离线诊断。
Rust 未改，包内 ELF 分别与该变体构建输出一致；不称这些大 ELF 已成功精简。

| APK | bytes | SHA-256 |
| --- | --- | --- |
| `mir2-native-mail-wire-debug-f895302b.apk` | 387797739 | `5b6ea829b045833ec902369cd63e08a10e54954fb4668b671972fea3e9443344` |
| `mir2-native-mail-wire-preview-f895302b.apk` | 391816211 | `61a236e9f8dfa5562f83bbcd2cec8e8d0732c7d9e5195adba6b4db8076caccb0` |

每包6647个选定UI PNG与3份metadata逐字节匹配已许可本机资源，
UI_32bit470..473仍匹配冻结 Windows 原图。entity override仍是
`android-archer-mount-bow-proof-20260912`，manifest
`929d146535bde9b61c75c33af0e98852dcc3aef83320869aac9f06f4bdd530d9`。
这不是完整获准共享发布或已验证线上 Web/Gateway 环境。

API31 arm64模拟器 `emulator-5554`，物理1080×2340、density440，
实际横屏原图2340×1080；本轮只发现模拟器，0真机。
两个包用 `-r` 保留数据重装，版本35→35，firstInstallTime不变，
安装 SHA 与本叶包相同。未卸载/清数据/wipe。

## 本版离线观察与原失败保留

五张未经编辑原图均已查看，四个冷进程 PID 独立。
邮箱恢复同PID、画面仍在邮箱；不把同进程累计恢复日志再次计入冷日志。

| 场景 / PID | 观察 | 冷日志 GL506 / rbo |
| --- | --- | --- |
| 邮箱 / 29059 | 离线模型可见、同PID恢复；手机小控件未完成 | 0 / 0 |
| 发信+键盘 / 29158 | IME可见；小按钮、邮箱顶部裁切，手机布局 FAIL | 0 / 0 |
| 既有商城JNI / 29233 | 实际 Java→JNI→共享消费者日志，画面777/33、1/14 | 23 / 23 |
| 普通包带mail参数 / 29306 | 未进入预览，仍是测试服未配置的登录页 | 0 / 0 |

四进程观察总23 GL506/23 rbo，fatal/panic0。零错误门仍 FAIL。
其他三个短窗口未出现错误不代表修复，不能将上一版72→本版23解释为渲染优化：
本轮没有渲染源码改变，窗口/进程/驱动状态不同，未做性能或稳定性归因。
既有商城 JNI 是另一个域的回归；邮件两场景仍为原合成共享模型，不是新邮件回执 JNI。

原准备错误分别保留说明：D8继承Java8导致版本错误，设置JDK17后同探针通过；
一次GitHub GraphQL连接中断，后用同仓库REST只读核验成功，未改网络/模型/Git配置。
原stderr在任务工具输出中，不冒称另有保存的原始日志载荷；不计为失败产品修补。

## 完整剩余目标与下一叶

继续补实际邮件 Java→JNI 的独立离线证据，再推进 NI-15–20 与完整 Windows 矩阵。
真实登录、StartGame/Zone移动/退出保存、在线邮件/金币物品结算、断网恢复、
完整手机 UI/多指/IME/多语言、资源/渲染/音频/更新、真机和人工验收仍 OPEN。
UI设计修复仍待人明确确认，自动 goal 续跑不构成确认；全目标不因此缩成邮件修复。

原两个工作区 HEAD/分支/status 的 Git checkpoint 与本轮前相同；
未递归哈希用户未跟踪内容。独立 PR253仍 Draft，不推回 Windows、不合并。
源码与证据分两次提交；发布后远端提交另行核验。

本机[原始证据及APK目录](</Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/mail-wire-20261003-4JjAIe>)。
APK在 `final-apks/`，五张原图在 `emulator/`；
原日志/XML/原图、探针二进制、缓存、APK和密钥均不提交Git。
