# Mir2-web3 游戏安全审计报告

- **日期**：2026-06-07
- **审计范围**：`mir2-web3/`（gateway / simulation / protocol / web / admin-api / admin-web）
- **方法**：源码静态审计（4 个并行子代理分域扫描 + 人工对关键发现逐条复核）。本报告对每条发现标注 **【已复核】**（作者亲自读代码确认）或 **【待复核】**（子代理报告、未逐行确认）。
- **威胁模型**：网络可达的恶意客户端构造任意 `ClientPacket`；可访问 admin-api 的攻击者；数据库/磁盘被拖库；中间人。核心问题是 **"服务器是否信任客户端"** 与 **"管理面是否做了鉴权"**。

> 说明：本项目目标是与 Crystal C# 服务端 1:1 复刻。部分问题（如明文密码）是从 Crystal 原版继承的设计；在原版单机/局域网语境下风险可接受，但在 Web 公网部署语境下需要重新评估。报告中已注明此类情况。

---

## 0. 结论速览

| 等级 | 数量 | 关键问题 |
|---|---|---|
| Critical | 1 | admin-api 读接口完全无鉴权，可拉取全量账号/玩家/经济/邮件数据 |
| High | 3 | 明文存储密码；协议解码器未限长导致预认证远程内存 DoS；ClickHouse 凭据硬编码兜底 |
| Medium | 5 | Passkey 密钥开发兜底（配置错误即可伪造令牌）；远程攻击缺服务端距离校验；`gatewayWs` 参数可被劫持；admin-web 默认全权限操作员；交易物品索引竞态（待复核） |
| Low | 6 | TCP 无读超时（slowloris）；密码非常量时间比较；dev DB 凭据兜底；内网 IP 泄露；无限流；Passkey origin 空值绕过 |
| Info / 纠偏 | — | 多项子代理"Critical"经复核为误报，见 §5 |

**优先修复**：F-01（admin 鉴权）→ F-03（协议限长）→ F-02（密码哈希）→ F-04（ClickHouse 凭据）。

---

## 1. Critical

### F-01 —【已复核】admin-api 全部"读"接口无鉴权（Broken Access Control）

- **位置**：`apps/admin-api/src/lib.rs:4058-4134`（路由表）；处理函数 `list_audit:5051`、`list_approvals:5063`、`read_accounts`、`read_account_detail`、`read_players`、`read_player_detail`、`read_economy`、`read_economy_aggregate`、`read_mail`、`read_auctions`、`read_items`、`read_guilds`、`read_risk`、`read_operators` 等（路由见 `lib.rs:4086-4106`）。
- **复核证据**：路由层未挂任何鉴权中间件（`admin_router_with_state` 直接 `.with_state(state)`，无 `route_layer`）。鉴权只在各 handler 内部通过 `operator_from_headers(&headers, ...)` 完成——而**读 handler 的签名根本不接收 `HeaderMap`**：

  ```rust
  async fn list_audit(State(state): State<AdminApiState>)        // 无 headers，无鉴权
      -> Result<Json<Vec<AuditRecord>>, ApiError> { ... }        // lib.rs:5051
  async fn create_approval(State(state), headers: HeaderMap, ...) // 写接口有鉴权
  { let operator = operator_from_headers(&headers, ...)?; ... }   // lib.rs:5081
  ```
- **影响**：任何能访问 admin-api 网络端点的人，无需凭据即可遍历**全部账号、玩家位置/等级/库存、经济总量与巨富榜、系统邮件、拍卖行挂单、封禁与风控图、操作员列表**。属于敏感数据全量泄露 + 运营情报泄露。
- **严重度说明**：若 admin-api 仅监听内网且有网络隔离，实际暴露面下降为 High；但代码层面缺失授权检查本身即为 Critical 缺陷，不应依赖网络边界兜底。
- **建议**：在路由层统一挂鉴权中间件（`axum::middleware::from_fn` 校验操作员令牌 + 细粒度 `*_read` 权限），让所有 `/admin/*`（除 `/health`）默认需要认证；读接口补 `headers: HeaderMap` 并调用 `operator_from_headers` + 对应 `require_*_read` 权限。

---

## 2. High

### F-02 —【已复核】账号密码明文存储与明文比较

- **位置**：`apps/simulation/src/runtime/save.rs:305`（`account.password = password.to_string()`）、`:336`（`if account.password == password`）；持久化 `apps/simulation/src/config.rs:2818`（`password_snapshot = &account.password` 直接写入 Postgres）。
- **影响**：密码以明文落库/落盘。一旦数据库或 `accounts.json` 被拖库，所有玩家密码（及其可能的跨站复用）立即泄露。同时 `==` 比较非常量时间（见 F-12）。
- **说明**：此为 Crystal 原版继承设计。Web 公网部署下应升级。
- **建议**：改用 `argon2`/`bcrypt` 存哈希（加盐），登录时 `verify`；迁移时对存量明文做一次性 rehash。Passkey 路径（`login_passkey_account`，无密码）不受影响。

### F-03 —【已复核】协议解码器未限长 → 预认证远程内存 DoS

- **位置**：`packages/protocol/src/packets.rs:1360`（`Chat.linked_item_count`）、`:1602`（slots `slot_count`）、`:1719`（`EditGuildNotice.line_count`）。
- **复核证据**：三处均从客户端读 `i32` 计数，仅拒绝负值，**未设上限、未对照 `reader.remaining()`**，随后 `Vec::with_capacity(count as usize)`：

  ```rust
  let linked_item_count = reader.read_i32()?;
  if linked_item_count < 0 { return Err(...NegativeLength...); }
  let mut linked_items = Vec::with_capacity(linked_item_count as usize); // 可达 ~2e9
  ```
- **影响**：帧整体被 `u16` 限制在 ~65 KB（`frame.rs:16-25`），但 `with_capacity` 是**按计数值预分配**，与实际字节数无关。一个约 10 字节的小包声明 `count = 2_000_000_000`，即触发数 GB 预分配 → OOM/abort。解码发生在 `decode_client_packet`（`tcp.rs:78` / web 入口），**先于登录**，故为未认证攻击者可触发的远程拒绝服务。
- **对比**：`PacketReader` 本身是安全的——`read_string`/`read_bytes` 在分配前 `ensure(len)` 对照剩余字节（`io.rs:73-88`），7-bit 长度上限 5 字节。问题仅出在上述三处"先按计数预分配，再循环读取"的模式。
- **建议**：分配前对照 `reader.remaining()`/每元素最小字节数设上限（如 `if count as usize > reader.remaining() { return Err(...) }`），或用 `Vec::new()` + 循环 `push`（容量按需增长，受帧大小天然约束）。

### F-04 —【待复核】ClickHouse 管理凭据硬编码兜底

- **位置**：`apps/admin-api/src/lib.rs:8274 / 8305 / 8338`（`fetch_clickhouse_*`），env 未设时回落到 `mir2` / `mir2_dev_password`。
- **影响**：若 `ADMIN_CLICKHOUSE_PASSWORD` 等未配置，admin-api 用公开可见的弱口令连接 ClickHouse；任何能访问该 ClickHouse 的人也可用同口令登录。
- **建议**：生产环境缺失凭据时应**启动即失败**，不提供硬编码兜底（与 F-05 同理）。

---

## 3. Medium

### F-05 —【已复核】Passkey HMAC 密钥开发兜底（配置错误即可伪造令牌）

- **位置**：`apps/gateway/src/auth.rs:47-60`；Web 侧同样模式 `apps/web/app/api/passkey/login/route.ts:108-122`。
- **复核**：`MIR2_PASSKEY_AUTH_SECRET` 未设且未检测到 production/staging 时，回落硬编码 `"mir2-web3-local-passkey-auth-secret"`（`auth.rs:57`）。生产检测依赖 `MIR2_RUNTIME_ENV/MIR2_DEPLOYMENT_ENV/MIR2_ENV` 三者之一等于 `production|prod|staging`（`auth.rs:62-72`）。
- **影响**：正常生产配置下 **fail-closed**（缺密钥则报错，见单测 `auth.rs:157-173`），设计是合理的。但属 **fail-open-on-misconfig**：若部署时这三个 env 拼写错误或漏配，则用公开 HMAC 密钥签发——攻击者可为**任意 accountId** 伪造合法 passkey 令牌（令牌 payload 绑定 accountId 且服务端校验签名，密钥一旦公开即可任意伪造，见 `auth.rs:20-45`）。
- **建议**：去掉硬编码兜底，未配置密钥一律拒绝；或将"是否生产"改为白名单显式开关而非字符串匹配推断。

### F-06 —【已复核】远程攻击缺服务端距离/视野校验

- **位置**：`apps/simulation/src/runtime/combat.rs:3116-3238`（`range_attack_impl`）。
- **复核**：函数按 `target_id` 查实体、校验其为存活且敌对怪物、伤害用**服务端** `target_position` 计算（`:3187` `ranged_attack_delay_ticks(&player_position, &target_position)`），这些都正确；但**没有对 `tile_distance(player, target)` 设上限**。即只要客户端知道某怪物的 `object_id`，可从任意距离发起攻击。
- **缓解现状**：`object_id` 通常只在玩家视野内被下发，远处目标 id 不易获取，故实际利用门槛存在。近战路径 `attack_impl_with_spell`（`combat.rs:2815`）则**有**服务端 `tile_distance` 距离校验，是正确的。
- **影响**：反作弊——可能实现超视距打击、破坏射程平衡。
- **建议**：远程攻击补充服务端最大射程校验（与 Crystal bow/魔法射程一致）。
- **纠偏**：子代理将"`target_location` 被原样回显"列为 Critical，经复核该值仅用于动画回显、伤害与命中均用服务端坐标，属 **Info**（见 §5）。

### F-07 —【待复核】交易确认时物品索引可能过期（竞态/错物风险）

- **位置**：`apps/simulation/src/runtime/packets.rs:4425-4448`（`stage5_trade_confirm_packet`）。
- **子代理观点**：交易确认按存入时记录的库存槽位索引扣除物品；若玩家在"存入→确认"之间 `move_item_impl` 移动过物品，索引校验（`:4425-4437`）可能放行到错误物品。金币侧在确认时已校验余额（`:4421`），物品侧的再校验充分性需进一步验证。
- **建议**：以稳定的 `unique_id` 而非槽位索引在确认时重新定位并校验物品；或在交易窗口期锁定/快照参与交易的物品。**本条未经作者逐行复核，列为待办验证项。**

### F-08 —【已复核】`gatewayWs` 查询参数可指向任意 WebSocket 主机

- **位置**：`apps/web/app/page.tsx`（`resolveGatewayWebSocketUrl`，约 `:876`）。
- **复核**：仅用正则 `^wss?://` 校验协议，不校验主机。攻击者诱导用户打开 `?gatewayWs=wss://attacker/ws`，即可将其游戏连接导向恶意网关，捕获后续明文登录凭据/游戏状态（登录在连接建立后于 WS 内发送）。
- **建议**：对 host 做白名单；或移除该 URL 参数覆盖能力，仅用构建期配置。

### F-09 —【待复核】admin-web 默认操作员头部携带全部权限

- **位置**：`apps/admin-web/lib/admin-api.ts:472-488`。
- **子代理观点**：env 未设时默认 operator=`local-gm`、role=`ops_admin`、permissions 含全部 23 项。若生产漏配 env，admin-web 将以全权限默认头部请求 admin-api。结合 F-01（读接口本就无鉴权）与写接口的令牌校验，需确认默认令牌在生产是否可用。
- **建议**：移除默认全权限兜底，未显式配置则拒绝发起请求。

---

## 4. Low / 加固项

- **F-10 —【已复核】TCP 帧无读超时（slowloris）**：`apps/gateway/src/tcp.rs:103-119`。帧长受 `u16` 限制为 ≤64 KB（**并非"无界分配"**，纠正子代理说法），但 `read_exact` 无超时——攻击者声明 64 KB 后缓慢发送即可长期占用连接任务。建议加读超时与每 IP 连接数上限。
- **F-11 —【已复核】dev DB 凭据硬编码兜底**：`apps/simulation/src/config.rs:956`、`apps/gateway/src/zone_lease.rs:375`（`postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2`）。均为测试/本地兜底，非生产密钥，风险低；建议生产路径去掉兜底。
- **F-12 —【已复核】密码非常量时间比较**：`save.rs:336` `account.password == password`，`String ==` 短路比较存在理论时序侧信道（被网络抖动掩盖，风险低）。修哈希（F-02）后用库的常量时间 `verify` 即可。
- **F-13 —【待复核】内网 IP 写入仓库配置**：`infra/cloudflare/.../wrangler.jsonc:25`（`GATEWAY_ORIGIN_URL: https://165.154.65.136.sslip.io`），便于侦察，建议核实是否仍为活动基础设施。
- **F-14 —【待复核】Passkey origin 空值绕过**：`apps/web/app/api/passkey/login/route.ts:88`，仅当 `requestOrigin` 非空才校验 origin；浏览器请求必带 Origin，风险低。建议强制校验。
- **F-15 —【待复核】无限流**：admin-api 各接口、登录、交易/买卖（`npc.rs:747`）均无速率限制，便于数据高速外泄或刷量。建议加限流中间件与交易冷却。

---

## 5. 纠偏：经复核为误报/被高估的子代理结论

为避免误导，以下子代理给出的"Critical/High"经作者读码后下调或否定：

- **❌ "经 character_index 的会话冒用"（误报）**：`apps/gateway/src/web.rs:1646-1665` 的重连键由**服务端**已认证状态 `authenticated_account_id` 构造（仅在登录校验通过后由该次登录的 accountId 赋值，`web.rs:1753-1759`），并按账号内的 `character_index` 限定。客户端无法借此恢复**他人**账号的会话。
- **❌ "可预测的 session_id / 会话固定"（不适用）**：协议中客户端**从不向服务端出示 session_id**（`packets.rs` 无该字段），session_id 仅为内部标识、非 bearer 令牌，故"可预测"不构成认证绕过。
- **🔽 "远程攻击 target_location 欺骗"= Critical → Info**：该坐标仅用于动画回显，伤害/命中均用服务端坐标（见 F-06）。
- **🔽 "TCP 无界分配"= High → Low**：实际受 `u16` 限制为 ≤64 KB（见 F-10）；真正的内存 DoS 在协议层 `with_capacity`（F-03）。
- **🔽 "Passkey 暴力破解无限流"**：HMAC 令牌伪造需要密钥，暴力穷举不可行，非真实威胁（真实风险是密钥兜底 F-05）。
- **ℹ️ admin-api 无 CORS**：缺少 CORS 层意味着浏览器跨域**默认被拦截**（无 ACAO 头），并非"默认放行"；对非浏览器直连无影响。真正问题是 F-01 的无鉴权，CORS 在此为次要项。

---

## 6. 做得好的地方（正面结论）

- **SQL 全参数化**：`apps/simulation/src/db_projection.rs` 使用 `$1,$2` 占位符，无字符串拼接，无 SQL 注入面。
- **GM 权限服务端权威**：`gm_level` 在角色加载时来自账号存储（`save.rs:643-657`），客户端无法运行时自改；GM 命令以 `gm_level > 0` 守卫。
- **协议读取器安全**：`PacketReader` 分配前 `ensure` 校验剩余字节，无越界/无超额分配（问题仅在调用方的 `with_capacity`，F-03）。
- **整数运算多用 saturating**：金币买卖等使用 `saturating_add/sub` 防溢出（`npc.rs:1047-1063`）。
- **近战攻击有服务端距离校验**；移动经服务端寻路+碰撞校验（`movement.rs`）。
- **账号存储路径无遍历**：accountId 作为 `BTreeMap` 键而非文件名，存盘用固定文件名 + 原子写。
- **CI 机密处理稳健**：未用 `pull_request_target` 跑不可信代码，机密经 `${{ secrets.* }}` 注入、未回显日志，权限最小化。
- **前端无 XSS sink**：未发现 `dangerouslySetInnerHTML`/`innerHTML`/`eval`，依赖 React 默认转义。
- **Cargo 全为本地 `path` 依赖**，无外部 git 依赖，供应链面小。

---

## 7. 建议修复顺序

1. **F-01**：admin-api 路由层统一鉴权，读接口补 `*_read` 权限校验。
2. **F-03**：协议三处 `with_capacity` 加上限/对照 `remaining()`。
3. **F-02**：密码改 argon2/bcrypt 哈希 + 常量时间校验（含 F-12）。
4. **F-04 / F-05**：去除 ClickHouse 与 Passkey 的硬编码凭据兜底，生产缺配即启动失败。
5. **F-06 / F-08 / F-09**：补远程攻击射程校验；`gatewayWs` host 白名单；移除 admin-web 默认全权限。
6. **F-07**：交易确认改用 `unique_id` 重定位并复核（先复核再修）。
7. **加固**：TCP 读超时、admin-api 与交易限流、内网 IP 核查。

> 后续可在真实环境运行 `cargo audit` / `npm audit` 检查传递依赖漏洞（本次为离线静态审计，未覆盖）。
