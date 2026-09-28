# 外部试玩发布检查 — 2026-09-28

## 2026-09-29 实施进度

用户已授权准备独立联网内测服与完整玩家 ZIP。已建立独立干净构建
工作区；原玩家存档和未提交的 quest-agent 资料保留。

- 修复原生连接的两个实际缺口：`tokio-tungstenite` 原来未启用 TLS，
  初连和断线重连也未带 staging 网关要求的 Origin。现在使用系统证书
  信任根，并从 WSS 地址构造 HTTPS Origin；不跳过证书或来源校验。
- 四项新增连接回归通过，原生全量 753 项通过、三个已有显式检查忽略。
  TLS ClientHello 测试不等于真实公网账号登录验收。
- Linux 发布工作流补齐专用非 root runner 的 publisher UID，并加入现有
  Linux 发布/保存恢复安全检查；实际云端构建与测试结果待后续记录。
- 已通过用户提供的登录方式进入现有服务器，只读确认线上仍是
  `79ba815c0-checkpoint-base64`。独立测试服准备使用新数据库、Redis
  实例、服务和私有端口；现有生产服务尚未替换。
- 外发脚本现要求真实 WSS 地址，并在签名前写入 newcomer-v2 与固定白天。
  127 个怪物/NPC/入口资源库、25,373 帧检查通过；15 张主线及补给沿途地图
  独立核对 24,780 个引用，补入 1,509 张原版地砖后，可绘制资源缺失为零。
  原素材越界 842、空帧 337 单独记录，未伪装成有效图块。
  Windows PowerShell 5.1 打包/验包自测与供应链检查通过；中文玩家说明已加入。
- 实际 Linux 非 root 安全检查通过。TERM 测试现在等待整个进程组完成清理，
  保留原有空目录断言；新增延迟清理、僵尸处理及超时失败回归。服务模板
  与既有可信安装器的 systemd 字节同步，未修改线上生产服务。
- 普通双账号联机验收仍在进行，已发现聊天延迟与退出后的租约残留。此时没有
  可称为已验收的外发包，也没有声称法师/道士完整路线或承载验收通过。

以下为 9 月 28 日的调查记录，保留其时间和验证范围。

本次范围：排查 Git 历史对象问题，核对当前版本能否给外部玩家使用。
未修改游戏逻辑，未部署或替换线上服务，未创建玩家账号。

## 结论

已有联网架构，也有可访问的公网 Web/Gateway 服务。当前本机验收包
仍连接 `ws://127.0.0.1:19910/ws`，其资源目录是开发工作区的 Junction。
它不能直接作为独立外发包。建议先准备小范围邀请试玩的完整客户端
ZIP；客户端与测试服务器配套验收后，再安排安装器和更新流程。

## 已查实的运行状态

2026-09-28 15:26 UTC 的只读 HTTPS 检查：

| 检查项 | 结果 | 能证明的范围 |
| --- | --- | --- |
| `https://mir2.obelisk.build` | HTTP 200 | 公网游戏网页可访问 |
| `https://165.154.65.136.sslip.io/health` | `ok=true`，HTTP/WS ready，Redis healthy，identity PostgreSQL | 网关和基础服务正在运行 |
| `/api/asset-manifest` | `20260818-main-97887dcd` | 当前公开资源版本；不能据此推定网关源码版本 |
| 当前开发客户端 | 源码 `dee30a74d`，配套本地 Gateway `abee09b21` | 本机版本有明确来源，尚无同版本公网联机证明 |

网关健康接口没有提供源码提交号。配置允许的连接数不能作为承载验收
结果；本次未登录公网账号、进行游戏或执行压力测试。

共享多人世界已在代码中实现：

- `apps/gateway/src/routing.rs` 使用共享 `ZoneManager`。
- `apps/simulation/tests/shared_zone.rs` 覆盖两玩家互见、移动广播，以及
  多人攻击同一怪物时共享血量。
- 这些能力与当前开发客户端连接本机服务器是两件事；不能把本机测试
  地址理解为没有联网功能，也不能把历史线上服务当成本轮版本的验收。

## 给测试玩家的版本应包含什么

1. 一套固定版本的 Windows 客户端和实际资源文件，不依赖开发目录链接。
2. 在生成并签署清单前写入真实测试服 `wss://…/ws` 地址。原生客户端
   只对 loopback 允许明文 WS，普通局域网地址同样需要 WSS。
3. 集中运行配套 Gateway，玩家使用各自账号；服务端数据持久化和备份
   留在服务器。玩家包采用现有 client-only 规则。
4. 包含版本号、校验信息、启动/操作说明和已知问题。法师/道士 0–30
   实玩及完整汉化仍是开放项，试玩说明应如实记录。
5. 用没有开发环境的电脑解压运行；以两个独立账号核对注册/登录、同图
   互见、共同战斗、换图、掉线重连，以及退出后重新登录的存档。

先安排少量邀请测试，再依据同版本压测和实际游玩结果决定扩大人数。
安装器解决安装位置、快捷方式、卸载等体验，自动更新需要另有更新流程；
已有 ZIP 流程可作为第一批试玩的交付形式。Windows 官方也列出独立
EXE/ZIP 与安装器等不同分发方式：
[Choose a distribution path](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/choose-distribution-path)。

## 现有发布工具及明确缺口

- `apps/game-client/platform-windows/scripts/build-attested-windows-candidate.ps1`
  和 `package-windows-candidate.ps1` 已有固定源码构建、完整资源复制、
  文件清单和签名验证流程。
- 打包脚本当前把 `mir2-client.toml` 的网关写为
  `wss://candidate-gateway.example/ws`。外发前需要接入真实测试服配置，
  并在生成清单/签名前写入，不能事后修改已经验证的包。
- `verify-windows-candidate.ps1` 核对资源、无链接目录、清单、WSS 等；
  现有开发目录的 Junction 不满足独立外发要求。
- `docs/WINDOWS-CANDIDATE-SUPPLY-CHAIN.md` 的正式发布还有 main 分支、
  发布环境、构建证明和签名要求。内部测试 CMS 签名不等于公开发行签名。
- Linux 服务器发布和回滚流程见 `docs/GATEWAY-RELEASE-RUNBOOK.md`。
  需先确定本轮版本对应的独立测试服，核对新手任务配置及资源来源。

本次没有新建安装器、生成可外发 ZIP，也没有把开发分支直接部署到公网。

## Git 历史对象调查

对共享对象库 `E:/mir2/.git` 完成 `git fsck --full --no-dangling`：

- 两个损坏的压缩基对象：`b474381147c777060e8f561c4d756d428d1c6a2a`、
  `1ae7f886fa87573bdbb59caf0772103929fac94c`。
- 两个依赖它们而无法读取的对象：`4a9c304d3781cfa06e6909498db5f324603f0b3f`、
  `9c0219f9e62b6b8ce3e13ae3eb58670a42993edb`。
- 全部位于 `pack-dbf0490fb500ffdeefe5483e77eecd14065538ec.pack`。
  已确认相关路径为旧的 `pkg-webgl2/mir2_bevy_runtime_bg.wasm` 和
  `pkg-webgpu/mir2_bevy_runtime_bg.wasm`。
- 原始 Deflate 可解出声明长度，但 Adler 校验与 Git 对象哈希均不匹配。
  单纯改校验码不能恢复原文件；针对第一个已报告对象的有限单字节恢复
  尝试也未找到与原始 Git 哈希一致的内容。
- 7 月 7 日提交 `064fbcd159085e3f5e10d2e7ea1b83c4ebf36a8f` 位于本地
  `codex/movement-feel-parity` 和 `codex/uncovered-map-tiles` 历史中。
  GitHub SSH 拒绝取该旧提交，Blob API 也未找到该对象。
- 当前发布分支 `666617bfe` 的可达对象列表不含上述四个对象。
  用户确认没有已知的其他旧备份。旧内容暂未恢复，原因不能归结为
  磁盘、内存或某个操作，因为当前证据不足。

受损 pack、idx、rev 已原样复制至
`C:/mir2-ui-repair-20260921/git-integrity-20260928/damaged-pack-backup/`，
各副本 SHA-256 与源文件一致。pack 的 SHA-256 为
`73A1BECD6C32D8CD3458556D3405FBF7843AE5A2847B5CADECAD66F2D86BFAC2`。
原库、旧分支、未提交的 quest-agent 数据和游戏存档均保留。

当前提交 `666617bfe0086b700ca7f38f573660e393904e9c` 的独立浅快照已建在
`C:/mir2-ui-repair-20260921/git-integrity-20260928/verified-release.git`。
它通过 `git clone --bare --no-local --depth=1 --single-branch` 从现有已推送
分支重建，独立保存对象，无原对象库链接；`git fsck --full --no-dangling`
退出码为 0，日志为空，确认该快照完整。源码树共有 122,971 个文件，
760.6 MiB；这不是最终玩家包大小。快照的 SHA 与已核验远端提交一致。

直接从 GitHub 获取完整浅快照的传输耗时过长，已取消本次传输并保留
临时目录；上述完整验证使用的是独立重建的本地对象库，未冒充完成的
远端全量下载。该快照保存当前可用源码，不恢复或重写损坏的旧历史。
