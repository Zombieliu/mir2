# R16 双向物品与金币交割

2026-11-09 在 `codex/windows-player-journey` 验证。Web 基线 `e30af0be43`，修复 `94820563616863150721871c6abd4fb427c18ce3`。两版共用 `ac6817dbf9` 的 Gateway 和 `bevy-0cd68510886134e0` WebGPU。隔离 File 存档来自同一测试源档。启动前只给两个测试角色分别设置 1000 与 700 起始金币。物品来自两角色原背包。 [隔离档核验](fixture-proof.json) · [基线启动](baseline-startup.json) · [修复版启动](startup.json)

## 第一人确认后的界面

旧 Web 用当前背包投影自有报价。R16 Gateway 在第一人确认时把物品移入暂存，报价索引仍保留。旧页面此时显示空报价与 0 金币，且没有显示锁定。实际 Page 回归也表明此时按确认无法发出 Windows R16 同样使用的 `TradeConfirm { locked: false }`。 [旧版截图](baseline-prepared.png) · [旧版包序](baseline-prepared.json) · [原回归](old-prepared-lock.log)

修复版在确认发到当前连接时记住已验证报价。收到同一交易的权威暂存快照后保留原物品、金额与锁定状态。物品展示与连接、角色、场景、交易代次、nonce 和报价索引绑定。暂存报价只读，仍禁用物品和金币编辑。取消、结束和会话退出会清理缓存。 [修复版暂存截图](primary-prepared-with-quote.png)

## 双方实际资产

| 角色 | 初始金币 | 交出 | 收到 | 交割后金币 |
| --- | ---: | --- | --- | ---: |
| 主角 | 1000 | 蜡烛 ×1，137 金币 | 木剑 ×1，59 金币 | 922 |
| 对端 | 700 | 木剑 ×1，59 金币 | 蜡烛 ×1，137 金币 | 778 |

两轮普通 WebSocket 验证由对端先确认。双 WebGPU 界面验证由主角先确认，两个独立浏览器环境都用普通登录、走位、按 T、接受、放入、填写金币及确认。第一次确认没有入账或终结。第二次确认后，每端先收到一次金币和物品入账，再收到一次结束回包。两个接收方各自的旧物品标识发生冲突，Gateway 发出新的物品标识，与最终格位及数量相符。双方合计保持 1700 金币，正常注销后同角色读回一致，最终停止服务后 File 副本也精确匹配。 [普通协议](wire-proof.json) · [协议包序](wire-trace.json) · [双浏览器记录](browser-proof.json) · [结算后截图](primary-completed-wallet.png) · [落盘核验](durable-save.json)

随后另开一笔交易，第一人确认后点解锁。当前 R16 Gateway 以两端关闭交易结束暂存，退回一件布衣和 17 金币。再次比对双方资产，与前一笔已完成交易一致。固定 Crystal `92b4ce4` 的 `PlayerObject.TradeConfirm(false)` 将锁定清除并保留交易。已暂存时如何恢复到可编辑状态，仍是共用 Gateway 的源行为差异。 [解锁包序与回读](browser-proof.json)

## 复测范围

- 旧页面第一次冷双开的邀请命令有发送，但先进入的页面随后显示登录。修复页面的首次冷双开也有先进入角色回登录的记录，尚未定因。普通 WebSocket 从原坐标和朝向直接交割另有正向证据。完整双 WebGPU 结果来自已预热的本机开发服务，双方确认前实际走位重新面对。 [旧冷试](baseline-first-invitation.json) · [修复版冷试](fixed-cold-dual-entry.json) · [冷试截图](fixed-cold-return-to-login.png) · [原朝向普通协议](wire-no-turn-proof.json) · [原朝向包序](wire-no-turn-trace.json)
- Node 汇报 151 项通过，包含一条 Stage5 脚本结果，该脚本内部 507 组单列。TypeScript `--noEmit --incremental false` 退出 0。两端实际调用共用预算桥 58 和 55 次，网络、页面和控制台错误列表为空。 [回归日志](final-web-tests.log) · [浏览器记录](browser-proof.json)
- 正式游戏、Native 窗口、真实 PostgreSQL 和全局 Candidate/Accepted 另验。隔离浏览器、两次本机启动及两次独立协议服务已退出，测试源档保留。 [清理核验](cleanup-verified.json) · [浏览器探针](browser.mjs) · [资产核对](receipt-lib.mjs) · [普通协议探针](protocol.mjs) · [隔离启动器](launch.mjs)
