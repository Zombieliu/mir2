# R16 组队邀请手动重试

固定 Crystal `92b4ce4ab` 的 `PlayerObject.AddMember` 与 `GroupInvite` 对照 `62acffcba4`。原客户端每次确认名字框都会提交普通 `AddMember`。网关按两秒冷却决定是否处理，关闭邀请和主动拒绝会给发起者具名系统消息，冷却内的请求静默。入队依赖原有权威包。 [Crystal 服务端](../../../../Crystal/Server/MirObjects/PlayerObject.cs) · [Crystal 窗口](../../../../Crystal/Client/MirScenes/Dialogs/GroupDialog.cs)

两组本机生产构建对照使用普通账号、普通组队窗和 File 存档副本。第一组点击拒绝，发起者看到 `Blade has declined your group invite.`。2,598 毫秒后重输 `bLaDe`，提交按钮可用，仍只有一条请求，输入框保留名字。第二组关闭邀请，第一条收到 `Blade is not allowing group.`。362 毫秒后发出的 `bLADe` 被静默处理。打开邀请并等到冷却结束后，再点同一拼写仍无新包。四次正常注销，第二组自有网关在进程退出等待超时后由脚本终止。 [旧界面](baseline-blocked-draft.png) · [对照回执](baseline-verification.json)

Web 和共用 Rust 预算在发包前保留现有角色、会话、地图和八项预算复核。`AddMember` 获准进入发送边界就结束本地占用。Web 页面保留确定未发送的草稿，发送结果未知时由玩家的下一次操作发起新请求。Windows 在原生命令队列接受该邀请后释放精确名字记录及其旧草稿。队伍成员、邀请许可与踢人的结算仍由原权威包控制。

## 本机生产构建复测

同一 Gateway、两份独立 File 夹具和新运行包 `bevy-22790e9223ec130f`。第一组在真实拒绝后再次提交 `bLaDe`，第二组在关闭邀请与两秒内静默后重新开启邀请，再次提交 `bLADe`。每次重试各出现一个原协议请求和一个邀请窗口。等待期间没有自动请求。接受前队伍为空，实际点击接受后两端收到 `Blade` 的两人名单，四次普通注销。

WebGPU 浏览器复测采用 `next build --webpack` 与 `next start`。实际 WebGL2-shared WASM 和 Page/窗口/发送顺序共 258 项，Stage5 适配器 508 组，TypeScript 通过。共用 Rust 社交 92 项及 JSON 边界 9 项。Windows 原生命令队列组队 6 项、桥接 156 项、指令归属 5 项分组记录，有筛选交集。 [新版入队画面](candidate-retry-party.png) · [回执与回归](candidate-verification.json)

三套运行包的直接文件及版本化文件通过 12 次 SHA-256 验证。预编译发行需同步更新运行包与清单。Windows 窗口和公网发行分别验收。 [构建与资源散列](runtime-verification.json)
