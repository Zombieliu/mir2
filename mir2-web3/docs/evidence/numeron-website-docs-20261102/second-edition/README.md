# 官网第二版与新域名接入 · 2026-11-02

本地第二版基于 canonical `codex/windows-player-journey` / `00858ce9b850218dd07ea96daea4bbfe6b68035b` 上的本轮网站工作区改动；未提交或推送。构建 ID `FIZRntWYU66_6t28AFzma`，649 静态页面。对应文件摘要见 `source-files.json`。

## 本地内容

- 中文首页以 Numeron 原创灾后世界为介绍，说明当前测试地图、职业和素材与最终原创内容的关系。未改实际游戏设定或数值。
- 桌面与移动主导航直接提供玩家指南/开发者文档，保留下载与团长推广。Token 与天机阁放入二级导航；移除首页模拟在线人数等数字展示。
- 玩家开始阅读直达新手步骤，补充下载、F8扫码、创角、首次冒险、卡点排查；加入两张2026-11-01 Windows r16参考图（登录、二维码过期），不声称是本轮新游戏验收。
- 图片来源：`E:/mir2-release/unified-playtest-publish-20261101/windows-r16/interactive-public-updater/public-updated-packed-game-login.jpg` 和 `interactive-wallet/qr-expired.jpg`。已目视确认账号密码空、无有效QR/钱包/订单。未修改图像。
- 开发者示例直接从新公开域名下载并运行，本地官网仅是可选环境。运行示例仅验证健康探测，不证明登录、付款或角色写入API。
- 本地配置新增 `/guide`、`/developers` 入口及 `/game`、`/promotions` 到已验证原域名的临时跳转，保留邀请路径和查询参数；未改认证/支付域名。
- metadata/sitemap/robots默认官网地址为新域名；构建显式保留现有Windows安装包。原用户暂存的`.env.production`与README没有修改。

## 验证结果

| 边界 | 结果与证据 |
| --- | --- |
| canonical | unified-codeline guard通过，HEAD如上 |
| 构建/类型 | Next16.3.3 build649静态页；typegen + tsc通过 |
| 国际化 | 16语言结构检查通过；中文第二版不表示其他语言已重新审核 |
| 文档/链接/公开内容 | 19文章62节44内容链接，27路由145锚点；图片HTTP200 JPEG；4临时跳转保留查询参数；114公开文本产物扫描，无内部路径或私钥标记 |
| CLI错误边界 | 7/7示例检查通过（健康/降级/HTTP错误/重定向/结构/非JSON/超时） |
| 原前端路由 | marketing routing8/8通过；游戏/推广/认证路径仍独立 |
| UI | 桌面1440×1000与手机390×844；实际宽1425/375无页面横向溢出；F8图naturalWidth1026且加载完成；主导航→开发文档→开始阅读可到公开quickstart；浏览器error日志空 |
| 公网域名 | 已绑定原官网项目；Cloudflare CNAME指向Vercel返回的目标、附加TXT且保留原TXT；Vercel verified=true / misconfigured=false；系统默认证书校验HTTPS200 |
| 公网读取 | 新域名首页/指南/开发文档/下载/示例均200；示例字节与本地一致；健康CLI于00:25:28Z返回healthy/184ms/exit0，仅证明该次探测 |

截图：`home-desktop.jpg`、`guide-login-preview.jpg`、`guide-login-mobile.jpg`、`quickstart-mobile.jpg`、`new-domain-production-guide.jpg`。浏览器临时viewport已reset；原预览仍监听127.0.0.1:3080。

## 生产状态与限制

- 新域名已对外生效。关联原官网项目 `prj_Q14gZuvjxkfqOCExhqOrYiR2UbxA`，实际指向既有 `dpl_4dXyLJo6QFudTXzKNS6kbuEMFJd4` / source00858ce9b850218dd07ea96daea4bbfe6b68035b。
- 第二版仅本地预览，未新建生产部署/切换生产内容。新域名当前仍为原已发布第一版。
- 新域名当前导航会跳原 `mir2.obelisk.build/game` 和 `/promotions`。第二版新增的路径重定向尚未进入生产；不承诺新域名同源登录或cookie共享。
- 没有改DNS的其他记录、游戏后端、Windows客户端、经济规则或钱包权限；没有Docker、签名或资金操作。
- 创角/地图现有截图含原用户角色与精确坐标，未新增公开；首战无匹配的干净实机图，留待后续采集。没有凭空补图或声称完整图文覆盖。
- 其他语言首页还需跟随中文第二版独立审校；尚未明确的免费/清档/死亡概率/正式收费政策保持原待确认范围，未自行决定。
- 下一次批准发布时须设置生产构建的 `NEXT_PUBLIC_SITE_URL=https://numeron.obelisk.build`，同时保留已验证Windows安装链接；先验收新构建再promote。当前未改全局Vercel环境变量。

详见 `domain-binding.json` 和 `public-domain-checks.json`。这些内部记录不进入网站public或公开源码提交。


## 内容收拢（最新本地版本）

用户反馈内容分散后，目录与侧栏收拢为开始游玩、边玩边查、规则与帮助三组；开发者目录按认识边界、动手接入、测试维护分组。统一上一篇/下一篇顺序，新手从开始游戏进入基础操作。开始游戏主目录只保留下载、登录、创角、首次冒险四步，二维码过期与其他故障置后折叠；重复身份解释集中到FAQ，键位集中在操作页。移除玩家正文末尾跨到开发文档的跳转，以及滚动时重叠的侧栏底部标记。

最新构建 `kh0ziLeLrCNvLhYDW0jGA`（取代本记录上方的旧本地build），build类型检查通过；文档19篇62节44内容链接、27路由141锚点、114公开文本产物扫描通过；目录每篇恰好归组一次、下一篇到controls、折叠状态与4步次序检查通过。桌面1425px/手机375px无横向溢出；搜索9→1→9，二维码说明关闭→展开→关闭通过，console error空。截图见directory-grouped-desktop.jpg及directory-grouped-mobile.jpg。

源码未提交、未推送、未部署；生产仍为原domain-binding.json记载版本。源文件摘要与本次回执见content-grouping.json。原用户暂存修改保留，未启动Docker或游戏。预览通过隐藏的已知Node进程监听127.0.0.1:3080。


## 生产发布完成（最新状态）

用户明确批准发布后，20个预览文件的摘要与已验收工作区一致，仅提交官网文件到cf1d1514df315595802e5992d46eced60325aa06并推送canonical分支。原用户暂存改动逐条对比保留。第一版生产构建上线后，完整公网检查发现Vercel空通配符会在/game入口追加尾斜杠；已补明确根路径规则，提交并推送aecf01a81d1a3552bb15283e06859e5a9f59d169，候选精确跳转通过后promote。

最终生产部署dpl_C56xb5m5UAS5WYZ4H8mCYojr3HCq，URL https://numeron-legend-of-rebirth-77ztq975u-obelisk-labs.vercel.app，numeron.obelisk.build别名已核对为该部署。构建显式保留原Windows安装器并设置新官网canonical；未修改全局环境变量或DNS。promote恢复正常生产发布流程。

最终公网node检查：19篇62节44内容链接、27路由141锚点、图片JPEG、示例字节、内部文件404、语言noindex、sitemap及四条临时跳转均通过。公开Node示例healthy，2026-11-02T10:49:41.289Z / 341ms / exit0，仅证明该次健康探测。系统证书验证保持开启。最终浏览器截图production-organized-guide.jpg已目视核对三组目录；DOM交互工具超时，未声称重新完成生产搜索交互，既有匹配源码的本地交互记录仍独立保留。

游戏前端/version通过已连接Vercel读取，前后均为3a52daef5c3fe92e1e738d6656b9fdbf54de56c3。没有启动Docker、重启游戏、修改账号角色点券数据或执行钱包操作。

本节取代上文各阶段“未发布”的状态；详细回执production-release-final.json、checks-public-organized-production.json、public-example-result.json。
