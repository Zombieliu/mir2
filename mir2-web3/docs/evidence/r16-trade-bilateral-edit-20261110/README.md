# R16 对端确认后的报价修改

Crystal `92b4ce4` 的 `PlayerObject.TradeItem` 在物品变动后清除双方的确认。`TradeGold` 先解锁，再校验和扣款。双 WebSocket 对照了 `870fdd4baa` 的 Gateway。对端确认后，另一方取回报价物品被拒，加价没有扣款，最后仍按旧金额成交。首轮探针把无变化的 KeepAlive 当成必有新快照，已保留超时记录。 [旧版通过的对照](baseline-wire.json) · [包序](baseline-wire-trace.json) · [首轮探针](initial-wire-probe.json)

Gateway 在报价成功变化时，于 Zone 锁内立即让对端旧确认失去匹配资格。冻结金币、原交易标识和物品保留。对端会话恢复自己的可编辑报价时收到解锁回包及新报价，空闲连接也会处理该事件。连续编辑保留对端尚未读到的最新金币与物品。物品操作失败保持确认，零金币或余额不足按 Crystal 清除确认且不扣款。 [构建和源指纹](candidate-build.json)

同样的普通协议复测通过取回、再放入、加价、重新确认、双向交割和同角重登。最终钱包为 909／791。两浏览器从 `/game` 普通登录、互相面对、邀请、接受、操作报价并重登。对方锁定时可取回、再放入或加价 11，另一窗口显示最新报价 70 并恢复编辑。只有双方重新确认才各交割一次，最终钱包为 933／767。四次正常注销、浏览器零错误、落盘背包及钱包一致，两个隔离服测试保持总金币 1700。 [候选协议](candidate-wire.json) · [候选包序](candidate-wire-trace.json) · [浏览器](browser.json) · [界面](gold-change-unlocks.png) · [落盘](disk.json) · [关停](shutdown.json)

Gateway `trade` 过滤器 65 项通过，三个条件 PostgreSQL 用例未接库。三项旧测试曾断言拒绝对端编辑，已改为核对旧确认失效、新报价和实际交割。Simulation 满包保管回归及 Node 两组交易观察的 15 项也通过。Windows 自己的报价投影接受有确切背包格的旧编号 `0`，仍拒绝缺失、重复或不匹配的身份。Native 报价、窗口、包处理和预算共 70 项通过。

背包格不足时仍保留原物品与冻结金币，报价暂不能恢复编辑，旧确认已经失效。Crystal 的独立交易格、发送者自己锁定时直接加价、Windows 窗口和真实 PostgreSQL 继续验收。
