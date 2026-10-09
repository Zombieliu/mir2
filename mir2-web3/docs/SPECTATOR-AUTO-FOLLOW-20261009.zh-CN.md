# 观战网页自动选镜头

2026-10-09，本机 Windows 测试服观战页的“自动选择”已改为依据近期掉血和附近交战选镜头。实现仅改变已授权延迟快照的网页视角；当前网关仍强制公开延迟 3 秒，只公开比奇地图 `0`。这是观战支线的限定交付，不是完整服务器精彩事件日志、AI 导演、B 站开播或经典玩法 Goal 的完成。

本机地址：`http://127.0.0.1:3211/spectate?spectateMap=0&bevyRuntime=0`。
直播采集地址：`http://127.0.0.1:3211/spectate?spectateMap=0&bevyRuntime=0&capture=1`。
刷新网页即可加载；此地址依赖本机网页进程，不是公开托管网站，也不需要更新 Windows 游戏安装包。

## 本轮实际行为

- 只在当帧 `selfPlayer/player` 的存活实体里选择，使用 object ID；同名 NPC、死亡实体、零生命玩家、保留的导演镜头 ID 不成为自动候选。
- 玩家近期 HP 净下跌计入热度；附近怪物 HP 下跌或死亡计入“附近交战”。静态低 HP、单纯怪多、治疗、出现/移动和普通掉落不表示正在战斗。
- 热度按捕获时间衰减，半衰期 6 秒；近期 6 秒内的交战才附带低血量和附近活怪的少量权重。分数有上限，玩家/怪物和事件有独立预算。
- 镜头至少停留 8 秒。之后候选同时超过当前分数 30 分及 35% 才切换；平分或两人闲置时保持镜头。死亡或从当帧消失立即换到可用玩家。
- 手选优先；手选玩家暂不可用时临时自动跟随，重新出现后恢复。回到自动选择重新计停留时间，保留仍然有效的近期热度。
- 跨图、录像标识变化、回放时间倒退、超过 15 秒的帧间隔、断线重连均重置状态。
- 普通自动选择不发送 follow、director、camera、移动、攻击或道具指令。只有用户手选会发送原有只读 follow。授权导演和镜头继续由服务器处理。

## 延迟帧的配对

Gateway 正常帧按 `worldSnapshot → spectatorStatus` 顺序发送；手选等控制回执只有 status。网页先暂存 world，再用对应 metadata 选择一次视角，避免先画默认玩家再切镜头。

孤立 status 只在 key 与已配对帧完全一致时用于手选重画，不能重复加热度。key 包括地图、录像标识、sequence 和 capturedAtMs；其他 key、非只读或无效 metadata 会使旧配对缓存失效。事件必须不晚于当前捕获时间、不早于上次已消费捕获时间，并在 4 秒窗口内。当前 AI 目标、目录最新时间和本机 Date.now 不参与战斗评分。

## 已通过的证据

- `node --test scripts/test-spectator-auto-follow.mjs scripts/test-spectator-endpoint.mjs`：**31/31**（24 个新行为、7 个相邻连接/新鲜度用例）。包含合并 AOI 数百怪物在玩家前面、已有缓存后错误 status 等复核发现的边界。
- `node node_modules/typescript/bin/tsc --noEmit --pretty false`、两个浏览器 smoke 的 `node --check` 和 `git diff --check` 通过。
- 隔离 Chrome 的双玩家合成帧浏览器验收 **9 个阶段通过**：稳定闲置、停留、8 秒后切换、同帧手选回执、持续掉血时手选优先、恢复自动、重新选择活跃目标、死亡换镜头、重连重置。读取实际客户端 render player ID，地图与人物就绪；直播采集模式无面板、按钮或自动网络指令。合成截图已目视核对，不冒充真实双玩家交战或平台接收验收。
- 首次合成和真实浏览器访问因本机 3211 服务停止而连接拒绝；恢复托管网页进程后合成测试通过。另一真实浏览器轮次在普通 QA 持续在线窗口结束后看到 stale 而超时。失败记录保留，未当成通过。
- 合成轮次保留两次 `NPC/94/meta.json` 初始 404；后续 metadata API 返回 200，场景精灵就绪。不能宣称该轮零 HTTP 失败。

真实测试服新帧已单独通过：普通自有 QA 在实际 `536b4a5` 网关自然登录、进入比奇，公开 HTTP/WSS 的权限、3秒延迟、私有数组脱敏、连续帧和正常登出通过。隔离浏览器实际地图/人物就绪，真实自动→手选→恢复自动对应正确 render player ID，0 个 HTTP 资源失败及玩家命令。

23 个连续新帧的捕获到网页处理后元数据观察耗时为 **3,129.5–4,640.5ms**，P95 **3,336.5ms**，HTTP 服务端时钟校准的保守上界 **4,672ms**，全部通过5秒门槛。它在就绪画面中包含两次动画帧回调，不是输入到像素或 B 站接收耗时；单个自有 QA 在线，不代表负载或真实双玩家交战验收。

正常退出后两套网关的6项连接/角色/重连租约/登录入口计数和观众全部归零。Playtest PID3099928、原网关 PID3855184、Caddy PID2107428，服务和代理指纹与本轮开始的只读核对一致；本轮未重启/替换远端服务。

证据：[双目标合成浏览器](generated/player-qa/spectator-auto-follow-20261009/browser-fixture.json)、[真实浏览器](generated/player-qa/spectator-auto-follow-20261009/live-browser.json)、[逐帧与时钟校准](generated/player-qa/spectator-auto-follow-20261009/live-latency.json)、[正常 QA 传输与退出](generated/player-qa/spectator-auto-follow-20261009/live-transport-02.json)、[最终健康与排空](generated/player-qa/spectator-auto-follow-20261009/postcheck.json)、[限定验证与源码指纹](generated/player-qa/spectator-auto-follow-20261009/verification.json)。原失败 JSON 和截图来源在同目录 README 列明。

## 当前局限与下一轮

1. 这是已延迟公开帧的网页评分。HP 差值是取样间的净变化，不能证明攻击者、技能、PK、Boss 或掉落稀有度；公开帧尚未携带完整可信信号。
2. Shared Zone 有部分可信信号：已签发的 `OwnedMonsterKillAward.boss_audit` 是 Boss 阳性证据；`OwnedPetPlayerKill` 也覆盖 direct_player 和玩家归属的死亡；`PlayerDamaged` 带实际 HP 结算；部分职业 JourneyEvent 带源对象/目标/伤害/源 Zone/时间。接入需在 `routing.rs::dispatch_zone_outbounds_with_fence_policy` 现有校验通过后白名单投影，不能公开原始账号、session、掉落身份或法律判定回执。
3. 当前 Hub 合并的是每玩家 AOI 投影，以 map_file_name 分桶，存在采样遗漏、5 秒残留、地图/频道/实例混合和事件截断限制。它不是完整全图日志；“从当帧消失”不等于零延迟识别真实离线。下一轮要先隔离完整 Zone，再接可信事件并随同延迟帧投放。
4. 现有暂停回放手选回执可能引用下一帧；网页拒绝把旧 world 配上未来 key。需要继续播放的新 world 才生效，不能算暂停回放即时手选已验收。当前录像关闭，本轮目标为实时公开观战。
5. 服务端3秒公开延迟由前一轮实际修改。本轮读到网关已由其他生命周期修复轮次更新为 `536b4a59f3a8af43f6d4484238e2847573dfb106`；本轮复用其公开接口，不部署旧分支网关或修改远端配置。

B 站继续用本人官方直播姬采集此网页。平台开播与另一设备的画面/声音验收仍由账号持有人完成；本轮未发布直播或测试平台延迟。
