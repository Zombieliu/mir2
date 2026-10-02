# Android 商城翻页缓存修复 — 2026-10-03

本叶修复已用新 APK 的实际触控验证，**完整 Windows 对齐 goal 仍 Active，NI-12/13 仍 PARTIAL**。
源码 `76d51415b5ceed667898a720c698b3a062bf7790`；父证据提交 `2c2b748263db10f6350c26c0cad9acb68fa5f2a0`。
独立分支 `codex/android-shared-sync`，源码远端已核验，[PR #253](https://github.com/Zombieliu/mir2/pull/253) 保持 Draft。
完整验收分母保留 Windows `3d735745f1117d42a7859e87604a106351dca935`，不随远端回指 `6ae080711` 降级。

## 根因与最小改动

翻页控制器已经改变 `NativePlayerUiState.game_shop_page`，但独立的
`GameShopRenderKey` 未包含页码，因此 `fill_game_shop_panel` 复用旧子树、画面仍停旧页。
只增加页码参数、缓存字段与相等比较；不改目录、排序、库存、购买、认证或存档规则。
四个源码文件：共享 `overlays.rs`、`game_shop_dialog.rs` 的新测试注册、新独立
`game_shop_hit_tests.rs`，以及 Android 版本号。Windows 宿主/渲染/Java JNI/协议入口未改。
共享缓存代码确实改了，不能把“Windows 宿主未改”写成“所有共享文件未改”。

先排除文字阻挡假设：固定 Bevy 0.19 的 Node 自动要求 FocusPolicy，真实生成文字/节点
与公共 ui_focus_system 的四项测试在修复前就通过。没有应用 FocusPolicy 补丁。
保留测试准备失败：私有 ui_stack_system 调用编译错误；新样本未补零名称产生字典序预期错误。
两者不作产品红证据，也未修改旧测试或源排序。修正样本后，两项实际保留窗口断言
编译执行失败（1/14 vs 2/14、2/14 vs 1/14）；补缓存后七项通过。
测试使用实际共享窗口/缓存，命中测试只提供 authored box 几何，不冒充 GPU 或真机。

## 新鲜源码门

| 门 | 本轮结果 |
| --- | --- |
| Android 正式 / 预览 Rust | 359 / 387 通过 |
| 共享 native-player-ui | 1267 通过；保留原 10 ignored |
| Runtime | 292 通过；保留原 1 ignored |
| Java 两变体，强制 fresh 执行 | 每变体 52 通过、六类、零 skipped |
| 实际 API31 arm64 两变体检查 | 通过 |

七门都绑定同一干净提交和 49 个源码输入，前后哈希一致。
两包原生 AArch64 ELF 与各自 stripped 输出一致，选定 6647 PNG + 三 metadata 的原字节核验通过；
这不是完整资源包/线上版本验收。

## 实际原生模拟器

API31 `sdk_gphone64_arm64` / `emulator-5554`，arm64-v8a，屏幕 1080×2340、
横屏原图 2340×1080，density 440。本轮只连接模拟器，没有真机。
升级使用保留数据安装，安装后的 APK SHA 与构建产物一致；原 firstInstallTime 保留。
没有清空/卸载应用、AVD wipe、日志清空、认证绕过或真实存档修改；未作存档内容哈希。

21 张本轮原始有效 PNG 已逐张检查。实际 Java→JNI→宿主→共享模型提供
105 条目录/末库存3、160 条仓库/末159槽/扩展，模型不是手填。
商城 PID24356：用与 v33 失败相同的 (1837,992) 下一页坐标，连续真实点按显示 1–14/14；
末页仅一个商品且库存3；再点下一页仍14/14，(1713,992) 返回13/14，
切后台恢复仍13/14、同 PID，Java_START/SENT 各一次，不重放批次。
窗口源码回归还检查全部105商品 identity 可达；不能从同名预览图片独自辨认全部 identity。
原有 friendly-name 会剥离末尾 ASCII 数字：`JNI received 001` 显示为 `JNI received`；
后缀未出现本身不证明文字被裁切，规则未改。

![实际下一页到第14页，末库存3](raw/ui-gameshop-jni/104-tap.png)

仓库 PID24896：实点 STORE II、同 PID 恢复保留第二页通过；**仓库遮挡源背包仍未修好**。
正式包 PID25060：传同一预览参数仍是 Login / Test server not configured；
无 Java 预览生产者、无共享预览消费者、无 UI_PREVIEW_READY，隔离门通过。
没有真实 HTTPS/WSS 登录、购买、Mail、存取、Zone 或存档闭环。

商城/仓库/正式三独立 PID 分别记录 GL506 6/0/11，总17；未观察 fatal/panic。
不是累计截图逐次相加，不与 v33 不同场景集比较成“渲染改善”。
**零渲染错误门仍 FAIL**；未实施新的 GPU 修复。锁定仓库密码/可见 IME 本轮未重新验收，
继续保留 [v33 FLAG_SECURE 导致空原截屏的缺项](../native-android-personal-jni-20261003/README.md)。

## APK（本地、未入 Git）

正式包：
[mir2-native-gameshop-page-cache-debug-v34.apk](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/gameshop-hit-v34-20261003-yxhQVz/final-apks/mir2-native-gameshop-page-cache-debug-v34.apk)

SHA-256 `1480b838f5e174e4f014679d1e02063f830b149ba3fd74ab1fe23d952eaa13c0`，387331897 bytes。

预览包：
[mir2-native-gameshop-page-cache-preview-v34.apk](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/gameshop-hit-v34-20261003-yxhQVz/final-apks/mir2-native-gameshop-page-cache-preview-v34.apk)

SHA-256 `a97163a19d08b8bc1d417b3a94724a6880baee6c5ea1a980c6f3249e4ce3cab3`，391205185 bytes。

版本34 / `0.1.31-gameshop-page-cache`；Gateway URL 为空。
预览包只作离线诊断，不称真实登录或原生完整游戏完成。

## 剩余与下一叶

手机商城重排、字号、≥48dp 控件/紧凑窗口/IME/多指/语言仍 OPEN；
仓库与背包的成组手机布局仍 OPEN。下一叶处理这两类呈现，保留共享动作、页数与交易规则。
NI-12 购买/Credit/Mail、NI-13 在线精确存取、NI-14–20、真实认证/Zone/保存/重连、
完整资源/音频/更新、零渲染错误、真机和人工最终验收都不因本叶变为完成。
原两工作区 HEAD/分支/Git status 前后相同；未递归哈希未跟踪内容。

`manifest.json` 是278个精确 payload/16957314 bytes（不包括本 README、manifest、
verifier 和 .gitattributes）；APK/SO/完整资源缓存/密码/签名密钥未入档。
索引原字节核验已通过；提交前文档空白检查曾报两处 Markdown 硬换行的尾随空格，
改成段落空行后重验。此为文档准备问题，不是新的产品失败；278个 payload 原字节未改。
`verify-evidence.mjs` 支持 working/index/HEAD，核对原字节、源码提交、红绿、七门、
安装/资源审计、PID隔离、21原图和剩余门；不能自动代替截图语义、APK实机运行或人工验收。
