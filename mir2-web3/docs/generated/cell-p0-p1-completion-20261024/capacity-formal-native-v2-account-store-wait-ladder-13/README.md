# Formal13 基础公开证据

Run 1792959622496-365f0175：2/10人阶段通过，25人测满300.254s但W3/W4战斗供给未达标；25人218条attack action均successful、0timeout，不能据此改为通过。50/100人、7200s soak及resume/save/post未执行；report/raw verifier/capacity/P0均false，P2未启动。

Root posthoc v2 collect与原verify均exit0，正常清理7项true；原[报告](release-native-v2-account-store-wait-ladder-13/reports/1792959622496-365f0175.report.json)、[catalog](collect-formal-native-v2-account-store-wait-ladder-13-public-artifact-catalog.json)、[copy provenance](public-copy-provenance.json)、[collection verification](collection-verification.json)、[public verification](public-verification.json)、[raw SHA manifest](raw-artifact-manifest.json)保持原字节。原copy计数185、验证前目录189files均为新增本README之前的真实快照。

50578926B/82186行raw仍在外部，只保原SHA绑定。无新的逐包900ms独立重放或服务器根因判定。见[失败分析及执行历史](../capacity-formal13-native-supply-failure-analysis-v1/README.md)和[独立Store companion](../capacity-formal-native-v2-account-store-wait-ladder-13-store-diagnostics/README.md)。
