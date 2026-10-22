# Windows 更新器完整校验性能验收（2026-10-22）

本轮将已安装文件的完整校验改为最多 4 个并行 worker。在本机已缓存的
154,579 个文件上，最终标准版的扫描耗时为 16.664 秒；同一二进制使用
1 worker 为 39.280 秒。本次顺序对照的扫描速度比为 2.357 倍，耗时减少
57.58%。这是本地完整校验阶段的结果，**不是首次安装或整体更新在 5 分钟内完成的验收**。

## 本次慢的实际原因

截图对应本机日志中的 `checking-installed`，不是正在下载。2026-10-22 这次旧引擎
扫描 154,579 文件耗时 596,076 ms（9 分 56 秒），随后 R25 的一个原始资源请求返回
404，更新未完成并回退启动原 R23。失败后没有接受新的 feed，后续尝试仍需检查。
此前 2026-10-10 一次仅 7 文件变化的更新，扫描仍耗时 414,474 ms，后续 staging、
prepare、install 合计约 10 秒。

首次安装还有独立瓶颈：历史一轮约 25 分 43 秒，事务激活就耗时 799,805 ms，
另有下载、解包、暂存及准备阶段。约 1 GB 内容拆成十五万多个小文件，反复打开、
校验、复制、同步写盘的成本显著。本次并行扫描补丁没有重构这些首次安装步骤。
具体历史计时留在 `E:/mir2-player-journey/work/player-acceptance/UPDATER-TIMING-BASELINE-20261022.json`。

## 源码与行为范围

- 最终代码提交：`f979b76775b55ab14d72520ff5884dad56401c0f`。
- 基线为已安装普通玩家渠道引擎的源码
  `b4390ee4c987bbf000ba1d95761151e39fc0d54f`；最终补丁只改
  `apps/game-client/windows-updater/src/update.rs`，新增同目录的
  `installed_scan.rs`、`installed_scan_tests.rs`。不带入五职业专用 feed 变更。
- 使用既有 `local_io::try_run` 和 `local_io::workers`，最多 4 个 worker。
  每个文件仍调用原有 `safe::target`、`safe::matches`，保留路径、普通文件及
  硬链接防护和完整 SHA-256 校验；不以长度、mtime 或缓存记录代替文件哈希。
- worker 返回清单索引与匹配结果，调用线程保持清单顺序生成待修复列表。
  producer、observer 与返回前均检查取消，`try_run` 等待全部 worker 退出后才返回。
  下载、事务安装、签名验证和回滚策略没有改变。

运行边界：游戏 metadata、引擎及已记住的 feed 均未变化且启动校验通过时，既有流程
会直接返回，本来就不逐文件扫描。旧引擎执行自升级时仍执行旧版本的串行扫描；本次
并行优化要在新引擎激活后、实际进入完整校验阶段时才生效，不能倒推缩短此前的升级过程。

## 最终标准版的只读对照

2026-10-22 使用最终源码生成的 release 测试程序，顺序执行 **4 worker → 1 worker**，
没有清理系统文件缓存。此前已经多次读取此安装目录，因此按暖缓存记录；没有受控的
冷缓存测试，也没有随机顺序、多轮统计或另一台电脑的测量。

| 项目 | 4 worker | 1 worker |
| --- | ---: | ---: |
| 清单读取 | 48 ms | 48 ms |
| 清单解析 | 427 ms | 428 ms |
| 完整文件扫描 | 16,664 ms | 39,280 ms |
| 清单结束复核 | 49 ms | 52 ms |
| 测试内部总计 | 17,189 ms | 39,809 ms |
| 已检查文件 | 154,579 | 154,579 |
| 待修复文件 | 0 | 0 |

两次均检查 `D:/Mir2Invite/game`，使用该目录的 `PACKAGE-MANIFEST.json`。
这是本机 **R23** 安装（`WN-CANDIDATE-20261009-invited-23`），不是 R25 首次安装。
清单 payload 总大小为 **986,575,666 字节**。每次开始、结束及两次之间清单 SHA-256 一致：

```text
2012C0163C8EC418ED09ED9D3953337DB1A246BB1B2D3191ABE6B0BA485EB03E
```

两次待修复列表均为空，摘要相同：

```text
E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855
```

基准直接调用生产扫描实现，只读取安装目录与清单，不调用更新安装流程、不下载、不清缓存。
检查开始时游戏客户端进程存在；该进程未被本轮停止。协调方在两次扫描期间不修改安装目录、
不启动更新。这里没有证明进程停用后的隔离测量，也没有证明每个文件都从物理盘读取。

最终测试二进制：

```text
C:/mir2-build/updater-five-minute-20261022-target/release/deps/mir2_windows_updater-f1459493f5bd5754.exe
SHA256 9491EA94549CD2D15FD41D68D235020AF4B74A234AF0604192B63F23B2F36B16
```

此 SHA 在执行前后相同。两次基准各为 `1 passed / 0 failed / 85 filtered out`。
总耗时比为 2.316 倍；不将本次暖缓存数据与用户此前 30–40 分钟的整体更新，或此前不同
运行条件的扫描日志直接相除。早期五职业基线的 88 项测试和 `-01` 基准保留为历史，
不作为本节最终标准渠道结果。

## 测试与签名构建

最终标准源码的完整 crate 检查记录为 **81 passed / 0 failed / 5 ignored**；
独立的 `tests/real_packages.rs` 另有 **2 ignored**。这些 opt-in 项没有因为默认测试
成功而被计为通过。本轮随后用同一最终 release 测试程序显式运行：

```text
platform::tests::cms_math_key_pin_usage_detached_and_renewal_contract
```

结果为 **1 passed / 0 failed / 85 filtered out**。复用
`C:/mir2-build/updater-five-minute-cms-20261022-01` 的临时公钥 CMS fixtures；
覆盖有效签名、同公钥续期、篡改、错误 pin、多签名、attached CMS、SHA-1、EKU、
key usage、错误密钥与认证发布时间约束。此项是密码学契约测试，**不是新发布 feed
的公网验收，也不是生产签名身份的替代证明**。

新增的 7 项普通测试覆盖串并行待修复列表与顺序一致、同长度且还原 mtime 的损坏、
缺失/长度差/空文件、硬链接/非法路径、producer 与 observer 取消、空扫描取消和
非法 worker 数。既有 `local_io` 测试同时覆盖有界 worker、错误和取消后的 join。

本地签名构建已完成，`BUILD-UPDATER.json` 记录代码提交为上述 `f979b767…`、
构建时 `sourceClean: true`，命令为：

```text
cargo +1.95.0 build --locked --release --target x86_64-pc-windows-msvc
```

本地产物位于 `C:/mir2-build/updater-five-minute-signed-20261022-01/`：

- `engine-release/Mir2Updater.exe`：1,971,200 字节，已复核实际文件 SHA-256
  `C8BD0FF9A351E91768464FB27F6D87A1890638CE36CA46166756116604FBA7B8`。
- 构建记录中的 launcher SHA-256：
  `283A1BF6AA44BA4C7587E5174229DFDAC4DE7F5C4D2668878F1127C39125BD2B`。
- 本地 `ENGINE.json` / `ENGINE.p7s` 和 sequence **21** 的 feed 已生成；feed 继续引用
  R25 游戏源码 `d917de1ff4988e2d9fc7ca58f958cb5c80861fdf`。
- `BUILD-UPDATER.json` SHA-256：
  `269F8E3346E2E4C27EF8C755171F5CB4757F34FF23141C76FE8F1E9E07789EA6`。

上述是本地候选构建状态。Detached CMS 不等于 Windows Authenticode；本轮文档不据此
宣称普通玩家渠道已更新、新引擎已激活或玩家安装已完成。

## 真实签名引擎升级与回滚

最终 release 的 `real_package_delta_preserves_personal_files_and_first_launch_rollback`
显式单项执行通过：**1 passed / 0 failed / 1 filtered out**。使用真实签名的 R25 游戏，
由当前 s20 / `9213EC3F…` 引擎更新到本轮 s21 / `C8BD0FF9…` 引擎。

- 新建隔离测试目录，完整复制 R25 后测试，不修改 `D:/Mir2Invite`。
- 本地文件源仅请求 1,971,200 字节的新引擎；游戏 payload 变化及请求数均为 0。
- 154,579 个游戏文件完整哈希复核通过；激活后实际引擎文件及指针均为 `C8BD0FF9…`。
- 模拟首次启动失败后恢复旧引擎，保留个人文件，最高签名序号不降，拒绝重复安装被隔离的 s21。
- 测试墙钟 207.235 秒，其中复制及准备 fixture 117.812 秒，更新阶段完整扫描 34.480 秒。
  该测试使用本地文件源且没有启动游戏 EXE，不代表网络下载、首次安装或 GUI 性能验收。

证据目录：`C:/mir2-build/updater-five-minute-engine-integration-20261022-01/`。

## R25 缺失资源与未完成验收

本轮发现 R25 原文件
`mir2-assets/generated/native-map-keyed/resource-input-provenance.json`
请求 404。其原清单绑定为 **33,779 字节**、SHA-256：

```text
792A09CD973F24EFD8614AF667909BCE8FF37D2657D873C8CC8F8A3CB13D4EFE
```

该文件已经在原 `DELIVERY.json` 的 bundle 中，但原 40 对象发布计划不包含嵌套资源原件。
更新器的小规模修复会跳过大 bundle 并请求单文件，因此性能补丁不能自行解决这个 404。
2026-10-22 18:04（Asia/Shanghai）已将上述精确原件补齐到源站和 CDN。两端公开 GET 均为
HTTP 200、33,779 字节、上述完整 SHA-256；两端当前 feed 仍为 sequence 20，字节及签名
未被此补发变更。源站采用发布锁和原子不覆盖追加，补发前后受保护文件及服务进程未变。
CDN 通过已认证的 Cloudflare R2 API 在确认目标缺失后上传，再从公开地址完整读回核验。
这关闭了本次已观察到的资源 404，不代表玩家更新事务或新引擎部署已经完成。
本机回执位于 `E:/mir2-player-journey/work/player-acceptance/r25-missing-resource-20261022/`：
`apply-recovery01.stdout.json`、`cdn-api-upload-receipt.json` 和
`public-get-after-origin-and-cdn-repair.json`。
一般性的发布闭包与单文件 fallback 缺口仅记录，本轮不修改发布脚本。

仍未验收：冷缓存扫描、首次安装全链路、下载/解包/落盘/事务激活总时长、正式渠道
sequence 21 发布与自动升级、最终新引擎在玩家机器上的 GUI 验收。
**“尽量 5 分钟内完成安装更新”目标仍开放**；本次确定关闭的是最终标准源码的并行完整
校验回归及本机暖缓存阶段测量。

## 本机证据与复跑入口

下列日志保留在 `C:/mir2-build/`，本文列出原件名称与 SHA-256；日志本身未随本文件提交。

| 日志 | SHA-256 |
| --- | --- |
| `updater-five-minute-bench4-standard-20261022-03.log` | `fce02bf25a990cd98c532d010cc704e39af30836e4dc3dcc1c78a843d89c0053` |
| `updater-five-minute-bench1-standard-20261022-03.log` | `64e71f0433cf88952c60d6387bd7cb6556fc8699ac5da191060a4d75f03c4aba` |
| `updater-five-minute-cms-standard-20261022-03.log` | `0e8259e17ad7cb7ac09520f8f2ef08d9e54a71d6da716f5129d971db98d00183` |
| `updater-five-minute-tests-standard-20261022-02.log` | `8ad841fc1b9b578541739e392ef35ba7d5ffcf3f1ff487777a071dda282f722c` |
| `updater-five-minute-release-standard-20261022-02.log` | `5c98988cfec6cc7009a8a461f02980322153850635d638e0745ad686f5a6ba39` |
| `updater-five-minute-signed-build-20261022-01.log` | `54a0f91039c95059cf20b1635b4005a9a3a17730f7aed83836dd21d3395474b2` |

只读基准的显式入口如下。将 worker 值设为 `4` 或 `1`，顺序执行，不并发争用磁盘；
安装目录与清单应在测量期间保持稳定。测试不负责阻止外部进程写入。

```powershell
$env:MIR2_UPDATER_BENCH_GAME_DIR = 'D:/Mir2Invite/game'
$env:MIR2_UPDATER_BENCH_MANIFEST = 'D:/Mir2Invite/game/PACKAGE-MANIFEST.json'
$env:MIR2_UPDATER_BENCH_WORKERS = '4'
& 'C:/mir2-build/updater-five-minute-20261022-target/release/deps/mir2_windows_updater-f1459493f5bd5754.exe' --ignored --exact update::installed_scan::tests::readonly_installed_scan_benchmark --nocapture --test-threads=1
```

CMS 显式入口：

```powershell
$env:MIR2_UPDATER_CRYPTO_FIXTURES = 'C:/mir2-build/updater-five-minute-cms-20261022-01'
& 'C:/mir2-build/updater-five-minute-20261022-target/release/deps/mir2_windows_updater-f1459493f5bd5754.exe' --ignored --exact platform::tests::cms_math_key_pin_usage_detached_and_renewal_contract --nocapture --test-threads=1
```
