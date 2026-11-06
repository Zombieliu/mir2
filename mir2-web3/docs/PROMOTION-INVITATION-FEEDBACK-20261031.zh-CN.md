# 推广邀请码反馈｜2026-10-31

用户反馈填写推荐码后没有反应。本轮只修复邀请码表单的状态与错误反馈，不改变活动、邀请归属、充值或计佣规则。

- 查询团长仅建立候选；页面明确提示核对团长、接受条款、再次确认后才建立邀请归属。
- 邀请码相关错误在表单旁持续显示，后台自动刷新不再清除该错误。仅显示经过映射的公开错误类别，不渲染原始服务端错误。
- 切换账号、钱包凭据、活动或邀请码时清理旧候选和反馈，避免把上一身份的成功提示显示给当前身份。
- 十种现有语言同步补齐步骤和拒绝原因。保留原条款及钱包验证门禁。

原 Windows 测试服的有效游玩日由真实击杀或完成任务事件产生，并随角色正常保存后采集。等级本身不构成有效游玩日；UTC 零点对应北京时间早上 8 点。本轮没有生成游戏事件、修改等级或补记历史活跃日。

本地验证：18 项前端实源与交易链路测试、36 项刷新/金额/赛季/代理相邻测试、22 项发布护栏测试均通过，TypeScript 检查通过。包含后台读取不清写入错误、两种 401 可见路径、查询不冒充确认，以及账号/凭据/活动/邀请码变化后旧状态清理；这些使用隔离测试数据，没有产生真实绑定或付款。

发布沿用原 `mir2-web3-web` Vercel 项目，先创建不分配正式域名的候选，再验证并提升。只读配置检查 [41111807209](https://github.com/Zombieliu/mir2/actions/runs/41111807209) 已通过，发布护栏的基线已更新为当前 Windows 网页部署 `dpl_2ekBnauSAihEAWLFguLcHKrs7kx3`，宣传站基线仍为 `dpl_7krVTQSx5Wj9Ht8Sd5KF5o9d3F97`。宣传主站、Cloudflare、Windows 客户端及游戏数据库不在本次网页发布的改动范围内。

2026-10-31 03:18 UTC+8 已完成[候选创建](https://github.com/Zombieliu/mir2/actions/runs/41113525300)及[提升验证](https://github.com/Zombieliu/mir2/actions/runs/41113991681)：源提交 `3d8f81752fc21400622808a5d00a871b47047ed5`，部署 `dpl_9yLef9wXkWDovACg9atHU1W1uBSx`。受保护候选与正式域名均通过当前/历史赛季分流、财务开关、游戏、主站导航及 13 项静态资源检查；两个游戏域名均已指向候选，宣传站部署不变，见[候选收据](evidence/promotion-invitation-feedback-20261031/STAGED-CANDIDATE.json)与[域名收据](evidence/promotion-invitation-feedback-20261031/ALIASES.json)。

随后使用普通公网 GET 独立复验 `/version` 精确匹配源提交、匿名 Windows 活动仍计佣开启/付款关闭，并读取 10 个 JS 资源，确认简中步骤、错误提示及“查询尚未确认”文案已在公网实际返回，见[公网验证](evidence/promotion-invitation-feedback-20261031/PUBLIC-VERIFICATION.json)。未使用用户 cookie、提交邀请或操作钱包；这不替代用户手机签名及最终目视 Accepted。
