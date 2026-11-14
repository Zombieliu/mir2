# R16 首步与静态资源联合定位（2026-11-14）

本轮没有发布或修改游戏规则。正式站仍为 `e6c923b7c669cb061a7a03703f05fadd91211c97`／`dpl_Ae3hNyFygQuSMZDesBXCZDpUECTY`。当前正式版的新浏览器回放再次失败：一步已发出，30 秒等待超时，后来在 **61.929 秒**收到同目标的权威确认。不能将后来到达倒填为原 8 秒通过；Goal 仍 active，完整 1:1 未完成。

## 两轮回包证据

| 观测 | 已回退的 7140 候选 | 当前正式 E6 的新观察 |
| --- | --- | --- |
| 首步 wire → ACK | 13,286 ms；CDP 13,283 ms | 61,929 ms；CDP 61,928 ms |
| 首个发出后完整 health 样本 | +463 ms，计数 +1／累计耗时 +524 µs | +852 ms，计数 +1／累计耗时 +674 µs |
| 同窗已记录 long task／timer drift | 0／0 | 0／0 |
| 连续 JS 回调最大间隔 | 2,002 ms | 2,002 ms |
| 最终功能与冷门槛 | 原失败保留 | 六字体完整 SHA、113/113、共享 HUD/Q、Worker 停启、普通注销通过；首步失败，Native 首帧后 124,326 ms，仍超过原 120 秒 |

两次 health 都是全局累计值，**缺实例、玩家及命令绑定**，不能把微秒差值当成该玩家的确切执行时间。无记录到的长任务也不等于完整排除渲染器。分析器始终输出 `rootCauseProven:false`、`actionIdentityBound:false`、`mainThreadCauseExcluded:false`。

- [旧失败的时序分析](failed7140-delivery-analysis.json)：重新使用原始 7140 失败输入，未重跑已消费的发布验收。
- [当前 E6 的时序分析](current-e6-delivery-analysis.json)与[完整分项摘要](current-e6-public-summary.json)：新 Testnet 钱包仅一次登录签名、一个普通 QA 角色、一条相邻 Walk，无额外游戏指令。移动超时后保留观测，后来到达仍判失败；正常注销与两个自有浏览器关闭完成。36 条静态 HTTP 错误、38 条 console、556 次 ERR_ABORTED 保留，不以恢复后的缓存掩盖。
- [最终独立生产 API](current-production-after-observation.json)：08:00:42 UTC 仍为 E6／Ae3／READY。此轮是当前正式版诊断，**不是新发布或新晋级**。

## 服务端只读与 TCP

固定主机指纹校验后，经已有授权的 PTY getpass 读取当前 PID/startTicks/InvocationID、health 和 35 秒历史日志。密码仅驻内存，不导出环境或原始日志。既有诊断读取器返回完整 9 条合法投影和远端 exit 0；当时未启用逐回包 tracing。最大慢阶段是 `socket_turn.input` 3,202 ms；缺少发送日志不能证明发送快，也不能精确跨两台时钟关联到该 ACK。[身份与读取来源](read-only-server-provenance.json)

新的 E6 浏览器回放同时启动已有、未修改的 `cloud_tcp_readonly.py`。实际 137 个样本，目标与 Caddy 身份持续校验；按未经校准的客户端时钟加前后各 5 秒选出 36 个样本。部分公共 443 流可见重传计数最高 13、发送队列最高 2,523 B、RTO 最高 2,240 ms。**这些流尚未绑定到游戏 WebSocket**，含独立 HTTP health 和其他同机流量，不能直接宣布网络根因。[TCP 范围与空值](tcp-window-summary.json)

本轮在浏览器结束后通过自己的 stop-file 结束采样；若干可选 TCP 字段缺失，`diagnosticComplete:false`，远端退出状态未知保持 null，不填 0。不安装、不重启服务、不写远端文件、不修改网络/VPN/DNS。

## 静态资源观察补齐

原观察只筛选字体和运行包，本轮增加公开静态路径白名单：

- [页面事件 730 行](public-page-static-network.json)：起始、响应、结束/失败、明确可得的 timing 和 sizes；无截断、无观察错误。
- [Worker 上游 708 行](public-worker-static-network.json)：独立同上下文 CDP 附着，含资源起止及数值字节；无截断、无观察错误，清理完成。未来 Worker 的附着竞态仍保留为限制。
- 页面和上游是两层观察，**不能相加当作网络字节**。不记录认证接口、URL 查询串、头或正文；不拦截请求，不改代理、DNS、TLS 或模型/游戏行为。
- Playwright 1.55.1 默认不导出 Worker 上游事件。真实本地 Chrome 检查先发现这个缺口，随后以独立 CDP 补齐，未开启实验网络标志。[真实浏览器验证及两次工具失败](local-chrome-worker-observer.json)

22 个新冻结 helper 的 SHA 和原模板来源见[清单](public-helper-manifest.json)。六份碰撞/地图来源在当前 E6 提交逐字节校验，原始发布失败、所有原时限和授权范围保持。两个[静态观察生产器快照](producers/observe-static-delivery.mjs)与[Worker 快照](producers/worker-static-network.mjs)仅为本轮证据，非生产入口；认证 helper 与原输入留在本机，提交仅含[摘要哈希](inputs-manifest.json)。

共享任务窗[截图](current-e6-shared-quest.png)已人工审阅；它仍显示 50/113 的加载失败覆盖层、英文公告和英文 NPC 别名。后来六字体与 113/113 成功属于独立 JSON 观测，不能把这张较早截图解释为最终视觉已对齐。[审图边界](visual-review.json)

## 可复测工具及下一项

仓库根目录执行，无需模型或部署：

```powershell
node --test mir2-web3/apps/web/scripts/startup-delivery-analysis.test.mjs mir2-web3/apps/web/scripts/observe-static-delivery.test.mjs
node mir2-web3/apps/web/scripts/analyze-startup-delivery.mjs receipt.json startup-probe.json server-readback.json output.json
```

服务端没有匹配的新只读记录时，第三个参数使用 `-`，保持 `server.observed:false`。输出使用独占创建，不能覆盖旧回执。16 项测试实际通过，覆盖计数重置、缺样、身份变化、部分读取、隐私投影、两层请求、redirect、超限和未完成请求。[测试原输出](observer-analysis-tests-16.tap)

下一步优先补**可绑定到同一普通游戏连接的发送/验证队列观测**，区别服务端排队与连接传输；现有静态完整瀑布用于定位资源争用，不继续盲改超时。正式域名 `/playtest/health`、`/playtest/ws`、`/ws` 三条只读探测均 404，目前仍走 sslip 网关；若研究第一方 WebSocket 路径，需独立候选和端到端验收，不直接替换线上地址。动态多人第三格占位、旧 R15 停步、公开长路线及整体 Windows 对照继续开放。
