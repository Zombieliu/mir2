# 普通攻击同步结果 V2 合同

本合同描述当前普通 WebSocket 攻击的可选回执。它不表示容量基线、上线验收或固定 Cell 分区已经通过。

## Wire 与能力

新客户端可在 `clientCapabilities.capabilities` 中声明 `nativeAttackOutcomeV1`；发送能力列表时应保留自身已有的其他能力。网关回应：

```json
{"type":"attackCapabilitiesV2","protocolVersion":2,"capability":"nativeAttackOutcomeV1","supported":true,"scope":"connection"}
```

该回应表示网关支持此连接上的协议接口。远端 Zone Host 仍须支持同名 typed RPC 能力；旧 Host 或不支持的运行时必须在执行前拒绝，不能回退到普通 RPC 或旧攻击。

攻击请求与明确结果示例：

```json
{"type":"attackV2","protocolVersion":2,"requestId":"1","objectId":206513}
{"type":"attackResultV2","protocolVersion":2,"requestId":"1","objectId":206513,"result":"launched"}
```

请求直接严格解析原始 JSON 字节，拒绝未知、重复、缺失字段或错误版本。`requestId` 是 `1..18446744073709551615` 的规范十进制字符串，不接受前导零、符号、空白或 JSON 数字；`objectId` 是非零 `u32`。结果必须匹配本次请求的版本、ID 与目标。

| `result` | 含义 |
| --- | --- |
| `launched` | 本次权威同步执行已起手；对应 `NativeAttackAdmission::Launched`。 |
| `notLaunched` | 同步路径成功完成并得到本次未起手的明确判定；对应 `NotLaunched`。 |
| `unavailable` | 未取得可用执行结果；`code` 为 `unsupported`、`executionUnavailable`、`authenticationRequired` 或 `requestIdNotFresh`。 |
| `unknown` | 执行结果不确定；`code` 为 `outcomeUnknown` 或 `previousOutcomeUnknown`。 |

`Launched` 不证明命中、正伤害或击杀，miss、格挡等仍由原战斗规则处理。`NotLaunched` 不提供具体拒绝原因；不能根据位置、`UserLocation`、空包、超时或客户端唯一 pending 请求猜测它。

## 认证、执行与防重

新入口要求有效认证会话、匹配的账号与当前角色，以及真实注册的共享 Zone actor。执行继续经过生产命令校验、owner lease、执行域与 teardown 检查。Session 在更新自身身份前核对返回命令为 `Attack`、包数一致、返回账号/角色索引/角色名与执行前完全一致。

起手判定取自本次 Zone 命令的原始出站结果，位于 FIFO 路由之前；此前 pending/predrain 包和其他请求的动画不参与判定。准入、rebase、计数溢出、teardown 或执行后错误不能被空包转换成 `NotLaunched`。成功回执在原普通身份更新、出站刷新及保存处理完成后发送。

防重状态仅在当前 socket 中保存，内存有界：一个单调请求高水位、一个 pending 请求、一个最近完成结果。首次执行前先预留 ID；同 ID、同目标、同完整角色绑定仅重发最近的不可变结果，不重放普通动画或伤害包。同 ID 改 payload、旧 ID 或已淘汰 ID不再执行；切换能力或在该连接中登录其他角色不会清空高水位。

执行入口返回 `Unsupported`、`BeforeExecution` 或 `Unknown` 时，网关尽力发送对应错误结果后关闭连接；执行后的身份、注册、flush、完成绑定或发送失败也关闭连接，不伪造成功/拒绝。预检查中的认证不足或旧 ID 可直接回复 `unavailable` 而保留连接，且不会执行攻击；未决结果为 `unknown` 时关闭连接。格式错误没有攻击执行语义。

客户端不得因超时、未知结果、旧服务器或不支持能力而换 ID、换端点或回退到旧攻击重试。typed RPC 在选定端点发出变更后采用单次执行；响应丢失不能向另一端点重发。

## 兼容与证明范围

旧 `{"type":"attack","objectId":...}` 及原二进制攻击协议保持原行为；V2 不增加客户端权限，也不改伤害规则和结算时序。新请求使用独立 tag，不能经通用 action mapper 降级执行。旧客户端不要求实现新回执。

浏览器请求 ID、防重状态和结果不是 Zone/checkpoint 持久字段，也不是经济 ledger。新连接是新 scope；断线后未决攻击不自动重放。本合同不保证跨连接、重启或迁移的 Exactly Once。

容量驱动通过 `--attack-results=v2` 显式启用，并先确认能力；没有自动降级。原攻击等待期限仍为 **900ms**，晚到结果或伤害不能补算本次按时成功。正伤害仍要求真实 owner 与 observer 的匹配 `ObjectStruck`、`DamageIndicator`、`ObjectHealth` 收据；起手回执不能替代这些证明。`notLaunched` 单独记录，不能计为正伤害命中；超时、错误和工作负载门槛不得由该回执豁免。没有 V2 回执的旧失败记录保持原判定。

源码依据：[严格协议与防重](../apps/gateway/src/attack_protocol.rs)、[WebSocket 接入](../apps/gateway/src/web.rs)、[Session 结果绑定](../apps/gateway/src/session.rs)、[同步权威执行](../apps/gateway/src/routing.rs)、[RPC 单次执行](../apps/gateway/src/zone_rpc.rs)、[容量驱动](../apps/web/scripts/playtest-capacity-soak.mjs)。
