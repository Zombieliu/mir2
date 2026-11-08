# Numeron R16 Web 发布候选

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

- 冻结源码、推送，更新草稿target，固定资产CI真实公开读回与最后manifest发布。
- Ubuntu24/Rust1.95独立Gateway包、安全/PG/地图门，以及现TEST旧库克隆、普通新号与保存回读。
- 新Game与营销同源production候选构建、真实页面/版本/PWA/下载检查；候选普通登录、素材、Q窗口与保存。
- 排空及一致备份TEST，保留keys/计费与旧数据，追加精确新origin，不触旧正式服。
- 用户域名最后切换，公开`/game`及真实WebSocket复验；失败只回退仍由本次候选持有的映射，不能覆盖他人并发版本。

当前没有将上述待执行项勾为完成。域名move仍独立于已实现的stage/promote脚本，必须在候选验证后执行。
