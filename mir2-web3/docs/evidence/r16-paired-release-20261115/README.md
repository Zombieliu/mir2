# 同源 Linux／Web 候选、旧档验证与公网回退

2026-11-15，继续 Windows R16 的 `codex/windows-player-journey` Goal。候选源码为 `aa524edc76d85e01f2ae8963957bbacd9728eac2`，发布控制器为 `39832d328e40406257942f1b9385a823effd8c07`。本轮确实切换过网关和 Web；**首场完整公网验收失败后，两端均已回退**。当前为 Web `20de5ea4fd`／`dpl_AWssgszkpKJznMksLsP7hiRFK6U8`、Gateway `fd3fcedc8b`，不是候选已留在正式。

源码仍包含已推送的[目标／期限／入口反馈](../r16-route-feedback-20261115/README.md)、[日记独立关闭](../r16-diary-close-20261115/README.md)及[共享中立家畜漫游](../r16-livestock-roam-20261115/README.md)。这些候选的本地结果保留，但本轮不将其他任务目标或公网寻路一并改判通过。

## Linux 与候选资格

- `aa524edc76` 为 Linux 打包加入共享 Zone 的集成、漫游／检查点及 Gateway 回归门槛；`39832d328e` 将隔离发布控制器锁到这份源码。两项提交均已推送，原资源版本和字节固定值未更换。
- [Linux 构建](https://github.com/Zombieliu/mir2/actions/runs/43677385790)完整成功。303 项共享 Zone 检查、216 项原生 PostgreSQL／计费回归通过；不是全工作区或 CCU 资格。回执为 [SHARED-ZONE-ACCEPTANCE.json](SHARED-ZONE-ACCEPTANCE.json) 与 [CI-ACCEPTANCE.json](CI-ACCEPTANCE.json)。
- 完整下载工件 `14147367864`，核验 1,841 个文件的 SHA、来源及 x86_64 ELF。网关为 93,454,400 字节，SHA-256 `ce04a5ce3525700616aa7f8d18aa15d9eeea79d254f5c62d473d42bfc1513808`。GitHub 声明的 ZIP 摘要未作为本地已核验的 ZIP 摘要。
- 发布保护 80 项通过。源码闭包为 15 个提交、105 个文件，独立 GitHub 比较与本地 Git 对齐。候选 `dpl_CKPpQfDV7GJsWDvdZ5tWtyUvoHzj` 的 [Stage](https://github.com/Zombieliu/mir2/actions/runs/43677749313)、[独立 Check](https://github.com/Zombieliu/mir2/actions/runs/43678298342)及 [Promote](https://github.com/Zombieliu/mir2/actions/runs/43682171174)均通过各自的 15 组完整资源和扫码待确认检查。这些检查不代替完整浏览器验收。

## 旧档、隔离实例与网关切换

先在同一 PostgreSQL 导出快照下保存完整备份和原数据指纹，私密保留 22 份服务／环境配置。恢复独立数据库后，307 个旧账号、219 个旧角色和219份存档逐字节一致。隔离环境使用自己的 Redis、监听端口、恢复目录，外部付费操作关闭。

隔离启动首轮将连接串写成了程序拒绝的关键词格式；修成同一克隆的标准 PostgreSQL URL 后启动成功。原正式进程未受这次隔离错误影响。随后使用普通注册、密码登录、创角、StartGame 和 Walk：`(288,616)` → `(288,617)` 的权威移动为99.704毫秒，观测到1条无主中立怪物 `ObjectWalk`，两次正常注销及同角色重登保存通过。没有使用传送、管理或 QA 游戏命令；克隆中全部旧行的指纹仍一致。

切换正式网关只增加自己的二进制覆盖和版本环境文件，实际环境仅 `MIR2_DEPLOY_REVISION` 改变，原22份配置、旧数据指纹及66连接／51会话上限保持。切换前无在线玩家、连接或恢复租约。随后才切换 Web，并由独立域名映射及公开版本接口确认两端为同一候选源码。

## 原门槛下的公网 R1

使用全新桌面 Chrome、正常系统网络和官方手机尺寸 Testnet 钱包浏览器，实际出码、唯一登录签名、兑换、创角并进入比奇。无充值或资产交易；这是内部 QA 身份，不是实体手机或独立客户。

| 项目 | 实际结果 |
| --- | --- |
| 普通相邻一步 | `(288,616)` → `(289,616)`；发包至 ACK 571毫秒，通过原8秒门槛 |
| 完整行走包 | 首读部分正文后停滞，重试无响应头；原65秒观察窗口未见完整包／合格自然PNG，失败 |
| 编译备用包 | 稍后读到完整764,724字节原载荷、240个PNG，SHA正确；到达太晚，不倒填原观察窗口 |
| 运行包正文 | 首帧后138.163秒完整，超过原120秒门槛 |
| Native HUD／Q 与113项 | 未通过，不能由备用任务窗或后来缓存读回替代 |
| 六套原字体 | 完整字节与 SHA 读回通过 |
| Worker 重启及正常注销 | 通过；两个测试浏览器均已关闭 |

[未改动的原判定](public-r1-original-classification.json)为 `accepted=false`。首个失败是 `PUBLIC_VISUAL_PACK_BODY_NOT_OBSERVED:entry-monster-walking`。R2只完成准备，未执行，未用另一轮覆盖首轮失败，也未调整8／65／120秒或完整资源标准。

[场景窗口结束画面](public-r1-terrain-window-end.png)和[备用任务窗](public-r1-quest-available.png)已查看：比奇、玩家和普通任务文案可见，Native完整界面未据此验收；混合语言等旧差距仍在。

网络记录进一步确定了两个等待边界：行走JSON首读在530,467字节后被空闲期限中止，下一次20秒未收到响应头；随后备用脚本耗时约104.6秒才完整。运行包第三次读取的响应头等待约81.8秒，之后才读取完整正文。源码中主中文字体仍由有效运行包响应头启动；这是一项待受控验证的串行依赖，不能写成已修复的根因。

## 回退、数据与下一项

[Web 回退](https://github.com/Zombieliu/mir2/actions/runs/43683010083)通过；独立 Vercel 别名和公开版本确认恢复 `20de`／`AWss`。网关仅删除本轮自己的两个覆盖文件，恢复原二进制及 `fd3`，**未恢复数据库覆盖新进度**。回退后原307个账号、219个角色和存档全部未变；本场新增的1个QA账号、1个角色及存档仍保留。22份原配置及66／51上限保持，隔离Gateway／Redis均已停止，SSH控制器已关闭。

回退后简中、繁中各一次真实二维码矩阵、同一请求待确认、正常取消及停止轮询通过，均只创建一次请求，浏览器已关闭。最终公开读回时间为 `2026-11-15T06:02:51Z`。

下一项从上述行走正文／备用包超时恢复及运行包响应头—字体启动边界做受控复现和修复，再完成匹配构建与两场完整公网验收；停止原样重发。其他目标、狩猎区、多图与断网、完整三职业自然1–30、阿洛跨端／模型、资源浮层覆盖血球、语言／像素、Windows窗口、实体手机和整体100% Candidate／Accepted仍开放，Goal保持active。

原30份内容WIP中29份原始SHA保持；另一份容量证据文档由并行任务继续修改，本轮未写入或回退它。原动画WIP继续排除在本次发行之外。详细安全摘录见 [qualification.json](qualification.json)，原始私密回执仅以[23份字节／SHA记录](private-receipt-pins.json)绑定，不发布账号凭证、钱包状态或登录请求。
