# Web R16 本地试玩：交付与验收记录（2026-11-04）

**这是初期验收快照。最终 v7 的可玩入口、连续主流程、真实模型教学、限额补给与整服重启结果已在[最终交付记录](WEB-R16-PLAYABLE-HANDOFF-20261104.zh-CN.md)完成。** 下文保留当时未完成的检查和失败边界，不把历史快照改写成最终成功；全项目 Candidate100 和人类 Accepted 仍未关闭。

取证根目录为 `F:/mir2-builds/web-r16-20261104`。已入仓的[安全证据索引](evidence/web-r16-local-20261104/README.md)及[收据快照](evidence/web-r16-local-20261104/receipts.json)记录原文件 SHA-256、各轮时间、测试结果和素材哈希。原日志时间为 UTC，本主题日期为 Asia/Shanghai。工作区含既有 WIP，不能仅用分支 HEAD 代替构建输入与实际包的 pin。

## 本轮范围与实际证据

| 范围 | 已有实现或限定证据 | 尚未据此关闭的关口 |
| --- | --- | --- |
| 登录与选角 | R16 登录、注册、改密、安全键、简体文案与浏览器设置布局；[登录](evidence/web-r16-local-20261104/login-zh-r16-aligned.png)及[选角](evidence/web-r16-local-20261104/select-zh-r16-aligned.png)实图；多轮普通 UI 注册、建角、StartGame 进城检查为 true | 页面实图不等于所有身份方式、钱包签名及账号安全全流程均已实测 |
| 地图、NPC、人物深度 | [Web 深度 producer](../apps/web/lib/crystal-world-depth.ts)按来源 tile、相机相对坐标、逐行 map/live pass 与地面物品先于活物的顺序产生 `sortZ`，交给共享 WASM renderer；正式三包构建有精确 ABI 与体积收据 | 已进城、战斗的画面不能代替所有地图、动作、遮挡与后端组合的视觉验收 |
| 原生背包 | Bevy Bag 输入所有权及重建可见性修复后，限定浏览器轮确认自己的 WoodenSword 装备，并在关包重开后保留同一件装备；[实图](evidence/web-r16-local-20261104/native-bag-equipped-after-reopen.png) | 限定轮结束于背包诊断；之后另一旅程仍出现几何稳定门槛失败，不能宣布背包在所有状态下已完成验收 |
| NPC Buy / Sell | Buy 使用共享原生窗口；Sell 叶按 R16 `176×147` 布局，显式选择背包 UID、显示价格、确认并接原 Core 回调；普通衣服转售有真实服务端回执 | Buy 所有权实图不是成功购买回执；Repair、所有 NPC 服务及每种物品操作未由本轮证明 |
| 组队与交易 | [布局](../apps/web/lib/r16-social-layout.ts)保留 `232×249` Group、左右 `204×152` Trade、Prguse 389/390 与 `2*x+y` 列优先格序；阿洛隔离世界的浏览器邀请后服务端组队为 2 人 | 布局和单元测试不等于两名普通玩家实际放物、金币、锁定、双方确认与取消全过程已完成 |
| 阿洛 | 伙伴六页入口及私有控制链已有浏览器证据；隔离开发世界确认培养 v1→v2，Actor 重启后同 archive 的 v2 与三项习惯保留，服务端组队为 2 人 | 本轮为 `rules`、`modelCalls=0`；未验证 Web 真模型、新冒险行为或完整 Gateway 重启，不能引用 Windows 的模型调用作为本轮结果 |

NPC 限定轮通过普通 UI 选择自己的 `BaseDress(M)`（`crystal-item-317`）并出售 1 件，金币 **0→60**；小药和蜡烛保留，`SellItem=1`、`BuyItem=0`，未调用辅助发物接口。见[确认价格](evidence/web-r16-local-20261104/npc-owned-dress-sale-review.png)、[出售后](evidence/web-r16-local-20261104/npc-owned-dress-sold.png)及收据中的 `observedResale`。这笔收入来自衣服转售，不能记为战斗掉落收入，也不能记为交易已完成。

另一独立普通游玩轮实际攻击 Scarecrow、消耗自己背包的 `(HP)DrugSmall`，随后收到服务器 HP **14→15** 的回血检查，并拍到怪物 0 HP。见[药水消耗](evidence/web-r16-local-20261104/real-owned-potion-consumed.png)、[怪物死亡](evidence/web-r16-local-20261104/real-monster-death.png)及 `observedCombat`。该轮未完成自然掉落拾取，不与 NPC 转售轮拼接成一段连续旅程。

## 各次真实浏览器轮的边界

以下为文档取证快照，各行是独立世界/运行；`checks=true` 只覆盖命名检查。表内整轮 `passed` 均为 false。

| 运行目录 | 已完成的限定检查 | 停止位置 / 未通过代码 |
| --- | --- | --- |
| `qa-bag-visibility-fixed-20261104` | 普通 UI 注册、建角、进城、自己的剑装备、重开 Bag 保留同装备 | `focused-bag-equip-diagnostic-complete`；主动结束的限定诊断，不算完整旅程 |
| `qa-r16-complete-20261104` | 注册、建角、进城、普通衣服转售 60 金币 | `ordinary-field-monster-combat-potion` / `ORDINARY_WALK_TARGET_OR_PLAYER_UNAVAILABLE`；角色在真实受击后死亡 |
| `qa-r16-ordered-journey-20261104` | 注册、建角、进城、真实怪物战斗、自有药水消耗及服务器回血 | `real-monster-drop-ui-pickup` / `BOUNDED_DROP_HUNT_DID_NOT_STAY_IN_TOWN_SAFETY` |
| `qa-r16-snapshot-aware-20261104` | 注册、建角、进城、真实怪物战斗、自有药水消耗及服务器回血 | `real-monster-drop-ui-pickup` / `UI_WALK_NOT_ACKNOWLEDGED` |
| `qa-r16-capture-journey-20261104` | 注册、建角、进城 | `ordinary-field-monster-combat-potion` / `NATIVE_BAG_OWNER_GEOMETRY_DID_NOT_STABILIZE` |

阿洛的[独立公开收据](evidence/web-r16-local-20261104/alo-dev-ui-restart-proof.json)说明：规则提供方、零模型调用、浏览器 UI 操作、培养版本 2 重启保留、一次反馈、私有命令未泄漏到 Gateway/聊天历史。导出只记录已组装并校验 SHA-256，不公开导出内容。该证据来自隔离开发世界，未操作既有 Windows 原世界。

## 构建与测试快照

生产 Next 构建 `next-production-build-v3.log` 为实际 **Next 16.2.11 / webpack**，编译、TypeScript 和静态路由生成完成。构建目录为 `apps/web/.next-r16-delivery-physical-v2`；构建日志和源码版本不能自动证明当前复制包已更新。

| 对象 | 此快照观察到的 pin / 字节数 |
| --- | --- |
| prepared v1 / v2、公开完整包 manifest 中的 Core | `81dd599a887e9338…`；WASM `250,136 B` |
| 上述包的 Presentation | `08bc05f41551a7f0…`；WASM `261,983 B` |
| 上述包的 NPC Purchase | `78c4de3cdd6f9789…`；WASM `165,791 B` |
| 独立 `build-core-final.log` | 校验到新 Core `3797298c65b4d93d…`、WASM `249,982 B`；此行不是旧完整包已经切换的证明 |
| renderer manifest | `bevy-ebebc42dff6639fb`；完整文件哈希见收据 |
| 本地运行依赖 | Node `22.18.0`、Undici `7.30.1` |

Renderer 采用固定两阶段 `-O1 --strip-debug`，再 `--merge-similar-functions --strip-debug`。Binaryen 131 工具 SHA-256 为 `a3e857e565f18773e24cb3d81d00baa70cee9be24efe775e1960565fea5e65b3`，每阶段均核对原始 import/export 元数据；最终才检查预算，未放宽 ABI 或上限。`build-renderer-bag-visibility.log` 的正式结果如下：

| renderer 包 | WASM B | gzip B | JS B | 精确接口保持 |
| --- | ---: | ---: | ---: | --- |
| WebGPU | 29,947,465 | 6,200,992 | 141,118 | true |
| WebGL2 | 16,402,082 | 4,213,985 | 117,176 | true |
| WebGL2 shared UI | 30,876,146 | 6,541,831 | 139,092 | true |

每包固定上限分别为 WASM `32,505,856 B`、gzip `7,340,032 B`、JS `204,800 B`。优化器收据的 `rendererInstantiated=false` 仅表示优化阶段不执行 renderer；实际进城画面来自前述独立浏览器轮。历史 `build-renderer-final.log` 的 `-Oz` 尝试曾被精确接口保护拒绝，不能因文件名含 final 就记为成功。

已保存测试日志的结果如下，套件存在交集，**不合计测试数**：

| 日志 | 结果 |
| --- | --- |
| `pre-integrated-ui-tests.log` | Bag、阿洛、NPC Sell 等 57/57 |
| `r16-ui-service-tests.log` | 深度、登录/选角、社交等 60/60 |
| `renderer-two-phase-tests.log` | 41/41 |
| `bag-shop-final-regression.log` | 210/210 |
| `companion-web-tests.log` | 28/28 |
| `alo-integrated-privacy-tests.log` | 49/49 |
| `alo-page-host-tests.log` | 12/12 |
| `package-ui-supplement-tests.log` | 10/10 |
| `service-package-final-tests.log` | 21/21 |
| `core-auth-tests.log` / `platform-auth-tests.log` | Rust 7 / 2 通过；分别 191 / 62 filtered out |

以上只能证明各自测试范围；零输出的 TypeScript 文件不单独作为全流程通过收据。

## 本地包、素材闭包与体积失败

[打包器](../apps/web/scripts/package-r16-playable-service.mjs)将准备好的 Next standalone、固定 Node/Gateway、service 与 Actor 模块运行闭包复制到独立目录。当前目标为 `F:/mir2-builds/Numeron-Web-R16-20261104`，server 相对入口为 `web/mir2-web3/apps/web/server.js`。`Start.cmd` 默认带 `--with-alo`，隐藏启动后打开本地浏览器；`Stop.cmd`、`Status.cmd` 使用本地私有控制链。服务数据属于 `private/`，发行/压包只允许 manifest 的 `files[]` 与 manifest 本身，不能递归带入运行后的整个目录。

默认 Gateway/Web 为 `127.0.0.1:19040/19041`，默认 Actor 为规则提供方。可选本机模型入口只引用已有机器设置，凭据由 Actor 子进程读取；包不复制设置 bundle 或 key。该入口的存在不构成 Web 真模型调用验收。最终持久世界的正常退出、完整重启和同角色恢复仍需实机回填。

| 度量范围 | 实际字节数 | 结论 |
| --- | ---: | --- |
| prepared Web v1 | 555,031,037（529.32 MiB） | 原 `377,487,360 B` / 360 MiB gate：**false**；超出 177,543,677 B |
| prepared Web v2 | 555,031,041（529.32 MiB） | 同一原 gate：**false**；超出 177,543,681 B |
| R16 有限资源 manifest v2 | 9,468 路径 / 153,042,158 B | 此限定集合逐源哈希通过；与 thin 原集合有重叠，不能直接相加 |
| 完整服务包旧公开 manifest 快照 | 18,992 文件 / 792,985,625 B | 包含 Node/Gateway/Actor/Web；不含 manifest 自身、后续补充及私有运行数据；不是 thin gate 的新分母 |
| 后续 UI 补充清单 | 13 文件 / 7,528 B | 独立安全收据；仍需最终包 manifest 合并与实机核验 |

prepared v1/v2 均已排除 Next 编译缓存，不能再次把缓存字节当作可删体积；本轮没有抬高 360 MiB 预算。完整服务包 v2 的复制日志为 18,992 文件 / 792,985,629 B，和旧 manifest 相差 4 B，故最终交付必须重新给出同一版本的文件清单、哈希与大小，不能混用两次快照。

有限资源 manifest SHA-256 为 `f5ec1d9a7d09f76c56a45cb944edf15166ecf4bacb0d29857a5db1d3dbbf5a5e`。后续[补充收据](evidence/web-r16-local-20261104/full-local-ui-supplement-proof.json)列出 Prguse `544`、`983–988`、`1085–1088`，以及 `CArmour/00/18`、`CHair/00/18` 的源字节和哈希。它修补的是明确路径清单，不能据此宣布全地图/全实体/所有 UI 素材本地覆盖。隐藏 DOM 或后台预加载产生的 raw 图片失败，需要与 Bevy atlas 主画面缺图分开核对；后续验收须记录实际 miss 与外部资源请求，不能用远程命中替代本地闭包证明。

## 待最终回填

- 在同一最终包、同一普通角色上完成持续主流程：进城、移动、自己的物品装备、真实战斗与用药、自然掉落拾取、NPC 买卖、组队/双方交易、保存退出及重新登录；保留服务端回执与实图。
- 解释并关闭最新旅程的移动确认、背包所有权/几何稳定失败；限定成功轮保留为独立证据。
- 对最终新目录重新合并更新与 13 项素材补充，核验发布清单、关键编译 pin、实际缺失请求及准确体积；360 MiB gate 继续按原预算单列。
- 实测 `Start/重复 Start/Status/Stop/再次 Start` 与 Gateway/Actor 正常停止、同角色/SQLite/权限日志恢复；不以开发夹具的新世界启动替代持久服务验收。
- 如果实际启用本机模型，另记真实提供方、调用及 token、教学/提案确认取消、Actor/完整服务重启结果；在此之前保留 `rules / 0 calls`。
- 保留全局 1:1、100% Candidate 与最终人类 Accepted 的原关口。
