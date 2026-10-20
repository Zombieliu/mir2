# 狩猎经济：Windows 原生客户端接入

## 第九阶段当前审计：原生报价已达，成功确认待验（2026-10-21）

生产独立repair source gate已修复真实命名equip slot、合法缺省UID及严格raw字段presence/dup检查，实际完整inventory hash参与身份；原buy gate逐字未改。Host60实际39/39，其中5项rich-source包含真实丰富库存形状；正向路径已到Quote→合法当前30秒Quoted→单Confirm实际TestSink→Pending→原Query Unknown仍Pending。这不是GUI或物理WebSocket收费验收。producer60b的7案例/1项测试、UI58的17/17、Runtime50的repair15及邻接41分别保留；不与43/44历史计数累加。

新EXE59已实际链接启动，普通登录/StartGame/NPC/Repair后拖入Bag得到服务端费用6。第一次显式Close后无扣款；顶X点击未建立关闭证据。第二次报价到OKAY控件操作约97秒，超过原30秒TTL，正常拒绝且未发Confirm。停服后独立psql的`C:/mir2-build/hunting-native-pg-final-after-deadline-56.json`在同一隔离schema记录0结果/0销毁、销毁金币0、绑定1,000,000、旧gold 1234、Bag100/1000、save_version15/revision14；不能声称成功扣款、原生Pending原Query、断线或最终冷登录已验。

fixture56已达到2小时截止正常退出0，proxy56按stop marker退出0，EXE59仅自己的窗口Alt+F4正常退出0，其他游戏未操作。原当前窗口30秒快速确认请求已撤回，当前不要操作；下轮新隔离环境再验成功Confirm→Pending Close→原Query→断线→普通保存冷登录，不延TTL、不改clock、不绕guard。Quoted状态仍显示通用选择文案是小UX待办，顶X装饰/点击关闭未验，不能当作成功Close。Gateway46的恢复纯5+物理WS1及PG45的1项当前会话/撤销/原expiry不延长证明保留原服务端范围。

汇总见[阶段九审计](generated/hunting-economy/acceptance-20261021-repair-native-windows.json)；本页后续第56轮及43–55轮文字保留历史范围。完整狩猎经济任务未完成，其他产出、材料传播、维护/PVP/沙巴克销毁、全服市场画像收益、通知和迁移发布仍开放；默认关闭、K=0.1、0.72/1.08许可不变，不提高Crystal/Candidate或生产资格。无Docker/WSL/虚拟机、远程资产修改或其它窗口干扰。

2026-10-20：用户明确要求检查游戏的 Windows 客户端。后续优先验收
`apps/game-client/platform-windows` 的 Bevy EXE。Windows 上直接运行的
Next/Gateway、浏览器截图和 Tauri WebView 均不能代替这一验收。

## 第56轮已记录状态（历史范围）

原生代码已接入现有 NPC/Bag/UI、物理 WebSocket、完整模型主线程应用及只读钱包。
更新至2026-10-21（北京时间）：第45、51、56轮均实际链接并启动了新的 Windows
EXE，通过普通登录、StartGame 和 NPC 交互进入现有游戏窗口。第56轮修理窗口已
稳定显示，真实只读钱包显示绑定金币1,000,000、非绑定金币0；但 Bag 拖入仍被
`ItemOrServiceChanged` 拒绝，未发出 `huntingRepairOwner` 报价/确认/查询请求，
没有发生扣款。当时正在修复repair专用的原始完整库存schema；现由本页第九阶段
审计更新，仍不能通过放松物品、服务或owner见证使其通过。

这仍不是完整 Windows 修理玩家流程验收：尚未实际 Confirm、原 Query、物理
断线恢复或完成本轮最终保存后冷登录，候选打包也未验证。历史候选摘要、旧 EXE
或重新打包旧 EXE不能证明本轮功能已交付。已通过的专项和当前实机事实分别见下文。

- `gameplay_bridge.rs` 将四个独立修理意图交给 host；hunting 模式使用
  Quote/Confirm/Query，原 UID 指令仅在明确的 legacy 模式允许发送。Unknown
  模式拒绝旧修理指令，政策关闭也不能使已声明 hunting 的连接降级。
- 原有 `npc_item_service.rs` 和 overlay 已显示服务端绑定费用。选择真实 Bag
  物品只请求 Quote，明确 Confirm 才进入收费发送；hunting 不使用旧金币余额
  或本地 float 费用授权，Hold 不自动确认。
- host 从原始物理收帧路径先处理严格 realm/mode、私有身份 grant 和修理回执。
  真实完整物品与 NPC 服务见证、账号/角色及完整本地 owner stamp 参与发送检查。
- 独立 repair bundle/gate 接入 NativeInbound 屏障和主线程完整模型替换，只有
  实际 Applied 才完成 Pending；最终完成回调同时复核物理 owner 和 Runtime gate，
  防止弹出 Applied 后的 reset 竞态。修理与未决购买互斥，不制造购买协议身份。

## 完整纵向切片

1. 从原始物理连接读取并锁定 realm 的 repair mode；hunting 声明不能因政策
   关闭而降级 legacy。账号、角色、连接及 scene/procedure stamp 必须保持关联。
2. 普通 NPC 服务及真实完整 Bag witness 只允许请求 Quote。停用 hunting 下
   的 Hold 自动确认、本地旧金币门槛及本地计算的经济费用。
3. 显示服务端绑定费用；玩家明确 Confirm 当前 quote。最终 proof 检查和
   `start_send` 应在现有同一短临界区，不能把旧 UID 指令改名作为 Confirm。
4. Confirm 进入发送后至多一次。超时、断线或未知结果只能 Query 原 quote，
   关闭窗口不能丢失 Pending。当前设计由玩家手动 Query 原结果，不要求自动
   Query。进程存活期间保留原操作；物理重连须重新获得可信身份 grant 和完整
   当前角色投影后才可 Query。换角和旧物理连接的回执不能应用到新角色。
5. 严格解码原始 owner frame，并先关联 request/quote/原 stamp，再预解码本次
   完整 snapshot。收到、入队、主线程实际应用是三个不同状态。
6. 主线程一次替换完整模型并返回独立、不可伪造的 Applied。null/坏 snapshot
   或应用失败保留待同步状态。成功后显示只读完成，普通 Close 清除展示。
7. 新钱包单独观察：真实 identitySession grant 私有绑定连接/账号/session/时效，
   只读固定钱包接口；余额不能作为 Confirm 授权，不能回退到旧 gold。

同一物理连接普通 LogOut 回角色选择仍保留身份 grant，StartGame 只重新绑定
角色；新 Login/NewAccount、返回 Login、Disconnect 或物理连接结束才清除身份。
服务端已新增经真实身份校验重新发放原 grant 的 resume 路径，发送顺序为
identitySession → sessionResumed → 完整 snapshot。原生 reader 在恢复隔离之前
接收 grant。第46轮5项纯测试及1项真实 WebSocket 恢复测试通过，第45轮1项隔离
PG测试通过：重新读取当前会话并拒绝已撤销会话，重发同一 session 的原签名字段，
不创建新会话、不延长原到期时间。这是服务端协议验收，不等于新 EXE 已完成断线
后的 Pending 查询。第45轮物理测试曾把连接初始未选角快照计入恢复帧，失败记录
保留；第46轮先验证并读取该初始快照，再保留严格的 grant 先于恢复角色快照断言。
EXE 全进程重启不承诺恢复原回执 UI，须用冷登录核对服务端持久资产与存档。

## 复用入口与边界

| 入口 | 可复用范围 |
| --- | --- |
| platform-windows/src/gateway.rs 的 NativeCommandStamp、最终发送锁 | 物理 connection/procedure/owner/scene/cancellation 及发送退休检查 |
| platform-windows/src/hunting_repair_client.rs、hunting_repair_gateway.rs | 原 quote/请求发送托管、超时、原 Query、收帧与实际 Applied 接线 |
| platform-windows/src/hunting_repair_connection.rs | 严格 mode/身份 grant 与固定只读钱包请求，私有凭证不进入 UI |
| platform-windows/src/npc_purchase_gateway.rs | 原始 marker 分流、fresh 投影、主线程实际 Applied 的接线模式 |
| platform-windows/src/npc_purchase_projection.rs | 全量模型预解码；repair 的 nativeNpcShop=null 可自然清退旧 NPC 权限 |
| runtime/src/hunting_repair_economy.rs、native_ingest.rs | 独立 gate/bundle、完整应用、只读模型投递及 reset/队列屏障 |
| runtime/src/npc_purchase_economy.rs | 共享完整模型预解码/应用引擎，不借用购买身份 |
| client-bevy/src/hunting_repair.rs、crystal_ui/npc_item_service.rs | host 只读模型及现有 NPC 窗口中的 Quote/Confirm/Query/Close |
| client-wire/src/hunting_repair.rs | Quote/Confirm/Query 与严格 raw frame 解码和关联 |

修理协议没有购买的 Begin、ActorKey 或 producer；必须使用独立 repair custody
和 gate。禁止制造 PurchaseBinding、服务器 revision 或购买确认凭据。

上述 transport、Runtime、UI 三个写集已分别实现并交由主代理串行整合验证。
共享文件仍遵守单一写者，后续不能在同轮并行修改 gateway.rs 或 Runtime 调度。

## 部署配置与钱包观察

- EXE 使用 `MIR2_GATEWAY_WS_URL` 或 `mir2-client.toml` 的
  `[server].gateway_ws_url` 选择真实 Gateway；发行地址沿用现有 WSS 校验。
  服务端须包含新的 `realmInfo.npcRepairMode` 协议。旧服务端缺少 mode 时，
  新客户端拒绝建立修理权限，不能仅更新 EXE 就假定远端已支持。
- 钱包现阶段只从启动进程的可信环境变量 `MIR2_NATIVE_WALLET_ORIGIN` 读取
  origin，必须与 Gateway 的 `MIR2_ALLOWED_WEB_ORIGINS` 精确匹配。例如本机
  隔离验收可显式设 `http://127.0.0.1:19540`，不能将该测试地址用于正式发行。
  HTTP 地址由当前 Gateway WS 地址固定派生为 `/v1/economy/hunting-wallet`，
  凭证仅在后台请求中使用，8 秒有界请求不阻塞游戏线程。
- 该 origin 尚未接入候选包 profile、launcher 或 TOML 字段；现有 TOML parser
  只接受 `server.gateway_ws_url`，自行加入 `wallet_origin` 会使配置检查失败。
  正式分发前应明确受控启动环境，或另行同时扩展配置、打包校验与测试。
- 缺少或无效 origin、HTTP 失败及过期身份只使新钱包余额显示为未知；不得回退
  到旧 `gold`，也不得用余额是否已加载决定服务器 Quote/Confirm/Query 权限。

## 已通过的本地测试

证据日志位于 `C:/mir2-build`。以下均为主代理原生串行运行的结果，各筛选集有
重叠，不相加为独立测试总数；未把浏览器阶段结果当作原生 EXE 结果。第43/44轮
记录保留为当时的通过范围，不能代替新增修复后的专项或当前完整实机流程。

| 检查 | 本轮结果 | 日志文件 |
| --- | --- | --- |
| Windows 整体编译检查 | 通过 | `hunting-native-windows-check-43.log` |
| 原生修理 UI | 10/10 | `hunting-native-ui-tests-43.log` |
| 独立 Runtime 修理 | 15/15 | `hunting-native-runtime-tests-44.log` |
| host/连接/协议 | 34/34 | `hunting-native-host-tests-44.log` |
| Runtime 相邻回归 | 39/39 | `hunting-native-runtime-adjacent-tests-44.log` |
| UI 相邻回归 | 18/18 | `hunting-native-ui-adjacent-tests-44.log` |
| 真实未决购买与修理互斥 | 1/1 | `hunting-native-purchase-mutual-tests-44.log` |

先前专项（45–55轮，历史范围）：

| 检查 | 已记录结果 | 日志文件 |
| --- | --- | --- |
| resume身份补发纯测试 | 5/5 | `hunting-native-resume-unit-tests-46.log` |
| 真实物理WS恢复、同grant及严格发送顺序 | 1/1 | `hunting-native-resume-ws-tests-46.log` |
| 隔离PG当前会话读取、原有效期及撤销 | 1/1 | `hunting-native-resume-pg-tests-45.log` |
| Runtime修理专项 | 15/15 | `hunting-native-runtime-tests-50.log` |
| Runtime相邻回归，含同owner的skill authority保留 | 41/41 | `hunting-native-runtime-adjacent-tests-50.log` |
| host/连接/协议 | 34/34 | `hunting-native-host-tests-50.log` |
| 首次producer激活前Begin，7个输入案例 | 1项测试通过 | `hunting-native-producer-tests-50.log` |
| 修理UI，含dialog→service分帧交接 | 15/15 | `hunting-native-ui-tests-55.log` |
| NPC dialog相邻回归 | 7/7 | `hunting-native-dialog-tests-55.log` |

实机先后发现并修复两处问题：首次购买 Begin 的完整模型应用不能用缺失的
skill authority 覆盖同owner既有权威；NPC dialog关闭与修理服务开启分帧到达时，
不能误关刚开启的修理面板。前者已通过Runtime相邻41项及host首次producer专项，
后者已通过修理UI15项和dialog相邻7项。它们的通过不能替代当前Bag拖入拒绝的修复。

## 第56轮实机与数据库边界

- 新EXE的第45、51、56轮实际链接分别记录于`hunting-native-game-build-45.log`、
  `hunting-native-game-build-51.log`、`hunting-native-game-build-56.log`；实机通过
  普通登录/StartGame/NPC进入，未借QA传送或旧EXE冒充本轮构建。
- 第56轮独立PG schema为`repair_network_250788_1792515744253798600_0`。
  `hunting-native-pg-quote-refused-56.json`由独立psql读取：修理结果0条、销毁0条、
  销毁金币0、绑定金币1,000,000、旧gold 1234、Bag耐久100/1000。
- `hunting-native-transport-56.json`记录`huntingRepairOwner`未发送，Confirm、
  Query、resume及故障省略/断线次数均为0；其中普通`npcPurchaseOwner`不能计作
  修理报价或收费。`hunting-native-quote-refused-56.png`保留当时的拒绝状态。
- 上述零扣款证明当前拒绝未收费，不证明成功修理、Pending原结果恢复、普通保存
  后冷登录或全进程重启回读已通过。原始完整库存schema修复后须用新EXE重新验证。

## 原生验证与后续关口

- 显式使用已安装的 `+1.95.0`，`--locked --offline`，Cargo jobs=2。
- 使用本聊天独立 CARGO_TARGET_DIR，保留原主树缓存、旧 EXE、HardLink 和运行中
  的游戏；链接使用新的输出名。不得启动 Docker/WSL/虚拟机或改虚拟化设置。
- 第45/51/56轮已有实际新EXE链接和有限GUI证据；尚无完整修理成功流程及候选包
  资格结论。服务端resume物理测试的通过也不代替新EXE实际断线操作。
- 已只读复核旧候选资源根的真实`0.map.gz`：gzip SHA256为
  `BBD02C7D0125FE78983E2DEA5F4AECC73B53D4B91517BDE6FDBEE9853BB183A5`，
  实际解出完整700×700原生布局。资源字节复用不继承旧候选的功能验收，Web地图
  可见也不代替原生布局。不得编造地图或借QA传送代替真实流程。
- 下一步在本机隔离PG fixture对合成RepairNetHero重新取得报价，30秒内显式确认
  一次；之后再核Pending关窗/原Query、完整应用、断线及最终普通保存/新登录，
  独立核对一条结果、一条销毁、一次扣款与保存耐久。服务端OutcomeUnknown的
  强制故障另作验收；不把报价成功或过期拒绝当作这些流程已通过。
- 本轮仅使用原生Windows进程，没有运行Docker、WSL或虚拟机；未关闭游戏或
  干扰其它窗口，未改动系统虚拟化设置、旧EXE或其HardLink。

完整狩猎经济任务仍未完成；默认关闭、K=0.1、0.72/1.08许可基准、旧gold定性和
远程资产保持。没有完整Windows玩家流程或Candidate百分比结论。
