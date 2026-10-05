# 下载器耗时诊断与分发进度

2026-10-05后续源码：已集成发行4c60运行时基线及解包、检查、暂存、准备备份、
安装/恢复的真实文件计数、200ms显示节流和阶段耗时。根独立18项相关检查及
Windows入口check通过，与worker重叠；原SHA/fsync、事务schema1及四处fresh
exact-game守卫不变。新进度尚未打包进公开引擎，不能宣称用户已看到或安装提速。
[进度源码和原始回执](generated/player-qa/native-delivery-20261005/updater-progress-01/README.md)。

固定R17 stage/promote脚本已集成，根与worker各46项fake检查通过且重叠；注册
工作流新增独立verify/stage/promote选择，保留默认Web流程，尚未执行真实stage。
实际LinuxCI37250860949是37通过/9个缺少夹具父目录的harness错误，原始日志保留，
没有产品断言失败，凭据及发布/Web任务跳过；根只修测试入口mkdir的parents参数，
Windows46项复验通过，Linux重试待执行。生产脚本未改，source8e1已由GitHub实际ref确认。
实际本地workerd/R2原生流、checksum和条件创建已验证；14个选定用例/152断言，
原10/11与3/4失败及后续真实边界说明保留。它们不能替代Cloudflare授权、远端
36对象完整SHA、CAS指针、公网缓存/range或下载测速；两种实际401仍需有效授权。
[CI工具及安全回执](generated/player-qa/native-delivery-20261005/ci-publication-01/README.md)、
[实际本地平台边界](generated/player-qa/native-delivery-20261005/workerd-platform-01/README.md)。

2026-10-05：原源站现已追加 R17 的11个压缩资源包、1个 EXE 差分及原始
签名描述文件，共14个不可变对象。Linux严格条件追加和31项负向检查通过；
实际HTTPS回环读取全部652,887,693字节及公网描述/范围采样通过。原signed
sequence12/feed、Candidate、引导器、更新引擎、游戏网关与Caddy没有重启或替换。
这关闭了“原源缺少合包”的问题；公网资源采样仍约0.11 MiB/s，带宽瓶颈未关闭。
既有r16引导器包含同一个支持合包的4c60引擎；已进入逐文件下载的运行需要
正常取消后再打开启动器发现合包，已验证的完整内容缓存可复用。

当前追加只改变分发文件，R2/CDN仍未部署。已注册Web发布工作流新增显式
`native_delivery_operation=probe`，原Web任务排除该分支；默认行为保持。
它以Actions现有不透明凭据进行两个固定只读Cloudflare请求，仅保存安全回执。
第一轮实际CI37240009948在用户令牌验证端点返回401，未请求桶，旧Web任务均
跳过；该结果不能单独证明令牌过期。随后按官方账户令牌端点固定路径修正，
仅改变docstring和VERIFY_PATH；根独立32项fake-transport检查通过，与worker
32项重叠，原31项及新增路径RED均保留。账户端点实际CI37243580805也返回401，
未请求桶且旧Web任务跳过；当前现有凭据无法通过验证，需要有效授权后再执行
账户/桶/Worker/路由预检。两次401均保留，不能断言令牌过期。读取成功本身也
不证明写入/Worker/路由权限。
[原始探针](generated/player-qa/native-delivery-20261005/ci-authority-probe-01/README.md)、
[账户端点与实际首轮CI证明](generated/player-qa/native-delivery-20261005/ci-account-probe-01/README.md)、
[实际原源追加与36对象TLS证明](generated/player-qa/native-delivery-20261005/origin-static-01/README.md)。

已集成四文件固定R17上传器，限定36个原始对象及其SHA，逐流条件创建，完整
公共字节验收后才能推进独立CAS指针。Worker模型107项与根独立重跑107项通过
且相互重叠；实际Cloudflare/R2及公共CDN仍未部署。既有Web上传语义保留，原生
前缀拒绝无条件覆盖。
[源码、原始RED与根审阅](generated/player-qa/native-delivery-20261005/r2-importer-01/README.md)。

实际本机Windows库流程已验收：首次安装取消后复用两个完整分包，重试只请求
剩余九包，125,965目标SHA/size全过；旧R16升级使用真实25,424,169字节EXE差分
及两个小文件，无全EXE下载。两条实际隔离回滚均保留六个合成个人文件见证。
这些是dev-profile库与本地文件映射源，不能表示发行EXE、互联网或另一台笔记本
的速度。fresh retry约41.16分钟，其中九包综合事件约230秒，其他时间未拆解；
UI在本地安装阶段缺少进度；发行4c60引擎的阶段进度与计时修复已在源码集成，
新引擎发布与同条件完整安装计时仍未完成。
[精确84-file原始QA与边界](generated/player-qa/native-delivery-20261005/windows-library-qa-01/README.md)。

下列是此前测量和准备记录；原404结果保留，不再表示追加后的当前原源状态。

用户报告 r16 在线下载器需要数小时。该 EXE 只是约27 MB 的引导器；当前
公开 sequence12 实际安装 R17，共125,965个文件、753,611,651字节。根在这台
Windows 开发机用直接 TLS、保持连接、8个签名PNG及两个2 MiB范围请求采样。
PNG等待约59–70毫秒；安装器与游戏EXE分别约0.0956/0.0954 MiB/s，即100 KB/s。
这些是当前机器的链路实测，不能冒称另一台笔记本或全部地区的速度，也没有
核对云供应商带宽套餐。按此速率连续传输全载荷约2.1小时，逐文件串行请求的
等待还会延长耗时。没有并发吞吐或完整首次安装时间验收。

公开 R17 DELIVERY.json 和 CDN client-updates/latest.json 均返回404；现有
R2 Web素材服务不能替代尚未部署的原生下载入口。生产 updater4c60 已支持合包
和差分，但这条下载链路仍回退到游戏服务器逐文件下载。

| 阶段 | 已核对结果 | 尚未完成 |
| --- | --- | --- |
| R17资源合包 | 11块、620,189,573压缩字节；全部125,965目标还原及CMS绑定通过；原源已追加，完整HTTPS字节及本地Windows库cancel/retry通过 | 发行release引擎、实际HTTPS/笔记本首次安装耗时 |
| R16→R17 EXE差分 | 25,424,169字节；真实R16源与真实R17目标SHA/size相符，实际Windows库使用及完整旧版回滚通过 | 发行EXE/HTTPS/正常启动验收 |
| R2发布准备 | 36个不可变对象，CMS与对象闭包通过，原game source6032不变；固定条件创建上传器107项通过；实际两种CI令牌验证401 | 有效授权、账户/桶/路由预检、真实stage、公网逐hash、MISS/HIT/range、独立CAS promote |
| R17→R18隔离更新 | 28,618变化路径+2删除；28,616个唯一内容请求、342,513,399载荷字节；完整新/旧hash、个人文件、pending/quarantine/回滚通过 | R18发布、实际首次启动、CDN/加速器/人类验收 |

下载加速使用独立冻结工具源码4c60。资源合包不能消除100 KB/s链路瓶颈；
R2/CDN实际提速必须同载荷对比，不能由单元测试或压缩率推导。本机CLI尚未
登录Cloudflare；现有Actions凭据是另一个待验证入口，不要求把密钥贴到聊天。
下载文件独立于游戏网关，因此分发侧发布不需要重启游戏或再次编译游戏。
新R18源码321316/209地图配套包保持冻结；P5等后续源码不混入该包。

[原始测量、签名、失败和实际回滚证据](generated/player-qa/native-delivery-20261005/README.md)。
