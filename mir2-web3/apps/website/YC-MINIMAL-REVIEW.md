# Numeron 官网最小真实性修改待审阅

核对日期：2026-11-03。此子任务只包含官网文字、公开数字、截图说明及长标签换行修正，未自行合并、提交、推送或部署。用户随后授权发布，由主任务统一处理三站最小修改的最终检查与发布；Atlas 仍独立保留。

## 来源与边界

- 工作树：`E:\yc-v2\numeron`；分支：`codex/yc-minimal-content-review`。
- 基线：`aecf01a81d1a3552bb15283e06859e5a9f59d169`。本轮在该干净基线上重新制作最小修改，没有复制旧 Atlas 重建包。
- 修改范围仅 `mir2-web3/apps/website`。Atlas 的独立源码、原 `/explore` / `/archive` 代理、`next.config.mjs`、`vercel.json`、依赖清单和锁文件均未修改。
- 登录、钱包、充值、游戏服务、官方频道环境变量、下载链接及其条件判断均保留原逻辑。此次本地官网构建不能代替游戏登录或充值验收。
- 策划案中的内容作为设计方向使用；用户确认的当前事实优先。没有用策划目标证明功能已经可用。

## 时间事实分别展示

| 字段 | 当前文案与含义 |
| --- | --- |
| 开发开始 | `2026.05 / 开始开发`；英文 `Development started: May 2026.`。此日期只表示开工。 |
| 当前内部验证 | `当前内部验证：游戏与内部充值测试流程已跑通；完成日期尚未确认。`。独立段落，不放入 2026 年 5 月的更新事件。 |
| 外部状态 | 开发中，准备外部测试；尚未上线、尚未开始外部测试。 |
| 结构化数据 | `VideoGame.creativeWorkStatus = In development`；`Development start` 与 `Current internal validation` 是两个独立的 `about` 对象。未添加发行日期或内部验证完成日期。 |

所有 16 个语言版本使用上述同一事实边界。`internalValidationCompletedAt` 明确为 `null`。更新记录原有日期没有被改成内部验证里程碑。

## 修改前后及移除清单

以下是已从公开输出移除、待用户审阅的内容。没有补造玩家数、收入、交易或留存数据。

| 修改前 | 修改后 |
| --- | --- |
| `12,684` 在线冒险者、`428` 活跃区域、`8,912` 今日事件 | 移除该组公开统计；原始多语言字典中这些数值也替换为破折号。 |
| 首页 `463` 世界地图、`03` 职业、`01` 共享世界作为产品规模指标 | 替换为独立开工日期、PC/Web 规划状态和无数据占位，不宣传已有在线世界规模。 |
| `Return to the continent of Mir`、Mir / Bichon / Sabuk 旧设定、旧地图名作为新游戏公开说明 | 统一为当前原创方向的灾后探索、生存与重建介绍；旧测试地图标题改为中性内部画面标题，未创造新地名。 |
| 世界在线、AI 世界导演、自动切片、外部扩展可用等表达 | 明确为原型或规划中，尚不可用。 |
| Watch 硬编码 `120 SEC`、CSS 伪元素 `READY` / `LIVE` | 删除未经确认的 AI 执行周期；状态改为 `—` / `PLANNED`，伪元素与可见文字保持一致。 |
| 订阅价格、优惠、额度、配额与账户示例 `0 / 200` | 价格和配额显示 `—`；商业条款待定；内部测试到账不等于商业收入。 |
| 教程 `1 测试 USDC / 100 点券`、`10 USDC / 10 美元月卡` 示例及具体完成日期 | 删除数额和未经本轮确认的完成日期，保留充值与消费分开的真实流程解释。 |
| 截图上的 `VERIFIED` 或已验证式标签 | 改为明确的内部截图与 Crystal 临时素材说明，放在图片下方，避免压住图片 HUD。 |
| 相册展示和分享标题 `Bichon Province` | 使用现有的本地化世界/系统/玩法分类名；兼容用的原内部 ID、文件路径、过滤与分享操作保留。 |
| `SIGNED INSTALLER: N/A` 与内部候选版状态混用 | 使用统一的内部验证版本状态文字，安装器下载是否配置仍由原条件判断决定。 |
| 页脚公司与产品关系说明不足 | 说明 Obelisk Labs、Dubhe 基础设施与工具、Dubhe Wallet、开发中的 Numeron 的关系，不删除基础设施业务。 |

此官网未发现 `$500K Protocol Revenue`、`50K+ Active Users`、`30+ Projects`、`97% Satisfaction` 或 `12K+ Players` 原文；其他站的移除结果由各自审阅包列出。本包不把任何 2025 年奖项归属当前游戏。

## 公开图片检查

全部原图二进制保留，未做生成、裁切或涂改。公开页面的图片旁标注内部素材，不能作为当前新设定或已经开始外测的证据。

| 图片 | 图内可见旧品牌或测试内容 | 处理 |
| --- | --- | --- |
| `public/gameplay/character-select.png` | `Legend of Mir 2`；测试角色、等级与 `Last Online 5/18/2026` 等 UI 信息 | 首页、实机卡片及 Play 图旁的可见说明注明内部截图与 Crystal 临时素材；不把图中日期当作当前内部验证完成时间。 |
| `public/gameplay/shared-world.png` | `Bichon Province`、`Legend of Mir 2 Server`、`On-chain Mine(testnet)` | 卡片、相册、Watch fallback 旁的可见说明和下载页 hero intro 均注明临时内部素材；移除公开说明中的旧世界观名称。 |
| `public/gameplay/skills-and-inventory.png` | 旧 Bichon 地图、Crystal 测试 UI、测试角色内容 | 同上。 |
| `public/og.png` | `NUMERON`、`Legend of Rebirth`、`传奇重生`；未见旧 Mir 品牌 | 保留原分享图。 |
| `public/zh-CN/guide-media/windows-login-r16.jpg` | Numeron、Dubhe Wallet，Crystal 临时风格界面 | 教程图片下明确内部验证截图与 Crystal 临时素材。 |
| `public/zh-CN/guide-media/wallet-qr-expired-r16.jpg` | Numeron、Dubhe Wallet，失效二维码测试 UI | 同上；仅说明内部重试步骤。 |

图中角色等级、日期、地图和 testnet 矿区标签只是旧或内部测试画面的组成部分，不能用来推断当前游戏在 2026 年 5 月已完成内部验证、已有真实玩家或已进行外部测试。

## 修改文件

| 文件 | 修改 |
| --- | --- |
| `lib/release-facts.ts`（新增） | 16 语言的已确认事实、时间边界与公开文案覆盖；只处理文字与展示数据。 |
| `lib/site-copy.ts` | 通过统一事实生成公开首页文案；删除原始虚假数字。 |
| `lib/product-copy.ts` | 通过统一事实生成产品、更新、状态、社区、账户等文案。 |
| `lib/download-copy.ts` | 同步内部版本与规划状态。 |
| `lib/market-copy.ts` | 删除附加语言字典中的原始虚假数字。 |
| `lib/guide-content.ts` | 内部测试边界、临时截图说明、移除未经确认的收费数额与完成日期。 |
| `app/[locale]/layout.tsx` | SEO/分享描述使用当前事实；公司 JSON-LD 与两项独立时间事实。 |
| `app/[locale]/page.tsx` | 首页原图增加临时素材说明。 |
| `app/[locale]/play/page.tsx` | 统一内部候选版本标签。 |
| `app/[locale]/download/page.tsx` | 统一安装器状态标签，保留原入口逻辑。 |
| `app/[locale]/watch/page.tsx` | 删除未经确认的 `120 SEC` AI 节拍。 |
| `app/[locale]/[section]/page.tsx` | Press 原图下载区紧邻添加本地化临时素材说明。 |
| `app/[locale]/account/page.tsx` | 去掉虚构使用量与额度。 |
| `app/components/gameplay-proof.tsx` | 临时素材说明放到图片下方。 |
| `app/components/highlight-gallery.tsx` | 展示/分享/替代文本不再引用旧地图设定。 |
| `app/globals.css` | 标题行距与长标签的最小换行、间距和宽度约束；手机截图说明限制在可见图域、隐藏与主标题重叠的装饰底牌；规划状态伪元素。 |
| `scripts/check-minimal-content.mjs`（新增） | 检查 16 语言事实与全部本地生成页面，防止旧设定、虚假数值和时间混写回归。 |
| `YC-MINIMAL-REVIEW.md`（新增） | 本审阅记录。 |

真实的 Discord / YouTube / Bilibili 渲染逻辑、原环境变量和现有官方入口保留；本地无配置的频道仍按原逻辑显示待配置，没有自行创造社区链接、成员数或背书。

## 验证

- `npm run typecheck`：通过。
- `npm run i18n:check`：通过。
- `node scripts/check-minimal-content.mjs`：16 个语言通过。
- `npm run build -- --webpack`：成功生成生产构建。使用现有依赖目录 junction，Webpack 参数仅用于本机避免 Turbopack 外部依赖根限制；没有改部署配置或依赖版本。
- `node scripts/check-minimal-content.mjs --build`：最终构建中的 **641 个本地化页面**通过旧设定/虚假数字及 JSON-LD 时间分离检查，包括页脚与各说明页。
- `git diff --check`：通过。
- `git diff --name-only -- public next.config.mjs vercel.json package.json package-lock.json`：无变更。
- 本地实际运行：`node node_modules/next/dist/bin/next start --hostname 127.0.0.1 --port 3280`。补齐最后两处可见临时图片说明后重新构建，最新 `BUILD_ID = jZlGFjp4K6QwoxseDQdh_`，启动显示 `Ready in 180ms`。入口 `http://127.0.0.1:3280/zh-CN`、`http://127.0.0.1:3280/en`。
- React/Next 静态复核：没有新增客户端状态、请求、依赖或登录/支付副作用；图片 alt 与分享标题跟随同一本地化内容。
- 首轮实际手机检查发现中文和英文首页截图标签超出可见区域，以及底部装饰牌覆盖主标题；已用所有语言共用的手机 CSS 修复。主任务最终 16 语言手机扫描通过，hero 标签无裁切或越界；Watch 的 topline/clip 标签及英、阿下载页目标标签未见矩形碰撞。此后按主审查意见仅补 Watch fallback 与下载页 hero 的可见 Crystal 临时素材说明，字段 guard 同步覆盖。桌面、手机最终截图以主审阅包为准。

仓库统一代码线检查因稀疏工作树缺少 `apps/web/vercel.json` 无法完整运行；没有为通过该检查修改范围外文件，也没有据此宣称游戏或全仓库验证通过。本包基线与分支按主任务指定；本子任务未执行回并与发布，最终整合、线上版本和线上截图由主任务单独记录。

## 仍待确认

1. 内部游戏与充值流程完成验证的具体日期；现仅展示当前结果，不补日期。
2. 外部测试开放日期、名单与访问范围；当前状态仍是尚未开始。
3. 新世界观的正式中英地名、职业及术语表；此次未自行新造译名。
4. 订阅/游戏月卡的正式价格、额度、退款条款与地区支持；保留待定状态。
5. 临时 Crystal/Mir 图何时替换为原创正式素材；目前仍可看见图内旧品牌，并有旁注。
6. 独立 Atlas 原型及其旧路径内容由旧包保留，本轮没有替换代理或声称其数据已经真实接入。
7. 本地没有部署环境配置的官方社区频道需要与真实部署环境核对，不以本地缺省状态推断其不存在。

## 生产 SEO 根域名修正（305c011cb 之后）

主任务发布后复核发现，Git 生产构建读取历史 `NEXT_PUBLIC_SITE_URL`，导致 canonical、hreflang 与分享图片根域名指向 `numeron-legend-of-rebirth.vercel.app`。这是部署特定 URL 配置残留，不是当前产品入口。

本次只修公开 SEO 链接来源：`lib/site-copy.ts` 明确导出 `publicSiteUrl = https://numeron.obelisk.build`，`getLocalizedAlternates` 直接生成完整公开地址；`app/[locale]/layout.tsx` 的 metadataBase 和 JSON-LD、`app/sitemap.ts`、`app/robots.ts` 统一使用该常量。未读取或修改部署密钥，原游戏、支付、登录及 Atlas 代理地址和行为均未改动，UI 无变更。

新增 guard 在导入前注入旧 `NEXT_PUBLIC_SITE_URL` 与 `VERCEL_URL` 值，检查 canonical/hreflang 仍使用已确认公开域名；最终 HTML 另检查每页 canonical、hreflang、Open Graph/Twitter 图片域名，并检查 sitemap/robots 的公开根地址。构建使用同样的旧 URL 环境值验证该回归，不能由旧环境值覆盖公开规范地址。

最后图片引用复核另发现 Play 图旁的可见介绍仍缺少临时素材说明，已仅在 `lib/release-facts.ts` 的 `play.copy` 追加本地化 Crystal 临时素材说明；`gameUrl`、按钮和逻辑均未改动。guard 同时检查全部 16 语言字典以及最终 Play 页 `launch-copy` 区域中的可见说明。

本轮增量共 7 文件：`lib/site-copy.ts`、`lib/release-facts.ts`、`app/[locale]/layout.tsx`、`app/sitemap.ts`、`app/robots.ts`、`scripts/check-minimal-content.mjs`、本报告。没有修改既有 Git 历史；后续提交和生产发布仍由主任务统一进行。

本轮最终结果：`npm run typecheck`、带旧公开 URL 环境变量的 `npm run build -- --webpack`、16 语言与全部 641 页 guard（含 16 个 Play 图旁可见说明）、`npm run i18n:check`、`git diff --check` 均通过。最终合并 SEO/Play 说明修复构建 `BUILD_ID = gpfCzU1nfwldXdzdld3AU`；按主任务要求不再重开本地预览，最终截图由主任务在线上完成。未提交、推送或部署本轮修复。
