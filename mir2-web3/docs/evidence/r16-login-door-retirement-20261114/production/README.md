# 公网失败、回退与扫码复验

`ea4b1c363e` 候选已经历创建、检查和一次发布，完整公网验收 **未通过**，随后已恢复 E6。
本地四场及资源所有权的资格仍有效；本轮没有保留新版为正式版本，也没有关闭冷加载或整体 1:1。

| 步骤 | 回执 | 结果 |
| --- | --- | --- |
| 创建独立候选 | [43601364918](https://github.com/Zombieliu/mir2/actions/runs/43601364918) | 成功，正式未动 |
| 保护入口检查 | [43601996300](https://github.com/Zombieliu/mir2/actions/runs/43601996300) | 15 项完整字节 / SHA、真实出码待确认通过，无写入 |
| 发布候选 | [43602838462](https://github.com/Zombieliu/mir2/actions/runs/43602838462) | 一次发布，前后各 15 项字节和扫码分项通过 |
| 完整普通浏览器回放 | [原门槛分类](public-acceptance.json) | **false**：运行包未完成、六字体未就绪、共享 HUD/Q 未通过 |
| 恢复稳定版 | [43604251204](https://github.com/Zombieliu/mir2/actions/runs/43604251204) | 成功，独立 API 和公网 `/version` 均为 E6 |

当前正式是 `e6c923b7c669cb061a7a03703f05fadd91211c97` / `dpl_Ae3hNyFygQuSMZDesBXCZDpUECTY`，
游戏项目、固定资源和域名保持原配置。网关仍为 `fd3fcedc8b8a4d0b6bb16f6c45cd57a77364c8b5`；
本轮未更新或重启服务器。[发布操作](release-workflows.json) · [独立 API](final-production-mapping.json) · [公网读回](final-public-service-status.json)。

## 实际通过与失败

普通 Chrome 的 `/game` 出码后，在官方 Testnet 钱包的手机尺寸浏览器中完成一次登录批准。
新建专用测试角色，普通键盘行走的权威确认 3746ms，通过原 8 秒；113/113 预取完成。
角色普通注销返回选角，两个自有浏览器都关闭。未进行充值、支付，也未使用实体手机摄像头。

运行包应用源在 8 秒没有响应头后切换 CDN，CDN 再等 120 秒仍没有响应头，取消后没有新的
应用源尝试。运行包正文没有完成，Native 未启动，因此主动预热六套字体也未开始；最终缓存检查
为 **0/6**。另有 CSS 繁体字体在多个源读取失败。不能把字体未就绪全部归因于字体排队策略。
补充 180 秒观察也没有改变原 120 秒失败。[分项](public-failure.json) · [原字体缓存](public-original-fonts.json)。

完整静态白名单观测记录 884 行，其中 Worker 302 行、末尾未完成 7 项、无截断。
开门大图从旧 E6 对照的 18 次后认证请求变为本轮两张顺序请求；这是队列行为的改变，
并未使公网冷启动通过。Page 和 Worker 是不同层，不能相加宣称实际网络传输量。
[原始静态观测](public-static-waterfall.json)。

验收分类器在发布前已冻结，并用旧 E6 的“功能通过、冷启失败”反例核对，保留 8 / 120 秒
及六套完整字体条件。[分类器检查](acceptance-classifier-contract.json)。

## 回退后复验

简体、繁体各一个全新普通 Chrome 上下文，从无参数 `/game` 真实点击一次出码，核对二维码
矩阵、官方钱包地址和同一 intent 的 pending；普通取消返回登录页，取消后零轮询、无登录批准，
两个浏览器均关闭。该检查不代替实体手机和完整进城验收。
[简体](qr-zh-cn-receipt.json) · [繁体](qr-zh-tw-receipt.json) · [汇总与私有原件 SHA](checks.json)。

下一步给“两个源都没有响应头”补明确的有界源切换与取消测试，保留最终截止、正文闲置、
字节上限和单次初始化边界；再重新构建和完整回放。不得原样重新发布 EA4。
