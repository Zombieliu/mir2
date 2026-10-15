# 普通 NPC 商店组合构建记录

Progress67，2026-10-05 UTC。Root接受本轮冻结源码的Windows开发EXE、生产Core、三renderer、Next及独立包的实际有限构建；各阶段独立复核均无确认blocker。用户“继续代码，暂不操作界面”持续有效。

实际 Windows EXE 为104602112B，SHA-256 `c50284e12d3c2a110beeab2fe41e82dee83ff98225d5e04edc233985d2e9538d`；profile 为 dev optimized + debuginfo，未启动，不称 release。静态导出测试实际26/26，保留原Storage16并新增NPC10。当前 verifier/test SHA 分别为 `09cab6f4b4c50c28f34bd57e8fa9331276c462c0527c9ab37e3cc9bd95220ccd` / `25ae9fc5841e8054cd19ede8f98c39cb9465b68c4d933b19a76fa7d418c95ffd`。

生产Core本轮真实执行 builder、bindgen、Binaryen131 `-O1 --strip-debug` 和内置verify，24个完整输入含18个Rust文件；不是复制QA产物。版本 `9191e74ce0905e56e549bcf6af40dd0ea9f29eb3aa1e6e6523e6154c04c7ccdd`，WASM258772B <262144B，JS19003B ≤204800B。Next/Thin的 `freshCoreBuildExecuted:false` 仅说明这两个阶段沿用已接受Core，不表示整轮没有构建Core。

三种生产renderer版本 `bevy-819bf5ec747151ce`，均实际优化，预算未提高：

| 包 | WASM B | gzip B | JS B |
| --- | ---: | ---: | ---: |
| webgpu | 30538266 | 5896422 | 127817 |
| webgl2 | 21957468 | 5380978 | 125791 |
| webgl2-shared | 31500980 | 6239903 | 125791 |

原上限依次为32505856 / 7340032 / 204800B。三对生产文件各检查Storage7与NPC7出口，包内三对再检查；仅静态JS AST与WASM Module元数据，没有Instance、能力值求值或优化语义等价证明。Canonical四次真实Cargo各有独立nonce、新鲜C盘采样，最低171018870784B ≥50GiB，全部Policy B completed/exited/disposed、exit0；Native/Core另各一次同守卫Cargo，严格串行。

Next实际outer/child exit-close0，webpack56s、严格TSC10.1s、13个静态页。编译清单绑定当前Core/Bevy；仅tsconfig新增两项include、next-env替换单一routes token，tsbuildinfo与next.config原字节不变。库存实际有限CLI哈希43897文件（Next19006、依赖24891），前后元数据及成员稳定，18687ms；依赖库存与旧件逐字节相同。此前REPL超时保留为非正面记录，不算成功。

Thin实际 `--skipBuild`，无额外Next/Cargo。outer0，child43832 spawn→exit→close0，stderr空且无IO/postErrors；stdout与尺寸报告字节相同。独立包377152588B（359.68MiB），7299文件／775目录／0链接，原377487360B预算余334772B。仅复制当前Core两叶和Bevy当前七叶及flat清单，未启动。

源图553：Core发布后552未变+唯一Core指针更新；canonical/Next/Thin阶段553均未变，Native812 Rust沿用Progress65源码。参考90、runtime105→107→114、Next元数据4、精确别名22；Next/Thin各805前后护栏。旧Core三版本六叶保留并增当前两叶；旧Bevy92目录159叶全部保留，增当前七叶后96目录166叶。本轮未重跑Progress66 Node249或Rust游戏测试。

主要证据在外部QA根 `C:/mir2-cross-platform-storage-20261002/repo-qa-shared-npc-shop-combined-build-01`；以下为实际文件bytes / SHA-256：

- `root-native-artifact01.json`：2657B / `6b61ffa8eb8f93037fab5753e1f37d76c00aa36e9aaf3f29449e945571da8235`。
- `exports01.result.json`：1499B / `5b64ffb7ce41999b77a2991c4711e286690b65b696210ecbcfc60ae8f2ab0507`。
- `root-production-core-acceptance01.json`：6076B / `158cab04e7458b2f5169af68cca026998d0ede7d9e427fa279b54acbcb09bbef`。
- `core-production-build01/core-build-result01.json`：44796B / `af71b58a275e37d17274873e72b32b572bed1fde42370948127f11be672fc1d8`。
- `root-canonical-production-acceptance01.json`：20809B / `e971c382add95b9e2f11f33c89e886682f51dd6b9458fe74bf65db296e1a06f7`。
- `canonical-build01/canonical-build-result01.json`：67262B / `a123f34c94fc4e9d55619743144b03bd37ac537b5e1e3e7de42722b3568529f8`。
- `next-production-build01/next-build-result01.json`：14373B / `1263e5a9e98b0a08728588914ab3a3b575dc6f08a7c35e6a4d892d418b99557f`。
- `thin-production-package01/thin-package-result01.json`：2942026B / `c4f69ce102aaf3d89a04a8ab50365308ccf6f450cfcbe7da624487afade11883`。
- `thin-production-package01/thin-client-size01.json`：6260B / `833519e835849ce493e2437460032c839faf332f48f155d2b601ca4e63954673`。
- `root-inventory-repl-timeout01.json`：517B / `7a98035bfa52b7ad7ccd88c9234aeae47e60848f641297316f79b0a12ed30c2f`。

- `root-next-artifact-inventory02/root-actual-inventory-verification01.json`：3271B / `713556398dd41692293fe31259817b33f84704584bae9b3bc278c1d4c0e60489`。

新产物有限构建通过不等于部署、远端Mount/Pet/Gate媒体覆盖、release:doctor、HTTP/启动/smoke、浏览器/原生玩家、触屏/移动真机、frontend/Candidate或整体goal接受。无UI操作；登录、战斗、保存重登及无专用Buy ACK的结果恢复仍开放。

下一代码叶优先修复已确认服务端普通Trade+Gold容量／metadata插入与扣款原子性P1；随后共同planner当前只接受合法空格的规则需补满包兼容stack，服务端修复不等于端到端闭合。完整Sell/Repair/BuyBack/Used/Pearl等仍open。

Thin独立复核逐项读取并核验7299文件、377152588B及775目录，第二次成员检查无变化；805输入guard前后相同。Root有限打包接受记录：`root-thin-production-acceptance01.json`，6556B，SHA-256 `1deec66d2b4742d278bdb1a692a02ec3fd3b7e073333c522901e7a66e8413318`。静态Module元数据不证明实际客户端运行；整体goal保持active。
