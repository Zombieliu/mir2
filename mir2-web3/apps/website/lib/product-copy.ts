import type { Locale } from "@/lib/site-copy";
import { createMarketProductCopies } from "@/lib/market-copy";

export const productSections = ["updates", "roadmap", "community", "press", "support", "status"] as const;
export type ProductSection = (typeof productSections)[number];

export const legalDocuments = ["privacy", "terms", "refunds", "ai-data"] as const;
export type LegalDocument = (typeof legalDocuments)[number];

export type ProductCard = {
  eyebrow: string;
  title: string;
  copy: string;
  href?: string;
  label?: string;
};

export type UpdateEntry = {
  slug: string;
  date: string;
  tag: string;
  title: string;
  summary: string;
  sections: Array<{ title: string; copy: string }>;
};

export type ProductCopy = {
  chrome: {
    updates: string;
    roadmap: string;
    community: string;
    press: string;
    support: string;
    status: string;
    account: string;
    legal: string;
    privacy: string;
    terms: string;
    refunds: string;
    aiData: string;
  };
  proof: {
    eyebrow: string;
    title: string;
    copy: string;
    verified: string;
    action: string;
    items: ProductCard[];
  };
  play: {
    eyebrow: string;
    title: string;
    copy: string;
    launch: string;
    browser: string;
    browserCopy: string;
    desktop: string;
    desktopCopy: string;
    candidate: string;
    candidateCopy: string;
    requirements: string;
    requirementItems: string[];
    safetyTitle: string;
    safetyPoints: string[];
  };
  membership: {
    founderEyebrow: string;
    founderTitle: string;
    founderCopy: string;
    examplesTitle: string;
    examples: ProductCard[];
    rollover: string;
    cancellation: string;
    refund: string;
    rights: string;
    waitlist: string;
    email: string;
    region: string;
    plan: string;
    consent: string;
    submit: string;
    submitted: string;
    previewSubmitted: string;
    submitError: string;
    company: string;
  };
  watch: {
    archiveTitle: string;
    archiveCopy: string;
    all: string;
    gameplay: string;
    systems: string;
    world: string;
    share: string;
    copied: string;
    footage: string;
    still: string;
  };
  account: {
    eyebrow: string;
    title: string;
    copy: string;
    signIn: string;
    preview: string;
    usageTitle: string;
    contentTitle: string;
  };
  pages: Record<ProductSection, {
    eyebrow: string;
    title: string;
    copy: string;
    cards: ProductCard[];
  }>;
  updates: UpdateEntry[];
  legal: Record<LegalDocument, {
    title: string;
    summary: string;
    sections: Array<{ title: string; copy: string }>;
  }>;
  common: {
    updated: string;
    draft: string;
    learnMore: string;
    back: string;
    unavailable: string;
  };
  ui: {
    official: string;
    supportWarning: string;
    legalDisclaimer: string;
    join: string;
    watch: string;
    founderService: string;
    usageContract: string;
    aiService: string;
    systemSafety: string;
    scroll: string;
    prototypeData: string;
    publicDraft: string;
    accountUsage: [string, string, string];
  };
};

const zhCN: ProductCopy = {
  chrome: {
    updates: "更新日志", roadmap: "路线图", community: "社区", press: "媒体中心", support: "支持",
    status: "服务状态", account: "账户", legal: "规则与安全", privacy: "隐私政策", terms: "用户协议",
    refunds: "订阅与退款", aiData: "AI 数据政策",
  },
  proof: {
    eyebrow: "REAL CLIENT · PUBLIC CANDIDATE",
    title: "不是概念图。\n这是正在运行的游戏。",
    copy: "以下画面直接来自当前浏览器客户端，展示角色选择、共享地图、背包装备与技能系统。画面会随候选版本继续演进。",
    verified: "实机录制 · 2026.08",
    action: "查看进入游戏说明",
    items: [
      { eyebrow: "CHARACTER", title: "角色与职业", copy: "同一账号管理多个角色，职业、等级与最后在线状态由游戏服务保存。" },
      { eyebrow: "SHARED WORLD", title: "比奇省共享区域", copy: "角色、守卫、怪物、地图与公开世界状态在同一客户端中呈现。" },
      { eyebrow: "SYSTEMS", title: "背包、技能与成长", copy: "装备、药品、技能、任务与角色成长拥有完整的游戏内界面。" },
    ],
  },
  play: {
    eyebrow: "ENTER WORLD 01",
    title: "先了解当前阶段，\n再进入玛法。",
    copy: "传奇重生目前提供浏览器候选版本。启动页会先说明服务状态、加载时间、账号方式与设备要求，再把你交给游戏客户端。",
    launch: "启动浏览器客户端",
    browser: "浏览器版本",
    browserCopy: "无需下载安装。首次进入会缓存地图与角色素材，网络和设备性能会影响首帧时间。",
    desktop: "桌面客户端",
    desktopCopy: "独立桌面版本正在进行签名、更新与设备验收，开放后会在这里提供官方入口。",
    candidate: "PUBLIC CANDIDATE",
    candidateCopy: "当前版本用于公开试玩和候选验收，不代表所有内容、性能和视觉已最终完成。",
    requirements: "建议环境",
    requirementItems: ["Chrome、Edge 或 Safari 最新两个主要版本", "桌面端建议 8GB 内存与硬件加速", "首次加载建议稳定宽带并预留约 1–2 分钟", "移动端可以访问，复杂操作仍优先推荐桌面设备"],
    safetyTitle: "进入前需要知道",
    safetyPoints: ["不要在第三方页面输入游戏密码", "Passkey 与钱包连接只在正式游戏域名完成", "链上映射处于测试阶段，不承诺资产价格", "遇到登录或加载问题请通过官方支持页提交"],
  },
  membership: {
    founderEyebrow: "FOUNDER ACCESS",
    founderTitle: "先登记需求，再开放真实结算。",
    founderCopy: "登记不会扣款。正式开放时会单独确认价格、地区、续费方式、隐私授权与退款规则。",
    examplesTitle: "额度如何消耗",
    examples: [
      { eyebrow: "01", title: "AI 伙伴对话", copy: "一次完整问答计 1 次；系统错误或安全拒绝不扣除额度。" },
      { eyebrow: "02", title: "精彩切片", copy: "生成一个可下载版本计 1 条；同一事件的字幕修订不重复计费。" },
      { eyebrow: "03", title: "地区化处理", copy: "字幕、标题与基础配音按最终导出版本记录，不按预览次数消耗。" },
    ],
    rollover: "月度额度默认不结转；正式开放前会在结算页再次确认。",
    cancellation: "可以取消自动续费，权益持续到当前计费周期结束。",
    refund: "未使用服务的退款按地区法律与正式退款政策处理。",
    rights: "玩家保留原始内容权利；AI 生成内容的使用边界以数据政策为准。",
    waitlist: "申请 Founder Access",
    email: "邮箱",
    region: "地区",
    plan: "感兴趣的计划",
    consent: "我同意仅为 Founder Access、产品研究和上线通知使用这些信息。",
    submit: "提交登记",
    submitted: "登记已提交。正式开放前我们会再次确认，不会自动扣款。",
    previewSubmitted: "预览登记已验证；部署时配置线索 Webhook 后才会真正入库。",
    submitError: "暂时无法提交，请稍后重试。",
    company: "公司",
  },
  watch: {
    archiveTitle: "实机档案与地区化切片",
    archiveCopy: "当前先公开候选客户端实机画面；接入正式频道后，同一内容编号会关联直播、横版、竖版和多语言版本。",
    all: "全部", gameplay: "玩法", systems: "系统", world: "世界", share: "分享", copied: "链接已复制",
    footage: "打开实机画面", still: "候选版本截图",
  },
  account: {
    eyebrow: "ACCOUNT-BOUND SERVICES",
    title: "一个账户，管理游戏与 AI 服务。",
    copy: "账户中心用于查看订阅、AI 额度、已生成切片、授权和账单。当前页面展示正式结构，真实账户连接将在身份服务接入后开放。",
    signIn: "前往游戏账户登录",
    preview: "ACCOUNT PREVIEW",
    usageTitle: "AI 使用额度",
    contentTitle: "最近生成内容",
  },
  pages: {
    updates: {
      eyebrow: "WORLD CHRONICLES",
      title: "每一次改变，都有记录。",
      copy: "候选版本、客户端、世界服务和地区化内容的公开更新日志。",
      cards: [],
    },
    roadmap: {
      eyebrow: "PUBLIC ROADMAP",
      title: "把承诺分成可以验收的阶段。",
      copy: "路线图只描述当前计划，不构成发布时间或商业承诺。每个阶段都需要工程、设备和玩家验收。",
      cards: [
        { eyebrow: "NOW", title: "共享世界候选版", copy: "浏览器客户端、共享区域、基础角色成长、天机阁原型与四地区官网。" },
        { eyebrow: "NEXT", title: "内容与社区闭环", copy: "真实直播、自动切片、公会组织工具、Founder Access 与账户中心。" },
        { eyebrow: "LATER", title: "桌面与移动发行", copy: "设备适配、签名更新、长时间稳定性与正式发行渠道。" },
      ],
    },
    community: {
      eyebrow: "COMMUNITY",
      title: "世界由玩家共同塑造。",
      copy: "加入测试、公会招募、创作者计划和地区共创。外部频道只有在配置官方链接后才会显示为可加入。",
      cards: [
        { eyebrow: "PLAYTEST", title: "公开测试", copy: "试玩当前浏览器候选版本，通过支持页提交设备与体验反馈。", href: "play", label: "进入测试" },
        { eyebrow: "GUILDS", title: "公会招募", copy: "登记地区、语言与组织方向，为后续公会目录和世界事件做准备。", href: "support", label: "联系团队" },
        { eyebrow: "CREATORS", title: "创作者计划", copy: "申请切片额度、地区化工具与可追踪推广链接。", href: "membership#founder-access", label: "申请资格" },
      ],
    },
    press: {
      eyebrow: "PRESS & CREATOR KIT",
      title: "用统一素材讲述传奇重生。",
      copy: "品牌、截图、产品事实和地区化规则集中在同一入口，避免渠道使用过期或未经确认的素材。",
      cards: [
        { eyebrow: "BRAND", title: "品牌与 Logo", copy: "NUMERON、Legend of Rebirth 与天机阁的标准名称、色彩和安全留白。" },
        { eyebrow: "GAMEPLAY", title: "实机截图", copy: "只使用标注候选版本与采集日期的真实客户端画面。" },
        { eyebrow: "FACT SHEET", title: "产品事实", copy: "共享世界、三职业、跨端目标、AI 服务与非 P2W 原则。" },
      ],
    },
    support: {
      eyebrow: "PLAYER SUPPORT",
      title: "先定位问题，再联系人工支持。",
      copy: "提交问题时请提供时间、设备、浏览器、所在页面和可公开的错误信息；不要提交密码或钱包助记词。",
      cards: [
        { eyebrow: "LOGIN", title: "登录与 Passkey", copy: "确认在正式域名操作，清除过期会话后重试，并记录具体错误时间。" },
        { eyebrow: "LOADING", title: "加载与性能", copy: "保持页面前台、启用硬件加速，并等待首次资源缓存完成。" },
        { eyebrow: "BILLING", title: "订阅与退款", copy: "正式付费开放后，账单、取消和退款会在账户中心与退款政策中统一处理。" },
      ],
    },
    status: {
      eyebrow: "SERVICE STATUS",
      title: "把可用性和产品完成度分开说明。",
      copy: "服务在线不等于所有候选功能已完成。这里分别展示官网、游戏入口、世界数据与 AI 服务状态。",
      cards: [
        { eyebrow: "PUBLIC SITE", title: "官网", copy: "四语言官网与核心内容页面。" },
        { eyebrow: "GAME CLIENT", title: "浏览器客户端", copy: "公开候选版本；首次进入需要加载资源。" },
        { eyebrow: "ATLAS", title: "天机阁", copy: "原型公开数据；真实索引服务尚未接入。" },
        { eyebrow: "AI SERVICES", title: "AI 与结算", copy: "产品设计阶段；不会在未确认前自动扣款。" },
      ],
    },
  },
  updates: [
    {
      slug: "shared-world-candidate",
      date: "2026.08.28",
      tag: "WORLD",
      title: "共享世界候选版本继续推进",
      summary: "区域权威、移动、可见性与持久状态进入公开候选验证。",
      sections: [
        { title: "这次更新是什么", copy: "玩家进入同一世界区域后，位置、方向与可见对象由共享世界服务统一处理，而不是每个浏览器各自模拟。" },
        { title: "玩家会看到什么", copy: "移动、附近玩家、公开聊天与离开区域会以同一世界状态广播；异常移动会由服务端纠正。" },
        { title: "仍未完成", copy: "候选自动化不能替代真实设备、长时间运行和玩家手感验收。" },
      ],
    },
    {
      slug: "cross-platform-client",
      date: "2026.08.27",
      tag: "CLIENT",
      title: "跨平台客户端与统一启动入口",
      summary: "官网先说明阶段、设备要求和账号安全，再进入浏览器客户端。",
      sections: [
        { title: "统一入口", copy: "所有官网按钮先到启动说明页，避免品牌官网直接跳入缺少上下文的登录界面。" },
        { title: "浏览器优先", copy: "当前公开候选版继续以浏览器为主；桌面与移动发行需要额外设备和签名验收。" },
      ],
    },
    {
      slug: "regional-content-system",
      date: "2026.08.26",
      tag: "GLOBAL",
      title: "地区化素材与内容编号体系",
      summary: "同一事实、事件和推广内容可以拥有不同语言、配音和封面，同时保持统一归因。",
      sections: [
        { title: "保持一致的部分", copy: "游戏事实、世界事件编号、玩法识别与公平规则在各地区保持一致。" },
        { title: "可以本地化的部分", copy: "语言、字幕、配音、标题、封面和文化表达由地区资源包独立交付与回退。" },
      ],
    },
  ],
  legal: {
    privacy: {
      title: "隐私政策",
      summary: "说明官网、游戏、天机阁、直播和 AI 服务分别处理哪些数据。",
      sections: [
        { title: "我们处理的数据", copy: "可能包括账户标识、设备与浏览器信息、游戏行为、订阅状态、支持请求以及玩家明确授权用于 AI 服务的内容。" },
        { title: "公开与私人边界", copy: "天机阁和直播只展示延迟、聚合或玩家主动公开的信息，不公开实时精确位置、私人背包、私聊和未授权身份。" },
        { title: "保留与删除", copy: "正式服务会按用途设置保留期限，并提供访问、更正、导出和删除请求渠道；法律或安全要求可能需要延长保留。" },
      ],
    },
    terms: {
      title: "用户协议",
      summary: "规定账户、游戏行为、内容发布、自动化工具和服务变更的基本边界。",
      sections: [
        { title: "账户与安全", copy: "玩家应保护账户凭据，不得出售、共享或通过未经授权的自动化方式操纵游戏、市场或排名。" },
        { title: "公平游戏", copy: "禁止外挂、漏洞利用、欺诈、骚扰和破坏服务。候选版本可能重置、回滚或调整非正式数据。" },
        { title: "玩家内容", copy: "玩家保留原创内容权利，同时授权服务在提供游戏、审核、直播和明确选择的推广范围内处理内容。" },
      ],
    },
    refunds: {
      title: "订阅、取消与退款",
      summary: "说明 Token 服务的计费、取消、额度和地区退款规则。",
      sections: [
        { title: "正式确认前不扣款", copy: "Founder Access 登记不构成购买。只有在结算页明确展示价格、周期、税费和续费方式并由玩家确认后才会收费。" },
        { title: "取消与额度", copy: "取消自动续费后，权益持续到当前周期结束。月度额度默认不结转，除非结算页另有说明。" },
        { title: "退款", copy: "退款资格取决于购买地区、使用情况与法律要求。正式开放时会提供可执行的退款流程和客服入口。" },
      ],
    },
    "ai-data": {
      title: "AI 数据与生成内容政策",
      summary: "说明 AI 功能使用哪些信息、如何授权以及生成内容的边界。",
      sections: [
        { title: "默认不读取私人聊天", copy: "AI 服务只处理实现明确功能所需、玩家主动提交或单独授权的数据。私人聊天、未公开资产与敏感位置默认不进入生成流程。" },
        { title: "训练与改进", copy: "用于模型训练或产品改进需要独立说明和选择。关闭授权不会影响基础游戏访问。" },
        { title: "生成内容", copy: "生成内容可能包含错误或不适合公开的信息，发布前应由玩家或运营审核。系统会保留来源事件和内容编号以支持纠错。" },
      ],
    },
  },
  common: { updated: "更新于 2026.08.28", draft: "公开草案", learnMore: "查看详情", back: "返回", unavailable: "尚未配置" },
  ui: {
    official: "官方", supportWarning: "请勿发送密码、Passkey 私钥、钱包助记词或未经处理的私人聊天记录。",
    legalDisclaimer: "本页是产品实施草案，不替代针对发行地区的正式法律审查。", join: "加入", watch: "观看",
    founderService: "创始成员服务", usageContract: "AI 使用约定", aiService: "AI 服务", systemSafety: "系统与安全",
    scroll: "继续浏览", prototypeData: "原型数据", publicDraft: "法律文件 · 公开草案", accountUsage: ["AI 伙伴", "精彩切片", "语音导出"],
  },
};

const en: ProductCopy = {
  ...zhCN,
  chrome: {
    updates: "Updates", roadmap: "Roadmap", community: "Community", press: "Press kit", support: "Support",
    status: "Status", account: "Account", legal: "Rules & safety", privacy: "Privacy", terms: "Terms",
    refunds: "Billing & refunds", aiData: "AI data policy",
  },
  proof: {
    eyebrow: "REAL CLIENT · PUBLIC CANDIDATE",
    title: "Not concept art.\nA world already running.",
    copy: "Captured directly from the current browser client: character selection, a shared map, inventory, equipment and skill systems.",
    verified: "LIVE CLIENT · 2026.08",
    action: "Read before entering",
    items: [
      { eyebrow: "CHARACTER", title: "Characters and paths", copy: "One account keeps multiple characters, classes, levels and last-online state." },
      { eyebrow: "SHARED WORLD", title: "Bichon shared zone", copy: "Characters, guards, creatures, maps and public world state appear in the same client." },
      { eyebrow: "SYSTEMS", title: "Inventory, skills, growth", copy: "Equipment, potions, skills, quests and progression have complete in-game surfaces." },
    ],
  },
  play: {
    eyebrow: "ENTER WORLD 01",
    title: "Know the current stage.\nThen enter Mir.",
    copy: "Legend of Rebirth currently offers a public browser Candidate. This launch page explains status, loading, account methods and device expectations first.",
    launch: "Launch browser client", browser: "Browser Candidate", browserCopy: "No install. The first visit caches maps and character assets, so connection and device speed affect first frame.",
    desktop: "Desktop client", desktopCopy: "Signing, updating and real-device acceptance are still in progress. The official download will appear here.",
    candidate: "PUBLIC CANDIDATE", candidateCopy: "Public testing and acceptance build; content, performance and visuals are not all final.",
    requirements: "Recommended environment", requirementItems: ["Latest two major versions of Chrome, Edge or Safari", "Desktop with 8GB RAM and hardware acceleration", "Stable broadband and roughly 1–2 minutes for first load", "Mobile access works, but complex control is still desktop-first"],
    safetyTitle: "Before entering", safetyPoints: ["Never enter a game password on a third-party page", "Complete Passkey and wallet connections only on the official game domain", "On-chain mapping is experimental and does not promise asset value", "Use official Support for login and loading issues"],
  },
  membership: {
    founderEyebrow: "FOUNDER ACCESS", founderTitle: "Register interest before real billing opens.", founderCopy: "Registration never charges you. Price, region, renewal, privacy and refunds are confirmed again before checkout.",
    examplesTitle: "How usage is counted", examples: [
      { eyebrow: "01", title: "AI companion", copy: "One completed question-and-answer counts once; system errors and safety refusals do not." },
      { eyebrow: "02", title: "Highlight clips", copy: "One downloadable output counts once; subtitle corrections for the same event do not." },
      { eyebrow: "03", title: "Localization", copy: "Captions, titles and basic voice count on final export, never on previews." },
    ],
    rollover: "Monthly usage does not roll over by default.", cancellation: "Cancel renewal any time; benefits last through the paid period.",
    refund: "Unused-service refunds follow regional law and the final refund policy.", rights: "Players retain rights in original work; AI output follows the AI Data Policy.",
    waitlist: "Request Founder Access", email: "Email", region: "Region", plan: "Plan of interest", consent: "I agree to use of this information only for Founder Access, product research and launch notices.", submit: "Submit request",
    submitted: "Request submitted. We will confirm again before launch and never charge automatically.", previewSubmitted: "Preview flow verified. Configure the lead webhook at deployment to persist requests.",
    submitError: "Unable to submit. Please try again later.", company: "Company",
  },
  watch: { archiveTitle: "Client proof and localized cuts", archiveCopy: "Candidate client stills are public now. Production channels will connect the same content ID to live, landscape, vertical and localized editions.", all: "All", gameplay: "Gameplay", systems: "Systems", world: "World", share: "Share", copied: "Link copied", footage: "Open client proof", still: "Candidate still" },
  account: { eyebrow: "ACCOUNT-BOUND SERVICES", title: "One account for the game and AI services.", copy: "The account center will manage subscriptions, AI usage, generated clips, permissions and billing. This page shows the production structure before identity integration.", signIn: "Continue to game account", preview: "ACCOUNT PREVIEW", usageTitle: "AI usage", contentTitle: "Generated content" },
  pages: {
    updates: { eyebrow: "WORLD CHRONICLES", title: "Every change leaves a record.", copy: "Public updates for the Candidate, client, world services and regional content.", cards: [] },
    roadmap: { eyebrow: "PUBLIC ROADMAP", title: "Promises divided into testable stages.", copy: "Plans are not release-date commitments. Every stage needs engineering, device and player acceptance.", cards: [
      { eyebrow: "NOW", title: "Shared-world Candidate", copy: "Browser client, shared zones, core progression, Atlas prototype and four-region website." },
      { eyebrow: "NEXT", title: "Content and community loop", copy: "Real broadcasts, automated cuts, guild tools, Founder Access and account center." },
      { eyebrow: "LATER", title: "Desktop and mobile release", copy: "Device support, signing, updates, soak and distribution acceptance." },
    ] },
    community: { eyebrow: "COMMUNITY", title: "A world shaped with its players.", copy: "Join tests, guild discovery, creator tools and regional co-creation. External channels appear only when official links are configured.", cards: [
      { eyebrow: "PLAYTEST", title: "Public test", copy: "Try the current browser Candidate and report device or experience issues through Support.", href: "play", label: "Enter test" },
      { eyebrow: "GUILDS", title: "Guild discovery", copy: "Register region, language and focus for the future guild directory.", href: "support", label: "Contact team" },
      { eyebrow: "CREATORS", title: "Creator program", copy: "Apply for clip usage, localization tools and attributable campaign links.", href: "membership#founder-access", label: "Request access" },
    ] },
    press: { eyebrow: "PRESS & CREATOR KIT", title: "Tell one consistent Legend of Rebirth story.", copy: "Brand rules, real client stills, product facts and localization guidance stay in one place.", cards: [
      { eyebrow: "BRAND", title: "Brand and logos", copy: "Standard names, colors and clear-space rules for NUMERON, Legend of Rebirth and Atlas." },
      { eyebrow: "GAMEPLAY", title: "Client stills", copy: "Use only real captures labeled with Candidate status and capture date." },
      { eyebrow: "FACT SHEET", title: "Product facts", copy: "Shared world, three paths, cross-device goals, AI services and non-P2W contract." },
    ] },
    support: { eyebrow: "PLAYER SUPPORT", title: "Diagnose first. Escalate safely.", copy: "Include time, device, browser, page and shareable errors. Never submit passwords or wallet seed phrases.", cards: [
      { eyebrow: "LOGIN", title: "Login and Passkey", copy: "Use the official domain, clear stale sessions and record the exact failure time." },
      { eyebrow: "LOADING", title: "Loading and performance", copy: "Keep the page foregrounded, enable acceleration and let first-time caching finish." },
      { eyebrow: "BILLING", title: "Billing and refunds", copy: "When payments open, billing, cancellation and refunds live in Account and the Refund Policy." },
    ] },
    status: { eyebrow: "SERVICE STATUS", title: "Availability is not the same as completion.", copy: "The website, client, public data and AI services report status separately.", cards: [
      { eyebrow: "PUBLIC SITE", title: "Website", copy: "Sixteen market locales and core product pages." }, { eyebrow: "GAME CLIENT", title: "Browser client", copy: "Public Candidate; first entry downloads assets." },
      { eyebrow: "ATLAS", title: "Atlas", copy: "Prototype public data; production index is not connected." }, { eyebrow: "AI SERVICES", title: "AI and billing", copy: "Product stage; no automatic charge before confirmation." },
    ] },
  },
  updates: [
    { slug: "shared-world-candidate", date: "2026.08.28", tag: "WORLD", title: "Shared-world Candidate advances", summary: "Authority, movement, visibility and persistence continue into public Candidate validation.", sections: [
      { title: "What changed", copy: "Players entering the same zone now share authoritative position, direction and visible-object state instead of simulating separate personal worlds." },
      { title: "What players see", copy: "Movement, nearby players, public chat and zone departures are broadcast from one world state; invalid movement receives a server correction." },
      { title: "What remains", copy: "Candidate automation does not replace real-device, long-session and player-feel acceptance." },
    ] },
    { slug: "cross-platform-client", date: "2026.08.27", tag: "CLIENT", title: "Cross-platform client and unified launch", summary: "The site now explains stage, device and account safety before the browser client.", sections: [
      { title: "One launch path", copy: "Every website CTA first opens a launch brief so players do not arrive at an unexplained login surface." },
      { title: "Browser first", copy: "The public Candidate remains browser-first; desktop and mobile distribution need separate device and signing acceptance." },
    ] },
    { slug: "regional-content-system", date: "2026.08.26", tag: "GLOBAL", title: "Regional content and shared attribution IDs", summary: "One event can support local language, voice and artwork while preserving a shared source and campaign ID.", sections: [
      { title: "What stays shared", copy: "Game facts, event IDs, gameplay identity and fairness rules remain consistent in every region." },
      { title: "What localizes", copy: "Language, captions, voice, titles, thumbnails and cultural framing ship as independently versioned regional assets." },
    ] },
  ],
  legal: {
    privacy: { title: "Privacy Policy", summary: "How the site, game, Atlas, broadcasts and AI services handle data.", sections: [
      { title: "Data we process", copy: "This may include account identifiers, device and browser information, gameplay actions, subscription state, support requests and content explicitly authorized for AI services." },
      { title: "Public and private boundaries", copy: "Atlas and broadcasts expose only delayed, aggregated or player-published information—not precise live location, private inventory, private chat or unauthorized identity." },
      { title: "Retention and deletion", copy: "Production services will define purpose-based retention and provide access, correction, export and deletion channels, subject to legal and security obligations." },
    ] },
    terms: { title: "Terms of Service", summary: "Account, fair-play, content, automation and service-change boundaries.", sections: [
      { title: "Accounts and security", copy: "Players must protect credentials and may not sell, share or use unauthorized automation to manipulate the game, market or rankings." },
      { title: "Fair play", copy: "Cheats, exploits, fraud, harassment and service disruption are prohibited. Candidate data may be reset, rolled back or rebalanced." },
      { title: "Player content", copy: "Players retain original rights while licensing the limited processing needed to operate, moderate, broadcast and promote content they explicitly select." },
    ] },
    refunds: { title: "Subscriptions, cancellation and refunds", summary: "Token billing, cancellation, usage and regional refund rules.", sections: [
      { title: "No charge before confirmation", copy: "Founder Access registration is not a purchase. Billing starts only after checkout clearly presents price, period, taxes and renewal and the player confirms." },
      { title: "Cancellation and usage", copy: "After renewal is cancelled, benefits continue through the current paid period. Monthly usage does not roll over unless checkout says otherwise." },
      { title: "Refunds", copy: "Eligibility depends on purchase region, usage and law. A working refund flow and support route will be available before paid launch." },
    ] },
    "ai-data": { title: "AI Data and Generated Content Policy", summary: "AI inputs, permissions and generated-content boundaries.", sections: [
      { title: "Private chat is excluded by default", copy: "AI services process only data required for a selected feature, actively submitted by a player or separately authorized. Private chat and sensitive location are excluded by default." },
      { title: "Training and improvement", copy: "Model training or product-improvement use requires separate notice and choice. Declining it does not block basic game access." },
      { title: "Generated content", copy: "Outputs can be inaccurate or unsuitable for publication and should be reviewed. Source event and content IDs remain attached for correction and attribution." },
    ] },
  },
  common: { updated: "Updated 2026.08.28", draft: "Public draft", learnMore: "Learn more", back: "Back", unavailable: "Not configured" },
  ui: {
    official: "Official", supportWarning: "Never send passwords, Passkey secrets, wallet seed phrases or unredacted private chat.",
    legalDisclaimer: "This implementation draft does not replace formal legal review for each launch region.", join: "Join", watch: "Watch",
    founderService: "Founder service", usageContract: "AI usage contract", aiService: "AI service", systemSafety: "System and safety",
    scroll: "Scroll", prototypeData: "Prototype data", publicDraft: "Legal · public draft", accountUsage: ["AI companion", "Highlight cuts", "Voice exports"],
  },
};

const ja: ProductCopy = {
  ...en,
  chrome: { updates: "更新情報", roadmap: "ロードマップ", community: "コミュニティ", press: "メディア", support: "サポート", status: "稼働状況", account: "アカウント", legal: "ルールと安全", privacy: "プライバシー", terms: "利用規約", refunds: "購読と返金", aiData: "AIデータ方針" },
  proof: { ...en.proof, title: "コンセプトではない。\n稼働中のゲーム。", copy: "現在のブラウザ版から直接取得した、キャラクター、共有マップ、所持品、装備、スキルの実機画面です。", action: "ゲーム開始前の案内" },
  play: { ...en.play, title: "現在の段階を確認して、\nミルの世界へ。", copy: "現在はブラウザ向け公開Candidateです。起動前にサービス状況、読込時間、アカウント方式、推奨環境を案内します。", launch: "ブラウザ版を起動", requirements: "推奨環境", safetyTitle: "開始前の安全確認" },
  membership: { ...en.membership, founderTitle: "本決済の前にFounder Accessを登録。", waitlist: "Founder Accessを申請", email: "メール", region: "地域", plan: "希望プラン", submit: "申請する" },
  watch: { ...en.watch, archiveTitle: "実機記録と地域別ハイライト", share: "共有", copied: "リンクをコピーしました" },
  account: { ...en.account, title: "ひとつのアカウントでゲームとAIを管理。", signIn: "ゲームアカウントへ" },
  pages: {
    ...en.pages,
    updates: { ...en.pages.updates, title: "すべての変更を記録する。", copy: "Candidate、クライアント、ワールドサービス、地域コンテンツの公開更新情報。" },
    roadmap: { ...en.pages.roadmap, title: "検証できる段階に分けたロードマップ。" },
    community: { ...en.pages.community, title: "プレイヤーと共に作る世界。" },
    press: { ...en.pages.press, title: "統一された素材で物語を伝える。" },
    support: { ...en.pages.support, title: "安全に問題を切り分け、サポートへ。" },
    status: { ...en.pages.status, title: "稼働状況と完成度を分けて表示。" },
  },
  updates: en.updates.map((entry, index) => ({
    ...entry,
    title: ["共有ワールドCandidateの進展", "クロスプラットフォームと統一起動", "地域別コンテンツと共通ID"][index],
    summary: ["権限管理、移動、可視性、永続化を公開Candidateで検証しています。", "端末要件とアカウント安全性を確認してからゲームを起動します。", "共通の事実と帰属IDを維持しながら、言語・音声・画像を地域化します。"][index],
    sections: entry.sections.map((section, sectionIndex) => ({
      title: [["変更点", "プレイヤーが確認できること", "残る検証"], ["統一された入口", "ブラウザ優先"], ["共通で保つもの", "地域化するもの"]][index][sectionIndex],
      copy: [
        ["同じゾーンに入ったプレイヤーの位置・方向・可視対象を共有ワールドサービスが一元管理します。", "移動、周辺プレイヤー、公開チャット、退出が一つのワールド状態から配信されます。", "自動テストは実機、長時間稼働、操作感の承認を置き換えません。"],
        ["すべての公式CTAは、説明のないログイン画面へ直接送らず、起動案内を先に表示します。", "公開Candidateはブラウザ優先です。デスクトップとモバイルには別途、端末・署名の検証が必要です。"],
        ["ゲームの事実、イベントID、プレイ特性、公平性ルールは全地域で共通です。", "言語、字幕、音声、タイトル、サムネイル、文化表現は地域別アセットとして独立管理します。"],
      ][index][sectionIndex],
    })),
  })),
  legal: {
    privacy: { title: "プライバシーポリシー", summary: "サイト、ゲーム、Atlas、配信、AIサービスにおけるデータ処理。", sections: [
      { title: "処理するデータ", copy: "アカウント識別子、端末・ブラウザ情報、ゲーム行動、購読状態、サポート依頼、AI利用に明示的に許可された内容を含む場合があります。" },
      { title: "公開と非公開の境界", copy: "Atlasと配信は遅延・集計・本人公開情報のみを扱い、正確な現在地、私有インベントリ、私信、未許可の本人情報は公開しません。" },
      { title: "保存と削除", copy: "正式サービスでは目的別の保存期間と、開示・訂正・出力・削除の窓口を設けます。" },
    ] },
    terms: { title: "利用規約", summary: "アカウント、公平なプレイ、コンテンツ、自動化、サービス変更の基本ルール。", sections: [
      { title: "アカウントと安全", copy: "認証情報を保護し、販売・共有・未許可の自動化によるゲームや市場の操作を行わないでください。" },
      { title: "公平なプレイ", copy: "チート、不具合悪用、詐欺、嫌がらせ、サービス妨害は禁止です。Candidateデータは調整・巻き戻し・リセットされる場合があります。" },
      { title: "プレイヤーコンテンツ", copy: "原著作物の権利はプレイヤーに残り、運営・審査・明示選択した配信に必要な範囲だけ処理を許諾します。" },
    ] },
    refunds: { title: "購読・解約・返金", summary: "Tokenの課金、解約、利用量、地域別返金ルール。", sections: [
      { title: "確認前は課金しない", copy: "Founder Access登録は購入ではありません。価格、期間、税、更新を決済画面で確認した後にのみ課金します。" },
      { title: "解約と利用量", copy: "自動更新を止めても支払済み期間の終了まで利用できます。月間枠は特記がない限り繰り越しません。" },
      { title: "返金", copy: "購入地域、利用状況、法令に基づき判断し、有料開始前に実行可能な手続きと窓口を用意します。" },
    ] },
    "ai-data": { title: "AIデータと生成コンテンツ方針", summary: "AI入力、許可、生成物の境界。", sections: [
      { title: "私信は既定で対象外", copy: "選択機能に必要な情報、本人が送信した情報、個別に許可した情報のみを処理します。" },
      { title: "学習と改善", copy: "学習や製品改善への利用には別の説明と選択が必要で、拒否しても基本ゲームは利用できます。" },
      { title: "生成コンテンツ", copy: "誤りや非公開情報を含む可能性があるため公開前に確認し、訂正と帰属のため元イベントIDを保持します。" },
    ] },
  },
  common: { updated: "2026.08.28 更新", draft: "公開草案", learnMore: "詳細", back: "戻る", unavailable: "未設定" },
};

const ko: ProductCopy = {
  ...en,
  chrome: { updates: "업데이트", roadmap: "로드맵", community: "커뮤니티", press: "미디어", support: "지원", status: "서비스 상태", account: "계정", legal: "규칙과 안전", privacy: "개인정보", terms: "이용약관", refunds: "구독과 환불", aiData: "AI 데이터 정책" },
  proof: { ...en.proof, title: "콘셉트가 아닙니다.\n실제로 실행 중인 게임입니다.", copy: "현재 브라우저 클라이언트에서 직접 캡처한 캐릭터, 공유 맵, 인벤토리, 장비와 스킬 화면입니다.", action: "게임 입장 안내" },
  play: { ...en.play, title: "현재 단계를 확인하고,\n미르에 입장하세요.", copy: "현재 브라우저 공개 Candidate를 제공합니다. 시작 전에 상태, 로딩, 계정 방식과 권장 환경을 안내합니다.", launch: "브라우저 클라이언트 실행", requirements: "권장 환경", safetyTitle: "입장 전 안전 확인" },
  membership: { ...en.membership, founderTitle: "실제 결제 전에 Founder Access를 등록하세요.", waitlist: "Founder Access 신청", email: "이메일", region: "지역", plan: "관심 플랜", submit: "신청 제출" },
  watch: { ...en.watch, archiveTitle: "실기 기록과 지역화 하이라이트", share: "공유", copied: "링크가 복사되었습니다" },
  account: { ...en.account, title: "하나의 계정으로 게임과 AI 서비스를 관리합니다.", signIn: "게임 계정으로 이동" },
  pages: {
    ...en.pages,
    updates: { ...en.pages.updates, title: "모든 변화는 기록으로 남습니다.", copy: "Candidate, 클라이언트, 월드 서비스와 지역 콘텐츠의 공개 업데이트입니다." },
    roadmap: { ...en.pages.roadmap, title: "검증 가능한 단계로 나눈 로드맵." },
    community: { ...en.pages.community, title: "플레이어와 함께 만드는 세계." },
    press: { ...en.pages.press, title: "일관된 자료로 전설을 전하세요." },
    support: { ...en.pages.support, title: "안전하게 문제를 확인하고 지원을 요청하세요." },
    status: { ...en.pages.status, title: "가용성과 완성도를 따로 표시합니다." },
  },
  updates: en.updates.map((entry, index) => ({
    ...entry,
    title: ["공유 월드 Candidate 진전", "크로스 플랫폼 클라이언트와 통합 실행", "지역화 콘텐츠와 공통 귀속 ID"][index],
    summary: ["권한, 이동, 가시성, 영속성을 공개 Candidate에서 검증합니다.", "기기 요구 사항과 계정 안전을 확인한 뒤 브라우저 클라이언트를 실행합니다.", "공통 사실과 캠페인 ID를 유지하며 언어·음성·이미지를 지역화합니다."][index],
    sections: entry.sections.map((section, sectionIndex) => ({
      title: [["변경 사항", "플레이어가 보는 것", "남은 검증"], ["통합 실행 경로", "브라우저 우선"], ["공통으로 유지하는 것", "지역화하는 것"]][index][sectionIndex],
      copy: [
        ["같은 지역의 위치, 방향, 가시 대상을 공유 월드 서비스가 하나의 권한 상태로 관리합니다.", "이동, 주변 플레이어, 공개 채팅과 퇴장이 하나의 월드 상태에서 전달됩니다.", "자동 검증은 실제 기기, 장시간 실행, 플레이 감각 승인을 대체하지 않습니다."],
        ["모든 공식 CTA는 설명 없는 로그인 화면으로 바로 보내지 않고 실행 안내를 먼저 보여 줍니다.", "공개 Candidate는 브라우저 우선이며 데스크톱과 모바일은 별도 기기·서명 검증이 필요합니다."],
        ["게임 사실, 이벤트 ID, 플레이 정체성, 공정성 규칙은 모든 지역에서 같습니다.", "언어, 자막, 음성, 제목, 썸네일과 문화 표현은 지역별 자산으로 독립 관리합니다."],
      ][index][sectionIndex],
    })),
  })),
  legal: {
    privacy: { title: "개인정보 처리방침", summary: "사이트, 게임, Atlas, 방송과 AI 서비스의 데이터 처리 방식입니다.", sections: [
      { title: "처리하는 데이터", copy: "계정 식별자, 기기·브라우저 정보, 게임 행동, 구독 상태, 지원 요청과 AI 기능에 명시적으로 허용한 콘텐츠가 포함될 수 있습니다." },
      { title: "공개와 비공개의 경계", copy: "Atlas와 방송은 지연·집계·사용자 공개 정보만 다루며 실시간 정확한 위치, 개인 가방, 비공개 채팅과 승인되지 않은 신원을 공개하지 않습니다." },
      { title: "보관과 삭제", copy: "정식 서비스는 목적별 보관 기간과 열람·정정·내보내기·삭제 요청 채널을 제공합니다." },
    ] },
    terms: { title: "이용약관", summary: "계정, 공정 플레이, 콘텐츠, 자동화와 서비스 변경의 기본 경계입니다.", sections: [
      { title: "계정과 보안", copy: "인증 정보를 보호하고 계정 판매·공유 또는 승인되지 않은 자동화로 게임·시장·순위를 조작해서는 안 됩니다." },
      { title: "공정 플레이", copy: "치트, 악용, 사기, 괴롭힘과 서비스 방해를 금지합니다. Candidate 데이터는 조정·복구·초기화될 수 있습니다." },
      { title: "플레이어 콘텐츠", copy: "원본 권리는 플레이어에게 남으며 운영·검토·명시적으로 선택한 배포에 필요한 범위만 처리할 수 있습니다." },
    ] },
    refunds: { title: "구독·취소·환불", summary: "Token 결제, 취소, 사용량과 지역별 환불 규칙입니다.", sections: [
      { title: "확인 전에는 결제하지 않음", copy: "Founder Access 등록은 구매가 아닙니다. 가격, 기간, 세금과 갱신을 결제 화면에서 확인한 후에만 청구합니다." },
      { title: "취소와 사용량", copy: "자동 갱신을 취소해도 결제 기간이 끝날 때까지 혜택이 유지됩니다. 별도 안내가 없으면 월간 사용량은 이월되지 않습니다." },
      { title: "환불", copy: "구매 지역, 사용 상태와 법률에 따라 판단하며 유료 출시 전에 실제 환불 절차와 지원 채널을 제공합니다." },
    ] },
    "ai-data": { title: "AI 데이터 및 생성 콘텐츠 정책", summary: "AI 입력, 권한과 생성 콘텐츠의 경계입니다.", sections: [
      { title: "비공개 채팅은 기본 제외", copy: "선택 기능에 필요한 정보, 플레이어가 제출한 정보 또는 별도로 허용한 정보만 처리합니다." },
      { title: "학습과 개선", copy: "모델 학습이나 제품 개선에는 별도 고지와 선택이 필요하며 거부해도 기본 게임 이용에는 영향이 없습니다." },
      { title: "생성 콘텐츠", copy: "오류나 공개하기 부적절한 정보가 있을 수 있어 게시 전 검토가 필요하며 정정과 귀속을 위해 원본 이벤트 ID를 유지합니다." },
    ] },
  },
  common: { updated: "2026.08.28 업데이트", draft: "공개 초안", learnMore: "자세히", back: "돌아가기", unavailable: "미설정" },
};

const productTranslations: Record<Locale, ProductCopy> = Object.assign({ "zh-CN": zhCN, en, ja, ko }, createMarketProductCopies(en));

export function getProductCopy(locale: Locale) {
  return productTranslations[locale];
}

export function isProductSection(value: string): value is ProductSection {
  return productSections.includes(value as ProductSection);
}

export function isLegalDocument(value: string): value is LegalDocument {
  return legalDocuments.includes(value as LegalDocument);
}
