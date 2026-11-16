# R16 阿洛跨端培养记录

同一角色在隔离 File 服通过普通 Web `/game` 登录比奇。玩家从伙伴面板教阿洛组队先支援、战后拾取、离队先询问，三项从 v1 的否变为 v2 的是。正常注销后关闭运行器，以只读方式确认 SQLite 的档案和教学记录，再启动新进程打开同一数据库。 [完整回执](roundtrip.json) · [Web 教学](web-teaches-v2.png)

Windows 测试进程使用正式 `platform-windows/src/gateway.rs` WebSocket 传输和 `NativeCompanionHostPlugin`。真实登录与角色启动后，测试入口启用正常游戏态，使用有归属标记的 `List`、`View` 和 `RevokeFeedback` 请求。读回同一档案、角色、世界和 v2 教学，撤销得到 v3。正常注销后原生私有投影清空。随后一个全新 Web 浏览器会话读回同一条已撤销记录，三项习惯都显示为否。两次 Web 注销和一次 Windows 注销均有回执，停止服务后 SQLite 包含一条教学和一条撤销。 [新 Web 会话读回](web-reads-v3.png)

实际 Web 页面错误为零，运行器使用规则模式，模型调用为零。35 项 Node 相关测试通过。另用读回的 v2 和 v3 在同一规则夹具比较决策，v2 在战斗中支援、结束后拾取，撤销后的 v3 恢复原优先级。Windows 窗口交互和跨端实战继续单列。 [规则回放](policy-replay.json)

首轮的原生读回和撤销成功，但脚本错误要求重启前后的成长修订号相等。新运行过程也会增加成长修订号。第二轮核对修订号递增，以及身份、教学版本、反馈和习惯的精确读回。 [首轮记录](initial-growth-revision-assumption.json)
