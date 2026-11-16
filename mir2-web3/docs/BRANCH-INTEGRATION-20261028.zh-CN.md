# Windows 钱包登录与完整分支合并（2026-10-28）

本轮范围是将 Windows Dubhe Wallet 扫码登录接入现有原生登录界面，
恢复该界面原有九语言机制，并按用户要求合入远端分支的完整历史。
本记录不表示生产发布、手机签名人工验收或全项目 Accepted。

## 来源与保全

- 本地原始提交：`bacb56f46f6b397e869cb12b3f490888d9d2c251`。
- 钱包与九语言改动检查点：`5ad6b04014a87ee8661496801137b6c664e58700`。
- 本轮合并目标：`origin/codex/windows-player-journey`，
  `e11c32fb15ca4cf18d933768b8e70866cd089f26`。
- 共同祖先：`2e9d95bbb3cd9f7852980af385a1298b57a1511e`。
- 合并前原分支有 2 个本地独有提交、198 个远端独有提交；本地检查点另作提交。
- 保留两个备份分支：`codex/pre-full-merge-wallet-20261028-1725`、
  `codex/pre-upstream-merge-wallet-20261028`。
- 合并前检查未跟踪文件与目标树的路径冲突为 0；未清理其他工作或测试存档。
- 原有短版协调规则继续使用；远端旧版长记录保存为
  [历史协调记录](AGENT-ORCHESTRATION-INCOMING-20261028.md)。

## 合并边界

客户端保留远端五职业、月卡/计费、商店/邮件/NPC 托管、显示设置及启动诊断，
同时保留本地装备交互、攻击与半月、石工坊和钱包登录。
钱包文案注册到原有九语言系统，语言持久化继续使用原有 locale 设置。
扫码会话在切换语言时保留；切换语言不会替换二维码或重建登录会话。

服务端同时保留生产/石头与远端征服、月卡、计费、活动持久化，
统一新写入 schema 为 10；旧版本按所属领域的原始最低版本验证后升级，
拒绝将新权限数据注入较低版本。保留两个独立 SQL migration，
不把同号旧 schema 当作同一结构。

Session 仍只承载个人状态，共享移动、碰撞、AOI 和战斗归 Zone。
本轮回归发现 tick 重复 finalize 会提前消费可见性变更，已改为单次收尾；
空的受限命令响应仍需完成事务，不向客户端额外泄漏 AOI。
合并回归 fixture 对齐真实目录物品、地图碰撞、冷恢复撤销在线权限和合法登录
checkpoint；余额、附件、物品身份及失败原子性断言保留。

## 验证与限制

本轮机器可读来源及完整日志：
[branch-integration-20261028](generated/player-qa/branch-integration-20261028/integration.json)。
首次服务端全量运行暴露的失败及修复后的结果分别留存，不能以历史结果替代。
本轮验证如下；各行有重叠，不相加为项目覆盖率：

| 范围 | 本轮结果 | 日志（相对上方证据目录） |
| --- | --- | --- |
| Web 钱包、装备、物品身份及便携构建 | 100 通过；源码 TSC 通过 | `web-selected-first.log`、`web-typecheck.log` |
| Windows host 全量 | 1173 通过，8 忽略 | `native-unit-all.log` |
| 共享原生 UI 全量 | 1592 通过，14 忽略 | `shared-ui-unit-all.log` |
| 九语言真实 Bevy 离屏 GPU | 1 通过，13 张截图；语言切换保留 QR | `native-gpu.log`、`nine-locale/wallet-offscreen-fixture.json` |
| 共享 UI 便携 feature | 编译检查通过 | `shared-portable-check.log` |
| 生产规则 / 持久 UID | 19 / 18 通过 | `production-unit.log`、`user-item-uid-file.log` |
| Simulation 全量及修复后补跑 | 全量 2908 通过、1 失败、23 忽略；仅精炼 fixture 的腰带预期修正后，精炼整组 33 通过 | `simulation-lib-verified.log`、`simulation-refine-verified.log` |
| Gateway 全量及修复后补跑 | 全量 1163 通过、2 失败、20 忽略；仅 AOI fixture 对齐当前 viewport FIFO 后，整组 14 通过 | `gateway-lib-final.log`、`gateway-live-aoi-verified.log` |
| 活动 / 月卡计费 / 充值 / 月卡 / 征服 | 分别 27 / 8 / 24 / 8 / 29 通过 | `simulation-integrations.log` |
| 共享 Zone / 半月延迟 | 209 / 1 通过 | `simulation-integrations.log` |
| 怪物绕障 / 刺杀 | 7 / 10 通过，修复 fixture 后重新编译运行 | `monster-movement-thrusting-final03.log` |
| Windows EXE / Gateway EXE | 均重新构建成功并启动 | `native-build-final.log`、`gateway-build-final.log`、`merged-build-and-launch.json` |
| 本机真实 relay 冒烟 | health 200、创建 200、轮询 pending 200、无效 proof 401 | `live-wallet-smoke.json` |

最后的精炼、AOI 和移动/刺杀补跑仅修改测试 fixture，没有在最后的全量运行之后
修改生产 Rust 逻辑。失败的原始日志保留，不把补跑描述成一轮零失败的全量运行。
临时 Cargo profile 参数只优化本轮测试进程的 simulation/gateway 与密码散列库；
没有为测试速度修改项目 manifest。

用户明确要求不启动 Docker；本轮后续未使用 Docker。
依赖隔离 PostgreSQL 的测试尚未运行，不能据文件存储测试声称 PostgreSQL 已通过。
离屏钱包截图使用测试二维码，不能代替真实手机签名和玩家进入游戏验收。

## 本机启动

从仓库根目录在 PowerShell 启动服务，保持该窗口开启：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\mir2-web3\scripts\start-native-wallet-test-services.ps1
```

在另一个 PowerShell 窗口启动 Windows 客户端：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\mir2-web3\scripts\start-native-wallet-client.ps1
```

两条命令均不需要 Docker。客户端按 F8 打开扫码登录。
服务脚本每次创建独立本机测试数据目录；它不是生产环境启动脚本。
如需保留上一轮已停止服务的测试角色，可传 `-SeedFromRunDirectory <上一轮目录>`，
仅允许本项目 `dubhe-wallet-local-test` 的普通 run 目录，并复制账号及渠道身份两个文件。
旧目录保留，新进程重新生成测试密钥；旧登录会话和旧恢复码不复用。
本次更新在旧客户端断开并等待网关保存后，以该方式保留了玩家测试角色。
