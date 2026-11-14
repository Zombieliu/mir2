# 个人智能求购 V1：公开验收证据

本目录保留可公开的原始测试日志、SQL 测试脚本、截图和 SHA256。它不包含运行账户数据库、密码、钱包私钥、Oracle 私钥、环境配置、依赖目录或可执行文件。测试角色仅用于明确授权的隔离验收；生产收费、主网部署和真人最终接受均未因这些测试自动通过。

功能与限制见 [主题文档](../../PERSONAL-PROCUREMENT-V1.zh-CN.md)。Dubhe 固定版本、Move/Node 测试和独立真实测试网交易，集中见 [链上证据](../../../onchain/procurement/validation/20261028/README.md)，不在本目录重复复制。

## 范围与快照

| 证据 | 结果 | 能证明的范围 |
| --- | --- | --- |
| [Rust 最终功能日志](rust/procurement-tests-v17-stable-wire-states.log) | 54/54 | 最终稳定协议状态、真实 owner 公开 open 投影、退款领取、报价来源、两单位费用冻结、装备实例描述、120 秒 outbox 租约、慢 RPC、恢复及托管不变量等模拟/本地账户持久化测试；不是实际链上或原生 PostgreSQL 联调 |
| [Schema 相邻回归](rust/procurement-schema-regression-v3.log) | 3/3 | Schema 12 及旧读取者保护的相邻回归 |
| [SQL 结果](sql/results.json) | 44/44 | 官方 PGlite 0.5.13 / PostgreSQL 18.6 WASM，内存临时表执行原始 0018 SQL 和 PL/pgSQL 触发器；不是原生 PostgreSQL/libpq、并发、崩溃耐久或生产迁移验收 |
| [Gateway v4](gateway/gateway-tests-v4.log) | 14/14，旧源码快照 | 包含耗时轮转公平性测试；后续改动由 v5 补测，不能把 v4 当成最终源码全量验收 |
| [Gateway v5](gateway/gateway-tests-v5-final.log) | 13/13，补测快照 | Gateway worker 补测，早于最后一次 runtime 报价来源修正；跳过 v4 已通过的一项耗时公平性测试，不能将两次计数相加成 27 项独立测试 |
| [Gateway v6 测试](gateway/gateway-tests-v6-asset-projection.log) | 13/13，最终资产投影补测 | 最终采购响应世界快照集成后通过；同样跳过先前通过的耗时公平性测试，不与旧快照计数相加 |
| [Gateway build v2](gateway/gateway-build-v2-final.log) | 成功，报价来源修正前快照 | 当时的 Gateway 开发构建；二进制 SHA 见 [证据索引](evidence-index.json)，不打包可执行文件。最终实连构建由协调者另补 |
| [Gateway build v3](gateway/gateway-build-v3-fee-source.log) | 成功，费用来源修正后、状态协议修正前 | 该快照 SHA `c7ddc929a6290070221f1808f5dfaafb3f7fdfdf2792d128e4e0a934271806b4`；最终状态协议构建和实际运行结果另行记录 |
| [Gateway build v4](gateway/gateway-build-v4-wire-state.log) | 成功，状态协议修正后、采购响应世界快照补齐前 | 保留真实成交验收所用的中间构建日志；最终资产 HUD/背包投影构建 v5 及其运行 SHA 另补 |
| [Gateway build v5](gateway/gateway-build-v5-asset-projection.log) | 成功，最终资产投影集成 | 最终运行 SHA `4b5bc8a10793583e0631ab9145862d86ad209c8a58f1481f1875f6892569060c`；构建含成功采购响应后的权威世界快照，以刷新普通 HUD 与背包。最终实际操作另见实连记录 |
| [客户端初始证据清单](client/initial-fixture/manifest.json) | 原始固定快照 | Windows 协议桥 3/3、共享协议 1/1、初始 native 9/9；五页及两种确认的 7 张生产 GPU 渲染截图。原始 sourceFiles 属于当时快照，不冒充最终客户端源码 |
| [客户端逻辑快照](client/final-fixture/native-focused.log) | 11/11，另 1 ignored | 最后一次费用校验 v3 修正前的客户端逻辑和相邻模态窗口回归；被 ignored 的 GPU 测试另行执行 |
| [客户端短期到期 GPU](client/final-fixture/gpu-short-expiry/procurement-native-gpu.json) | 1/1，2 张图 | 原生生产 renderer 的离线发布页与确认页，缺字/越界/溢出均为 0；明确 serverConnected=false、testnetTransactions=0、humanAccepted=false。此为 v3 费用修正前快照 |
| [Windows 构建快照](client/final-fixture/windows-build-exit.json) | exit 0 | v3 费用修正前 Windows 客户端构建及二进制 SHA；不把构建当作实连验证。最终实连构建另补 |
| [客户端 v3 逻辑](client/v3-fixture/native-focused.log) | 13/13，另 1 ignored | 包含按 status 中的完整费率规则校验报价、动态费用单位及来源严格匹配；没有把旧 GPU 截图冒充重新渲染结果 |
| [Windows v3 构建](client/v3-fixture/windows-build-exit.json) | exit 0 | 最终候选 SHA `1a8c941bac3ae9be03bf64652dc64b555d8699a6a65bd65c9ca532da6b0bc144`；不等于实际交付验收 |
| [Windows 实连](windows-live/README.md) | 三单闭环和双方最终重登通过，18 快照 / 9 笔真实交易 | Windows → Gateway → File 游戏资产权威 → 真实测试网 → 成交双方领取、取消退款领取、120 秒真实到期退款领取；含最终重登的 18 快照独立审计 passed=true。人类 Accepted / 原生 PG / 生产收费不据此通过 |

SQL 目录的 [README](sql/README.md) 列出 44 项范围：迁移重复应用、普通账号正常写删、托管 authority 的删/null/omit 和十类历史集合保护、状态推进、追加及事务失败回滚。[sql-test.log](sql/sql-test.log) 是保留的早期 37 项旧运行；当前结果以 `results.json` 和 `sql-test-schema12.log` 为准，不重复累计。

[Rust v14](rust/procurement-tests-v14-equipment-description-lease.log) 保留报价来源修正前已通过的 54 项快照；[v15 编译反证](rust/procurement-tests-v15-machine-fee-source.log) 记录新增测试首次引用了未依赖的签名库而编译失败，随后复用项目已有库修正，[v16](rust/procurement-tests-v16-machine-fee-source.log) 通过。v17 再补稳定状态字符串和真实 owner 请求中的公开列表/退款领取断言，是当前通过结果。未为此新增依赖。

初始客户端清单引用的 [gpu-v1](client/initial-fixture/gpu-v1/procurement-native-gpu.json) 是已修复问题的历史反证。最终 fixture 在 `gpu-final` 与 `client/final-fixture`，两者均不含真实游戏服务器会话或链上资产交付。

早期独立真实测试网样本使用测试币、测试资产托管证明：发布 2 笔；发布到成交、取消、到期分别各 3 笔。该批链证据明确 `custodyFixtureOnly=true`、`realDatabaseDelivery=false`，不能替代游戏交付。随后 [Windows 实连](windows-live/README.md) 是另一批 9 笔实际交易，`custodyFixtureOnly=false`、File 游戏权威实物交付，三单均已到账。两批范围和原始材料分别保存，Gas 不混算，不据此推断 0.10 美元收费盈利。

## 完整性与使用

所有包内 artifact 路径与仓库 source pin 路径均为相对路径。复制的原始日志/原始 JSON 可能保留实际执行时的本机路径；没有为美化路径改写原始证据字节。`.gitattributes` 禁用本目录文本换行转换，证据 artifact 始终按原始字节计数和计算 SHA。源码 source pin 则明确先将 UTF-8 文本的 CRLF 统一为 LF，再计算字节数和 SHA，避免仓库源码强制 LF 后失配；不做其他文本或 BOM 改写。

在本目录执行：

```text
node generate-manifest.mjs
node generate-manifest.mjs --verify
node generate-manifest.mjs --verify --verify-sources
```

[manifest.json](manifest.json) 记录每个公开证据文件的原始字节数、原始 SHA256 和组别。`sourceSnapshot` 是生成清单时按上述 LF 规则规范化的仓库源码快照；历史日志与初始客户端清单各自保留原始运行范围，不能用新快照反向认证旧日志。`--verify-sources` 额外检查当前 checkout 的规范化源码与快照一致；普通 `--verify` 只验证原始证据包。修改或追加实连证据后由协调者重新生成清单。

Windows 实连的电脑控制截图允许保留实际 JPG 格式，不改后缀或称作 native capture PNG。其 sidecar 应关联目标窗口、实际保存时间及对应订单状态证据；JPG/PNG 均按二进制原始字节哈希，不进行 UTF-8 私钥文本扫描。

脚本只读取本目录和明确列出的源码文件，不联网、不启动服务、不读环境或密钥、不执行测试。它拒绝符号链接、敏感目录/文件名以及原始私钥格式。`manifest.json` 本身不纳入自身哈希；其生成脚本、README 和属性文件均纳入清单。
