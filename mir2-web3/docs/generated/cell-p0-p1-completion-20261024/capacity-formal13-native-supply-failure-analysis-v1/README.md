# Formal13：原生战斗供给失败分析

本轮 1792959622496-365f0175 已结束，原始report、raw verifier和capacity均为false。2/10人分别实测300.259/300.261s通过；25人测满300.254s，五窗中W3/W4未通过。25人阶段218条攻击action全为successful，0timeout、0not-launched；失败是逐fighter攻击发起供给，actor05的正伤害率也低于按真实时长计算的门槛。见[终态事实](facts/formal13-actual-terminal-independent-facts-v1.json)与[失败分析](report-analysis/analysis-v1.json)。

50/100人、7200s soak、resume/save/post均未执行。原阈值未放宽，P0/capacity仍false，P2未启动。7项正常清理为true，另有[Root OS实查](facts/formal13-root-independent-cleanup-v1.json)。

Root基础collect/verify与独立Store collect/verify四项均exit0，只确认归档一致性，保留游戏失败。Store共106条、23083B，诊断complete；[Store分析](store-analysis/analysis-v1.json)只讨论已完成且超过50ms的记录，嵌套阶段不可相加，不能据此确定Store/IO实际holder、目标竞争、碰撞或容量上限，未确立旧5s停顿根因。successful action并非独立逐请求900ms typed+双端正包重放。

事后采集器只修正catalog.scope轮次并加入self/proof白名单，原100冻结输入保持。可核[原采集器](post-collector/collect-public.mjs)、[派生v2](post-collector/collect-public-current50-v2.mjs)、[精确逆变换与syntax](post-collector/collect-public-current50-v2-derivation-v1.json)、[有效v2独审](post-collector/review-v2.json)及[实际采集执行](execution/formal13-base-collection-execution-v1.json)。[v1审查原件](post-collector/review-v1.json)是PowerShell记录构造错误留下的null JSON，作为历史保留，不视为有效审查；有效v2在实际采集前完成。

大raw/monitor/录像/EXE/private不复制。原始报告和185份基础copy见[基础目录](../capacity-formal-native-v2-account-store-wait-ladder-13/README.md)，Store独立数据见[Store目录](../capacity-formal-native-v2-account-store-wait-ladder-13-store-diagnostics/README.md)。新说明、白名单、复制收据和核验不改原catalog/verification。
