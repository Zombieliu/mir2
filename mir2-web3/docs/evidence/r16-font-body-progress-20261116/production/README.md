# 8bf 同源资格通过，公网授权交接失败后成对回退

应用源码 `8bf3dbcbeae86c0f4782e2c7dcdd8dd203756efe`，发布控制源码
`133d1c790d2069a31ac8698d418034fa609b1176`。已配套试发，但原第一场公网失败于
`APPROVED_WALLET_LOGIN_NOT_OBSERVED`，尚未进入选角和游戏，第二场未执行。
**Web 和 Gateway 均已恢复 e0／Fo**，新字体修复没有留在正式站。
这次失败没有测到新修复的公网字体、运行包或 Native 性能，不能改写为它们超时。

## 同源资格与发布过程

- 新的 [Linux／PG 流水线](https://github.com/Zombieliu/mir2/actions/runs/43795056603)完成。
  独立下载并核对新源码包，**1,841 文件 SHA、310 项共享世界、216 项原生 PG** 全部通过；
  含 7 个原后台执行边界案例，PG 未使用 Docker、未跳过。
- ZIP 95,700,080 字节，SHA `4f3f294faed5b12e5a5580117bcbd854c28e5fa35d64eb501dd837ca7cee51f0`。
  ELF 93,576,560 字节，SHA `22bb54130b2084a34c31d7b0b7bd604cc99a260a08e8d924a5513b329ee4ec96`。
  [独立服务包核验](linux-package-verification.json)
- 首次 CLI 传输在 600 秒终止，保留原 67,059,712 字节；新尝试通过两个严格 HTTP Range
  续读，最终完整 ZIP 与 GitHub digest 一致。短期下载凭证只在内存中，原失败文件保留。
  这是构建包传输处理，未改变任何游戏门槛。[传输回执](artifact-transfer.json)
- 正式库的同一导出快照恢复到独立 PG，320／232／232 行与备份一致。匹配 ELF 在
  独立数据库、Redis 和 loopback 端口完成普通注册、登录、建角、移动 **106.792 ms**、
  中立怪行走、两次注销与同角色落点重登；旧行不变，QA／管理指令为零。
  [普通协议回执](ordinary-canary.json)
- 正式切换前确认会话、WS 和重连租约均为零，关闭两个自有 canary。切换只改变
  `MIR2_DEPLOY_REVISION` 和对应可执行文件路径；原行与原 24 份配置 SHA 保持。

| Web 操作 | 精确运行 | 结果 |
| --- | --- | --- |
| Stage | [43795530333](https://github.com/Zombieliu/mir2/actions/runs/43795530333) | 完成，新候选 AKHV |
| Check | [43796668955](https://github.com/Zombieliu/mir2/actions/runs/43796668955) | 完成，15 个关键文件、QR pending 与源码闭包通过 |
| Promote | [43800105588](https://github.com/Zombieliu/mir2/actions/runs/43800105588) | 曾切换 AKHV，公开文件与 pending 通过 |
| Rollback | [43800988484](https://github.com/Zombieliu/mir2/actions/runs/43800988484) | 恢复 Fo |

候选 `dpl_AKHVPVfrzTisZ2838JhDZPy9vbsu` 的独立 READY、项目、源码与实际 Worker
73,875 字节／SHA `2333c5f6c64c05c2485c65bf4965df859e25d0bc446f72ac72ab716885f4aba2`
均已核对。六提交／106 文件、两个检查点 90／23 文件及 88 项保护保持。
[Web 回执](web-jobs.json) · [候选字节](candidate-asset-readback.json) · [发布 pin](../release/release-pin.json)

## 原公网第一场失败

本场使用全新普通 Chrome、默认系统网络和官方手机尺寸 Testnet 钱包；23 个原辅助程序
只更新源码、部署、运行标签及钱包版本，原分类器 SHA 和 8／65／120、两次 20 秒、
完整六字体／113 门槛不变。

| 观测 | 原值 |
| --- | --- |
| 二维码交接 | 03:51:54.983 UTC，报告剩余 118,872 ms |
| 手机尺寸钱包完成签名 | 03:52:23.413 UTC，交接后 28,430 ms |
| 钱包行为 | 1 次点击、1 次精确登录批准、0 越权操作；TLS／CORS 及同一会话通过 |
| 桌面轮询 | 11 次 HTTP 200，观测状态只有 `pending`，HTTP 错误 0 |
| 渠道交换／游戏连接／LoginSuccess | 0／0／0 |
| 主线程观察 | 最大长任务 69 ms，记录到 1 个长任务 |
| 原分类器 | `accepted:false`，两端浏览器已关闭 |

签名完成时，报告的二维码有效期仍剩约 90 秒；观测不能支持“因等待手机太久而过期”
或“长时间主线程阻塞”的解释。它们也不足以断定是缓存、relay 复制或服务端逻辑问题。
当前服务器轮询代码已有 `cache:no-store`；下一步应对照请求、响应和 relay 状态实证复现，
不能凭猜测增加随机参数、扩大时限或原样重发。

本场没有建角、移动、任务窗或 Native 就绪测量。原分类器中的相应 false 是未达到，
不是新的 120 秒超时数据。第一场已不满足原资格，因此第二场未执行。
[原判定](original-r1-gates.json) · [授权交接观测](auth-handoff-diagnosis.json)

## 回退与当前正式

Web `dpl_FoNUgjSRpw56hwUFDNrSs7LZZQFN` 与 Gateway 均为
`e0ceea22964c03041d45e49e506b3a8badd6984e`；独立 Vercel alias、公开 `/version`
和网关 `/playtest/health` 均读回该源码。Gateway PID 1754094，原可执行文件 SHA 匹配。
**没有恢复数据库**，仅移除本次自有的两份 override，原 320／232／232 行逐字节保持，
原 24 份配置不变。自有 canary、SSH、浏览器和其他本地句柄均已关闭。

恢复后简中／繁中各一次真实出码、矩阵验证、pending、取消及取消后零轮询通过。
这只证明该范围，**不证明签名后的桌面交接已修复**，也没有完成实体手机相机验收。
[回退后 QR](postrollback-qr.json) · [完整结果与最终读回](outcome.json)

29 份原内容 WIP 的原始 SHA 和 15 处私有构建覆盖的恢复状态保持；认证凭证、原账号行、
意图、签名和钱包私密信息未发布。[证据与私有回执 SHA](evidence-index.json)

本轮保留 [215 项／Chrome／本地整局](../README.md) 的已验证范围；它们不替代原公网失败。
新一轮先受控定位授权状态交接，保留当前失败，随后以新不可变源码重新资格验证。
首次冷 Q／黑帧、语言、更多地图与技能、三职业完整 1–30、阿洛、Native 同档及人类
Accepted 继续，整体 100% Candidate 和 Accepted 均未完成，Goal active。
