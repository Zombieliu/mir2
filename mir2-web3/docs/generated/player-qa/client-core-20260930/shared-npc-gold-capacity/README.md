# Progress69：共同 NPC 金币买入容量证据

本批闭合普通 Gold/unlimited/panel0 买入的共同 Rust 容量准入。服务端只读完整库存生成 `npcGoldTradeCapacity`，严格内层仅 `rosterValid` 与 `freshCompatibleUniqueIds` 两个必填字段；UID 列表有界、去重，false 必须空列表。服务端复用第68轮完整 carried validator、canonical fresh factory 和既有 strict stack identity，不以购买 planner 成功作为证据 oracle，不声称外部 storage/equipment/Quest 树已完整逐项唯一性验收。

共同 planner 中，None 沿旧严格身份与仅真实空格准入；false 即使有空格也禁买；true 的未列出物品只占格。只有 listed、当前 carried root 唯一、数量与自身 stack 上限合法、完整 raw Info 与 catalog Info 相等且 fresh 载体相容的现代 Bag/Belt 行，才能贡献堆叠剩余容量。Bag1/Bag2 统一为 container0 的 0..79；合法 Belt 分区保留。旧跨 grid root alias 仅在 NPC 专用证据门槛中允许，普通 Bag/Storage 验证未放宽。价格仍由既有 Rust planner 唯一计算，不添加 JS 价格/容量策略。

Native DTO 与 planner 保留完整 u64 UID，Web/portable 接口仅接受安全 JS 整数；显式 null 与缺失证据兼容旧 None。完整 raw carrier 缺失或不匹配不能获得 merge 容量。满包兼容堆叠、多个 partial stacks、Info/base/AddedStats/dura/socket 差异、legacy/default aliases、reserved/Quest nested 身份冲突与非法格/数量均有有限内存回归。

局部 Gained/Lose/Move/Merge/Split、装备、仓库、socket/rental 等 mutation，在现有 handler/log/callback 前同步撤销购买可用性。完整同 owner、当前 source stage 与库存指纹成功投影才恢复；重入或旧阶段不能恢复新 owner。Native reserve、发布、readiness 等待及最终 entry 保留完整库存 proof；Web request/live proof 也保留证据。仅持久 Core authority 排除证据字段，evidence-only 更新不能释放 Entered/Flushed/Unknown barrier。未增加购买 ACK、timeout 或 auto retry。

## 实际有限检查

| 范围 | 实际通过 |
| --- | ---: |
| Server NPC | 128 |
| Native NPC | 107 |
| Portable NPC | 68 |
| Windows gateway NPC | 32 |
| Runtime NPC host | 19 |
| Shared WASM | 编译检查 exit0 |
| Web NPC / adjacent / Storage | 108 / 83 / 48 |

Rust 合计354次跨配置执行，不是354个不同场景；Node 合计239项。新增有意义 Rust28/Node10。六个 Rust 命令均由 Root 串行通过既有 .NET CargoGuard Policy B 执行，实际磁盘检查、exit/close/dispose 与源守卫见原始回执。Root 对本轮17个 unique owned PID 的实际查询均 absent，没有查询受保护的旧客户端 PID；本文件作者没有执行命令或查询进程。

Source02 共990当前输入与 after 备份、989 before 完整核对；25旧文件变化、1新增、964旧文件保护。TSC 仅限定承接 Source01 实际通过，Source02 只变四个 mjs 测试夹具，生产 TS/TSX 与编译有效输入字节未变，不宣称 Source02 新跑 TSC。Source01 的 NPC19、adjacent1、Storage1失败及其原日志完整保留；四个夹具补齐真实旧依赖/完整 inventory proof，既有行为断言未弱化。

## 证据绑定

- [Root actual checks](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-npc-gold-capacity-01/root-actual-checks01.json)：28054 B，SHA256 `7232768996539b381d6a95977e7918139c8be36191ae4a39cabccf5b431e9bfc`。
- [Source02 snapshot](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-npc-gold-capacity-01/web-snapshot02.json)：501541 B，SHA256 `fa96dd2f9786341e75d106f92cab30b433381fe7d7b5a54862d5a993ac53ac45`。
- [保留的 Source01 失败](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-npc-gold-capacity-01/root-failed-web-check01.json)：3622 B，SHA256 `47a06a9e40c21656a4935cd6a654701facf86ee3ff2156d5f9ef3d0cd67241a9`。

M14 最终实际独审经 Root 确认通过：current990＋after990＋before989＝2969 个 pins 全匹配，确认 blocker0；六个 fresh Guard Policy B 调用均 closed，新增 Rust28/Node10 具名通过。上列 root-actual-checks01.json 的 independentActualReviewAccepted:false 保留它生成时的历史状态；最终独审接受由 Root 另行封存，不覆盖原记录。

## 下一项与接受边界

Progress70 复用已验收有限 builders，重建 Native、三 renderer、Next/Thin；Core24 输入/18 Rust 当前逐字节未改，只限定承接第67轮生产 Core。原预算保留：Core WASM<262144 B、JS≤204800 B；每 renderer WASM≤32505856 B、gzip≤7340032 B、JS≤204800 B；Thin≤377487360 B。旧 Thin377152588 B仅余334772 B，新组合需实际测量，不能当成本轮产物。

本批没有生产组合构建、WASM Instance/renderer启动、UI、JS sink运行、HTTP或移动真机验收，也没有生产发布。name-tag expiry、无专用购买 ACK 恢复、Pearl/resale/Sell/Repair 与完整 NPC 服务边界仍 open。用户“继续代码，暂不操作界面”持续有效，最终 frontend/Candidate 与整体 goal 未完成，goal active。
