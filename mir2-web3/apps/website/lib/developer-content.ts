import type { DocumentationArticle } from "@/lib/documentation-types";

export const developerArticles: DocumentationArticle[] = [
  {
    slug: "overview", title: "产品与架构", description: "先分清游戏、通用工具与钱包，再选择接入范围。", status: "testing", environment: "Numeron 当前测试实现 / Dubhe 通用文档",
    sections: [
      { id: "products", title: "三个产品，三个职责", table: { headers: ["产品", "职责"], rows: [["Numeron", "由个人运营，Obelisk Labs, Inc. 受托开发。涵盖本游戏的账号、角色、世界状态和玩法规则。"], ["Dubhe", "支持应用开发的通用引擎与工具体系；具体模块以各自文档和版本为准。"], ["Dubhe Wallet", "钱包与身份、交易确认入口；连接、登录与交易授权分别处理。"]] } },
      { id: "authority", title: "本游戏的权威边界", paragraphs: ["客户端提交操作意图，游戏服务端验证身份与规则，并维护角色和共享世界状态。网页、钱包地址或链上资产本身，都不提供任意修改角色、道具或战斗结果的权限。", "官网、游戏入口和筹备中的推广合作说明属于同一官网体系，但应用有各自的构建与服务边界。部署官网页面，不等于部署游戏服务器或升级合约。"] },
      { id: "division", title: "避免重复维护通用教程", paragraphs: ["本组文档说明 Numeron 当前可用的流程与限制。Dubhe SDK、合约工具与通用授权模型，请阅读对应的 Dubhe 原文；不要把工具库提供的能力直接当成本游戏已经开放的接口。"], links: [{ label: "Dubhe 通用文档", href: "https://dubhe-docs.obelisk.build/" }, { label: "Numeron 开放边界", href: "developers/capabilities" }] },
    ],
  },
  {
    slug: "capabilities", title: "开放能力与权限", description: "当前能够读取什么，以及哪些操作需要独立授权。", status: "testing", environment: "官网公开只读接口 / 游戏认证边界",
    sections: [
      { id: "matrix", title: "先按能力选择入口", table: { headers: ["能力", "当前范围", "状态"], rows: [["官网健康探测", "GET /api/game-status；无需密钥，只有健康摘要。", "已上线：官网，2026-11-02 只读探测通过"], ["钱包登录与账号关联", "现有游戏入口中的完整扫码流程；由服务端验证身份。", "测试中：不是通用第三方登录 SDK"], ["个人角色与游戏操作", "要求有效登录和对应权限；不是按钱包地址公开查询。", "测试中：当前游戏客户端协议"], ["管理员操作与世界写入", "内部权限边界；不向普通客户端或第三方默认开放。", "没有公开接入契约"], ["第三方衍生玩法", "尚未形成可公开调用的 Numeron 扩展 API。", "规划中，不可调用"]] } },
      { id: "least-privilege", title: "从最小权限开始", items: ["最小示例只读取状态，不创建账号、订单或交易，不访问角色隐私。", "钱包连接只返回或确认账户，不等于已经获得游戏会话。", "登录身份只在有效期和目标服务内使用，不等于拥有转账或后台管理权限。", "不要重放扫码会话、猜测玩家身份或用调试路径绕过认证。"] },
      { id: "not-an-sdk", title: "这里还不是游戏写入 SDK", paragraphs: ["当前没有在本组文档中发布第三方角色修改、装备铸造、市场写入或世界事件创建 API。没有列出的端点不应被视为稳定、开放或允许调用。", "需要更大范围接入时，先通过现有支持入口说明用途、需要的数据和权限；在获得明确接入约定前保持只读。"], links: [{ label: "运行只读示例", href: "developers/quickstart" }, { label: "贡献与反馈", href: "developers/contributing" }] },
    ],
  },
  {
    slug: "environment", title: "环境准备", description: "运行公开示例只需 Node.js；本地官网开发再安装工程依赖。", status: "testing", environment: "Node.js 22+ / 可选的本地官网",
    sections: [
      { id: "versions", title: "第一版核对的工具版本", table: { headers: ["项目", "版本与用途"], rows: [["Node.js", ">=22；本轮本机使用 22.18.0，示例使用原生 fetch。"], ["Next.js", "官网依赖锁定 16.3.3。"], ["React / React DOM", "官网依赖锁定 19.2.3。"], ["TypeScript", "官网开发依赖锁定 5.9.3。"], ["依赖安装", "官网已提供 package-lock.json，以下命令使用 npm ci。"]] }, note: "这些是官网与只读示例的版本，不是 Dubhe SDK 或链上合约的兼容性保证。仅运行示例时无需安装 Next.js。" },
      { id: "local-site", title: "可选：开发官网时再启动本地预览", paragraphs: ["只运行状态读取示例时，直接进入“最小接入”，无需下载或启动官网工程。", "如果你已有访问权限，并希望修改官网源码，在现有官网工程目录执行以下命令。它只启动官网本地预览；不会启动游戏服务器、数据库或 Docker。"], code: { language: "PowerShell", value: "npm ci\nnpm run dev" }, links: [{ label: "直接运行公开示例", href: "developers/quickstart" }, { label: "本地玩家指南（3080）", href: "http://127.0.0.1:3080/zh-CN/guide" }, { label: "本地开发者文档（3080）", href: "http://127.0.0.1:3080/zh-CN/developers" }] },
      { id: "upstream", title: "健康摘要依赖哪个上游？", paragraphs: ["官网通过服务端配置 GAME_HEALTH_URL 访问上游 /health，并把结果转换为精简摘要。未配置本地上游时，可能使用工程现有的远端默认值。", "本地官网可运行、状态摘要为 degraded，也属于正常的降级行为。不要为显示绿色状态而绕过证书验证、关闭认证或填写真实凭据。"], links: [{ label: "Dubhe 通用环境准备", href: "https://dubhe-docs.obelisk.build/dubhe/sui/quick-start/prerequisites" }] },
    ],
  },
  {
    slug: "quickstart", title: "最小接入：读取服务状态", description: "下载公开源码，用 Node.js 读取官网健康摘要；无需密钥或工程依赖。", status: "testing", environment: "公开官网只读示例 / Node.js 22+",
    sections: [
      { id: "run", title: "1. 下载并运行公开示例", paragraphs: ["准备 Node.js 22 或更新版本，在一个临时目录打开 PowerShell，执行以下命令。不需要启动本地官网、安装 npm 依赖、连接钱包或提供 API Key。", "下载的是本站公开的完整示例源码；可以先打开 read-status.mjs 检查，再运行最后一行。"], code: { language: "PowerShell", value: "node --version\ncurl.exe --fail -o read-status.mjs https://numeron.obelisk.build/zh-CN/docs-examples/read-status.mjs\nnode .\\read-status.mjs https://numeron.obelisk.build" }, links: [{ label: "查看示例源码（.mjs）", href: "/zh-CN/docs-examples/read-status.mjs" }, { label: "工具版本与可选本地环境", href: "developers/environment" }] },
      { id: "result", title: "2. 读懂返回结果", code: { language: "JSON · 输出结构示意，非实时状态", value: '{\n  "endpoint": "https://numeron.obelisk.build/api/game-status",\n  "state": "healthy",\n  "checkedAt": "<ISO 8601 时间>",\n  "latencyMs": 120,\n  "scope": "Configured game-health probe only; not a login or payment check."\n}' }, paragraphs: ["state 为 healthy 表示官网这次配置的上游健康请求获得成功响应；degraded 表示上游失败、超时或不可达。接口 HTTP 200 本身不等于上游健康，需要读 ok 字段。", "checkedAt 和 latencyMs 是本次探测时间与耗时。这不是在线人数、世界状态、登录成功率或支付到账的证明。", "公开官网已于 2026-11-02 返回过有效健康摘要。状态随时间变化；实际结果以本次命令输出为准。请低频读取并缓存展示结果，不要把健康探测当作压力测试。"] },
      { id: "public-site", title: "3. 可选：切换到本地官网", code: { language: "PowerShell", value: "node .\\read-status.mjs http://127.0.0.1:3080" }, paragraphs: ["仅在你已启动本地官网时使用这条命令。示例接收一个官网 origin，自动读取它的 /api/game-status；不需要修改源码。", "本地和公开官网可能使用不同的上游健康配置。不要为了得到 healthy 而关闭证书检查或绕过权限。"], links: [{ label: "怎样启动本地官网", href: "developers/environment" }] },
      { id: "failure", title: "失败时怎样处理", items: ["示例限制单次等待时间；网络错误、非成功 HTTP、重定向或响应结构不兼容时，退出码为 1。", "合法的 degraded 响应仍是一次成功读取，退出码为 0；界面应显示降级，而非伪造健康。", "不要在日志、截图或示例参数中附带密码、访问令牌、助记词或私钥。"], links: [{ label: "接口与兼容边界", href: "developers/interfaces" }, { label: "错误处理与安全", href: "developers/security" }] },
    ],
  },
  {
    slug: "identity", title: "身份、角色与钱包登录", description: "了解现有扫码登录如何被服务端验证，以及它不授予什么权限。", status: "testing", environment: "现有 Numeron 游戏登录流程",
    sections: [
      { id: "flow", title: "现有登录链路", ordered: true, items: ["官方客户端或页面创建有有效期的登录意图，生成本次扫码会话。浏览器不能随意指定中继、网络或身份凭据。", "手机 Dubhe Wallet 展示登录请求，玩家核对后确认个人消息签名。", "服务端核对会话、消息、签名地址、公钥、网络与有效时间，并验证签名；仅提交一个钱包地址不足以登录。", "通过验证后，渠道身份映射到游戏规范账号，再交换短期会话凭据。首次登录可建立账号关联，不会自动创建角色。", "角色列表、创建角色与进入游戏继续通过游戏认证和角色权限检查。"] },
      { id: "permissions", title: "连接、登录、操作授权分别处理", table: { headers: ["动作", "含义"], rows: [["连接钱包", "让应用识别玩家选择的钱包账户；不自动付款。"], ["网页登录签名", "验证一次有范围、有时效的身份请求。"], ["限权操作授权 / session key", "仅在产品明确支持时，按限定权限与有效期授权；不是全权密钥。"], ["交易签名", "针对具体网络、接收方与金额确认交易。"], ["恢复密钥 / 助记词", "钱包全权恢复材料；应用、客服和示例均不应收集。"]] } },
      { id: "integrations", title: "第三方接入的当前范围", paragraphs: ["以上用于解释 Numeron 已有登录流程，不是给第三方复制内部登录端点的授权。当前没有在这里发布跨站扫码登录 SDK、角色身份查询 API 或批量账号创建 API。", "登录签名与链上 USDC 付款使用不同的确认与核账流程。不要把登录回执当成支付回执，也不要把恢复密钥当作 session key。"], links: [{ label: "Dubhe 通用 session key 文档", href: "https://dubhe-docs.obelisk.build/dubhe/sui/contracts/session-keys" }] },
    ],
  },
  {
    slug: "interfaces", title: "已有接口、SDK 与数据", description: "只列出有实现和证据的范围，避免把内部协议当成开放契约。", status: "live", environment: "官网公开只读 GET /api/game-status（2026-11-02 探测）",
    sections: [
      { id: "status-api", title: "GET /api/game-status", paragraphs: ["此接口属于官网，不要求登录或 API Key，没有请求体。它读取配置的游戏健康上游，再返回下面四个字段；不会返回角色、账号或订单信息。", "官网浏览器页面使用同源请求；接口当前没有配置面向任意第三方站点的 CORS。外部最小示例因此使用 Node.js，不假设跨域浏览器调用已经开放。"], table: { headers: ["字段", "类型", "含义"], rows: [["ok", "boolean", "上游 HTTP 响应是否成功；异常和超时返回 false。"], ["checkedAt", "string", "ISO 8601 格式的探测完成时间。"], ["latencyMs", "number", "这次探测的毫秒耗时。"], ["source", '"game-health"', "摘要来源标识。"]] }, note: "当前接口对降级状态也返回 HTTP 200，并设置 cache-control: no-store。网络可达、HTTP 成功和上游健康需要分别判断。该接口目前没有公开的版本号或服务等级承诺。" },
      { id: "game-data", title: "角色和游戏事件不是公开数据源", paragraphs: ["游戏客户端已有认证协议、角色数据与事件处理，但本版没有把它们声明为稳定第三方 API。不得仅凭钱包地址读取他人角色、重放内部凭据或自行发送写入世界的命令。", "图鉴中的示意数据、链上公开对象、后台管理数据与游戏权威状态也有不同来源。链上可读不等于数据能直接驱动角色装备、金币或战斗。"] },
      { id: "sdk", title: "通用 SDK 与 Numeron 配置", paragraphs: ["Dubhe 提供自己的客户端与合约工具文档。使用通用 SDK 前应锁定对应网络、包版本与应用权限；本官网的 package.json 没有为第三方声明一个 Numeron 游戏 SDK 依赖。", "当前没有在这里发布事件订阅地址、角色查询参数、测试合约地址或世界写入接口。需要这些能力时先确认明确接入约定。"], links: [{ label: "Dubhe 客户端文档", href: "https://dubhe-docs.obelisk.build/dubhe/sui/client" }, { label: "运行已验证的读取示例", href: "developers/quickstart" }] },
    ],
  },
  {
    slug: "testing", title: "测试环境与测试资产", description: "把本地、游戏测试服和链上测试网分别核对。", status: "testing", environment: "本地官网 / 指定 TEST 环境 / Sui Testnet",
    sections: [
      { id: "environments", title: "环境名称不能代替核实", table: { headers: ["环境", "用途与边界"], rows: [["本地官网", "文档与只读示例预览；上游健康地址按本地服务端配置。"], ["游戏 TEST 环境", "验证账号、角色与具体游戏流程；功能开关按该环境配置。"], ["Sui Testnet", "测试链上签名、Gas 与测试 USDC 等资产；不是主网。"], ["正式商业环境", "本组测试文档不构成真实收费或资金操作的验收。"]] } },
      { id: "assets", title: "测试资产的含义", paragraphs: ["测试 SUI 支付测试网络 Gas，测试 USDC 用于指定充值或付款验收。测试资产没有真实货币价值，不能据此宣传收益或可兑换保证。", "月卡、充值和推广合作是不同业务步骤。推广合作仍在筹备，参与资格、支持地区、佣金与结算规则尚待审核。只读状态示例不验证计佣资格或付款，也不代表可领取佣金。"] },
      { id: "fixtures", title: "先在隔离环境验证失败情况", items: ["读取示例应覆盖 healthy、degraded、网络超时、非成功 HTTP 与响应结构变化。", "不要为文档演示调用真实收费、签名交易或账号写入。", "未来需要测试写入时，先确认环境、测试账号、授权范围和可恢复的数据边界。"], links: [{ label: "玩家测试与数据说明", href: "guide/playtest" }] },
    ],
  },
  {
    slug: "security", title: "兼容、错误处理与安全", description: "校验响应结构、按错误类别处理，并保留最小权限。", status: "testing", environment: "只读示例与现有认证流程",
    sections: [
      { id: "compatibility", title: "不要依赖未声明的稳定性", paragraphs: ["文档更新时间是内容核对时间，不是服务器、客户端或合约版本号。代码合入、安装包版本和指定环境部署分别验证。", "当前健康接口没有公开版本化契约。客户端应验证必需字段、容忍额外字段、处理无法解析的响应；字段不兼容时停止正常展示并提示重新核对文档。"] },
      { id: "errors", title: "错误与降级策略", table: { headers: ["情况", "建议处理"], rows: [["ok=false", "显示降级和最近探测时间，不能显示健康。"], ["网络失败或超时", "终止本次等待；人工重试或有上限地退避，不无限循环。"], ["401 / 403", "核对目标服务与权限；不要通过关闭认证绕过。"], ["429", "遵循服务端提示并减少频率。"], ["非 JSON / 字段不兼容", "按不兼容处理，记录脱敏错误并核对接口。"], ["签名失效或会话过期", "重新开始完整登录流程，不重放旧签名。"]] } },
      { id: "secrets", title: "密钥不进入浏览器和文档", items: ["不要收集、复制或写入助记词、恢复密钥、私钥和 API 密钥。", "登录回执与会话令牌不能放入 URL、公开日志或客服截图。", "限权操作授权必须明确范围、有效期与撤销方式；恢复密钥不能作为 session key。", "服务端验证身份和权限；浏览器声称已登录、传入一个账号 ID 或钱包地址都不能代替验证。", "示例不需要凭据。未来示例只使用明确的占位值与测试资产，不输出敏感配置。"] },
    ],
  },
  {
    slug: "contributing", title: "贡献与反馈", description: "提交可复现问题和明确接入需求，帮助文档与实现保持一致。", status: "testing", environment: "现有官网支持入口 / Dubhe 公开文档仓库",
    sections: [
      { id: "feedback", title: "Numeron 的反馈入口", paragraphs: ["通过官网现有支持页报告文档错误、示例失败和接入需求。这里不添加未经核实的 GitHub 仓库、社群邀请或联系邮箱。"], items: ["包含页面地址、核对日期、平台与工具版本。", "说明预期、实际结果和最小复现步骤，附脱敏后的输出。", "接入需求请说明用途、需要读取或修改的数据、权限和目标测试环境。"], links: [{ label: "打开官网支持页", href: "support" }, { label: "查看更新记录", href: "updates" }] },
      { id: "dubhe-docs", title: "通用能力回到通用文档", paragraphs: ["Dubhe SDK、通用合约工具和授权模型的问题，优先查阅 Dubhe 文档原文。其公开文档仓库可用于查看和反馈通用文档；不代表 Numeron 游戏源码已经公开接受所有类型的贡献。", "本轮没有确认到可公开访问的钱包专用开发文档仓库，因此这里只链接钱包产品入口，不发布不可访问的旧教程。"], links: [{ label: "Dubhe 文档", href: "https://dubhe-docs.obelisk.build/" }, { label: "Dubhe 公开文档源码", href: "https://github.com/0xobelisk/dubhe-docs" }, { label: "Dubhe Wallet 产品入口", href: "https://wallet.dubhe.obelisk.build/" }] },
    ],
  },
  {
    slug: "roadmap", title: "规划与尚未开放的能力", description: "将产品方向与已经可以调用的接口分开。", status: "planned", environment: "当前不可调用 / 无承诺开放时间",
    sections: [
      { id: "future", title: "这些内容不属于当前接入契约", items: ["面向第三方衍生玩法的公共扩展 SDK、世界写入权限和创作分发。", "更广泛的角色数据读取、事件订阅与工具接入，需要先定义数据权限与兼容契约。", "势力玩法与玩家经济的后续扩展仍需产品批准和指定环境验证，没有公开接入契约。"], note: "这些方向尚不可调用，也没有承诺开放时间。本页不发布内部经济假设、虚构端点、参数、合约地址或收益保证。" },
      { id: "approach", title: "现在可以做什么", paragraphs: ["从现有公开状态读取示例开始，熟悉错误处理和数据来源边界。对需要角色数据、事件订阅或玩法写入的需求，先提交明确的接入场景。", "未来文档会在实现、权限与验证证据齐备后逐项更新；通用 Dubhe 能力与本游戏开放状态分别核实。"], links: [{ label: "回到当前开放能力", href: "developers/capabilities" }, { label: "反馈接入需求", href: "developers/contributing" }] },
    ],
  },
];
