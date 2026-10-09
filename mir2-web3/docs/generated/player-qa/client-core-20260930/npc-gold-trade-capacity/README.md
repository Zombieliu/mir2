# 普通NPC金币购买容量与完整载体验收

2026-10-05 UTC · Progress68 · 已接受本批服务端源码与有限内存测试。整体goal保持active，客户端共享满包准入及实际玩家验收继续开放。

普通Trade/Gold购买在私有库存克隆中校验Bag1/Bag2/合法Belt分区、容量、格位、当前行自身stack上限、完整UserItem及真实root/nested/reserved身份；兼容现代堆叠吸收购买数量，剩余量要求真实空格。失败返回前不写入live库存或钱包；成功后一起提交，并发送原始购买数量的GainedItem及精确LoseGold。catalog UID和0仅在私有分配视图临时排除，原reserved集合原样恢复。fresh载体保留规范durability、identified=false、完整空socket宽度和fresh默认字段；模板基础属性不进入AddedStats。name-tag expiry仍沿旧None行为，不据此接受完整CreateFreshItem parity。

无sidecar的旧根编号是各packet grid内的legacy alias。Bag1/Bag2共用Bag域，Belt独立域；两个legacy根跨grid同号可保留，但与真实metadata根、nested或reserved同号仍拒绝。旧物品只占格，不贡献fresh合并余量。Pearl、BuyBack、Used及原价格/rate/数量/service门槛路径保持。

实际验证

- 最终Source04购买模块：16通过，0失败/ignored；1914项过滤。
- 同源相邻NPC：115通过，0失败/ignored；1815项过滤。115个不同测试名已包含上述全部16个；两个命令共131次执行。普通默认角色Trade、两个Pearl以及BuyBack/Used回归实际通过。
- 16项含完整现代堆叠、多个Bag/Belt余量、容量不足整笔回滚、最后合法Bag2空格、metadata不兼容、非法格/容量/重复UID、自身数量上限、storage/equipment/reserved/Quest根和nested冲突，及默认legacy购买原编号保留、legacy满包不误合并、7类legacy冲突的正负控。
- 两个实际Cargo均为Root串行、locked/offline/jobs1/lib/test-threads1；每次fresh actual C≥53687091200字节。Guard .NET completed/exited/disposed=true、PolicyB、Cargo exit0，Node spawn/exit/close0，外层session32106与83563实际关闭0。
- 独立M14只读复核确认0个blocker；813 Rust输入及备份一致，前端553输入未改。只改inventory.rs新planner、npc.rs普通Gold接线及新增独立测试文件；旧runtime/tests.rs、全局allocator/codec/严格兼容谓词及保存迁移原样保留。

失败历史

Source01独立源码审发现Quest身份遗漏及测试filter问题，未执行；修复后Source02实际13项5通过/8失败，定位默认分配UID0及不完整装备夹具。Source03实际购买13/13，但相邻112项111通过/1失败，定位legacy格编号被误提升全局身份。Source04修复仅在新购买planner及追加测试，原13项测试/48断言前缀逐字保留；所有失败配置、日志、源快照与Guard回执保留，无skip。

后续

共同Rust客户端目前仍只按空格准入，满包兼容堆叠尚未接通。下一批使用完整snapshot授予的可空roster/compatible-UID证据，Windows与Web透传，局部owned包同步撤销可用性。证据参与request/liveproof，避免仅撤证据便释放Core Entered/Unknown屏障。无专用购买ACK的结果恢复仍开放。

本批未重建Windows EXE/Core/renderer/Next/独立包；Progress67已接受的构建产物属于其原源快照，未据此当作含本修复的新产物。没有操作界面、浏览器、原生窗口、WASM Instance、HTTP/socket服务、真实账户或存档；移动真机、最终frontend/Candidate及整体goal未完成。

证据

- Root QA目录：C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-trade-capacity-01。
- root-verification01.json：8460字节，SHA256 d5315a95d1707f2d168ec8df07a12124c6367745f687bb1860f0e44201a6b104。
- rust-snapshot04.json：387462字节，SHA256 f5419d6d0c2a7723a34353a3bd7186f8a867bd4b5734fd17899f7f4c964f53bc。
- gold_capacity04.result.json：1574字节，SHA256 763060d0a301e92b4dbea7bc0968bb6c8f1e45a23a5566c67208726277530977。
- npc_adjacent04.result.json：1537字节，SHA256 6e7494e056fc31c73834dacd95708901c57f195dac993d9a717f7b834f5321fb。
