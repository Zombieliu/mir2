# Windows 冷启动工程证据

本轮完整原始目录：`E:/mir2-release/windows-cold-start-20261101`。当前 R19 本地正常安装通过，公网冷测未通过；R17/R18 原失败记录保留。

- R17下载通过、完整流程失败：[实际失败记录](r17-cold-start-failed.json)。原始运行目录为 `acceptance-audit/windows-https/r17-cold-01`，保存真实下载文件、原始 JSONL、curl响应/结果、直接连接采样、启动器及错误截图。
- R17失败包撤回、原频道保持：[撤回记录](r17-withdrawn.json)。源站和 R2 的不可变输入保留，未提升任何频道。
- packed-only 启动修复：[Windows测试记录](packed-startup-fix-result.json)。针对性7/7，相邻11通过/1既有忽略；源码3文件 SHA 和原始日志 SHA 在记录中。
- R18统一源码：[冻结记录](r18-source-lock.json)。Game与engine均须来自该提交，发布后文档提交可能为其后继；尚未以源锁宣称公共验收通过。

原始失败运行未被覆盖或改为通过。源码冻结、制品构建、内部 CMS、真实公网下载、GUI首次启动和发布状态分别记录。工程冷测通过不替代全游戏人工 Accepted。

## R19当前门禁：未通过公网时限

- [交付状态](DELIVERY-READINESS.json)：本地normal smoke通过，公网480s timeout；默认发布和canonical推送均未执行。
- [R19正常单页安装与自动启动](r19-normal-installer-smoke.json)：实际同字节、Ready/Healthy、Job和正常退出。
- [R19公网下载失败](r19-cold-download-failed.json)、[R18/R19对比](r18-r19-download-comparison.json)：原钟和原始失败均保留。
- [运行时源码锁](r19-runtime-source-lock.json)、[安装配方源码锁](r19-installer-recipe-source-lock.json)：同线祖先24a50→10a5，不重标未变的runtime。
- [现网络资格诊断](network-eight-stream-qualification.json)与[不含IP的出口分类](network-exit-classification.json)不构成完整冷测。
- [既有GitHub路径资格检查](network-existing-github-qualification.json)：两条入口均未达到约定带宽，没有创建新Release或上传副本。
- [发布操作收尾](r19-publication-operational-closure.json)：115个不可变对象在R2内验证完成，7个默认频道与发布前逐字节相等；临时写入入口与凭据撤销。该记录不等于公网完整冷测通过。
- [下载服务有界源码审查](DELIVERY-PATH-AUDIT.json)：当前部署etag和保留模块一致，尚无足以解释实测低速的代码瓶颈证据；27项内存流检查通过，不作为真实Workers吞吐或完整冷测证明。
- [R19私有重复分片回收](r19-private-parts-retired.json)：完整Setup及16片双轮SHA核对后，仅移除固定私有分片；可用空间约1.21GB→2.25GB。完整包、公网源文件、R2和日志保留，后续不要盲目重跑原私有回拉。
- [未启用的镜像配置](download-mirror-plan.json)、[接入与费用方案](../../WINDOWS-DOWNLOAD-MIRROR-PLAN-20261101.zh-CN.md)。

完整Setup、partial、原始UI图片与进程/HTTP日志留在本机release目录，不提交大型二进制或认证内容。等待合格路径后再做一次实际完整验收；不修改原失败或用私有回拉代替客户端。
