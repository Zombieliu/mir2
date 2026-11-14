# 独立两客户端 PvP 功能探针

[第二次原报告](attempt-2/reports/1793116390227-48ba2327.pvp-report.json)通过：总计16.235秒，其中功能操作5.817秒。两名合法2级战士普通密码登录，在Map0相邻格移动并互见；2次普通攻击请求中证明1次双端伤害，HP24→23，使用持有的普通658药后23→24，库存12→11；正常退出并重新登录后坐标及原UID库存保持。每端self1000与foreign50000/50001分别核验。[清理原件](attempt-2/cleanup-v1.json)确认driver/gateway均exit0、端口释放。

30秒是操作上限，成功可提前结束；没有执行30秒持续战斗。攻击使用legacy，未冒充V2 launched计数。该结果只证明普通协议闭环，不证明任何CCU容量、普通成长/补给、100人P0或500人全员持续战斗。

只在隔离副本中保留两账号并将离线保存位置设到(234,590)/(235,590)，原等级、HP、装备和12瓶药均未改。原Store不变。第二次保留第一次同端点认证账本；生产认证政策未修改。

[第一次失败报告](attempt-1/reports/1793116142548-79a0e9ae.pvp-report.json)完整保留：探针错误地读取药品模板字段，尚未攻击即停止；随后修正为真实tooltipSource及nullable字段，未改服务器行为或验收门槛。

Root在最终源上独立执行 node --test apps/web/scripts/playtest-capacity-pvp-probe.test.mjs，19项通过、0失败/跳过。最终driver SHA256为59f38980c32d608f50d3c86475fde428b976d7b1dcc7818a17e9a4342cf18688；test为7af08c77daa60703a7f886473c79aaf524618c39f89ac4ceca8230b510bfca16。[复制清单](copied-evidence-manifest.json)逐份绑定12份公开原件。私有Store、pool、账本、密钥、录制及原始Gateway日志均未复制。
