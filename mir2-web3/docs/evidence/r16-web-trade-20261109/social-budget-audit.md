# R16 Web 跨社交共用预算审计（未结）

日期：2026-11-09。代码基准：`codex/windows-player-journey`，HEAD `6d30e0f9e5`，以及本轮正在整合的 Web 工作树。本文只记录源码生命周期映射与内存复现，**没有实现共用预算，没有产生或验收新 Candidate**。交易 Cancel 的产品修改由另一个 worker 独占；本文不替代该修改的测试收据。

本轮未启动网关、浏览器或模型，未连接网络，未操作账号、存档或服务器。产品文件没有由本审计修改。下列行号是审计时定位，后续整合以所列函数名为准。

## 已观察到的差异：8 个组队请求后仍可进入第 9 个公会请求

Native 的 [SocialPendingOperation](../../../apps/game-client/client-bevy/src/social.rs#L303) 当前有 **20 个枚举变体**，全部经过同一个 `SocialModel.pending`。`MAX_SOCIAL_PENDING = 8`，`begin_pending` 在第 9 个操作前拒绝，见 [social.rs:30](../../../apps/game-client/client-bevy/src/social.rs#L30) 与 [social.rs:405](../../../apps/game-client/client-bevy/src/social.rs#L405)。

Web 的 [GroupOperations](../../../apps/web/lib/group-operations.ts#L20) 只有自己的 8 条子队列；[GuildRankOperations](../../../apps/web/lib/guild-rank-operations.ts#L105) 使用另一个 Map。二者没有共用计数。实际 Page 在 `sendRaw` 中分别复核、claim 各自 proof 后发送，没有跨社交总容量判断，见 [page.tsx:6490](../../../apps/web/app/page.tsx#L6490)。

外置复现加载并在内存中 transpile **真实 TS 模块**，不复制实现、不创建 socket、不执行 Page 或 Rust。输入均为虚构值：

```js
owner = {
  socket: {}, connectionGeneration: 1, sessionGeneration: 2,
  sceneRevision: 3, playerObjectId: 1000, mapFileName: "0"
};
groupSource = { owner, playerName: "Leader", members: [] };
guildSource = {
  owner, scope: {}, guildName: "Guild", actor: "Leader", ownRankIndex: 0,
  permissions: ["CanChangeRank"],
  ranks: [
    { index: 0, name: "Leader", permissions: ["CanChangeRank"] },
    { index: 2, name: "Members", permissions: ["CanRecruit"] }
  ]
};
```

调用顺序：

1. 对 `Absent0` 至 `Absent7`，依次调用 `group.reserve("addMember", name, groupSource)`，再调用真实 `group.claim(proof, groupSource, groupOperationCommand(proof))`。8 次均进入。
2. 同一个 GroupOperations 的第 9 个名称被拒绝，证明原组队子队列上限确实生效。
3. 同 physical owner 下调用 `guild.reservePermission(2, "CanStoreItem", true, guildSource, 1000)`，再 `guild.claim(proof, guildSource, proof.command, 1000)`，仍然进入。期间不提供任何 ACK。

实际收据：

```json
{
  "status": "open",
  "parityPassed": false,
  "observed": {
    "groupEntered": 8,
    "sameQueueNinthRejected": true,
    "ninthGuildPermissionEntered": true,
    "combinedUnresolved": 9,
    "noAcknowledgementsSupplied": true
  },
  "nativeLimit": 8
}
```

外置产物目录名为 `social-budget-audit-r1`：`reproduce.mjs` 与 `observation-r1.json`。复现命令不依赖固定工作盘：

```text
node <external-audit-directory>/reproduce.mjs <repo-root> <external-audit-directory>/observation-r1.json
```

`node --check` 已通过；实际复现退出码为 **1**，表示 `9 > 8` 的未对齐结果，不能倒置成常绿产品测试。重新运行必须用新收据文件名；脚本使用独占创建，不覆盖旧证据。

边界：这不是实际 Page/E2E 运行。Page 的 permission `finally` 随后会删除其本地节流行，但这不是服务器 ACK，也不否定第 9 次发送准入缺少总预算的问题。Native 侧本轮没有执行 Rust；比较依据是实际常量、统一准入实现，以及已存在的混合操作回归 [social_guild_pending_tests.rs:275](../../../apps/game-client/client-bevy/src/social_guild_pending_tests.rs#L275)。

复现读取的精确源摘要：

| 源文件 | SHA-256 |
|---|---|
| `apps/game-client/client-bevy/src/social.rs` | `17dd0a98c36f2cae7a1c6a8ee5d81cc3470adb6530e287820b247764cfc7cf2b` |
| `apps/web/lib/group-operations.ts` | `08fce78a67fef84c380bb4084aa052bfb86f7e9957f2104fbe1b9440ecc2c8ea` |
| `apps/web/lib/guild-rank-operations.ts` | `172ba45b0f074657cb6f416d0a6337fe38baf2e7b93f87d902bf849edec304ee` |
| `apps/web/lib/social-parity-actions.ts` | `5acb357f9df207a9ad5ddf852799ab2f6f8e7aa7eff18d8bb733db00959c016d` |

## 全部 20 变体与当前 Web 生命周期

Native 的真实 UI 准入映射在 [overlays.rs:3473](../../../apps/game-client/client-bevy/src/crystal_ui/overlays.rs#L3473)；typed rename、permission、roster read 使用其相邻专用入口。Windows [gameplay_bridge.rs](../../../apps/game-client/platform-windows/src/gameplay_bridge.rs#L3827) 在实际发送前复核 typed 请求并释放确定未发项。Native 的完成判别集中在 [social.rs:1143](../../../apps/game-client/client-bevy/src/social.rs#L1143)。

以下“缺少 pending”只表示缺少覆盖发送后到权威结果之间的持久本地操作项，不表示没有权限、owner、窗口或最终发送校验。

### Group：4 变体

| Native 变体 | Web 当前队列 / 入口 | 真实释放证据与未结项 |
|---|---|---|
| `GroupSwitch { allow_group }` | Page `groupLeave` / `groupToggleAllowInvites` 直接 `send`，没有对应持久 pending；约 11947 | `SwitchGroup` 的精确 bool 回显。`DeleteGroup` 不能替代其它组队操作的 ACK。 |
| `GroupAdd { name }` | `groupOperationsRef` → `submitGroupOperation`；独立 Map 最多 8，约 11919 | 同 physical owner 的同名 `AddMember`，或实际 `GroupMemberInfo` 包含该名。普通 snapshot、Chat、SwitchGroup 不释放。 |
| `GroupRemove { name }` | 同一 `GroupOperations`；精确 op/name 防重 | 同名 `DeleteMember`，或实际完整 roster 不再包含该名。地图/关窗不退休。 |
| `GroupInviteAccept { inviter, invite_epoch }` | `socialRepliesRef` 的 group 请求与 epoch → `replySocialRequest`；[social-incoming-replies.ts:40](../../../apps/web/lib/social-incoming-replies.ts#L40) | 目前 claim 消费 UI 请求，send 成功后即 `finish`，并没有等待结果的预算行。Native 依实际加入、邀请替换/撤销等收敛；接受请求必须保留该上下文。拒绝邀请 `acceptInvite:false` 不属于此 Native pending 变体。 |

Group ledger 的现有 matching/unknown/retire 规则见 [group-operations.ts:123](../../../apps/web/lib/group-operations.ts#L123)。

### Guild：9 变体

| Native 变体 | Web 当前队列 / 入口 | 真实释放证据与未结项 |
|---|---|---|
| `GuildInfo` | legacy Native 变体；Web 普通刷新走下一行 | Native 新 UI 入口把裸 GuildRequestInfo 转 typed read。预算不能让 legacy 与 typed 对同 guild 的一个读取双占槽。 |
| `GuildRosterRead(request)` | `guildRankReadRef` → `requestGuildRankRoster`，约 Page:12627 | 同 owner/guild/scope 下新鲜、有效的 status255 full roster。应保存发送前 member revision；新的只读回复允许内容相同，重复使用旧投影不算新回复。 |
| `GuildMember { change_type, rank_index, name }` | 招募0、踢人1、移阶2经过 `sendSocialRosterCommand`，没有持久 pending；创建4由 `GuildRankOperations` 独立处理 | 踢人：status3 + 同 name；移阶：status5 + 同 name/目标 rank，或真实自己的 status8。Web 创建已有严格 status6 + 完整真实 readback。**Native generic 招募0/创建4当前仍缺精确完成判别**；不能用任意255假 ACK，也不能把本地预算工作宣称为修好了这两个合同。 |
| `GuildRankRename(request)` | `guildRankOperationsRef` 的 rename 行 | 实际 status7、同 guild/scope/actor、嵌套 `ranks[0].index` 和预期名字。Native typed 请求还保存观察 change revision，旧 cursor/重复投影不能确认新请求。 |
| `GuildRankPermission(request)` | 同 adapter；Page `submitGuildRankEdit` finally 调用 `finishPermissionAttempt`，约 12600 | **该方法删除 entered permission 行只是结束输入节流，不是 ACK**。共用预算须另存 option/expectedBool/actor/scope/观察 revision，等待真实匹配的 status7。300ms 不释放预算。 |
| `GuildNotice { notice }` | `editGuildNotice` 直接 `send`，约 Page:11860，没有持久 pending | 现 wire `GuildNoticeChange.update < 0` 是无替换正文的终结回复；或 Native 已有的请求内容与实际改变后正文匹配。不能任何公告广播都清任意请求。 |
| `GuildInviteAccept { inviter, invite_epoch }` | `socialRepliesRef` 的 guild epoch | 与 group reply 相同，目前只有一次提交租约。应由真实入会/邀请替换/撤销收敛；guild scope 退休是取消，不标成功。拒绝邀请不进入此 Native 变体。 |
| `GuildStorageGold { change_type, amount }` | `guildDepositGold/guildWithdrawGold` 直接 `send`，约 Page:12552，无持久 pending | 精确 `GuildStorageGoldChange` 的 amount 与请求 type，0/1 的失败对应3/4。余额广播本身、其它金额不是本项 ACK。 |
| `GuildStorageItem { change_type, from, to }` | `socialItemOperationsRef` 的单项屏障，跨 trade/guild 两个物品表面 | [social-window-operations.ts:167](../../../apps/web/lib/social-window-operations.ts#L167) 校验 tuple、user、需要时 UID；0..2 成功、3..5 失败。原物品屏障继续等待 matching ACK 后的同 owner 完整 snapshot。Native `changeType > 2` 的只读刷新不占此 pending 预算。 |

职级创建/改名/选项仍须保留各自 proof，不能仅由共用预算授权。职级 scope 与权限来源在 Page `readGuildRankEditSource`；成员增量先经 [guild-rank-projection.ts:152](../../../apps/web/lib/guild-rank-projection.ts#L152) 合法投影，再处理预算回执。

### Trade：7 变体

| Native 变体 | Web 当前队列 / 入口 | 真实释放证据与未结项 |
|---|---|---|
| `TradeRequest` | Trade 快捷键/窗口直接 `send({type:"tradeRequest"})`，无持久 pending | Native 要求实际 `TradeRequest/TradeAccept` 状态转换；不能 send 返回 true 就结束预算。 |
| `TradeReply` | `socialTradeReplySpentRef` 保留 incarnation 的一次提交标记；`socialTradeSendingRef` 只覆盖同步发送 | Native 此变体是 unit，不能按 accept bool 分成两个并行名额。实际状态转换或该 exchange 终结释放；socket 异常不能解除消费标记。 |
| `TradeGold { amount }` | `sendTradeUiCommand` 的同步 proof，尚无持续到 ACK 的金额 pending | 同 owner/partner/open revision/nonce 的**自己报价**快照精确正增量；客方 `TradeGold` 绝不是自己报价 ACK。 |
| `TradeDeposit { from, to }` | `socialItemOperationsRef` | 精确 `DepositTradeItem` 的 tuple + success（含 false）；保留原经济 owner 与随后完整 snapshot 屏障。 |
| `TradeRetrieve { from, to }` | 同上 | 精确 `RetrieveTradeItem` 的 tuple + success（含 false），不能其它格回包解锁。 |
| `TradeConfirm { locked }` | 同步 trade proof，尚无独立持久预算行 | Native 由终结或合法解锁/报价/物品变化收敛；不能用自己的 send 成功代替确认，不得拿 guest Gold 当 own Gold 的完成。 |
| `TradeCancel` | 本轮并行修复已有 `socialTradeCancelSpentRef`、`socialTradeItemOperationRef` 与同 exchange 物品绑定 | 不在本文验收其新实现。总预算后续仍需按真实终结取消/完成清对应 trade；`TradeCancel.unlock=true` 仅解锁，不结束 exchange、不清全部 trade pending。 |

自己报价的真实 Native 投影是 [trade_projection.rs:27](../../../apps/game-client/platform-windows/src/trade_projection.rs#L27)，完全相同的快照不产生新的 `NativeOwnTradeSnapshot`。Web 已有 `socialOwnTradeRef`，由同 owner 完整 snapshot 捕获，约 Page:18456，可以在此基础上复用同一只读语义。

## 真实 ACK 的可用范围

已有可关联的失败结果包括：

- `GuildStorageGoldChange` 的3/4，对应请求0/1，且 amount 必须相同。
- `GuildStorageItemChange` 的3..5，对应请求0..2，且 tuple/user/UID 等原规则必须保留。
- `DepositTradeItem/RetrieveTradeItem.success=false` 的同 tuple 失败。

源码搜索 `packages/protocol/src`、`apps/gateway/src`、`apps/simulation/src` 没有 `GroupInviteResult`、`GuildInviteResult`、`GuildNoticeResult` 的发送实现。这三个名称目前是 Native SocialModel 的兼容解析与 fixture 能力，**不能列为当前服务已经提供的失败 ACK**。

组队找不到目标、权限/邀请拒绝等路径由 [shared_groups.rs:299](../../../apps/gateway/src/shared_groups.rs#L299) 返回系统消息；其生产调用经过 `shared_social_message`。文案不是 request ID/owner receipt，不能解析 Chat 中的人名来猜某个请求失败。服务端 invite 节流还有直接不返回新 packet 的路径。时间经过同样不能证明请求结果。

现协议没有覆盖全部社交操作的统一 request ID。新鲜、匹配的状态证据只能证明所观察状态满足对应合同，不能伪称已经得到可区分任意迟到同名旧请求的 RPC 回执。失败 ACK/招募及创建的 Native typed 合同仍需另列工作，不能借本地预算补造。

## 三个生命周期陷阱

1. **UI 退休不等于物理会话退休。** Native [clear_scene](../../../apps/game-client/client-bevy/src/social.rs#L572) 保留全部社交 readmodel/pending，[clear_session](../../../apps/game-client/client-bevy/src/social.rs#L568) 才清全部。Web `retireSocialRequests` 会在 MapChanged/关闭邀请窗执行，不能在它里面顺便清新预算。全量清理需绑定真实 socket/connection/session/player 退休；guild scope 变化仅取消该 guild，交易终结仅取消对应 exchange。Page `retireEquipmentSession` 是核对实际角色终结接线的入口，不能只因某函数名含 retire 就沿用其所有调用点。
2. **发送阶段的局部清理不是 ACK。** `SocialIncomingReplies.finish`、permission 的 `finishPermissionAttempt`、`socialTradeSendingRef` finally 只处理提交/节流租约；不能删除共用预算的 entered 行。物品 ledger 的 ACK 与后续完整 snapshot 是两个阶段，预算结算也不能撤掉原经济屏障或授权下一物品操作。
3. **重复投影/重复 Accept 不能制造完成。** Web 当前 TradeRequest/TradeAccept handler 每次增加 incarnation，不能不经状态核对就拿这个变化清旧预算。Native 对重复 Accept 保留当前 offer，见 [social_trade_tests.rs:189](../../../apps/game-client/client-bevy/src/social_trade_tests.rs#L189)。预算应保存请求前的有效 readmodel/receipt cursor，仅处理其后接受的 matching packet 或合法自己报价变化；不能重新发布一个旧 projection 就产生 ACK。只读 roster 的新255回复允许内容不变，不能错误要求每次正文变化。

## 最小逐步接入方案（提案，未实施）

1. 新增一个纯本地 `SocialOperationBudget`，先完整列出20变体的命令分类、逻辑去重键和 scope；`GuildInfo/GuildRosterRead` 处理同一读取的 alias 关系。保留现有 Group、GuildRank、SocialIncomingReplies、SocialWindowOperations 等 proof。好友、排名、婚姻/离婚/师徒、出租、公会 Buff 等不属于这20变体；例如 Native social-bond 有自己的 [pending](../../../apps/game-client/client-bevy/src/crystal_ui/social_bond_dialog.rs#L143)，不能因为 Web 共用了 helper 就扩大8槽范围。
2. 在 `sendRaw` 最终路径、**任何相关 social proof claim 之前**先预留总容量；reserved 同样占槽防同步重入。所有既有 owner/权限/DTO/window/layout/source 复核仍完整保留，最后一次实际 socket 尝试前提交预算。容量满应在原 proof 变为 entered 前拒绝。后续明确未发只归还这一预留；进入 `socket.send` 后的异常保留 unknown，不自动重发、不排队补发、不增加 wire 字段。
3. 由现有合法 packet/readmodel 接收点统一处理匹配 receipt，保存发送前 revision、真实物理 owner、guild scope/actor/rank/bit/expected value 或 trade exchange/tuple。不能按任意广播、同名 Chat、任意255、同步 send 返回值或输入节流结束结算。预算项和现有经济/布局屏障各守自己的合同。
4. 逐批补单元与真实 Page AST 生命周期测试；阶段可分 Group/邀请、Guild、Trade，但**全部20分类与生命周期接线完成前，不宣称共用8完成**。可先保留不明结果占槽直到真正终结，不能为方便补全而发明 ACK。之后由负责人安排普通 UI 验收，不能以本轮模块复现代替。

禁止新增通用 timeout 解锁。唯一已存在的 Native 特例是 [expire_guild_rank_renames](../../../apps/game-client/client-bevy/src/social.rs#L558)：严格超过5秒允许 rename 重试，不宣称成功；Web现有同例外。300ms permission 与5秒 create 是输入节流，不是失败/成功回执。若预算实施时决定调整这个 rename 特例，必须单独说明，不能称已无差异对齐。

## 可复用测试接缝与验收门槛

- [test-group-operations.mjs](../../../apps/web/scripts/test-group-operations.mjs)：精确名称/op、8条子队列、unknown、地图保留、错误与无变化 roster 不解锁。
- [test-guild-rank-operations.mjs](../../../apps/web/scripts/test-guild-rank-operations.mjs) 与 [test-guild-rank-create.mjs](../../../apps/web/scripts/test-guild-rank-create.mjs)：typed proof、actor/rank/bit/scope、严格6及完整 readback；补跨队列容量测试不能只检查它们各自的 Map。
- [test-storage-page-gate.mjs](../../../apps/web/scripts/test-storage-page-gate.mjs)：执行真实 Page AST，可注入最终 socket 前 listener 重入、owner/scene替换、实际 packet 和完整 snapshot。本轮由交易 Cancel worker 独占；后续扩测必须等其释放。
- [social_guild_pending_tests.rs:275](../../../apps/game-client/client-bevy/src/social_guild_pending_tests.rs#L275)：Native 混合8槽；同叶 typed permission/rename/roster cursor 回归可用作对照。
- [social_trade_tests.rs](../../../apps/game-client/client-bevy/src/social_trade_tests.rs)：自己报价精确增量、同 partner 新 exchange、重复 Accept、unlock 与终结只清 trade；[guild_permission_wire_tests.rs](../../../apps/game-client/platform-windows/src/guild_permission_wire_tests.rs#L53) 覆盖 Native bounded sender 确定未发的归还。

下一实现至少应证明：跨 Group/Guild/Trade 混合满8后第9项在原 proof entry 前拒绝；确定未发归还；send unknown 保留；匹配成功/失败恰好归还对应项；重复回包/旧投影/foreign owner 不解锁；map/关窗保留；session 清全部；guild/trade 终结只清所属范围。原物品 ACK→完整 snapshot、UID/布局/权限检查不得减少。

本轮只执行外置模块复现及其语法检查，没有重新运行上述仓库测试，没有 Cargo、UI 或公网验收。`parityPassed:false` 是应保留的开放差异证据。
