# Windows 原生可玩垂直切片 Candidate 路线图

状态：Playable vertical-slice Candidate；自动闭环与六张原生证据已通过，最终 Crystal 1:1 人工视觉/手感接受仍开放
交付目标：`mir2-platform-windows.exe`（Bevy 0.19 原生窗口）
固定验收视口：1024×768
权威后端：现有 Gateway + Simulation
非交付目标：浏览器套壳、Tauri/WebView、仅能自动运行的 headless demo

> 2026-08-19 visual checkpoint: the WN-VIS-002 map route is implemented and
> WN-VIS-003/004 are complete. The exact
> native login screen is Accepted at 100/100 after structured review; the latest
> gates pass at Windows 80/80, shared Bevy 22/22 and native-ui 72/72. This does
> not change the functional contract below and does not close character select,
> in-game HUD, complete map/entity coverage, DPI or final human acceptance.

后续原生视觉收敛、Crystal 帧规格、Agent 写权限和验收门统一由
`docs/NATIVE-WINDOWS-VISUAL-PARITY-PLAN.md` 管理；本文件继续作为已完成的
功能竖切合同，不在这里重复或覆盖视觉执行队列。

## 1. 最终结果

本路线图只在以下玩家可见流程完整、可重复并通过验收后结束：

1. 玩家在 Windows 直接启动原生可执行文件，不预先设置账号密码环境变量。
2. 原生窗口显示可操作的登录界面；账号和密码由玩家输入并发送给真实 Gateway。
3. 登录成功后显示服务器返回的角色列表；玩家可选择已有角色，也可在空槽创建角色。
4. 玩家点击开始后进入比奇省；切换到游戏画面必须以服务端 `UserInformation`/世界初始化完成为准，不能把 `StartGame` ACK 当作进图成功。
5. 玩家能通过原生输入完成移动、转向、目标选择、普通攻击、NPC 交互和地面物品拾取。
6. 玩家先完成真实任务 1 的短引导，再接取真实任务 2，击杀稻草人获得任务物品，看到任务进度变化，回到 NPC 交付并收到真实奖励。
7. 玩家退出并再次登录后，角色位置、背包、属性和任务完成状态与服务端保存结果一致。
8. 同一提交不降低现有 Web 行为、Web 视觉门或 WASM/跨平台构建状态。

“能编译”“能打开地图”“能用环境变量自动登录”“能显示调试文本”都不等于完成。

## 2. 为什么这是单独的原生前端工作

Web 与 Windows 原生客户端可以共享协议、Gateway、Simulation、任务数据、地图数据、图集清单和纯数据 read model，但不能共享最终 UI 渲染代码：

- Web 使用 React/DOM/CSS。
- Windows 原生客户端使用 Bevy UI、Bevy Sprite、Winit 和 WGPU。
- 后端只决定“发生了什么”，不决定按钮、窗口、HUD、NPC 对话框如何画出来。
- 因此必须为 Bevy 写原生 UI 和输入适配，同时保持数据语义与 Web 一致。

共享边界如下：

```text
Crystal 数据/行为基准
        |
        v
Gateway JSON 协议 <----> Simulation 权威状态
        |                         |
        +-------------+-----------+
                      |
             client-core read models
                 /             \
                v               v
        Web React/CSS       Bevy 原生 UI/Sprite
          (对照面)          (本 Goal 交付面)
```

原生 UI 不复制服务器规则。它只展示权威状态并发送玩家意图；任务完成、掉落、经验、金币、角色保存都只能由 Gateway/Simulation 确认。

## 3. 可玩闭环选定任务

### 3.1 引导任务 1：Assistant's Request

- 任务 ID：1
- 起点：`Assistant_Jane`，地图 `0`，坐标 284,606
- 终点：`CraftsLady_Jude`，地图 `0`，坐标 294,619
- 内容：运送 `CannibalLeaves`
- 奖励：10 XP
- 用途：验证 NPC 选择、对话、接取、携带物和交付的最短闭环，并解锁任务 2

### 3.2 主验收任务 2：The CraftLady's Request

- 任务 ID：2
- 前置任务：1
- 接取 NPC：`CraftsLady_Jude`，地图 `0`，坐标 294,619
- 交付 NPC：`Assistant_Jane`，地图 `0`，坐标 284,606
- 内容：击杀 `Scarecrow`，取得 `GingerTea` 并送回 Assistant Jane
- 奖励：30 XP、200 Gold
- 服务端能力标签：`item-drop-objective`、`prerequisite-chain`

任务 2 是第一个验收主线，因为它能在很短的地图路线内同时覆盖：

- 接任务；
- 移动与地图碰撞；
- 目标选择；
- 普通攻击和受击/死亡反馈；
- 服务端掉落；
- 显式 `PickUp` 或服务端定义的任务物品处理；
- `NewQuestInfo` / `ChangeQuest` 进度更新；
- `FinishQuest` / `CompleteQuest`；
- XP、Gold 与任务状态持久化。

任务数据来源是 `docs/generated/quest-agent/warrior-1-50.json`，路线坐标来源是 `apps/web/scripts/quest-agent/policy.mjs`。不得在原生客户端硬编码“任务已完成”或直接增加奖励。

## 4. 产品状态机

原生窗口从启动到进游戏采用显式状态机：

```text
Boot
  -> Connecting
  -> Login
  -> Authenticating
  -> CharacterSelect
       -> CharacterCreate
       -> StartingGame
  -> InGame
  -> Disconnecting
  -> Login

任何联网状态
  -> ConnectionLost / Error
  -> Retry 或返回 Login
```

### 4.1 状态转换门

| 当前状态 | 玩家/网络事件 | 下一个状态 | 必须满足 |
|---|---|---|---|
| Boot | Bevy app 建立 | Connecting | 原生窗口已经可见，不等凭据才开窗 |
| Connecting | WebSocket ready | Login | 可显示服务器地址与连接状态 |
| Login | SubmitLogin | Authenticating | 账号/密码非空，密码不得写日志 |
| Authenticating | LoginSuccess | CharacterSelect | 使用服务端角色数组，不造本地角色 |
| Authenticating | LoginFailed | Login | 显示可读错误并允许重试 |
| CharacterSelect | Create | CharacterCreate | 只编辑本地表单，提交后等服务端结果 |
| CharacterCreate | NewCharacterSuccess | CharacterSelect | 合并服务端返回角色或重新登录刷新 |
| CharacterSelect | Start | StartingGame | 发送所选真实 `characterIndex` |
| StartingGame | StartGame ACK | StartingGame | ACK 仅表示请求被接收 |
| StartingGame | UserInformation + world bootstrap | InGame | 玩家、地图、位置和基础 read model 均有效 |
| InGame | Logout/窗口关闭 | Disconnecting | 先发送退出/断开，服务端保存权威状态 |
| 任意联网态 | Socket closed/error | ConnectionLost | 保留可理解错误，不静默卡死 |

### 4.2 UI read model

建议在 `client-bevy` 增加平台无关、可单元测试的模型：

```rust
enum NativeScreen {
    Connecting,
    Login,
    Authenticating,
    CharacterSelect,
    CharacterCreate,
    StartingGame,
    InGame,
    ConnectionLost,
}

struct LoginForm {
    account_id: String,
    password: String,
    focused_field: LoginField,
}

struct CharacterSummary {
    index: i32,
    name: String,
    level: u16,
    class: String,
    gender: String,
}

struct NativeShellModel {
    screen: NativeScreen,
    characters: Vec<CharacterSummary>,
    selected_character: Option<i32>,
    notice: Option<ShellNotice>,
}
```

建议的 UI 意图：

```rust
enum NativeUiIntent {
    Login { account_id: String, password: String },
    CreateCharacter { name: String, class: String, gender: String },
    DeleteCharacter { character_index: i32 },
    StartGame { character_index: i32 },
    RetryConnection,
    Logout,
    Interact { object_id: u32 },
    SelectNpcDialog { target: String },
    AcceptQuest { npc_index: i32, quest_index: i32 },
    Attack { object_id: u32 },
    PickUp { object_id: u32 },
    FinishQuest { quest_index: i32, selected_item_index: i32 },
}
```

建议的 Gateway 入站事件：

```rust
enum NativeGatewayEvent {
    Connected,
    LoginSucceeded { characters: Vec<CharacterSummary> },
    LoginFailed { message: String },
    CharacterCreated { character: CharacterSummary },
    CharacterDeleted { character_index: i32 },
    StartGameAcknowledged,
    PlayerBootstrapped,
    NpcDialogUpdated,
    QuestAdded,
    QuestChanged,
    QuestCompleted,
    Disconnected { reason: String },
}
```

字段名最终以现有 Gateway JSON 为准。UI 层不得依赖 `apps/simulation`，`client-core` 不得依赖 Bevy、DOM 或 Windows API。

## 5. 原生屏幕规格

### 5.1 登录界面

玩家必须看到：

- 1024×768 比例下完整背景，不拉伸关键构图；
- 账号文本框；
- 密码文本框，显示掩码；
- 登录按钮；
- 当前连接状态；
- 可读的登录失败信息；
- 键盘 Tab/Shift+Tab、Enter、Backspace，以及鼠标点击。

资源优先复用 `apps/web/public/bootstrap/login/chrsel-0-1024.webp` 或对应的可打包格式。若 Bevy 当前图片特性不能直接解码 WebP，构建期生成 PNG，不在运行时依赖浏览器解码。

安全规则：

- 不再要求 `MIR2_NATIVE_ACCOUNT` / `MIR2_NATIVE_PASSWORD` 才能启动窗口；
- 可以保留显式开发自动登录，但只有账号和密码均明确提供时启用；
- 不得回退到 `demo`；
- 不得打印密码或把凭据写入截图 fixture；
- 非 loopback 的明文 `ws://` 继续拒绝。

### 5.2 角色选择/创建

玩家必须看到：

- 服务端角色槽；
- 名称、职业、性别、等级；
- 当前选中态；
- 创建角色入口；
- 开始游戏按钮；
- 创建失败/重名/非法名称的服务端错误；
- 空角色列表不会自动发送 `StartGame(0)`。

角色创建必须走现有 `NewCharacter` Gateway 命令，选择必须使用服务端返回的 `index`。

### 5.3 游戏 HUD

第一版可玩 HUD 至少包含：

- Crystal 风格底部主框架；
- HP/MP 与数值；
- 玩家名、等级、金币；
- 物品快捷栏或最小可操作背包入口；
- 聊天区域；
- 当前任务摘要；
- NPC 对话窗口；
- 目标名称/HP；
- 断线/错误覆盖层；
- 世界与 UI 正确裁剪，不让 UI 根节点遮挡世界输入。

画面可以分阶段逼近 Crystal，但不能用开发者调试面板代替玩家 HUD。

### 5.4 NPC、任务、战斗与拾取

输入最低要求：

- WASD/方向键：走；
- Shift + 移动或明确按键：跑；
- 鼠标左键：选择/移动/与 NPC 交互，行为按对象类型路由；
- 攻击键或对怪物左键：发送 `Attack { objectId }`；
- 拾取键或地面物品点击：发送 `PickUp { objectId }` / `PickUpTile`；
- NPC 对话选项：发送 `SelectNpcDialog`；
- 接取/交付按钮：发送 `AcceptQuest` / `FinishQuest`。

以下反馈都必须来自服务端事件后再显示成功：

- 攻击动画与目标 HP；
- 怪物死亡；
- 地面物品出现/消失；
- 背包变化；
- 任务进度；
- 完成任务；
- XP、金币和等级变化。

## 6. 代码边界和预计写集

### 6.1 主设计 Agent 独占的高冲突/高风险文件

- `apps/game-client/platform-windows/src/main.rs`
- `apps/game-client/platform-windows/src/gateway.rs`
- `apps/game-client/platform-windows/src/session_config.rs`
- `apps/game-client/client-bevy/src/lib.rs`
- `apps/game-client/runtime/src/lib.rs`（仅在确有必要时）
- 认证、重连、协议路由和最终跨分支集成

这些文件同一轮只允许一个 writer。认证和安全相关改动必须由 frontier 主 Agent 设计并复核。

### 6.2 Spark UI worker 写集

仅新增或修改低冲突的 `client-bevy` UI 模块，例如：

- `apps/game-client/client-bevy/src/native_shell.rs`
- `apps/game-client/client-bevy/src/login_screen.rs`
- `apps/game-client/client-bevy/src/character_select.rs`
- `apps/game-client/client-bevy/src/native_theme.rs`

职责：纯状态、Bevy UI、键鼠焦点、按钮意图和单元测试。不得连接 Gateway，不得编辑 `lib.rs`。

### 6.3 Spark gameplay UI worker 写集

仅新增低冲突模块，例如：

- `apps/game-client/client-bevy/src/npc_dialog.rs`
- `apps/game-client/client-bevy/src/quest_panel.rs`
- `apps/game-client/client-bevy/src/target_panel.rs`

职责：把结构化 read model 渲染为 NPC/任务/目标 UI，并发出意图。不得判定任务完成，不得编辑 Gateway。

### 6.4 Spark test/fixture worker 写集

仅新增测试与 fixture 文档/脚本，例如：

- `apps/game-client/platform-windows/tests/native_shell_smoke.rs`
- `apps/game-client/platform-windows/tests/native_gateway_contract.rs`
- `apps/game-client/platform-windows/tests/fixtures/`
- `docs/qa/NATIVE-WINDOWS-PLAYABLE-CHECKLIST.md`

职责：可重复 fixture、协议样例、状态机 smoke 和人工验收清单。不得把 QA/admin 命令暴露给正常客户端。

## 7. 分阶段执行路线

### G0 — 可重复构建基线

目标：任何后续失败都能判断是新增回归还是既有问题。

已完成：

- 清理约 33 GiB 可再生 Cargo target 缓存，恢复工作盘空间；
- 统一 standalone runtime 的 `windows` 依赖到 0.62.2，消除 WGPU/GPU allocator COM 类型冲突；
- `mir2-bevy-runtime`：131 tests passed；
- `mir2-client-bevy`：22 tests passed；
- `mir2-client-core`：12 tests passed；
- `mir2-platform-windows`：33 tests passed。

退出门：

- 上述四套测试连续通过；
- 固定共享 Cargo target 路径仅作为本机运行参数，不提交绝对路径；
- 记录 Web/WASM 当前门状态；
- 保留用户现有未提交地图图块和对照文档，不覆盖、不回滚。

### G1 — 状态机与双向协议适配

目标：先把“登录—选角—进图”变成可测试的数据流，再画 UI。

工作项：

1. 增加 `NativeScreen`、角色摘要、通知、登录/创建表单模型。
2. 将只有 Walk/Run/Turn 的 `PlayerIntent` 扩展为完整 `GatewayCommand`。
3. 增加入站 `NativeGatewayEvent` 通道，Bevy 主线程消费事件。
4. 解析 `LoginSuccess` 的角色数组。
5. 解析 `NewCharacterSuccess`、`DeleteCharacterSuccess`、失败事件。
6. `StartGame` 改为玩家选择后才发送。
7. 只有收到 `UserInformation` 并完成世界 bootstrap 后才切 `InGame`。
8. 为登录失败、空角色、重连、重复 ACK、乱序断线增加状态机测试。

退出门：不启动渲染器也能用测试证明所有合法和错误转换。

### G2 — 原生登录与角色界面

目标：玩家第一次真正可以在 EXE 内操作账号与角色。

工作项：

1. 窗口先启动，默认进入 Connecting/Login。
2. 实现账号、密码输入焦点和掩码。
3. 实现登录、重试与错误提示。
4. 实现角色列表、选中态和开始按钮。
5. 实现角色创建的名称、职业、性别选择。
6. 隐藏游戏世界/HUD，直到进入 `InGame`。
7. 打包登录和选角所需资源。
8. 建立无后端 fixture 截图模式，但 fixture 只能驱动 UI 状态，不能作为真实 E2E 通过证据。

退出门：固定 1024×768 下可人工完成登录和角色选择/创建；真实 E2E 使用 Gateway 返回数据。

### G3 — NPC、任务、战斗和物品协议

目标：打通任务 1 + 任务 2 的真实协议闭环。

工作项：

1. 世界对象选择：玩家、NPC、怪物、地面物品使用服务端 `objectId`。
2. NPC 交互和 `NPCResponse` 对话页适配。
3. `AcceptQuest`、`FinishQuest` 命令。
4. `NewQuestInfo`、`ChangeQuest`、`CompleteQuest` read model。
5. `Attack` 命令和服务端攻击/HP/死亡反馈。
6. `PickUp`/`PickUpTile` 与背包更新。
7. 当前目标、任务追踪、NPC 对话原生 UI。
8. 验证任务 1 解锁任务 2，而非 fixture 直接篡改客户端任务状态。

退出门：真实后端上从接任务 1 到完成任务 2，全程不使用客户端作弊状态。

### G4 — 地图、图集与 HUD 可见质量

目标：功能闭环不再被占位色块、重复渲染或裁切问题破坏。

工作项：

1. 固定地图摄像机、世界缩放和 1024×768 viewport。
2. 补齐任务路线需要的地图 tile/object atlas。
3. 补齐玩家、稻草人、NPC、地面任务物品的 entity atlas。
4. 消除 fallback 与 atlas 双重渲染。
5. 把基础 HUD 布局调整到 Crystal 参考位置。
6. 在 Windows 100%、125%、150% DPI 下验证逻辑尺寸。
7. 扩展 `package-assets.sh`，使发布目录包含登录背景、任务路线地图和实体资源。

退出门：六类验收截图都没有关键占位块、黑屏、地图洞、双影或 UI 越界。

### G5 — 持久化与失败恢复

目标：从“能演示一次”提升到“正常退出后还能继续玩”。

工作项：

1. 关闭窗口/Logout 时走安全离开流程。
2. 等待或明确触发服务端保存，不在客户端保存权威角色副本。
3. 重新登录后核对位置、等级、金币、背包和任务状态。
4. 已完成任务 2 不再重复给奖励。
5. 任务未完成中途断线后恢复正确进度。
6. 网络中断时显示重试/返回登录，不冻结窗口。

退出门：两次独立原生进程之间状态一致，且重复交付不会重复奖励。

### G6 — Candidate 验收与 Web 保护

目标：形成可以让人工直接判断的证据包。

必须产出六张 1024×768 Windows 原生截图：

1. `01-login.png`：账号/密码/登录按钮和连接状态；
2. `02-character-select.png`：真实角色列表与选中角色；
3. `03-in-game.png`：比奇省、角色、地图和原生 HUD；
4. `04-quest-accepted.png`：任务 2 的 NPC 对话和已接取任务；
5. `05-combat-progress.png`：稻草人战斗、目标 HP、任务进度或 GingerTea 掉落/物品状态；
6. `06-quest-complete.png`：交付成功，XP/Gold/完成状态可见。

额外建议截图：角色创建界面、断线恢复界面、重登后的已完成任务状态。

自动化门：

- 原生 shell 状态机 tests；
- Gateway JSON contract tests；
- Bevy UI headless smoke；
- runtime/client-core/client-bevy/platform-windows 全测试；
- Windows native `cargo check`/release build；
- Web/WASM build/test；
- 现有 Web 登录/选角/游戏视觉门；
- 任务 fixture smoke 与重复奖励断言。

人工清单必须记录每一步的输入、预期服务端事件、可见结果和截图文件，而不只写“看起来正常”。

## 8. Agent 分工与模型策略

### 主设计 Agent（frontier reasoning）

负责：

- Goal 和架构口径；
- 关键协议、认证、安全与生命周期；
- 高冲突文件；
- 子 Agent 写集隔离；
- 每批 patch review、整合和回归；
- 真实 EXE 启动、人工操作、截图和最终 Candidate 判定。

强度：跨模块集成与安全使用 high/xhigh；最终验收使用 xhigh。

### GPT-5.3-Codex-Spark worker

适合：

- 一个明确模块的 Bevy UI；
- 纯数据模型与单元测试；
- JSON fixture/contract tests；
- 机械资源清单；
- 文档与 QA 清单；
- 根据截图做一轮一个变量的快速 UI 微调。

默认强度：

- medium：探索、fixture、文档、QA；
- high：常规 Bevy UI 实现和测试；
- xhigh：仅限边界清晰但风险较高的实现，仍由主 Agent 复核。

Spark 不独立负责认证、安全、协议总线重构、跨分支合并或最终验收。未确认在当前账户可用的 Grok、Gemini 或 DeepSeek 只可作为外部建议来源，不能成为交付链依赖，也不能替代本地测试。

## 9. 每轮并行规则

每轮执行遵循：

1. 主 Agent 先锁定接口与高冲突写集。
2. 最多一个 Agent 修改任一高冲突文件。
3. Spark worker 只领取一个可独立编译/测试的垂直模块。
4. Explorer 默认只读；转为 writer 前必须重新分配写集。
5. 子 Agent 完成后主 Agent检查 diff、运行目标测试，再合入下一层接口。
6. 不让两个 worker 同时编辑 `client-bevy/src/lib.rs`、`platform-windows/src/gateway.rs` 或 `runtime/src/lib.rs`。
7. 不回滚用户已有地图图块、对照截图、脚本或 Crystal 子模块改动。

首轮实现分配：

| Lane | Worker | 写集 | 交付 |
|---|---|---|---|
| A | Spark high | 新建 `client-bevy` shell/state 文件 | 状态、表单、角色模型、意图和纯单测 |
| B | Spark high | 新建 `client-bevy` quest/NPC read-model 文件 | NPC/任务/目标展示模型和纯单测 |
| C | Spark medium | 新建 platform contract fixture/tests | 登录、角色、任务 Gateway JSON 样例和解析断言 |
| Lead | frontier xhigh | gateway/main/session config/lib 集成 | 双向通道、状态转换、安全和完整构建 |

## 10. 回归保护矩阵

| 领域 | 原生必须通过 | Web 不得退化 |
|---|---|---|
| 认证 | 可见登录、失败提示、无默认账号 | 现有登录流程不变 |
| 角色 | 真实列表、创建、选择 | Web 角色列表契约不变 |
| 世界 | 地图/对象/移动可见 | Web snapshot/event 语义不变 |
| 战斗 | 目标、攻击、HP、死亡 | Web Attack 协议不变 |
| 物品 | 掉落、拾取、背包更新 | Web PickUp 协议不变 |
| 任务 | 接取、进度、完成、奖励 | Web quest-agent 路线继续通过 |
| 资源 | 发布包离线可找资源 | 不移动/破坏 Web public 资源 |
| 构建 | Windows release 可运行 | WASM/Web CI 绿色 |

## 11. 停止条件与不可接受捷径

只有出现以下情况才暂停并要求人工决定：

- 需要改变真实账号数据或删除不可恢复的数据；
- 服务端协议出现两种互斥语义且仓库证据无法判定；
- 需要新增具有生产权限的凭据或外部服务；
- 用户已有改动与必要实现直接冲突且无法绕开。

不可接受捷径：

- 用 WebView/Tauri 页面截图声称是原生 UI；
- 依赖环境变量自动登录来跳过登录界面；
- 在每个本地 session 伪造远程玩家；
- 在客户端直接改 XP、金币、任务完成或背包；
- 用 `demo` 账号默认回退；
- 把 QA/admin 命令暴露给普通客户端；
- 只跑 headless 测试，不启动真实 Windows EXE；
- 只截静态 fixture 图，不做真实 Gateway E2E。

## 12. Definition of Done

此 Goal 只有同时满足以下条件才可标记 complete：

- `mir2-platform-windows.exe` 从可见登录页启动；
- 可用真实凭据登录真实 Gateway；
- 可选择或创建角色；
- 可进入比奇省并人工移动；
- 可完成任务 1 和任务 2；
- 任务 2 包含真实战斗、掉落/物品处理、进度和交付；
- 可见 HUD/NPC/任务/目标 UI 足以让玩家独立操作；
- 重登后状态持久；
- 六张规定截图齐全；
- 人工操作清单和自动 smoke 齐全；
- Windows、共享 Rust、Web/WASM 与视觉回归门全部通过；
- 没有覆盖或回滚用户无关改动；
- 主设计 Agent 完成最终 diff、安全和行为复核。

## 13. 2026-08-19 Candidate 实证

Windows 交付面现在是独立的 `mir2-platform-windows.exe`：Bevy/Winit/WGPU 创建并渲染原生窗口，没有 WebView、Tauri 或浏览器 DOM。默认启动不需要环境变量凭据；环境变量自动登录只保留为显式开发/截图入口，且 Debug 输出继续脱敏。

一次全新账号、全新角色的同进程真实 Gateway smoke 已以 `ok: true`、退出码 0 完成：

- 可见流程对应登录、角色创建/选择、StartGame 和 `UserInformation` 世界门；
- 任务 1 从 Jane 接取并在 CraftsLady 交付；
- 任务 2 接取后通过正常 Walk 意图到达野外；
- 对同一 Scarecrow 的 20 次有效攻击把权威 HP 从 100% 降到 0%；
- 击杀产生 `ObjectDied`、30 EXP 和 `GainedItem(item_index=1112, count=1)`；
- 玩家死亡时通过正式 `TownRevive`、`Revived/ObjectRevived`、`UserLocation` 和正 HP 快照恢复；
- 回到 Jane 后获得 30 EXP、200 Gold、GoldenPendant 和 CopperRing；
- Logout/Login/StartGame 后坐标、经验、金币、背包和任务 1/2 完成状态一致。

Crystal `Q` 标记任务物品在共享 Zone 中按原始语义直接进入合资格玩家的任务包，不先成为普通地面对象；因此 GingerTea 的权威证据是击杀结算后的 `GainedItem`。普通地面物品仍走 `ObjectItem` 与 `PickUp/PickUpTile`，由原生命令契约、Gateway 事务和 Simulation 回归保护。

固定 1024×768 的六张截图、SHA-256、人工输入/权威事件/可见结果对照表，以及已知视觉限制见 `docs/NATIVE-WINDOWS-PLAYER-QA.md`。

本轮最终自动回归：

- Windows 原生：73/73 tests；锁定 Release 构建成功；EXE SHA-256 `AA0BAD31216604D532B9F4C7E66EFE8A6EFC89C74DC65AC9C535BD12E64B2FF1`；
- `mir2-client-bevy`：默认 feature 22/22，`native-ui` 56/56；
- WASM：`webgl2` 与 `webgpu` 两套 `wasm32-unknown-unknown` check 均通过；
- Web：TypeScript typecheck 通过，Bevy runtime policy 5/5；
- 在线保护：Web 根路径 200，Gateway `/health` 的 HTTP/WS/TCP stub 均 ready，Crystal map API 18/18 且缺失资源为 0；
- Rust fmt、Node syntax 与 `git diff --check` 通过。

该状态只关闭“Windows 原生可见、可操作、可完成任务且可持久化”的垂直切片。截图仍暴露 starter entity atlas 动作帧/锚点覆盖和原生 HUD 像素级复刻差距，所以不得把它描述为最终 Crystal 1:1 Accepted。
