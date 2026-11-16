# 普通 NPC 金币购买：共享规则与真实消费者回归

Progress61 · 2026-10-04。用户要求继续代码、暂不操作界面。本批完成共享 Rust 报价/准入规则、Windows 两个购买按钮和 Web 实际页面的接入；验收范围是源码与有限自动化检查，整体 goal 保持 active。

## 已实现

- `client-bevy/src/npc_shop_buy.rs` 成为普通金币、无限库存、panelType=0 商品的共同 planner。使用 catalog UserItem.unique_id，按服务端的 u32 饱和乘法 → f32 乘原始 rate → floor 顺序计算总价；不把已取整单价乘数量。缺原始字段、倍率、完整来源、合法面板或一致 UID 时拒绝已标记普通商品。
- Native 两个提交入口及数量、总价、按钮状态使用同一 planner。网关先检查原始 UserItem 完整性，再使用既有转换；顶层 panel 绑定全部商品，缺失/非法值不补成 0。
- Web Gateway 的 NPCGoods 增加完整 tooltipSource，保留原始 UserItem 与 ItemInfo；页面把完整目录、背包、钱包交给 Runtime `getMir2NpcGoldBuyPlan`，TypeScript 不复制报价或容量算法。
- Web 实际回调签发本地 opaque proof；真实 sendRaw 在同步 action listener 后复核 owner、service/catalog revision、完整 raw 和当前模型，再一次性 claim，随后发送既有四字段 DTO。改选择、数量或布局不能解除同权威状态的重复发送屏障；socket throw 保留未知结果屏障，无自动重发。
- NPC 请求、新对话、地图切换和连接/会话退休撤销旧商店。修复移动确认快路径跳过新对话的问题：新旧 dialog 任一非空必须进入完整投影；无 dialog 的普通移动仍保留快路径。

## 实际验证

| 检查 | 实际结果 | 证据文件（下述 QA 根目录） |
|---|---:|---|
| Native client-bevy 商店相关 | 64 / 64 | native02.result.json |
| portable client-bevy 商店相关 | 41 / 41 | portable01.result.json |
| Runtime 数据查询 | 1 / 1 | runtime01.result.json |
| Windows 原始目录投影 → planner | 3 / 3 | native_platform01.result.json |
| Gateway NPC 投影 | 2 / 2 | gateway01.result.json |
| shared WASM check | 通过 | shared_wasm01.result.json |
| Web adapter / 实际 Page 消费者 | 27 / 27 | npc03.result.json |
| 实际移动 predicate / HUD | 3 / 3 | hud01.result.json |
| 仓库邻接 | 48 / 48 | storage02.result.json |
| Bag / item identity / equipment / spells / combat 邻接 | 82 / 82 | adjacent02.result.json |
| 仓库 surface 集成 | 20 / 20 | integration01.result.json |
| 严格 TypeScript | 通过 | tsc01.result.json |

Web 共 180 次通过执行；Rust 各配置分别列示，不把跨配置重复执行算作独立用例。Rust 新 planner 有 16 项纯规则测试，Native 另有实际 process_overlay_buttons 内存 App 用例；Web 新增 10 项通过 AST 提取当前 Page 完整函数、真实 dispatcher 和 sendRaw 的回归。Web 测试使用固定 Runtime 响应夹具，Rust 规则另行实际执行，不据此宣称浏览器/WASM 端到端已验收。

每次 Cargo 经既有 .NET 守卫执行：实际 C 盘至少 50 GiB 的新鲜采样、locked/offline、jobs=1；六项最终检查均记录 completed/exited/disposed 与 Policy B。TSC 关闭增量写入；限定 diff check 和新增文件空白检查通过。没有启动玩家客户端、浏览器、HTTP 服务或 renderer Instance。

## 失败与独立复核

原失败完整保留：native01 在未预建 receiptRoot 时没有 forward；初始 Web runner 未加入 npc 组而拒绝入口。storage01 为旧夹具漏提取新增退休函数（47/48），adjacent01 为严格 guard 断言未列出新增 buyItem-only 分支（81/82），npc02 为漏写 legacy 固定数据函数（25/27）。修复只补对应真实提取、固定夹具和精确断言；没有删除原断言或改变产品行为以迁就测试。

Sol high 按单文件单写者实现规则和测试；Root 负责 Native/Web/Runtime/Gateway 接缝与最终验证。独立只读审查关闭原始字段默认补齐、商品覆盖顶层 panel、移动快路径漏 dialog 三处 P2。最终声明 Rust 输入 797/797 验证字节/哈希；本批审阅的 planner/消费者增量没有剩余确认 blocker，这不是完整购买生命周期验收。

QA 根目录：`C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-01`。
源码冻结 `root-source-snapshot02.json`：533 输入 = 15 个声明既有变更 + 514 个旧输入保护 + 4 个新增；前轮 529 输入中所有未声明路径字节不变。完整检查结果见 `root-verification01.json`，原日志和失败记录保留。

## 仍待完成

1. 共同 Rust selection/quantity/controller、Native/portable painter 与独立 Runtime/Web host；当前 Web 商店仍用 React 绘制。
2. Native 精确本地传输生命周期：旧 Buy{UID,count} 在确定未入 transport 时也可能留下锁；无限库存的旧 pending 不由目录或背包刷新释放，换数量还能绕过这个精确键。需要把 attempt token 贯穿到真实传输入口；DefinitelyUnsent 只取消对应尝试，Entered/Unknown 保留屏障。
3. 没有专用 BuyItem ACK。钱包、物品或目录变化只能作为观察，不能当作购买成功或触发自动重试；client service/catalog revision 也不证明服务端 NPC 来源。完整结果恢复和用户反馈仍未验收。
4. 服务端 key-based 容量预检考虑合并，而带耐久元数据的实际插入禁止合并，判断不一致；本批 planner 保守要求实际合法空格。服务端修复与对应回归单独排队。
5. resale/Pearl/Sell/Repair 沿用既有路径；is_shop_item 不独证 resale 来源，完全缺 marker 的旧 schema 仍属于 legacy 范围。
6. 新生产 renderer/Windows/Next/独立包组合、实际购买/登录战斗/保存重登、触屏与 Android/iOS 真机、最终 frontend/Candidate 保持 open。前轮 Progress60 组合产物属于其原冻结源码，不能继承为本批新购买接口的构建或玩家证据。
