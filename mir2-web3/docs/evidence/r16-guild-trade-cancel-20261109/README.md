# R16 公会仓库待回包时取消交易

2026-11-09，`codex/windows-player-journey`。基线 `b861dfceb5`，修复 `ada80ec7c279d6f6efd292c8e94407921f5fdfae`。两版均使用 `ac6817dbf9` 的共用 Gateway、`bevy-0cd68510886134e0` WebGPU，各自从同一份隔离公会档建立 File 副本。实际会籍与仓库权限由 Gateway 提供，公会在测试前已建档。 [基线启动](baseline-startup.json) · [修复版启动](startup.json)

Windows R16 的 `trade_dialog::cancel` 为已打开的交易单独排队取消。固定 Crystal `92b4ce4` 的 `TradeDialogs.TradeCancel` 也独立发送。旧 Web 的物品操作门禁吞掉了可点击的取消。修复版仅对已打开的交易开放该入口，保留同连接、角色、地图、交易代次、单次发送及共用八项预算的检查。交易结束包只清交易，公会物品仍要有自身回包及后续同角色完整快照。

| 界面流程 | 观察结果 |
| --- | --- |
| 旧版存入蜡烛时点击取消 | 服务器已成功存入，暂缓向浏览器交付匹配的回包。按钮可点，发出的取消指令为零，交易保持打开。 [旧版包序](baseline-r3.json) |
| 修复版存入 | 木剑已放入交易，蜡烛由背包索引 3 存入仓库索引 0。延迟仓库成功回包 1185 ms。浏览器只发一条取消，双方各收到一次 `Unlock=false`，仓库仍受操作锁保护。新快照到达后可操作，重登读到同标识蜡烛仍在仓库，木剑仍在原背包 |
| 修复版取回 | 取回同一件蜡烛到背包索引 4，成功回包延迟 1192 ms。两端交易已关，仓库在自身回包和新快照齐备后解锁。界面显示原仓库格已空，双方同角色重登读到精确资产 |

主角使用真实界面与快捷键，另一方使用普通 WebSocket 客户端。探针只暂缓每场匹配的仓库回包，原始内容以同一哈希释放。两个场景刻意让先发的完整快照先于仓库回包到浏览器。释放后空闲时完整快照计数保持，打开成员页触发真正的 `RequestGuildInfo` 读取才收到下一份完整快照。取消不依赖这次额外读取，仓库锁依赖自身回包和完整资产状态。两场只提交一次仓库修改，迟到回包均未重开交易。六项实际流程断言通过，观察到 Native 预算桥 92 次调用，浏览器错误列表为空。 [浏览器记录及包序](browser-proof.json) · [存入等待截图](deposit-guild-pending.png) · [取回时交易已关截图](retrieve-trade-closed-guild-still-pending.png)

两次注销后同角进城分别读取当次完整仓库列表。最终双方正常注销，关闭测试 Gateway 后还核对了 File 副本的槽位、标识、数量和金币。原隔离源档哈希一致，六次启动的隔离端口和进程均已清理。 [持久化结果](durable-save.json) · [清理结果](cleanup-verified.json)

初次浏览器脚本查错公会快捷键，第二次仓库选择器被交易窗口覆盖。最终采用实际的 `Guilds` 绑定及正常标题拖动。第一次修复版试跑收到两端取消包，但在刻意重排回包后一直空闲，缺新的完整快照。后续用成员页取得新快照，第一次又误把重复点击刷新当成能取得第二份全量列表。Crystal 同会籍会话只发一次全量，之后由增量更新。取回时，新背包先到会让旧仓库格与背包短暂报告同一标识，选择器因此隐藏，最终以仍显示的仓库操作锁和交易回包核对。 [快捷键失败](baseline-r1.json) · [窗口覆盖失败](baseline-r2.json) · [无后续快照](candidate-before-post-ack-snapshot.json) · [重复列表断言失败](candidate-repeated-list.json) · [取回选择器断言失败](candidate-retrieve-picker.json)

定向 Page 回归在原发送条件下失败。修复后 Page、社交预算 host 和社交窗口共 149 项通过，TypeScript `--noEmit --incremental false` 退出 0。范围是隔离 WebGPU/File，共用客户端源和实际 WebSocket。公开试玩、Native 窗口、真实 PostgreSQL、双向物品和金币交割与全局 Candidate 继续分开验收。 [原回归](red-page.log) · [通过日志](final-web-tests.log) · [探针](browser.mjs) · [隔离启动器](launch.mjs)
