# Numeron R16 Web 发布记录

## 2026-11-10 第一方扫码中继与出码读取

Numeron 游戏部署改用 `relay.dubhe.obelisk.build`，复用已在线的 Dubhe 中继和官方钱包批准页面。第一轮用同一 `f2de762309` 产品代码和固定资源先出候选，再切游戏域名的部署指向。候选和公网均验证了实际创建会话及待确认查询。 [隔离构建](https://github.com/Zombieliu/mir2/actions/runs/42731630805) · [切换记录](https://github.com/Zombieliu/mir2/actions/runs/42732390871)

直连浏览器第二轮实拍二维码 2401 个模块完全匹配。官方钱包手机尺寸 PWA 用测试网身份确认了个人登录消息，中继返回实际批准。验收在读取兑换结果前被通用请求识别条件拦下，原始请求未留类别，保留为部分验收。

默认 Chrome 冷游戏页的两次按钮试验在 12 秒中止。另一次同网络同源诊断在 3.215 秒收到响应头，13.309 秒才读完整 1320 字节。轻量页面读取同样响应只用约 0.6 秒。出码候选因此给首次和一次瞬时失败重试各 20 秒。待确认查询使用原会话、原有效期和 12 秒上限。签名后的单次兑换仍走原流程。24 项钱包回归、69 项发布守卫和 TypeScript 检查通过。 [读取时序、发布和部分验收](evidence/numeron-web-release-20261108/qr-first-party-relay-20261110.json)

最终公网版本读回 `83eadf4f`。隔离源提交以 `f2de762309` 为父提交，只包含扫码客户端文件。素材仍按原发布清单复核。发布首轮在拉取仓库时触及 runner 限时，原候选重试后通过受保护出码和公网待确认，域名指向同一候选。 [候选](https://github.com/Zombieliu/mir2/actions/runs/42734782902) · [发布](https://github.com/Zombieliu/mir2/actions/runs/42735819303)

最终直连 Chrome 一次实际出码，官方 Dubhe Wallet 手机尺寸 PWA 以虚拟通行密钥确认个人登录消息。轮询由待确认转为已批准，兑换 200，一次 `LoginSuccess` 后创建独立测试角色。`StartGame(4)` 到比奇，177 张已加载可见地块通过画面取样，正常退出收到 `LogOutSuccess` 并返回选角。钱包请求审计通过，两个浏览器上下文均退出。实体手机扫码相机另验。 [公网完整链路](evidence/numeron-web-release-20261108/qr-first-party-relay-20261110.json)

同机默认网络的首轮冷启动两次在 20 秒中止。独立默认 Chrome 在等待资源的上限到达后点击，15.101 秒收到完整出码正文，9.236 秒完成待确认查询。最后用独立默认 Chrome 冷启动并在登录入口出现时立即点击，两次请求仍各约 20 秒中止。冷启动稳定性继续验收。 [默认网络对照](evidence/numeron-web-release-20261108/qr-first-party-relay-20261110.json)

## 2026-11-10 扫码冷启动与登录资源

在 `83eadf4f` 上，关闭可选预热把点击时的并发 GET 从 21 降至 12，两次扫码请求仍在 20 秒内等不到正文。同一默认代理的轻量页面分别用 HTTP/2 和 HTTP/1.1 读完 1,320 字节，耗时 569 和 759 毫秒。游戏页只屏蔽原登录音乐时，出码接口返回正文但待确认查询仍超时。诊断同时屏蔽登录大字体和音乐后，普通按钮显示二维码并收到待确认状态。 [脱敏对照](evidence/numeron-web-release-20261108/qr-login-load-diagnostic-20261110.json)

隔离候选 `a23cc382` 使用原字体生成的简中和繁中 WOFF2 字集，原始字形的采样字宽及边界一致。登录阶段保留可见原素材并跳过默认可选精灵预热，完整字体延后到选角，普通登录交互后可播放原音轨，扫码等待期间暂停。实际本机生产构建接隔离中继，简中和繁中普通入口出码约 341 和 235 毫秒，各一份小字体，均收到待确认。取消后点击账号框发出原音乐请求。隔离构建用于验证加载流程，公网签名和进城另验。 [本机回执](evidence/numeron-web-release-20261108/qr-login-startup-local-20261110.json) · [隔离发布作业](https://github.com/Zombieliu/mir2/actions/runs/42742128253)

## 2026-11-10 公网钱包扫码恢复

普通浏览器打开 [Numeron 游戏](https://numeron.obelisk.build/game) 后，出码、Dubhe Wallet 登录确认、进比奇和正常注销已复测。出码超时期间，Chrome 同源 HTTP/2 请求未收到响应；换到经 Cloudflare 的同域候选后可出码。另一个故障只在签名后出现：Caddy 缺少 TEST 的 `POST /playtest/v1/channels/session/exchange` 路由，返回 404。

- 在同一 CNAME 开启 Cloudflare 代理，保留 Strict TLS 与 Vercel 目标。[DNS 发布](https://github.com/0xobelisk/dubhe-wallet/actions/runs/42672637650)记录了旧值和新值，发布只变更这一条记录。
- 用线上 Caddy 2.6.2 验证并热载精确的 TEST `POST` 路由，改动在 `2b5a0461a8ae`。[路由](../infra/playtest/Caddyfile.routes)转发到原 TEST Gateway；无效签名得到 401，相邻路径和其他方法仍未开放。线上 Web `f2de76230976` 与 TEST `d53f9e3b22e2` 均原版本。
- Chrome 使用正常公网 DNS、TLS 1.3 和 HTTP/2。实际二维码、批准轮询、交换 200、普通 `LoginSuccess`、`StartGame(4)` 与当前角色及比奇地图回包均匹配。画面有 174 张已加载的可见地块并通过像素检查；收到一次 `LogOutSuccess` 后回到选角。[脱敏收据](evidence/numeron-web-release-20261108/wallet-qr-incident-readback.json)

签名由官方 Dubhe Wallet 手机尺寸 PWA 使用测试网虚拟通行密钥完成，没有转账。使用一个无资产的独立测试钱包与一个新测试角色。实体手机相机未验；成功的桌面回合仅为该测试绕开本机已有的系统代理，代理路径仍有静态资源等待。早期签名后 404 和进城视觉探针失败继续保留。本轮不涉及公开服阿洛、三职业全任务、Sui 主网或全局验收。

## 2026-11-10 公网扫码分段复验

同一公网 Web 版本仍为 `f2de762309`。直连出码、待确认轮询和官方钱包手机尺寸页面通过。明确经本机代理的桌面请求约 12 秒中止。另一次全直连试验中，钱包读取中继会话约 22 秒超时。该试验创建了隔离测试网钱包，停在签名前，游戏账号与角色均未创建。完整复验仍开放。 [直连、代理及中继回执](evidence/r16-web-fishing-loop-20261110/public-qr-readback.json)

## 2026-11-09 已发布结果

最新候选已发布至 **https://numeron.obelisk.build/game**。本次为具体发布任务，持续1:1 Goal已清除；没有扩大为Sui主网、全局Candidate或最终真人Accepted。

| 部分 | 实际版本与证据 |
| --- | --- |
| Web与营销 | `f2de762309768b51b39438c2437c14e819df9bf7`；[stage 42495986185](https://github.com/Zombieliu/mir2/actions/runs/42495986185)、[promotion 42498751311](https://github.com/Zombieliu/mir2/actions/runs/42498751311)成功 |
| 用户域名 | [42503221364](https://github.com/Zombieliu/mir2/actions/runs/42503221364)成功；官方项目间move至`numeron-r16-web`，未改DNS、注册或旧Mir2项目 |
| Linux TEST Gateway | `d53f9e3b22e2b92c2dc86644b6ae370fc1acab53`；[42483702437](https://github.com/Zombieliu/mir2/actions/runs/42483702437)整体通过。d53→f2de没有Rust/Gateway/游戏逻辑差异 |
| 内容 | 共同基准`platinum_fiveclass` v1 / `newcomer-v2` / Native引导；真实realm bundleHash=`a7d5126c0418536e30a835667e101687a5f54700060985349205f6b93f7262da` |
| 固定素材 | `mir2/v/20261108-r16-08ec86d5105c`，42738份；[f2de复核42494701555](https://github.com/Zombieliu/mir2/actions/runs/42494701555)全部公开读回一致、写入0；原完整发布42483702580成功 |

新Game deployment为`dpl_Ech5Rehf78MG9FXH7fAV9wwFC7GZ`，营销为`dpl_56jyXmxzdtTfNYGpzy4MhofziJRt`。`/version`实际返回f2de；`/game`、官网首页、中文下载页均200，同源营销资源、PWA和Web按钮切换前后均核对。

连接既有隔离 **TEST `/playtest`**。原`mir2.obelisk.build`项目、正式Gateway和钱包服务未替换；未更新或删除Windows EXE。旧Numeron官网Windows卡片原本不可下载，新部署保持这一状态，原Mir2下载不变。

### 数据保留与切换

正常停止TEST后，保留一致PG备份、原12个有效EnvironmentFiles、unit/drop-ins、进程配置、recovery树及顶层小状态文件。恢复树的空子目录与0B文件均保留。新Gateway运行后二次检查：原129账号、43角色、43档、33迁移保持，三类原始JSON行散列全相同；所有旧配置文件保持。原正式Gateway和钱包服务PID未变。

原MIR2配置除明确的内容profile、地图包、追加Origin保持；`newcomer-v2`原已存在。另补齐`MIR2_DEPLOY_REVISION=d53…`元数据：停前连接/会话/登录计数均0，再次正常停止、备份和启动，health版本与实际二进制一致。没有重置签名key、钱包、计费或业务政策，没有恢复覆盖原数据库。

三职业QA后另做一次原行子集比较：现库132账号/46角色/46档、迁移仍33，原129/43/43的JSON行均保持。新增的正常认证、游戏事实及关联审计记录单列于[后续保全收据](evidence/numeron-web-release-20261108/post-public-protocol-preservation.json)，不称全库没有新增。

切换后全表散列变化单列：`shared_conquests`、`shared_guild_clock`为运行时钟；`promotion_revenue_source_holds`两行通过最终备份与现库真实COPY逐列比较，**只有`updated_at_ms`变化**，金额、状态、关联字段保持。不能写成全库逐字不变。

### 实际验收范围

- 原43档真实simulation三轮129次StartGame成功；两个profile直接checkpoint/store/snapshot零差异。普通登录共同归一化及二次序列变化完整保留，严格原档逐字比较仍false。
- 两个隔离低权限PG克隆，4个原有角色在e911/d53各8次普通认证/StartGame/正常注销；72组跨版本投影零差异。克隆凭证不证明原用户钱包所有权；全部临时Gateway/Redis和隧道已停止。
- 公开TEST战士、法师、道士普通注册创角、进城、Walk/UserLocation、首项V2任务2110001唯一接交、准确奖励及密码重登入档通过，全部正常注销。仅新增三个人工QA账号/角色；没有GM、赠物、WorldCommand或链交易，不冒充公开服全26任务至30级复验。
- 最终Numeron域名钱包会话创建200、合法Wallet批准URL、119秒有效期、pending查询200。未扫码、签名、付款或调用模型。
- 最终域名浏览器三职业普通登录、选原角色、实际进城、中文共享Native Q绘制及正常LogOutSuccess均通过。战士另有两次可信键盘步进及对应UserLocation；法师/道士沿用同一任务浏览器的真实缓存，各正常退出。三张任务窗截图已目视核对。首次driver误把注销后选角页当作登录页，原false保留，同实例普通点选角“退出”后完成续验，未绕过权限或重新注册。
- 本次UI没有重做公开服打怪/吃药/拾取/组队/交易或全26任务；原本机三职业30级和这些操作的旧证据仍属本机范围。

### 保留的失败与修复

首轮资源取消后保留已有新prefix对象，续发逐项读回、最后manifest，未覆盖旧prefix。钱包打包错用Web锁，修复后clean-source CI整体通过。

首轮frontend promotion因候选误暴露旧安装包链接被挡下，零promotion/域名写入；f2de显式清除构建时继承的Windows链接，确认真实下载页后再比较合法零链接基线。新项目素材代理缺失曾404，f2de补齐两个固定CDN服务器变量后，Title完整GET200。

服务器r1只读计划被旧目录组权限检查挡下，未写入；r2在备份空恢复子目录时中止并恢复旧服务；r3完成PG/SQL/recovery备份后发现`systemctl show`重复EnvironmentFiles行被字典丢弃，在启动新服务前恢复旧服务。r2/r3配置备份均只覆盖末个env文件，不能称完整。**旧12个env文件从未被systemd reset**；r4聚合并备份全部12项，实际Linux ROOT41/41后切换成功。旧失败目录/隔离文件保留，无数据库回滚。

候选浏览器短轮和五分钟冷载因本机到Vercel的间歇TTFB/正文等待未通过，失败保留。服务器侧同3资源完整GET：24KB Core约1.17秒、16.4MB中文字体约3.77秒、manifest约4.34秒；Windows curl同资源成功，Chrome HTTP/2有不稳定等待。没有放开域名保护或修改系统代理；后续UI使用任务内HTTP/1.1实例对照。

最终场景首次请求3张blend时新prefix返回404，随后**按manifest已声明的旧prefix继承路径全部200恢复**，后两职业从实际SW缓存读取也均200。完整字体、六个Core文件与场景已绘制；保留3次首次404、console error4/pageerror0，不写成所有请求零错误。另从Linux带实际Numeron Referer完整GET三张继承图片，字节数及SHA均匹配固定manifest；未新增对象或改manifest。首次加载仍偏慢，后两职业复用真实缓存的结果不冒充冷启动性能通过。

安全收据见[发布证据索引](evidence/numeron-web-release-20261108/README.md)。凭证、PG原始导出、真实账号/物品UID、私钥和保护分享票据不入库。云端阿洛运行器未启用；手机签名、付款、全地图/Atlas性能、CCU和最终真人Accepted仍开放。

## 以下为发布前的历史门槛记录

以下“当前/待执行”均指切换前的记录，以本页上方2026-11-09结果为准，保留失败经过。

用户本轮明确授权将新版本部署到 `https://numeron.obelisk.build/game`。持续1:1 Goal已由用户清除，本次为具体发布任务；原完整对齐范围和未结项保持，不宣称全部Candidate或最终真人Accepted。

## 发布目标与保留边界

- 新隔离前端项目：`numeron-r16-web` / `prj_QzS3a2wirTJCixU4tYRVY5nmvSjC`。候选先不分配用户域名。
- 现营销项目：`numeron-legend-of-rebirth` / `prj_Q14gZuvjxkfqOCExhqOrYiR2UbxA`；当前基线 `dpl_22qaP2Cn1my46GJk6xrS9tk8m4RK`。官网代码、下载和语言保留。
- 原游戏项目 `mir2-web3-web` / `prj_Bl3ttB4PzcdCpJMYLRhzQjkdXyTz` 的当前 `dpl_3NBhFUPKxNF4nccRnG94ZhDq1iz6` 保持，不改旧游戏入口及旧正式服务器。
- 新Web候选连接既有隔离TEST `/playtest` 服务；当前仍是e911、三职业，待Linux新候选与旧库克隆验证后切换。共享五职业/V2以仓库基准为准，不能把旧正式服或Windows PE当作已更新Linux。

## 已完成的准备

公会共用窗口、局部输入、普通新建职级、权威统计及只读仓库已有[限定源证据](evidence/r16-guild-layout-20261108/README.md)。旧失败与Native窗口范围继续公开。

固定公开素材为 `mir2/v/20261108-r16-08ec86d5105c`，本地逐文件验证42738份；逻辑417098594B、传输341600433B，根包318602609B。Bundle SHA256为`7328514b323fe73169332f0c6dca35cacc872ff021ddce8212db3ac216bc3dcc`，payload为`08ec86d5105c7c00405c7342fff3392ba118216d780e22d50e716c47cccdd453`，最终manifest为`32255399aaae2dc9a133a5827252256ece782aaae5d237152ffebc2d8f272155`。固定发布器仅写新prefix，先校验全部对象，最后写manifest；旧对象不覆盖或删除。GitHub草稿目标须在冻结后固定到同一提交，CI实际执行结果另外记录。

新站使用专用 `numeron-r16-web-assets.json`，默认旧production profile逐字节保持。两文件名通过受控选择器选择。新profile诚实声明有限素材，fullpack/Atlas预算未验，不将跳过记录算成旧Atlas性能门通过；a469三包固定从新CDN目录读取，不能错误回退到没有它的旧prefix。

原公开资源Worker允许Numeron同站来源；Vercel候选来源对直链可能403，已有公开 `/hotlink-ok/` 备用路径保留。没有为本轮重写Worker或放开账户权限。

服务器凭证已在内存通道验证；原TEST签名配置通过不落明文的通道同步到专用加密CI变量，服务器原key不变。Vercel连接器正式env写权限不足、本机CLI不在目标团队，发布改用既有CI身份，不在记录中保存凭证。

Linux启动门已改为读共享基准并对照真实匿名realmInfo；14项stdlib检查通过。它仍是空PG启动/退出与匿名握手检查，不代表普通注册、新角色、旧档升级或重登已验。SQL注册顺序与e911相同也不能代替这些运行验证。

## 第一轮真实CI及续发修复

源码`6ac7004a8`已推送。Gateway CI `42479116957`的Linux网关作业通过，实际二进制SHA256为`697696b221e1c5d98f006f96217d9ddddd246884b90e885a122d2fd03419d181`；原生PG/Redis启动、五职业/V2匿名身份与正常退出有独立收据。该run整体失败：钱包打包器错误比较Web锁与钱包独立锁中的Next版本。修复仅改用钱包自身精确锁与全部直接依赖验证，不改依赖版本；独立实际构建与standalone认证检查10项通过，正式clean-source打包须由下一轮CI核验。

第一轮资源CI `42479115894`逐文件发布速度不足。续发实现将远端文件并发从8调至32，仍保留每项先GET、缺失时单次PUT、完整字节读回、失败停止新调度并等待在途，全部通过后才发布manifest。已上传对象必须重新读回一致后保留；取消或超时留下的未知写入不能当作成功。固定payload及旧prefix不变。

营销工作流未在默认分支注册，直接dispatch返回404。新增入口使用已注册的Gateway工作流转调同提交的reusable工作流；Numeron模式排除原有网关/钱包作业，默认off保留旧行为，不向主分支合入游戏改动。域名单独操作采用官方项目间move，只有精确候选、生产映射及公开页面核对通过才执行；未知结果只查询，不重发或覆盖他人映射。

服务器已完成在线一致PG预备快照，并恢复两个独立低权限克隆。预备快照包含129账号/43角色/43档/33迁移，原服务仍运行；这不是切换排空后的最终一致备份，也不是旧档加载或真人认证已通过。

## 尚未执行的切换门

后续真实结果：d53资源CI `42483702580`已完整通过，42738项公开读回一致，续传保留18645项、新增24093项，manifest最后发布；同源运行器8份gzip均按公开解码语义校验。d53网关/钱包/配套后端CI `42483702437`整体通过。原43份档案双投影一致，真实simulation三轮129次StartGame均加载；两profile直接比较checkpoint/store/snapshot零差异，共同登录归一化及二次序列变化完整保留，不称原档逐字不变。另4角色在e911与d53各8次普通登录/StartGame/正常注销，两轮72组网络投影一致。

首轮前端stage `42486493463`两候选READY；promotion `42489950552`因旧官网无Windows按钮而候选误暴露旧安装包链接被门槛阻止，零promotion/域名写入。新发布配置显式保留旧官网Windows不可下载状态，先确认真实下载页面才允许零链接比较；不修改原Mir2下载或EXE。保护候选的登录贴图、十语菜单与中英阿截图已有，但完整冷启动尚未通过。实际GET确认新项目资产代理未配置而404；下一候选将补齐两个固定CDN服务器变量，实际字体/Core/入城仍须复验。

- 冻结源码、推送，更新草稿target，固定资产CI真实公开读回与最后manifest发布。
- Ubuntu24/Rust1.95独立Gateway包、安全/PG/地图门，以及现TEST旧库克隆、普通新号与保存回读。
- 新Game与营销同源production候选构建、真实页面/版本/PWA/下载检查；候选普通登录、素材、Q窗口与保存。
- 排空及一致备份TEST，保留keys/计费与旧数据，追加精确新origin，不触旧正式服。
- 用户域名最后切换，公开`/game`及真实WebSocket复验；失败只回退仍由本次候选持有的映射，不能覆盖他人并发版本。

当前没有将上述待执行项勾为完成。域名move仍独立于已实现的stage/promote脚本，必须在候选验证后执行。
