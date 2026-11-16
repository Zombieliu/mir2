# 阿洛培养与有限补给：2026-11-03 公开证据

本目录把已有测试日志与可公开的游戏收据分别留证。本次整理仅做只读核对和复制，没有重跑测试、游戏进程或模型/API调用。当前 bundle 尚待根任务交付与验证；不认定整体 Crystal parity、最终人类 Accepted、付款或留存成功。

## 四个验收范围

| 范围 | 证据 | 可以据此判断 |
| --- | --- | --- |
| 冻结旧 R16 Windows GUI，规则模式 | [基线结果](baseline-replay-outcome.json)、[复核说明](baseline-replay-notes.md)、01–07截图 | 普通GUI组队/预设教学、教学后真实行动改变、57948→12012保留同archive/v2、GUI真实导出摘要。0模型调用；不作新增教学/补给UI验收。 |
| 当次 Node 源码 + 已验证旧 R16 Gateway，规则脚本 | [规则邻格单轮结果](rules-adjacent-once-outcome.json) | 组队/跟随/教学/重启，以及自有药消耗、实际回血和危险撤退存活。该记录明确 newGatewayValidated=false，不能转记到新网关。 |
| 新 Gateway 普通本地 /ws，独立合成世界 | [供给只读复核](supply-live-reviewed.json)、[原始实际summary](supply-acceptance-summary.json) | 自有开局剑/衣普通卖出筹85游戏金币；真实购1瓶扣40、返占位主人邻格；撤销；第二笔真实commit后丢方法结果、换Node仅Query原操作。0模型调用，不是真人试用。 |
| 新 Native / Gateway 的真实 Luna 教学 UI | [教学UI与重启结果](model-ui/teaching-ui-outcome.json)、model-ui/01–06截图 | 两次真实提案、确认三项true写v2、单项false预览后取消，以及59820→56204保留同archive/v2且不增加调用。新世界教学后战斗未另验。 |

新供给中，39金币案例仅证明观察到40报价且未再支出、已有药量未变，不能单独诊断唯一拒绝原因；服务端真实预算边界另看对应Rust测试。51上限拒绝的原始响应未放进summary，复核保留了该证据限度。故障恢复是明确的JS方法结果丢失注入，不冒充真实断网或Gateway重启。

供给summary的passed发生在finally清理前；复核另外确认两个actor PID不存在，但没有独立Gateway退出证据，因此不宣称供给全部自有进程均退出。规则邻格结果的进程退出证据属于它自己的旧网关单轮。

## 自然语言教学与当前交付

[真实教学与重启摘要](model-ui/teaching-ui-outcome.json)及6张已查看截图确认了两次gpt-6-luna调用，合计2232 input / 249 output tokens（分别1010/134、1222/115）。第一次三项false→true经预览确认写v2；第二次仅askBeforeLeaving=false的提案被取消，原v2三项true未变。false没有提交落盘，不能把取消当成已培养为false。

不同Node进程59820→56204重启后，同archive `3b663035-5b4b-419c-b6da-e6fe033c417f`、v2和三项true保留，模型调用次数没有增加。本次整理没有发起调用，也不声称这个新模型测试世界已另行通过教学后战斗。规则基线的战斗证据和新模型UI保持分开；最终bundle仍待交付与验证。

截图：[提案预览](model-ui/01-model-proposal-before-confirm.jpg)、[确认v2](model-ui/02-model-teaching-confirmed-v2.jpg)、[单项false预览](model-ui/03-model-partial-false-preview.jpg)、[取消不变](model-ui/04-cancel-keeps-habits-v2.jpg)、[补给授权预览](model-ui/05-supply-budget-confirmation.jpg)、[重启记忆](model-ui/06-restarted-model-memory-v2.jpg)。补给确认窗口最终取消，不构成GUI已授权或已购药；实际购药仍来自独立普通/ws场景。

## 自动测试日志

精确路径、字节数、SHA256、结果行和各target结果见 [tests.json](tests.json)，原始已检查的日志保存在 logs/。

| 分组 | 记录结果 |
| --- | --- |
| Node最终 | 310/310，0失败/跳过 |
| Gateway伙伴分组 | 78/78，0失败/忽略 |
| Simulation bounded | 6/6，0失败/忽略 |
| Simulation transaction | 37/37，0失败/忽略 |
| Simulation owner wire | 11/11，0失败/忽略 |
| Bevy伙伴UI | 24/24，0失败/忽略 |
| Bevy controls | 38/38，0失败/忽略 |
| Windows伙伴桥接 | 13/13，0失败/忽略 |

这些分组不相加。Bevy两份日志有23个同名用例；即使名称不同，也不据此声称运行快照一致或全局无交集。源码测试、普通联机收据和Windows GUI截图各自限定范围。

## 二进制与资料边界

旧GUI基线Native SHA256为 `ff590f072b0f112f1bd6392ed87cd00d232f86d10299b04c24989d2f47bf70c7`；旧规则邻格Gateway为 `165eba1719866912d2b7da28ad430164bd1a9a8ef704fe976400d0b232dccc7b`。

整理时只读核对的新Native构建SHA256为 `e7e5b7baacbb0300c73d1525fc7b70b3cec365dd1b7d5f2b8be277cf39cfeaab`；新Gateway为 `cd35f1f8cba5c2d6602dc344421b25db613c4c82061fdf6f34b1946df914a3d7`，与供给summary一致。它们是构建产物哈希，不是已经交付的bundle资格，也不替代启动时冻结的源码清单。

本目录没有复制原用户角色state、账号配置、私有MAC/签名key、SQLite或完整角色archive。基线只公布GUI导出的身份/习惯/计数与SHA摘要；完整archive保留在原私有目录。outcome内的私有源路径只是出处，不代表把这些目录内容发布。

攻击tick的confirmed:false原样保留，真实命中另由ObjectStruck归因核实。首次怪物过早死亡与旧Candle过期未计入基线通过。
