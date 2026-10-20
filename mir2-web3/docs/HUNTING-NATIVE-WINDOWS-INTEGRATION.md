# 狩猎经济：Windows 原生客户端接入

2026-10-20：用户明确要求检查游戏的 Windows 客户端。后续优先验收
`apps/game-client/platform-windows` 的 Bevy EXE。Windows 上直接运行的
Next/Gateway、浏览器截图和 Tauri WebView 均不能代替这一验收。

## 当前缺口

- `platform-windows/src/gameplay_bridge.rs` 的 RepairItem/SRepairItem 仍映射到
  `native_protocol.rs` 的旧 `repairItem/specialRepairItem` UID 指令。
- `client-bevy/src/crystal_ui/npc_item_service.rs` 仍使用本地 float 费率及
  `inventory.gold`，Hold 可以直接提交。新绑定余额不能沿用这个判断。
- 原生 host 尚未消费 `realmInfo.npcRepairMode`、`identitySession` 和
  `huntingRepairOwner`。共享 client-wire 已有 DTO 不代表原生端已接入。
- 当前树没有已核验包含这些新功能的 Windows 候选包。历史候选摘要、旧 EXE
  或重新打包旧 EXE均不能证明本轮 Windows 支持。

## 完整纵向切片

1. 从原始物理连接读取并锁定 realm 的 repair mode；hunting 声明不能因政策
   关闭而降级 legacy。账号、角色、连接及 scene/procedure stamp 必须保持关联。
2. 普通 NPC 服务及真实完整 Bag witness 只允许请求 Quote。停用 hunting 下
   的 Hold 自动确认、本地旧金币门槛及本地计算的经济费用。
3. 显示服务端绑定费用；玩家明确 Confirm 当前 quote。最终 proof 检查和
   `start_send` 应在现有同一短临界区，不能把旧 UID 指令改名作为 Confirm。
4. Confirm 进入发送后至多一次。超时、断线或未知结果只能 Query 原 quote，
   关闭窗口不能丢失 Pending。换角和旧物理连接的回执不能应用到新角色。
5. 严格解码原始 owner frame，并先关联 request/quote/原 stamp，再预解码本次
   完整 snapshot。收到、入队、主线程实际应用是三个不同状态。
6. 主线程一次替换完整模型并返回独立、不可伪造的 Applied。null/坏 snapshot
   或应用失败保留待同步状态。成功后显示只读完成，普通 Close 清除展示。
7. 新钱包单独观察：真实 identitySession grant 私有绑定连接/账号/session/时效，
   只读固定钱包接口；余额不能作为 Confirm 授权，不能回退到旧 gold。

## 复用入口与边界

| 入口 | 可复用范围 |
| --- | --- |
| platform-windows/src/gateway.rs 的 NativeCommandStamp、最终发送锁 | 物理 connection/procedure/owner/scene/cancellation 及发送退休检查 |
| platform-windows/src/npc_purchase_gateway.rs | 原始 marker 分流、fresh 投影、主线程实际 Applied 的接线模式 |
| platform-windows/src/npc_purchase_projection.rs | 全量模型预解码；repair 的 nativeNpcShop=null 可自然清退旧 NPC 权限 |
| runtime/src/npc_purchase_economy.rs | 完整模型原子应用与队列 barrier 的结构 |
| client-wire/src/hunting_repair.rs | Quote/Confirm/Query 与严格 raw frame 解码和关联 |

修理协议没有购买的 Begin、ActorKey 或 producer；必须使用独立 repair custody
和 gate。禁止制造 PurchaseBinding、服务器 revision 或购买确认凭据。

建议三写集：transport owner 独占 gateway.rs/gameplay_bridge.rs/main.rs；runtime
owner 负责独立 repair bundle/gate 与 native_ingest/调度；UI owner 负责独立模型、
npc_item_service 与现有 overlay。共享文件只能由一人整合，先约定接口再开写。

## 原生验证

- 显式使用已安装的 `+1.95.0`，`--locked --offline`，Cargo jobs=2。
- 使用本聊天独立 CARGO_TARGET_DIR，保留原主树缓存、旧 EXE、HardLink 和运行中
  的游戏；链接使用新的输出名。不得启动 Docker/WSL/虚拟机或改虚拟化设置。
- 基线 check 不是新功能验收、EXE 链接、候选打包或玩家视觉验收。
- 原生启动还要核验资源闭包，包括真实 `0.map.gz`；Web 地图可见不能证明原生
  布局资源齐全。不得编造地图或借 QA teleport 代替真实流程。
- 新 EXE 接本机隔离 PG fixture，普通登录/NPC/拖入/报价/取消/确认、Pending
  关窗/原 Query、完整应用、普通保存和新登录。独立核对一条结果、一条销毁、
  一次扣款与保存耐久。服务端 OutcomeUnknown 的强制故障另作验收。

本计划属于仍 active 的完整狩猎经济 goal；默认关闭、K=0.1、0.72/1.08 许可
基准、旧 gold 定性和远程资产保持。没有 Windows 支持或 Candidate 百分比结论。
