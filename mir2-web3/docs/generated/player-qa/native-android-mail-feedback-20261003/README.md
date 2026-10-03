# NI-14：自身邮件结果关联与共享回执保护

源码：`4aee63e635ee64befcd9af75c96f769bc13eb61e`；分支：`codex/android-shared-sync`。
完整 Windows 对齐 goal 仍 Active；NI-14 仍 PARTIAL。
冻结 Windows 分母 `3d735745f1117d42a7859e87604a106351dca935` 不缩减。
本轮只读核验真正 Windows 来源 `56ee063fb4f9d6b581ec548659066ef9c5e05e9b`，
仅新增1提交/83份文档与证据，0功能源；旧 Windows 分支 `6ae080711` 是祖先。
[56输入哈希、七门、APK/安装/原图及135份本机原始证据索引](source-evidence.json)。

## DEBUG REPORT — DONE_WITH_CONCERNS

Symptom：原 Android 宿主没有把自身 MailSent/ParcelCollected 关联到实际写出的操作。
即使加入结果，普通 MailModel 队列也会覆盖回执；只保护队列后，消费者仍可能在
同一帧读取后续快照，令 UI 看不到反馈。这是源码回归，不是线上邮件结算验收。

Root cause：结果没有连接/角色/意图边界，且原队列把带操作反馈的邮箱当普通可合并
快照；单帧 consumer drain 又不保留观察反馈的时机。

Fix：只在实际认证 socket.send 返回接受后记录一个自身 send/claim。Java 宿主替换
快照中的内部连接代次，过滤服务端伪造私有回执、跨角色/类/连接结果，并保留原 claim
ID。Rust 私有 ingress 严格核验这些边界，暂存结果，等待下一份权威邮箱后才附上
冻结 Windows 的 typed feedback。runtime 将它保留为有额度限制的 critical/ACK，
当帧交付后把后续 MailModel 留到下一帧；不延迟其他域。SceneReset 保留个人操作，
DataReset 清空。ACK 本身不增加金币、物品或 claimed，不复制邮费、资格与结算规则。

Evidence：四组准备完成后的产品红保留：
native queue0/1、Android ingress0/4、JVM TLS2/2失败，以及第二层消费者 drain0/1；
对应断言修复后通过。最终相邻 Android44/44、runtime16/16、Windows邮件5/5。
共享宿主 headless 证明真实 ClaimMail producer 保留 pending，ACK 单独不改变它；
权威邮箱到来后反馈可见、原共享 pending 清除，金币77和claimed=false保持服务端值，
下一帧才消费后续空邮箱。这里没有实际 Android JNI、TLS账号或 GPU 帧证明。

Regression：五个功能源文件；没有第六文件、服务器、Windows、共享 UI 布局/规则修改。
12受保护整文件相对父提交 `b9548d878e80e8a19eea78adf92cfb7754c052bf` 字节一致；
8认证方法字节一致，5原runtime方法和30个旧runtime测试规范化一致（仅Rustfmt风格）。
Java 两个旧发送测试补 ACK+权威刷新间隔，原命令字段/线缆断言保留；不能宣称整个
旧 Java 测试文件未变。附件 helper/七命令序列化字段与冻结 Windows 仍一致。

Related：org.json 可能将超 signed-long 的 JSON 邮件 ID 转为 Double。当前收取写口
拒绝小数/已舍入 ID，不以错误身份关联；这不是完整 Android u64 等价，仍 OPEN。
失败的 UI v36 已在历史叶回退，本轮不恢复；UI设计修复待人明确确认。

Status：本叶源码接线 DONE_WITH_CONCERNS；整套 Windows 对齐未完成。没有将本地
TLS/headless 或离线预览视作真实账号、在线邮件、Zone/存档或真机验收。

## 精确源码的新鲜完整回归

56个选定构建输入在七门前后哈希一致；Rust1.95/锁定离线依赖/NDK26/JDK17/API31。
Java 两个变体强制重跑且禁用构建缓存；旧 ignored 不删除、不计为通过。

| 门禁 | 结果 |
| --- | --- |
| Android 普通 Rust | 394通过，0失败 |
| Android ui-preview Rust | 422通过，0失败 |
| shared native-player-ui | 1292通过，10个原 ignored |
| runtime | 296通过，1个原 ignored |
| Java 普通 / ui-preview | 各65/65，六类，0失败/错误/跳过 |
| API31 arm64 普通 / ui-preview | 两项 check 通过；另有下文实际 APK 构建 |

准备错误没有抹掉：新宿主测试误写 CollectParcel 而非 ClaimMail，E0599准备失败；
Windows bin-only package 首次误用--lib，命令准备失败；扩展 Java 11测试中10通过、
1项 MockWebServer 关闭握手 teardown 失败，原业务断言保留，补夹具回显关闭后通过。
检查命令的错误文件名/过宽目录输出、非提权部分克隆读失败和一次 SSH 只读关闭，
仅在任务工具记录中；不伪称这些控制台已有原始目录载荷。随后同仓库 HTTPS 只读
核验成功，不修改 Git/网络配置。上述不算三次失败产品修补或验收通过。

## 实际 APK、资源和保留数据安装

两份包都来自本叶干净源码；版本Code仍35，版本名仍 `0.1.32-gameshop-phone`。
未为本叶另改版本文件，因此以源码与 SHA 标识，不叫v37；构建网关为空，原生
普通包明确显示测试服未配置。包内arm64原生库逐包等于该变体刚构建的输出；
Gradle日志“Unable to strip”保留，不把该输出声称为成功裁剪的精简 ELF。

- 普通原生 APK：`mir2-native-mail-feedback-debug-4aee63e6.apk`，387733009 bytes。
  SHA-256：`f251805fb8deefd4219f6908fea93635e75cddc1e7fd5fb7c2e5e1fb579ae9d5`。
- 独立离线预览 APK：`mir2-native-mail-feedback-preview-4aee63e6.apk`，391751381 bytes。
  SHA-256：`55146a511ed1b33d8a747ef874a9ef050adfdb2aaee6b27005fc9373e18e3360`。

每包选定6647个UI PNG和3份metadata逐字节等于本机已许可资源；
UI_32bit470..473也等于冻结Windows原图。entity pack是已有本机
`android-archer-mount-bow-proof-20260912` override，manifest
`929d146535bde9b61c75c33af0e98852dcc3aef83320869aac9f06f4bdd530d9`。
此本地证明包不等于完整获准共享发布，也没有核验线上 Web/资源/Gateway 版本。

API31 arm64模拟器 `emulator-5554`，物理1080×2340、density440，实际横屏2340×1080。
以 `-r -d` 保留数据重装两个调试包，从旧失败v36回到回退源码的版本35；
未卸载/清数据/wipe，firstInstallTime保持，实际安装APK SHA逐包一致。
这不等同递归检查模拟器所有数据内容，也不代表旧v36变成通过。

## 五份本版原图与真实模拟器观察

五张未经编辑原图均已查看；对应安装SHA/PID/启动/后台日志在本机索引中。

| 进程场景 | 观察 | GL506冷日志 |
| --- | --- | --- |
| 离线邮箱 | 邮箱出现；切后台恢复同PID，画面仍为邮箱 | 7 |
| 离线发信 | 软键盘出现；发信按钮仍桌面偏小、邮箱顶部被裁切，手机布局未通过 | 24 |
| 既有商城JNI诊断 | Java原诊断进入实际JNI并得到共享catalog/storage消费者回执，原图显示777/33、1/14 | 29 |
| 普通包传mail预览参数 | 不进入预览，显示共享登录/测试服未配置；未伪造角色 | 12 |

四个独立PID冷日志共72 GL506/72 rbo，fatal/panic0；邮箱恢复日志仍7，不重复累加
同一进程的累计快照。GPU零错误门仍 FAIL，fatal0不等于稳定性或渲染成功。
商城实际JNI是原域的回归，不是邮件 MailSent/ParcelCollected 实际JNI验证；
邮箱两场景仍是原离线共享模型样本，不是本轮Java服务端结果的真实发送/收取。

## 剩余门与下一叶

NI-14仍PARTIAL：邮件结果实际Java→JNI、完整Android unsigned JSON、获准账号的
真实登录/邮件收发及物品金币结算、断网重连/退出保存、手机邮箱触控和IME/多语言、
真机验收未完成。完整手机UI、NI-15–20/社交/Hero、资源/渲染/音频/更新和最终
人工验收仍 OPEN；不按测试数量估算百分比、不部署生产、不改真实存档、不绕过认证。

下一叶优先补邮件实际JNI离线观察与精确无损ID边界，再推进完整NI队列和获准在线门；
UI设计修复仍需人明确确认，自动goal续跑不构成确认。原两个工作区HEAD/分支/status
与上叶、回归前后相同，未递归哈希未跟踪内容。独立PR253保持OPEN/Draft，
不推回Windows、不合并PR；发布后的远端提交另行核验。

本机原始目录与APK位置：
[mail-feedback-20261003-wIBsMt](</Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/mail-feedback-20261003-wIBsMt>)。
APK在其中 `final-apks/`，五张原图在 `emulator/`。
原日志/XML/原图/缓存/APK/密码/签名密钥不提交Git；本页与轻量JSON保留可核验索引。
