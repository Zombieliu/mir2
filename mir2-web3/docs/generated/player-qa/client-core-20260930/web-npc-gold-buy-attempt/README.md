# Web普通NPC金币购买：持久共享Core状态

Progress63 / 2026-10-04 UTC。Web普通Gold、无限库存、panel0买入已接同一Rust Core attempt状态；保留原Rust价格/容量planner，JavaScript不再分配购买token或决定已发送状态。当前仅接受源码、有限测试和QA Core构建/优化步骤。

## 实际行为与范围

独立ABI1 bridge采用严格字段解析、规范正u64十进制字符串、完整UTF8长度边界和精确原始BuyItem四字段body。Core持久holder绑定document/Core版本：同版本组件/renderer/HMR重挂载复用状态，版本不一致封锁旧holder，缺少新ABI的普通买入不降级为不受保护发送。未标记旧商品目录继续既有legacy路径。

连接/会话/player、完整service/catalog/inventory authority与独立UI选择/数量/布局来源分开。每次已观察的权威入站变化先交给Core，React合并渲染不能掩盖A→B→A；新对话先退役旧目录。旧producer cleanup、旧socket、过期proof和同步重入均不得发送当前请求。真实Page在全部既有gate和最后callback后完成精确Core entry，随后同一同步尾部只调用一次捕获的socket.send。发送前确定拒绝只撤该请求；调用返回记录Flushed、异常记录Unknown，二者只代表本地发送观察，未证明交付/购买成功，不自动重试。

## 选定有限检查

| 检查 | 通过 | 使用范围 |
| --- | ---: | --- |
| Web NPC购买 | 46 | npc01 / 记录Web01；未变有效依赖限定承接 |
| HUD / Storage实际入口 | 3 / 20 | hud01、integration01 / Web01限定承接 |
| Storage相邻回归 | 48 | storage02 / Web02限定承接 |
| Bag/Equipment/Combat等相邻回归 | 82 | adjacent02 / Web02限定承接 |
| 正式优化库 | 38 | core_builder01 / 最终Web03实际新执行，25旧+13新有意义测试 |
| 严格TSC / builder语法 | 退出0 / 退出0 | TSC按未变TS图承接；语法最终Web03新执行 |
| Core attempt / Native购买入口 | 17 / 5 | Rust01未变有效依赖限定承接 |
| Web bridge / 相邻Mail | 17 / 12 | 最终Rust02新执行；3项既有Mail回放ignored，未计通过 |

选定Node共237次通过，最终Web03新执行38，其余199及TSC按未变业务/TS依赖图承接；Rust共51次跨配置通过，最终803图新执行29，Core/Native22限定承接。不是独立场景总数，也不称完整最终图重跑。NPC Node测试执行真实TS/Page函数并用脚本化bridge响应；Rust bridge测试独立运行，两者不证明实际JS↔WASM运行时调用。

所有实际Cargo串行、locked/offline/jobs1/inc0/64MiB栈/testThreads1；6次lib测试加2次实际WASM编译各有新鲜C盘≥50GiB守卫及Policy B completed/exited/disposed。通用core_wasm01仅准备、未执行。30个本批明确Guard/Cargo/probe/Bindgen/Binaryen PID的收尾瞬时查询均不存在，旧Native/launcher未查询。

## 小Core及正式构建策略

原始QA构建270097B和输出编码优化后的268657B均实际超出严格上限，失败保留。当前803 Rust源码生成268657B WASM/19003B JS；固定本地Binaryen131/O1/strip-debug将完整WASM优化为258772B，严格小于262144B，余3371B；JS≤204800B。没有修改原预算。旧CLI probe与正式optimizer API在独立QA副本上的两次实际产物字节一致；Module import/export元数据相同，无name段，未Instance/instantiate。

正式build-client-core已在发布前/verify强制原预算，使用明确绝对路径及exact SHA的Core工具配置；优化库纳入sourceFingerprint。真实API验证使用当前库、固定本地工具和四个公共环境字段，不下载或PATH回退。38项纯代码测试覆盖固定参数、预算边界、工具/输入漂移、metadata、清理与真实完整指纹函数。仅删除最后一条镜像源码接线测试，旧25项主体保留。生产完整Builder/publish尚未执行；现有b17 Core指针和两叶、Windows/renderer/Next包仍不是包含本批购买改动的已发布/组合构建版本。

## 源码与保留记录

最终组合539、Rust803、Web156、Core派生805，联合970唯一代码输入；相对Progress62有12个既有变更、525保护输入、2新Rust文件和完整before备份。805包含当前builder及优化库指纹依赖；旧804仅历史。八份文档新增本批段落，旧正文全部逐字节保留。M14独立源码、实际回执及限定承接审查无剩余确认blocker；最终文档/封存结果单独复核。

保留Storage01 47/48缺dispatcherRef夹具、Adjacent01 81/82旧top-level socket AST夹具和精确最小修复；Core预算01在Cargo前因legacy环境hash失败，仅修复为四个明确公共键，02/03真实超限不改写为通过。错误closure02的assumed-empty记录不得使用，closure03保留初次55148仍在、后续退出及全表空的真实纠正，closure04是本批30个明确PID的瞬时收尾记录。

证据：[代码检查](C:/mir2-cross-platform-storage-20261002/repo-qa-web-npc-gold-buy-attempt-01/root-code-verification01.json)、[保护基线](C:/mir2-cross-platform-storage-20261002/repo-qa-web-npc-gold-buy-attempt-01/root-source-audit01.json)、[限定承接](C:/mir2-cross-platform-storage-20261002/repo-qa-web-npc-gold-buy-attempt-01/qualified-prior-checks01.json)、[正式优化API实际回执](C:/mir2-cross-platform-storage-20261002/repo-qa-web-npc-gold-buy-attempt-01/core_product_opt01.result.json)、[收尾](C:/mir2-cross-platform-storage-20261002/repo-qa-web-npc-gold-buy-attempt-01/owned-process-closure04.json)。

## 下一批与开放边界

先让Native消费普通商店共同selection/quantity/page controller和painter，再接portable/runtime/Web薄host。只读源流发现Native数量热键未像按钮先撤销Bound未entry请求；下一批须修复并测实际入口，不能释放Entered/Unknown屏障。混合Pearl/有限库存/resale目录整体保留legacy交接，不能静默过滤商品。Sell/Repair/BUYBACK/USED、无专用购买ACK的恢复及服务端容量预检/带元数据插入不一致仍开放。

用户“继续代码，暂不操作界面”持续有效。本批无UI/CUA/浏览器/窗口/截图/renderer启动/Instance/HTTP/socket/server/真实账号存档；共同触屏、移动真机、连接玩家登录战斗保存重登、生产组合及最终frontend/Candidate与整体goal仍未验收，goal保持active。
