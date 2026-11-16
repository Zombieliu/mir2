# Web R16 新手 1–30 三职业验收

本机试玩地址为 `http://127.0.0.1:19041/`。试玩包 v21-r3 来自 `codex/windows-player-journey`，沿用 Windows R16 的 `platinum_fiveclass` 与 `newcomer-v2`。战士、法师、道士各有独立本机测试世界和从 1 级创建的角色。任务链按实际 WebSocket 协议执行，服务器逐项裁定击杀、职业练习、交付和成长。浏览器另验正常选角、Q 日记、F11 技能说明和注销。120 分钟时限与真人验收单列。

| 职业 | 实际等级 | 主线 | 成长 | 同角色浏览器读回 |
| --- | --- | --- | --- | --- |
| 战士 | 30 | 22/22 | 4/4 | 通过 |
| 法师 | 27 | 19/22 | 3/4 | 通过当前存档，沃玛第 20 项 0/3 |
| 道士 | 30 | 22/22 | 4/4 | 通过 |

## 本机包与素材

- 补齐三职业成长装备、召唤骷髅和沃玛森林 `1`、矿洞 `D401`、沃玛寺庙 `D021`／`D022` 路线的原版素材。有限素材清单共 42,724 项、336,156,477 字节，保留此前 24,262 个条目。七个篡改清单检查均拒绝，素材与任务回归 31 项通过，V2 协议与装备回归 75 项通过。[素材收据](evidence/web-r16-newcomer-30-20261106/assets-v21-r3.json) · [素材与任务回归](evidence/web-r16-newcomer-30-20261106/regression.log) · [协议与装备回归](evidence/web-r16-newcomer-30-20261106/protocol-regression.log)
- 本机 HTTP 校验了 248 个素材响应体、四张小地图的原图与 12 个地图场景区域。全部区域成功返回，缺图计数为 0。[HTTP 收据](evidence/web-r16-newcomer-30-20261106/http-scenes-v21-r3.json)
- 发行目录 51,319 个文件逐项核对长度和散列。v20 网关、Next、Bevy、Node 和阿洛程序沿用相同二进制。旧目录留档，配置与运行数据在停机后逐文件校验，随后在原地址恢复。精简包仍为 737,960,507 字节，超过 360 MiB 预算。[发行收据](evidence/web-r16-newcomer-30-20261106/package-v21-r3.json) · [数据保全收据](evidence/web-r16-newcomer-30-20261106/preservation-v21-r3.json)

## 道士实机记录

道士通过 22 项主线和 4 档成长，达到 30 级。26 项各有唯一交付、匹配的成功回执和完成包，36 条物品奖励增量与最终 File 存档核对通过。原 120 分钟窗口结束时为 27 级、22/26，沃玛任务 `2110020` 为 1/3。其余四项在单独的 10 分 10 秒功能复验完成，三次复活保留在原记录。[协议与存档收据](evidence/web-r16-newcomer-30-20261106/taoist-protocol.json)

浏览器实测在炼药师处采购 20 瓶大红，扣除 4,500 金币并收到持久成交回执。后续复测通过 Q 日记回读第 4 章及 26 项进度，F11 悬停显示完整的灵魂火符说明，正常注销。首次 30 级回读发现召唤骷髅 `Monster/078` 帧缺失；v21-r3 补齐 234 张帧及其元数据后，复测未出现资源 404。[补给收据](evidence/web-r16-newcomer-30-20261106/taoist-supply-browser-durable.json) · [浏览器收据](evidence/web-r16-newcomer-30-20261106/taoist-browser-final.json) · [日记](evidence/web-r16-newcomer-30-20261106/taoist-final-01-web-quest-diary.png) · [技能说明](evidence/web-r16-newcomer-30-20261106/taoist-final-02-web-class-skill-tooltip.png)

## 战士实机记录

战士通过 22 项主线和 4 档成长，达到 30 级。15 段普通协议记录中，26 项均有唯一交付、成功回执和完成包，22 条物品奖励增量与最终 File 存档核对通过。原时限最后确认 22 级、16/26，矿洞第 15 项完成了职业练习，尚差一只僵尸。功能续跑用实际攻击完成第 15 项，确认第 16 项刺杀剑术命中及三只目标的击杀。沃玛段用普通金币采购 46 瓶大红和 4 张回城卷，第 21 项实测半月弯刀及刺杀剑术命中和三只沃玛勇士，随后回城报告、领取 30 级奖励并正常注销。功能钟约 55 分 37 秒，原窗口与续跑均为零复活。[协议与存档收据](evidence/web-r16-newcomer-30-20261106/warrior-protocol.json)

同一战士在浏览器正常重登，读回 30 级、19,653 经验、62,010 金币及全部 26 项。Q 日记显示第 4 章并且主线与可交付均为 0。F11 悬停读取半月弯刀的完整说明，关闭窗口后提示收起，随后正常注销。[浏览器收据](evidence/web-r16-newcomer-30-20261106/warrior-browser-final.json) · [日记](evidence/web-r16-newcomer-30-20261106/warrior-final-01-web-quest-diary.png) · [技能说明](evidence/web-r16-newcomer-30-20261106/warrior-final-02-web-class-skill-tooltip.png)

## 法师实机记录

原计时段完成 19 项主线、3 档成长及 22 次唯一交付，27 级时在第 20 项用完三次复活并暂停。独立浏览器正常重登后看到 111/111 HP、同位置和同任务计数，实际打开 Q 和 F11，核对大火球说明后正常注销。该次浏览器重登观察到的是已恢复的生命值，没有发送复活动作。[原计时协议收据](evidence/web-r16-newcomer-30-20261106/wizard-ordinary-negative.json)

第一次功能计时确认普通跨入 D022 和回城，截止仍在第 20 项 0/3。一次驱动中断缺少正常注销回执。另一次外置中止回调参数写错，截止之后仍出现普通移动和一次用药，均从计时通过证据排除。[功能计时负结论](evidence/web-r16-newcomer-30-20261106/wizard-functional-clock-negative.json)

时限之外另行确认西侧走位、普通补给与任务怪的有效大火球伤害。一次撤退后确认 HP0。末段由东侧接敌，确认两次大火球伤害和八次走位，测试驱动触发单步预算，角色用普通回城卷返回，最后 HP6 并正常注销。末段 File 存档与权威快照一致。随后在 v21-r3 浏览器再次正常重登，读回 27 级、22 项已交付、16,830 金币及沃玛战士 0/3，Q 日记和完整大火球说明可用，正常注销且无资源 404。[时限外结果](evidence/web-r16-newcomer-30-20261106/wizard-post-clock-verdict.json) · [末段协议和存档](evidence/web-r16-newcomer-30-20261106/wizard-latest-protocol.json) · [浏览器收据](evidence/web-r16-newcomer-30-20261106/wizard-browser-revive.json) · [日记](evidence/web-r16-newcomer-30-20261106/wizard-revive-01-web-quest-diary.png) · [技能说明](evidence/web-r16-newcomer-30-20261106/wizard-revive-02-web-class-skill-tooltip.png)

[脱敏证据索引](evidence/web-r16-newcomer-30-20261106/index.json)
