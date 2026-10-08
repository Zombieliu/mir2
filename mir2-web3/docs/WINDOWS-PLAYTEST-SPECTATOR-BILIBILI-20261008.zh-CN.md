# Windows 测试服观战与 B 站接入

2026-10-08：已完成接入代码、本机只读网页、服务器配置预备和校验。
**尚未启用在线观战，也未向 B 站开播。** 用户此前要求游玩期间不切换；
退出确认仍待回复。最新检查两套网关连接均为零，但这不替代用户确认。
原测试服二进制、角色存档、客户端安装和现有 Caddy 配置保持原状。

## 玩家现在要做的事

1. 用本人的 B 站账号登录直播中心，按平台提示完成实名认证和开播资格检查。
2. 从[官方页面下载电脑版直播姬](https://live.bilibili.com/liveHime/)，登录同一账号。
3. 先等待本页的“启用和验收”完成，再在直播姬中添加观战浏览器的窗口捕获，
   调整预览、游戏分类和音量，然后用自己的账号确认开播。
4. 在另一设备打开直播间，检查实际画面、声音和连续播放。

找不到第三方推流码不必阻塞首轮验收：官方直播姬可以采集浏览器窗口。
第三方推流另受账号权限限制；官方排查指南目前列出 5,000 粉丝要求，
而官方直播姬的开播门槛不同。[B 站官方权限说明](https://www.bilibili.com/blackboard/activity-pSrb2KQb6G.html)
不要把推流码、密码或直播后台截图中的密钥发到聊天或提交到仓库。

## 已完成的接入

- 网页支持独立的 `NEXT_PUBLIC_MIR2_SPECTATOR_WS_URL`。设置观战测试服不会
  改变玩家登录入口；派生 HTTP 地址保留 `/playtest` 前缀。
- 本机观战页使用 `wss://165.154.65.136.sslip.io/playtest/spectator/ws`，
  状态 API 使用 `https://165.154.65.136.sslip.io/playtest`。
- 观战页显示等待、公开延迟缓冲、实时、停止更新和录像状态；保留 30 秒公开
  延迟，首轮缓冲不会被误报为停更。观战 socket 不发送玩家 keepAlive。
- 初始地图为比奇，地图文件名 `0`；录制关闭，采集间隔 1 秒，每图缓冲 90 帧，
  每帧最多 256 个实体。公开代理只增加 matches、只读 spectator WS 和 AI status。
- 默认运维命令只准备配置和校验；上线需要两个独立显式动作。运维脚本检查
  两个网关的六项连接/租约/登录入口计数、配置与二进制指纹，保护并发发布。
  临时维护入口的实际 Caddy 适配顺序已经验证，失败恢复不覆盖其他操作者的改动。
- 独立编码容器支持 B 站 RTMP 输出和私密文件挂载。不依赖完整开发数据库栈；
  不把推流码放入容器环境变量或 Git，编码日志使用私有权限。

## 本机网页

本次检查的开发页为
`http://127.0.0.1:3211/spectate?spectateMap=0&bevyRuntime=0`。
它只在当前开发电脑运行，依赖本次 Node 进程，不是已发布的公共观战网站。
`bevyRuntime=0` 使用现有兼容渲染路径；真实地图、人物和持续动画尚待数据接通验收。

要在受控观战电脑重新启动，在 `apps/web` 配置以下环境后运行本机 Next 服务。
勿修改玩家网关配置，也勿把私密 token 放入下面的公开配置。

```powershell
$env:NEXT_PUBLIC_MIR2_SPECTATOR_WS_URL='wss://165.154.65.136.sslip.io/playtest/spectator/ws'
$env:MIR2_SPECTATOR_HTTP_URL='https://165.154.65.136.sslip.io/playtest'
$env:NEXT_PUBLIC_MIR2_ASSET_BASE_URL='https://assets.mir2.obelisk.build/mir2/v/20260818-main-97887dcd'
$env:MIR2_ASSET_BASE_URL=$env:NEXT_PUBLIC_MIR2_ASSET_BASE_URL
node node_modules/next/dist/bin/next dev --webpack --hostname 127.0.0.1 --port 3211
```

可在基础验收通过后追加 `aiLive=1&aiLiveAudio=0` 隐藏观战控制面板供直播采集。
这不会启用 AI 解说，也不代表解说服务已部署。

## 启用和验收

服务器已私密预备到 `/var/lib/mir2-playtest-spectator-stage`，权限 `0700`。
状态为 `proxyApplied=false`、`gatewayActivated=false`。有效 Gateway 版本为
`c6c32381a646dae1067dafdeec57691f606b1779`。

用户确认正常退出后，操作者依次执行以下步骤，不能把默认预备成功视为已上线：

1. 重新核对实际 Caddy、systemd、环境、二进制指纹与两套网关入口计数。
   若其他发布改变了输入，停止并针对新版本重新评估，不能重启另一轮发布。
2. 在 Linux 主机上执行 `python3 stage-spectator.py --apply-proxy`，然后
   `python3 stage-spectator.py --activate`。脚本只设置观战特性覆盖，不换二进制或数据库。
3. 检查 HTTPS matches、只读 WSS、非公开地图拒绝、写操作代理仍拒绝；检查
   网关原版本、容量限制、录像关闭以及错误日志。
4. 由普通玩家正常登录 Windows 客户端进入比奇。没有玩家进入时没有新的画面，
   不得借用玩家账号或拿旧生产服缓存充当当前测试服数据。
5. 用网页实际接收连续递增的 sequence 和捕获时间，确认实体、地图资源和人物
   动画可见；同时确认浏览器输入不发出玩家指令。
6. 再用官方直播姬采集这个窗口，完成 B 站实际接收和第二设备观看验收。

启动 `scripts/smoke-spectator-readiness.mjs` 默认只检查等待页；设置
`MIR2_SPECTATOR_EXPECT_LIVE=1` 才要求真实连续帧。浏览器使用隔离的 headless
配置，不占用用户日常浏览器或登录角色。实时检查也不等于 B 站推流验收。

## 第三方 RTMP 可选路径

只有账号正式允许第三方推流、观战实时验收通过后，再使用
`infra/ai-live/.env.bilibili.example` 与 `infra/ai-live/compose.standalone.yml`。
把推流地址与直播码组成的完整 RTMP URL 保存到仓库外的私密文件，设置
`MIR2_BILIBILI_SECRET_FILE`，并将 `MIR2_AI_LIVE_RENDER_URL` 换成编码容器可访问、
已指向当前测试服的观战页面。模板中的 `.invalid` 地址故意不能开播。
只监听 `127.0.0.1` 的本机 Node 服务不能直接供 Docker 容器访问。

首轮预设 1280×720、30fps、视频 3000k，编码容器最多 2 CPU / 2GB 内存。
应在独立编码电脑运行；当前 4 核游戏主机未安装 Docker/Chromium/FFmpeg。
本机 Docker daemon 也未运行，故本次没有构建容器或验证 HLS/RTMP。
FFmpeg 进程参数和日志仍可能包含完整输出地址，只能保留在受控编码主机。
健康检查仅检查进程，不证明 B 站已收到视频。

## 检查证据与范围

[准备和检查记录](generated/player-qa/spectator-playtest-20261008/readiness.json)
及[等待页面截图](generated/player-qa/spectator-playtest-20261008/spectator-waiting.png)。

| 检查 | 实际结果 |
| --- | --- |
| URL 前缀、可信配置、隐私查询与实时/缓冲状态 | Node 6/6 通过 |
| 运维准备、占用入口拒绝、失败恢复、并发修改与两个入口顺序 | Python 19/19 通过 |
| 网页严格 TypeScript | 通过 |
| 隔离浏览器等待 UI、异常与玩家输入 | 通过；无玩家指令或运行时异常 |
| 编码脚本语法、独立 Compose 静态配置 | 通过 |
| 真实服务器 Caddy validate/adapt 与版本指纹 | 通过；没有 reload/restart |
| 当前测试服新帧、实际地图/人物、50–100 人负载 | 未验收 |
| B 站账号权限、真实开播/接收、AI 解说 | 未验收 |

数据来自真实共享 Zone 的玩家视野投影，再按地图合并；不是完整地图的原子快照。
首轮只验证一个测试服、一个地图和现有频道。公开地图限制只约束观看，
不能据此宣称所有地图采集内存都已有界；每个玩家仍会构建投影，地图条目也
不会自动全部释放。NPC 任务标记还可能反映视野来源的进度。库存等主要私密字段
由现有服务端过滤；本次没有修改 Gateway 采集算法或宣称全部隐私/性能验收完成。
旧生产服观战缓存来自更早会话，不用它证明 Windows 测试服直播已接通。

保留的失败：跨盘 node_modules 链接导致本机 Next 解析失败，已改为隔离工作区
自己的依赖；首次维护入口排序验证发现 legacy 响应位于代理之后，改为匹配
handle 后真实适配校验通过。所有实际开播和线上重载仍是待完成项。
