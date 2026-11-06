# Windows 真实三单交付验收

2026-10-28 至 29 日（Asia/Shanghai），协调者使用原生 Windows 客户端、普通已认证测试账号和真实 File 游戏资产权威，完成 NPC 发布、跨玩家交货、离线待领、双方领取、取消退款与 120 秒真实到期退款。测试网使用隔离测试签名者和测试币，没有主网、生产收费或生产密钥。

[独立三单核验](evidence/live-three-scenarios-independent-audit.json) `passed=true`，含最终重登的 18 份阶段投影和 9 笔真实最终交易均绑定对应订单。这是 `custodyFixtureOnly=false` 的真实游戏资产闭环，与 [早期独立链上夹具](../../../../onchain/procurement/validation/20261028/README.md) 分开；Rust Mock、PGlite 和离线 GPU 也分别保存。

## 实际版本

- [Gateway process](process/server-process.json)：v5 SHA256 `4b5bc8a10793583e0631ab9145862d86ad209c8a58f1481f1875f6892569060c`。
- [Windows 买家 process](process/client-final-buyer-process.json) / [卖家 process](process/client-final-seller-process.json)：v3 SHA256 `1a8c941bac3ae9be03bf64652dc64b555d8699a6a65bd65c9ca532da6b0bc144`。
- 发布和 Open 重启恢复等较早步骤使用保留的 v3/v4 Gateway 中间构建；最终 v5 修正采购后 HUD/背包权威快照，继续处理原订单，没有重新发布扣款。
- process 仅含测试进程、程序 SHA、本地 endpoint 与角色元数据；公开投影仅含本功能所需资产/ledger，不复制账户整库、密码、钱包/Oracle 私钥、env 或依赖目录。

## 真实订单、结果与 Gas

| 路径 | 订单 ID | 最终游戏结果 | 交易 / 净 MIST |
| --- | --- | --- | ---: |
| 发布→成交 | `0x70cdaa52f85d1e1e787fa40749cf5f3ad72245a6399fdfdc24caa8a30807a14f` | Filled；买家离线时原件持久待领，随后装备入普通背包；卖家 500,000 金币已领取 | 3 / 34,100,931 |
| 发布→取消 | `0x243e18275fd667bc4d29082c906bfe963b021f9b942a47bde3a402b427452d11` | Cancelled；原 500,000 金币退款已领取，提交待确认阶段仍有托管 | 3 / 34,100,418 |
| 120 秒发布→到期 | `0xcbe276110159616beafbe56a648b2e117518e04bc7c6ead04ec5aa440421de49` | 激活成功后 Expired，非 PublishFailed；500,000 金币退款已领取 | 3 / 34,100,418 |

[成交三笔](evidence/chain-fill-three-transactions.json)、[取消三笔](evidence/chain-cancel-three-transactions.json)、[到期三笔](evidence/chain-expire-three-transactions.json) 保存全部 digest、checkpoint、effects、事件与 Gas。每单 create + activate 为两笔发布，终态再一笔。净 Gas 合计 **102,301,767 MIST**，公式 `computationCost + storageCost - storageRebate`，不重复加 nonRefundableStorageFee，不含部署、早期夹具和服务器/RPC/索引成本，也不证明美元利润。

## 资产与额度

- 18 阶段总金币均为 **2,011,280**。最终买家 **1,500,000**、卖家 **510,000**；其余原有账号 **1,280** 未变，托管 **0**、待到账金币 **0**。
- template **245** / 原 UID **1028024501** 的有效归属始终一处，最终为买家背包；历史 released lock、claimed receipt 保留审计副本，不计可用拥有者。
- 耐久 **31400/34000**、强化值 **1**、强化附加 **2**、MinDC +2 / MaxDC +3，以及全部已记录实例字段保留。锁中原始 JSON SHA256 `7bfb7db030cfac6479a8fda3e18d8bc6bd2452a2b060d7d1d64aec5b01c63579` 匹配链上 asset hash；全实例规范化字段 SHA256 `9132a6c0cf9e76df2653996d435ae2cd8765f60460d04a76de29a686e1365b3d` 前后一致。见 [成交独立核验](evidence/live-fill-independent-audit.json)。
- 显式隔离预置 Test 10，加 NPC 免费 Test 套餐 10，共 **20**；三次发布消费 **3**、预留 **0**、可用 **17**。取消/到期不退已经完成的发布额度，符合冻结规则；没有生产点券销售或现金收入。
- 结清证据为 [expiry-refund-claimed-final.json](evidence/expiry-refund-claimed-final.json)。双方最终再次重登已通过，[第 18 份快照](evidence/final-after-both-relogin.json) 再确认总金币 2,011,280、买家 1,500,000 / 卖家 510,000、托管/待到账 0、原 UID 仅归买家、三单终态/全部领取和 Test 消费 3 不变。

## 画面与阶段关联

本批 **27 张 JPEG** 是电脑控制保存的真实目标 Windows 窗口画面，原始 `*.jpg.json` sidecar 记录窗口标识、程序、时间和尺寸。它们不是 native capture PNG，也不是 Web 或离线模型渲染。原始 native capture 目录为空，没有编造 PNG。

[snapshot-associations.json](screenshots/snapshot-associations.json) 逐张关联 JPEG、原始 sidecar、订单和公开阶段投影的 raw SHA。图片和投影分开采集，是同一步骤的关联，不是原子截图/数据库事务证据；资产结果仍由回执与独立核验确定。

主要画面：[NPC](screenshots/02-final-npc-entry.jpg)、[发布确认](screenshots/03-final-publish-confirm.jpg)、[原件交货确认](screenshots/10-real-equipment-delivery-quote.jpg)、[原件待领](screenshots/14-buyer-original-uid-pending.jpg)、[普通背包](screenshots/16-buyer-normal-inventory.jpg)、[原件 tooltip](screenshots/17-buyer-original-equipment-tooltip.jpg)、[取消退款](screenshots/21b-cancel-refund-visible.jpg)、[120 秒到期确认](screenshots/22-expiry-120s-explicit-quote.jpg)、[全部到账](screenshots/24-all-deliveries-and-refunds-received.jpg)。

最终重登：[买家背包与 1,500,000 金币](screenshots/25-final-buyer-relogin-inventory.jpg)、[卖家空背包与 510,000 金币](screenshots/26-final-seller-relogin-inventory.jpg)。最新 process 元数据对应买家 PID 68868、卖家 PID 84368；之前的不同 process 凭据保留在 `process/before-final-relogin/`。

较早的报价拒绝、重启要求登录等画面是保留的联调反证；NPC 分页、来源精确 test、状态 Open/open、HUD/背包快照四项修复详见 [主题文档](../../../PERSONAL-PROCUREMENT-V1.zh-CN.md)。

## 覆盖限制

这证明隔离 File 权威上的三条真实自定义功能闭环，包含 Open 跨重启保留和买家离线待领。双方分别领取不等于实际重发同一领取请求的幂等测试。并发交货、伪造、重放、提交未知、背包满等仍按单独 Rust/Mock 证据陈述。

没有把此结果升级为人类前端 **Accepted**、原生 PostgreSQL/libpq、多 Gateway 并发、长期崩溃恢复、生产密钥治理、生产收费或盈利验收。原始文本/图片按 raw SHA256 保留，源码哈希按清单声明的 LF 规则规范化。
