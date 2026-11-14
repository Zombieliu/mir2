# 阿洛：R16 Windows 伙伴与长期记忆

维护代码线：`codex/windows-player-journey`，主项目 `mir2-web3`。
基线包含 R16 `e9117b6691e3f7ec238ff1c1eb77b699b0646c2f`。
旧 `codex/wn-candidate-recovery` 的构建和测试是历史记录，不能作为本代码线的验收。

本轮实现的工程能力：

- 独立普通角色接受真实邀请，跟随、支援战斗、使用已有血药与危险撤退。
- Windows 游戏内教学、查看习惯、撤销或删除教学与经历。
- 自然语言生成三项软习惯的部分修改提案，逐项预览、确认或取消；明确否定也能教成 `false`。
- SQLite 保存身份、教学和行动记忆；重启保留已提交习惯。
- 完整签名导出，停掉 A 后向全新 B 目录导入并继续冒险。
- 主人显式确认后，独立向鲁本买一瓶小型生命药水，遵守游戏金币上限、时效和返程边界。
- 使用、自述、提醒、成本与付款来源分别记录；不把意向算作付款。

R16 现有 shared groups、owner health 和 Zone 继续裁定游戏规则。
不迁入旧 PartyHub、个人会话组队模拟或旧药品恢复桥接。
未配置模型时使用规则执行。可选 OpenAI Responses API 用于理解私人委托、检索有效记忆和提出有限目标/教学提案；程序持续执行普通动作，服务器裁定结果。显式授权的一瓶补给也由固定程序执行，不需要模型。购买运行服务的价格尚未配置，`offer:null`，与游戏内买药不是同一项。

最新迁移状态与最终 EXE、测试证据见
[迁移记录](../../docs/ALO-R16-MIGRATION-20261103.zh-CN.md)。

本次模型入口、验收范围与当前真实 API 验证状态见
[OpenAI 接入记录](../../docs/ALO-OPENAI-20261103.zh-CN.md)。模拟响应测试不代表真实模型已通过验收。

同日后续已通过真实OpenAI三次调用及Windows界面联调：2/2实际参战击杀、返队、不同Node进程保留习惯／进度，下一次模型计划准确引用旧击杀记录。详见上述记录顶部和独立live证据，不与此前模拟回执混记。

当前自然语言教学与有限补给的实现、测试和后续交付统一见
[培养与补给记录](../../docs/ALO-CULTIVATION-SUPPLY-20261103.zh-CN.md)。上述历史结果不能转记为新增真实模型调用、独立补给或新 Windows UI 已通过；这些实时验收仍需单独执行。

## Windows 本地试用

交付目录中双击 `Numeron-Alo.exe`，会启动独立的本地网关、阿洛运行器和 Windows 客户端。
启动时立即显示等待窗口；重复打开会唤回现有游戏或初始化窗口，不会再次启动配套服务。
首次自动初始化专用测试账号；以后继续使用同一世界、角色与 SQLite 记忆。
在游戏右侧打开“伙伴”，邀请组队，在“培养”页提交教学；等待保存回执再退出。
正常关闭游戏并等待后台保存结束，再次启动即可检查上一场的习惯与经历。

该目录保留 R16 的资源验证，从本机已验证的完整 R16 素材目录读取资源。
这是本机可玩的开发交付，不是可在另一台电脑直接安装的完整素材包，也不会更新公网服务。
不要移动原素材目录；实际输入与二进制 SHA-256 见交付内的 `BUILD.json`。
启动后的账号、密钥、个人记忆和日志保存在本目录 `apps/ai-player/.data`，不要将它公开分享。
此目录应保留，以便下次继续使用同一个阿洛。直接运行 `game` 下的游戏 EXE 不会启动配套服务。

## OpenAI 私人委托

双击交付根目录的 `Numeron-Alo-Settings.exe`，由本人在本机填写 API key。默认 `gpt-6-luna`，模型名可编辑。
保存后重启本地试用，在“伙伴 → 私人委托”输入：
“在附近打两只怪，再回来找我，途中别捡东西。”
需先邀请阿洛组队、一起接近可见怪物。当前目标支持跟随、最多三只的附近狩猎、返回固定比奇安全点和等待。
它不会跨地图寻找不可见怪物。实际参战击杀要求阿洛命中后收到对应死亡事件；模型回复不增加计数。

每条新私人委托或教学生成各最多一次模型请求，共用每小时最多20次的额度（可配置更低）；不自动重试计费请求。确认/取消提案不调用模型，实时动作不逐帧调用模型。
模型网络支持标准HTTP(S)_PROXY／NO_PROXY及系统可信CA，仅作用于模型Actor。可选私有`openai-network.local.json`只保存无密码代理路由，环境变量优先；没有关闭TLS验证或修改系统代理。
模型请求仅携带有界的局部观察、三条习惯和相关有效经历，不发送账号、密码、附近聊天或运行配置。
密钥保存在当前 Windows 用户的凭证库；模型名和凭证引用在私有 `openai.local.json`，不在角色归档里。
换模型不换角色。重启保留经历和培养；旧委托暂停，需重新委托确认。召回、死亡或记忆变化会中断旧计划。

`node scripts/run-model-companion-acceptance.mjs --gateway-exe <canonical-exe>` 使用明确的模型测试替身和真实本地网关。
只有额外指定 `--live-openai <configured-bundle-directory>` 才使用本人已配置的本机凭证，发起一次真实请求；输出会区分两者。

## 自然语言教学与有限补给

以下是当前源码的界面/服务契约，新增入口须使用本轮相应交付，不能用历史 EXE 的截图推定已验收。

在“培养”页选择用自己的话教学，输入反馈并生成提案。模型只能提出 `assistBeforeLoot`（先支援）、`lootOnlyAfterCombat`（战后拾取）、`askBeforeLeaving`（离队先问）三个软习惯的非空部分修改；值可以是 `true` 或 `false`，没提到的习惯保持原值。界面显示每项当前值 → 建议值，点“确认这些习惯改动”并收到保存回执才生效；也可取消。预算、交易、购药和地图权限不能通过教学改变。原有固定教学按钮仍可在规则模式使用。

私人接口分别是 `proposeTeaching{text,expectedPolicyVersion}`、`confirmTeaching{proposalId,expectedPolicyVersion}`、`cancelTeaching{proposalId}`。确认只引用运行器保存的提案，不能从客户端提交任意 patch；版本、来源记忆或世界上下文变化会使旧提案失效。重启保留已确认反馈，待确认提案不会作为可执行授权恢复。实际调用与测试替身必须分别记录。

补给默认没有支出授权。在“补给”页查看并显式确认：地图 `0` 的鲁本 `Merchant_Ruben`（NPC9，288,608），买 **1 瓶** `(HP)DrugSmall`（模板658），最多 **50游戏金币**，有效 **300秒**。当前 Windows UI 固定提交 `authorizeSupply{expectedPolicyVersion,budgetGold:50,quantity:1,expiresInSeconds:300}`；服务端允许预算收紧，拒绝扩大数量、时效或上限。这不授予拾取、卖物、交易和换图动作。

阿洛在比奇 `x=270..310、y=595..635` 内按真实碰撞与当前视野行动。返程点取自主人的服务器权威坐标；到授权点一格内才算到达，主人仍占格时停在邻格。自保、用已有血药和危险撤退仍优先，补给不带来免伤或新游戏资产。

服务器先持久化不可花费的 draft，运行器校验培养版本后回 ACK；只有版本、permitId 和 `authorized` 状态匹配才 activate。Conflict、错误、超时或断线不得用新 draft 替换旧有效授权。买药前，运行器先保存原操作，服务器先持久化整个预算的 reservation，再调用普通受限交易并记录真实收据。普通 `BuyItem` 或无上限购买接口不能绕过这个边界。

“已授权”不代表已买到。“完成”需要真实扣款、一瓶入包、同源完整状态确认和实际返程。缺钱、报价超额、路被挡或结果未知都应查看具体状态。可以撤销补给或召回；取消幂等，旧请求重试不会撤掉后来新授权。已提交购买不会因取消自动退款。未知结果与重启恢复只查询原操作，不重买、不自动恢复支出，也不凭换模型、删记忆或导入角色重新得到预算。

## 配置与运行

要求 Node.js 22.18.0；模型网络依赖精确锁定 `undici@7.30.1`。源码运行先执行 `npm ci --omit=dev --ignore-scripts`，Windows交付已包含该依赖。Rust 使用项目工具链 1.95.0。
以下路径均相对本应用，账号密码和签名密钥不写入 profile、Git 或命令行参数。

1. 为主人和阿洛准备独立的正常游戏账号、角色及角色索引。
2. Gateway 配置明确的 `MIR2_REALM_ID` 和 `MIR2_COMPANION_POLICY_PATH`。
   policy 绑定主人／阿洛的最终 canonical 账号和角色索引，固定 `worldEpoch`。
   缺少 policy 时私人控制关闭，非法配置或不支持的世界权威拒绝启动。
   受限补给另需 Gateway 独占的 `MIR2_COMPANION_SUPPLY_JOURNAL_PATH` 与专用32字节
   `MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY`；本地启动器管理其私有配置，不写入角色 profile。
   该 key 与档案签名 key 分开，不进入阿洛/Native 环境、导出或公开证据。既有 journal/head
   丢失、损坏或回滚时拒绝继续花费，不能删文件重置预算。
3. 复制 `profiles/alo-experience.example.json` 为私有 `.local.json`，填写对应的
   URL、realm、epoch、角色、archiveId、ownerRef、memoryPath 和 runtimeId。
4. 通过 `MIR2_AI_PLAYER_ACCOUNT`、`MIR2_AI_PLAYER_PASSWORD` 提供阿洛的专用账号。
   `archiveKeyEnv` 指向外部 32–64 字节 hex 密钥；不把实际密钥写进 profile 或导出包。
5. 保持 `provider/model` 为 `rules`，价格 `experiment.offer` 为 null。

```powershell
node src/main.mjs --profile profiles/alo-experience.local.json --duration 3600
```

远端使用 WSS。支持普通 `/ws` 和 R16 `/playtest/ws` 路由；拒绝 URL 凭据、查询参数和
fragment，不绕过服务端 Origin、账号认证或权限校验。已有账号登录失败不会创建替代账号。

主人正常登录配套 EXE，在“伙伴”页邀请入队、教学、召回和查看记忆。所有修改必须等可信
保存回执；发送请求或选择行动不等于成功命中、拾取或交付。未知结果先刷新核对，禁止
自动换 requestId 重发修改。其他人的附近聊天不能教学，组队关系本身也不授予控制权。

阿洛使用普通已有装备与药品，默认无金币支出授权。只有主人在补给页显式确认后，才执行
上面的一瓶受限采购；没有任意采购、出售装备、任意传送、离线免伤或生成奖励。
实时移动、命中、死亡、掉落和药品消耗仍由服务器裁定。

## 记忆与换运行器

`characterArchive` 保存固定身份与基础设定；`playerGrowth` 保存习惯、教学、经历及删除
墓碑；`runtimeService` 保存服务商、模型、路径和凭据引用。只有前两项进入完整 HMAC 归档。
导出不包含签名密钥、游戏资产、生命、位置、背包、队伍在线状态或服务配置。

当前 SQLite schema为3，旧schema1/2原子迁移并保留已有ID、顺序和培养版本；绑定主人/世界的
schema1接纳仍须显式本机操作。新导出为 `mir2-companion-archive/2`（version2），包含部分习惯
修改及其来源。严格支持旧v1导入，但v1即使签名正确也不能夹带新版教学patch；导入会交叉
检查习惯、有效反馈和来源。DB schema版本与培养 `policyVersion` 是不同概念。

可迁移成长不包含可执行提案、补给permit、server journal或reservation。已发生的补给经历
可以成为记忆，但复制/导入记忆不复制支出权，换运行器也不能清掉未知订单或重置预算。

游戏内可完整导出；也可在停掉服务后执行：

```powershell
node src/memory-cli.mjs inspect --profile profiles/alo-experience.local.json
node src/memory-cli.mjs export --profile profiles/alo-experience.local.json --output .data/alo-portable.json
node src/memory-cli.mjs import --profile profiles/alo-experience-runner-B.local.json --input .data/alo-portable.json
```

B 使用全新 memoryPath 和 runtimeId，保持 archive／owner／realm／epoch／角色绑定一致。
A 必须实际停止。本机档案锁和网关在线登记会阻止并发运行；这不是跨主机分布式锁。
导入拒绝非空目标、篡改包、错误密钥和外来绑定，不合并或覆盖旧成长。撤销教学与删除
墓碑随包保留，重试不能复活它们；清除成长保留身份与服务器角色。

导出采用独占文件创建，完整上限 2 MiB，不截断历史；EXE 核对长度与 SHA-256 后写盘。
默认目录为 `%APPDATA%/mir2-web3/companion-exports`，界面显示实际路径。

## 体验与成本记录

`experience-cli.mjs inspect|record|report` 只管理本地运营台账，不发送邀请、收费、退款或采购。
自述、运营者确认、实际服务器观察、估价和已发生费用分别记录。没有真实依据时不填付款。

```powershell
node src/experience-cli.mjs inspect --profile profiles/alo-experience.local.json
node src/experience-cli.mjs report --profile profiles/alo-experience.local.json --output .data/participants.csv
```

金额以带币种的整数 micros 字符串记录。未计量的托管、支付和分账成本保持 null；人工支持
另记秒数。缺成本时不计算贡献收益。规则模型调用为 0，不等于运行和人工成本为 0。
本轮全部工程 fixture 使用 test 环境，不能计作真人回访、真实收入或付费使用。

## 本地重新验收

先在仓库项目目录执行代码线守卫和构建：

```powershell
python scripts/verify_unified_codeline.py
cargo +1.95.0 build --locked -p mir2-gateway --bin mir2-gateway
node --test apps/ai-player/tests/*.test.mjs
node apps/ai-player/scripts/run-local-acceptance.mjs --gateway-exe target/debug/mir2-gateway.exe
node apps/ai-player/scripts/run-local-companion-acceptance.mjs --gateway-exe target/debug/mir2-gateway.exe
node apps/ai-player/scripts/run-local-experience-acceptance.mjs --gateway-exe target/debug/mir2-gateway.exe
node apps/ai-player/scripts/run-supply-acceptance.mjs --gateway-exe <canonical-exe> --out-root <absolute-private-output-root>
```

这些脚本使用私有环回网关、随机账号和隔离存档，不继承远程数据库或管理配置。
只通过正常登录、移动、战斗、丢下和拾取原生物品制造场景；不使用 QA 发物、改血或传送。
显式记录正常路由租约与 bootstrap 等待，不声称即时重连。

供给脚本是待实跑的验收入口，不是已通过证明。它只为独立合成角色，通过普通商人卖出其
自有开局剑/衣筹备测试金币；这是测试准备，不能称打怪收入或赋予产品卖物能力。结果须保留
真实收据、实际返程、撤销以及不同进程只Query恢复的证据；明确标识注入的结果丢失故障。

详细权限与协议见[控制合同](../../docs/COMPANION-CONTROL-CONTRACT.md)。
公网 R16 安装、更新通道和服务器是否已部署本功能是另一项事实；本地构建不自动发布。
