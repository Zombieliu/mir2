# 旧邀请迁入 Windows：本人确认页面

用户授权保留旧活动的原团长与团员关系。本页面承接后端显式准备的迁移凭据：总览展示原团长、来源活动、旧确认时间、真实准备时间、当前条款与审计编号；团员须独立勾选当前条款，再提交普通 `confirm` 请求。报名勾选或普通邀请码勾选不能代替迁移同意。

请求成功仅显示等待刷新。只有 `me.attribution.migration` 返回真实最终归属，才展示迁入完成、旧与本次确认时间、证据摘要。错误经过后台定时刷新继续可见，账号、凭据、活动、条款或凭据替换会隔离旧同意与迟到响应；迟到 401 不能注销新上下文。缺失、过期、未绑定钱包或条款不符均禁用确认。现有 10 种语言与不含新字段的旧后端继续兼容。

后端实现为分支 `codex/windows-promotion-bridge-20261030` 的 [`173332a004c01312802a67042ff7800a04893134`](https://github.com/Zombieliu/mir2/commit/173332a004c01312802a67042ff7800a04893134)。该功能不修改真实充值、等级、游玩日、月卡、佣金或付款事实，最终计佣仍须通过当前活动权威来源校验。不得以页面显示或模拟截图替代后台确认。

## 验证

- [本地验证及源码 hash](evidence/promotion-migration-ui-20261031/LOCAL-VERIFICATION.json)：真实 TSX 控制器测试 29/29、发布守卫 23/23、TypeScript 与 diff 检查通过。新增守卫拒绝缺少迁移组件的候选工件，即使 `/version` 匹配。
- [真实组件浏览器 6/6](evidence/promotion-migration-ui-20261031/MIGRATION-UI-RECEIPT.json)：现有 esbuild 直接导入生产 React 组件及真实 CSS Modules，独立 Chrome、全模拟 API；验证勾选门禁、精确确认请求、提交与完成分离、后台刷新后错误保留、过期零写入及 390px 布局。见[构建输入](evidence/promotion-migration-ui-20261031/COMPONENT-HARNESS-BUILD.json)与[视觉复核](evidence/promotion-migration-ui-20261031/VISUAL-REVIEW.json)。
- [本地 Next 边界](evidence/promotion-migration-ui-20261031/LOCAL-NEXT-BOUNDARY.json)：两次隔离开发服务均返回 500，分别为跨盘 junction 模块路径与 CSS 插件编译错误；保留失败证据。组件浏览器验证没有冒称 Next 服务器、真实条款确认或真实数据库端到端验收。

模拟截图带有醒目标记：[桌面待确认](evidence/promotion-migration-ui-20261031/migration-pending-desktop.png)、[桌面完成](evidence/promotion-migration-ui-20261031/migration-completed-desktop.png)、[手机待确认](evidence/promotion-migration-ui-20261031/migration-pending-mobile.png)。模拟数据均非真实邀请、付款或迁移。

发布仍采用原 Vercel 项目，先构建无域名分配的 production 候选、检查精确源码及受保护路由，再提升同一工件。游戏官网基线为 `dpl_9yLef9wXkWDovACg9atHU1W1uBSx`，宣传站基线保持 `dpl_7krVTQSx5Wj9Ht8Sd5KF5o9d3F97`。记录时新候选尚未发布，真实迁移未准备；不打开付款开关，不启动 Docker，不发布 Windows 客户端。
