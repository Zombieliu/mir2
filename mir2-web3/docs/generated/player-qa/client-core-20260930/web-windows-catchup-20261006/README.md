# Web / Windows 固定基线追赶 QA

比较基线为 `f72e36fb84c3574fff0aeb2abed856454b14289d`。本页记录当前有限代码检查与 build-only / 静态包证据；没有运行服务、HTTP、WASM instance、浏览器、原生客户端、账号、存档或玩家流程。退出 0 和静态闭包均不代表可玩或 Candidate 接受。

Web 静态包根目录：[`.mir2-thin-client-web-windows-catchup-20261006-02`](E:/mir2-player-journey/mir2-web3/apps/web/.mir2-thin-client-web-windows-catchup-20261006-02)。Native09 归档 EXE：[mir2-platform-windows.exe](C:/mir2-cross-platform-storage-20261002/repo-qa-web-windows-catchup-20261006-01/w5-native-build09-mir2-platform-windows.exe)（104,694,784 B，SHA `0ab54f647ee618a35b067fafe08e80f4d04df1d47694b323b6c13ff1aaddbd39`，未启动）。

## 当前源与有限检查

- Source09 冻结 405 输入，SHA `a6df3325ae47cada0aacb6245f8175db701e00a187f3b50f94a924b20dde19ae`；八文件审查 SHA `3b4630764cf180aa644a380e4b780c59aa4c08129a072d863933aeba9044e691`，0 P0/P1。12 项新增与 2 项保留 Rust 测试通过。`ChangeAttackmode` / `ChangePetmode` 属 runtime Web host（输出 `changeAMode` / `changePMode`）；共享 Quest getter 为 `getMir2QuestNameTargets`，HUD 接口为 `hpView` / `HPView`。
- TSC02 `c73c53` exit 0；Stage5-04 `a06b31` exit 0，260 组 / 0 fail；Adjacent04 `802873` 216/216/0（含 Bag 3 组 / 14 cases）；Cross05 `81f434` 114/114/0。Next02 `956ce8` exit 0，builder 83012 closed/disposed，65,268 ms，严格 TypeScript 与 13 个静态页成功。Native09 `2ff6f3` 与 renderer07 `008073` build-only 退出 0，guard/builder 均 closed/disposed。Cross04 `113/114/1` 夹具失败及 Cross03、Adjacent03 更早失败均保留；Cross05 后续通过，未删除原断言。
- QA 收据：[Native09](C:/mir2-cross-platform-storage-20261002/repo-qa-web-windows-catchup-20261006-01/w5-native-build09-result.json)、[renderer07](C:/mir2-cross-platform-storage-20261002/repo-qa-web-windows-catchup-20261006-01/w5-renderer-build07-result.json)、[Next02](C:/mir2-cross-platform-storage-20261002/repo-qa-web-windows-catchup-20261006-01/w5-next-build02-result.json)、[Stage5-04](C:/mir2-cross-platform-storage-20261002/repo-qa-web-windows-catchup-20261006-01/w5-final-stage5-04-result.json)、[Adjacent04](C:/mir2-cross-platform-storage-20261002/repo-qa-web-windows-catchup-20261006-01/w5-final-adjacent04-result.json)、[Cross05](C:/mir2-cross-platform-storage-20261002/repo-qa-web-windows-catchup-20261006-01/w5-final-cross-page05-result.json).
- Next02 standalone 输出：19,006 个普通文件共 589,834,498 B，完整 hash manifest SHA `5d952d709e20452160d99f874f5f0bb3a192e371c2526f6138af780430b25046`；61 个 NFT trace / 37,094 路径，16,712 个附加资源与 standalone mirror 字节一致，2 个 node_modules 目录指向冻结的 24,891 项依赖根。required-server-files 中序列化的 Core / renderer manifest 与源一致，client chunk 的两版标识匹配。Next02 result SHA `1ba6326ef29b9e74365f841771b84b219d10328d22094506000f19d7c21035b5`。
- renderer07 Bevy `e31f4cb651a7b4ef`：GPU 30,866,951 B / gzip 5,997,202 / JS 133,785；lean 17,751,071 / 4,226,708 / 116,615；shared 31,829,091 / 6,338,599 / 131,759。均低于原 cap 32,505,856 / 7,340,032 / 204,800。9 exports 只扫描 WASM section 7 名称，未调用 WASM API。

## Thin03 静态包审计已闭合

Thin03 chunk `82cd65` exit 0；builder 90244 closed/disposed，19,279 ms。包内 7,299 文件、775 目录、0 links，总计 372,474,859 B（355.2197065 MiB），低于原 cap 377,487,360 B，余量 5,012,501 B。最终 SHA-256 清单覆盖全部文件。3366 个 public 文件与源字节相同或属于下述 JSON 空白压缩；79 张 atlas PNG 与源文件及 manifest 字节匹配。231 个压缩 JSON 文件已逐项验证 token、数字、字符串原拼写不变。Core 2 leaves、renderer 6 leaves、immutable/current flat manifests 均相同；打包 required-server-files 与 Next02 原件 hash 相同；所选 Win32-x64 依赖文件已打包，zero links；条件模块解析与服务启动仍待验证。

Thin result SHA `870c145a1e354108cbdebf17c6525f5e53058bf038485dede70a2fbc70d5cdfc`；独立 closure SHA `4f90e8a28b26751087d39146b93b47f52a5a4564269bdab732c1e45eeab09657`；7,299 文件 hash manifest SHA `b173eabbf76ef84c74d85649f49f93775b49042e4b077ea2bc369bf5f87bee78`。

[Thin03 result](C:/mir2-cross-platform-storage-20261002/repo-qa-web-windows-catchup-20261006-01/w5-thin-build03-result.json) · [Static closure](C:/mir2-cross-platform-storage-20261002/repo-qa-web-windows-catchup-20261006-01/w5-thin-build03-static-closure.json) · [Output hashes](C:/mir2-cross-platform-storage-20261002/repo-qa-web-windows-catchup-20261006-01/w5-thin-build03-output-hashes.json) · [Size report](C:/mir2-cross-platform-storage-20261002/repo-qa-web-windows-catchup-20261006-01/w5-thin-build03-size-report.json)

## 明确未验收范围

Thin size report 有 44 个 dependency warning records，代码为 `Error`，虽然 build stderr 为空；该计数仍需解释，不能忽略。没有启动 package server，所以 service startup 和远端资源 HTTP 覆盖未验证。省略的原客户端媒体要求通过 `MIR2_R2_PROXY_BASE` 从 immutable asset origin 走 same-origin miss proxy；源 manifest 有 Mount 条目，但没有 Pet / Gate 的完整远端库存，不能声称其资源覆盖完整。

W4b 七项、F10/Hero/双 SkillBar 的代码候选及其对应 TSC / finite / Next 检查已通过；实际 UI 行为、登录 / 游戏、保存 / 重登、服务 ACK 和两端玩家比较仍未执行。用户暂缓 UI。F11 原 44 项分母未冻结；goal active，不报整体百分比、100% Candidate 或 goal complete。