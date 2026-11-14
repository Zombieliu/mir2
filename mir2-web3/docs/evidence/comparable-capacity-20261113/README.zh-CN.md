# Crystal / 当前服务器同负载对照（进行中）

本记录对应用户批准的：先两人校准，再以共同动作测试 25 / 50 / 100 人、
每档 300 秒，另测共同近战场景；找出差距后再验证实际云机。
下面的本机结果不能认证云机或三端实际分发的 CCU。

## 最新已验证结果（2026-11-15，本机 100 人及 fd3 云机 25 人档均未通过，云机上限已恢复）

正式优化 Windows Release（源/集成 `6024d8ff`，PE `c155bb78`）的修复后 100 人共同负载已完整闭合，见[新闭合事实](projection-singleflight-dense100.closed.json)。稳态 16:16:06.015–16:21:06.015 UTC 共 300s，15398/16000 按时完成、71 超时、531 未发送，159/500 逐人分钟窗口通过，整体仍 FAIL。报告中 12 次装备操作、100 人普通 Logout/Close、fresh 0/99 状态保持均成功，最大主连接注销 1181.46ms、Close 24.46ms；controller/driver/server=1/1/0、3611 行资源，双 Job 全员自然退出且无 kill。这些协议观察不等于全套独立生命周期认证：FileV8 按前置门拒绝、sampleCount=0，未读取私密 Store；EnhancedV8 与两个验证包装器均实际退出 1，失败原样保留。114 次新认证完整追加为 V30=1539，旧 1425 前缀逐项不变，失败轮次不能充当已通过的前置轮次。内部热点改善尚不足以让 100 人达标，下一步依据本轮失败分布和资源选择窄诊断；未部署云端或认证三端 CCU。

[本轮报告与资源分析](projection-singleflight-dense100.failure-analysis.json)已实际受限退出 0，Root 以第二实现重算 CPU 与超时分类一致。71 次超时全部是 Walk：owner 晚/peer 准时 30、owner 准时/peer 晚 26、双方晚 15；531 未发为 386 次 previousActionPending、145 次 preconditionFailed。首超时 actor60/t8 发送偏差 4.3491ms，owner 超原截止 95.1924ms、peer 原期限内；这里只引用本 epoch 报告回执，未额外扫描 raw 或还原服务阶段。完整 300s 重叠区间服务平均 1.218218 核、最大约 1s 区间 3.932823 核；injector 平均 0.226872 核；全轮峰 Job Commit 为 2647486464/817881088 字节，不能当 RSS、剩余容量或长尾因果。下一轮只启用同 PE 原有 SLOW20，先新两人校准再同计划 100 人；input/delivery 保持关闭，无新游戏源码或期限变动。

Linux `47e6bb7f` 的两份官方摘要匹配 ZIP 已下载；Root 在新目录以未改审计器实际 0/0、峰 Commit 61624320B、正常无 kill 重核本地原生子集：七门槛、真实 PG/Redis startup=0、clean Release ELF 与 1620 地图 Git/包内容一致。ELF `d966c8c0314673eaf2fb9598c6aae164c40d152bf4a2ba19180064ce3adea1c2`、103858400B；这是与 Windows `6024d8ff` 分开的工件。[本地子集回执](projection-singleflight-linux-artifact.native-subset.json)明确 fullCandidateVerificationPassed=false；完整冻结验证器未运行，仍需当前 fd3 的 fresh 地图/身份实采及新不可变绑定。未把旧 8f 证明换标为 fd3，未操作远端或把本地 100 人失败记通过。

默认关闭的输入/写回阶段诊断已合入维护线 `b950fce0a14f739d89008c864670dcb8f7c8df57`，保留已提交的快照压缩改动；合并生产源码已在 clean b0cc37a3 基线上独立完成 45 项原生检查（selector 3、transport 30、socket 8、gzip 4），4 个验证 Job 均 0/0、全员正常退出无 kill；编译 helper 清理失败与后续验证分开保留。本次独立制品仍来自 clean `525e74364ab87ad49e84c26c8dd22db9fe9348d5`，PE SHA `4c6a18829d3bc50972d2dd84b2682a5f16347ba3dc77b396c9fdc9e2dc16fac5`，不能称为 b950 构建或公网部署。34 项原生检查通过；Cargo 0、包装器 1 及仅对已验证归属的编译 helper 进行强制清理的事实保留。新两人完整 60 秒共同负载 64/64、2/2 逐人窗口、装备/普通退出/fresh/全员 FileV8 与 EnhancedV8 均通过，controller/driver/server 为 0/0/0，双 Job 全员正常退出且无 kill，见[两人闭合投影](input-gate-trace2.closed.json)。新增 12 次认证全部追加，累计 1299 次。阶段日志校准 V1 因误把 delivery 的 ObjectPlayer/ObjectRemove 当作 UserLocation 拒绝，V2 又暴露首条动作在首次真实绑定前缺输入追踪；旧拒绝均保留。V3 16 项 Python 检查及真实闭合日志独立受限重跑均 0/0，证明 `a4d72b83c188cf5415d45f386a601e982719aa156141cca47e1007cc0bd42cb8` 验证全部 64 条原动作/回执，63 条绑定后输入链，首条绑定前输入明确 Unknown。校准只覆盖首次真实绑定后的已观察 Text 范围，完整 registration 捕获=false、物理帧/Binary 完整性未知。新 100 人诊断已闭合，见[严格负载结果](input-gate-trace100.closed.json)：11:43:21–11:48:21 UTC 完整 300 秒，15114/16000 按时完成、139 超时、747 未发送，119/500 逐人分钟窗口通过，整体仍 FAIL。100 人普通 Logout/Close、12 次装备操作及 fresh 0/99 全部通过；controller/driver/server=1/1/0、3069 行资源、双 Job 全员正常退出且无 kill。FileV8 按 SLO 门拒绝，未读取私密 Store；EnhancedV8 退出 1。114 次新认证完整追加，累计 1413，旧 1299 前缀不变。

100 人阶段取证单遍完整核 530696778 字节/1941385 行，实际 0/0、峰 Commit 167313408B、正常无 kill，19449 条追踪未耗尽预算，两名角色均以新 epoch raw 身份独立绑定。actor60/t8 的精确 owner 回包入队至 writer 为 1.918452 秒，其中同连接 Tick 持写门形成 1.915982 秒不可发送屏障；该消息自己的 dequeue→gate 只有 94μs、socket send 20μs，不能误写成它整段都在 read().await。另三条 owner 回包也有同类屏障，见[原始引用与边界](input-gate-tick-gate.causal.json)。peer 回包 2.432 秒的 writer 未选中，具体 Store、CPU/WAN 内因仍未知；完整 shared Tick 的受限原生诊断结果见下段，仍不能据这些局部日志宣布 100 人已修复。

两次完整原生 100-owner Tick 诊断均实际 0/0、全员普通退出且无 kill。细分 V5 的 100 并发 Tick P95 625506μs、最大 630604μs；4 caller 后续 300 Tick P95 27209μs，全部 100 人正常 Logout、最终 presence=0。`shared_session.experience.peer_projection_and_wars` 在 ≥20ms 日志中出现 155 次、最大 572ms，定位到需验证的重复全员投影热点，不能将交叠耗时求和为 CPU。它使用 Memory、新建角色 L1、map0、无 WebSocket，Gateway test package opt0/其余 Release opt3，与真实 File/L40/Dog 及默认 Release 不同，明确不是 CCU 认证，见[原生诊断与限制](shared-tick-native-v5.diagnostic.json)。新修复仅合并重叠扫描，保留 owner fresh 与独立 cold 请求重查语义；其原生对照见下段；修复后正式容量仍未通过，见本节开头。

修复的同条件原生 V6（制品源 ebc5dd90，生产变更 b17e30ab） 已实际 0/0、全员正常退出且无 kill：同 100 actor / 400 Tick / 12 Turn，100 并发 Tick P95 由 V5 的 625506μs 降至 144636μs（约 76.88%），最大 146040μs；后续 4 caller / 300 Tick P95 由 27209μs 降至 14027μs，但最大值由 78470μs 增至 207205μs，不能声称所有耗时均下降。100 次普通 Logout、presence=0；初始化 186591ms、整轮 206.234s，仍按原 240s 诊断预算。见[完整对照与限制](shared-tick-native-v6.diagnostic.json)。这不是默认 Release / File / SDK 或云机 CCU 通过。新专项实际 9 项加相邻 18 项通过；首次正例 7/8 的失败已独立定位为首次 owner 发布推进代次，随后以确定性同步分别覆盖同代复用与变代重扫，生产校验未放宽。负例已进一步以真实 native Tick 固定并显式断言同代前提；最终 fresh f6511a5d 的 27 项 Gateway、1 项 Simulation 逐轴权威身份和 1 项真实 replica 提升集成测试全部通过，6 个 native Job 均 0/0、全员正常无 kill，见[29 项实测与编译清理事实](shared-experience-projection.native.json)。代码已合入维护线 `6024d8ffc095dd3a149ddb2b12bd3fce4cc4245c` 并通过统一代码线检查。正式优化 Windows Release 的构建与校准结果见下。Linux 额外普通入口 [43607145500](https://github.com/Zombieliu/mir2/actions/runs/43607145500) 的 shared_pet_pvp 为 32 通过/6 失败，未产生合格 Gateway 包；相关 Zone/Sim 测试树与 fd3 相同，只读发现既有夹具安全区/飞行时序前提冲突，未重跑证明基线、未修改断言或将六项记通过。随后冻结分支入口 43612459430 的配套 source 检查按原规则拒绝非维护分支，剩余过时构建已请求取消。正确的原七门槛入口已由维护线 [43613343285](https://github.com/Zombieliu/mir2/actions/runs/43613343285) 完成，整体与 native job 已 success、严格工件下载复核仍待完成，源码 `47e6bb7f` 的 Gateway/Simulation/protocol/打包脚本与 Windows 所测 6024d8ff 相同；CI 源版本仍单独记录，R2 发布关闭，未更新云端服务。见[保留的 CI 尝试事实](projection-singleflight-ci-attempts.json)。 正式默认优化 Windows 版本已构建并冻结（PE `c155bb78`、源 6024d8ff），新两人完整 60s 校准 64/64、2 个窗口、装备/普通退出/fresh 2/全员 File 与 Enhanced 均通过，controller/driver/server=0/0/0、95 行资源、双 Job 全员正常无 kill，见[校准闭合事实](projection-singleflight-calibration2.closed.json)。12 次新认证完整追加为 1425、旧 1413 前缀不变；同程序原计划 100 人正式复测按原 30/900000ms 预算完成，结果见本节开头，未通过。

此前一次容量实测的生产逻辑集成为 `fd3fcedc8b8a4d0b6bb16f6c45cd57a77364c8b5`。新 Windows 游戏服 SHA `e3409b70c3e1661a8bee7f3432aaab9816f92f26283fc3fed72fbe121aad8e38`，99613696 字节，clean fd3 源构建 Cargo 0，证明 `10f16844a1ea6a8bd83fa03742b02a8585965cc84d25fda380feacdd1dc9d138`；包装器因本次 VCTIP 超过 180 秒普通收尾期限保持退出 1，只在创建时间/路径/哈希/Job 归属及 held handle 重核后清理该 helper，最终全员退出。`compileCleanupWasForced=true`，不能称整个构建正常/noKill；闭合事实 `ca632f637a14dedb63713bf54d015185907589ec1942d199abc53a9d7a51a1ef`。独立功能两人 64/64、近战两人 104/104 及各自 2/2 窗口、真实伤害、装备/普通退出/fresh/全员 FileV8 和 EnhancedV8 均实际通过，两轮 controller/driver/server 都是 0/0/0、Game Job 全员正常退出且无 kill。报告分别 `f61e057bb042e641f29c13d4f07284443410d8594f5899c3eddd4d1b2f40159d` / `5b3f0eaf85785941aea080a5e52f2851f9948f3222fc102dbc423d27a9d9173a`；增强证明 `d3c39dc5f0468290d71960434152a270749c7629f823a1f761a573a7501dd1b8` / `0155a0e2341b6f4f603c7ea30e00c3c2942113645ef32fda0be04adb1e39888a`。新增 24 次认证完整追加到 V25=1173，SHA `ad1d00e20b3e89d3f8008fe439d1c67fff1b5f8f21edc6a29787b9055e6d1ed6`，旧 1149 前缀不变。

同计划、关闭录盘且不启用 slow/trace 的 fd3 本机 100 人现已完整闭合：15157/16000 按时完成、123 超时、720 未发送，113/500 逐人分钟窗口通过，整体仍 FAIL。100 人全部普通 Logout/Close、12 次穿脱操作与 fresh 0/99 通过，controller/driver/server 实际 1/1/0；3651 行资源、两个 Game Job 全员正常退出且无 kill。File V8 按 SLO 前置门拒绝、sampleCount=0；增强 V8 实际退出 1，不能写成全员磁盘回读或容量通过。小份[闭合结果](mentor-finish-100.closed.json)固定原报告 `ec685f35f84b07863c66683442b951296e6c80401ae1544d5ec2f087dd7f26d1`、controller `45d4c029da997756136be2dbacbe567f0f47658df053977b0f5545013df297c6`、增强复核 `8b3f362a2183b2d35b3b203a396ff042bfd9e63fe314f39b8f79eddef61408a3`。新增 114 次认证全部并入 V26=1287，SHA `d224cf06211f0a425a5020f5459968b8e031f97d38a712e04f0d2899fdb36baf`，旧 1173 前缀逐项不变。这次导师查询修复未解决主要 100 人延迟差距。

同一 UTC 重叠区间公式复核，fd3 本机 300s 服务端平均 1.312036 核，区间 P95/最大为 3.411306/3.902454；全轮峰 Job Commit 2502.847656MiB，明确不是 RSS/RAM。injector 平均 0.223341 核、峰 Commit 798.484375MiB。旧 8f 的 1.258587357386147 核及原版 1.5215163241495104 核均由同公式重算吻合；资源结果 `ae666b9fc68836da03080bbd3d5371a60a15e16db2208a05baa85b2a21a7a4c0`。失败时的平均 CPU 不能认证容量余量。

首三个超时 Walk（actor53/60/61）已按精确 connection、ticket 与 raw receiveSeq/floor 单次流读核对：发送偏差 1.719/2.097/7.081ms，owner 和 peer 权威回包都晚于原 2s 截止；同 socket 的 KeepAlive 回声仍为 0.306/0.159/2.245ms，收流洞期间其他 68–97 个连接仍持续收包。取证 FREEZE `66c04ed83ea741e6552330f12010b3a1bdf93e936d9595209d0038ebb66bbd78`，537675246 字节完整 raw SHA 匹配，受限离线 Job 实际 0、正常闭合无 kill。证据不支持覆盖该整段的全 Node 连续停收，也不能直接认定某把服务端锁；本轮没有可绑动作的 slow/trace，下一步只补默认关闭的逐连接阶段诊断。

Linux CI `43564709820` 的 fd3/head 全工作流与 native job `132355893186` 均已 success；两份工件下载均匹配官方摘要。严格 V10 全验实际 0/包装器 0、Job 正常闭合无 kill，7 项原生门禁、真实 PG/Redis startup 和全部 1620 地图 Git/工件/当前云端内容一致，回执 `353ba55807f81162e19fa9faaf702fadcd8724359a5f46660cf7b1e62db17365`。新 Linux ELF 为 `11fc151644ecb75ccb5a86789cefb2010a062ef48c3a9ddfd5ccf29b67dfe926`、103744856 字节；这是工件通过，尚未部署或通过容量。验证器仅将已核对源的 CRLF 统一为 LF，完整 AST 不变，使用新输出重新全验后得到固定源 SHA `959069beff2135e475f33567c37f9a8b0371fa4ce241c89ce8c6cfd316c91e89`，工具集成 `f6fb7da603`；旧源与旧回执未覆盖。下述地图读取时云机仍为 8f/EDD/PID1193390/51-66-51；现已按后述 V11 流程更新。新的 fresh 同 epoch status（公共 SHA `b7b16e4c7b4e8b89cbc2ca59d90c93c004682c2e76568daf947c0941b9f38106`）后，V6 对当前 8f 的全 1620 地图只读实采实际退出 0，证明 `00ca2f5cd66bb95a62ff7ea6bf7e522654194dba9378783ee40b7df7c07124d6`：42111309 字节、完整文件/内容摘要逐项核对、40 配置/8 保护进程前后不变；不能将该只读证明作可变操作授权。

V11 后续控制工具集成 `aa4b316004`，源 SHA `046f8a2fe0e030ab727a54d8bce8e936e0d4f210db84cbec3b83d6955573098e`。raw-parent 最后 signal/unlink 与 partial cleanup 补强后，V2 完整 72 项及 Root 新副本重跑通过；独立 peer `ee259e8323e4288e5916cf21960323510b8a8ba529c1353707feab6e6597ed8e` 无阻断，实际工件 17 常量绑定的 6 项检查另过。严格读取旧 4004 安装状态与恢复后 8b/c489 trace 终态作为历史锚，使用新 private/release-scene-v8 状态，未改写旧 PID/状态。实际 install 退出 0（`f531086b672b2c9ad740106d2fbd4d8a99c24949d6e3b9cc4a5ffda2feac3ca7`），新 TEST 为 fd3/11fc，baseline21 EnvFile/20 drop-in；fresh 九零后临时上限 120/150/120 实际生效（`0f76ad9c8ebbd7d0342af040c915738b40bb9507ba0d62c88623d7caf232c151`），22/21、trace unset、旧 40 文件及 8 保护进程前后不变。测量 epoch 为 PID1232758 / start1553418134 / invocation c541c068817546b88a18a2e107b692ee。原池 P3 before 实际 0（`a6830597f94c372d87d72e4a39c3207508d9a51bf4500b873132eeb719f855bd`）及第一份同 epoch 健康主监控后才启动原 2/25/50/100 ×300s 公共 movement/chat 负载；该轮现已闭合，详见[云测闭合结果](cloud-fd3-progressive.closed.json)：2 人完整 300.846s，五个完整分钟共 320/320、10/10 逐人窗口通过（含边界的 observed span 为 322/322，另列）；25 人仅运行 61.235s，首完整分钟 796/800、3 超时/1 未发送、21/25 逐人窗口通过，严格 SLO 失败即停止，未进 50/100。4 轮/8 操作穿脱通过，25 次真实业务 LogOutSuccess 均在原 5s 内，但 actor4/20/23 随后的独立 Close 握手超时；其 WebSocket 只在记下失败后 terminate，不能冒充全员正常关闭。OS Job 实际退出 1、全员退出且未杀进程；P3 after 实际 0（`bc63ca486dade3d287ec0648e452f9db094da2fbbd3c484d34b3f9eaa2a21be8`），最终 fresh-save 未执行。主监控 424 samples/completed/exit0，关闭时 stderr 的 socket10038 保留；TCP 424 samples、exit1、不完整/remote exit null 亦保留。固定 fd3 summary `b428dd5d431fac355a9d91482cdacacfa443e77f923291070270eba5c39ac246` 经原28加3项及 Root31项回归通过，实际闭合汇总 0/0、SHA `7faa01f35a9589d1f6498938d7bc927814a76f6bc9c8a01e32c39b49287e8f9f`，容量标志仍 false；首轮因缺 inputs.json 拒绝的结果保留，公共 inputs 是闭合后从真实预跑绑定明确投影，没有伪写预跑文件。

随后 fresh 九零后，V11 恢复临时上限实际退出 0，回执 `427c13382244218f622dcf89b670cbafca1e73ad8cc564e99dc873d0c470d2b9`：现为 fd3/11fc、51/66/51、21 EnvFile/20 drop-in，PID1242617 / start1553569321 / invocation54b0b00891c344a58ab8e5a6d05ddb5b；健康、九零、trace unset，40 个原保护文件及 8 个保护进程前后精确一致。云端首失败取证 FREEZE `2790dbf79e6e86507cc155e39a339dec07ee0c914c0e0b54925e47bcded7e2e3` 单流 11140377 字节/61133 行、实际 0/0：三个超时均在 legacy owner ACK 等待，但一例旧 pending marker 可指向先前动作，不能写成三个唯一精确服务端延迟；三例 Close-only 失败则都有唯一 LogOutSuccess 引用。VPN 和非 TEST 服务未动。

本轮 V11 私有闭合证据归档 615 文件、1400844063 字节，manifest `95b9c47130f96644681c89e4203e5192669ec3b393a60b69c5f663c500779e4e`。旧十份 manifest、源文件及保护 DACL 未变，所有副本逐项哈希读回；明确排除 private、账号池/账本/认证历史、存档、工作树、缓存及尚在进行的输入阶段诊断。归档不提升容量结论。

修复版后端源码为 `7403bc5ee5cbb472477db101b8be603136049dcf`，Windows Release SHA `95614fad36fa8228c2aeabfb0f7b1bf29c6fdde478f915c1a53134cefedf4008`。干净快照构建的 Cargo 实际退出 0；外层构建 Job 因 MSVC 后台子进程未在收尾期退出而失败，原失败保留。性能运行由新的独立 Job 另行完整验证，不把构建 Job 写成正常退出。

新 SDK7 两人 dense 校准：64/64、2/2 窗口、装备/正常退出/fresh/全员 File 与 v7 独立复核全部通过，controller/driver/server 均 0。新两人近战：104/104、2/2 窗口，原始回包独立重建为 56 次有效伤害与 44 次明确零伤害阻挡，双方普通退出后朝向分别 Right→Right、Left→Left；全员 File 与 gameplay verifier V8 都实际退出 0。File 回执 SHA `c560c94aaa145d714bbb91f56e2b6438a2363a186573a19db5059c550b97cc95`，增强复核 `94f279bf84c99a4afa4727d42f9f39ac1a16fa9b4e7b54d337bdf01940304fb5`。V7 漏读真实 `damageType` 字段的拒绝保留；V8 仅补三种数值别名严格一致校验，100 项离线测试通过，其他伤害、HP、原始引用及 SLO 门不变。旧 SDK6 近战仍因 fresh 朝向/正常注销证据失败。

同一修复版 100 人共同 dense 计划已完整结束：1047/16000 按时完成，4713 超时、10240 未发送，0/500 逐人分钟窗口通过。controller/driver/server 分别退出 1/1/0，3751 行资源证据完整，正常停止且没有 kill；全部 100 人普通 Logout/Close 和 fresh 0/99 通过，但整体仍 FAIL。File V8 因 SLO 前置门拒绝，未读取 private store；增强 V8 实际退出 1。不能将生命周期通过写成 100 人容量通过，也不能沿用旧版本 25/50 人结果认证本修复版。报告 SHA `bbbfa86f878254357d363af4ea7bd76c2062f88af833ece7bbfa630fae341929`，File/增强结果分别 `daa7a3ec0823a138105560b82e9bcb847ae10223113dd2424625f30fdb7fb2e1` / `3a2ad3979b6c964cec39c043ee0d9c2d08c35e90a4e86e7acde310476997369c`。新增认证全部并入原历史，累计 669 次，未重置 30/900s 配额。

该轮完整 300 秒内 server 平均 1.627782 核、全轮峰 Commit 2427.949MiB；injector 平均 0.123107 核、峰 294.137MiB。大量未发送使较低 CPU 不能代表吞吐改善。首个 Walk 的 owner/peer 正确回包分别晚至 3757.494/6114.347ms，发送偏差只有 3.25ms；该 actor 没有更早的 Walk/Run，也未在此动作闭合前发送第二个 Walk，接收和 KeepAlive 持续。证据支持动作发出后出现多秒延迟，尚不能排他归因到某一把锁或网络。资源汇总 SHA `6d2f6d2ea06b95f862285b28300389af63cb1c4ae4aff279b9ac8fa827fbaf77`，首超时取证 `143a19335e8eec93ba332430d10072044edc1e4d1ad1a8d4cf37adc3abd811d9`。

Linux CI `43491883414` 全流程成功，新 ELF SHA `26c131881c6c1e9de4ca2179d9f7a05f97d07f7e29269c80dca3dcd04cae1082`；七项门禁、真实 PG/Redis startup 与全部 1620 张地图通过，严格 artifact V8 回执 `46827f7441affa122213faef6c1b7f7a7030483daaeaf32652271addb8a47390`。这是另一份 Linux 工件。云机同池准备 V5 已结束：65 人本轮 fresh 加 35 人继承已验证准备，共覆盖 100 人；本轮 65 人全部普通注销，账号池和账本前后校验通过，不能标成全 100 人本轮 fresh 或容量通过。报告 SHA `0f45c358b229861cedfc2bed0da8d2fded21afcf39eeb8a7678a8840c7de3c21`。

V8 更新工具在发送 SSH 命令前因完整请求超过 98304 字节限制拒绝 install；失败保留，实际只读复核确认旧服务及全部受保护基线未变。V9 只修正完整请求的有界压缩传输，wire 上限不变，解压限 131072 字节，全部审计字段保留；120 项离线测试通过，规范目录补测 34 项通过。仅将本地 V8 的未安装 prepared 计划原字节归档，原 V5 installed 状态不动；使用新鲜 status 重新 prepare/install V9 通过。实际同一 TEST 已运行 7403 / ELF 26c，install 回执 SHA `3e28434f9fd302ed8a7ac9ca5f5c9ea9534ecaaa2239259d0d05cae1d7006365`，19 个 EnvFile、原上限 51/66/51，受保护进程指纹均未变。第一轮临时测试上限 120/150/120 已在普通收尾后恢复 51/66/51，restore 回执 `cd4a9ff9a2dbba3a6652a3e6effca1786482fa85f3189e817959bc55d0c6b602`。第二轮临时上限也已恢复，结果及回执见下文。

云机实际配置与本机不同（云机 recording 关闭、测试服务 MemoryMax 1600MiB，且共享主机），故另做 2/25/50/100、每档 300 秒的实际测量；首个安全或 SLO 失败即停止升级。该公共 movement/chat 计划与本机 common dense 分开报告，不据此宣称本地 100 人已修好、完整战斗容量或真实三端渲染通过。

第一轮实际云测已结束，报告 `ed10b6a4096de8ee55fdb0ec17c8a05809e37155fce47040ef3f002c6332e240`：2 人与 25 人各完整活动 300 秒通过，评分动作 320/320、4000/4000，10/125 个逐人分钟窗口通过且阶段内零超时；25 人阶段 target CPU 平均约 0.40 核、采样峰 cgroup 内存约 475MiB，游戏 WS 应用下行约 0.326Mbps。主机 NIC 同期约 1.399Mbps，包含其他服务，不能当成游戏流量。进入 50 人档时、30 人已入场后，既有玩家的 continuous minute 超时导致整轮 FAIL，未完成 50/100 稳态和最终 fresh-save。31 个 cleanup normal 标记包含 30 个真实已入场连接及一个未建立连接的 no-op，不冒充 31 次业务 LogOut。六个穿脱回合通过，owned Job 全成员退出且无 kill，同池前后校验通过。独立闭合汇总 V3 实际退出 0、容量标记保持 false，回执 SHA `421536305224d083d96c2a15cd21c2b30dacd7d5048f7a98b3b1a4c9f3cc8df0`。V1 监控字段拒绝、V2 漏计 admission 超时的旧结果均保留；V3 区分已完成阶段零 timeout 与全日志四次 timeout，19 项离线检查通过。

四次 timeout 的共同见证者为 peer20/plain-Web。四步 owner ACK 均按时，但 peer20 的原目标包分别在发出约 2.35–2.38 秒后才收到；该连接 2313.10ms 没有任何 received 或 KeepAlive echo，同期其他 29 个角色收到 1284 包、36 个 echo。首步其他观察者约 63ms 收到同一个移动；实际位置在 Zone AOI 内，没有对应 ObjectRemove。可确认单连接应用接收空档，尚不能定位 Gateway writer、代理/TCP 或本机单连接处理。证据 SHA `7129efcaf9ada83525a122e4a204cf6f6d24fa2a09a715b45988b52bfc6cdfbe`；不能把约 30 人解释为硬件上限。第二轮同一 load/probe、频率、截止时间、评分和原账号/账本的结果见下文；只读 TCP 统计另行补采，不为旧两轮补造覆盖。

第二轮云机测试已经结束，整体 FAIL：2 人 300 秒通过；25 人阶段开始时发现 26 个 WS / 25 个 active，约 0.214ms 后因原人口门槛停止。此前新角色 warmup 已发出 23 次动作，全日志 546/546 outcome 完成且零动作超时，不能称为全轮未发动作。全部 25 人收到普通 LogOutSuccess，但其中 9 人未在原 5 秒内收到正常 Close，随后本地强制清理不算正常关闭证明。额外 WS 的归属及计数原因未确定，没有停止未知客户端或放宽人口门槛。独立闭合汇总 V5 实际退出 0，第一轮 SHA `e16485202a53099b26a77d5ebd06c756330cc68e292086d6f4d1507a512c6e1d`，第二轮 `dcdb2c96cafe820ba2adf341a5ac5e3bfb7bc7a39fafcc6ccd76070b022311d5`；汇总成功不改变负载 FAIL，全部容量认证标志仍为 false。两轮临时上限均已恢复 51/66/51，第二轮 restore 为 `restore-caps-20261113T223148.133381Z.json`，19 个 EnvFile、受保护进程指纹不变。

本机 100 人关闭录盘对照已完整结束：同一 7403 Windows exe、SDK7、a2caf 300 秒计划、4CPU/8GiB 和全部原门槛，仅增加 `MIR2_SPECTATOR_RECORDING_ENABLED=false`，spectator 保持开启。15172/16000 按时完成，121 超时、707 未发送，109/500 逐人分钟窗口通过，仍为 FAIL。全部 100 人正常 Logout/Close、fresh 0/99 和 12 次穿脱后的重登通过；controller/driver/server 分别退出 1/1/0，普通收尾且没有 kill。File V8 因 SLO 前置门拒绝，未打开 private store；增强 V8 实际退出 1，不能宣称全员磁盘重读通过。报告 SHA `06d9f15549b108ed146b42724569654e61b904121c40d1fa1b9c62c2e5294c6b`，增强结果 `23f0d978bc278902e26c1bf70eaf934e9294150238596460a0b24b4fb15fcd46`。认证历史完整追加至 783 次，没有重置额度。

关闭录盘轮完整 300 秒内 server 平均 1.186168 核、全轮峰 Commit 2447.797MiB，injector 平均 0.206284 核；资源汇总 SHA `3fcdd323a0b2b022fed1c4d31128257f1df269017e0aa17d74199ef7654e2851`。首个已发超时 Chat 的 owner 为 57.255ms、正确 peer 包为 2560.295ms；peer 曾有 1264.034ms 的所有接收空档，但该空档内没有 KeepAlive 发送，不能从空档前后的快速 echo 推断空档期间网络正常。取证 SHA `afc8f819155da4cb01618fbecd219dc5062ebea87cf32205cf7893a5a8eec1e6`。关闭录盘后的明显改善是受限对照结果；硬限 CPU 并非独占物理核心，其他时间差异仍在，不能把全部差距只归因于录盘。

新增本机 100 人慢阶段诊断已正常结束，在关闭录盘配置上仅增加 `MIR2_GATEWAY_SLOW_STAGE_MS=20`：15164/16000 按时完成、121 超时、715 未发送，104/500 逐人分钟窗口通过，仍 FAIL。完整测量区间为 2026-11-14 00:29:34.201–00:34:34.201 UTC；全部 100 人普通 Logout/Close、fresh 0/99 通过，controller/driver/server 为 1/1/0，3076 行资源证据完整、两 Job 全员退出且无 kill。报告 SHA `e4fb014fdf288a208d302551f9d985256a36513f7267105b4222dd8b67d1728d`，原始事件 `672c9b217b93cab988d07a6d5503af6cdc63e1bb01fd98761d4db64db26813c4`。File V8 的负向前置门验证退出 1，未打开 private store；增强 V8 退出 1，独立计数一致，结果 `a68548e26bce49e13c0d2b85263abd133d1dcde3420e6247c8794233a3157d3e`。114 次新增认证全部保留，累计 897，旧 783 条前缀未变。

诊断 server 平均 1.236493 核、全轮峰 Job Commit 2417.270MiB，资源汇总 SHA `7511816f07ffb9ee8331a2b585d832e109a45196597c707650799b2816c76a4f`。两次只读解析器缺项失败保留；V3 实际退出 0，完整读取原日志 29180169 字节、363476 条慢阶段记录。补读时间不当作源事件时间，日志自身的开销使本轮保持诊断用途。源时间投影可证 AccountStore 的 experience-admission 等待最大 1311ms、guild lookup 等待最大 980ms；它们不带 actor 绑定，交叠不等于该动作因果。读取时间邻域内另有真实已完成的串行门等待 3219ms、runtime Tick 3211ms、predrain 3005ms；该表不是所有调用的 P95，也不把嵌套 elapsed 相加为 CPU。相关性分析实际退出 0，原始阶段字节和哈希逐行复核。

首个 actor56/ticket8 的真实 owner/peer 回包分别为 3225.107/2765.475ms，发送偏差仅 6.219ms。服务器自身的 queue 记录表明 move 曾排在 runtime Tick 后，等待 1736ms/1443ms；SDK connection 与服务器诊断 connection 独立计数，未把同号或耗时接近当作已证跨日志绑定。两轮关闭录盘测试各有 121 次超时，但只有 30 个动作身份相同，不支持将全部失败解释为固定夹具。首超时补充证据 SHA `d03c6f5d7c06baf8d9fbcb9d29042575245717cdb440d9d9b4b47d4a04575230`。

最小修复已集成 `63da6bd0d7fe34d582e9678698ac32904208e64b`：只将 forced checkpoint 不使用的 mentor/guild 资格查询移入原非 forced 分支，writable、完整 checkpoint、RNG、持久化及回滚下半段原字节不变。旧实现持有无关 AccountStore 时的真实 RED 用例退出 101；新实现八组独立 native Job 共 54 个去重测试通过、5 个原性能剖析忽略，全部测试 Job 正常退出且没有 kill。稳定源/测试冻结 SHA `41af3bea891cd30836c474e83e0e2eba9df847d47091b6f9309df1fda1e7b75d`，GREEN 证明 `ec066212bdbfba8280cb4385950e40fc67b16ec478b2aa4ca3bf25c2f1e11151`。这证明窄修复的功能与锁依赖变化，不代表 100 人 SLO 已通过。

测试程序编译的 Cargo 实际退出 0，构建包装器因 MSVC 遥测子进程超过 180 秒普通收尾期限而保留失败 1。普通 CloseMainWindow 返回 false 后，只对本次 Job 内、创建时间/路径/哈希均复核一致的 VCTIP 进程作定点清理；`helperTerminated=true/compileCleanupWasForced=true`，最终 Job 清零，不能把 `killAppliedByClose=false` 误写成整个构建没有强制清理。独立测试 Job 的 54 项结果与此分列。清理终态 SHA `3b5f9e77d2a45dde341078360c439e1060c66123ab567ec47b8499ce36360a64`，未更改全局遥测、更新、电源、VPN 或其他进程。

发布候选源码为文档子提交 `8f3099f4a0114a7825ff61db36b548d3bfb5b3fa`，与 63da 的编译输入相同，不能将二者写成同一提交。干净 C 快照通过统一主线检查，受影响的两份源文件与实际 GREEN 字节相同。新 Windows Release 的 Cargo 实际退出 0，PE SHA `e5d4eb392086da0da0f56e2fa7c646cce1ace370b6c9db9c99415675bfcec119`、99614720 字节，构建证明 SHA `25409f4cae24ad11b5329e7c886ac32506e8f864118cbfe808d8a0b22de1215d`。其包装器仍因同类遥测残留保留期限失败 1；仅清理已核对身份的本次 helper，最终 Job 清零，强制清理标记与程序构建结果分列。

该 Windows 程序的共同两人校准实际通过 64/64、2/2 窗口，普通退出/fresh/全员 File 重读及增强 V8 均通过；报告 SHA `76b09233ba389891fbd1b8c8ce9cde57629dd2b23376ebcd6a6413fdaf1961eb`，File `087bf585eef52e1e358b1435d00db6521bb35b59d2afcffde7f5f7629a2adc92`，增强 `89226a9dc434f6ceec40f2e321bceb47dc56e7ce1bd449a8fff5006abec6a00c`。共同两人近战也实际通过 104/104、2/2 窗口及同样的退出/重登/磁盘验证，报告 `4ff6af89ef26bae1b4a09b318f6b8e0f7c8af02a24ad5cb21e7fa8f260144927`、File `8377285ee001e06e029a79ab78802d06010853d412992429171bef41ce8a7334`、增强 `62e2c45b4e69e00e1ac8da3e0efe67223851a7f656946c27cf10ebd2a2dd469a`。两次 controller/driver/server 均 0，两 Game Job 全员正常退出且无 kill；新增 24 次认证完整追加到历史 921。新 100 人同计划、录盘关闭/观战开启/无 slow20 已于 03:08 UTC 正常收尾：完整 300 秒（03:01:39.926–03:06:39.926 UTC），15207/16000 按时完成、112 超时、681 未发送，107/500 逐人分钟窗口通过，整体 FAIL。报告 SHA `6ff4329510673539def81a147802795c867e05ddddff721cda3a32de4a916db3`，controller/driver/server 为 1/1/0，3411 行服务端资源、两 Job 全成员正常退出且无 kill。全部 100 人普通 Logout/Close 与 fresh 0/99 通过；File V8 因公共 SLO 前置门拒绝、未打开 private store（回执 `daa7a3ec0823a138105560b82e9bcb847ae10223113dd2424625f30fdb7fb2e1`），增强 V8 独立复核实际退出 1，SHA `d6fb6d5ed9fe07d5a70f44d3b003152af3d5dcf2ca6b838adbc7c37051493e86`。本轮全部 114 次认证追加到原 921 前缀，累计 1035，历史 SHA `81a1ea351f7d3cfad7a31b46b594a9501441175f6324f08d98572d7ef0de420a`，未重置配额。结果与旧 OFF100 的 15172/121/109 窗口接近，窄资格查询修复未解决主要性能差距。

同一 8f Windows 程序的第二轮 100 人诊断也已正常关闭：仍为 4CPU/8GiB、同一 SDK7/a2caf 计划、关闭录盘且保持观战，只增加 slow20 和两个接收者的 delivery trace。完整活动区间为 04:40:04.554–04:45:04.554 UTC，15482/16000 按时完成、54 超时、464 未发送，169/500 逐人分钟窗口通过，整体仍 FAIL。报告 SHA `811fab9a9af86633b8ba9ba6bf94993e5ec4363caf84d0bc2abf190e296fe246`；controller/driver/server 为 1/1/0，3070 行服务资源，两 Job 全员正常退出且无 kill，全部 100 人普通 Logout/Close 与 fresh 0/99 通过。controller SHA `c5b2805b6f90bc26f546f8e58129d48e89f3f93139ac9b758f2b88aa19cece30`。114 次认证完整追加，1035 条旧前缀不变，V23 共 1149 条、SHA `2dfd201dc13453e824768831adef362e447ded9f104af679ee7b7f98ad97960c`。日志会影响排程，这轮只用于诊断，不能把次数变化写成新修复或容量通过。

闭合后的原 File/增强 V8 均实际退出 1，File 的公共 SLO 前置门拒绝且 sampleCount=0，不构成全员磁盘重读证明；增强结果 SHA `33e375acb48371d9848c29bb39d48f3b96d1cc85f2606e79bbf80f39de9b6f0a`。有界投影实际 0，完整读取 33693048 字节；离线分析 V1 因 SDK7 actionSent 没有顶层 connection 而拒绝，原失败保留。V2 只从唯一 owner receive floor 取 connection、保留精确 raw send-time 关联并拒绝显式冲突，16 项回归及 runner 5 项通过；实际分析退出 0，SHA `921548265d017769ecc78e8fb99eb5ccc6ccf5921272df1b6bbe4169e919bdc1`，五个离线 Job 均正常退出且无 kill。两个已独立绑定接收者 12/13 本轮都没有超时，它们的队列最大 631.498/577.728ms、gate 最大 35.723/37.046ms；不能将其称为这轮 54 次超时的对应角色。全局确实记录到出站 read_gate 最大 2723.877ms、experience_admission StoreWait 最大 1313.574ms，但尚无对应超时 actor 的 registration 绑定。guild_lookup 的 738.145ms 等待只指 shared_guild_for_identity，不能冒称 finish mentor 查询。关键投影 SHA `078a7ebebf261183aed0038f9da01b325fa48d5a91984bb1201e9340be6becfe`，闭合清单 `022b1885a315f172f09fcd756bafdbe0e3f30c2e8da54668e798ea30357171d0`。

新一处窄修复已集成 `fd3fcedc8b8a4d0b6bb16f6c45cd57a77364c8b5`：仅在 finish 的原导师子句前，直接读取保存构建器相同的当前 XP/selected-character level；两者明确未变时不再等待结果必为 false 的导师资料查询，身份缺失/不确定时仍执行原式。eager Stage5 JSON 解析、其他 force-save 条件、has_uncommitted、RNG、保存/CAS/回滚保持原字节；没有扩大 journal 空判断或缓存伙伴身份。真实旧代码持锁 RED 在 5 秒兜底释放后退出 101，包、完整 owner save/revision、journal 与 durable 状态的断言先全部通过，只在“应在显式释放前返回”的最终断言失败，证明 SHA `e10b05b208e866f3b88f239a5da0c82e77f7d8d7d171836124b721b842a6468d`。修复版同一用例、XP-only/selected-level-only 的新导师关系与源失败回滚、非法 before JSON、身份不确定及相邻 NPC/mentor/pet 回归共 59 个去重用例通过，5 个原 ignored profiles 不计通过；7 个 native Job 全员正常退出且无 kill，GREEN SHA `c3f98f9a207d790428ac2d76457d2e527c5f667d40f469eae62c306bad4be793`。RED/GREEN test 构建也均 Cargo/包装器 0、完整正常收尾，无本次 helper 强制清理。Native 实验使用 canonical c40 的干净快照加两份固定源码，随后集成的两份字节与 GREEN 完全一致；再切到干净 fd3 快照构建游戏服。此时新 Windows 工件及 Linux CI `43564709820` 仍在生成，尚未跑新 100 人，不能把单查询依赖移除写成现场主因或容量修复。

三案相同资源公式复核退出 0，SHA `2fa909a9347979b8f2741511c8394770e329d307248614d9bc321316c9b566fc`：原版 100 平均 1.521516 核、全程采样峰 Commit 3473.324MiB；旧 7403 OFF100 为 1.186168 核/2447.797MiB；新 8f100 为 1.258587 核/2528.039MiB、重叠采样间隔 CPU P95 3.267875 核。峰 Commit 不是稳态独占 RAM，失败/未发送的较低 CPU 不能代表吞吐余量。应用下行分别约 0.878593/4.931909/4.938494Mbps；原版包含 4 字节原生帧头且为计数读窗，现代为精确 epoch JSON payload 窗，不能当作同编码线速或云机带宽合同。

已直接使用同一 GREEN native exe `2957e857e6f991f62b44704cdd6e532f016e2f3c02572e30f22bf385502cba71` 跑原有 ignored Crystal Tick profiler，实际退出 0、66.607 秒、独立 4 核/8GiB Job 全成员正常退出且无 kill。4 个 owner、256 次 memory-store Tick 的 outer P95 540.9μs、最大 1798.2μs，checkpoint begin/finish P95 241/94.2μs；64 次实际 wrapper 基线 P95 444μs。它没有 Gateway、File/PG 或并发持锁者，不能排除现场锁等待，也不能认证 CCU，只说明这份无争用叶节点计算本身未复现秒级停顿。另用同一 exe 对当前 `platinum_fiveclass` 内容配置直接执行对应 profiler，实际 0/0、48.475 秒、独立 Job 全成员正常退出且无 kill；仍为 4 owner/256 次 memory Tick，outer P95 497.6μs、最大 1929.9μs，实际 wrapper P95 518μs，计算与现场并发等待的边界相同。

Linux CI `43539916415` 的同一 8f/head、canonical branch 全工作流已 success，明确 `playtest_wallet_no_docker=true/publish_r2=false/numeron_operation=off`。新 ELF SHA `edd9f48cdb36ddd96334598aca4abc2f2e212e46ef2cbb60d4a1f5ab15a7ded5`、103748080 字节；独立 V9 实际校验退出 0，证明 SHA `6990c78333a0e28369fd701276c981e4611bd93b5027c1adbe808716ed46ce7d`，七项 native 门、真实 PG/Redis startup、源码/包及全 1620 地图匹配通过。更新前 7403 测试服的全量地图只读 V5 实采也退出 0，证明 `b44d5f3c250b508f8005f4157c35af9516bbf767abcba5c77722dc740083761e`；V4 漏投影 systemd 限制导致的实际拒绝 `8fff224d5ec280cce009e4b857dd8defbd3391350d7d56e5a23bf7483b7bec63` 保留，未改写或重标旧 82 证明。

02:44 UTC 同一 TEST 实际 install 退出 0，回执 SHA `64e9359de27a9a89508ece2cf7ae8e9023abb9de8f2c246eafd1aa1f9713d074`：运行源码 8f、上述 ELF，PID 1150828，20 个 EnvFile、19 个 drop-in，原上限 51/66/51。独立工具集成提交 `7d55b0411f`，V10 只绑定实际 native 证明的 24 项常量，函数/核心 AST 与已审查 V2 相同；V2 离线 121 次执行包括 5 次继承重复，不能称 121 项独立保护。实际绑定 5 项、负例 19 个及公共负载准备 9 项均通过。原 V5 与 V6 安装状态保持原字节。

02:45 UTC 临时上限 120/150/120 已实际应用，回执 `735caa62a6cd7d1c5f2999750aa29b4d86610cb4be32d2860776392e37defdd5`：PID 1151510、invocation `fbb25efb4f464bae96a74b948f3b0744`、startTicks 1551402181，21 个 EnvFile、20 个 drop-in。两次均先以 9 个有类型零计数排空、普通 SIGINT 退出，再验证新激活；旧 Gateway/Zone/Caddy/四 validator/aux 共 8 个进程指纹及原配置不变，无 SIGKILL。云机新一轮 `capacity8f-progressive-v1` 已开始 2/25/50/100 各 300 秒公共 strict movement/chat，沿用原账号池、账本、P3/W4 与 25+5/901000ms 认证预算；主监控和只读 TCP 同采。首个 SLO/安全失败停止加人，所有已收尾结果均需正常排空并恢复 51/66/51。本轮已闭合失败：2 人 300.878 秒通过，25 人约 180.995 秒因逐人分钟门失败，未完成该档 300 秒、未到 50/100；全日志 2971 offered、2966 completed、4 次 Walk timeout（3 owner ACK、1 peer witness）和 1 次 pending 未发送，未以 NoLegal/AOI/资源保护失败代替真实超时。全部 25 次真实 LogoutSuccess 与独立 Close 均在各自 5 秒内；实际执行 4 轮/8 操作穿脱全部通过，未入场 99、未完成 fresh-save。报告 `37985b8668064a44e0971a9e24e58a2953554530c1115429cc94aeb0a420f5f0`，同池 after `89f69ef2690acaf4e90ca82873ca5e107ecbee2da830792380ced6233d9e3fcf`，固定新源码闭合汇总实际退出 0，SHA `aa066e7b6080e054caa390837dc15e1be033e9862478bb1eb9921a1f512f9c4d`；所有容量标志保持 false。主监控 412 samples 本地退出 0，TCP 391 samples 本地退出 1/不完整、远端退出码 unknown，未冒称完整 TCP 诊断。03:04 UTC 排空九零后普通恢复 51/66/51，回执 `331b35a28ac901d42ce0df102147d48169c60fd74c742586b53c6eca5308f090`：仍运行 8f/EDD，PID 1159915、invocation `7fc1d202504a4cbf82df22b5dc6080ed`、startTicks 1551513392，20 EnvFile/19 drop-in，8 个受保护进程不变。云端公共 movement/chat 与本机共同 dense、真实三端渲染及完整战斗容量分列。

云机第三轮 2/25 人诊断于 00:18 UTC 完整结束：每档 300 秒，窗口评分 320/320、4000/4000，10/125 个逐人分钟窗口全部通过；27 个连接（含最终 fresh 2）全部在各自原 5 秒门槛内取得 LogOutSuccess 和正常 Close，6 轮穿脱与 fresh 保存核对通过，最终 WS/active 为 0。load client 实际退出 0、所有 owned 成员退出且无 kill，同池 after 校验通过；报告 SHA `2e83a85433ee37437ee026d35653f0b5d4786d04c9cded764833309a4dccef0a`，after `f89891e33fd6a2834d1df538ff98363502ba7a816856357f2d53ba1759ff2cc7`。25 人 target CPU 平均 0.429567 核、采样峰 cgroup 约 344MiB、游戏 WS 应用下行 0.342651Mbps。沿用 51/66/51，无服务重启或上限操作；一次成功不能排除前两轮的间歇性故障，也不是 50/100 人、完整战斗或物理三端认证。

该轮主监视器 505 samples、本地退出 0；TCP 补采 505 samples，按 own stop-file 结束、本地退出 1，部分内核字段缺失，远端退出码仍为 unknown/null。独立汇总 V5 曾将 SSH method 的 -1 未取得退出码标记拒绝，旧拒绝保留；V6 原样保留 method=-1、remoteExitStatus=null/received=false，28 项离线检查通过，实际闭合汇总退出 0，SHA `2af24c06653068efc2f3c11664710705c7a0f652f20974c32c8cceff29e8ae6e`，容量标志继续 false。TCP 有公网重传/排队阳性，但包含 Caddy 全部路由，未绑定 TEST、actor 或旧失败连接；不据此修改网络。主机带宽“30MBPS”的合同单位尚未核实。

新 8f 云测的历史 invocation `fbb25efb4f464bae96a74b948f3b0744` / PID 1151510、03:00–03:02 UTC 日志已作有界只读实采：965 行、静态选取 825 行，未触及 2000 行上限，远端实际退出 0；回执 SHA `e3c4d7eca4da6f5bd84d971ad60ce663ba50c74e8d76a8210ab190b3fbabbf66`。采集前后当前程序与 8 个保护进程全指纹一致，未改变服务/网络设置。整窗已记录 runtime Tick 最大 141ms、AccountStore wait 42ms、Zone outbound serial wait 31ms；四次超时发送所在 03:00:45–53 UTC 的已选行最大 input turn 65ms。日志缺失仍不证明没有服务端延迟，SDK 与服务端 connection 编号没有可直接相等的证据，暂不据此把两个约 2.07 秒接收空档断言为 WAN 故障。闭合取证 SHA `d48e93dac229d5ff4216832f5bc0279e9c06e63ffbf0f2831f70873b03c18845`：两个 plain 连接共同停收 2067.708ms 时，其他 23 个连接收到 858 包及 26 次 KeepAlive echo；第四次 timeout 的 legacy probe 单 pendingMovementAck 槽关联存在歧义，不能另造为已证实的第四个服务端 2 秒停顿。现有 owner delivery 的 recipientZoneID/坐标及 enqueue→gate→send→sent 追踪可继续缩小范围，但 sent 也不是 TCP ACK 或客户端收到的证明。新的有界只读 trace status 实际退出 0、SHA `b6e0ceb35fe42e2eee2179c5fa825c83238ccec0820120eb66b360f9b881789d`：两个开关均 unset，九计数零、现有 V7 pair 纳入完整 20 EnvFile/19 drop-in 的保护；该次采集时尚未应用临时 trace 配置；后续实际开关及恢复见下文。

云机随后以固定新 trace operator V2（集成 `cfeea42f13`、源 SHA `d97d22c93bc12e99af0327cdcee35415d3ee3fd47ea312410e91855e9a7ccdac`）在九零排空后，只给同一 TEST 临时增加 delivery trace 及接收者 50006/50012 两个开关。原 51/66/51 上限、认证预算和非 trace 配置不变。第一次客户端在执行负载前因装备 actor99 超出 25 人名单拒绝，实际退出 1、正常无 kill、未进入 runLoad；保留原失败，改用本次名单内 0/1/0/1 并先离线验证参数后才开始第二次。第二次实际 2/25 人各完整 300 秒通过：评分 320/320、4000/4000，10/125 个逐人分钟窗口全部通过；全日志另含入场与边界动作，共 4632/4632、零超时/未发送。27 个真实连接（含最终 fresh 0/1）全部在原各 5 秒预算内收到普通 LogoutSuccess 和独立 Close，4 轮/8 操作穿脱通过，fresh 状态客户端核对通过；不冒充 25 人全员独立磁盘验证。client Job 实际 0、全员退出无 kill，同池 after 实际 0，报告 SHA `efbb3aaf83048a57eb0347292ec61344831d83a338a94b6a2758d13fb4cc7dc2`，固定 8f 闭合汇总实际 0、SHA `d396b04365661ab946b13afd2d67a67a1c02d13d1854964113957de043330088`。25 人观察均值约 0.425557 核，采样峰 cgroup 353058816 字节，应用下行约 0.372337Mbps；TCP 500 samples 仍为不完整、本地退出 1/远端退出 unknown，主监控 512 samples 正常退出 0。日志会影响时序，本轮未复现旧间歇失败，不能据此宣称网络或 100 人已修复，也不是物理三端或完整战斗认证。

05:10 UTC 在 fresh status 九零后已普通恢复两个 trace 开关为 unset：restore 实际 0、SHA `42888ac74b3b029b242e6799ed527dd6a4036c1b6a13d97ddcfb891940e2e8b2`；同一 8f/EDD，当前 TEST PID 1193390、invocation `391ec99506d8426d88a3cb457535fb0b`、startTicks 1552270986，20 EnvFile/19 drop-in、51/66/51、owned trace 文件为空，旧 40 个文件及 8 个保护进程未变。恢复后独立只读状态实际 0，公共投影 SHA `5978af403644a604427559986f923a322fffa13ca2f6e4d682f5a1f06bbcb1f5`。原 V7 安装状态 4004 已因这次正常重启成为历史，后续控制必须沿新的终态 successor chain，不能拿旧 PID 状态继续操作。VPN、路由、防火墙、更新及电源设置未改。随后只读实采旧 trace invocation 的 05:01:40–05:08:00 UTC 窗口，当前程序仍按恢复后的新 activation 完整保护：14842 行、14540 条选取记录、零解析拒绝，远端/本地均实际退出 0，SHA `d0fed9cf4686bafe1be13e01128d540afc39cfa0f0505435eb76686158714012`。8 个保护进程与当前配置采前后不变，未触及数量/字节上限；日志未选到的内容仍不能作不存在证明。

新增已闭合材料归档到受保护的私有 evidence 目录 V7：314 文件、1000676388 字节，manifest SHA `3c8f8065087ae98b9daa5e64c328b8233bf14c2992e24e30671646e1bc84a9fd`。V1–V6 与原始文件保持不变，排除账号池、认证账本、存档、工作树与构建缓存；归档不升级任何容量标志。

后续 V8 另存 97 文件、6436530 字节（manifest `800eee0dc4252216f93dc84ddad22d6442b912d42407ce9e6c7b6fe7e9031f0f`）。本轮 V9 已封存新闭合的本机 100 诊断、离线投影/失败与修正、云机 trace 操作公开证明、闭合 2/25 结果及只读 journal：345 文件、967768786 字节，manifest SHA `e5f5b360eb7c8cc193bdd015ec01709e0042f6f870b3332a5c4ba995ed969ae4`。保护 DACL 未改，所有副本逐文件复读哈希与源 stat 稳定，Root 另抽验四份关键终态；旧档和原件未变。明确排除账号池、认证历史/账本、存档、凭据、缓存及尚在进行的 finish 持锁实验工作树。新增小份云端链路摘要晚于 V9 封存，留待后续归档，未补写旧 manifest。

用户提到的重启亦作只读核对：近三天 System 日志中的两条 1074（2026-11-12 02:44/02:46 UTC，即本地 10:44/10:46）分别记录 MoUsoCoreWorker 与 TrustedInstaller 发起计划内系统更新重启。该日志投影 SHA `e614b2e8804b707f68935e682e4d79a9293675410107d47101ee99b5223f8383`；本轮没有修改更新、电源或 VPN 设置。

另行完整调用 Hub 的 Release 专项测量已正常结束：18 组、33600 次真实 publish，包含真实互斥、节流、事件、环缓存、录制、回放和 digest/字节一致性断言；独立 Job 退出 0、所有成员退出且无 kill。100 owner / 4 producer 的三次 publish 外层 P95 为 3.493/3.615/3.990ms，最大 37.893ms，未复现游戏路径的多秒延迟。这是 13 秒左右、匿名构造输入、每轮空闲时重设 emit gate 的专项实验，不能认证 4Hz 吞吐或 CCU，也不能排除长运行的录制、snapshot/Zone/owner 路径。借用编码虽在小组件中更快，不能据此宣称解决 4 秒超时。本轮实际失败的录制文件最大约 849MB，抽样中 91 个实体约 88KB；专项帧最大约 115KB、单目录仅数 MB，长期文件行为仍不同。完整 build/test Cargo 已成功，外层因 MSVC 子进程未退出而失败的记录保留，独立冻结 exe 后重新实跑才作为正常退出证据。结果记录 `full-publish-profile-summary-v2.json`，测试 exe SHA `85739f74f4fa8de2a49f33c0d3f106ef1c9defbcaad050cd96b3ac8980c4fee2`。

以下均为修正后的共同 dense 排程；旧失败记录仍在后文保留。

| 本机版本 | 人数 / steady 时间 | 按时完成 | 超时 / 未发送 | 逐人分钟窗口 | 退出、重登与全员存档 |
|---|---:|---:|---:|---:|---|
| 原版 A7 | 100 / 300s | 16000 / 16000 | 0 / 0 | 500 通过 | 全 100 人正常退出、fresh 2、原 loader 全员读回；v4 独立复核通过 |
| 观战候选 | 2 / 60s | 64 / 64 | 0 / 0 | 2 通过 | 正常退出、fresh 2、File 读回；v7 增强 source 复核通过 |
| 观战候选 | 25 / 300s | 4000 / 4000 | 0 / 0 | 125 通过 | 全 25 人正常退出、fresh 2、v7 全员 File 回读及增强 source 复核通过 |
| 观战候选 | 50 / 300s | 8000 / 8000 | 0 / 0 | 250 通过 | 全 50 人正常退出、fresh 2、v7 全员 File 回读及增强 source 复核通过 |
| 观战候选 / SDK V5 | 100 / 计划 300s，排程超时 | 1887 / 16000 | 245 / 13868 | 未通过 | 全 100 人正常 Logout/Close、fresh 2；全员 File 回读前置门拒绝，未读 private store |
| 同一观战候选 / SDK V6 | 2 / 60s | 64 / 64 | 0 / 0 | 2 通过 | 正常退出、fresh 2、全员 File 与 v7 增强复核均通过 |
| 同一观战候选 / SDK V6 | 100 / 300s | 13914 / 16000 | 444 / 1642 | 12/500 通过 | 全 100 人正常 Logout/Close、fresh 0/99 通过；整体仍 FAIL，File 前置门拒绝，增强 v7 复核实际退出 1 |
| 修复版 7403 / SDK7 | 100 / 300s | 1047 / 16000 | 4713 / 10240 | 0/500 通过 | 全 100 人正常 Logout/Close、fresh 0/99 通过；整体 FAIL，File 前置门拒绝、增强 V8 退出 1 |
| 同版关闭录盘 / SDK7 | 100 / 300s | 15172 / 16000 | 121 / 707 | 109/500 通过 | 全 100 人正常 Logout/Close、fresh 0/99 通过；整体 FAIL，File 前置门拒绝、增强 V8 退出 1 |
| 同版关闭录盘 + slow20 诊断 | 100 / 300s | 15164 / 16000 | 121 / 715 | 104/500 通过 | 全 100 人普通 Logout/Close、fresh 0/99 通过；诊断用途，整体 FAIL，File 前置门拒绝、增强 V8 退出 1 |
| 资格查询修复 8f / SDK7，关闭录盘 | 100 / 300s | 15207 / 16000 | 112 / 681 | 107/500 通过 | 全 100 人普通 Logout/Close、fresh 0/99 通过；整体 FAIL，File 前置门拒绝、增强 V8 退出 1 |
| 同版 slow20 + 两接收者 trace 诊断 | 100 / 300s | 15482 / 16000 | 54 / 464 | 169/500 通过 | 全 100 人普通 Logout/Close、fresh 0/99 通过；诊断用途，整体 FAIL，离线闭合复核另记 |
| 导师 finish 修复 fd3 / SDK7，关闭录盘 | 100 / 300s | 15157 / 16000 | 123 / 720 | 113/500 通过 | 12 次装备操作、全 100 人普通 Logout/Close、fresh 0/99 通过；整体 FAIL，File 门拒绝、增强 V8 实际退出 1 |

25 人 v7 独立复核 SHA256
`61eda521c04c8fa22613fb6b5deeb4cbfb6d955461e01e079e15b5d47274b4f5`；
全员 File 回读 SHA256
`5cb400ec33d8193d634032c3c0bc6c0c1ff3d7c1569bd31346b846edd8cfe807`。
认证等待不计为 steady 时长，也未增加逐动作截止时间；跨轮账本完整保留。
50 人增强复核 SHA256
`89c36cab003cfbe318e99492984afba2d938e7a9b1a4f2aa1d247b6e48080c75`，
全员 File 回读 SHA256
`960ecf935c3da938ff6b24519b2f479a9ca1b91d983703a13c2dcb4936bd7e46`。
100 人首次启动检查因缺少运行回执而在建进程前拒绝；补齐后才首次实际启动，
此前完整认证历史未重置。这个准备错误与服务器负载失败分开记录。

SDK V5 的 100 人失败原样保留：controller/driver 实际退出 1、server 正常退出 0，
3540 条资源行完整，两个 Job 所有成员均正常退出且没有 kill，来源绑定稳定。
全部 2132 个已发送动作最终收齐对应回执，其中 245 个迟到仍记失败；不把迟到
闭合改成按时完成。全部 114 次认证（含装备/最终 fresh 重登）继续保留在完整
跨轮账本，累计 354 次，没有重置窗口。

原始 594340 行完整复核发现压测器混杂：measurement 45.366–375.226s 之间，
全部连接没有 received 或 KeepAlive sent，却持续同步记录 notSent。SDK 在落后
排程后无 await 地追赶，每次又对所有 pending witness 从最多 6000 条历史头部
重复查找，阻塞 Node 事件循环。首批响应迟到的服务端份额尚未隔离，不能将本轮
解释为硬件 100 人上限，也不能将 100 人改成通过。先修复工具的公平调度与增量
取证，再以相同冻结二进制、plan、频率、deadline、评分门槛实跑。
分析 SHA256 `798ab68453ee2ef2847320f1faf71df6471dad4800ffc646ecf4783e24584eb1`。

SDK V6 每个排程 offer 都让出一次事件循环，并用增量 receiveSeq 游标替代重复
全历史扫描。保留原 plan、动作时钟、频率、截止时间、6000 条历史上限与评分；
让出循环所花时间仍计入迟到，迟到结果不会复活。63 项工具测试实际通过。
同一 Windows 二进制先完成两人复验，增强复核 SHA256
`5b80add3484a1b905a30384825c8d6693d76ebd99e70c43514777926ecc92bd5`。
V6 真实 100 人随后完整结束：controller/driver 退出 1，server 正常退出 0，
3613 条资源采样完整、两个 Job 正常关闭且无 kill、来源绑定稳定。完整 300 秒
内有 444 次超时、1642 次未发（1026 次 previous pending、616 次前置条件），
488/500 个人分钟窗失败。不能用后来的成功回执覆盖失败。全部 114 次新增
认证继续保留，完整历史 366→480，SHA256
`242d3d5f7a0f197e64ac1666f9de859658b7b3a58413454b3faead9f85a126ff`。

完整 514806779 字节 / 1894366 行原始日志和 28716 个 receipt 引用复核表明：
接收与 KeepAlive 连续，没有重现 SDK V5 的全进程长静默。最慢 Walk 样本
发送偏差 10.219ms，owner/peer 分别迟到 4025.928/3236.428ms，同连接期间
两次 KeepAlive 往返只有 1.390/1.448ms，分段发送排程 P95 约 14ms。
本轮已消除先前无 await 追赶造成的长事件循环饥饿，不能据此宣称服务器延迟已修复。
300 秒 server/injector 平均为
1.790900/0.257072 核，整轮 server 峰 Commit 2514.922MiB。证据支持游戏
响应/排队路径存在多秒延迟，不能单凭客户端时间排他归因到 Hub、Zone 或 CPU。
唯一完整分析 SHA256
`53cf76aeed23d296d62df47d99fde22073c26b6c527a78bc4cf0af3060080da5`。
另行使用相同二进制、SDK V6、25 人共同计划进行慢阶段计时诊断（阈值 20ms），
认证历史完整继承 480 次；诊断与下面的组件基准均不增加新的容量通过结论。
候选暂不部署。

云端尚未替换程序。当前 82 版本的两人普通功能基线已通过：复用原账号
25/26，一路 plain-Web、一路 native-gzip，fresh bootstrap 后互见、穿脱、
移动、聊天、普通 Logout/Close 与两人新连接存档校验全部通过。报告 SHA256
`91794ecd85c6f8311b6048c01af9409ddeea79b5c4502c82daba3428dbf65540`。
这是旧功能契约的两人基线，没有连续负载或 CCU 认证；前两轮包装器启动采样
失败及 0/99 距离 32 格超过 AOI 的前置布局失败均另行保留，没有改成通过。
云端旧 82 版本的 25 人独立基线首分钟未通过：795/800，actor0 首五次走路
因 `No real AOI movement witness` 未发送，零请求超时，其余 24 人全部达标。
strict 模式实际 62.157 秒便停止升级，没有完成计划 300 秒或最终 fresh 校验。
六次普通穿脱回合与全部 25 人正常 Logout/Close 已闭合，账号池前后复核通过；
所有 owned Job 成员正常退出、无 kill，业务阶段资源证据完整，初始进程出生采样
缺口仍保留。报告 SHA256
`081c0d29bc34d8324c0c9d34904a59c3119c6c4b48337ad49f1d7d60c584ab70`。
暂不能把这五次没有见证者的 offer 解释为服务器容量不足；须先复核位置夹具。
完整原始几何已确认同组最近距离依次 17/16/15/14/13，大于工具的 12 格要求；
第六次达到 12 格后立即成功。另一次相同来源重测在全部 25 人入场前，actors0/1
已被真实怪物打死；装备前检查拒绝，未开始 steady。报告 SHA256
`c0d1bc4072f9d4d7882c58f3184a51b404be726596946367ca28e1e2818afd7a`。
这次仅 23 人有正常 Logout/Close：旧工具先要求存活才能捕获最终状态，抛错
后跳过了两位死者的正常 close，最终落入 WebSocket terminate；不能把进程
Job 正常退出当成普通协议收尾。两位死者没有发出 LogOut，不是服务器 ACK
超时。旧 82 与当前服务端 LogOut 分支及 save 模块一致，未设 alive 门。
最后 fresh 六项 health 计数为零；两轮后的同池前后身份校验均通过。
工具已将最终状态捕获与正常 close 分离：即使死者状态捕获抛错，也调用原
`c.close()`；分别保留 `stateCaptured`、`captureFailure`、`normalLogout`。
状态捕获失败仍使整体失败，也不能进入 fresh-save 通过证明。旧来源及两位
死者没有发出 LogOut 的失败记录原样保留，不据此指称服务器漏 ACK。
最小修复的 25 项离线测试通过；新 load SHA256 为
`4a70deae9d6790effe1a332cba6ab48ef7b4244809df4e8e7c99aba64f5d3938`。
同池普通准备工具（19 项离线测试）首次实际执行让 0/1 正常回城恢复并步行到
安全区，fresh 标志为真；actor2 找不到去目标的合法路径后整轮保持 FAIL，三人
全部正常注销关闭，前后同池身份复核均通过。未改存档、未建新身份、未重置额度；
准备结果不能算 100 人负载通过，仍待核对实际占用与合法路径。
它使用原公共 legacy 契约，与本机 common dense 分开报告。原 100 人公共
账号及认证账本不重建；客户端直接执行 Node，其他 UI 工作仅改变 package.json
的 npm test 脚本，新来源规则只接受该 scripts 元数据差异，其他 25 个运行文件
仍逐字节锁定，并明确记录原始 package 字节已不同。

已完成案件的资源时间线按计划 UTC 300 秒边界，对累计 Job CPU 线性插值：

| 案例 | 平均 CPU 核 | 全日志累计峰 Commit GiB | 应用下行 Mbps |
|---|---:|---:|---:|
| 原版 A7 / 25 | 1.514767 | 3.365139 | 0.055370 |
| 原版 A7 / 50 | 1.566025 | 3.356087 | 0.220358 |
| 原版 A7 / 100 | 1.521516 | 3.391918 | 0.878593 |
| 观战候选 / 25 | 0.169852 | 0.764427 | 0.326475 |

最大相邻 CPU 采样间隔 1.072 秒；独立区间积分与端点插值交叉一致。
内存包含启动期累计峰，最后资源采样后的退出瞬时峰未被完整观察。
原版应用流量含 4 字节 native 帧头，候选只含 JSON payload；均不含 TCP/TLS，
不是云公网 30 Mbps 链路实测。数据只支持这些场景，不能据此线性外推上限。
数值汇总 SHA256 `602f324a6e94827766a413a9ee6cad7636bd02707cff5de3b5340da65b2b3023`。

代码集成提交 `6f04b710766a9e7a2deb553e0f5ec0e1761db598`；本机实测仍是下文
明确标注的 base31e + 9 文件 Windows 构建。Linux 候选来自其直接子提交
`5a356ae0d5b79b846675f623bd5d2dc21c09a10f`：全仓仅多一处未参与 native
构建的 numeron 发布脚本改动，11 项 native 构建输入 Git 对象一致。
[CI 43432291642](https://github.com/Zombieliu/mir2/actions/runs/43432291642)
整体及 native job131908522957 均成功，实际 ELF SHA256
`2b4d0c0590626a376c24486e408de911f0de8c157a9ef221e65508827ff1fa5c`。
1620 张包内地图已逐文件对 Git、manifest 与云端实际加载目录验证，
共 42111309 字节；live map 证明 SHA256
`89bbbf17f63d34116ba53ccc6188aa7ebbf0de9b10ba2236f36fa24dba30e6fc`。
严格 Linux 工件复核实际通过，SHA256
`d6e13878e7b1e747b813a183f4b2a1f13c44bd439fa7e5b584a11e5f688e857d`。
V7 只读 status 已实机通过；此前 V1 本地 ACL 检查因 Windows PowerShell 模块
自动加载失败而在 SSH 前拒绝，最小 .NET ACL 读取修复保持权限要求不变，
97 项测试实际通过，真实私有目录通过、额外 Everyone 权限与缺失目录拒绝。
只读检查不等于部署；此刻仍是旧 82 云端程序，没有部署 Linux 候选，也没有
完成云端 100 人或近战实测。VPN 身份仍未确认，未知辅助 Node 进程按完整
身份单独保护，绝不据已知 VPN 名称扫描为零推断没有 VPN。

## 近战两人校准与修复边界

原版 A8 的两人共同近战校准已实际通过。现代 SDK V6 同样完成 104/104 动作、
2/2 逐人分钟窗口及 8 次装备强校验，但整体仍 FAIL：两人的最后 owner
UserLocation 分别为 Right / Left，新连接初始 UserInformation 与完整快照
却都为 Down。HP/MP、地图/坐标、完整 UID/格位/数量/耐久、gold/capacity
均逐字段一致，没有加入方向或回血容差。公共原包重建分析 SHA256
`d3a81e141b1e79ce4eaa9d473d21182faea93b64b92c0e37949d6ecfb533933b`。
这些动作通过不能替代最终存档门，也没有形成现代近战容量通过结果。

源码断口已确认：物理攻击改变了 Zone 朝向，却未发出既有 SaveTransform
通知；Gateway presence 仍保留 Down，普通 LogOut 前的权威同步据此覆盖个人
镜像。修复只在既有 accepted 门内、实际物理 transform 改变且最后一条同 owner
通知不等于最终值时追加 SaveTransform；magic、拒绝攻击与 Zone 行动期限逻辑
未改。其现有 consumer 会递增个人 runtime tick，不能描述为所有时钟状态不变。

原构建的直接测试实测 2 失败 / 1 通过已冻结；修复构建的独立受限进程
`facing-regression-green-direct-job-v1` 实际退出 0，3/3 通过，覆盖 PvP、PvE
朝向到 presence/正常 LogOut/新 Zone fresh 存档，以及拒绝攻击保持原朝向。
所有 owned 成员正常退出、无 kill；Green 回执 SHA256
`767f5cbd1a3ebf9e1268a25e064d15c5a8b6da9664a956c772c6d95c58be5557`。
相邻回归已在独立受限进程完成：Gateway 攻击结果 15、owner 动画 1、位置顺序 8、保存围栏 6、观战 26 项均通过，组件剖析保持 ignored；共享 Zone 四项攻击/移动/转向回归也通过。旧安全区夹具在修复前同样失败，改用真实导入的安全区边界后，三项攻击模式测试通过。代码已集成 `7403bc5ee5cbb472477db101b8be603136049dcf`；后续构建及真实两人结果见顶部。单元测试本身不代表 100 人延迟已修复。

现代 SDK V7 另修复最终 fresh 校验的收尾缺口：比较失败也会分别尝试一次原期限
的普通 LogOut 与 WebSocket Close，不重发未知结果，不复活状态/退出失败。
66 项离线测试通过（原 63 加 3 项生命周期回归），旧 SDK V6 对同三项反例失败；
冻结包 `modern-adapter-v7` 没有执行新游戏负载，也没有改变动作、排程、认证额度、
期限或评分。云端 legacy load 的死者收尾修复与此 SDK 修复分别锁定来源。

## 观战 previous-world 组件测量

默认优化 Release 的同一测试可执行程序已实跑受限 A/B：25/50/100 owners，
每个 8-ID 重叠视口、每 owner 32 次、3 trials，虚时 250ms emit gate；输入为
实际 SceneView/TerrainPatchTemplate DTO 形状的 0 或 1024 个公共合成瓦片。
它调用生产 merge/prune/world_events，未运行游戏、网络、数据库或录盘。

100 owners / 1024 瓦片条件下，previous 捕获 + world_events + previous 释放
三段累计由 3.7364462s 降为 0.8105117s，约 4.61 倍；相应合并段逐调用
P95 为 0.4899ms / 0.1031ms。9600 次更新事件序列及 96 次 emit 的编码字节、
digest 全部相同。原始 stdout SHA256
`9300148b743b77fffafd7611696404b15f30a360997576bf573caad7bc3cf64f`。
这不是完整 publish、锁等待、Tokio 或 CCU 的加速比；同次测量未覆盖非空掉落、
ring 淘汰、真实地图或进程并发。previous 事件字段投影已有实现，26 项观战回归及原始字段/异常值差分测试通过；修复版录盘开/关两轮 100 人均已结束且 FAIL，结果见顶部，旧失败保持不变。

## 已固定的边界

- 本机服务器进程及其所有后代合计：4 个逻辑 CPU，亲和性 `0xf`，8 GiB
  committed memory 上限。包装器也计入；OS 与压测器另记，压测器用 `0xff0`。
- 第一层为 `game-core-local`：原版文件存档，对比当前 File authority +
  in-process session cache。PostgreSQL / Redis / Internet / TLS 尚未合格。
  本机固定 `crystal_full`、DogYoHyun 与共同装备；不是云端正式内容配置的认证。
- 基线服务器 Release 的源码：`31e9b6a74661c31d6515dd7322b466285f975d71`；
  二进制 SHA256：`ae8c83c6395e39e79e05dfb69989381669a0130ab855d9803946d84d00a9e9b4`。
  这是本机测试产物，未据此声明公共服务已部署。
- 原版仍用冻结 GUI host `26f07d85…`，原游戏库 `8104a1cb…`、Shared `fb90224b…`，
  未重编或修改原游戏源码。原版正常加载 463 张地图，测试 Map440 无怪 / NPC。
- 双方使用活动 `DogYoHyun.map`，原始 SHA256
  `d34167876db45bcac3d879e3ca1e8909efc38fffedbfc1511cef4c4215c2f608`。
  仓库同名 gzip 解压后实际上是原版 `Unused` 版本，不能按名字认作同一地图。
- 原库导出的导航 SHA256：
  `5a6119c4cf673f0df0020a5406584118d91bcd2e44f422dcf242ef6f7e940697`，
  404451 个合格格子；固定不重叠路线，避开门、传送、NPC 和安全区。
- 40 级男战士、相同普通装备及药量。普通登录、走路、聊天、穿脱、退出、
  新连接重登；不使用 QA / 传送 / raw account-id 登录。
- 正常登录限流与本机跨轮 attempt history 保留；旧公共 100 人账号池与账本保留。
  本轮没有改动 VPN、路由、防火墙、Windows Update、电源策略或旧服务。

## 旧排程的实际结果（保留，不能替换成修正后结果）

| 本机版本 | 人数 / steady 时间 | 动作完成 | 超时 / 未发送 | 退出与存档 | 独立复核 |
|---|---:|---:|---:|---|---|
| 原版 A5 | 2 / 60s | 64 / 64 | 0 / 0 | 全员正常退出、fresh 2、原 loader 全员读回 | v1 通过 |
| 当前 modern-v1 | 2 / 60s | 64 / 64 | 0 / 0 | 全员正常退出、fresh 2、File 全员读回 | v2 通过 |
| 原版 A5 | 25 / 300s | 4000 / 4000 | 0 / 0 | 全员正常退出、fresh 2、原 loader 全员读回 | v1 通过 |
| 原版 A6 | 50 / 300s | 8000 / 8000 | 0 / 0 | 全员正常退出、fresh 2、原 loader 全员读回 | v3 流式复核通过 |
| 原版 A6 | 100 / 300s | 15990 / 16000 | 0 / 10 | 全员正常退出、fresh 2、原 loader 全员读回 | 未通过：actor99 第 5 分钟失败 |

100 人没有服务器响应超时，但压测器未发出 10 个聊天，不能以总体完成率
99.9375% 覆盖失败窗口。该轮原始日志 477416419 字节，旧失败保持失败。

已确定共同生成器错误：walk phase 是 `floor(ordinal*2000/population)`，
chat phase 却是 `floor(ordinal*1000/population)`。100 人的 actor99 会在
15980ms 走路、15990ms 聊天。Windows 一次调度晚约 15ms，就会同次分发二者；
前一步实际只耗时约 3.67ms，也来不及在后一 offer 之前完成。单未决规则正确
地把聊天记为 `notSent`。这轮不能归因为服务端 CCU 上限。

旧 50 人窗口内下行应用帧 8264348 字节 / 300 秒，约 0.2204 Mbps；
包含 native frame header，排除 TCP/IP，且未混入登录、目录及穿脱。
这不是云出口链路的实测，也不是其它战斗 / 聚集场景的带宽承诺。

## 修正后的共同排程

`workload.v2.json` 明确 `same-primary-phase-with-midpoint-offset`：
chat due 为 `15000 + floor(ordinal*primaryInterval/population)`，再每 30000ms。
走路间隔 2000ms 时，聊天距本人的前后走路各 1000ms；近战间隔 1200ms
时为 600ms。双方共同修正，动作数量、频率、超时、P95、资源上限均未放宽。
所有人口的间隔回归、旧新定义差异检查等 13 项生成器测试通过。

新 dense 计划为 `plan-functional2.v3`、`plan-dense{25,50,100}.v3`；
定义 SHA256 `9172f5786dd1fad2bcbd3d0fc173c4070c738322710707128b9466e245a4e6e5`。
修正后须重新校准、复测两边，不能把旧通过结果改名为新结果。

修正后原版 A7 与 modern-v3 均实际完成两人 64 / 64，零超时 / 未发送；
两边正常退出、fresh 2、全员磁盘读回及 v3 独立复核均通过。

原版 A7 修正后 25 人 / 300s 已完成：4000 / 4000、零超时/未发送，
125 个逐人分钟窗口全部通过，全员正常退出、fresh 2、原 loader 全 25 人
读回。controller / driver / server 实际退出 0，310 条资源行完整，独立 v4
复核实际退出 0；复核进程峰值约 296MB，4.6s。新版仍待候选回归后同负载重测。

原版 A7 修正后 50 人 / 300s 也已通过：8000 / 8000、零超时/未发送，
250 个逐人分钟窗口通过，全员正常退出、fresh 2、原 loader 全 50 人读回。
controller / driver / server 及 v4 独立复核均实际退出 0；312 条资源行完整。
修正后 100 人随后完成并通过，见顶部；旧 100 人失败仍保留。

新版修正后的 25 人已实际建连，但未进入稳态：首个 RemoveItem 的 ACK
约 185ms，随后普通 LogOut 从 10:02:46.574Z 到 10:02:53.148Z，耗时
6574.5944ms，超过原 5000ms 门槛。该轮仍失败，不把未执行的动作解释成
服务器活动吞吐为零。等待中同 socket 的 KeepAlive 仅 0.2–1.5ms，
服务器 Job 持续约 3.86–4.04 核，injector 很低；资源围栏成立不代表 CPU 有余量。
实际全员 Close 完成，其他 24 人正常退出；actor0 的 owner 消失发生在
迟到的正常退出之后，不能误判角色死亡。

现有代码会在 Remove ACK、owner snapshot 之后继续同步 `flush.save`，
个人 owner turn 要到这一步结束才交给随后 LogOut；目前尚不能把延迟归因
为 LogOut 自身保存、前一保存、排队或 Tick 的哪一项。

两轮正常登录 25 人、静置 60 秒、一次穿脱与原 5 秒退出门槛的诊断已完成，
controller / driver / server 均实际退出 0，均正常关闭；短诊断没有复现上述
6.57 秒退出。第二轮只增加观战内部计时，二进制 SHA
`de71013b73cff40bc26730727359f0de3dc8b7b8886eee027d23985514a7d765`，
不属于行为修复或容量通过。慢阶段日志主要集中在 `spectator.snapshot`；
只记录大于 50ms 的 wall time 样本，不能把样本比例当成完整 CPU 归因。

独立资源时间线核对：尚未发出走路/聊天的稳定 18 人区间，324.386s 内
服务平均 2.484116 核；稳定 25 人区间，513.406s 内平均 3.935604 核，
占该 Job 四核预算 98.3901%，injector 分别约 0.004 核。这里仅证明登录后
后台工作已消耗预算，不据此线性外推人数上限。

已测的观战候选保留观战及录制配置，单一 Full / SpectatorScene builder
避免观战构造随后被丢弃的私人 UI；完整客户端快照继续走 Full。两者复用
原共享 Zone 的权威位置、AOI、血量与玩家投影，RPC 默认仍读取远端 owner。
输出一致/状态不变回归、受控诊断与 25/50 人共同负载已完成，结果分别记录；
100 人仍失败，不能把前述局部改进扩展为该档容量通过。

首轮 simulation 回归实际 6 通过 / 1 失败：新换图夹具的 platinum_176
白名单拒绝 DogYoHyun。仅改夹具使用允许的 D401，并强制检查 MapInformation
及 UserLocation 后比较；生产准入规则未改，后续 7 项实际通过。Gateway 首轮
test 编译的三个子模块类型导入错误也已修正；两次退出 101 均原样保留。
后续语义测试仅将待测 package 的优化等级设为 0 来缩短大型测试库编译，
实际性能二进制仍须用默认 Release 优化单独构建、冻结后测量。

修正后 simulation 7 项、Gateway 观战相关 25 项均已实际通过。相邻回归
发现原有两个 AOI 夹具使用默认 demo 已持有的 `Scout`，普通建角返回全局
重名 `result:5`，尚未执行快照。仅改这两个测试名，保留全局唯一性校验；
随后 4 项 AOI 检查及独立进程执行的 9 项权威回归实际退出均为 0，覆盖
hidden monster、个人 quest icon、变身身份/成员校验、权威位置/AOI 与换图。
候选生产路由前缀未因这些夹具修正改变。上述语义回归与随后默认优化 Release
构建、实际性能实跑分别记录，不能相互替代。

默认优化候选已实际构建 0：Windows exe SHA
`dca380d20fdd67490ea6fc24d941588a90262d8f6484fb27b2f5a8c1fb1ee177`，
build manifest SHA `f42d482f98d8520544731a86ecad42aff47639a7d1dfa7dc2fd989c8df01def1`；
base31e + 9 文件 source-inputs-v4 SHA
`0eb1e809d6278b922d4279cdb5f2c353c615ba1e95533c9fb10994597a17c1ee`。
这不是 Linux 工件，也不是云端已部署版本。

候选同条件 25 人诊断（v3）已实际完成：controller / driver / server 均 0，
102 条资源行完整，25 Logout 与 25 真实 Close 均通过原 5 秒门槛，双 Job
正常退出且未 kill。60.015s idle 的 UTC/累积 CPU 边界积分为 0.110643752 核，
对照 v2 的 3.815757280 核，下降约 97.10%；最慢 Logout 647.8868ms，
对照 4097.8812ms；Remove ACK 53.549ms。观战/录制、SLOW50、人口、固定
时长及底层 driver/contract/WS 保持一致，原始收流/ACK/Close 已独立核对。
这是观战冗余私人 UI 构造的直接受控性能证据，仍不等于 100 人稳态或云端
CCU 通过，也不能覆盖旧 6.57 秒失败。公开分析 summary SHA
`e95d692215a0055abee040f90c897f44c71d32c6f22e9a86a50e8e8d37005950`。

大日志复核已在独立 8 CPU / 4 GiB Job 实跑：旧 50 人峰值约 436MB、13.2s；
旧 100 人完整复核到 500 个窗口峰值约 776MB、45.2s，失败窗口仍保留。
日志双遍全扫描与 hash / 行数 / 文件稳定性检查都不截断，选中证据预算为
256MiB / 500000 行。它是复核器内存预算调整，没有放宽游戏 SLO。

## 云机只读复查

2026-11-13T10:07:12.999Z 验证主机 key 后读取，服务身份仍为 PID803500、
invocation `2645494fe370413593ea1b04c29707bc`、start ticks1541658437；
公共服务源仍 `82efff2c8c0eef72a950501d84acad3c6c3de958`，exe SHA
`6ed4ee817b2577858860a9bf53cea12845d5ab2cb0d855eb5ffa678ba9ccc01a`。

TEST 配置 caps51/66/51、并发登录/建角/进场2/1/2、workers4/tick300ms、
PG pool4；cgroup memory.max 为 1600MiB、high1400MiB。整机4逻辑CPU、
约7.752GiB由旧Gateway、Zone、4validator、PG等共享，读取时 available
约1.416GiB。当前 PSI avg 为0、OOM计数0，仅代表读时状态；51是配置值，
不是这台硬件测出的稳定极限。该 TEST 并没有独占本机对照的4CPU/8GiB预算。
本次只读没有服务、配置或VPN变更，原始投影与SHA清单在证据目录保留。

10:59:09.832Z 第二次只读投影确认同一服务身份：观战开启、录制关闭、
public 开启，月卡价格存在且为正。对照本地基线的录制默认开启、月卡价格
未配置，不能声称所有生产功能配置相同；缺价格也不能推断 billing 全部关闭。
AI 缺失配置项继续列为未知，源码默认值与运行中持久化状态分开记录。

另核对内容配置：云端 platinum_fiveclass 的地图白名单不含 DogYoHyun，
共同夹具 SharpBracelet（482）也不在物品白名单。IronBracelet（476）允许，
但 Accuracy 不同，不能悄悄代换后沿用旧结果。真实云端同负载测试前需另立
允许地图/装备的共同夹具并对两版重校准；保留现有白名单与旧本机结果。

近战契约与两版工具另有固定补药及目标因果要求。原版 A8 与现代 fd3/e340 + SDK7 的最新两人校准均已闭合通过；现代 104/104、2/2 窗口、朝向 Right/Left 正确持久化，File 与 Enhanced 通过。旧 V6 朝向失败仍保留；当前 6024 候选尚无 fresh 近战实跑，更高人数普通战斗容量仍未认证。

本机完整工作证据位于 `%TEMP%/mir2-comparable-capacity-20261113`；
`ACTIVE-HANDOFF.json` 记录每次实际退出码、冻结构建、失败与当前执行位置。
私密账号、环境密钥和账本不写入此公开记录。

云端位置准备 V2/V3 的失败仍保留：V2 完成 13 人后遇原目标真实怪物占用；V3 在首步前合法另选目标，完成 23 人后 actor23 在已确认移动后触发准备器错误。24 名已登录者均收到普通 LogOutSuccess，Job 成员全部退出且未 kill；after-preflight 确认原 100 人身份与账本策略保留，最后 health 六项在途数均为 0。监视器经 stop-file 正常结束，服务仍为 PID803500，没有服务或 VPN 操作。这些仅为场景准备，不是 CCU 测量。

V4 进一步完成 35 人／7 组，在 actor35 起点 `(285,606)` 因 NPC 邻格缓冲围住全部出口而停止，原报告 `f23d59317b76dcbe53239462ef1ed0435a7fb31d87c2381fc2ee76725c8e38d0` 仍 FAIL。36 人的普通 LogOut 均在原 5s 内，Job 全员退出且未 kill；监视器正常结束，after-preflight `8058d23b230d68c92b3f326160e2074f56bf38d804eb0f5c1efcf448728b451e` 通过。旧原始关闭帧未记录 code，保持 unknown，不编造 1000；processEnvelopePassed=false 及启动采样缺口保留。独立只读对照证明仅将 NPC 邻格缓冲保留给目的地选择、通行保留 NPC 本格和全部观测 Spell 阻挡，即有普通 Down 起步的 7 步合法路线。V5 将用固定公共证据续备剩余角色，继承的 35 人不冒充本轮 fresh 检查；最终容量负载仍须正常重登全部 100 人。
