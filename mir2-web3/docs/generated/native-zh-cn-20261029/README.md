# Windows 简体中文验收记录（2026-10-29）

**本轮 UI 技术目标已完成**：修复后的 Windows 客户端通过简体启动、十语菜单、English→简体切换、三个充值页面和重启保留简体偏好；已有经济状态保持一致。`complete=true` 仅指这些目标，完整发布门限、全部文案人工审校和真人前端 Accepted 仍未完成。

首次实机启动暴露 `native_display.rs:233` 旧九列标签表访问简体 index 9 的越界（len 9 / index 9），尚未点击游戏控件，客户端停止。该失败保留在[脱敏记录](windows-live/evidence/first-simplified-startup-failure.json)。随后修复十语表约束，显示专项 7、任务计数专项 1、最终游戏 GPU 3 图及新构建实机启动/重启均通过；此前离线 Shell fixture 未覆盖这条启动路径。

最终修复构建为 **149412864 字节**，SHA-256 `abd92be30f421328c3309eac66bbafcdd542df4117ed24ced872098e9b9b64ff`。[构建记录](build/windows-build.json)核对真实产物和 18 个编译源 pin；[旧 5dc223… 构建](build/windows-build-before-startup-fix.json)作为启动失败历史保留，不作为最终产物。二进制不收入证据。

`zh-CN` 是第十种客户端语言，显示“简体中文”，原九语索引不变。**9769 个系统文案键**都有简体：2362 条复用现有简体，7407 条由固定官方 OpenCC 1.4.2 `tw2sp` 离线生成；占位符、数字和显示命令逐条校验。游戏运行时没有 OpenCC 依赖。玩家姓名、聊天、公告、玩家邮件内容及模板参数保持原文，未知文本不作广泛转换。9769 是覆盖数，不是人工审校数量。

见[目录完整性](offline/catalog-validation.json)与[生成来源](../../../packages/game-data/data/native-i18n/extra/zh-CN-provenance.json)。`Administrator` 和 `Recommended` 仅在有类型的 NPC 角色标题与任务分组显示中映射为“管理员”“推荐”，原始身份、分组选择键和玩家同名文本不变。账号、协议、装备身份和经济数值未改变。

| 检查 | 结果与执行阶段 |
| --- | --- |
| 文案目录 / 两处系统标题 | 15 / 1 通过，最终 9769 键 |
| 玩家文本 / 职业练习 / 充值逻辑 | 11 / 7 / 19 通过；在两处独立标题补充前执行，相关行为未变 |
| Windows 语言偏好 / 字体 | 4 / 16 通过；字体分为 7 个注册/pin、9 个有界缓存/塑形检查 |
| 启动显示标签 / 任务计数 | 7 / 1 通过；显示专项另有 1 个忽略 GPU，不计为执行通过 |
| 独立 GPU | Shell 44 次捕获（选存 6 图）、充值 39 图、最终游戏 3 图、字体 10 图 / 180 次切换 |
| 修复构建真实 Windows 操作 | 10 张窗口图；简体→English→简体，重启种子 en 仍恢复已存 zh-CN |
| 经济状态 | 导航前、三页后、重启后均为 100 点券 / 10000 金币 / 3 历史 USDC 单，所比较字段一致 |

[测试摘录](checks/native-tests.json)固定原始日志哈希、用例名和通过/忽略数。[GPU 阶段](checks/gpu-results.json)明确：Shell/充值早于两处系统标题、通用 Han 回退兼容和显示表修复；最终游戏三图、生产字体和真实窗口另行补验。不能说所有旧截图都在最终源码重跑。字体三个默认忽略 GPU 中，本轮另跑十语字体 GPU；旧 11 分钟 GPU soak 和负面对照未重跑。

Shell 全部 44 次布局测量与原报告哈希保留；为减少重复背景，只归档简体四屏及英/阿语言菜单六张 PNG，未归档者标为 `imageIncluded=false`。充值、游戏、字体所述图片完整归档。

以下为**真实 Windows 客户端窗口**，连接隔离的文件权威 Gateway，未新发付款或购买/使用月卡：

| 操作 | 证据 |
| --- | --- |
| 修复后简体欢迎页 | [01](windows-live/screenshots/01-zh-cn-welcome.jpg) |
| 设置与十语菜单 | [02](windows-live/screenshots/02-zh-cn-options.jpg)、[03](windows-live/screenshots/03-ten-language-menu.jpg) |
| English 与恢复简体 | [04](windows-live/screenshots/04-english-switch.jpg)、[05](windows-live/screenshots/05-zh-cn-restored.jpg) |
| 商城已有点券/金币 | [06](windows-live/screenshots/06-zh-cn-shop.jpg) |
| 法币充值 / USDC 链上充值 / 独立月卡 | [07](windows-live/screenshots/07-zh-cn-fiat.jpg)、[08](windows-live/screenshots/08-zh-cn-usdc.jpg)、[09](windows-live/screenshots/09-zh-cn-monthly.jpg) |
| 相反 en 种子重启后仍为简体 | [10](windows-live/screenshots/10-zh-cn-after-restart.jpg) |

[实机操作报告](windows-live/navigation-result.json)与每图 sidecar 记录产物哈希、时间和目的；[公开最终回执](windows-live/evidence/final-ui-result.json)、[语言偏好摘要](windows-live/evidence/saved-locale-before-restart.json)及[经济比较](windows-live/economy-preservation.json)独立区分保存版本号变化与经济字段不变。第十图之后一次关闭欢迎框的工具点击返回 `unknown screenshotId`，未重试，不计为关闭成功，也不改变已经捕获的重启观察。没有复制 APPDATA 或完整账号库。

以下为**离线生产渲染器测试图**，不表示真实付款或游戏经济数据：

| 页面 | 证据 |
| --- | --- |
| NPC 简体角色/正文，原样玩家参数 `龍城{Gold}` | [截图](native-mock/gameplay/zh-CN-npc-opaque-name-1024x768.png) |
| “推荐”“首套巡逻装备”与“列表：1/20” | [截图](native-mock/gameplay/zh-CN-quest-diary-detail-1024x768.png) |
| 背包真实图标与铁剑提示 | [截图](native-mock/gameplay/zh-CN-bag-item-tooltip-1024x768.png) |
| 登录、注册、创建、选角和十语菜单 | [报告](native-mock/shell/shell-i18n-report.json)、[菜单](native-mock/shell/shell-en-language-popup.png) |
| 三语言充值状态 | [充值 UI 记录](../billing-entry-ui-20261029/README.md) |

简体主字体为未修改的完整 **Noto Sans CJK SC Regular**，上游提交 `165c01b46ea533872e002e0785ff17e44f6d97d8`，16437364 字节，SHA-256 `2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b`，含 44810 个码点 / 65535 字形，遵循 OFL。原 TC 字体不变；通用 Han 保留 TC、两字补充、SC 回退，`zh-CN` 显式用完整 SC 主字体。Fontique locale 兼容细节见[来源](../../../apps/game-client/platform-windows/assets/fonts/SIMPLIFIED-SOURCE.json)。

[语料报告](fonts/corpus-verification.json)核对 1996 个可显示字符 / 1875 个 Han，零缺字；含控制字符集合为 1998。字体和 OFL 留在[仓库资产](../../../apps/game-client/platform-windows/assets/fonts/)，此包仅固定来源和哈希，不再复制 16.4 MB 字体。

[安装/打包专项](packaging/static-focused-validation.json)通过十语配方 10 个正/负断言、每语 15 个自定义消息、11 个完整字体 pin 和两个脚本各 5 个精确许可文件 / 30 个相近文件拒绝检查。[原摘要](packaging/summary.json)、[配方结果](packaging/recipe-static-result.json)、[打包结果](packaging/focused-packaging-result.json)同时保留 14 个改动源码 pin。固定官方字体的 CFF/cmap 数值区只在完整 SHA 与 SFNT 表边界匹配时遮罩；没有整字体豁免或放宽任意路径规则。

**完整发布门限仍开放**：package 与 verify 的完整 selftest 均被未改动的既有 skillbar `Title/meta.json` 身份 pin 不一致提前阻断；可信 Inno 编译器未安装，完整编译器翻译闭包、安装器实际编译、安装与签名没有验证。静态专项通过不等于完整打包或发布通过。全部 9769 条人工语言审校、真人前端 Accepted、生产部署也未完成，详见 [index 限制](evidence-index.json)。

本轮没有新付款、月卡消费、Gas、主网或 Docker 操作。[旧 USDC 交易包](../sui-usdc-recharge-v1-20261029/README.md)保持不变。运行 `node generate-manifest.mjs --verify --require-complete` 校验本轮 UI 完整性；manifest 按原始证据字节及规范化源码固定 43 个源文件。提交前还须核对暂存区 Git blob。
