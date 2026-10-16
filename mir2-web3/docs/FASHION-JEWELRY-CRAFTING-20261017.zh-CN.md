# 时装与首饰制作：本地候选说明（2026-10-17）

用户已确认：**时装只改变外观，不增加战斗属性；制作首饰沿用原版道具属性**。本轮代码和验证位于独立本地候选中，制作服务默认关闭，尚未部署。代码候选 `fd21fbdc38c110289e8e92bcb6105d452c8b2db1` 的 Native/Gateway 两端构建已成功，均为exit0、原Guard/audit接受、1453行0漂移，实际EXE长度/SHA与artifact收据匹配；这完成本轮配套构建，不代表安装或实机验收。

数据crate第二轮72通过与Simulation有界组87通过分别绑定 `174269caf8`；Native完整lib1582通过、host focused6通过、Gateway有界组10通过及两端build绑定 `fd21fbdc38`。前序fd21完整Windows host的1139通过/1失败/7原忽略保留为历史；资源修复轮 `53700985cf98f1a4aa479e763f3b27ba3c68053b` 在新的独立QA资产根上完整1140通过/0失败/7原忽略，已关闭该测试关口。原D安装基底未改，不代表全局资产包更新。用户physicalEsc后GUI输入已停止，runtime06未启动，真PostgreSQL未执行。`Candidate100=false`，不据此上调整体1:1百分比或标记人类验收完成。

本轮持久化脱敏摘要入口：[原SHA与结果摘要](generated/player-qa/fashion-20261017/outcomes.json)、[既有安装资源缺口摘要](generated/player-qa/fashion-20261017/host-asset-gap.json)，由Root整理并随最终文档提交归档；摘要只记录原SHA/结果及证据摘要，不是原始完整日志。后续doc-only提交仅安排缓存产物指纹一致性复核，不将它描述为新HEAD重新执行这些测试。

## 普通玩家入口与固定配方

入口为地图 `0` 的 Alice，坐标 `(321,265)`，服务端同时核对 NPC objectId `33`、实际实体和 `BichonProvince/BichonWall/CraftsLady` 脚本。只有显式启用 `MIR2_PLAYER_ATELIER=1` 且拥有可写 File Source 或 PostgreSQL SourceOfTruth 权威时提供工坊服务；本轮本地准备保留独立的 `MIR2_STONE_WORKSHOPS=1` 开关。默认配置不会自动开放工坊。

客户端沿普通 NPC 对话选择发送 `SelectNpcDialog` 和当前服务端展示的 `@atelier:v1:*` target。配方、材料、Gold、制作结果、衣橱和穿戴/卸下选项由服务端提供；未新增通用玩家 CraftItem、Stage5、GM 或 QA 制作入口，也未解除 profile 既有的旧制作通道限制。这四项是本轮明确新增的有界配方，不冒充已完整导入 Crystal 原版配方系统。

| 配方标识 | 材料 | Gold | 结果及职业范围 |
| --- | --- | ---: | --- |
| `copper-ring` | Copper 矿石 `824` ×1 | 300 | 原版 CopperRing `404`，真实背包首饰 |
| `silver-bracelet` | Silver 矿石 `826` ×1 | 750 | 原版 SilverBracelet `479`，真实背包首饰 |
| `traveler` | Copper 矿石 `824` ×1 | 500 | 解锁旅人时装；战士、法师、道士、刺客、弓手，两性别 |
| `jade-gala` | Copper 矿石 `824` ×1、JadeCrystal `904` ×1 | 1500 | 解锁玉晶礼装；仅战士、法师、道士，两性别 |

首饰输出使用真实 Crystal 模板、原始 stats/要求和普通装备路径；当前模板 CopperRing `404` 的原始属性为 `MaxDC +1`，SilverBracelet `479` 为 `Agility +1`，不添加工坊专属战斗加成。实现将真实材料 UID 的承载重分类为对应首饰，并验证完整背包、重量和保管约束；费用与材料消费随权威提交结算。两份 profile 仅新增 `JadeCrystal` 白名单及相应版本/bundle 更新，不新增怪物掉落或虚构素材来源。矿石、切石、制作的自然取得全链路仍需实际游玩验证。

## 衣橱、实际装备与页面

时装制作消费真实材料和 Gold 后只解锁持久衣橱条目，不生成 Armour 装备、不自动替玩家穿戴。已拥有款式的穿戴与卸下继续走当前 NPC target。衣橱拥有集合、选中款式和交易收据分别持久化；旧存档缺失这些字段时默认空衣橱/无选中款式。

场景自身外观、PlayerUpdate 与 Zone 观察者外观使用同一选中款式投影。纸娃娃仅覆盖身体绘制层，真实 Armour、武器、头盔/头发、装备格、耐久、重量、stats 与 item tooltip 仍读取实际装备；翼效果继续依赖真实 Armour。旅人款复用现有五职业资源；玉晶款没有可用刺客/弓手 alternate 库，故明确拒绝这两职业制作/选择，不能用假图或缺失资源代替支持。

工坊使用现有 NPC modal 渲染和可滚动行/选项，在满足视口条件时沿侧栏布局并让 Bag 避让；小屏和触控保留原 modal 回退。普通 Alice 菜单中的 Home 入口本身不启用工坊侧栏。四语标题/footer、当前页面/源戳、已退役页面、旧按下来源、退出和切图门禁均保留。代码接线和静态资源核对不等于 GUI 已验收；浏览器 HUD/纸娃娃尚未完整承接新时装字段，也不记作本轮 Web 完整支持。

## Source、收据与失败边界

服务端要求已认证的当前角色、活动场景、活着且在该 NPC 服务范围内，并核对当前实际展示的链接。动作绑定 catalog 版本、角色身份、Source revision、配方/款式、材料 UID 和完整承载；revision 必须是非零规范 u64，request 是规范的 64 个字符小写十六进制标识。对话报价在最外层 Source 命令完成后才发布，避免使用中间 revision 的立即过期报价。

材料/Gold、首饰或衣橱结果、选中款式及成功收据在同一次权威 Source 提交中结算，提交时再次核对 roster、durable revision、角色和材料保管状态。仅成功提交后发布内存状态与结果；纯内存库或 Replica 不作为该交易的权威。保存/恢复会验证衣橱选择和收据结构，公开客户端视图不携带私密交易收据。

成功收据最多保留最近 **64** 项，涵盖制作、穿戴和卸下。超出时按最早 committed revision 淘汰，不是每角色只能操作 64 次，也不是永不淘汰的永久请求日志。仍在保留集合中的同请求重放不再扣材料、扣费或发放；动作/版本不匹配拒绝。已淘汰的旧请求仍须通过当前页面、角色和 revision 检查，不能复用旧报价再次结算。

材料或金币不足、已拥有款式、不支持的职业、未知配方、坏 target、旧 revision、错误角色/当前页面及材料承载不符均拒绝本次业务操作。明确拒绝不应产生该次业务扣费或产物；**提交结果未知**则保持未知，不伪报成功或退款，不自动重试，只展示通知和 Exit。实现约束的已测范围见下表；Native/GUI 与真 PG 的完整故障、重放及重登行为仍待后续真实收据。

## 已取得的本轮证据与保留的失败

本机 ignored QA 根为 `.mir2-player-qa/fashion-20261017`。以下路径仅定位本机原始日志和 Guard 收据，**不是仓库持久 QA receipt**；不将原始大日志、私密 actor、账号数据或凭证复制入 Git。

| 调用 | 真实结果 | 绑定与本机证据 |
| --- | --- | --- |
| `data-tests01` | 67 passed / 5 failed / 1 ignored；未接受 | 旧候选 `2e45f318ea`；`data-tests01/cargo.log`、原 `result.json` 与失败 audit 保留 |
| `data-tests02` | **72 passed / 0 failed / 1 既有 benchmark ignored**；exit0，audit accepted | `174269caf8b909c20e0d5ba26cffcfbf032c193c`；`data-tests02/cargo.log`、`result.json`、`receipt-audit.json` 与每 RunId 原始收据；1453 行 |
| `simulation-tests01` | **87 passed / 0 failed / 1 原 PG ignored / 2861 filtered**；测试1065.43s，编译另4m02s；exit0，原 Guard audit accepted | 同一 `174269caf8b909c20e0d5ba26cffcfbf032c193c`；`simulation-tests01/cargo.log`、`result.json`、`receipt-audit.json`；1453 行、0 source drift |
| `native-tests01` | **编译失败：2处 E0624，零测试执行**；exit101，未接受 | 同一 `174269caf8b909c20e0d5ba26cffcfbf032c193c`；`native-tests01/cargo.log` 与原 `result.json` 保留；不能记作 Native 测试通过 |
| `native-tests02` | **完整 lib：1582 passed / 0 failed / 14 原 ignored**；测试14.79s，编译3m02s；exit0，原 audit accepted | `fd21fbdc38c110289e8e92bcb6105d452c8b2db1`；`native-tests02/cargo.log`、`result.json`、`receipt-audit.json`；1453 行、0 drift |
| `native-host-tests01` | **历史完整 suite：1072 passed / 68 failed / 7 原 ignored**；当轮未全绿 | 同一 `fd21fbdc38`；`native-host-tests01/cargo.log` 与原结果保留；默认稀疏资源问题待分项核实 |
| `native-host-tests02` | **历史完整 suite：1139 passed / 1 failed / 7 原 ignored**；测试17.10s，当轮未全绿 | 同一 `fd21fbdc38`；通过既有 `MIR2_NATIVE_ASSET_ROOT` 显式选择本地正式只读资产根；`native-host-tests02/cargo.log` 与原失败结果保留 |
| `native-host-focused03` | **6 passed / 0 failed / 0 ignored / 1141 filtered**；测试0.00s；exit0，完整原 Guard accepted | 同一 `fd21fbdc38`；runner03/正式只读资产根；`native-host-focused03/cargo.log`、`result.json`、`receipt-audit.json`；1453 行、0 drift |
| `gateway-tests01` | **10 passed / 0 failed / 0 ignored / 1170 filtered**；测试79.50s；exit0，完整原 Guard audit accepted | 同一 `fd21fbdc38c110289e8e92bcb6105d452c8b2db1`；low_latency 7项 + 既有 shared_player_appearance 3项；`gateway-tests01/cargo.log`、`result.json`、`receipt-audit.json`；1453 行、0 drift |
| `native-build01` | **构建exit0，receiptAccepted / productAccepted / 原audit accepted均true**；完整生命周期，1453行、0 drift | 同一 `fd21fbdc38c110289e8e92bcb6105d452c8b2db1`；`native-build01/result.json`、`receipt-audit.json`及原日志；实际 `mir2-platform-windows.exe` 为131208192B，SHA见下文 |
| `gateway-build01` | **构建exit0，receiptAccepted / productAccepted / 原audit accepted均true**；完整生命周期，1453行、0 drift | 同一 `fd21fbdc38c110289e8e92bcb6105d452c8b2db1`；`gateway-build01/result.json`、`receipt-audit.json`及原日志；实际 `mir2-gateway.exe` 为123352064B，SHA见下文 |
| 弓手资源轮 `archer-focused01` | **原失败exact：1 passed / 0 failed / 0 ignored / 1146 filtered**；测试0.02s；exit0，原Guard/audit accepted | `53700985cf98f1a4aa479e763f3b27ba3c68053b`；新QA `archer-assets-20261017`；1549行、0 drift，95项overlay前后hash不变 |
| 弓手资源轮 `native-host-full01` | **完整host：1140 passed / 0 failed / 7 原 ignored / 0 filtered**；测试19.54s；exit0，原Guard/audit accepted | 同一 `53700985cf98f1a4aa479e763f3b27ba3c68053b`；独立QA资产根；1549行、0 drift，95项overlay前后hash不变 |

已只读核对 `native-build01` / `gateway-build01` 的原 `result.json`、`receipt-audit.json` 和实际EXE：source均为 `fd21fbdc38c110289e8e92bcb6105d452c8b2db1`，构建及审计exit0、完整生命周期、1453行0漂移。两份实际文件的长度/SHA与原artifact收据一致：

| 二进制 | 长度（B） | SHA256 |
| --- | ---: | --- |
| `mir2-platform-windows.exe` | 131208192 | `d927d8adfe8c05414d6971adbef7bf44652ac58fbb46bf8d1830078f4290ae8b` |
| `mir2-gateway.exe` | 123352064 | `a2c76a2c5a48cb66aa2dc63be19f5a76093ea65219cf8b6dbe972c3414824d83` |

两端已按单Cargo通道顺序构建完成，尚未启动runtime06或GUI；后续文档提交不改变以上实际构建的原source SHA。

前序fd21 Windows host首轮多项失败当时表现为 sparse 工作树缺完整 resources / fallback 帧；显式指定正式资产根后，既有正式入口诊断的20项门槛保持严格，完整重跑只余 `effects::tests::archer_normal_arrow_and_focus_catalog_follow_exact_source_frames` 一项。当轮独立只读定位确认：本地既有安装资源包缺 Magic3 普通箭80张帧（`1030 + 10*d + [0..4]`，`d=0..15`）及 Focus 的 `2730..2739` 共90张PNG与对应metadata，安装包 `effects.generated` 中这两个effect均为0条；仓库90张均存在且逐项SHA匹配。`effects.rs`、`main.rs`、`assets.rs` 与旧 `a455` 源SHA完全一致，因此该最后失败定位为既有安装资源覆盖缺口。当轮只定位而未修改资产、帧断言或增加ignore，完整suite结果1139/1/7作为历史保留，不将其改写为通过。

本机缺口审查证据为 `host-asset-gap-review.json`，SHA256 `c7cf8de4f98f0141bf0fed653c100b0ce56c2b857ee4f2809efc17b6b14560cf`；它仍属于上述 ignored QA目录的本机证据。本文不复制其原始路径清单或大日志；持久化脱敏版本入口为[资源缺口摘要](generated/player-qa/fashion-20261017/host-asset-gap.json)。

本轮53700985另以只读安装基底生成新的 `.mir2-player-qa/archer-assets-20261017/assets`，补入90张真实PNG、对应metadata/Source配置（2个JSON）及3个真实WAV，共95个overlay文件；原D基底244项文件未变。这是在指定独立QA资产根上的资源闭包和测试修复，不是修改原安装包或完成全局发行资源更新。原失败exact与完整host两次新调用均exit0/原Guard接受、1549行0漂移，95个overlay文件前后hash相同。最小修复范围见[弓手资源修复说明](NATIVE-ARCHER-ASSET-FIX-20261017.zh-CN.md)，实际新SHA/结果见[脱敏收据](generated/player-qa/archer-assets-20261017/outcomes.json)。

随后独立执行的 `native-host-focused03` 通过6项：2 cosmetic + 2 cursor + 1 wing + 1 weight，0失败/0忽略、1141过滤，原Guard/audit接受、1453行0漂移。它确认这些有界用例当时在正式资产根下真实通过，不替代当轮失败的完整host suite；后续修复的完整通过另绑定53700985记录。

Native 首轮错误来自 `atelier_ui` 两处 `cfg(test)` 期望跨模块调用私有 `QuestLogRect::new`。Root 仅将这两处期望改为相同数值的 public 字段 literal，提交 `fd21fbdc38c110289e8e92bcb6105d452c8b2db1`；生产、数据和后台源码与 `174269caf8` 完全相同，未公开私有构造器、未放宽断言。两处 literal 修复另经独立只读复核，确认期望值和断言语义未变。`native-tests02` 已在该新头完成完整 lib 1582/0/14原忽略；原有数据/Simulation 收据继续按各自原 SHA 记录，不改标为新头测试。

数据第二轮修正精确 profile/数据夹具后重新执行，没有删除或 ignore 首轮失败项。Simulation 的87项包括19项 atelier（含真实 WorldRuntime 调用）、6项 fashion，以及 Stone/Source guild 等相邻回归；这是该过滤组的真实执行通过，不是全 simulation 通过。原 PG 项仍忽略，未执行真数据库。以上结果只绑定该 SHA，不自动转记给后续修复提交。

此前 Stone 集成整套 `simulation-tests02` 留有 **2861 passed / 38 failed / 23 ignored** 历史，路径为 `.mir2-player-qa/stone-20261016/simulation-tests02/cargo.log`。未完成的既有失败继续开放，不能用本轮有界过滤测试宣布整个 simulation 全绿。旧源码 `a455ebf3d4` 的 map-bootstrap **7/7** 与既有 Native/Gateway 成对构建仅属于旧候选；其 GUI 没有完成修后复测，也不能作为当前时装候选的新测试或实机证明。

## 待补的最终验收区

<!-- fashion-acceptance-pending:start -->
| 关口 | 当前状态 | 后续只按真实结果补充 |
| --- | --- | --- |
| 源码与交付SHA | 前序代码fd21为1453行；本轮资源修复53700985为1549行，分别保留原收据 | 数据/Simulation保持174原SHA；Native lib/旧host/Gateway/build保持fd21原SHA；新弓手exact/完整host绑定537原SHA，后续doc-only不改标执行版本 |
| Simulation 本轮有界回归 | **已完成：87 passed / 0 failed / 1 原 PG ignored / 2861 filtered** | `simulation-tests01`，绑定 `174269caf8`；19 atelier、6 fashion 和相邻回归，原 Guard accepted / 1453 行 / 0 drift；不代表整套 simulation 全绿 |
| Native UI 回归 | **已完成完整 lib：1582 passed / 0 failed / 14 原 ignored**；首轮编译失败保留 | `native-tests02` 绑定 `fd21fbdc38c110289e8e92bcb6105d452c8b2db1`；exit0 / 原 audit accepted / 1453 行 / 0 drift；GUI 另待验收 |
| Windows host 回归 | **本轮537已完整1140/0/7，0过滤；原失败exact另1/0/0，1146过滤**；旧fd21两次失败与focused03保留为历史 | 独立QA overlay补90PNG+2JSON配置/metadata+3WAV；原Guard/audit accepted / 1549行 / 0 drift，95文件前后hash不变，原D基底未改；不代表GUI或全局资产包更新 |
| Gateway 测试 | **已完成有界组：10 passed / 0 failed / 0 ignored / 1170 filtered** | `gateway-tests01` 绑定 `fd21fbdc38`；low_latency 7 + 既有 shared_player_appearance 3；原 Guard accepted / 1453行 / 0 drift；不是GUI或完整Gateway suite验收 |
| 当前候选 Native/Gateway 成对构建 | **已完成两端build01，均exit0、原Guard/audit接受** | source同fd21、各1453行0漂移；Native131208192B/Gateway123352064B及完整SHA如上，实际bin与收据匹配；runtime06/安装/GUI另待执行 |
| 普通玩家 GUI、保存与重登 | 未执行；检测用户 physicalEsc 后已停止全部 GUI 输入，runtime06 未启动 | 普通登录建角、自然材料来源、Alice 制作、穿戴/卸下、真实装备/属性不变、结果和持久化；旧客户端选择不计新候选验收 |
| 真 PostgreSQL Source | 未执行 | 真实数据库中的成功、失败、重放/边界收据和角色行一致性 |
| 发布、线上部署、人类验收 | 未执行 | 独立发布和玩家验收结果；本轮本地准备不构成发布许可或完成证明 |
<!-- fashion-acceptance-pending:end -->

Windows Computer Use 可用；在选择既有旧客户端期间检测到用户 `physicalEsc`，Root 已停止全部 GUI 输入。本轮没有新的 GUI 验收，也未启动 `runtime06`；该停止原因不记为工具不可用或实机通过。本轮完整host已在新QA资产根通过；GUI、真PG、部署与全局资源分发仍未验收。

Root 另以 `runtime-map-asset-baseline.json` 重新只读计算真实哈希：地图 `0`、`0103`、`d401` 的 D 盘客户端资源与当前 source 完全相同。该记录仅证明所列地图静态字节一致，不作为当前候选成对 build 或 GUI 的执行证明。

本地 `runtime06` helper 仅处于准备阶段，使用全新 File 库与恢复目录、loopback 地址、新普通角色 `QaCraft17`、全清继承 MIR2 环境和独立本地 secret。新启动必须核对同一最终 accepted freeze 与成对构建收据；旧 runtime、账号、日志和正常停服原始退出码保持。运行计划由 Root 绑定最终冻结 HEAD，启用前必须复核同头构建收据；不复用旧候选收据冒充本轮通过。

唯一计划中的离线前置为停止状态下将新角色设为 **Lv12 / Gold10000** 并同步对应生命/魔法与经验上限，保留原有正常初始道具，不发矿石、首饰、时装或其他物品，不改位置；该前置必须单列，不能计作自然养成、采矿收益或赚取金币。helper 准备不表示它或任何服务/GUI 已经执行。

源码入口：[固定配方/素材与职业范围](../packages/game-data/src/atelier.rs)、[权威交易与收据](../apps/simulation/src/runtime/atelier.rs)、[普通 Alice 对话与最终报价](../apps/simulation/src/runtime/atelier_npc.rs)、[外观投影](../apps/simulation/src/runtime/fashion.rs)、[工坊页面分类](../apps/game-client/client-bevy/src/atelier_ui.rs)、[真实纸娃娃绘制](../apps/game-client/client-bevy/src/crystal_ui/character_page.rs)。
