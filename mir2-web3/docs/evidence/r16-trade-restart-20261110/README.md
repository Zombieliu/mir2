# R16 独立交易格与重启恢复

固定 Crystal `92b4ce4` 的报价物品有独立格。放入后腾出背包，显式解锁保留原报价，取回必须选空格，满包取消按原物退件。旧 R16 及上一轮异常重启的复现保留。 [旧版与首个候选](../r16-trade-detached-grid-20261110/README.md)

同一报价格贯通 Web、Native 投影和共享网关。普通 WebGPU 双开走过改物、改价、双方重新确认、一次交割及原角色重登。r4 的预填 40 格副本用普通穿戴操作填满和腾出唯一空格，确认、解锁与取回保持物品编号。两场 File 存档核对通过。 [双开构建与回执](browser-summary.json) · [双开成交](browser-bilateral.json) · [r4 满包回执](browser-full-bag-r4.json) · [存档核对](browser-full-bag-save-r4.json) · [满包画面](browser-full-bag.png)

File 和内嵌 PostgreSQL 的进程内经济使用同一账户源交易，一次保存两份匹配记录。角色收到物品和金币时，资产、同一事件标记及该角色的记录一起保存。网关重启后按记录继续交付。已标记的单方报价若无匹配记录，则退回原物与金币。显式解锁先保存再发回包。无新版本标记的旧确认仍等待已有权威结果。

| 实际强停时机 | 同档进场结果 |
| --- | --- |
| 一方确认，另一方未确认 | 蜡烛回原格，17 金币退回，原角色再次重登一致 |
| 已收到显式解锁回包 | 强停前存档已解锁，重启退回物品和金币 |
| 40 格占满，仅一方确认 | 一封退件保留原蜡烛编号及数量，金币退回，再次重登保持 |
| 已成交，一方尚未入账 | 同档重启后收到一次蜡烛和 17 金币。确定性 File 回归还覆盖一次存档失败，以及一方已保存、另一方重开后补账 |

四条验证均有 r4 网关的普通 WebSocket 收据。已成交的定时探针延长隔离服空闲推送间隔并采样 File，验证实际尚有待投递记录才强停。早先一次 SIGKILL 撞上通用 File 发布写入，触发原有 `PENDING PUBLICATION` 冻结。后续记录写入标记为空时才发出信号，保留通用冻结与来不及卡住待投递窗口的记录。 [单方确认](file-single-confirm-restart.json) · [解锁](file-unlock-before-restart.json) · [满包退件](file-full-bag-restart.json) · [已成交待投递](file-pending-peer-r4.json) · [早期复现](file-pending-peer-r2.json) · [通用写入冻结](file-publication-freeze-r2.json) · [未卡住窗口](file-peer-already-applied-r3.json)

使用独立 PostgreSQL 16.15 实例验证受租约保护的交易。结算事务锁住双方准确报价，更新两份来源版本，并和账本及 outbox 同次提交。退回事务也锁来源行并检查匹配的投递记录。真实库测试验证带原编号蜡烛和金币的报价成交后不能退回、双方重登各入账一次，以及报价先退回时旧确认无法再成交。内嵌 PostgreSQL 用同一账户源的配对记录验证只有未投递一方重开后补账一次。

Gateway 70 项、Simulation 80 项、旧报价兼容 13 项、Web 100 项和 TypeScript 检查通过。此前针对同一 Native 投影的 74 项组件检查亦通过。Windows 原生窗口、完整 Hosted 浏览器链路及公网发行继续分项验收。 [运行范围和计数](regression-results.txt)
