# Progress71 · 普通 NPC 金币购买有效期与共同日期精度

本批完成普通 Gold Trade 名称标签有效期的服务端创建及 plain UserItemExpireInfo 的精确 JSON 传输。Windows 与 portable/Web 使用共同 Rust 编解码和完整原始载体校验；原 Crystal 二进制 i64 与 Buy 内层四键保持。

日期规则直接对应 Crystal Envir.cs：首个成功括号匹配、Int32 前缀及溢出归零；m/h/d 为时长，M/y 保留时刻并钳制月末；未知单位为 Unspecified MaxValue。一次 UTC 捕获用于本笔购买，完整交付物品与 live sidecar 一致。日期超界、范围/金币不足、全部格位占用均在金币和库存提交前拒绝。

新 JSON 将 signed i64 写成规范十进制字符串，并可读旧整数。Web 只救 plain expiry 单字段结构的旧 unsafe 整数；依赖实际 JSON source，缺少 source 时保留 null/unknown，新字符串保持。Rust 直接浮点输入拒绝；Web 安全 legacy 数字仍按既有 JSON 语义解析，例如1.0/1e0可成为number1。UID/count/dura、Rental/Sealed及无关日期未被放宽或迁移。

Source01联合992输入：before990、12既有变化、2新增、978保护。current992、完整after992与before990备份均精确。源码独立审查0个确认blocker。

| 实际有限检查 | 结果 |
| --- | --- |
| Simulation NPC | 138/138 |
| client-bevy native-ui NPC | 112/112 |
| client-bevy portable-quest-ui NPC | 73/73 |
| Windows gateway NPC | 32/32 |
| Runtime NPC host | 19/19 |
| 实际 Gateway lib 精确日期投影 | 2/2 |
| shared WebGL2 WASM | 编译通过 |
| Node NPC | 111/111 |
| Node adjacent | 84/84 |
| Node storage | 48/48 |
| TypeScript | 实际严格检查通过 |

Rust376次为跨配置执行，Native/portable的5项重复覆盖；17项新增具名Rust测试全部实际执行。Node总243项包含parser文件的一项wrapper；该文件35内部断言组不另加总，6项新增parser＋3项新增NPC传输测试均执行。所有选定检查零fail/ignored/skipped/cancelled。首次server01因漏建receiptRoot被Guard90阻止，零Cargo转发；原日志保留，补齐固定普通目录后server02通过，未修改Guard/预算/源码或减弱断言。

七次Cargo均Root串行，fixed1.95.0、locked/offline/jobs1及单测试线程；每调用fresh actual C≥50GiB与原immutable.NET Guard PolicyB，probe/before/after及Guard/outer终态完整。22个明确自有PID实际查询已absent，未查询或操作受保护原生PID。原四项Next元数据、22 alias及既有产物保持。

当前自然Trade目录没有timed商品；新增事务用test-only World-local目录与固定clock经真实内存Session Login/StartGame/BuyItem入口验证。Pearl/Used/BuyBack原路径和旧capacity/evidence/runtime测试保留。本批没有生产EXE/Core/renderer/Next/独立包重建，也没有浏览器、窗口、WASM Instance、HTTP/真实账户或存档操作。实际玩家、保存重登、移动真机、最终frontend/Candidate待验收；整体goal active。

下一项是服务端直接返回typed普通金币购买处理结果；随后协商可选请求关联、同连接未结算屏障和两端精确恢复。原Crystal binary缺少关联回执，不能把该能力默认继承。后续组合构建需绑定新源码。

- [冻结源码](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-trade-expiry-01/web-snapshot01.json>)
- [实际检查与原日志索引](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-trade-expiry-01/root-actual-checks01.json>)
- [独立验证接受记录](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-trade-expiry-01/root-verification01.json>)
- [原 pre-Cargo 拒绝](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-trade-expiry-01/root-pre-cargo-refusal01.json>)
- [自有进程检查](<C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-trade-expiry-01/root-owned-process-check01.json>)
- [后续任务队列](../../../../AGENT-TASK-QUEUE.md)
