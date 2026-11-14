# V6 ordinary full lifecycle — actual 16/16

同一次普通双客户端完整生命周期已通过 **16/16**，`fullLifecyclePassed=true`，共 **351 条普通发送命令**。这次证据绑定当前 `ee06c05f…` V6 二进制 `ad13d15a…`、55 个后端输入及 `d9ec9d209…` 执行源码；不是旧 V5 的结果。

- Realm：`1793057632464-9c427e44`；protocol run：`1793057646570-c2fb7dc7`；原始 [report](run/1793057646570-c2fb7dc7.report.json)、[realm](run/1793057632464-9c427e44.realm-evidence.json)、[neutral audit](run/1793057632464-9c427e44.independent-audit.json)。
- 原 `full-lifecycle` 900 秒预算、16 项断言及两个新普通注册账号流程保持。首轮真实击杀没有掉落，`sharedDrop=failed` 的原始尝试保留；第二轮通过，最终共享击杀、掉落、唯一拾取、死亡/城镇复活、跨图、退出重登均由原收据及 raw trace 交叉验证。
- 当前本地模块闭包 **14**、原入口 **6**。原 V6 已执行的 Rust **32** 与构建严格复用；本次没有新构建或新 Rust 测试。新 Root Node 资格为 **264+328+55=647**、**42** 个命名输入；该静态资格与真实运行16分列，不能把647当作完整生命周期运行。
- PID **56936** 正常退出码0；运行前后六项容量计数归零。Root 于 `2026-10-26T23:36:06.8928018Z` 独立确认 PID 不存在、17600/17610 无监听；[原 OS 收据](cleanup/root-independent-cleanup-v1.json)。

[实际摘要](actual-summary.json)、[输入/原件复制清单](copied-evidence-manifest.json)、[白名单](public-copy-allowlist.json)、[归档校验](archive-verification.json)。全部原件 EXCL 复制并逐字节核摘要；未改写 pending/frozen 中启动前的原 `false/null` 标记，实际成功以运行报告和独立审计为准。

Root647 [元收据](qualification/root-focused-receipt-v1.json)在此保留；六份原日志复用 [W9 node-v9 目录](../bank-prepare-v9/node-v9/)，[完整交叉引用与原 pin](external-provenance-references.json)，没有再复制日志。V6 原构建/Rust32元数据位于 qualification；原日志/二进制只作摘要引用。

未归档私有 Store、恢复目录、四密钥、注册凭据、Gateway 日志、完整 spectator 或二进制。原公开 report 中的私有文件路径只是原样元数据，不含相应文件内容。

这里只证明该构建的普通双客户端功能生命周期。**capacityAccepted=false、p0Accepted=false**；不证明100/250/500 CCU、300秒档位、2小时持续作战、多机、多核扩展或人类前端验收，P2未开始。
