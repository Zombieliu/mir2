# Numeron 官网文档第一版：内部证据与交付记录

日期：2026-11-02（Asia/Shanghai）。本文件仅供内部核对，不导入官网、
不复制到 public，不作为公开产品条款。

内部隔离检查标记：`NUMERON_INTERNAL_DOCS_EVIDENCE_20261102`。

## 范围与源码

- 根工作区：`E:/mir2-player-journey`；维护分支 `codex/windows-player-journey`。
- 本轮开始 HEAD：`e45f36420d1a4241a1e96fed58a86345f97e36e5`，统一代码线检查通过。
- 官网：`mir2-web3/apps/website`，Next 16.3.3、React 19.2.3、TypeScript 5.9.3。
- 游戏网页与认证：`mir2-web3/apps/web`；网关/角色权威：`apps/gateway`、`apps/simulation`；
  Windows：`apps/game-client/platform-windows`、`client-bevy`。
- 本机 Node 22.18.0、npm 11.13.0；官网采用现有 package-lock，`npm ci` 安装，未引入依赖。
- packageManager 字段另声明 pnpm 9.15.4；与本机全局 pnpm 版本不一致，故本轮未混用锁文件。
- 初版本地验收时尚未提交或部署；用户随后授权生产发布，最终源码与生产部署见本文件末尾。
- 开始时工作区已有 Windows 资源/安装/更新器、官网 `.env.production`/README、
  AGENT-CURRENT、下载镜像等未提交改动；本轮保留，不将其测试或改动算入交付。

## 来源、版本与冲突

| 来源 | 版本或核对位置 | 本轮采用方式 |
| --- | --- | --- |
| 用户本次执行上下文 | 2026-11-02 | 原创 IP 名称、无剧透摘要和世界观硬约束；不据此推定代码已实现 |
| `docs/GAME-DESIGN-CODEBASE-DRAFT.zh-CN.md` | v0.1，2026-10-25，开头、世界观与商业化章节 | 可修改讨论初稿；不是批准后的完整世界观或正式付费规则 |
| `docs/GAME-DESIGN-BUSINESS-LOOP.zh-CN.md` | v0.2 立项草案，旧名称 | 历史输入，不公开收益/节点/回本设想 |
| `docs/LEVEL100-SLG-PROGRESSION.zh-CN.md` | 等级100方向与待实现清单 | 仅内部待确认，不等同已上线等级或生命次数 |
| `apps/website/lib/site-copy.ts`、`product-copy.ts`、`download-copy.ts` | 现有官网宣传内容 | 复用框架和导航；旧玛法/魔法/AI订阅、平台描述与本次方向有差异，未借机重写所有旧页面 |
| 完整内部原创世界观、最新批准策划 | 未定位到可访问的批准原件 | 公开简介仅采用用户明确摘要；不冒称审核过主线真相 |

世界观内部边界：玩家人类没有天生异能或生物变异力量；灵微子不是万能创世能源，
主神不是拟人情绪化恶反派。公开页面未复制这两项内部设定或剧情原文。
100级后身体/角色消耗、有限复活、角色交易、身份社交继承、官方供给、AI世界导演、
创作者代币兑换以及约200美元回收均不是本次可发布规则；具体讨论留在内部，
公开规划页只写尚无公开契约的方向，不给日期、端点、地址或收益承诺。

## 玩家内容证据

路径均相对 `mir2-web3/`，定位到符号后以当前源码为准。

| 页面结论 | 当前证据 | 限制 |
| --- | --- | --- |
| Windows 测试发行与钱包入场 | `docs/UNIFIED-PLAYTEST-PUBLISH-20261101.zh-CN.md` r16 / `e9117b6` 记录；`platform-windows/src/wallet_login.rs` create；`client-bevy/src/native_shell_ui.rs` F8 | 11月1日指定环境记录，不冒称本轮又做手机签名或所有平台验收 |
| 角色创建独立于登录 | `client-bevy/src/native_shell.rs`、`platform-windows/src/shell_bridge.rs` | 网页推广报名不创建角色；钱包地址不是角色授权 |
| 三/五职业按内容配置开放 | `packages/game-data/data/content_profiles/platinum_176.json`、`platinum_fiveclass.json`；native_shell 创角白名单 | 不保证所有服务器开放五职业 |
| 背包F9/I、装备F10/C、技能F11/S | `client-bevy/src/crystal_ui/keyboard_defaults.json` | 默认值；设置、焦点和版本会影响操作 |
| 战斗与任务引导 | `client-bevy/src/combat_input.rs`；`config/quest-guidance/newcomer-journey-v2-ui.json` | 不发布未经验证的固定成长/收益数值 |
| 玩家交易与行会 | `gateway/src/routing/tests/trade_invitation_tests.rs`、`trade_item_edit_tests.rs`；`simulation/src/runtime/shared_guilds.rs`、`shared_guild_management_commands.rs` | 有代码不等于本轮双人流程验收；以对应环境状态说明为准 |
| 返回绑定点复活、死亡物品风险 | `simulation/src/runtime/movement.rs` 复活分支；`runtime/drops.rs` PvP死亡处理 | 具体概率/例外地图/补偿政策待批准；不承诺永不掉落 |
| 充值、到账、月卡消费分阶段 | `gateway/src/web_billing.rs` 商品/开关；`simulation/src/monthly_card.rs`；r16 发布记录 | 1测试USDC→100点券是指定TEST验收；法币/月卡本轮没有重新付款验收 |
| 清档与免费范围 | monthly_card 配置默认值、历史数据保留记录 | 默认配置和单次保留不能推导永久政策，公开留待公告 |
| 支持入口 | 官网 `/{locale}/support` | 未新增邮箱/Discord；原支持页的默认联系方式仍需运营确认有效性 |

## 开发者内容证据

- 官网 `apps/website/app/api/game-status/route.ts`：公开 GET，3500ms 上游超时，
  ok/checkedAt/latencyMs/source；异常仍HTTP200，no-store，未开放任意跨域CORS。
- 本轮只读公网探测：HTTP200；2026-11-02 05:44:37 上海，ok=true，latencyMs=435。
- 新例子真实运行：`node public/docs-examples/read-status.mjs https://mir2.obelisk.build`，
  2026-11-02 05:55:27 上海，退出0，healthy，latencyMs=206。只证明配置的健康探测。
- 本地例子真实运行：2026-11-02 05:59:08 上海，退出0，healthy，latencyMs=1041。
- `apps/web/lib/server/dubhe-connect-auth.ts`：Origin、固定relay/网络、2分钟意图、
  Bearer轮询、消息/bytes/地址/公钥/时间/网络一致性与Sui签名验证。
- `apps/web/app/api/channels/session/exchange/route.ts`、`gateway/src/web.rs`、
  `gateway/src/auth.rs`、`gateway/src/channel_identity.rs`：proof消费、canonical账号关联、
  token与StartGame认证。公开只解释流程，不发布内部写入端点或第三方登录SDK。
- Windows 的意图创建也在 `platform-windows/src/wallet_login.rs`；不是必须先开网页。
- 通用 Dubhe 文档与公开源码、钱包产品入口：本轮重新GET均最终200：
  `https://dubhe-docs.obelisk.build/`、`/dubhe/sui/client`、
  `/dubhe/sui/contracts/session-keys`、`/dubhe/sui/quick-start/prerequisites`、
  `https://github.com/0xobelisk/dubhe-docs`、`https://wallet.dubhe.obelisk.build/`。
- 旧 `https://github.com/0xobelisk/dubhe-wallet` 返回404；没有确认公开钱包开发文档原件，
  不把它添加为教程链接。内部vendor与私有revision不进入公开教程。

## 实现与公开边界

- `/{locale}/guide`：9篇；`/{locale}/developers`：10篇。首版正文仅中文。
- 非中文URL保留站点语言导航，正文标记zh-CN/ltr，并明确翻译未审，noindex；
  canonical与sitemap只宣传中文文档版本，避免假称16种正文已审。
- 服务器渲染文章，仅目录关键词筛选为客户端交互；没有新增写入API或交易授权。
- 显式公开内容仅在 `apps/website/lib/*content.ts`，不扫描/导入内部 docs。
- 示例最终在 `public/zh-CN/docs-examples`，复用已有语言前缀静态路径转发；
  localized文档路径沿用已有营销路由转发。未改认证、玩法、数值、合约、DNS、凭据。
- 未部署生产，未启动Docker、游戏服务或资金操作。官网本地预览使用127.0.0.1:3080。

## 验收记录

已执行：

- `python mir2-web3/scripts/verify_unified_codeline.py`：通过。
- `npm ci --prefer-offline --fetch-retries=0 --fetch-timeout=20000 --no-audit --no-fund`：现有锁文件28包安装成功。
- `npm run typecheck`、`npm run i18n:check`：通过。
- `npm run build`：Next16.3.3构建通过，648个静态页面。
- `npm run docs:example:test`：7/7（健康、降级、HTTP错误、重定向、结构、非JSON、超时）。
- `npm run docs:check -- --origin http://127.0.0.1:3080`：19篇/59节/40内容链接、
  27路由/139锚点通过；非中文提示、未知路由404、首页/下载/支持导航、示例原文与sitemap通过。
- `node --test apps/web/scripts/test-marketing-routing.mjs`：8/8；包含16种语言的新文档路径和示例静态文件转发，原游戏/推广/认证边界继续通过。
- 最终本地构建ID：`3uKxJ__0IEaopk7bfnOGO`。源码仍是上述HEAD上的未提交文档工作区；这不是新的线上部署revision。
- 最终公开构建扫描：114个相关公开文本产物，以及实际文档HTML，未检出内部绝对路径、私钥标记、内部证据canary或未批准经济机制。
- 内部策划路径、内部交付文档路径及 `/.env.production` 实际访问均404；不是依赖隐藏导航或robots保护。
- 浏览器桌面1440×1000与手机宽度390×844检查：实际内容区1425/375px（扣除滚动条）无页面横向溢出；
  关键词“月卡”筛选9→1、手机目录展开、主导航双入口与跳转、首页指南卡片、示例本页锚点通过。
- 文档手机页隐藏原官网浮动游戏CTA，避免遮挡正文；代码块和表格在自身容器内横向滚动。
- 开发示例浏览器错误日志为空。本轮只是响应式浏览器检查，不冒称实体手机签名或游戏完整验收。
- [机器检查回执](evidence/numeron-website-docs-20261102/checks.json)；桌面与手机截图见同目录。

## 修改文件清单

官网目录 `apps/website/`：

- 新路由：`app/[locale]/guide/page.tsx`、`guide/[slug]/page.tsx`、
  `app/[locale]/developers/page.tsx`、`developers/[slug]/page.tsx`。
- 新内容：`lib/guide-content.ts`、`lib/developer-content.ts`、`lib/documentation.ts`、
  `lib/documentation-types.ts`、`lib/documentation-labels.ts`。
- 新展示：`app/components/documentation-page.tsx`、`documentation-directory.tsx`、
  `documentation-entry.tsx`、`app/documentation.css`。
- 现有入口：`app/[locale]/page.tsx`、`app/[locale]/layout.tsx`、
  `app/components/site-chrome.tsx`、`app/sitemap.ts`。
- 示例与检查：`public/docs-examples/read-status.mjs`、`scripts/check-documentation.mjs`、
  `scripts/test-documentation-example.mjs`、`package.json` 的新增检查命令。

最终无需修改 `apps/web/next.config.ts`；跨应用仅扩展 `apps/web/scripts/test-marketing-routing.mjs` 的文档与示例路径回归覆盖。
内部本文件与 `docs/evidence/numeron-website-docs-20261102/` 保留证据，不进入公开内容源。
已有官网README和 `.env.production` 的暂存改动不是本轮创建或修改。

## 本地预览与复验

```powershell
cd E:\mir2-player-journey\mir2-web3\apps\website
npm run dev
# 已构建时可用 npm run start；不要与现有3080预览同时启动。
npm run typecheck
npm run i18n:check
npm run docs:example:test
npm run docs:check -- --origin http://127.0.0.1:3080
```

## 待确认规则与上线前清单

1. 提供最新批准版完整原创策划与世界观原件，确定公开术语；处理旧营销页玛法/魔法/AI订阅与新文档的差异。
2. 批准正式免费范围、月卡商品与价格、退款/地区、清档/回档/保留/补偿政策。
3. 确认各测试环境职业开放、死亡掉落规则与正式上限；不从代码默认值代填产品政策。
4. 确认真实支持联系方式，补充可公开访问的钱包开发文档。
5. 若扩展角色读取/事件/第三方玩法，先批准权限、数据隐私、版本兼容和实际接口，不先铺空壳。
6. 中文内容人工审阅；其他语言逐项审核后才加入索引与语言alternates。
7. 对精确提交构建并在预览环境核对跨应用路由、静态例子、导航和站点基线；生产推广需另行授权。

证据截图目录：[evidence/numeron-website-docs-20261102](evidence/numeron-website-docs-20261102)。

## 生产发布准备（用户于本轮明确授权）

已确认只发布现有营销官网项目：prj_Q14gZuvjxkfqOCExhqOrYiR2UbxA / Obelisk Labs。
官网当前基线 dpl_5Tt9YxztUDSbFKgQQTCdkZG3myhx，源码427f1d11；
游戏域名基线 dpl_F1ZwKtAph1YVBVYHdVfKVWhfBZ9S，/version仍为3a52daef。
现有游戏路由已转发所有语言子路径，故示例从public/docs-examples迁到
public/zh-CN/docs-examples/read-status.mjs，正式URL为/zh-CN/docs-examples/read-status.mjs。
撤销本轮新增的apps/web/next.config.ts改动，游戏前端与后端均无需部署。
此前的历史本地回执仍适用于旧示例位置；发布验证按新位置重新执行。
仓库可公开访问，因此本内部证据文档和截图不进入源码提交，只提交明确列出的公开官网代码及路由测试。
本机gh身份失效，使用已有Vercel连接器与SSH Git授权，不新增凭证。

## 最终生产结果

- 已推送统一分支：`00858ce9b850218dd07ea96daea4bbfe6b68035b`，仅22个公开文档相关文件；15个原有暂存文件的模式与blob逐项保持。
- 最终官网部署：`dpl_4dXyLJo6QFudTXzKNS6kbuEMFJd4`，READY / production，Git source与上述SHA一致。
- 主官网别名已确认指向最终部署；正式文档为 https://mir2.obelisk.build/zh-CN/guide 与 /zh-CN/developers。
- 第一次生产尝试的页面可用，但遗漏原部署一次性Windows下载配置；已恢复旧官网，再带入同一公开安装器URL重建。修正生产候选先验证下载链接与原版一致，之后才提升为正式版本。
- 最终不可变构建命令设置canonical origin与原Windows下载URL，不改DNS、域名所有权、长期凭据或全局环境配置。
- 最终公网重新运行 `node scripts/check-documentation.mjs --origin https://mir2.obelisk.build`：19篇/59节/40内容链接、27页面/139锚点通过；相关公开产物114个、内部文件404检查通过。
- 最终公网Node示例再次退出0：healthy，2026-11-02 07:04:07上海，latencyMs255。
- 原Windows安装器链接精确保持；游戏 `/version` 仍为 `3a52daef5c3fe92e1e738d6656b9fdbf54de56c3`，PWA入口/game及推广页面可读。
- 只发布官网；未构建/发布Windows客户端、未部署游戏前后端、未启动Docker。
- 内部证据及本文件仅本地保留，没有加入公开Git提交。
- 机器证据：[最终结果](evidence/numeron-website-docs-20261102/production-final.json)、[实际公网检查](evidence/numeron-website-docs-20261102/checks-public-production.json)。


## 第二版与 numeron 官网域名（2026-11-02）

第二版仅本地预览，未提交或推送；新域名已绑定当前原官网第一版部署。
内容范围、实际生产deployment与本地构建、检查和未完成截图范围见[本轮单一证据记录](evidence/numeron-website-docs-20261102/second-edition/README.md)。
