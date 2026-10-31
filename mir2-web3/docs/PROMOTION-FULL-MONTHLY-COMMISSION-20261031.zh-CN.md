# 合格月卡按实付金额全额计佣

日期：2026-10-31，Asia/Shanghai。

状态：代码已推送，官网与原测试服后端已发布，当前 Windows 测试赛季已启用全额计佣。原服实际月卡的待结算佣金已由 0.164839 USD 更新为 1.00 USD；原业务数据保留。付款开关仍关闭，没有生成转账、付款或结算单，不启动 Docker，不发布 Windows 客户端。

## 用户确认的规则

用户确认：赛季内购买并激活、满足邀请及真实游玩资格的合格月卡，按核实后的实际付费价值全额计佣；取消月卡服务期与赛季剩余天数的比例折算。保留前 50 名 10%、第 51–200 名 20%、第 201 名起 30% 的分段费率，同一张月卡只计一次，退款扣回对应佣金。充值点券本身不计佣，赛季结束核账后统一结算，当前付款开关不打开。

此次不启动 Docker，不发布 Windows 客户端或更新器，不修改旧邀请、旧充值、月卡、账号角色或存档，不生成链上付款。已有真实月卡消费按新口径重新计算待结算预估，不能直接改写收入金额或手工增加佣金。

## 实现与历史兼容

- 新活动明确配置 `rules.revenueRecognition = monthly_purchase_full`，算法版本为 `numeron-segmented-full-monthly-v3`。官网管理界面新活动默认采用此策略；正式活动仍需其既有审批流程。
- 缺省字段的旧活动仍采用 `numeron-segmented-v2` 的服务期比例算法；已冻结的结算继续使用原快照，不按当前规则重新解释。
- 全额算法保留真实付款及激活在活动半开区间内、玩法资格、金额上限、退款、唯一付款和月卡凭据等检查。只取消时长折算与时长覆盖额度；未激活、来源不明和不合格消费不生成佣金。
- 已激活的续费月卡即使服务期延伸至季后，也按全额策略导入；导入前仍检查付款与激活属于该季，防止旧季抢占后续季的唯一月卡 UID。
- 资格版本、佣金运行记录的输入 hash、收益接口和新结算使用真实选定的算法版本；离线 `promotion_recalculate` 根据冻结版本复算新旧两种结果。
- 当前 Windows 测试赛季使用有审计的单活动修改入口：仅限 test、精确活动 ID、既有 Operations/Finance 两个不同身份、有规则 hash 的比较后更新、付款关闭且没有任何结算。该入口不创建管理员、不代替参与者签名、不伪称人工财务签字。操作和同事务投影均记录完成审计，精确重复提交不新增记录。

## 首轮实际诊断

活动 `numeron-windows-playtest-20261030` 在 2026-11-05 04:35:38.449 结束。团员于 2026-10-31 05:54:35.018 激活一张 30 天月卡，完整核实价值为 10 USD，原算法覆盖 427263431 / 2592000000 毫秒，因此预估佣金为 0.164839 USD。

真实资金来源按点券批次 FIFO 对应原 C7 订单的 100 点券和新 `usdc_9923…8e25a` 订单的 900 点券，共消费 1000 点券；另余 100 点券。这些来源和消费均已有权威凭据，不需要再次充值或再次激活月卡。新策略应给该合格团员确认 10 USD 计佣收入、1 USD 待结算佣金。

## 验证与发布记录

- 后端源码：`13fc2e9f60203e65df152792d9ee63fb059065db`；[精确构建与原生 PostgreSQL CI](https://github.com/Zombieliu/mir2/actions/runs/41136078361)。
- 前端源码：`3a52daef5c3fe92e1e738d6656b9fdbf54de56c3`；[候选构建](https://github.com/Zombieliu/mir2/actions/runs/41135922836)。111 项本地相关测试通过，TypeScript、最终同盘 Next production build 及 6 个模拟接口浏览器场景通过。模拟截图不代表真实钱包登录或真实付款。
- 新增后端纯逻辑与原生 PostgreSQL 用例覆盖季末全额、续费、后续活动防抢占、分段、部分及全额退款、冻结基数与原费率、资格、重复凭据、权限和规则 CAS、预览零写、原业务保留、已有结算拒绝及精确重放。
- 新发布辅助工具只允许新批次、原 28 条 SQL 迁移及全部旧业务保留、自然空闲和实际 Redis owner fence、正常保存停服、新备份及同源 Gateway/worker。与旧迁移发布失败恢复工具分离。
- 隔离验收使用原库强制只读的一致性快照，在两个全新库完成正常修改、拒绝与保留检查；同工件离线复算新旧实际输入结果。该阶段不修改原库。

精确 CI 已成功，实际执行 80 项，0 跳过：原生 PostgreSQL 及计费组 58、CLI 组 14、钱包身份组 8。[制品收据](evidence/promotion-full-monthly-20261031/BACKEND-ARTIFACT.json)记录了认证 ZIP、完整 1830 文件清单、规范归档及四个 ELF 的 SHA256；协调者独立重验归档、清单、认证元数据与公开收据。

官网候选 `dpl_F1ZwKtAph1YVBVYHdVfKVWhfBZ9S` 已经原项目发布，[提升 CI](https://github.com/Zombieliu/mir2/actions/runs/41136625743)成功。两个游戏别名和 `/version` 精确对应前端源码，主站、游戏、推广路由及资源通过[公网只读验收](evidence/promotion-full-monthly-20261031/FRONTEND-PUBLIC.json)。宣传站原部署及三个旧活动保留。前端先于原服策略发布；该检查点如实记录旧折算口径，策略启用后的公网结果见本文末段。

[发布保护测试](evidence/promotion-full-monthly-20261031/RELEASE-GUARDS-LOCAL.json)、[隔离工具测试](evidence/promotion-full-monthly-20261031/CLONE-GUARDS-LOCAL.json)和[原服操作工具测试](evidence/promotion-full-monthly-20261031/ORIGINAL-GUARDS-LOCAL.json)分别 17、13、12 项通过，另有独立第二次只读复核。工具测试不冒充实际服务器结果。

[真实原库克隆验收](evidence/promotion-full-monthly-20261031/ACTUAL-CLONE-ACCEPTANCE.json)已经通过：完整月卡价值 10 USD，对应佣金由 164839 重算为 1000000 microUSD，即 1.00 USD。原库只读快照在两个新库验证，原财务、账号、角色、存档、邀请确认及资格时间保留；精确重放零新增，已有结算拒绝修改，旧 v2 与新 v3 的冻结输入均可离线复算。没有修改原库或执行付款。

[早先原服文件就位记录](evidence/promotion-full-monthly-20261031/ORIGINAL-STAGED-PENDING-IDLE.json)保留了当时仍有 1 个在线角色、仅暂存而没有停服或修改策略的检查点。用户随后确认完全退出 Windows 客户端；重新读取六项操作／连接计数、会话记录及 route lease 均为 0，实际 Redis owner lease 检查通过后才继续。

## 原服发布与真实计佣验收

[原服发布验收](evidence/promotion-full-monthly-20261031/ORIGINAL-PUBLICATION-VERIFIED.json)确认 Gateway／worker 均为上述精确 `13fc2e9f6020` 工件，正常保存、停止、离线新备份后切换；全部 28 条迁移和旧业务行保留，仅两张运行时时钟表允许有实际逐字段证明的 metadata 推进。旧配置原顺序、权限、UnsetEnvironment 及其他服务保留，未修改 Node／Caddy，也未执行旧充值恢复或邀请迁移操作。

首轮只读预览和随后 V1 的应用前检查曾因后台记录推进而拒绝；实际策略没有尝试，worker 已正常恢复，旧批次未重放。[拒绝记录](evidence/promotion-full-monthly-20261031/ORIGINAL-GUARDED-ATTEMPTS.json)及[真实 11 秒只读采样](evidence/promotion-full-monthly-20261031/SOURCE-HOLD-TIMESTAMP-READONLY.json)记录：来源核验每轮刷新 `promotion_revenue_source_holds.updated_at_ms`，付款、身份、金额、hold 状态和其他业务字段不变。

新 V2 辅助入口把整表 hash 与每行原始 PG JSON／SHA 绑定同一个 exported READ ONLY snapshot，完整比较所有已知及未知字段、类型、主键和行集合；仅预览前后以及预览到暂停 worker 后的比较，允许当前测试活动中 inactive、已核实、零退款行的 `updated_at_ms` 单调更新。实际应用前后仍逐行严格保留完整 source hold 表，不放宽账目保护。V1 文件保持原 SHA，V2 的[25 项真实函数保护测试](evidence/promotion-full-monthly-20261031/ORIGINAL-V2-GUARDS-LOCAL.json)及独立差异审查通过。

[新原服只读预览](evidence/promotion-full-monthly-20261031/ORIGINAL-POLICY-PREVIEW.json)的规则 hash、精确源码、原服 DSN hash、既有两个操作者身份及相同工件的克隆／发布凭据均经复核，使用全新批次，未复用克隆预览。随后[单次原服策略修改](evidence/promotion-full-monthly-20261031/ORIGINAL-POLICY-VERIFIED.json)状态为 `verified`：实际执行一次 policy apply，仅新增 1 条策略审计、2 条资格审计、3 个佣金运行及 2 个支持的资格出箱事件；旧审计、旧运行、财务、来源分配、角色、存档、邀请、资格时间、条款和其他活动均保留。Gateway 在策略修改时保持运行，worker 正常暂停后恢复相同源码；未回放或回滚任何账本。

北京时间 2026-10-31 11:32:21 的[最终原服只读核验](evidence/promotion-full-monthly-20261031/ORIGINAL-FINAL-READONLY.json)确认：125 账号／38 角色／38 存档／28 条迁移；同源服务健康，六项操作和连接计数为 0；当前规则 `monthly_purchase_full`，实际团长合格收入 10000000 microUSD、预估佣金 1000000 microUSD、算法 `numeron-segmented-full-monthly-v3`，付款关闭，唯一策略审计为 1。资格出箱均 completed／attempts1／无错误。既有 4 条 `payout.address.verified` 外部适配器失败仍保留，本轮未处理该付款能力；不能把计佣验收当作链上付款验收，也不能在 worker 恢复后声称全库完全静止。

北京时间 11:33:40 的[真实公网验收](evidence/promotion-full-monthly-20261031/PUBLIC-POLICY-ACTIVE.json)通过：原域名 `/version` 仍为 `3a52daef5`；默认和指定 Windows 活动 bootstrap 的 campaign／public 两处策略均为 `monthly_purchase_full`，计佣开启、付款关闭、reserve0，原 v1 条款和起止日期不变。全新无 Cookie Chrome、无 API mock、无写请求的 3 项检查全部通过：中文收益与规则页实际显示全额计佣、充值本身不计佣、退款扣回，390px 无横向页面溢出，控制台及页面错误为 0。已查看[桌面截图](evidence/promotion-full-monthly-20261031/PUBLIC-FULL-MONTHLY-DESKTOP.png)和[手机截图](evidence/promotion-full-monthly-20261031/PUBLIC-FULL-MONTHLY-MOBILE.png)。匿名页面没有访问钱包或核验登录态金额；真实 1 USD 的金额证据来自前述原服只读结果。
