# Android 最终输出附件诊断与底层对照（不是完成验收）

本叶是 `58a92eb7b72391013e1a5a20195778e205e565d8` 与
`1fbc63f8737cc8b9415bafd19fb068b4856b1253` 的独立源码证据。
两版诊断都没有修改渲染行为；已修复的 VIEW_FORMATS 兼容性仍来自历史 `1f01c1351`。
完整 Windows 对齐 goal 仍 Active，NI-17 仍 PARTIAL。不能把本叶收窄为离线灯光目标。

## 基线与保护

Windows 只读刷新时间：2026-10-06T03:11:06.559Z。当前选定分支
`codex/playtest-registration` = `56ee063fb4f9d6b581ec548659066ef9c5e05e9b`；
冻结功能基线 `3d735745f1117d42a7859e87604a106351dca935`。差异仍为一个提交、
83 个文档路径、零功能路径。旧 windows-player-journey 不是本 goal 的最新功能来源。
PR #253 在刷新时仍 OPEN Draft，Android 远端当时为 `43578a615`；
本叶提交/推送结果另由本机 delivery 记录核验，不把此推送前快照称作推送后结果。

两版各有 446 构建输入绑定，哈希分别为：

- 58版：`56f7c8838917ede452ba5958763547a1932a6f3a432c5cc96d26ed0395d708d0`。
- 1f版：`9ff0b6f2f0333134d4ba61201dae2b996adece1e0624f00dd336a775740a3d94`。

58版三个诊断文件，443 个其他整文件受保护；
1f版仅两个文件/七行增加，444 个其他整文件受保护。
原主体、首次输出 atomic、八个原预算测试、所有游戏规则保持字节一致。
只在 Android 上、显式 `MIR2_ANDROID_GPU_SURFACE_PROBE=1` 时读取已有 HAL 元数据；
每进程总预算 128，不随 resize/lifecycle 重置，默认关闭，不发 GL 命令、不改变队列顺序。
诊断本身可改变竞态时序，任何零错误短样本都不是生产接受。

两个原 worktree 的 HEAD、分支与 Git 状态核验保留；
这是 Git checkpoints 保护，不冒充已递归核验所有原未跟踪文件内容。
不推 Windows，不 reset/clean/stash/force，不部署生产，不改真实存档，不绕过认证。

## 两个精确原生诊断 APK

都只是新建 uiPreview 原生 Bevy 包，不是 WebView、Store release 或真实玩家验收。
API31/arm64-v8a，code35/name0.1.32-gameshop-phone，Rust release profile；
普通 variant 本轮没有重建，不重绑其旧包。
两个 Gateway 为空，preview 不允许联网。
保留数据安装和回读安装包 SHA-256 一致，variant ELF 也核验一致。

| 源码 | APK 字节数 | APK SHA-256 |
| --- | ---: | --- |
| 58a92eb7b | 542088495 | `8671c291f34d975af9c94455021ad6186630c2fdcd7ba4c4734c586d7237af74` |
| 1fbc63f87 | 542087175 | `600018c2bc2d2191cae1e64ee619df06c35bb39a603edbe9b9ede825bb67d6b6` |

本机 APK 路径：

- `/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/gpu-render-pass-20261006-dfsojC/final-apks/mir2-native-render-pass-probe-preview-58a92eb7b.apk`
- `/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/gpu-output-attachment-20261006-SfDfsL/final-apks/mir2-native-output-attachment-probe-preview-1fbc63f87.apk`

ELF SHA-256：

- 58版 `fbc052a15cc41bafcf1b8ca3775e505807fa1cd6e5ed0151c687d81ec61c0e7b`。
- 1f版 `136824a46db2475ab4eb4ae82153bc0f32b6b6f13a761d04bb65c3b8e382b0c2`。

各包选定 6647 UI PNG、3 metadata、10 原 Lighting 图逐字节核验；
原 UI_32bit470–473 与 Lighting0–9 匹配冻结 Windows。实体 manifest 为
`929d146535bde9b61c75c33af0e98852dcc3aef83320869aac9f06f4bdd530d9`。
本地比奇有界帧607 tiles/7 atlases/unresolved0并不关闭全图2969缺失引用或完整资源发布门。

## 每版十四项新鲜适用门

两版都在各自干净精确提交上重新执行并绑定输入，均 PASS：
Android529、preview577；shared native UI1324 +10原ignored；runtime309 +1原ignored；
fresh Java debug/uiPreview 各103/8类，failure/error/skip均0；
API31普通/preview编译；Mac-host Windows原光照14、地图4、效果2、协议21、
Hero FIFO1、skill FIFO1；真实获准本地比奇资源门1。
各计数重叠，不能相加；Mac-host 子集不是 Windows OS 全量验收。
新增预算八测试已由历史951叶提供，本叶未宣称新增游戏GPU回归测试。
完整逐项日志/结果SHA在 [source-evidence.json](source-evidence.json)。

## 实际游戏 APK 样本：GPU 门仍 FAIL

设备：专用 Mir2_API_31_ARM64 / emulator-5554 / Android12 API31；
五次独立冷启动各采样8秒，均实际 resumed、无 fatal/render validation/AppExit；
十张原始2340×1080图片逐张查看。
地图地板、Self/实体、FireWall 和夜间/黎明灯池可见，白天灯光关闭。
空快捷槽和大型诊断HP面板仍在，不算完整手机UI；没有登录、在线移动、Zone保存或真机验收。

| 源码 | 夜间三个PID（GL506） | 黎明PID（GL506） | 白天PID（GL506） | 总GL506 |
| --- | --- | --- | --- | ---: |
| 58a92eb7b | 15367(0),15445(4),15523(0) | 15602(0) | 15680(1) | 5 |
| 1fbc63f87 | 15848(0),15926(4),16005(0) | 16084(5) | 16163(0) | 9 |

58版仅 tracked main/UI 两通道可见，最终 upscaling 走 raw encoder，诊断覆盖不完整，
不得据此否定旧最终附件假设。1f版补只读 OutputColorAttachment 入口后，
两个实际失败帧都是首次3520×1980→2340×1080，
acquire RBO5 → current-size main/UI texture → final output RBO5 → GL506 → present RBO5。
旧最终输出 handle 假设仅在这些失败帧被否定；未覆盖的无相机 clear 和预算外帧不冒充已验证。
这仍只是编码时元数据，不是 GPU/驱动实际绑定测量。

## 同模拟器底层最小对照

本机独立 C 工具：
`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/gpu-output-attachment-20261006-SfDfsL/gles-attachment-repro.c`
源码 SHA-256 `2d5451965b55f2bcd0afd2e71fe7f45a1650c28f197dc03885c34c9a39ec8289`；
已编译 ARM64 二进制 SHA-256 `a55851e624e5f21e5117d866da94b326108bc3d040e9d1bbddc137bf325975d9`。
只用独立 GLES3 pbuffer，不经过游戏、JNI、认证、网络或存档。
专用 helper 路径 `/data/local/tmp/mir2-fbo-repro-20261006-1fbc63f87`，推送前不存在、
推送后哈希匹配。未卸载/擦除AVD、清系统日志或 kill 全局adb。

三轮/九个独立进程的相同序列，对照结果：

| 模式 | 结果 | status / drawError | 像素回读 |
| --- | --- | --- | --- |
| keep_old：旧RBO仍存活，仅纹理替换 | 3/3 PASS | 0x8cd5 / 0 | [51,102,204,255] |
| texture_only：旧RBO删除，仅纹理替换 | 3/3 FAIL（有效重现） | 0x8cd6 / 0x506 | 不读取不完整FBO |
| dual_detach：旧RBO删除，先显式解除RBO类型再解除纹理类型 | 3/3 PASS | 0x8cd5 / 0 | [51,102,204,255] |

texture_only 的退出1是预期失败证据，不是绿色游戏测试。
首轮工具错链 GLES2 导致 GLES3 符号未解析：未编译/未上设备，单独保留；
改为 GLESv3 链接后才得到上述有效对照，未修改 C 程序或游戏源码以伪造通过。

官方 Android12 源码比较取自固定 tag：
[GLClientState.cpp](https://android.googlesource.com/device/generic/goldfish-opengl/+/refs/tags/android-12.0.0_r1/shared/OpenglCodecCommon/GLClientState.cpp)、
[GL2Encoder.cpp](https://android.googlesource.com/device/generic/goldfish-opengl/+/refs/tags/android-12.0.0_r1/system/GLESv2_enc/GL2Encoder.cpp)。
本机检索/哈希保留；报错行4585一致不等于已证明模拟器二进制的精确源修订。
源码与独立对照证明此设备的附件类型保留问题；游戏正式修补后仍须重新复现，
不能仅凭候选机制或独立像素测试关闭游戏GPU门。

## 下一步范围门与完整剩余验收

提出固定同版本 wgpu-hal29.0.4 本地源码副本，仅 Android workspace 使用：
81 个未经行为修改的上游文件中，只计划修改 GLES queue 一个文件；
再改 Android Cargo.toml/Cargo.lock，共83个文件，不升级版本、不改其他workspace。
investigate 技能超过5文件要求范围确认，已向人类提问，当前 PENDING；
尚未引入依赖、尚未实施正式行为修补，不把默认选项当授权。

批准后做绑定原问题的 compiled red/green、完整适用门、未启用诊断的真实APK
重复冷启动/resize/后台恢复，并保持原错误证据。
原Windows功能分母AP-01–21不缩减；仍须完成：
无错误游戏渲染、全手机共享UI/多指/IME、完整地图对象动作与render-ready、
真实HTTPS/WSS/认证/玩家闭环、共享Zone权威移动与重进存档、
NativeResumeV1/后台/网络/音频、获准全资源/缓存/更新交付、
Android真机与最终人工接受。
真机当前0台；完整 goal Active，NI-17 PARTIAL。

APK、日志、原图、缓存和官方源比较保存在本机忽略目录；Git只提交文档与小型证据清单。
旧阶段旧包、旧日志和旧来源均不重绑为本叶成功。
