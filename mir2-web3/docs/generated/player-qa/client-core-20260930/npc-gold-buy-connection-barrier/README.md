# Progress73：共同普通金币购买的连接保护

2026-10-05。Source02、实际有限检查及独立只读复核通过；本批代码验收完成，整体跨端 Candidate 尚未完成。此前“整体 35%”没有固定验收分母，不再作为进度口径。

Windows 与 Web 共用 Rust Core 购买保护。同一实际连接上，entered / flushed / unknown 请求不会因角色、完整模型、目录、界面关闭或 HMR 变化而被清除。严格更新的可信连接凭据才能退役旧请求；仍需新的完整权威模型和玩家新的明确购买意图，不自动重发，不把退役推断成已成交或已拒绝。queued / bound 未发送请求可以在来源变化后取消。

Windows 凭据来自实际 run/connection fence；Web 凭据来自当前 OPEN socket 的私有身份关联，公开调用不能伪造连接操作。最后一次 socket 读取在可信完整来源复核之前，之后仅做纯身份检查与 Core enter。复核回调必须真实覆盖完整当前来源，任意返回 true 的伪回调未被证明安全。旧版缺少 connection facade contract 的 HMR holder 会先 poison 再拒绝，保留原 holder 和未决请求；匹配契约才复用相同 holder/连接。

## 实际检查

| 检查组 | 本批实际结果 |
| --- | ---: |
| Rust Core ordinary purchase | 21 通过 |
| Rust platform-web bridge | 21 通过 |
| Rust Native UI | 115 通过 |
| Rust portable UI | 75 通过 |
| Rust Windows binary purchase transport | 35 通过 |
| Rust runtime Web quest host | 19 通过 |
| Shared WASM | 编译检查退出 0；无实例运行 |
| Node NPC | 124 通过 |
| Node adjacent | 84 通过 |
| Node storage | 48 通过 |
| 严格 TypeScript | noEmit / incremental=false，退出 0 |

共 11/11 组实际通过：Rust 286 次跨配置执行、14 项新增 distinct；Node 256 项、13 项新增 distinct。两个 Gate 测试分别在 Native/portable 执行，重复执行不增加新增 distinct 数。expiry 的 35 个内部断言组属于既有单一包装测试，未额外加算。

七次 Cargo 串行通过既有不可变 .NET Guard Policy B：每次实际 C 盘新鲜样本不少于 50 GiB，本批最小 158899126272 字节，UTC 样本年龄 59–67 ms，Stopwatch 新鲜度 20.9485–22.2896 ms，均低于 2000 ms。七个 .NET 子进程 completed / exited / disposed 及外层退出证据齐全；Root 核对 21 个本批自有 PID 全 absent，未查询保护的 Native PID。模型容量中断时 Native 原执行已完成，由原线程完成记录恢复，不以复跑代替。

## 源码与限定

Source02 声明 994 项输入：13 项既有授权变更、981 项保护、0 个新源码文件；994 current、after02、before 和未执行 Source01 after 的物理档案匹配。第一次 Source01 的 holder 契约缺陷保留，未验收或执行；修复后原 71 项 Node 测试全文前缀保留，再追加两项契约回归。先前获准迁移的同连接释放语义测试不算新增。

Node 测试运行真实 TypeScript facade/dispatcher 源码，Core 协议响应使用夹具；没有执行 WASM Core 实例。严格 TSC 的本次退出 0 和声明输入不漂移已核实，完整 TSC 实际消费依赖图并未冻结。Progress72 的 153 项 Simulation 和 2 项 Gateway 仅按源码不变限定承接，不算本批新执行。既有生产包、121 项 runtime、4 项 Next metadata 和 22 项别名绑定保持原验收状态；本批未重建生产组合。

普通 BuyItem 二进制和内层四字段 JSON、公开 ABI/capability 不变。金币差额、物品出现、flushed、定时器或重连都不是准确购买 ACK。服务端本地 typed outcome 已有基础，但购买结果的关联回执、幂等和客户端结算闭环仍未实现。未操作 UI、浏览器、原生窗口、HTTP/server/socket、renderer Instance、真实账号/存档或移动真机；不标玩家、生产组合、Candidate 或整体 goal 通过。

下一项先把 typed NPC outcome 贯通真实 Gateway owner lease 路由，默认不支持/RPC/Crystal 通道在执行前拒绝且零 fallback；再实现关联请求、幂等结果和完整 post-processing 投影，接回两端 Core。最后重新构建组合产物；实际界面与移动真机验收继续暂缓。

## 原始证据

- [Source02](C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-connection-barrier-01/web-snapshot02.json)
- [实际检查](C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-connection-barrier-01/root-actual-checks01.json)
- [实际 Node 检查](C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-connection-barrier-01/root-actual-node-checks01.json)
- [正式有限验收](C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-connection-barrier-01/root-verification01.json)
- [原 Native 外层完成恢复](C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-connection-barrier-01/root-native-outer-recovery01.json)
