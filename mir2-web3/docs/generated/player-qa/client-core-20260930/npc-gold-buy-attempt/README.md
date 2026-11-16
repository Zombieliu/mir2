# Native普通NPC金币购买：精确本地attempt

Progress62 / 2026-10-04。普通Gold、无限库存、panel0买入已接入共享Rust Core attempt状态，Windows两个实际购买入口、UI取消、队列和最终transport边界通过有限源码检查与独立复核。Web仍使用Progress61页面dispatcher，新持久Core façade与共同商店控件尚未接入。

请求先取得checked u64 token，在发布前绑定完整transport ticket。队列满、断线、UI关闭或换面板等确定未发送状态只撤销对应请求；owner/service/catalog/inventory变化使旧请求失效。实际进入start_send后的Flushed/Unknown保留屏障，旧回执不能释放新请求，关闭重开、改变数量或选择不会自动重发。generic legacy sender=false只释放被拒的精确BuyPending。

最终owner/source检查与Core begin_entry、同步Sink.start_send连续执行；等待readiness时的本地撤销会唤醒并拒绝旧请求。原始NPC对话缺失/null/无效或完整Value变化退役旧目录，两种正常Response/Goods/snapshot顺序均有回归。保证限定于客户端已观察到的来源变化；出站future仍占用网关读取循环，尚未读取的入站更新不在本批证明范围。

BuyItem仍只有type、itemIndex、count、panelType四字段。Flushed表示本地发送观察，不能当作购买ACK；没有自动重试或购买成功推断。对话fixture证明raw Value身份和真实处理helper顺序，未证明实际服务器投影或连接玩家体验。

## 实际有限检查

| 配置/筛选 | 通过次数 | 实际版本及使用方式 |
| --- | ---: | --- |
| Core attempt | 17 | core01 / Source02；未变Core依赖图限定承接 |
| Windows gateway及投影 | 28 | native_platform02 / Source03；Source04唯一改动是Bevy cfg(test)夹具，普通依赖不启用它 |
| Native shop | 64 | native_shop03 / Source04新执行 |
| Native gold entry | 5 | native_gold03 / Source04新执行 |
| Portable gold gate | 4 | portable_gold03 / Source04新执行 |
| Portable shop | 41 | portable_shop03 / Source04新执行 |
| Native NPC dialog | 7 | native_dialogs03 / Source04新执行 |
| Native chat | 29 | native_chat03 / Source04新执行 |
| Native trade | 25 | native_trade04 / Source04新执行，修正实际模块筛选 |
| Web Runtime gold planner | 1 | runtime03 / Source04新执行 |
| Shared WASM | 编译通过 | shared_wasm03 / cargo check，无Instance或renderer启动 |

合计221次跨配置测试执行（Source04新执行176，限定承接17+28），不是221个独立场景。Web源码未改，未重复执行Progress61 Web180/TSC，亦不将旧结果冒称新执行。每次Cargo单独actual C盘≥50GiB守卫、locked/offline/jobs1/inc0/64MiB栈及单线程libtest；15次实际作业均有独立Policy B completed/exited/disposed和原始日志。45个明确属于本批的guard/Cargo/probe PID收尾查询均不存在，旧Native/launcher未查询。

## 源码与保留历史

最终Source04：537组合输入、801声明Rust输入，联合963个唯一文件；9个既有代码变更、524保护输入、4个新Rust文件及完整9份before字节。八份进度文档及本README单独记录，不混入上述代码计数。

保留native_platform01的两处E0616编译失败；Source03仅将新测试两处私有队列断言改为既有公开drain API，净增110B。保留native_shop02的60通过/4panic；Source04只给旧cfg(test)共用纯ECS夹具加一项必需队列资源59B，全部断言/生产代码未变，随后64项通过。native_trade03真实0匹配只作编译记录；runner02唯一机械变化是正确trade模块筛选，native_trade04实际25项通过。旧NativeGold01为已被新执行替代的记录。

Root负责集成；C2实现Core slot并审阅Native/portable边界，M13编写24项真实通道/内存Sink测试，M14独立核验源图、机械runner、全部15作业和最终结果，无剩余确认blocker。上述有限代码证据不等于实际客户端/生产验收。

本地证据：[实际检查清单](C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-attempt-01/root-code-verification01.json)、[最终源码接受](C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-attempt-01/root-source-acceptance01.json)、[限定承接](C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-attempt-01/qualified-prior-checks02.json)、[本批进程关闭](C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-attempt-01/owned-process-close-audit01.json)。

## 下一步与未完成边界

先让Web通过独立ABI和document/Core-version持久façade消费同一Core slot，u64使用规范十进制字符串；组件/renderer重挂载不能重置Entered/Unknown。保留Rust planner与现有入口，在真实socket.send前做精确Core entry；再接共同selection/quantity/page/controller、painter和portable/runtime/Web host。小Core WASM严格<262144B、JS≤204800B及整体包/renderer预算照旧，须实际重建校验。

有限库存、resale/BUYBACK/USED、Pearl、Sell、Repair、无专用购买ACK的结果恢复和服务端容量预检/带元数据插入不一致仍未关闭。当前EXE/Core/renderers/Next独立包不能继承为包含Progress61/62购买改动的已构建版本。共同触屏控件、实际玩家登录/战斗/保存/重登、移动真机、完整NPC商店、frontend/Candidate与整体goal仍开放。用户“继续代码，暂不操作界面”持续有效；本轮未操作UI、启动HTTP/socket/server、renderer/WASM Instance或真实账号。
