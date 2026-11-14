# 全球五分钟可玩：工程证据

任务状态 **Active / 尚未通过300秒验收**。本目录只记录本次实际执行及明确的设计推算，不能用上一轮登录页Healthy代替进入地图可操作。

- [目标、边界与实施顺序](../../WINDOWS-FIVE-MINUTE-GOAL-20261101.zh-CN.md)。
- [资源布局离线审计](PACK-LAYOUT-AUDIT.json)：冻结R19 catalog/pack元数据与source规则的静态分析；20个启动资源16.64MB散落16pack/533MB，出生视野图4.89MB散落33pack/约1.069GB。重新分组只是尺寸模拟，尚不是新的可运行安装包。
- [Game资源就绪与观测合同](GAME-STREAMING-AUDIT.json)：区分启动guard、实际登录依赖、地图/动态角色/技能和真实可玩门禁；保留签名及缺素材错误语义。
- [默认关闭的读取profile检查](G0-RESOURCE-PROFILE-CHECKS.json)：实际Windows Rust1.95执行4个新测试及8个既有回归，全部通过、无忽略。普通PackStore不启用profile，仍要求全部packs。原日志与源码SHA绑定；后续源码改动需看相应新的集成记录。
- [无损传输尺寸实验](G0-LOSSLESS-TRANSFER-SIZING.json)：原catalog与Game合计202468528B，gzip6后68530187B，两份实际解码size/SHA相同。实验没有接入下载协议、签名或发布；完整Inno安装包原本已压缩，不能从它再重复扣除这项节省。工作站编解码耗时不代表普通电脑验收。
- [更新器和兼容审计](UPDATER-STREAMING-AUDIT.json)：已有断点续传/增量复用可复用，V2资源就绪和传输规则尚未实现。旧Game SDK不接受全局active engine数值大于2，不能直接bump后声称保留旧版回退。
- [当前CDN只读观察](CDN-EXISTING-OBSERVATION.json)：现有素材Worker已经使用Cache API；单次HEAD MISS不表示缓存未启用，也不证明缓存填充或带宽。未修改配置或部署。
- [重新分包库最终检查](G0-REPACK-LIBRARY-CHECKS.json)：19项实际测试通过，包含旧12项及新增7项；这次原stdout仅留在工具历史，记录明确`rawLogPersisted=false`，没有伪造原日志。
- [Game接入最终检查](G0-GAME-PROFILE-CHECKS.json)：读取诊断7、generation9、capture22共38项通过，编译检查通过。并行落盘造成的两次E0583原失败日志保留，最终复验时依赖已冻结。
- [真实全量重分包](G0-REAL-REPACK-VALIDATION.json)：实际读取247477个唯一payload，并验证新pack的全部hash/index；独立Python逐项核对255063个逻辑路径的size/SHA相同。20个startup guard物理依赖533MB→16.64MB；它们不是完整首玩闭包，当前runtime仍要求全部packs。
- [G0集成状态](G0-INTEGRATION.json)：最终Game验证的12份输入SHA再次与当前文件核对一致；无真实首玩采集、部分资源挂载、发布或300秒验收。

原始审计、脚本、编译/测试日志和局部实验文件保存在`E:/mir2-release/windows-five-minute-20261101`。维护代码仍在canonical工作树；本目录不包含大体积二进制、账号令牌或密钥。

用户已明确同意先玩、其他素材后台补齐；规划审计中历史pending字段保留其当时状态。尚无真实首玩profile、部分资源runtime、全球地区Windows连续300秒结果或新版本发布；缺口保持可见。
