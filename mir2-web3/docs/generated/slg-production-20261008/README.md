# 制作基础模块限定验证

日期：2026-10-08。源码位于 `packages/production`，游戏接线范围见[制作代码进度](../../SLG-PRODUCTION-IMPLEMENTATION.zh-CN.md)。

| 实际调用 | 结果 | 受测输入 |
| --- | --- | --- |
| [production-tests-01](production-tests-01/result.json) | 19 passed、0 failed、0 ignored；Cargo exit 0 | 14 个声明输入，调用前后无漂移 |
| [simulation-check-01](simulation-check-01/result.json) | Cargo check exit 0 | 450 个声明输入，调用前后无漂移 |

原始完整输出：[规则测试](production-tests-01/cargo.log)、[simulation 编译](simulation-check-01/cargo.log)。规则测试实际运行 0.60 s；首次测试依赖编译 17.45 s；simulation 首次编译检查 1m 08s。编译检查有现有 simulation 的 84 条警告，不作为测试通过数量。

源快照记录受测 Windows 工作区字节的 SHA256：[规则模块](production-tests-01/source-snapshot.json)、[simulation 接入](simulation-check-01/source-snapshot.json)。这不是全工作区、全平台或全部游戏场景覆盖。测试夹具显式提供模拟材料模板与开放条件，正式目录仍关闭，真实 ItemState 和数据库/账号事务没有参与这批测试。

两个调用串行经过原始未改 CargoGuard、actual C-volume 探针和 Policy B。阈值仍为 53687091200 B，新鲜度仍要求不超过 2000 ms；原 nonce 文件包含 `probe-process.json`、`before.json` 和 `after.json` 记录及探针原始字节。实际剩余空间分别为 247523606528 B、247515983872 B；两个子进程均 `completed=true/exited=true/disposed=true/exitCode=0`。

独立审查见 [源码回复](independent-source-review.json)和[结果回复](independent-result-review.json)。不得把准备好的规则方案称作已经扣料或交付成功。未启动游戏、网关、UI、浏览器或网络测试，未进行正式玩家入口、两客户端竞争、真实容量/重量、账号持久提交或产品发布验收。原始 stdout 末尾空行保留，不按源代码空白规则清理证据。
