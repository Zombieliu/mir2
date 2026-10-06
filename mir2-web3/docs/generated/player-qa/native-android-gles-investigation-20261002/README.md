# Android GLES 定位与无探针基线 — 2026-10-02

完整 Windows 对齐 goal 仍 **Active**。本轮完成有界定位、移除全部临时探针、
刷新两个无探针原生诊断 APK；**游戏渲染未修复**，v28 任务场景仍有24条
GL0x506，零错误渲染门 FAIL。不关闭手机 UI、完整资源、真实联网或真机门。

## 结论与边界

按 investigate 的失败先行流程，先排除两种解释，再用同一 EGL/设备中的私有
4×4对象复现第三种。没有修改游戏规则、认证、Windows行为、MSAA、后端选择，
也没有增加摄像机等待帧数、隐藏错误或改写本机 Cargo 缓存。

| 假设 | 实测 | 判定 |
| --- | --- | --- |
| surface RBO 没有有效颜色存储 | 每场景129条附件及3条配置记录；live、RGBA8、尺寸、samples=0均正确，错误仍发生 | 排除 |
| 活跃视图仍持有旧 surface | 每场景130条视图记录；活跃视图串线0次；后台恢复时各3次旧视图仅发生在 camera/view均缺席，无新增绘制错误 | 排除为这些绘制错误的解释 |
| 模拟器保留旧附件类型元数据 | 私有 FBO 从 RBO换成有效 texture，解绑后删除旧 RBO，查询仍显示 texture，但 completeness从 COMPLETE变为 INCOMPLETE_ATTACHMENT；显式清除 RBO类型再挂 texture 后恢复 | 两场景独立复现；尚未修入真实游戏 |

第三项是**依赖修复候选**，不是游戏修复通过：私有复现不使用 wgpu 私有对象、
不绘制、不清 GL error；保存/恢复已有绑定并删除自己的对象。v27每场景额外一条
format0日志来自刻意的私有失败检查，和实际游戏 GL0x506分开计数。

官方 Android12 源码提供一致的解释：attachTextureObject未清旧 hasRbo，
附件格式查询优先旧 RBO，绘制前由编码器本地检查 completeness。
这是对源码与本机复现的推断；不声称源码分支与 SDK二进制逐字节一致。
[GLClientState.cpp](https://android.googlesource.com/device/generic/goldfish-opengl/+/refs/heads/android12-release/shared/OpenglCodecCommon/GLClientState.cpp)
及 [GL2Encoder.cpp](https://android.googlesource.com/device/generic/goldfish-opengl/+/refs/heads/android12-release/system/GLESv2_enc/GL2Encoder.cpp)。

当前 wgpu-hal29.0.4 的 GLES ResetFramebuffer 只用 texture_2d(None)清附件。
已提出固定当前依赖源码、只修改一个 Android 路径的候选；约83个依赖源文件的
纳入范围已单次向用户询问，**尚无答复，未 vendor、未改缓存、未实施候选**。
其他安全 UI/宿主代码工作继续，不把整个 goal 标成 blocked。

## 精确源码、设备与独立复现

设备只限 emulator-5554 / Mir2_API_31_ARM64 / Android12 / API31。
density440，未裁切原始截图2340×1080。每例冷启动、同PID后台恢复；
安装前后核验实际 base.apk SHA与版本。未清日志、wipe、卸载或访问服务端。

| 版本 | 源码 | 独立 PID / GL0x506 |
| --- | --- | --- |
| v24新鲜基线 | 9a6bff0db9a7ff3eed003c72c90310ec632572d0 | login16035:12 / world16125:0 / quests16219:10；合计22 |
| v25 surface探针 | 3bedb220771954a499d31472670108709b066532 | world16421:19 / quests16511:10；合计29 |
| v26 view探针 | 8c7d7cfd8ca30e3ef92a7dc2129e1c437d4c1b59 | world16740:21 / quests16832:4；合计25 |
| v27私有复现 | 2a9d81facb54fd41a7912dcf75ef9eaceeb5aac0 | world17096:0 / quests17187:8；合计8，私有format0另列 |
| v28无探针当前基线 | 84fe8e6344bb0c041e01c66a149b5b42b8198565 | login17484:0 / world17575:0 / quests17665:24；合计24，FAIL |

这些是独立运行，**不是错误下降趋势**。只统计每PID最新累计日志，
不把 cold/latest重复相加；后台恢复没有新增GL0x506，所选fatal/panic模式均0。
本轮24张原始PNG已逐张查看。没有本轮新增UI实现、控件点按或在线验收；
历史v24五PID82条失败和旧操作证据保留在各自报告，不被本轮22条替代。

## 当前安装包与源门禁

v28 / 0.1.25-diagnostic-free；原生 Bevy GameActivity/Rust cdylib，
Gradle Debug诊断构建，不是商店 Release、Web套壳或完整原生产品验收。
两个渲染文件与已发布277ab0a6566b5241c418844e5366865547cd03e0逐字节相同。

- Debug：387145913字节；SHA-256
  `b8b93a2157bd8f3a715edb0fe7e4ef758aa28cc55d717434b87b1660bdba2e0c`。
- uiPreview：391013021字节；SHA-256
  `7d4aedf5dc3c0d17f2fda76e616880de325e32177363cbf057ebf104341b6fae`。
- 两包位置见 [精确打包记录](raw/builds/package-baseline.json)，仅保存在 Android target，
  不提交APK/解包缓存/素材/密码/签名密钥。正常包Gateway为空，preview禁止联网。
- 41项源码输入匹配Git精确84fe提交；六门禁前后源码hash相同。正常327/327、
  preview350/350；共享UI1238通过/0失败/10原有ignored，Java强制重跑两变体
  各41项/0失败/0跳过；实际arm64 API31两检查通过。计数重叠，不换成产品完成率。
- 两包的选定Items1003/StateItem5192/MagIcon224/MagIcon2224/UI_32bit4，
  共6647 PNG及3 metadata匹配既有输入；四UI_32bit图另匹配冻结Windows3d。
  仅选定资源完整性，不能代表完整游戏资源或发布来源验收。
- 保留 view探针第一次E0425编译失败（Msaa类型路径），第二次检查通过；
  基线辅助脚本第一次shell quoting失败也保留。它们是设置/编译失败，
  不是“产品修复先红后绿”的证据。探针check发生在提交前的准备源码，
  package/安装才绑定各自已提交源码，原始status/hash记录不改写。

## Windows来源与原工作区

完整对齐分母继续冻结为3d735745f1117d42a7859e87604a106351dca935，已在Android祖先中。
上一轮观察的Windows56ee063fb之后，本轮API读取同名远端分支为
6ae080711fc7b3aaf06dd9c6bcf63f121682dbe4，是落后上一观察33提交、
落后冻结分母32提交的祖先。原因未调查，不推断操作者或意图，不自动降级分母。
[原始比较](raw/builds/windows-ref-comparison.json)返回300文件，有API上限，
**不是完整变化文件目录**。没有reset、写Windows远端或改变PR base。

两个原工作区的HEAD/branch/status与v24前记录相同；只读比较在
[原工作区检查](raw/builds/original-checkouts.json)。这不是未跟踪文件内容的逐字节审计；
不称整目录内容hash通过，也没有向这些目录写入。

## 下一叶与完整验收缺项

继续共享手机NPC任务列表/确认/提示、compact/IME/九语言/多指，
以及完整NI-11–20宿主入站和动作路由。共享文件改动前声明具体写集，
保留Windows默认路径与服务端权威，不复制任务、战斗、交易或存档规则。

真实认证JNI/HTTPS/WSS、角色/StartGame/Zone权威移动与重进存档、
完整对象/资源及稳定切场、音频/可信更新、真机及最终人验仍OPEN。
获准测试环境/普通账号、完整资源来源和物理设备缺项各自保留；
不部署生产、不使用未知线上存档、不搬用其他工作区凭据。

可在此独立worktree运行 `node mir2-web3/docs/generated/player-qa/native-android-gles-investigation-20261002/verify-evidence.mjs`。
验证原始字节、历史Git源码hash、PNG尺寸、PID/安装绑定、所有计数与门禁；
**不重新连接设备/服务器、不重新运行游戏，也不证明图形修复**。

首次证据提交的Git默认换行转换影响5个avd.txt（磁盘原件未改）；后续有界证据
修补仅在本目录raw/**关闭text转换，重存原始字节。保留初次5项hash差异记录，
核验脚本逐项比较manifest、磁盘与HEAD blob，保证后续checkout也可核验。
