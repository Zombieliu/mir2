# NI-14：共享邮件操作到 Android 宿主（源码阶段）

源码：`49693ab1a66ca1ba5573895ce825ea091d5864f4`；独立分支：`codex/android-shared-sync`。
完整 Windows 对齐目标仍 **Active**，NI-14 仍 **PARTIAL**。
冻结 Windows 分母：`3d735745f1117d42a7859e87604a106351dca935`；
本轮只读远端核验 Windows：`6ae080711fc7b3aaf06dd9c6bcf63f121682dbe4`。
[逐文件哈希、门禁、失败分类与原始证据索引](source-evidence.json)。

## 本阶段做到什么

现有 Android 只消费 shell/quest 意图，没有正式消费者取共享
`NativePlayerUiIntentQueue` 中的邮件操作。本阶段接通七种公开命令：
readMail / collectParcel / deleteMail / lockMail / mailCost / mailLockedItem / sendMail。
保留接收人、正文、完整附件 UID、原顺序/五格填充、金币和盖章选项。
共享附件 helper 函数体与冻结 Windows 一致，Bag1/Bag2 容量仍来自既有
`InventoryModel::bag_slot_capacity`，不推断扩容、不用槽号代替 UID、不静默裁掉附件。

仅增加共享纯传输转换及邮件专用有界取队列接口；不消耗其他 UI 意图。
Android 复用现有认证队列、租约、C copy/callback、连接 generation 与 DataReset；
每帧最多16条，48-KiB 命令限制为64-KiB JNI 包装预留空间。
未登录/无已核验角色/未 render-ready/后台/断网/失焦不发送。
同角色切场暂存有界意图，DataReset 用原共享消费者退役；终止登录/断线边界不重放。

当前背包失效、队列满或字节超限时终止此批次，进入既有断线/清理边界，
不留下不可关联的邮费占位，不计算本地邮费或假装操作成功。
原 pending registry 仍禁止无请求 ID 的 claim/send 并发；“socket Sent”不是服务端 ACK。

## 证据与失败分类

- 准备编译错误：新增测试误用了不存在的 lease.json() 和 LogOut 拼写；原日志保留。
- 产品基线：旧宿主加四项真实队列测试，编译后0通过/4失败；注册正式消费者后
  同四项断言4/4通过。没有删除或降低断言。
- 扩展测试8通过/2失败：测试宿主漏了正式共享 UI 清理消费者，并错误移除了
  runtime reset 必需的 InventoryModel。补原消费者并把缺资源负例隔离成 producer
  seam 后10/10通过；不是 UI、玩法或渲染修复。
- 共享转换/附件/有界FIFO/pending五项通过；新Java本地TLS两项通过，
  验证七种命令的阶段门及盖章/五附件/正文/大于2^53的有符号Long保持。
- 审计生成器两项准备错误（正则转义、重复变量声明）单独分类；其初次完整控制台
  留在任务记录，未整段复制到原始目录。不将准备错误称为产品红→绿。
- ADB只读设备探测首次被沙箱阻止，获范围权限后复验：一台模拟器，零台真机；
  没有安装、启动、清数据或修改模拟器尺寸。

## 精确提交门禁

56个真实构建输入，七项门禁均核对开始/结束哈希一致，源码当时已提交且工作树干净。
使用已锁定离线依赖、Rust1.95、既有SDK/NDK与JDK17；旧测试和原ignored保留。

| 门禁 | 实际结果 |
| --- | --- |
| Android普通 Rust | 388通过，0失败 |
| Android预览 Rust | 416通过，0失败 |
| shared native-player-ui | 1292通过，10个原有ignored |
| runtime | 292通过，1个原有ignored |
| Java普通/预览，全任务强制重跑 | 各59/59，六类，0失败/错误/跳过 |
| API31 arm64 普通检查 | 通过，仅check，不是APK |
| API31 arm64 预览检查 | 通过，仅check，不是APK |

十二个受保护文件（邮件规则/消费者、Windows宿主、认证、Activity、runtime）
与父提交 `f0b3e8c21d946ce8aeb2e0173461ddd56a9c3fdb` 字节一致；
冻结附件函数体及七种出站序列化字段也一致。
本阶段七个源文件仅新增行，未删除旧行；没有改变服务端或共享结算规则。
原两工作区的 HEAD/分支/status 与上一阶段记录一致；未递归哈希未跟踪内容。

## 明确保留的未完成项

1. 邮件结果关联尚缺：MailSent/ParcelCollected 必须绑定本连接及原意图，
   再交给原共享反馈/清理规则。匿名包仍不能虚构成功。
   原共享 ReceiveMail 刷新可证明指定邮件已收取，但不替代完整发信/失败反馈。
2. 本阶段没有新APK、APK SHA、安装、实际Java→JNI、模拟器画面或手机邮件操作。
   C ABI headless 与 JVM TLS 不算真正 Android JNI，也不重绑旧v36安装包。
   Rust完整u64与JVM Long不是Android org.json 对大于Long.MAX_VALUE的实测证明。
3. 没有真实Gateway/账户认证、在线发信/收取、金币物品结算、Zone或断线保存验收。
4. v36布局保持回退；UI修复因design-review风险门等待用户明确确认，本次自动工作
   不代表批准。旧渲染零错误门仍FAIL，未改渲染、不宣称改善趋势。
5. 真机、完整NI-15–20、剩余UI/手机输入/九语言/资源/音频/更新与人工接受均未完成。
   不能用测试数量、提交距离或离线截图算“完整度百分比”。

下一阶段先补本连接邮件结果关联与精确APK/JNI证据，继续完整NI队列；
获UI确认后再修仓库/背包手机呈现。不得部署生产、改真实存档或绕过认证。

原始本机日志/报告/XML/哈希目录：
[mail-egress-20261003-JaQ0L1](</Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/mail-egress-20261003-JaQ0L1>)。
原始文件与缓存不提交Git；轻量清单可以随源码复核。
