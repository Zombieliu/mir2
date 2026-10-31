# Windows 新版启动器、可选更新与品牌图标（2026-10-30）

用户反馈：双击后直接更新，无法判断何时能启动，失败后才能尝试游戏；桌面缺少正确品牌图标。用户明确选择 **自动检查版本，点击后才下载更新**。

根据用户随后对界面和强制更新的反馈，已重做为黑金武侠主视觉，并将直接启动已验证的本地游戏作为首页主操作。用户要求交付后，已构建、验证并发布包含完整 unified-11 游戏资源的新安装包；实际从安装后的启动器进入简体中文登录界面，用户也已确认。离线预览中的 r42、80 MiB 和 HTTP 503 均为测试数据；预览的「开始游戏」仅关闭示例，不用于玩家交付。

## 玩家交付（2026-10-30）

- [下载完整 Windows 试玩安装包](https://165.154.65.136.sslip.io/client-updates/installers/3b926d5eb5dd8ee1fc46b6d39fba3eda3eb538ba257405733abe1defae840172/Mir2Setup.exe)：安装器 `2026.10.30.12`，`1,026,815,762` bytes，约 1.03 GB；[玩家使用说明](https://165.154.65.136.sslip.io/client-updates/installers/3b926d5eb5dd8ee1fc46b6d39fba3eda3eb538ba257405733abe1defae840172/README.zh-CN.txt)、[SHA256](https://165.154.65.136.sslip.io/client-updates/installers/3b926d5eb5dd8ee1fc46b6d39fba3eda3eb538ba257405733abe1defae840172/SHA256SUMS.txt)。
- 安装器 SHA256：`3B926D5EB5DD8EE1FC46B6D39FBA3EDA3EB538BA257405733ABE1DEFAE840172`。现有内部 CMS 校验覆盖游戏包、updater bundle 和发布描述符；公共发行者 Authenticode 尚为 `NotSigned`，不能将 CMS 等同于公共代码签名。
- 交付源码为独立干净 worktree 的 `codex/windows-launcher-delivery-v2`，commit `677e2ab56704660c4e93c14a1d29945f82b55f3e`，基于已发行的 `77a2007a449d1f437a8d411bfa41f105be992fcf` 集成。保留该基线的十语言、Unified 维护范围、packed/legacy 最低序列及安装器合同。主工作区其他未提交工作没有混入交付。
- 游戏继续使用签名 Candidate `WN-CANDIDATE-20261029-unified-11`，游戏源码 `1875c6c20e1285ffefc45ef6c21eab33e26fc536`，未重编译游戏。新的 engine sequence 为 `2026103001`。
- 新玩家运行安装器并使用桌面图标；老玩家保存并退出游戏后覆盖安装，保留自动识别的原目录。安装包含完整游戏资源，启动器只自动检查版本，下载须点击「更新」。本地版本验证通过时，检查失败仍可「开始游戏」。
- 以内容哈希目录发布新增安装包及四个公开说明/签名文件；原有三条 update feed、Caddy 配置、服务和游戏数据未修改。公网 HTTPS HEAD、首尾 Range 与四个 companion 文件回读通过；服务器原子发布前对完整安装器验 SHA256，未用局部 Range 冒充整包下载验证。

本次交付的原始记录只归档于[交付证据目录](evidence/windows-launcher-delivery-20261030/evidence-index.json)。验证范围如下，不代表项目整体 100% 或人类前端 Accepted。

| 检查 | 新鲜结果 |
| --- | --- |
| 集成源码 `cargo +1.95.0 test --locked` | 库 153 passed / 8 ignored，Updater binary 4 passed，integration 4 ignored；合计 157 passed / 12 ignored；[日志](evidence/windows-launcher-delivery-20261030/updater-integrated-tests.log) |
| 打包与安装器回归 | packed preparation 13、bootstrap installer 80、unified compatibility 3 通过；CMS 原生合同单独重新执行 1 项通过；日志见证据目录 |
| 游戏完整性 | 实际重新验证 255,079 个文件，签名、manifest、源码证明与游戏 EXE 通过；[报告](evidence/windows-launcher-delivery-20261030/game-candidate-verification.json) |
| 完整安装器及真实覆盖升级 | r11 → r12 实际安装通过；未传 `/DIR` 或 `/LANG`，仍记住原目录和中文；设置与个人文件保留，游戏 EXE 不变；独占锁时 exit 7 拒绝覆盖；[报告](evidence/windows-launcher-delivery-20261030/upgrade-qa-result.json) |
| 安装后的真实启动 | 新签名 bundle 的 Launcher → engine → game 进程链通过；更新检查不可用时点击「开始游戏」仍到达中文登录界面，用户确认；[报告](evidence/windows-launcher-delivery-20261030/installed-launcher-smoke.json)、[首页](evidence/windows-launcher-delivery-20261030/installed-launcher-home.png)、[登录界面](evidence/windows-launcher-delivery-20261030/installed-game-login.png) |
| 用户既有安装保护 | 20 个既有文件/偏好及原普通安装、原验证安装注册项前后相同，原玩家游戏进程保留；见真实启动与升级报告 |
| 公开下载与签名 | 内容地址新增发布、完整 origin SHA256、TLS、长度、Range、四文件回读、回读描述符 CMS 与三 feed 不变均通过；[发布回执](evidence/windows-launcher-delivery-20261030/installer-publication.json)、[公网回读](evidence/windows-launcher-delivery-20261030/public-download-verification.json)、[CMS](evidence/windows-launcher-delivery-20261030/public-readback-cms.json) |

未操作账号认证或验收在线玩法；本轮没有发布强制更新政策，也没有变更现有更新通道。下文保留交付前的本地开发记录，其测试数量和入口不可代替上述最终安装包证据。

### 交付后的下载速度复查

用户随后质疑源站直下速度，实际重新请求此成品前 8 MiB：116.324 秒，约 0.072 MB/s；若全包始终同速，约 237 分钟。此为当前 Windows 机器的单条线路样本，不代表每个玩家，也不是整包完整下载测试。[测速记录](evidence/windows-launcher-delivery-20261030/origin-download-speed.json)。交付时已验证可下载及完整性，但当时未做速度验收；当前源站链接不宜作为主要玩家下载入口。

既有 R2 publisher/原生下载通道的本地合同将 artifact 限制为 128 MiB；本次约 1.03 GB 成品不能直接沿用该上传计划。Cloudflare 常规套餐的单文件可缓存上限为 512 MB，参见[当前官方说明](https://developers.cloudflare.com/cache/interaction-cloudflare-products/r2/)。后续加速应单独验证分块缓存和续传分发，保留服务器为备用；本次复查只读测速，尚未变更 R2、Worker、路由或更新通道，也未取得该新成品的 CDN 测速结果。

## 行为

- 启动器先完成本地恢复与签名版本验证，验证通过后首页立即开放「开始游戏」。后台独立检查可选更新，超时或失败不阻挡本地启动；后台线程不持有安装写锁或游戏租约，离开首页无需等待阻塞中的 HTTP 线程。
- 首页「查看更新」进入签名计划；存在可验证安装时，计划页仍提供「先玩目前版本」。跳过更新会先退出更新调用栈并释放锁，再重新验证和启动本地版本。
- Legacy、packed generation 和引擎维护都在必要的签名/绑定检查后计算计划，等待玩家点击「更新」，再下载 payload 或激活。可选大体积 DELIVERY 描述符也在确认之后请求。
- 相同且完整的已安装版本不重复要求下载确认。尚未激活、但数据已缓存的新版本仍需确认；预计资料量可以为 0。
- 检查和本地处理显示工作已用时间，首页与确认页等待点击时不计时；既有本地文件阶段保留完成数量。下载显示新增网络字节、速度和当前下载的估时；首字节之前和传输停滞时显示等待，避免假造整体完成时间。
- 计划量显示为「预计更新资料」，明确实际网络量受缓存、传输包和重试影响；它不是整体网络硬上限。当前下载阶段的大小可以是保守上界。
- 完成后停在「已准备就绪 / 开始游戏」，等待玩家再次点击。
- 更新失败后保留重试入口；仅已安装版本仍满足既有验证时，提供尝试启动该版本，并保留重试按钮。不可验证或损坏的安装不开放启动；packed 损坏不可变对象沿用 fail-closed，需要可信安装器修复。
- UI 初始化失败单独报错。启动前由 UI 线程串行检查取消并关闭窗口，再允许写启动义务及创建进程，避免玩家点击取消之后继续启动。提交启动之后的签名回退、首次启动健康判断与数据保留合同继续沿用。
- 既有显式 `--check-only` 工具入口保持自动更新且不启动游戏，普通桌面入口不使用该参数。

## 图标与构建

主视觉为本轮生成的原创插画，文字、状态、进度和按钮均由原生 Win32/GDI 绘制。素材：[launcher-hero-v2.png](../apps/game-client/windows-updater/assets/launcher-hero-v2.png)；精确生成方法及提示词：[launcher-hero-v2.prompt.txt](../apps/game-client/windows-updater/assets/launcher-hero-v2.prompt.txt)。PNG 解码有尺寸及分配上限，界面采用双缓冲并限制下载刷新频率；九语继续使用系统字体与 DPI 缩放。

复用原生游戏已有 Numeron ICO，包含 16/24/32/48/64/256 六档。Launcher、Updater、离线预览 EXE 使用资源组 ID 1；窗口、任务栏、桌面快捷方式、安装器统一引用对应品牌资源。原生游戏已有图标机制保持。

安装器支持 `/DMir2IconFile=<path>` 以适配外部 staging；缺失资源时构建/安装器预处理显式失败。发行脚本和 packed source snapshot 把新 RC、PNG 与共享 ICO 纳入受检编译输入，覆盖忽略文件与 assume-unchanged 的隐藏修改。

安装在玩家机器上的旧固定 bootstrap 不会被引擎更新自动替换，因此本次通过新的完整安装器交付共享 ICO 和 Launcher；上文已列实际成品和真实升级证据。

## 交付前的本地开发验证（历史）

新版原始结果集中在 [v2 证据目录](evidence/windows-launcher-v2-20261030/)，不累计为项目整体完成率。第一轮白色界面截图保存在 [历史证据目录](evidence/windows-launcher-20261030/)，不能作为新版视觉验收。

| 检查 | 结果与范围 |
| --- | --- |
| `cargo +1.95.0 test --locked`，独立 updater crate | 库 136 passed / 6 ignored；Updater binary 4 passed；Launcher binary 0 tests。四个依赖真实签名包的 integration test 为 ignored；[日志](evidence/windows-launcher-v2-20261030/cargo-test.txt) |
| 公开 CMS fixtures，显式重新运行原生 CMS 合同测试 | 1 passed；无私钥导出或发行签署；[日志](evidence/windows-launcher-v2-20261030/cms-test.txt) |
| `test-prepare-packed-generation.py` | 13 passed，包含 RC/PNG/ICO 忽略输入和隐藏修改拒绝；[日志](evidence/windows-launcher-v2-20261030/packaging-tests.txt) |
| `test-bootstrap-installer.py` | 77 passed；[日志](evidence/windows-launcher-v2-20261030/installer-tests.txt) |
| 新 Launcher / Updater / preview 构建及 PE 图标检查 | 六档图标 payload 与共享 ICO 一致；[报告](evidence/windows-launcher-v2-20261030/icon-resources.json) |
| Inno 图标输入 | repository 默认与外部 staging 覆写预处理通过；最小 installer fixture 编译通过；缺图标显式失败。仅 fixture，不是完整发行安装包；[报告](evidence/windows-launcher-20261030/installer-icon-resources.json) |
| 后台检查及原生窗口逻辑 | 实际 HTTP fixture 挂起时仍能取得安装锁/游戏租约；无效或缺失 metadata 不下载 payload、不修改选择；Home/Choice 区分本地启动与下载授权，后台克隆退出及迟到结果不关闭窗口；九语组合文案在 100% / 150% / 200% DPI 的原生文本测量通过 |
| 原生窗口实际操作 | 使用 computer-use 打开真实安装的首页，发现可选 r2026102912 后点击 Start game，实际启动 unified-11 到登录画面；[EXE 哈希与结果](evidence/windows-launcher-v2-20261030/real-game-result.json)、[进程](evidence/windows-launcher-v2-20261030/real-game-process.json) |

### 窗口证据

- [真实首页：发现可选更新，开始游戏仍可用](evidence/windows-launcher-v2-20261030/real-home.png)，[原生控件状态](evidence/windows-launcher-v2-20261030/real-home.txt)
- [实际游戏登录画面](evidence/windows-launcher-v2-20261030/real-game-login.png)

## 交付前的本机开发入口（历史）

本机现有 `F:\mir2-build\unified-20261029\installed-validation` 安装包含签名 unified-11 游戏。将本轮 debug Updater 按 SHA256 复制为 `.update/local-launcher-83A6CD7ED9C9DB478DE15FA3DFBD35AC86BAAD40396EACD2095C36C5E2D8E934.exe`，仅作为本地开发入口使用；保留本地签名、EXE 哈希、安装锁、恢复及首次启动健康检查。没有替换现有 Launcher、签名引擎描述符、固定 bootstrap 或游戏包。复制及哈希见[记录](evidence/windows-launcher-v2-20261030/local-game-launch.json)。

新增独立桌面快捷方式 `Numeron - 新版启动器（本地）.lnk`，指向上述新入口并使用其内嵌 ICO；原有快捷方式保留。详见[快捷方式记录](evidence/windows-launcher-v2-20261030/local-shortcut.json)。已观察到真实登录画面，未操作账号认证。

## 本地开发阶段的验证边界（历史）

上述本地开发阶段没有执行 ignored integration test 或真实安装器覆盖升级；后续完整交付已执行的升级、发布和窗口验证以上方「玩家交付」为准。自动登录、在线玩法和玩家前端 Accepted 仍未执行。服务器是否接受旧客户端继续按既有登录协议处理。
