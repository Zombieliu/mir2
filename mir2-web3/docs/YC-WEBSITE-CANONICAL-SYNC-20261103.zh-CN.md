# 官网内容同步回维护分支（2026-11-03）

后续事实更新：用户已明确确认“已经邀请外部玩家参加测试”。下文保留同步时点的记录；当前测试状态以[最新事实更新](../apps/website/YC-EXTERNAL-PLAYTEST-UPDATE.md)为准，原先关于内测／外测的待确认项已解决。

根据用户“开始弄”的指令，将已发布、已合入 main 的 Numeron 官网内容同步回维护分支 `codex/windows-player-journey`。

- 同步前维护分支：`aecf01a81d1a3552bb15283e06859e5a9f59d169`。
- 官网内容来源：`369e99d7a97f4c9c15994c412bb675e9e5a462dd`；对应 main 官网子树来自 `29da1af9c06efaa96937bc688d6a4642a5ab6e78`。
- 维护分支源码提交：`0bbc4caeafff3a4fa1c4aaed3f92d4a26ed303de`。
- 精确同步 20 个官网文件。提交后的官网 Git 子树为 `65d977085c3bdf306f79b5aa14755de102218a81`，与已发布版本及 main 完全一致。

同步前，main 与维护分支整支合并会出现 17 个官网 add/add 冲突。同步后使用 `git merge-tree --write-tree --name-only --no-messages` 做不改变分支和工作树的合并演算，退出码为 0，冲突数为 0。这不是把整个游戏分支合入 main，也不表示 main 已包含当前游戏开发进度。

本次提交仅包含上述 20 个文件，没有夹带原有 staged/unstaged 的下载器、资源库、Windows 客户端、官网 README 或环境配置改动；提交前后单独比对了其他文件的 Git index 条目。没有使用 stash、重置工作树或切换维护分支。

## 本次验证

- 官网生产构建 `npm run build -- --webpack`：通过。
- 官网 `npm run typecheck`：通过。
- `npm run i18n:check`：16 种语言通过。
- `node scripts/check-minimal-content.mjs --build`：16 语言文案及 641 个生成 HTML 页面检查通过，包括 canonical、hreflang、OG/Twitter、robots/sitemap，以及开发起点与当前验证日期的区分。
- 文档检查：19 篇文章、62 个章节、44 个内容链接通过；没有把它记为浏览器 HTTP 路由或锚点验收。
- 文档示例测试：7/7 通过。
- 统一代码线校验：本次同步前通过；提交后再次校验的结果见[结构化记录](evidence/yc-website-canonical-sync-20261103/verification.json)。

本次没有修改游戏运行逻辑，没有重新制作安装包或发布生产环境。官网子树与既有发布版相同，未重新采集截图或重新宣称真人验收通过。

## R16 与状态表述

R16 验收记录使用冻结客户端源码 `e9117b6691e3f7ec238ff1c1eb77b699b0646c2f`，该历史已在维护分支中。交易报价显示、成交后背包显示等问题在 R16 对话中有记录；本次同步没有修复这些游戏问题，也不把局部交易测试当成整个游戏 Accepted。

官网继续沿用用户此前确认的事实：2026 年 5 月开始开发；未正式上线、未开始外部测试；当前内部游戏与充值流程已跑通，但完成日期未知。已另行询问 R16 是团队通过公网做内部 QA，还是已邀请外部玩家；未收到新事实前不改写测试阶段，不将测试账号、游戏金币或测试充值当成玩家规模、商业收入或留存数据。

