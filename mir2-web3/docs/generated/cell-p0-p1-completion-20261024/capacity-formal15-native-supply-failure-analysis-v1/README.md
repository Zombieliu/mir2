# 正式15原生供给失败分析

运行 `1792974410829-a5288349` 的游戏验收失败，容量与 P0 未通过，P2 未启动。2人和10人阶段分别测量300.126秒、300.257秒并通过；25人阶段测量300.276秒，五窗为 T/T/T/F/T。50人、100人和两小时 soak 均未执行。[原报告](../capacity-formal-native-v2-scout-anti-convergence-ladder-15/release-native-v2-scout-anti-convergence-ladder-15/reports/1792974410829-a5288349.report.json)与[公共验证](../capacity-formal-native-v2-scout-anti-convergence-ladder-15/public-verification.json)保留原裁决；[独立重演结果](../capacity-formal15-ended-verification-analysis-v1/tool/final-verification.json)的通过只表示证据一致。

唯一失败完整分钟是25人阶段 W4，实际60.048秒。战斗角色05发起/命中16次，0f发起/命中15次，折算15.987210和14.988010次/分钟，低于原18次发起门槛；两者12次命中与67/67原生完成项通过。全程451次攻击结果均为 success。报告测量阶段计232次，分析中的 `stage25AttackActions=233` 按 raw phase 标签聚合；两种时间边界口径分别保留，不猜具体迟到行，也不改报告。

W4 的 scout/chase 移动为05的8/36次与0f的12/33次。两者后来选择相同 home6/target206706；home6声明坐标(255,609)，记录的实际 observer 是(257,609)，不可混用。0f最长27.702秒攻击间隙跨 W4 至通过的W5，不能称全部位于失败窗内。记录说明行程与重复目标占用机会，不能证明存在更好可达替代、策略分支有错、全局无怪、CPU上限或唯一根因；也不是因果 A/B 或逐请求900ms双端包重演。

[Store公共验证](../capacity-formal-native-v2-scout-anti-convergence-ladder-15-store-diagnostics/store-public-verification.json)对应92条已完成、阈值50ms的记录，全程最大401ms出现在W4之后；W4没有收到Store记录，也没有已记录的producer完成span与W4相交。未完成、较短或未覆盖等待仍不能排除，嵌套span不能相加，trace ID不是账户或请求身份。Store不贡献游戏验收。

本目录逐字节保存四份经允许的分析原件：[摘要](summary-v1.json)、[详细分析](analysis-v1.json)、[有限scalar证据](selected-evidence-v1.json)、[分析脚本](analyze-v1.py)。原分析执行过一次闭raw流读；本次归档未重读 raw、monitor、trace、录像、私密状态或二进制，也未执行分析器/测试/服务。角色6/16的候选与见证集合不能从只记录连接1/2的wire诊断重建。原门槛、游戏/source及历史失败保持不变。[复制证明](public-copy-provenance-v1.json)绑定四份原件与公开副本。
