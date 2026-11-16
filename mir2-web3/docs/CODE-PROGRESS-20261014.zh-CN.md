# 三个聊天的代码进度与提交整理（2026-10-14，北京时间）

这份记录核对用户指定聊天的历史结果、对应工作树、实际源码和远端分支。
代码提交、源码回归、候选包、线上发布及玩家验收分别记录。全项目仍未达到
100% Candidate；本轮不根据局部测试重新估算整体百分比。

## 聊天与代码位置

| 聊天 | 主要工作 | 当前代码状态 | 仍待完成 |
| --- | --- | --- | --- |
| [规划 Windows 版本下一步](codex://threads/01a07e72-8bad-77d2-95df-64ee5cda82fa) | NPC 刷怪动作、道士宝宝任务计数，保留活动与账号 schema9 | `codex/gateway-disconnect-lifecycle-20261008` 的 `a045b4327f` 已提交；本轮补推并核对远端一致 | 最终合并源码的 Linux/数据库资格、Windows 完整玩法、地图事件与 P1–P7/Mentor 验收 |
| [玩法](codex://threads/01a11971-9221-73b1-8152-eabcc3e6379f) | 五职业 1–50 级、技能与素材、动作输入、更新器和共享网关接线 | `codex/five-class-completion-20261010` 的 `9877fef767` 已在远端；配套 `ea0d3e3661` 本轮补推并核对一致 | 完整 R26 签名包、旧更新器过渡包、资源/网关成对发布及新职业实机验收；高于50级不在本批闭合范围 |
| [充值](codex://threads/01a11a48-f3c9-7060-ae91-558064abe279) | 独立充值网站、活动中心、玩家中心和账号权益登记 | 独立充值 `e1ec0ae8fb` 已提交上线；活动分支已有 `bad3737f26`，本轮追加并推送部署修复 `e09ad2940f` | 活动网页重新发布及普通浏览器验收；月卡实际发放、称号/时装、拉新和正式返还结算仍开放 |

用户在三个聊天中的“代码整理推送一下”最近几次执行均被中断；源码实际已保存在
不同分支。此次沿原分支补齐推送，没有将不同产品/发布分支强行合并成一个版本。

## 可核验的提交与证据

| 分支 | 核对后的远端提交 | 证据范围 |
| --- | --- | --- |
| `codex/five-class-completion-20261010` | [`9877fef767`](https://github.com/Zombieliu/mir2/commit/9877fef767d39fbc422ed552c52ec71c14f60584) | 地图 profile 修复 Node15 与 PowerShell scope；完整 Native987通过、7个原有外部测试忽略 |
| `codex/five-class-live-gateway-20261012` | [`ea0d3e3661`](https://github.com/Zombieliu/mir2/commit/ea0d3e3661dad62a5fad694816af000c54790ad0) | 424项独立检查，0失败/忽略；169个受审源文件长度与SHA256本轮只读复核一致 |
| `codex/gateway-disconnect-lifecycle-20261008` | [`a045b4327f`](https://github.com/Zombieliu/mir2/commit/a045b4327fd98775b80afadd20df77d03e84ae3c) | 790项独立本地检查，cadence7项另行；2个原有PostgreSQL测试本地忽略；47个受审源文件本轮核对一致 |
| `codex/shop-mail-publication-20261010` | [`e1ec0ae8fb`](https://github.com/Zombieliu/mir2/commit/e1ec0ae8fbd8935ef70b43973c03bb9ba5970f7b) | USD1 Stripe TEST→100点券、网站重登、两次普通游戏保存/重登、网站退出后游戏保持连接 |
| `codex/website-activities-20261012` | [`e09ad2940f`](https://github.com/Zombieliu/mir2/commit/e09ad2940f58c9935aa65116cd1168adc14ebf55) | 本轮Python20与Node29全部通过，0失败/跳过；部署修复接入原活动CI |
| `codex/slg-production-core-20261008` | [`3898116075`](https://github.com/Zombieliu/mir2/commit/38981160754b266844cc69ef052ccf2da5ed319a) | 包含已推送的制作加工、原石工坊和R23基础显示记录；原石默认关闭，22项游戏内玩法未由该显示记录验收 |
| `codex/windows-player-journey` | 整理前为 [`2390768dbf`](https://github.com/Zombieliu/mir2/commit/2390768dbfc553e71dae4e9b8bb97b5252a9e2af) | Web Source78空技能页、共享Bag/Character及保存重登；144项回归、34项打包检查为此前的有限记录 |

上表中历史回归没有在本轮重复执行。重新执行的是部署Python20项和网站Node29项。
本轮只读核对了真实远端；网关推送分别补齐原远端缺少的3个、4个历史提交。

五职业细节与边界：

- [1–50级内容、Native回归及完整交付边界](https://github.com/Zombieliu/mir2/blob/9877fef767d39fbc422ed552c52ec71c14f60584/mir2-web3/docs/FIVE-CLASS-CANDIDATE-20261011.md)。
- [地图profile的实际拒绝与修复](https://github.com/Zombieliu/mir2/blob/9877fef767d39fbc422ed552c52ec71c14f60584/mir2-web3/docs/FIVE-CLASS-R26-MAP-PROFILE-20261012.md)。
- [配套网关transport合并与424项范围](https://github.com/Zombieliu/mir2/blob/ea0d3e3661dad62a5fad694816af000c54790ad0/mir2-web3/docs/FIVE-CLASS-LIVE-TRANSPORT-MERGE-20261012.md)。

NPC与充值细节：

- [NPC/宝宝任务及活动schema9合并](https://github.com/Zombieliu/mir2/blob/a045b4327fd98775b80afadd20df77d03e84ae3c/mir2-web3/docs/CLASSIC-NPC-SPAWN-ACTLIST-20261012.md)。
- [独立充值网站上线与测试付款验收](https://github.com/Zombieliu/mir2/blob/e1ec0ae8fbd8935ef70b43973c03bb9ba5970f7b/mir2-web3/docs/generated/player-qa/recharge-web-20261012/README.md)。
- [活动中心的权益登记与发放边界](https://github.com/Zombieliu/mir2/blob/e09ad2940f58c9935aa65116cd1168adc14ebf55/mir2-web3/docs/ACTIVITIES-PLAYER-PORTAL-20261012.md)。
- [本轮部署修复与49项离线回归](https://github.com/Zombieliu/mir2/blob/e09ad2940f58c9935aa65116cd1168adc14ebf55/mir2-web3/docs/RECHARGE-DEPLOYMENT-REGRESSION-20261014.md)。

## 交付关口与本轮处理

1. **R26安装包仍未通过最后一次打包。** 实际
   `work/fiveclass-20261010/candidate-package-r26-package02/result.json`
   为 `passed=false/accepted=false/published=false`：native-map-keyed manifest
   的 ADS 枚举提示路径不存在。较早地图文档中的“新build pending”之后已有
   Windows CI38201546287构建记录；完整包的这次拒绝仍保留，不能标记R26已发布。
   既有R2上传凭据权限问题也未由本轮提交解决。
2. **活动页面的已知部署源码问题已修复。** 原来校验后再次读取staging会产生
   文件替换窗口，现改为发布同一组已校验缓冲区。incoming CRLF与existing LF
   的片段比较会拒绝正常更新，现兼容明确的行尾变体，并保持跨变体唯一性、
   完整Caddy原字节、CAS、精确认证路由及回退检查。新旧CRLF站点和混合行尾均有
   离线回归；本轮未执行线上切换或付款。
3. **新活动CI正在执行。** [run38516667990](https://github.com/Zombieliu/mir2/actions/runs/38516667990)
   对应 `e09ad2940f`；核对时为 `in_progress`。本轮本地49项通过，完整Linux/真实PG
   CI的最终结果应以该run为准，不把旧提交的成功结果移作新提交验收。
4. **主工作区保留其他工作。** 制作加工/原石相关源码大量工作树副本，与已经
   推送的SLG分支逐项相同；Web配置、打包脚本和旧QA证据也有原有差异。本轮
   不重复提交这些副本，只在主分支增加进度汇总和入口，保留原始未提交内容。
5. **Git自动维护有历史对象损坏。** 本轮活动提交时自动GC读取对象
   `1ae7f886fa87573bdbb59caf0772103929fac94c` 失败；代码提交和普通推送均成功，
   远端精确SHA再次核对一致。没有删除对象或重写历史；后续命令关闭本次调用的
   自动GC，仓库修复留作独立维护任务。

下一步仍按各发布分支的原流程推进：先取得最终源码的完整资格结果，修复并重新
生成受守卫的R26包，核验更新器/资源/服务器组合，再做活动页发布与普通玩家
流程验收。当前Web主队列仍保留数量确认框、非空技能/施法、装备/Bag2、完整
战斗任务和移动真机检查。最终人类前端验收及全项目100%状态保持开放。
