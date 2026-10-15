# 共享邮件列表与阅读器：M13

2026-10-03，Root 已接受 Source08 的共享代码、自动回归、本地 Windows/WASM/Next 构建和修正后的独立包技术验证。用户要求继续代码、暂不操作界面；没有启动 EXE、浏览器或图形场景。本记录不代表实际游玩或整个 Candidate 已完成。

邮件列表与信件/包裹阅读器共用 Rust painter。Web 通过当前身份、模型和画面 revision 保持唯一输入 owner；撤回收到确认后才能回退到旧 DOM。写信编辑器、IME、邮票、附件、费用及发送流程仍是下一批共享工作。

Source08 共455文件，相对448原文件改动19个、新增7个，保护429个。八项实际 Rust 验证有175次通过执行（不是175个唯一测试）：runtime53、实际 JS/Rust 邮件夹具1、Native common95、portable26；原先三项 ignored 中邮件夹具另行实际执行，其余没有冒充执行。Source04 的22个 Mail与118个相邻 Web 测试、TypeScript退出0，以133个前后相同的 Web 文件沿用；没有声称重新生成 Source08 Node 夹具。

三种当前 canonical WASM 的纯数据 resolver 都实际消费原始 Node 产生的21组完整捕获：63组完整解析输出一致，15组错误输入拒绝。初始化3个数据模块，未调用 boot/图形 getter；前后1054项输入一致。原生新 EXE 为103,921,664字节，SHA-256 `84f8d64a894e044daa4817e7851c8f76182806853677167926055508a046b631`，构建成功，未启动。三个 canonical WASM、46个实际编译导出与 fresh Next构建均通过。当前 runtime为 `bevy-b070e88b10ce12c4`；shared WASM为32,499,664字节，比固定32,505,856上限少6,192字节。完整 gzip/JS预算由运行时验收记录绑定。

首次实际独立包构建退出1，380,426,871字节超过固定360 MiB上限2,939,511字节。失败记录保留。只对组装包内 `crystal_respawn_manifest.json` 和 `localization_bundle.json` 的字符串外 JSON 空白做无损压缩，所有原始 token与完整解析值一致；实际压缩和重新统计命令均退出0。结果为377,468,964字节，余18,396字节。全部7,301文件与775个嵌套目录（含4个空目录；连根目录776个）已记录，零链接；仅两个文件改变，7,299个保持原字节。原始两个载体备份保留，八个运行时文件、21个 Sharp文件及两个 WebP保持原字节。不能把后续成功改写成首次构建退出0。

两份完整隔离副本各自通过完整路径/哈希校验。实际 HTTP 共18个响应：14个完整 JSON与4个完整 WebP字节一致。0、D011有实际地图 cell；HTTP缺省中心与显式(8,8)完全一致，显式 manifest中心与缺省响应不同，按当前行为保留。字面N16返回诊断200，但manifest中不存在此地图、cells为空，不代表N16公共任务内容已验收。直接调用当前编译模块7266.cK，共8次调用，0=(350,200)、D011=(200,200)的null-center fallback与显式中心完整蓝图一致。两个HTTP消费者各实际读取respawn载体21次，两个direct探针各13次，其中每次null-center调用自身有3次真实读记录；未mock返回值。

HTTP之后，Root再次独立读取全部1,105物理输入与原包/两份副本的所有文件，均与授权记录一致。1,096个源码/运行时/Next guard行闭合；22项已知 Next/TypeScript工具元数据链接显式映射到相同物理文件，没有为产品源码或运行时开启别名。四个受控服务/探针均实际spawn、exit0、close0；两个Root外层执行器Exited/Disposed且退出0。Root CLI随后确认六个相关PID不存在、22741/22742没有监听。所有证据均为本地CLI/HTTP/数据消费者证据。

仍开放：真实客户端画面与输入、登录—游玩—在线read/claim/delete/lock ACK—保存—重登、公共资源、移动浏览器及Android/iOS真机与生命周期、共享compose/附件/发送、最终Candidate及总goal。原先失败的Source05/06/07超限canonical构建、Source04测试编译101与零执行、纯resolver03漏pin失败等记录保留；本轮通过不覆盖历史失败。

证据：[Source验收](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-inbox-01/root-source-acceptance04.json)、[Windows构建](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-inbox-01/build-retry04/root-native-build-acceptance01.json)、[运行时](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-inbox-01/root-reviewed-runtime-pin01.json)、[纯resolver实际验收](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-inbox-01/root-pure-resolver-acceptance01.json)、[Next](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-inbox-01/root-next-build-acceptance01.json)、[最终包/HTTP](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-inbox-01/root-reviewed-package01.json)、[完整HTTP原始结果](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-inbox-01/package-http02/http/http-result.json)。最终包验收为8,622字节，SHA-256 `f1a176cdf85ccd0ec15311cd8dbc24b3f05ae9aad3e384c108e8293f8ab0fbcc`。
