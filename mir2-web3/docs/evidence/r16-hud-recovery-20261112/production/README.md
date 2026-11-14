# R16 HUD恢复正式发布：分项通过

2026-11-12，已发布到[numeron.obelisk.build/game](https://numeron.obelisk.build/game)。发布成功，完整公网试玩仍为false，不能记作整体1:1完成。

- 运行源码`a4f99c21115f0c558f78914c120eb1ce12028bff`，部署`dpl_FmfiLkJySDfuwDMPv9VUArWgS7cy`，替换c6bc815bae。
- [Stage43246163242](https://github.com/Zombieliu/mir2/actions/runs/43246163242) / [Promote43248260792](https://github.com/Zombieliu/mir2/actions/runs/43248260792)均成功，同一构建切换；辅助程序`c336ce7a8fe2a8a449093642d983e4e63cd3e676`。
- 177项及生产构建、53份源码绑定、68项发布保护、切换前后QR/启动素材与9条独立公网文件核验通过。

本版只对HUD初次实际失败的URL做一次有界补读，最多8项、并发2；完整正文成功才更新同一个113项run，初始失败、配置、取消日志保留。HUD尝试及恢复结束后并行字库与阶段限时运行包。原字体、WASM、游戏规则、存档与授权未修改。[候选代码与177项](../README.md)。

## 实际通过的公网分项

普通Chrome简中与繁中出码、待确认、取消通过。官方手机尺寸浏览器的临时Testnet虚拟Passkey钱包完成一次登录签名；普通新角色进入比奇、完整视觉包、共享HUD、繁中共享Q、当前会话**113/113、0失败**、原版985/986标记缓存及正常退出通过。初次107/113，6个实际失败URL在本次同一run全部恢复；初始失败记录保留。没有资金转账或真实玩家钱包使用。原始认证记录仅本机保存，仓库只保存筛选字段。

点击进游戏到首可操作画面约7.2秒，共享HUD观测约232.2秒。这是一次公网样本，不作固定时延保证。手机侧是设备模拟，实体摄像头未测。

## 未通过与下一项

首步已真实发出walk，最终UserLocation到达正确相邻格；回包耗时19.7秒，超过原8秒标准，所以签名试玩总结果与本回执passed均为false。同期WebSocket KeepAlive约14.1秒，前后约132–157ms；CDP也记录约19.7秒才到帧，客户端最长任务206ms。证据指向通信链路期间的长尾，单凭客户端不能确定是服务端还是网络。继续定位，不提高门槛、不重跑覆盖失败。本轮未部署Gateway。

原120秒Native冷启失败、其他地图/职业与整体1:1仍开放。HUD恢复通过不等同于首登性能达标。

- [完整分项、移动失败与恢复历史](receipt.json) · [提交/构建绑定](committed-source-binding.json)
- [Stage](stage.json) · [Promote](promote.json) · [独立公网文件摘要](final-public-readback.json)
- [实际共享任务窗截图](public-shared-quest.png)

[前次108/113](../../r16-runtime-delivery-20261112/production/README.md)和[更早发布/回退](../../r16-font-release-20261112/README.md)保留。原65文件保持；其他WIP未纳入，未修改DNS/官网或迁移生产存档。
