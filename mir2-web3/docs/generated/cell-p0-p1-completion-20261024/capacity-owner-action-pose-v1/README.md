# 本地动作旧坐标覆盖新移动确认修复

P1通过、P0未通过、P2未启动。正式2/10通过；25完整300秒战斗频率失败。V10全员50入场，但准备期AOI missing停止，未进入测量或启动100。[原报告](v10-original-report.json)及false结论保留，[独立清理](v10-root-independent-cleanup.json)确认进程63240消失、端口释放。

[确切事件](v10-exact-own-attack-source.json)：observer16的UserLocation在1793026896105将self置(235,602)，紧接6896106的own ObjectAttack(seq5688,wire1000)又应用旧anchor(236,601)。owner17后续移至(244,585)，相对这两个self坐标的距离分别17/16，旧self使AOI产生错误可见预期。初始分析器将该source归other；Root只额外定位读取末尾2MiB恢复具体ObjectAttack类型，没有第二次完整samples读取。原始缺失判定没有追认通过。

真实Web与driver同步修复：仅own ObjectAttack/ObjectRangeAttack/ObjectMagic保留当前x/y，继续方向、动画、特效及弹道；remote同包仍更新位置。UserLocation、Walk/Run、Push、Dash、BackStep、地图等真实位移原处理不变。依据server owner_action_transform不把这三包认作位移，攻击anchor为动作时快照，Blink/Teleport或方向技能实际移动另发UserLocation。未改AOI/时延/活动/死亡门槛或服务端权威。独立只读review未见阻塞问题，后续projectile/effect路径不再写x/y。

新鲜[四入口Node242/242](full-driver-receipt.json)绑定canonical9+导航2+observation2共13已识别输入；新鲜[直接观察器/Web39/39](focused-receipt.json)、[完整Web typecheck](typecheck-receipt.json)绑定上述13+前端3共16输入，均前后相同。这里不声称已冻结所有传递依赖。canonical9仍657、导航仍4ecd；新的observation及直接test此前不在9项内，本轮新增独立SHA/Git绑定。Rust52未改，明确按相同raw/GitLF/Git对象复用[既有57项](../capacity-zone-delivery-trace-v1/slow-span-validation-v1/root-focused-receipt-v1.json)，没有重跑Rust。

初次合并focused为24/25：旧scene结构脚本先在未改renderer锚点断言失败；[原代码对照](original-scene-baseline-receipt.json)同失败。旧脚本已恢复，未放宽断言；新增独立生产helper tests15项与观察器24项合计39通过。此既有renderer结构检查仍未通过，不称整个前端测试全绿。

[gate审计](v10-gate-holder-audit.json)另外确认read gate长等待，health慢项不普遍；fatal窗无checkpoint慢项，不能据此移除保存锁。reg16与publicconn16是独立计数器，只能按时间签名关联，不能当严格同一身份。该性能问题没有因客户端修复而宣告解决。新Release/50复测待执行，未证明硬件容量上限，也未完成GUI/人类Accepted。
