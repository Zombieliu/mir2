# R16 完整场景快照压缩候选（2026-11-14）

已实现服务器与 Web 的显式协商压缩，完成协议/真实 TLS、实际 WASM、独立生产构建，以及默认/共享 WebGL2 的四个完整游戏浏览器会话，**尚未部署**。24 条已协商快照的消息载荷合计减少 81.836%；原公网 61.929 秒 ACK 失败与根因未明的结论保持，不宣称公网延迟已经修好或整体 1:1 完成。[原失败和采样](../r16-startup-delivery-20261114/README.md) · [实际浏览器回执](browser-verification.json)

## 行为与兼容

Web 只在加载到匹配的实际解码器后声明 `serverWorldSnapshotGzipV1`。声明完整发送成功后，解码器绑定当前物理连接；后续能力更新保留这个已绑定的能力，换连接重新声明。旧 WASM 仍只声明目录能力，旧服务器及未声明能力的客户端继续原 Text 路径。

服务器仅压缩原 `worldSnapshot` 整条文本，保留所有字段、UTF-8、数字拼写、字段顺序和消息位置。身份、普通协议包、经济回执及 `npcPurchaseOwner` 保持原路径。未删快照、改奖励、碰撞、组队、战斗、存档或授权规则；大于限制、过小或压缩无收益的快照继续原 Text 表示。

| 二进制域 | 规则 |
| --- | --- |
| Magic | `M2WSPGZ1`，与目录的 `M2CATGZ1` 分离 |
| 载体 | 复用原有有界 gzip framing，16 字节头、一个 UTF-8 文本条目 |
| 声明与正文 | 仅一个 `worldSnapshot`，payload 必须为对象；重复/未知外层字段及其他消息种类拒绝 |
| 边界 | 解码区含长度前缀最多 128 KiB，文本最多 128 KiB−4，wire 132 KiB；原上限不扩大 |
| 失败 | CRC、尾随成员/数据、截断、长度或类型异常在任何交付前拒绝；沿用当前连接关闭与换连接隔离 |
| 顺序 | 同步 Rust WASM 解码后交回原 Hero/raw 和普通消息处理，无异步 Blob 队列 |

目录 ABI 保持 1，新增独立快照 ABI 1 导出；目录验证器仍拒绝快照域。四份 Core policy 包保持原各自小于 262,144 B 的预算，目录 policy 为 195,564 B。渲染器未重编译，独立 Next 构建使用已发布 `bevy-0c553a754308e5b8` 的固定清单，未修改 canonical 的渲染器清单。

## 已通过的范围

- 协议 **100 项**：50 单元、16 目录 envelope、34 codec。涵盖精确文本与 u64 拼写、边界、CRC/截断、独立 magic 和拒绝身份/回执等。
- Gateway **35 项**：新连接级开关/实际 writer、原有目录、写入期限/失败围栏，以及本地真实 TLS WebSocket 的普通注册→登录→建角→进城→转向 ACK→正常注销。旧目录 opt-in、旧客户端和原 resume 回归同时通过。
- Web/Core **59 项**：实际优化后的 WASM、独立 ABI/manifest/预算、旧模块兼容、真实 Page 能力替换和原消息入口、Text/快照/目录同步顺序及坏包/旧连接隔离。
- 独立 Next 生产构建与 TypeScript 通过，BUILD_ID `wBO16WGldRpPirWZxFGa1`。用于后续本地 UI 的同源 Gateway debug EXE 也构建通过，未当作生产工件或 Windows 玩家安装包。

194 项是本轮三个相关集合之和，不重复累计早先的 4/4/14 子集，也不是全工作区或完整游戏验收。[资格范围](qualification.json) · [检查投影](checks.txt)

真实 TLS 用例的四条已协商快照：原文本 **167,824 B**，压缩后的 WebSocket **消息载荷 38,070 B，减少 77.316%**。这个数不包含 WS/TLS 头，也不代表整个连接流量或公网延迟下降同等比例。登录前尚未协商的首条快照仍是 Text。

本地默认数据用例包含 **502 条完整目录 envelope**，新旧客户端逐条原字符串相等；公网的 529 条验收门槛保持不变。第一次新测试错误地套用了公网计数，实际 34/35，通过修正为与同一完整本地旧客户端逐条对照后 35/35；原失败保留。[失败及工具恢复](retained-failures.json)

## 来源与保全

构建从已提交 `40d1662f83a52511d2e845ca237180fc6201dec8` 提取，加本轮 11 份明确文件；缺失的 56 个编译期 include 文件全部从同一提交补齐，未复制其他 Native/Shell WIP，也未执行这些 SQL 文件。初次源子集缺 include 的构建失败保留。

首次 Next 构建的跨盘依赖链接生成了无效 `./E:/...` 路径；旧链接保留，改为新物理源码及物理依赖目录后完整通过。新 Core 清单的 source fingerprint 与 canonical 本轮源相等，编译后的 Next 清单与匹配的四 policy 清单相等；八个新不可变文件逐项 SHA/大小校验，旧叶子未覆盖。[清单](core-manifest.json) · [发布到本地源码的叶子](core-publication.json) · [隔离布局](build-layout.json)

原始输出留在 `C:/r16-world-snapshot-gzip-20261114`，提交仅含脱敏投影及[原始输入摘要](inputs-manifest.json)，未导出实际游戏快照、认证凭据或模型密钥。既有 Shell 未编辑，仍 20+/9−；其他未提交改动继续保留。

## 复测与下一项

原命令在独立源码使用已安装 MSVC、Rust 1.89.0、WASM Rust 1.95.0、绑定工具及已验证 Binaryen 131；相关包未新增依赖或扩大预算。

```powershell
cargo +1.89.0 test --locked -p mir2-protocol --features catalog-gzip
cargo +1.89.0 test --locked -p mir2-gateway --lib -- web::world_snapshot_transport:: web::catalog_transport:: web::transport:: --test-threads=1
node --test scripts/test-gateway-catalog-transport.mjs scripts/test-client-core-runtime.mjs scripts/test-client-core-release-files.mjs scripts/test-client-core-build.mjs
```

下一步先对新增的网关诊断合流补集成回归和同源工件，再补旧 Gateway 的实际浏览器兼容，随后做受保护发布和公网扫码、首步、完整字库/HUD 冷启验收。本轮四会话只覆盖本机匹配新 Gateway/Web，不代替公网；原 8 秒移动、120 秒冷启动、字库、公共目录与完整 1:1 门槛继续有效。

## 四会话实际游戏复验

源码为已提交 `199a294130f386a245e86d3c1d81f731d7d7277b`；本地隔离构建仍用 `40d1662f83a5-world-snapshot-gzip-candidate` 标签。匹配新 Gateway debug、四份新 Core 与固定 `bevy-0c553a754308e5b8` 渲染器配对，专用生产构建 BUILD_ID 为 `KQ7eN1Lo2uNQhIxqKTlt7`。本轮补齐固定公开资源及本机 Gateway 的编译环境；未复制 Native/Shell WIP。前次 `wBO16…` 为编译检查，本次才是带完整试玩配置的配对构建。

| 本机路径 | 冷场 Native 就绪，首帧后 | 完全关闭浏览器后，同档 Native 就绪 | 商人路线 |
| --- | --- | --- | --- |
| 默认 WebGPU，英文，男战士 | 54.578 秒 | 1.336 秒 | 189 次走跑，`(324,291)` → 权威终点 `(322,293)` |
| 共用 WebGL2，繁中，女战士 | 52.239 秒 | 1.296 秒 | 同上 |

四场均验证六份完整字体 SHA、113/113 HUD 预取、共用 Q 任务窗、实际右键走路/释放/黑边 DOM 菜单取消、Worker 停止后读回缓存、普通退出及权威位置保存。第二场还通过 NPC 名字/对话实点、已在稻草人半径内的零步提示、九份完整碰撞块及商人长路，到达后停止。每次实际位置前进仍受原 8 秒门槛约束；Native 保留 120 秒门槛。受控 8 秒无响应头、两次 15 秒零正文、字体源恢复等原判定继续执行，没有提高期限。

每条物理连接均实际声明快照能力，Node 被动观察器用本轮 WASM 还原消息，并将 tick/金币/HP 与**游戏自身原始消息入口**的记录匹配；浏览器继续正常画面和操作。已匹配的证据按连接保留，避免 Page 仅保存最新 50 条诊断时被后续普通包挤出。四场各收到全部 **529** 条目录（145 item、200 quest、79 recipe、105 shop）。这四场是本地真实 R16 数据，公网 529 门仍须独立复验。

24 条已协商快照原文本 **1,171,080 B** → **212,715 B** WebSocket 消息载荷，减少 **81.836%**；四条协商前首快照保留 Text。数字不含 WS/TLS 头或其他流量，不推导 WAN 延迟、CCU 或根因。[结构化回执](browser-verification.json) · [脚本、原始记录摘要及失败](browser-inputs-manifest.json)

[英文任务窗](default-en-quest.png) · [英文商人到达](default-en-vendor-arrival.png) · [繁中任务窗](webgl2-tc-quest.png) · [繁中商人到达](webgl2-tc-vendor-arrival.png)。已逐张检查画面；英文截图仍有中文攻击模式与金币预算行，完整语言对齐继续开放。本轮只测战士和正常协议准备的 3 级角色，没有新增三职业从 1 到 30 的前端验收。

首轮测试脚本缺构建回执别名；第二轮取晚期 PacketRuntime 记录，未覆盖提前返回的快照路径；第三轮已在首帧匹配实际入口，后续再次查最新 50 条记录时旧样本已被挤出，catch 又有诊断变量作用域错误。三轮均按失败保留，自己的浏览器/服务均关闭、原黄金档未变；修正测试观察与路径后 r4/r5 完整通过，未修改产品或降低门槛。最终测试工具及原失败 SHA 在摘要文件中可复查。

2026-11-14 10:36:32 UTC 独立 Vercel API 仍读回 E6／Ae3／READY，正式入口未切换。[独立读回](browser-production-readback.json)。旧服务器完整 UI、Linux 发布和公网首步/冷启资格继续单列；整体 Goal active。

## 同源 Linux 发行工件

[工作流 43582710524](https://github.com/Zombieliu/mir2/actions/runs/43582710524) 已全部成功，源固定为 `199a294130f386a245e86d3c1d81f731d7d7277b`。目标 Gateway 任务的 schema union、月卡/计费/活动 PostgreSQL、发行边界及实际发行子进程 PG/Redis 启动均通过；没有启动容器或接触公开游戏服务，新增采购/充值关闭。该启动验证不代替 Linux 实际 WebSocket 压缩与公网玩法资格。

已从 GitHub 下载并逐层核验：84,740,570 B 工件 ZIP 的 SHA 为 `82cd661135f47cc2dfac289294db7bb3f96548c12126228a78ff2ffb4213c544`；内部 Gateway tar.gz 为 `61d7d56b42995ced7899fa0e5e0c6039a0c12b0b719bc09fa68b1edef3a11468`；实际 ELF Gateway 为 `df84177a4778ca466458b1924186a4d4a823868392ace8c0b503dd5c691b7fee`，Zone host 为 `317948e1149f9a6a02c3e0eee9df0ead05415cc4d2af987f8bc665a0fd0b32a4`。地图包整包 SHA 与 sidecar 相等，`0.map.gz` 的有界解压结果与本轮 UI 的 `0.map` SHA `ed4783215ffa989658f79892c2dd6720753fb111e1102cb14da8e6182d88d7f6` 相同。

原串行 CLI 下载长时间没有落盘数据，验证归属后只停止自己的下载；保留自己的序列前缀，用四条明确 Range 补齐后拼接，**完整 ZIP SHA 与 GitHub artifact digest 相等**，再解包验证实际二进制。原临时文件保留。第一次包检查使用了错误的 Zone host 文件名，第二次遗漏地图单文件为 gzip；修正检查后通过，包内容未修改。[CI、工件与逐层核验](linux-ci-verification.json)

可复用本地目录是 `C:/r16-world-snapshot-gzip-20261114/linux-package-verified`：`gateway-releases/` 保存原发行包，`gateway-expanded-r2/` 保存已核验 ELF，`playtest-wallet-release/` 保存地图包。该目录是验证工件，未安装远端服务、替换环境/数据库或修改生产入口。下一次发布必须按届时服务身份与现行发布保护重新检查。

## 分支合流边界

验证期间，R16 分支合入 `525e74364a` 的有界输入/关闭延迟诊断，集成头为 `b950fce0a14f739d89008c864670dcb8f7c8df57`。与本轮 `199a294130` 相比，Web、协议和 Core 源码逐项 Git 差异为空；Gateway 的 `routing.rs`、`web.rs`、`web_transport.rs` 有新增诊断。因此本轮资格固定在 **199a**，不转记为 b950 网关已通过。现有四场浏览器证据、发行工件和未提交改动均保留；下一项先核验该合流。[来源、私有输入、图像和边界检查](validation.json)

## 合流与旧网关兼容复验

后续已在冻结提交 **`9244ce3dd7cd2129a70e650be1430e971b6c96ab`** 完成合流验证。它的 Gateway 产品代码包含 b950 诊断；Web、协议和 Core 与 199a 相同。本轮未提交改动继续保留。先将旧 199a debug Gateway 完整保存到 `C:/r16-snapshot-integration-20261114/preserved-199a/mir2-gateway.exe` 并校验原 SHA，之后复用私有 Cargo 编译目录，仅用 Git 中三份确定的网关文件覆盖私有编译暂存；原物理 Web 源码、Linux 199a 包和所有旧记录保留。

- 合流 Gateway **38 项**通过：原快照/目录与真实 TLS、写入期限、关闭保护，加三个诊断选择器用例；0 failed/ignored。新 debug Gateway SHA `6557d9da87e8cb867616caab80efb28c3063ac7361de1c5d59b0615d401c0842`，147,968,000 B。
- 默认英文男战士、共享 WebGL2 繁中女战士各冷启与完全关闭浏览器后重登，共 **四场**通过。继续使用字节相同的已编译 Web KQ7eN1Lo2uNQhIxqKTlt7；每场完整 529 目录、六字体 SHA、113/113、Q、原 8 秒移动/120 秒 Native、189 次商人走跑和权威保存通过。冷 Native 分别54.565/52.188秒，重登1.377/1.322秒。
- 另用旧网关工件 SHA `f35261772f379b04759011a63a2f3c463a36e754ebbde71915e893c84d77d490` 跑默认英文的冷启/完整重开 **两场**。新 Web 声明能力后，旧网关仍发送 Text 快照；游戏自身原入口匹配 Text 数据，二进制快照数明确为0，完整529目录、Q、原门槛、商人路线和保存通过。此项不是当前公网 fd3 服务的验收。

四场合流会话共24条压缩快照，1,171,080→212,717消息载荷字节；两场旧网关单独记为 Text 兼容，没有算进压缩结果。六场原黄金档未改，自己的浏览器/Next/代理/Gateway 全部正常关闭。[六场结构化结果](merged-browser-verification.json) · [私有输入与旧工件保全](merged-private-inputs.json) · [合流默认到达](merged-default-vendor-arrival.png) · [合流共用WebGL2到达](merged-webgl2-vendor-arrival.png) · [旧网关Text到达](legacy-text-vendor-arrival.png)

发布保护从旧7140候选更新为精确9244来源：相对199a的 **5个完整提交ID、23个路径/状态/Blob ID**，已逐项与实际 GitHub compare 相等。保留目标团队/游戏项目、固定资源、E6生产基线、只创建候选且不自动接域名等原保护；校验完整提交集合，拒绝缺失、重复、未知、额外提交、错误merge-base或非数组响应。75项发布/入口/域名检查通过。[独立来源核对](merged-release-source-remote.json)

[同源Linux工作流43587682763](https://github.com/Zombieliu/mir2/actions/runs/43587682763) 已全部成功，84,747,336 B 工件ZIP完整SHA `25c04ed92da19464fe9b07f6402270c6e49666f61c3eee57bd22cd43c9bf8842`、内部发行包 `2c450f290af80659cd510eb9bc7ab6d2b0e67387d993e2853c9e6c7059fed713`、Gateway ELF `6c42314b92de52114260d3233ac0df013b0fae460be843a49b5c04de345c5553`、Zone host `d65a74fd30137613fe8d9cabbf00cb1e61e5d1708ccb945afed38dc27d79cada` 及地图0解压后的原SHA均已本地核验。[Linux资格与私有日志摘要](merged-linux-ci-verification.json)。可复用工件在 `C:/r16-snapshot-integration-20261114/linux-package-verified`，没有安装远端服务。

2026-11-14 11:41:13 UTC 独立API读回正式入口仍E6／Ae3／READY；本轮尚未切生产。[读回](merged-production-readback.json)。该时点SSH认证被拒，随后以仓库记录的ubuntu用户验证成功；没有修改在线服务。

## 候选实际云端检查与正式站对照

9244独立候选已READY，云端80项发布保护、15项完整资源字节/SHA及二维码create/pending实际检查通过，正式映射保持原E6。新增有界静态时间线工具通过8项与实际Edge/Worker夹具；当前E6正式站一次官方手机尺寸Testnet确认、建角、移动136ms、六字体/113/Q/Worker/注销最终通过，但Native首帧后211585ms仍超过原120秒。1423条静态GET中观测到开门大图进城后仍下载，先对其取消/截止和并发做受控修复。原公网长尾、候选正式切换与整体1:1仍开放。全部明细、失败边界和回执见[候选与完整静态对照](candidate/README.md)。
