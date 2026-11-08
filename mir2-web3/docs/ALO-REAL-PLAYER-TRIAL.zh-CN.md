# 阿洛 Windows EXE 真人试点执行手册

日期：2026-11-03。当前状态：P0 普通真实网络 12/12 已通过，实际 Windows 原生窗口
已验证部分操作，剩余 GUI 和完整 Gateway 回归仍在进行。本文件是待执行手册，不是已招募、
已回访或已付费的结果报告。
本轮交付入口为 `mir2-platform-windows.exe`，不以 Web 页面测试代替原生客户端交付。

当前已知验证：最近一轮 Node 163/163、Gateway companion 15/15 与 PartyHub 15/15、
ui-core 52/52（含十项新增检查）、完整 client-bevy native-ui 391/391（含五项新增 ECS）、
完整 Windows 平台 325/325（隔离组合资源环境），以及 Release 构建和 locked check。
普通真实网络 P0 12/12 的私有源报告为
`apps/ai-player/.data/experience-47fc7b3a-f156-4bd1-93b4-8768549e03d1/summary.json`；
它使用普通游戏网络、真实 Node／CLI 的 A 停止→B 新路径导入及下一场冒险，明确标记
`environment:test`、syntheticParticipants=true、realHumanTrial=false、paymentPerformed=false。
统一脱敏归档目录为 `docs/generated/player-qa/alo-experience-2026-11-03/`；
工程 fixture 不计真人、付款或自然回访。
历史 B2 陪战证据继续保存在 `alo-companion-2026-11-03/`，不能替代本轮 P0 验收。

已进行窗口操作的中间构建 `mir2-platform-windows.exe` 为 67,677,696 字节，SHA-256：
`16589f6e57ea35e7aada3353bb69d151240241d4a9e825ce6b68efac92683e7c`。
此构建已实际验证教学 `v1→v2`、普通邀请组队、撤销至 `v3`、删除教学正文、再次教学至
`v4` 和 EXE 完整导出。原生窗口的 A→B 流程、成本页及最终中文调整仍在进行；完整
Gateway lib 回归及调整后的最终二进制 hash 待确认，不据此宣布 GUI 全通过或立即开始真人试点。

接口依据：[P0 控制合同](COMPANION-CONTROL-CONTRACT.md)、
[运行 profile 示例](../apps/ai-player/profiles/alo-experience.example.json)、
[客户端运行说明](../apps/ai-player/README.md)。当前教学是规则与模板解释，没有调用大模型。

## 1 试点要观察什么

招募目标为 6–10 位真实参与者，其中包含非开发者。每位安排三次、每次约 20–30 分钟的
体验；实际提前结束、缺席或中断均照实记录，不补写为完成。

| 阶段 | 实际操作 | 要保留的证据 |
| --- | --- | --- |
| 每人 D0 首次体验 | 正常登录 EXE、邀请阿洛、一起战斗、在伙伴页教学、确认下次选择改变 | 实际进入及结束时间、本人理解、服务端确认、干预与失败 |
| 每人 D1–3 回访 | 记录是否提醒和收到几次提醒；重新选择伙伴并继续冒险 | 本人自述与运营触达分别记录；无回访留空／标未发生 |
| 后续第三次选择 | 允许继续选阿洛、选实际可用的其他伙伴或停止；选择原因由本人填写 | 是否因为共同经历、战力、设置便利、好奇或其他；真实服务使用 |

两周观察窗口从**第一位真实参与者首次实际体验**开始，不从工程脚本、创建 profile
或填写本手册的日期开始。全局窗口起止与每个人的 D0 分开记录；不在没有真人参与时
预填日历。第三次未发生就记录未发生，不把预约、付款或续购当作一次回访。

参与者无需理解模型设置。实验中如有多个已授权伙伴，保留真实可用的选择条件，不
伪造另一个“新人”来制造选择结果。招募、邀请和提醒由运营者按实际授权执行，本轮
工具只记录已发生的联系，不主动发送消息。

## 2 启动前配置

运营者先准备正常真人账号、独立 Agent 账号及现有游戏角色，确认已验证的 EXE
构建、网关地址、素材与中文字体可用。不要把账号、密码、实际签名密钥或原始私有
policy 粘贴进本手册、截图、Git 或公开验收报告。

1. Gateway 明确配置 `MIR2_REALM_ID`、`MIR2_SHARED_PARTY_ENABLED=1` 与
   `MIR2_COMPANION_POLICY_PATH`。policy 内 `worldEpoch` 是运营者持久保存的世界代际，
   不随重启生成新值。缺少 policy 时此功能关闭，错误配置拒绝启动。
2. 按控制合同填写 enrollment 的真实 owner／actor 账号与角色索引，以及稳定
   `residentId`、`archiveId`。`ownerRef` 由 Gateway 对规范化后的 owner 账号 ID 求
   SHA-256 得到，不从昵称、组队关系或玩家自述推断权限。
3. 复制 `alo-experience.example.json` 为同目录的私有 `.local.json`。填写实际 realm、
   epoch、archive、owner、Agent 角色索引、队长名、memoryPath、runtimeId 和实验绑定。
   每个参与者用稳定的匿名 participantRef；同一人不同伙伴仍归同一 participantRef。
4. `MIR2_AI_PLAYER_ACCOUNT`、`MIR2_AI_PLAYER_PASSWORD` 和 profile 中
   `archiveKeyEnv` 指向的外部密钥由运营环境供给。归档签名密钥为 32–64 字节的 hex，
   不写进 profile 或导出包；`credentialRef` 当前为 null。
5. 当前 `provider`、`model` 都是 `rules`；不要把模板教学描述为大模型学习。
   `experiment.offer` 保持 **null**：用户尚未给定价格，购买意向入口应处于关闭状态。
6. 工程／自动测试使用 `environment:"test"` 和独立 ledger；真人试点使用自己明确
   绑定的 `trial` ledger，不能将测试 payment、grant、visit 或 reminder 混成真实数据。

三类对象必须分开：

| 对象 | 保存什么 | 换运行服务时的处理 |
| --- | --- | --- |
| `characterArchive` | 稳定 archiveId、主人／世界／游戏角色绑定、基础外观与设定 | 保留身份，不随进程、目录、模型或 provider 改变 |
| `playerGrowth` | 习惯、教学、经历、撤销／删除墓碑及已有来源的关系 | 完整导出／导入；没有关系证据时保持空 |
| `runtimeService` | runtimeId、provider、model、promptVersion、路径、credentialRef、运行授权 | 在目标环境重新配置，不放入 portable archive |

游戏资产仍由服务器保存：位置、HP、装备、背包、金币、任务和在线队伍不从记忆包
恢复。导入不能扩大预算或权限。schema 1 升级保留原记录和策略版本；补充 owner／epoch
必须由本机运营者显式 `adopt-legacy`，不能替换已有主人或世界绑定。

## 3 运行与原生窗口入口

以下 Node 命令在 `apps/ai-player` 目录执行，路径是待填写的本机私有配置，不代表已
创建真实参与者。先由运营者配置外部环境，再启动 Agent 服务：

```powershell
node src/main.mjs --profile profiles/alo-experience.local.json --duration 3600
```

真人打开运营者提供、带本轮校验值的 `mir2-platform-windows.exe`，正常登录已授权的
owner 角色。原生网关连接使用现有 `MIR2_GATEWAY_WS_URL`／`mir2-client.toml` 配置；
账号密码通过登录流程或现有专用环境配置供给，不写入 TOML。没有本轮正确构建时
先完成工程验收，不让真人以旧 EXE 的结果代替本轮结果。

当前源码中的原生入口为“伙伴 Companion”，面板含：

- **培养与体验**：邀请组队、召回、恢复行动、当前习惯和“教它：组队先支援，战后再拾取，离队先问”。
- **记忆管理**：分页查看教学、撤销、删除文字、清除全部成长；危险删除有明确确认。
- **行动与成本**：区分 selected 决策与确认结果，查看个人累计使用、未知成本与未来服务状态。

该入口已经实现，窗口已通过前述教学、组队、撤销、删除与导出；其余 GUI 验收仍待完成。
按钮发出请求不等于成功。只有相关
SQLite 提交和可信回执后才显示新版本。`unknown` 时刷新当前状态再决定下一步，
不自动换 requestId 重试消费或修改。未授权角色不应看到别人的伙伴、记忆或导出。

## 4 一次体验怎样执行

1. 在“培养与体验→体验自述与选择原因”中，由本人选择“自己想来／收到提醒／约好时间／
   不确定”和提醒次数，再开始本次体验。本人不确定时保留不确定，运营者不代填成自然回访。
2. 邀请组队，等真实名单确认；正常走路、打怪、吃已有药和危险撤退。真人仍亲自操作自己
   的角色。不要用 QA 发物、改 HP、传送、加速 tick 或伤害倍率制造通过结果。
3. 在安全条件下观察一次收集偏好；从 EXE 伙伴面板应用教学。下一次相似交战中确认
   阿洛先支援，战后实际拾取。解释“生存和硬权限先于习惯”，不把每一次动作差异都叫学习。
4. 查看对应教学和经历，指出 `selected` 只表示选择，不能算命中、击杀或交付已完成。
   本人填写是否理解培养效果与原因；模板解释不充当事实依据。
5. 在记忆页撤销一条教学，确认习惯重算；在需要时删除正文或清空成长。删除源经历不能
   留下仍生效的教学，旧 ID 重放不能复活已撤销内容。清空保留身份与游戏存档。
6. 记录本次结束。操作失败、断线、人工解释、支持用时和退出原因均保留，不能只写成功。

先在工程账号完成删除、清空和迁移演练，真人不必为了测试而删除自己希望保留的经历。
真人真正请求删除时执行真实删除，不以“展示过按钮”代替。私人内容只走当前主人通道，
不进入 worldSnapshot、AOI、通用调试历史或旁观者画面。

## 5 导出、换服务与继续冒险

EXE 的“导出角色与完整成长”按清单分块接收、核对长度与 SHA-256 后写文件。
默认目录来自 `%APPDATA%`（或 `%LOCALAPPDATA%`）下的
`mir2-web3\companion-exports`；可由绝对路径 `MIR2_COMPANION_EXPORT_DIR` 覆盖。
面板显示实际写入路径，“打开导出目录”用于定位。导出不要求购买服务。

导出上限为完整 2 MiB，超过上限明确失败，不悄悄只导出最近 100 条。HMAC 使用
目标运行环境已信任的外部主人密钥校验；包内声明和校验和不构成游戏事件公证。

停掉 A 并正常保存退出后，运营者也可离线导出：

```powershell
node src/memory-cli.mjs inspect --profile profiles/alo-experience.local.json
node src/memory-cli.mjs export --profile profiles/alo-experience.local.json --output .data/alo-portable.json
```

准备 B 的私有 profile，保持同一 archive／owner／realm／epoch／角色绑定，换一个
**全新** memoryPath 和 runtimeId。模型及凭据在 B 侧配置，不从包里恢复：

```powershell
node src/memory-cli.mjs import --profile profiles/alo-experience-runner-B.local.json --input .data/alo-portable.json
node src/main.mjs --profile profiles/alo-experience-runner-B.local.json --duration 3600
```

确认 A 的真实进程已停止；仅换 guardPort、路径或 runtimeId 不能绕过同档案本机锁。
Gateway 另拒绝同一已登记角色的并发在线注册。该机制不等于跨主机离线分布式锁。
导入只接受新目标和明确匹配的绑定；错误密钥、外来或篡改包、非空目标拒绝。相同
快照的精确重试幂等，不支持合并／回滚现存成长；旧包不能覆盖已清空的现有档案。
导出文件采用独占创建，不覆盖已有同名文件。

在 B 中实际重新受邀、开始下一次冒险，核对同一身份、旧经历、习惯和真实行动。
保留已有路由租约／bootstrap 等待并记录实际耗时；历史 B2 脚本曾明确等 31 秒和 16 秒，
不能写成即时重连。迁移完成是工程能力，不等于用户已经偏爱阿洛或愿意续费。

## 6 使用、收入与成本怎样记

当前价格未确定，`offer:null`，不开放购买意向，不发生自动扣款或服务发放。若运营者
随后明确一种固定价格、币种、时长和冒险额度，再配置唯一 offer；同一 experiment
中的固定 offer 不允许变价，不混合币种。服务费购买的是**未来使用**，不能以停止
购买为由扣留旧身份、成长、查看、删除或导出权限。

必须分开：

| 记录 | 来源 | 不能推导成什么 |
| --- | --- | --- |
| visit、原因、提醒次数、理解反馈、purchaseIntent | 参与者自述／按钮意图 | 自然回访证明、真实付款、服务完成 |
| invitation／reminder、payment／refund、grant、实际费用 | 运营者逐项确认和引用 | 工具已发消息、已实际扣款、已发生参与 |
| runtime／sharedParty 秒数与自有角色同队命中 | 运行器实际采样与服务器观察 | 真人持续注意、完整通关、完整击杀贡献或商业留存 |

`trial` 统计范围是**同一参与者跨全部已授权伙伴**：visit、购买、runtime 和费用均累计，
不是只计算当前面板选中的阿洛。`selectedCompanionSelectionCount` 单独描述当前伙伴
被选择的次数；不要把各伙伴选择数相加当独立真人或访次数。不同 owner 的数据不合并。

付费使用还需要已确认 payment、对应有效 grant，以及额度和时限内的真实观察事件。
同一 visit／角色不会因重复命中无限计数；新付款或续购本身不创造新 visit、冒险或回访。
退款不得超过实际付款，已退款的款项不能重新开 grant。付款、提醒、退款等保留唯一
operationId 与引用，重试同一操作不另造一笔记录。

金额使用带明确币种的十进制字符串 `amountMicros`，按百万分之一货币单位计，不用
浮点数。model、hosting、payment、revenueShare 区分 actual 与 estimate；缺失费用
保持 null，不补 0。只有所需实际费用齐全时才计算贡献，估价不混入实际贡献。
支持劳动单列 `supportSeconds`，不能默认为免费，也不把该贡献直接叫完整净利润。
规则执行器模型调用数为 0，并不说明托管、支付和人工支持费用为 0。

## 7 手工运营台账 CLI

命令只读／记录本地运营台账，**不会进行外部收费、支付退款、服务采购或邀请提醒发送**。
只在有实际依据且操作获得授权后记录。原始 receiptRef、account 和私人联系方式存放在
运营侧，不放进本仓库结果表。以下输入文件由运营者按真实事实创建，不提供假记录值：

```powershell
node src/experience-cli.mjs inspect --profile profiles/alo-experience.local.json
node src/experience-cli.mjs record --profile profiles/alo-experience.local.json --input .data/operator-record.local.json
node src/experience-cli.mjs report --profile profiles/alo-experience.local.json --output .data/participants-report.csv
```

record 输入的顶层仅为 `operationId`、`kind`、`payload`：

| kind | payload 必填字段 |
| --- | --- |
| payment | paymentId、offerId、amountMicros、currency、paidAt、receiptRef；金额须等于当前固定 offer |
| refund | paymentId、amountMicros、currency、occurredAt、receiptRef |
| grant | grantId、paymentId、startsAt；时长／额度来自固定 offer，不自行填写 |
| cost | category、amountMicros、currency、basis（actual 或 estimate）、reference |
| support | seconds、reference |
| invitation／reminder | channel（manual／wechat／email／other）、occurredAt、reference |

时间为实际 Unix 毫秒整数，金额为非负十进制 micros 字符串，ID 与引用使用合同允许的
有界字符。没有真实依据时不创建 payment、grant 或 reminder。CSV 独占创建并转义公式
前缀；保留未知项和每个人的明细，不把配置行或工程测试行计为实际用户。

`inspect` 和 `report` 会按 profile 注册台账 participantRef；**注册配置不等于真人参与**。
以有真实来源的 visit 和实际体验记录区分“已配置、未参加”和真正到访。

## 8 真人逐人结果表（空白模板）

以下 P01–P10 仅是预留栏位，没有对应已招募的人、已发生的日期或零值。
确认实际参与后再填写；未观察、未发生、拒绝与缺失分别标注，不将空白改成成功。

全局首次真人开始时间：________　两周窗口实际截止：________

| 预留编号 | 非开发者？ | D0 实际体验与干预 | D1–3 回访／自述／实际触达 | 第三次选择与原因 | 教学理解与迁移后行为 | 实际记录／证据 |
| --- | --- | --- | --- | --- | --- | --- |
| P01 |  |  |  |  |  |  |
| P02 |  |  |  |  |  |  |
| P03 |  |  |  |  |  |  |
| P04 |  |  |  |  |  |  |
| P05 |  |  |  |  |  |  |
| P06 |  |  |  |  |  |  |
| P07 |  |  |  |  |  |  |
| P08 |  |  |  |  |  |  |
| P09 |  |  |  |  |  |  |
| P10 |  |  |  |  |  |  |

| 预留编号 | 意向／实际付款／退款 | grant 与实际服务使用 | 实际费用与未知项 | 支持秒数 | 贡献是否可计算／金额 | 停止／不续用原因 |
| --- | --- | --- | --- | --- | --- | --- |
| P01 |  |  |  |  |  |  |
| P02 |  |  |  |  |  |  |
| P03 |  |  |  |  |  |  |
| P04 |  |  |  |  |  |  |
| P05 |  |  |  |  |  |  |
| P06 |  |  |  |  |  |  |
| P07 |  |  |  |  |  |  |
| P08 |  |  |  |  |  |  |
| P09 |  |  |  |  |  |  |
| P10 |  |  |  |  |  |  |

工程通过只证明可执行的功能与边界。实际真人回访、购买、续用和服务贡献需上述
真实数据；本轮不据测试夹具宣布已成立，也不表示已获得 Sui 的认可或共同投入。
