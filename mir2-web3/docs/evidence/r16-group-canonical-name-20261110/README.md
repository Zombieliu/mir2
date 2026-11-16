# R16 组队邀请的角色名回执

本机共享 Zone 使用不区分 ASCII 大小写的角色名查找。玩家手输 `bLaDe` 邀请 `Blade` 时，普通请求保留原拼写，服务端成功回执和名单使用角色原名。旧版 Web 和共用 Bevy 各以原拼写匹配回执，可能留下已成功的待处理邀请。

两端只在权威 `AddMember` 或完整 `GroupMemberInfo` 匹配时解除该次操作。名称比较沿用 Gateway 的 ASCII 规则。其他角色、其他会话、移除回包和聊天均不能完成该邀请。固定 Crystal 走原系统消息或静默限流，拒绝后的手动重试继续单独验证。

Web 新用例在原源码复现，修复后限定组队测试 22 项通过，连同社交预算和动作适配共 53 项通过。原提交的 Bevy 用同一新增用例编译后失败，隔离构建修复后 91 项社交测试通过。Rust 使用 1.95.0、原 Cargo 锁和已提交的同分支依赖。

开发构建三次未取得组队证据。两次等待第一个浏览器的组队面板，均停在发邀请前。第三次曾在第一个浏览器打开面板，第二个浏览器进场时第一个已回登录，未发送 AddMember。此轮网关观察到的首个连接离开早于第二个游戏连接，成因以生产构建和浏览器导航复测为准。

实际运行包 `bevy-81b7623f6e65d0ea` 包含本次共用 Rust 改动，源自 `6c55c05aae` 加 `social.rs`，使用冻结的 `runtime/src/lib.rs`。预编译发行需一并更新清单及资源。 [三套运行包及散列](../r16-trade-own-gold-20261110/runtime-verification.json)

## 本机生产构建复测

冻结组队源码、交易组件和三套 WASM。通过 `next build --webpack` 后用 `next start` 服务 `/game`。两位玩家各用一个浏览器进比奇，实际打开组队窗。发起者输入 `bLaDe`，对方接受，两边收到 `Blade` 和两人名单。发起者输入原名 `Blade` 移出成员，在原连接再次输入 `bLaDe`，对方接受并恢复两人名单。

每人一条游戏连接、一次成功进场和一次正常注销。两端版本化 WebGPU JS 和 WASM 返回 200，页面与网络错误为零。隔离 File 网关和 Next 均已关闭。Windows 窗口及拒绝或静默后的手动重试继续验收。 [输入原拼写](typed-bLaDe.png) · [第二次入队](second-authoritative-roster.png) · [普通浏览器回执](browser-receipt.json) · [回归与开发模式诊断](verification.json)
