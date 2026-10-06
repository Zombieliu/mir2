# NI-14：邮件失败时保留跨域断线清理（源码叶）

源码：`7280d07532b9739a31a9caaadb250a096592bab6`；分支：`codex/android-shared-sync`。
完整 Windows 对齐 goal 仍 **Active**，邮件 NI-14 仍 **PARTIAL**。
冻结 Windows 分母 `3d735745f1117d42a7859e87604a106351dca935` 不缩减；
本轮只读远端 Windows `6ae080711fc7b3aaf06dd9c6bcf63f121682dbe4` 未变。
[逐输入哈希、七门、失败分类和38份原始载荷索引](source-evidence.json)。

## DEBUG REPORT — DONE_WITH_CONCERNS

Symptom：headless 正式邮件 producer 的队列失败会进入断线，但另一项仍等待响应的
改密码操作可能保留 pending。这里是可复现源码状态，不是用户手机实测故障。

Root cause：原失败路径先调用 `mark_terminal_reset` 清掉关联信息，稍后既有
`AndroidGatewayHostAdapter::on_connection_lost` 就无法判断改密码仍在途。

Fix：只在原邮件失败边界先调用既有 adapter，让它在记录仍存在时完成跨域 unknown
清理；资源缺省时保留原队列 reset fallback。没有增加另一套认证、存取、交易或
结算规则，也不把断线后的未知结果当作成功。旧共享 UI 和 Windows 不改。

Evidence：新增回归编译后0/1失败，断言定位上述 pending 残留。最小修复后同断言通过，
涵盖旧操作仍在队列及已 drain/Sent 两种状态；邮件出站相邻回归合计11/11。
基线在首个 queued 场景失败即终止，不能宣称它也执行了第二个场景。

Regression：只追加一个新测试，原文件完整前缀/旧断言字节保留；只改两个 Android
文件、78新增/1删除。既有 adapter 本身不改。十二个邮件规则/消费者/Windows/
认证/Activity/runtime 文件与父提交 `0ff18952d515446e566ea2d492001431cff47de5`
字节一致，冻结附件 helper 和七命令字段仍一致。回归前后 guard 一致。

Related：MailSent/ParcelCollected 本连接结果关联尚未实现。接线前要证明带操作反馈的
邮箱模型不会被同帧后续普通快照覆盖；当前普通 MailModel 的队列策略可合并。
这属于下一叶的待测风险，不宣称已有实际结果丢失或已修复。

Status：本叶断线修复 DONE_WITH_CONCERNS；整套对齐目标未完成。UI 修复仍等待
用户明确确认，自动续跑不构成批准；原 GPU 零错误门仍 FAIL。

## 精确干净提交的新鲜回归

56个构建输入、七门开始/结束 SHA-256 一致；使用锁定离线依赖/Rust1.95/SDK31/
既有 NDK/JDK17。Java 两项强制重跑、关闭构建缓存；旧 ignored 不删、不计通过。

| 门禁 | 结果 |
| --- | --- |
| Android 普通 Rust | 389通过，0失败 |
| Android ui-preview Rust | 417通过，0失败 |
| shared native-player-ui | 1292通过，10个原 ignored |
| runtime | 292通过，1个原 ignored |
| Java 普通 / ui-preview | 各59/59，六类，0失败/错误/跳过 |
| API31 arm64 普通 / ui-preview | 两项 check 通过，不是 APK 构建 |

准备错误与产品失败分开：最初测试误写 LogOut，编译准备失败日志保留；修为 Logout
后才产生产品0/1红。一次 runner shell 引号错误未启动子进程，格式检查还发现原有
风格差异；两次完整控制台仅保存在任务记录，不冒充原始目录已有文件。
最终仅格式化追加测试、撤回旧测试的机械格式变化，不改其他模块。

原两个工作区 HEAD/分支/status 与上一源码叶及本次回归前后一致；
未递归哈希未跟踪内容，不把 Git checkpoint 等同全部文件内容审计。
提交前只读 PR253 为 OPEN/Draft；发布后另行核验远端源码与交付提交。

## 本叶未验收与下一叶

没有新 APK、APK SHA、安装、启动、截图、实际 Java→JNI、模拟器邮件交互或真机
验收；既有 v36 安装包不重绑到当前源码。上叶的1模拟器/0手机探测是历史证据，
本叶未重测设备。C ABI headless/JVM TLS 不等于实际 Android JNI 或真实 Gateway。

真实账号登录/在线邮件收发结算/Zone/退出保存、Android 上限 u64 JSON、完整
手机 UI/九语言/渲染/资源/音频/更新、NI-15–20 与最终人工验收仍 OPEN。
不部署生产，不改真实存档，不绕过认证；不按测试数量估算完整度百分比。

下一叶：在真实认证 WebSocket 写成功后记录唯一自身 send/claim，围住连接代次与
原 claim ID；只把服务端结果交给下一份权威邮箱与既有共享反馈消费者，补背压和
同帧覆盖回归。再交付精确 APK/实际 JNI 和获准在线证据；不重写邮费/资格/物品规则。

本机原始目录：
[mail-results-20261003-OJustu](</Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/mail-results-20261003-OJustu>)。
原始日志、XML、缓存与 APK 不提交 Git；本页与轻量 JSON 可随源码复核。
