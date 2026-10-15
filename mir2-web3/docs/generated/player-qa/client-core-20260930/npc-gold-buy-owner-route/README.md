# Progress74：NPC 金币购买的本地 owner 路由

2026-10-06 记录；测试发生于 2026-10-05 UTC。Source03、实际有限检查及独立复核均已通过，0 blocker。

## 本批结果

| 实际检查 | 通过 / 执行 | 范围 |
| --- | --- | --- |
| npc03 | 18 / 18 | 16 项新增路由测试及 2 项原有到期物品测试 |
| session03 | 34 / 34 | 原有网关会话回归 |
| game_shop03 | 1 / 1 | 原有 typed GameShop 转发回归 |
| 合计 | 53 / 53 | 3 / 3 组；16 个不同的新测试 |

本地 typed owner 调用将每次购买的已知提交/拒绝结果传到网关。未认证、未开始游戏、失效租约、错误模式/元组及不支持能力在执行前阻止购买；RPC/Hosted owner 不通过影子会话或普通 execute 回退。原有共享预排空、Zone/AOI 协调、尾部包、元数据和事件顺序保留。提交后的处理异常保留本次已知结果；捕获前异常返回 Unknown，不借用上一次结果。

只修改 Gateway 的 lib.rs、routing.rs、session.rs，并新增 npc_gold_buy_route.rs 与独立测试文件。996 项声明来源中，原有 994 项里的 991 项保持；原共享 execute 的 804 行 / 38192 字节通过逆向机械还原逐字节相同。没有新增网络回执协议；BuyItem 三字段、四键 JSON 和旧 GameShop 行为未改变。

## 证据及失败记录

- [Source03 冻结记录](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-owner-route-01/web-snapshot03.json>)与 [独立源审查](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-owner-route-01/rust-source-review03.json>)。
- [三组实际检查](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-owner-route-01/root-actual-checks01.json>)、[Root 正式核验](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-owner-route-01/root-verification01.json>)与 [共享通道逆向核验](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-owner-route-01/root-shared-pipeline-inverse01.json>)。
- [有限检查后保护边界](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-owner-route-01/root-protected-boundary-after-finite01.json>)与 [仅本批 15 个已有 PID 的退出核验](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-owner-route-01/root-own-pid-check01.json>)。
- [Source01 失败](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-owner-route-01/root-actual-failed-check01.json>)及 [夹具修正](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-owner-route-01/root-fixture-repair01.json>)：NPC 初始位置与玩家同格，18 项中 3 通过 / 15 失败。
- [Source02 失败](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-owner-route-01/root-actual-failed-check02.json>)及 [预期修正](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-owner-route-01/root-fixture-repair02.json>)：既有 Demo 初始化会补入药品，StartGame 已分配首个序号，18 项中 16 通过 / 2 失败；修正为相对初始值断言，产品源未改。

两轮失败保留，不计入当前 53 项通过数。仅编译及纯内存测试，账号/数据库/存档路径均为显式内存配置。实际三次 Cargo 串行，最低实际 C 盘余量 158324486144 字节，高于 50 GiB；nonce、原工具退出、正常回执与 Policy B 的子进程完成/退出/释放记录一致。Root 仅查询来自五轮实际调用的 15 个 PID，均已退出；没有查询受保护 Native PID。独立复核只读 5562 项数据 pins，0 drift。

## 当前总目标状态与下一步

上次模型容量错误在 2026-10-05 07:20:57 UTC 中断执行；当前前台代码工作已恢复。2026-10-05 16:18:23 UTC 实读 goal 调度状态仍为 blocked，未宣称后台恢复或目标完成。此前整体 35% 估算撤回：没有可审计的非重复加权总分母，阶段数、提交数和测试数量都不作为项目整体百分比。

当前跨端阶段：

| 阶段 | 实际状态 | 剩余验收 |
| --- | --- | --- |
| CP-00 共享边界 | 边界契约已完成 | 不等于产品完成 |
| CP-01 新手任务 | 部分实现及历史证据 | Windows 与实际 Web 接取—装备—提交—保存—重登 |
| CP-02 共享 UI / 输入 | 大量代码及有限检查已完成 | 完整界面、真实资源及输入链路 |
| CP-03 移动端 | 真机验收未完成 | Android/iOS 触控、IME、安全区、内存与生命周期 |
| CP-04 完整流程 / 恢复 | 部分实现 | 购买 ACK、幂等与投影恢复，以及完整玩家闭环 |

阶段数量和测试通过数都不是整体完成百分比；不继承未验收的原生行为。

下一轮先建立关联请求与权威处理回执：重复请求只扣款一次，错误元组不执行，已提交而投影/序列化失败仍保留 Committed，再把结果与完整投影应用接到 Windows/Web 的共享 Core 恢复屏障。跨连接耐久性需实际持久化证据，尚未验收。之后构建当前完整组合并补实际玩家与移动设备证据。

本批未验收：网络 ACK、幂等性、客户端 Unknown 恢复、新生产组合、真实 UI/玩家/Android/iOS、100% Candidate 与总 goal。沿用用户“继续代码，暂不操作界面”；历史生产包和历史界面证据仅保留各自原有限范围。
