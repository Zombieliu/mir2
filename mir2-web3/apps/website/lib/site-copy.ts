export const locales = ["zh-CN", "en", "ja", "ko"] as const;

export type Locale = (typeof locales)[number];

export type SiteCopy = {
  languageName: string;
  nav: {
    explore: string;
    world: string;
    classes: string;
    global: string;
    pulse: string;
    watch: string;
    highlights: string;
    chronicles: string;
    atlas: string;
    token: string;
    locale: string;
    menu: string;
    home: string;
  };
  hero: {
    eyebrow: string;
    titleTop: string;
    titleBottom: string;
    description: string;
    primary: string;
    secondary: string;
    status: string;
  };
  metrics: Array<{ value: string; label: string }>;
  atlas: {
    eyebrow: string;
    title: string;
    copy: string;
    button: string;
    live: string;
    metrics: Array<{ value: string; label: string }>;
  };
  world: {
    eyebrow: string;
    title: string;
    copy: string;
    cards: Array<{ number: string; title: string; copy: string }>;
  };
  classes: {
    eyebrow: string;
    title: string;
    copy: string;
    items: Array<{ key: string; title: string; role: string; copy: string }>;
  };
  global: {
    eyebrow: string;
    title: string;
    copy: string;
    markets: string[];
  };
  chronicles: {
    eyebrow: string;
    title: string;
    items: Array<{ date: string; tag: string; title: string; copy: string }>;
  };
  watch: {
    eyebrow: string;
    title: string;
    copy: string;
    live: string;
    demo: string;
    liveTitle: string;
    liveCopy: string;
    enter: string;
    openAtlas: string;
    highlightsEyebrow: string;
    highlightsTitle: string;
    highlightsCopy: string;
    clips: Array<{ tag: string; duration: string; title: string; copy: string }>;
    channelsEyebrow: string;
    channelsTitle: string;
    channelsCopy: string;
  };
  membership: {
    eyebrow: string;
    title: string;
    copy: string;
    serviceBadge: string;
    prototypePrice: string;
    monthly: string;
    annual: string;
    annualNote: string;
    perMonth: string;
    perYear: string;
    recommended: string;
    unavailable: string;
    plans: Array<{
      id: "free" | "token" | "architect";
      name: string;
      tagline: string;
      monthlyPrice: string;
      annualPrice: string;
      features: string[];
    }>;
    usageEyebrow: string;
    usageTitle: string;
    usageCopy: string;
    usage: Array<{ label: string; free: string; token: string; architect: string }>;
    fairnessEyebrow: string;
    fairnessTitle: string;
    fairnessCopy: string;
    fairnessPoints: string[];
    faqEyebrow: string;
    faqTitle: string;
    faq: Array<{ question: string; answer: string }>;
  };
  finalCta: { eyebrow: string; title: string; copy: string; button: string };
  footer: {
    note: string;
    legal: string;
    game: string;
    data: string;
    watch: string;
    regions: string;
    enter: string;
    worldPulse: string;
  };
};

const zhCN: SiteCopy = {
  languageName: "简体中文",
  nav: {
    explore: "游戏世界",
    world: "世界",
    classes: "职业",
    global: "地区版本",
    pulse: "世界动态",
    watch: "直播",
    highlights: "精彩时刻",
    chronicles: "编年史",
    atlas: "天机阁",
    token: "TOKEN",
    locale: "地区",
    menu: "菜单",
    home: "首页",
  },
  hero: {
    eyebrow: "A NUMERON SERIES // WORLD 01",
    titleTop: "传奇",
    titleBottom: "重生",
    description:
      "重返玛法大陆。在一个共享、持续演化的世界里，与真实玩家并肩作战、争夺荣耀，写下属于你的下一段传奇。",
    primary: "进入游戏",
    secondary: "探索世界",
    status: "世界服务在线",
  },
  metrics: [
    { value: "463", label: "张可探索地图" },
    { value: "03", label: "条经典职业路线" },
    { value: "01", label: "个共享世界" },
  ],
  atlas: {
    eyebrow: "NUMERON ATLAS // WORLD PULSE",
    title: "世界没有停下，\n它正在留下记录。",
    copy: "即使没有进入游戏，也能查看公开的角色、公会、装备、市场和世界事件。精确位置、私人资产与敏感数据不会公开。",
    button: "进入传奇重生天机阁",
    live: "玛法一号在线",
    metrics: [
      { value: "12,684", label: "在线冒险者" },
      { value: "428", label: "活跃区域" },
      { value: "8,912", label: "今日公开事件" },
    ],
  },
  world: {
    eyebrow: "A LIVING WORLD",
    title: "这里不是副本，\n而是共同生活的世界。",
    copy:
      "每一次相遇都发生在同一片大陆。城镇、荒野、地下迷宫与沙巴克，共同构成一个持续运行、由玩家推动的世界。",
    cards: [
      { number: "01", title: "共享区域", copy: "玩家、怪物与掉落由同一权威世界驱动，每次遭遇都真实发生。" },
      { number: "02", title: "经典战斗", copy: "走位、方向、技能时机与装备成长，保留经典传奇最纯粹的战斗节奏。" },
      { number: "03", title: "跨端旅程", copy: "网页、桌面与移动端共享角色进度，让冒险不被设备打断。" },
    ],
  },
  classes: {
    eyebrow: "CHOOSE YOUR PATH",
    title: "三种道路，一段传奇",
    copy: "力量、元素与道术从来不是答案；你如何与世界中的人并肩，才决定最终的名字。",
    items: [
      { key: "01", title: "战士", role: "近战 · 生存 · 爆发", copy: "以钢铁与意志踏入敌阵，在最短的距离内决定胜负。" },
      { key: "02", title: "法师", role: "元素 · 范围 · 控制", copy: "驾驭火焰、雷霆与寒冰，让整个战场成为你的法阵。" },
      { key: "03", title: "道士", role: "召唤 · 治疗 · 诅咒", copy: "联结生者与灵界，在持续战斗中改变队伍的命运。" },
    ],
  },
  global: {
    eyebrow: "ONE WORLD · MANY CULTURES",
    title: "同一个玛法，\n为每个地区重新讲述。",
    copy:
      "语言、字体、配音与地区视觉以独立资源包交付；核心战斗识别保持全球一致，本地文化由当地团队与玩家共同校准。",
    markets: ["简体中文", "ENGLISH", "日本語", "한국어"],
  },
  chronicles: {
    eyebrow: "WORLD CHRONICLES",
    title: "世界正在发生",
    items: [
      { date: "2026.08", tag: "WORLD", title: "共享世界继续扩展", copy: "区域权威、移动与玩家可见性持续进入可验证的候选版本。" },
      { date: "2026.08", tag: "CLIENT", title: "跨平台客户端演进", copy: "桌面、网页与移动入口围绕同一角色和世界状态逐步收敛。" },
      { date: "2026.08", tag: "GLOBAL", title: "地区化素材系统启动", copy: "官网、活动视觉与文化素材将支持独立版本、回退和审核。" },
    ],
  },
  watch: {
    eyebrow: "NUMERON LIVE // WORLD SIGNAL",
    title: "世界正在发生，\n传奇正在被记录。",
    copy: "AI 导播从公开世界事件中选择镜头、生成解说并制作精彩切片。当前页面使用原型数据，接入正式频道后将展示真实直播与回放。",
    live: "世界直播",
    demo: "DEMO FEED",
    liveTitle: "玛法一号 · 世界观察台",
    liveCopy: "只展示经过延迟与隐私处理的公开事件，不公开精确坐标、私人资产或敏感战术信息。",
    enter: "进入游戏",
    openAtlas: "查看天机阁",
    highlightsEyebrow: "HIGHLIGHT ARCHIVE",
    highlightsTitle: "今日精彩时刻",
    highlightsCopy: "同一个世界事件可以生成横版、竖版与地区化版本，并通过独立内容编号追踪实际注册与进入游戏转化。",
    clips: [
      { tag: "沙巴克", duration: "00:37", title: "最后一道城门", copy: "守城方在最后一分钟重新集结，战局在城门内完成逆转。" },
      { tag: "世界首杀", duration: "00:29", title: "赤月恶魔首次倒下", copy: "三支队伍争夺最后一击，稀有掉落进入公开世界记录。" },
      { tag: "玩家对战", duration: "00:22", title: "七步之后", copy: "残血战士在狭窄地形完成反击，连续击退三名追击者。" },
    ],
    channelsEyebrow: "GLOBAL DISTRIBUTION",
    channelsTitle: "一个事件，多种地区表达",
    channelsCopy: "保留同一事实与内容编号，为不同地区生成字幕、配音、标题和封面；所有版本最终回到同一官网与游戏入口。",
  },
  membership: {
    eyebrow: "NUMERON TOKEN // AI MEMBERSHIP",
    title: "不是月卡奖励，\n而是一层持续进化的 AI 服务。",
    copy: "NUMERON Token 是绑定账号的游戏服务订阅：它为冒险、创作和世界回顾提供 AI 能力，但不出售战斗数值，也不是可交易或承诺升值的资产。",
    serviceBadge: "ACCOUNT-BOUND SERVICE",
    prototypePrice: "原型价格 · 尚未开放购买",
    monthly: "月付",
    annual: "年付",
    annualNote: "年付约省 16%",
    perMonth: "/ 月",
    perYear: "/ 年",
    recommended: "推荐",
    unavailable: "即将开放",
    plans: [
      {
        id: "free",
        name: "旅人",
        tagline: "进入世界，认识你的第一段传奇",
        monthlyPrice: "免费",
        annualPrice: "免费",
        features: ["完整游戏基础访问", "公开天机阁与世界动态", "每日 1 次 AI 世界简报", "社区直播与精彩切片"],
      },
      {
        id: "token",
        name: "重生 Token",
        tagline: "为长期冒险者准备的个人 AI 层",
        monthlyPrice: "¥68",
        annualPrice: "¥680",
        features: ["每月 200 次 AI 伙伴对话", "角色成长与装备解释", "个人周报与世界关系回顾", "每月 10 条 AI 精彩切片", "四语言字幕与基础配音", "订阅期专属身份外观"],
      },
      {
        id: "architect",
        name: "架构师",
        tagline: "面向公会组织者与内容创作者",
        monthlyPrice: "¥168",
        annualPrice: "¥1,680",
        features: ["每月 1,000 次 AI 伙伴对话", "公会公开数据分析助手", "每月 60 条 AI 精彩切片", "多语言字幕、配音与标题方案", "优先 AI 推理与视频队列", "创作者数据与归因面板"],
      },
    ],
    usageEyebrow: "USAGE, NOT POWER",
    usageTitle: "像 AI 产品一样，清楚显示你订阅了什么。",
    usageCopy: "额度只用于 AI 计算、内容生成和便利服务。核心游戏、战斗胜负与装备掉落不按订阅等级出售。",
    usage: [
      { label: "AI 伙伴对话", free: "1 / 日", token: "200 / 月", architect: "1,000 / 月" },
      { label: "精彩切片生成", free: "—", token: "10 / 月", architect: "60 / 月" },
      { label: "地区化语音", free: "—", token: "基础", architect: "优先" },
      { label: "公会公开数据分析", free: "—", token: "—", architect: "包含" },
    ],
    fairnessEyebrow: "FAIR WORLD CONTRACT",
    fairnessTitle: "付费获得的是工具，不是胜利。",
    fairnessCopy: "订阅可以让玩家更理解世界、记录故事和组织社区，但不能购买伤害、掉率、移动速度或隐藏情报。",
    fairnessPoints: ["不增加角色战斗属性", "不提高装备与稀有掉落概率", "不出售其他玩家的私人数据", "Token 绑定账号且不可交易", "取消后保留角色与已生成内容"],
    faqEyebrow: "SUBSCRIPTION FAQ",
    faqTitle: "订阅之前，需要说清楚的事",
    faq: [
      { question: "Token 是加密货币吗？", answer: "不是。当前设计中 Token 是绑定账号的服务订阅凭证，不可交易，也不承诺价格或升值。" },
      { question: "订阅会让角色更强吗？", answer: "不会。付费额度只用于 AI 对话、总结、内容生成和创作者工具，不改变战斗数值与掉落。" },
      { question: "可以随时取消吗？", answer: "正式支付接入时应支持随时取消；权益持续到当前计费周期结束，不删除游戏角色和已有内容。" },
      { question: "AI 会读取私人聊天吗？", answer: "默认不会。只使用明确授权的数据，并对公开直播、分析与训练用途提供独立开关和可审计说明。" },
    ],
  },
  finalCta: {
    eyebrow: "THE GATE IS OPEN",
    title: "你的名字，等待被世界记住。",
    copy: "选择职业，踏入比奇，开始一段由你亲自书写的旅程。",
    button: "立即进入玛法",
  },
  footer: { note: "一个由玩家共同塑造的持续世界。", legal: "NUMERON · LEGEND OF REBIRTH", game: "游戏", data: "世界数据", watch: "观看", regions: "地区", enter: "进入游戏", worldPulse: "世界脉搏" },
};

const translations: Record<Locale, SiteCopy> = {
  "zh-CN": zhCN,
  en: {
    ...zhCN,
    languageName: "English",
    nav: { explore: "Explore", world: "World", classes: "Classes", global: "Regions", pulse: "World pulse", watch: "Live", highlights: "Highlights", chronicles: "Chronicles", atlas: "Atlas", token: "TOKEN", locale: "Region", menu: "Menu", home: "Home" },
    hero: {
      eyebrow: "A NUMERON SERIES // WORLD 01",
      titleTop: "Legend of",
      titleBottom: "Rebirth",
      description: "Return to the continent of Mir. Fight beside real players in one shared, evolving world—and write the next legend in your own name.",
      primary: "Enter the world",
      secondary: "Discover Mir",
      status: "World service online",
    },
    metrics: [
      { value: "463", label: "world maps" },
      { value: "03", label: "classic paths" },
      { value: "01", label: "shared world" },
    ],
    atlas: {
      eyebrow: "NUMERON ATLAS // WORLD PULSE",
      title: "The world never stops.\nIt leaves a record.",
      copy: "Explore public characters, guilds, equipment, markets and world events without entering the game. Precise locations, private assets and sensitive data stay private.",
      button: "Open the world atlas",
      live: "Mir World 01 online",
      metrics: [
        { value: "12,684", label: "adventurers online" },
        { value: "428", label: "active regions" },
        { value: "8,912", label: "public events today" },
      ],
    },
    world: {
      eyebrow: "A LIVING WORLD",
      title: "Not an instance.\nA world we inhabit together.",
      copy: "Every encounter unfolds on the same continent. Cities, wildlands, dungeons and Sabuk form a persistent world shaped by its players.",
      cards: [
        { number: "01", title: "Shared zones", copy: "Players, monsters and drops belong to one authoritative world, making every encounter real." },
        { number: "02", title: "Classic combat", copy: "Position, direction, timing and equipment preserve the deliberate rhythm of classic Mir combat." },
        { number: "03", title: "Cross-platform", copy: "Web, desktop and mobile share character progress, so the journey follows you." },
      ],
    },
    classes: {
      eyebrow: "CHOOSE YOUR PATH",
      title: "Three paths. One legend.",
      copy: "Strength, elements and spirit are only tools. The people you stand beside decide what your name becomes.",
      items: [
        { key: "01", title: "Warrior", role: "MELEE · SURVIVAL · BURST", copy: "Carry steel and resolve into the front line, deciding battles at arm's reach." },
        { key: "02", title: "Sorcerer", role: "ELEMENT · AREA · CONTROL", copy: "Command fire, thunder and ice until the battlefield itself becomes your spell." },
        { key: "03", title: "Taoist", role: "SUMMON · HEAL · CURSE", copy: "Bridge the living and spirit realms, changing the fate of a battle over time." },
      ],
    },
    global: {
      eyebrow: "ONE WORLD · MANY CULTURES",
      title: "One Mir,\nretold for every region.",
      copy: "Language, type, voice and regional art ship as independent packs. Combat readability stays global; cultural expression is tuned with local players.",
      markets: zhCN.global.markets,
    },
    chronicles: {
      eyebrow: "WORLD CHRONICLES",
      title: "The world is moving",
      items: [
        { date: "2026.08", tag: "WORLD", title: "The shared world expands", copy: "Authoritative zones, movement and visibility continue into a verifiable Candidate build." },
        { date: "2026.08", tag: "CLIENT", title: "Cross-platform evolution", copy: "Desktop, web and mobile are converging on one character and one world state." },
        { date: "2026.08", tag: "GLOBAL", title: "Regional art system begins", copy: "Site and campaign art will support independent versions, fallback and local review." },
      ],
    },
    watch: {
      eyebrow: "NUMERON LIVE // WORLD SIGNAL",
      title: "The world is moving.\nLegends are being recorded.",
      copy: "AI direction selects public world events, creates commentary and packages highlight clips. This prototype switches to real broadcasts and replays when production channels are connected.",
      live: "World live",
      demo: "DEMO FEED",
      liveTitle: "Mir World 01 · Observatory",
      liveCopy: "Only delayed, privacy-filtered public events are shown. Exact coordinates, private assets and sensitive tactics remain hidden.",
      enter: "Enter the world",
      openAtlas: "Open Atlas",
      highlightsEyebrow: "HIGHLIGHT ARCHIVE",
      highlightsTitle: "Today's highlights",
      highlightsCopy: "One world event can become horizontal, vertical and localized editions while a shared content ID measures registration and StartGame conversion.",
      clips: [
        { tag: "SABUK", duration: "00:37", title: "The final gate", copy: "The defenders regroup in the final minute and turn the battle inside the gate." },
        { tag: "WORLD FIRST", duration: "00:29", title: "The first fall of Red Moon", copy: "Three parties contest the final blow as a rare drop enters the public record." },
        { tag: "PVP", duration: "00:22", title: "Seven steps later", copy: "A wounded warrior uses the terrain to reverse a three-player pursuit." },
      ],
      channelsEyebrow: "GLOBAL DISTRIBUTION",
      channelsTitle: "One event, many local expressions",
      channelsCopy: "The facts and content ID stay shared while subtitles, voice, titles and thumbnails adapt by region—all leading back to the same site and game entry.",
    },
    membership: {
      eyebrow: "NUMERON TOKEN // AI MEMBERSHIP",
      title: "Not a monthly reward pack.\nA living layer of AI services.",
      copy: "NUMERON Token is an account-bound service subscription for adventure, creation and world memory. It never sells combat power and is not a tradable or appreciating asset.",
      serviceBadge: "ACCOUNT-BOUND SERVICE",
      prototypePrice: "Prototype pricing · checkout unavailable",
      monthly: "Monthly",
      annual: "Annual",
      annualNote: "Save about 16% annually",
      perMonth: "/ month",
      perYear: "/ year",
      recommended: "Recommended",
      unavailable: "Coming soon",
      plans: [
        { id: "free", name: "Wanderer", tagline: "Enter the world and begin your first legend", monthlyPrice: "Free", annualPrice: "Free", features: ["Full core game access", "Public Atlas and world pulse", "One daily AI world brief", "Community live and highlights"] },
        { id: "token", name: "Rebirth Token", tagline: "A personal AI layer for long-term adventurers", monthlyPrice: "$9", annualPrice: "$90", features: ["200 AI companion conversations monthly", "Character growth and equipment explanations", "Personal weekly and relationship recap", "10 AI highlight clips monthly", "Four-language captions and basic voice", "Subscriber identity cosmetic"] },
        { id: "architect", name: "Architect", tagline: "For guild organizers and content creators", monthlyPrice: "$24", annualPrice: "$240", features: ["1,000 AI companion conversations monthly", "Public guild-data analysis assistant", "60 AI highlight clips monthly", "Localized captions, voice and title options", "Priority inference and video queue", "Creator analytics and attribution panel"] },
      ],
      usageEyebrow: "USAGE, NOT POWER",
      usageTitle: "An AI-style subscription with visible limits.",
      usageCopy: "Usage applies only to AI compute, content generation and convenience. Core play, combat outcomes and loot are never sold by tier.",
      usage: [
        { label: "AI companion", free: "1 / day", token: "200 / mo", architect: "1,000 / mo" },
        { label: "Highlight generation", free: "—", token: "10 / mo", architect: "60 / mo" },
        { label: "Localized voice", free: "—", token: "Basic", architect: "Priority" },
        { label: "Public guild analysis", free: "—", token: "—", architect: "Included" },
      ],
      fairnessEyebrow: "FAIR WORLD CONTRACT",
      fairnessTitle: "Pay for tools, never victory.",
      fairnessCopy: "A subscription helps players understand the world, preserve stories and organize communities. It cannot buy damage, drop rates, movement speed or hidden intelligence.",
      fairnessPoints: ["No combat-stat increases", "No improved rare-drop odds", "No sale of private player data", "Account-bound and non-transferable", "Characters and generated work remain after cancellation"],
      faqEyebrow: "SUBSCRIPTION FAQ",
      faqTitle: "What must be clear before subscribing",
      faq: [
        { question: "Is Token a cryptocurrency?", answer: "No. In this design it is an account-bound service credential. It cannot be traded and carries no price or appreciation promise." },
        { question: "Does subscribing make my character stronger?", answer: "No. Paid usage covers AI conversation, summaries, creation and creator tools without changing combat or loot." },
        { question: "Can I cancel at any time?", answer: "The production payment flow should allow cancellation at any time. Benefits last through the billing period; characters and existing content remain." },
        { question: "Does AI read private chat?", answer: "Not by default. Only explicitly authorized data is used, with separate controls and auditable notices for broadcasting, analysis and training." },
      ],
    },
    finalCta: { eyebrow: "THE GATE IS OPEN", title: "Let the world remember your name.", copy: "Choose your path. Enter Bichon. Begin a journey written by your own hand.", button: "Enter Mir now" },
    footer: { note: "A persistent world shaped by its players.", legal: "NUMERON · LEGEND OF REBIRTH", game: "Game", data: "World data", watch: "Watch", regions: "Regions", enter: "Enter game", worldPulse: "World pulse" },
  },
  ja: {
    ...zhCN,
    languageName: "日本語",
    nav: { explore: "ゲーム世界", world: "世界", classes: "職業", global: "地域版", pulse: "ワールド動向", watch: "ライブ", highlights: "ハイライト", chronicles: "年代記", atlas: "天機閣", token: "TOKEN", locale: "地域", menu: "メニュー", home: "ホーム" },
    atlas: { ...zhCN.atlas, title: "世界は止まらない。\nすべては記録される。", copy: "ゲームを起動せずに、公開キャラクター、ギルド、装備、市場、世界イベントを確認できます。正確な位置や非公開資産は公開されません。", button: "ワールドアトラスを開く", live: "ミル・ワールド01 稼働中" },
    hero: { ...zhCN.hero, titleTop: "LEGEND OF", titleBottom: "REBIRTH", description: "ミルの大地へ帰ろう。ひとつの世界で仲間と戦い、あなた自身の名で次の伝説を刻む。", primary: "世界へ入る", secondary: "世界を探索", status: "ワールド稼働中" },
    world: { ...zhCN.world, title: "インスタンスではない。\n共に生きる、ひとつの世界。", copy: "街、荒野、迷宮、そして沙巴克。すべてが同じ大陸でつながり、プレイヤーの選択で動き続けます。" },
    classes: { ...zhCN.classes, title: "三つの道、ひとつの伝説", copy: "力、元素、道術は手段にすぎない。誰と共に立つかが、あなたの名を決める。" },
    global: { ...zhCN.global, title: "ひとつのミルを、\nすべての地域の物語へ。", copy: "言語、書体、音声、地域ビジュアルを独立配信。戦闘の視認性は共通のまま、文化表現を現地と共に磨きます。" },
    chronicles: { ...zhCN.chronicles, title: "世界は動き続ける" },
    watch: {
      ...zhCN.watch,
      title: "世界は動き続け、\n伝説は記録される。",
      copy: "AIディレクターが公開ワールドイベントを選び、実況とハイライトを生成します。現在はプロトタイプデータを使用しています。",
      live: "ワールドライブ",
      liveTitle: "ミル・ワールド01 · 観測所",
      liveCopy: "遅延処理とプライバシー保護を施した公開イベントのみを表示し、正確な座標や非公開資産、機密性の高い戦術情報は公開しません。",
      enter: "世界へ入る",
      openAtlas: "天機閣を開く",
      highlightsTitle: "今日のハイライト",
      highlightsCopy: "ひとつのワールドイベントから横型、縦型、地域別バージョンを生成し、共通のコンテンツIDで登録とゲーム開始を計測します。",
      clips: [
        { tag: "沙巴克", duration: "00:37", title: "最後の城門", copy: "防衛側が終了直前に再集結し、城門内部で戦況を覆す。" },
        { tag: "WORLD FIRST", duration: "00:29", title: "赤月悪魔、初討伐", copy: "三つのパーティーが最後の一撃を争い、希少ドロップが公開記録に刻まれる。" },
        { tag: "PVP", duration: "00:22", title: "七歩のあと", copy: "瀕死の戦士が地形を利用し、三人の追撃を逆転する。" },
      ],
      channelsTitle: "ひとつの出来事を、各地域の表現へ",
      channelsCopy: "事実とコンテンツIDは共通のまま、字幕、音声、タイトル、サムネイルを地域ごとに最適化し、同じ公式サイトとゲーム入口へ導きます。",
    },
    membership: {
      ...zhCN.membership,
      title: "月額報酬パックではない。\n進化し続けるAIサービス。",
      copy: "NUMERON Tokenは、冒険、創作、世界の記録を支援するアカウント連携型サービスです。戦闘力を販売せず、取引や値上がりを目的とする資産でもありません。",
      prototypePrice: "プロトタイプ価格 · 購入機能は未公開",
      monthly: "月払い",
      annual: "年払い",
      annualNote: "年払いで約16%お得",
      perMonth: "/ 月",
      perYear: "/ 年",
      recommended: "おすすめ",
      unavailable: "近日公開",
      plans: [
        { id: "free", name: "旅人", tagline: "世界に入り、最初の伝説を始める", monthlyPrice: "無料", annualPrice: "無料", features: ["ゲーム本編への基本アクセス", "公開天機閣とワールド動向", "1日1回のAIワールド要約", "コミュニティ配信とハイライト"] },
        { id: "token", name: "Rebirth Token", tagline: "長く旅するプレイヤーのための個人AIレイヤー", monthlyPrice: "¥1,480", annualPrice: "¥14,800", features: ["月200回のAIコンパニオン対話", "育成と装備の解説", "個人週報と関係性の振り返り", "月10本のAIハイライト", "4言語字幕と基本音声", "購読者限定の外観"] },
        { id: "architect", name: "アーキテクト", tagline: "ギルド運営者とクリエイター向け", monthlyPrice: "¥3,480", annualPrice: "¥34,800", features: ["月1,000回のAIコンパニオン対話", "公開ギルドデータ分析", "月60本のAIハイライト", "多言語字幕、音声、タイトル案", "優先AI・動画キュー", "クリエイター分析と流入計測"] },
      ],
      usageTitle: "AI製品のように、利用枠を明確に。",
      usageCopy: "利用枠はAI計算、コンテンツ生成、利便機能だけに適用されます。戦闘結果やドロップをプラン別に販売しません。",
      usage: [
        { label: "AIコンパニオン", free: "1 / 日", token: "200 / 月", architect: "1,000 / 月" },
        { label: "ハイライト生成", free: "—", token: "10 / 月", architect: "60 / 月" },
        { label: "地域音声", free: "—", token: "基本", architect: "優先" },
        { label: "公開ギルド分析", free: "—", token: "—", architect: "含む" },
      ],
      fairnessTitle: "購入するのは道具であり、勝利ではない。",
      fairnessCopy: "世界の理解、物語の保存、コミュニティ運営を支援しますが、攻撃力、ドロップ率、移動速度、非公開情報は販売しません。",
      fairnessPoints: ["戦闘能力を上げない", "希少ドロップ率を上げない", "非公開プレイヤーデータを販売しない", "アカウント連携で譲渡不可", "解約後もキャラクターと生成物を保持"],
      faqTitle: "購読前に明確にすること",
      faq: [
        { question: "Tokenは暗号資産ですか？", answer: "いいえ。アカウント連携型のサービス利用資格であり、取引や値上がりを約束するものではありません。" },
        { question: "キャラクターは強くなりますか？", answer: "いいえ。AI対話、要約、制作ツールの利用枠だけで、戦闘やドロップは変化しません。" },
        { question: "いつでも解約できますか？", answer: "正式版ではいつでも解約でき、期間終了まで利用可能にします。キャラクターと既存コンテンツは残ります。" },
        { question: "AIは非公開チャットを読みますか？", answer: "初期設定では読みません。明示的に許可されたデータだけを使用し、用途別の設定と説明を提供します。" },
      ],
    },
    finalCta: { ...zhCN.finalCta, title: "世界が、あなたの名を待っている。", copy: "職業を選び、比奇へ。自らの手で物語を始めよう。", button: "今すぐミルへ" },
    footer: { note: "プレイヤーと共に形作る、持続する世界。", legal: "NUMERON · LEGEND OF REBIRTH", game: "ゲーム", data: "ワールドデータ", watch: "見る", regions: "地域", enter: "ゲームへ", worldPulse: "ワールド動向" },
  },
  ko: {
    ...zhCN,
    languageName: "한국어",
    nav: { explore: "게임 세계", world: "세계", classes: "직업", global: "지역 버전", pulse: "월드 동향", watch: "라이브", highlights: "하이라이트", chronicles: "연대기", atlas: "천기각", token: "TOKEN", locale: "지역", menu: "메뉴", home: "홈" },
    atlas: { ...zhCN.atlas, title: "세계는 멈추지 않습니다.\n모든 순간이 기록됩니다.", copy: "게임에 접속하지 않아도 공개 캐릭터, 길드, 장비, 시장과 월드 이벤트를 확인할 수 있습니다. 정확한 위치와 비공개 자산은 공개하지 않습니다.", button: "월드 아틀라스 열기", live: "미르 월드 01 온라인" },
    hero: { ...zhCN.hero, titleTop: "LEGEND OF", titleBottom: "REBIRTH", description: "미르 대륙으로 돌아오세요. 하나의 살아 있는 세계에서 동료와 싸우고, 당신의 이름으로 다음 전설을 쓰세요.", primary: "세계 입장", secondary: "세계 탐험", status: "월드 서비스 온라인" },
    world: { ...zhCN.world, title: "인스턴스가 아닌,\n함께 살아가는 하나의 세계.", copy: "도시와 황야, 던전과 사북이 하나의 대륙으로 이어지고 플레이어의 선택으로 계속 변화합니다." },
    classes: { ...zhCN.classes, title: "세 개의 길, 하나의 전설", copy: "힘과 원소, 도술은 수단일 뿐입니다. 누구와 함께 서는지가 당신의 이름을 결정합니다." },
    global: { ...zhCN.global, title: "하나의 미르,\n모든 지역의 이야기로.", copy: "언어, 글꼴, 음성, 지역 비주얼을 독립 패키지로 제공합니다. 전투 가독성은 통일하고 문화 표현은 현지와 함께 다듬습니다." },
    chronicles: { ...zhCN.chronicles, title: "세계는 계속 움직인다" },
    watch: {
      ...zhCN.watch,
      title: "세계는 움직이고,\n전설은 기록됩니다.",
      copy: "AI 디렉터가 공개 월드 이벤트를 선택해 해설과 하이라이트를 제작합니다. 현재 페이지는 프로토타입 데이터를 사용합니다.",
      live: "월드 라이브",
      liveTitle: "미르 월드 01 · 관측소",
      liveCopy: "지연 처리와 개인정보 보호를 거친 공개 이벤트만 보여주며 정확한 좌표, 비공개 자산과 민감한 전술 정보는 노출하지 않습니다.",
      enter: "세계 입장",
      openAtlas: "천기각 열기",
      highlightsTitle: "오늘의 하이라이트",
      highlightsCopy: "하나의 월드 이벤트를 가로형, 세로형, 지역별 버전으로 만들고 공통 콘텐츠 ID로 가입과 게임 시작 전환을 측정합니다.",
      clips: [
        { tag: "사북", duration: "00:37", title: "마지막 성문", copy: "수비대가 종료 직전 다시 집결해 성문 안에서 전세를 뒤집습니다." },
        { tag: "WORLD FIRST", duration: "00:29", title: "적월악마 최초 처치", copy: "세 파티가 마지막 일격을 다투고 희귀 전리품이 공개 기록에 남습니다." },
        { tag: "PVP", duration: "00:22", title: "일곱 걸음 뒤", copy: "빈사 상태의 전사가 지형을 이용해 세 명의 추격을 역전합니다." },
      ],
      channelsTitle: "하나의 사건, 지역마다 다른 표현",
      channelsCopy: "사실과 콘텐츠 ID는 공유하되 자막, 음성, 제목과 썸네일은 지역에 맞추고 모두 같은 공식 사이트와 게임 입구로 연결합니다.",
    },
    membership: {
      ...zhCN.membership,
      title: "월간 보상 상자가 아닌,\n계속 진화하는 AI 서비스.",
      copy: "NUMERON Token은 모험, 창작과 월드 기록을 위한 계정 귀속형 서비스 구독입니다. 전투력을 판매하지 않으며 거래나 가치 상승을 약속하는 자산도 아닙니다.",
      prototypePrice: "프로토타입 가격 · 결제 미오픈",
      monthly: "월간",
      annual: "연간",
      annualNote: "연간 결제 약 16% 절약",
      perMonth: "/ 월",
      perYear: "/ 년",
      recommended: "추천",
      unavailable: "곧 공개",
      plans: [
        { id: "free", name: "여행자", tagline: "세계에 들어와 첫 전설을 시작하세요", monthlyPrice: "무료", annualPrice: "무료", features: ["핵심 게임 기본 이용", "공개 천기각과 월드 동향", "하루 1회 AI 월드 브리핑", "커뮤니티 라이브와 하이라이트"] },
        { id: "token", name: "Rebirth Token", tagline: "장기 모험가를 위한 개인 AI 레이어", monthlyPrice: "₩12,000", annualPrice: "₩120,000", features: ["월 200회 AI 동료 대화", "캐릭터 성장과 장비 설명", "개인 주간 보고와 관계 회고", "월 10개 AI 하이라이트", "4개 언어 자막과 기본 음성", "구독자 전용 외형"] },
        { id: "architect", name: "아키텍트", tagline: "길드 운영자와 콘텐츠 제작자용", monthlyPrice: "₩29,000", annualPrice: "₩290,000", features: ["월 1,000회 AI 동료 대화", "공개 길드 데이터 분석", "월 60개 AI 하이라이트", "다국어 자막·음성·제목 제안", "우선 AI 및 영상 대기열", "제작자 분석과 유입 추적"] },
      ],
      usageTitle: "AI 제품처럼 사용량을 명확하게.",
      usageCopy: "사용량은 AI 연산, 콘텐츠 생성과 편의 기능에만 적용됩니다. 전투 결과와 전리품은 구독 등급으로 판매하지 않습니다.",
      usage: [
        { label: "AI 동료", free: "1 / 일", token: "200 / 월", architect: "1,000 / 월" },
        { label: "하이라이트 생성", free: "—", token: "10 / 월", architect: "60 / 월" },
        { label: "지역 음성", free: "—", token: "기본", architect: "우선" },
        { label: "공개 길드 분석", free: "—", token: "—", architect: "포함" },
      ],
      fairnessTitle: "도구를 구매하되 승리를 구매하지 않습니다.",
      fairnessCopy: "세계를 이해하고 이야기를 기록하며 커뮤니티를 운영하도록 돕지만 공격력, 드롭률, 이동 속도나 비공개 정보는 판매하지 않습니다.",
      fairnessPoints: ["전투 능력치 증가 없음", "희귀 드롭 확률 증가 없음", "비공개 플레이어 데이터 판매 없음", "계정 귀속 및 양도 불가", "해지 후 캐릭터와 생성물 유지"],
      faqTitle: "구독 전에 분명히 할 점",
      faq: [
        { question: "Token은 암호화폐인가요?", answer: "아닙니다. 계정에 귀속된 서비스 이용 자격이며 거래나 가치 상승을 약속하지 않습니다." },
        { question: "구독하면 캐릭터가 강해지나요?", answer: "아닙니다. AI 대화, 요약과 제작 도구 사용량만 제공하며 전투와 전리품은 바뀌지 않습니다." },
        { question: "언제든 해지할 수 있나요?", answer: "정식 결제에서는 언제든 해지하고 기간 종료까지 이용하도록 설계합니다. 캐릭터와 기존 콘텐츠는 유지됩니다." },
        { question: "AI가 비공개 채팅을 읽나요?", answer: "기본값은 아닙니다. 명시적으로 허용된 데이터만 사용하며 용도별 설정과 감사 가능한 설명을 제공합니다." },
      ],
    },
    finalCta: { ...zhCN.finalCta, title: "세계가 당신의 이름을 기다립니다.", copy: "직업을 선택하고 비천으로 향하세요. 직접 써 내려갈 여정을 시작하세요.", button: "지금 미르 입장" },
    footer: { note: "플레이어가 함께 만들어가는 지속 세계.", legal: "NUMERON · LEGEND OF REBIRTH", game: "게임", data: "월드 데이터", watch: "시청", regions: "지역", enter: "게임 입장", worldPulse: "월드 동향" },
  },
};

export function isLocale(value: string): value is Locale {
  return locales.includes(value as Locale);
}

export function getCopy(locale: Locale): SiteCopy {
  return translations[locale];
}
