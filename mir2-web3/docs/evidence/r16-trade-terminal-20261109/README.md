# R16 交易终结与双角色读档

2026-11-09 在 `codex/windows-player-journey` 的 Windows R16 共用 Gateway 上验证。基线 `8cd766e53eede54e8c79c93fbb2ec860fb7fab90`，修复 `ac6817dbf95ab1e5d1e98f78a9dd88b6885a8731`。两个二进制均为本机 Windows、Rust 1.95、锁定依赖的 dev 构建，保留在隔离测试目录。

| 二进制 | SHA-256 |
| --- | --- |
| 基线 `baseline-gateway.exe` | `d22478391fc3bf6bf89ff4757a72de8dc601c0800660d30fe7009de0d9ebfb31` |
| 修复 `fixed-gateway.exe` | `60dbb6b2ad9df5165dc1472dc64c7c0281e6094a1c1208c161dfccf35f5ea974` |

修复版先从基线加本次两个 Rust 文件构建，随后将同一改动提交。普通协议探针里的 `sourceHead` 是试验启动时的 Git HEAD。二进制由 `binarySha256` 区分，浏览器启动收据另记录修复提交。

## 服务端顺序

Crystal 固定参考 `92b4ce4` 的 `PlayerObject.TradeReply` 和 `TradeCancel` 向双方发送接受与结束包。R16 在受邀方快速接受、编辑和取消时，发起方的个人交易还可能在待初始化队列里。旧 Gateway 丢弃待初始化项，随后个人取消对尚未打开的交易不产生终结包。此时对端已排队收到接受与报价，后来的 `trade=null` 快照留下协议状态未决。

当对端仍待初始化，修复版将 `TradeCancel { unlock: false }` 追加到同一个有序 Gateway 队列。已初始化的交易沿用个人取消、资产退回和准备态协调。新回归在基线看到 `TradeAccept → TradeItem` 后失败，修复版验证 `TradeAccept → TradeItem → TradeCancel`、单一终结和资产精确。 [基线回归](baseline-regression.log) · [修复回归](fixed-trade-tests.log)

## 两个普通 WebSocket 角色

两方使用正常登录、选角、互相面对和交易命令。各次单独克隆同一个隔离 File 账户档并使用新恢复目录。表中序号是探针记录的双端总顺序。选取关键回包保留原 JSON 字节，较大报价和快照记录原字节哈希与必要字段，登录凭证、私有档案和完整快照未纳入证据。

| 场景 | 实际结果 | 证据 |
| --- | --- | --- |
| 旧版，默认节拍，取回后取消 | 发起方在序号 16 收到接受，19 收到空交易快照，缺少终结且保持未决。双方资产和同角重登入档正常 | [结果](wire/baseline-default/proof.json) · [包序](wire/baseline-default/wire-trace.json) |
| 旧版，5 秒节拍，取回后取消 | 同样缺少终结且保持未决。第一次启动复制了绑定原目录的恢复所有权元数据而退出，重试使用新恢复目录 | [首次启动失败](wire/baseline-observe/proof.json) · [重试结果](wire/baseline-observe-r2/proof.json) · [包序](wire/baseline-observe-r2/wire-trace.json) |
| 修复版，默认节拍，取回后取消 | 发起方 16 接受、17/18 报价、19 终结、20 空快照。两个普通连接均关闭交易，原连接能重新发起并取消，双方同角重登的物品标识、格位、数量和金币正确。已取回的蜡烛在所选背包索引 4 | [结果](wire/fixed-default/proof.json) · [包序](wire/fixed-default/wire-trace.json) |
| 修复版，5 秒节拍，放入后取消 | 发起方 13 接受、14 报价、15 终结、16 空快照。双方资产恢复，同连接重新交易及同角重登通过 | [结果](wire/fixed-early-deposit/proof.json) · [包序](wire/fixed-early-deposit/wire-trace.json) |
| 修复版，实际交割一件蜡烛 | 第一次确认只进入准备态。第二次确认后收件方收到一次 `GainedItem`，随后双方各收到一个最终 `TradeConfirm`。旧物品标识重合时收件方获得新标识，自己的原物品保留。双方背包、金币与普通注销后同角读档精确一致 | [结果](wire/fixed-settlement-r2/proof.json) · [包序](wire/fixed-settlement-r2/wire-trace.json) |

交割第一次探针错误地把发送方旧标识当成收件方新标识，资产断言失败，尽管两端已收到实际终结包。重试按 `GainedItem` 的权威标识定位，并检查收件方原物品未变。两次输出均保留。 [首次交割断言失败](wire/fixed-settlement/proof.json) · [原包序](wire/fixed-settlement/wire-trace.json) · [最终探针](probe.mjs)

## WebGPU 界面

复用上一轮 [Page、桥与真实 WASM 固定件](../r16-social-budget-20261109/README.md)，使用本次精确修复版 Gateway。主角通过可见界面操作，另一普通角色经真实 WebSocket 响应。只延迟各场匹配的成功物品回包，分别为放入 788 ms 和取回 599 ms。主角在等待时点击取消，双方交易都关闭，迟到原回包释放后窗口没有重开，也没有重复经济指令。双方资产符合放入退回与取回换格的预期，双方正常退出均有回执。本轮重新读档的证据在前面的普通协议场景。

交易阶段观测到 `processMir2SocialBudget` 实际执行 43 次，未注入游戏动作或替换桥。浏览器网络、页面和控制台错误列表为空。公会只覆盖无会籍时的只读门控。截图仍可见中文界面混用英语物品、按钮和提示，语言对齐继续追踪。旧浏览器的 `PEER_TRADE_NOT_CLOSED` 保留在原证据中。 [原失败](../r16-social-budget-20261109/ui/browser-r3/proof.json) · [启动来源](headless/startup.json) · [浏览器结果](headless/browser-r1/proof.json) · [等待放入回包截图](headless/browser-r1/deposit-pending-cancel-enabled.png) · [取回后已关闭截图](headless/browser-r1/retrieve-cancelled-after-old-ack.png)

## 验证范围

- `cargo +1.95.0 test --locked --offline -p mir2-gateway --lib -j 4 trade -- --test-threads=1` 汇报 64 项通过。未设置 `MIR2_TEST_ZONE_SAVE_OPERATION_URL`，其中三项 PostgreSQL 用例未运行数据库部分。`node --test apps/ai-player/tests/observation-trade.test.mjs apps/ai-player/tests/gateway-owner-trade.test.mjs` 通过 15 项。 [Gateway](fixed-trade-tests.log) · [Node](node-trade-tests.log)
- 浏览器、隔离 Next 和 Gateway 均已退出，原 File 账户档哈希一致。启动器的 `ownNextStopped=false` 只检查退出码字段，独立进程与端口复核确认服务已退出。 [准备脚本](prepare-headless.mjs) · [浏览器关闭](headless/browser-r1/browser-closed.json) · [启动器退出](headless/shutdown.json) · [独立清理](headless/cleanup-verified.json)
- 继续验有会籍的 Guild 物品操作未决时发起交易取消、双向物品与金币成交、Native 窗口/TCP、PostgreSQL 和同候选跨端同档。此轮为隔离的 File/WebSocket 验证，公开试玩入口仍使用既有版本，全局 Candidate 与真人 Accepted 保持开放。
