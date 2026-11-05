import type { LegalDocument, ProductCopy } from "@/lib/product-copy";
import type { Locale } from "@/lib/site-copy";
import { operationsUpdatedAt, operatorLegalName, refundWindowDays } from "@/lib/operations-copy";

// These are separate Chinese source drafts, not legally approved policies.
// Do not reuse an arbitrary translated document to fill another document type.
export const legalSourceLocale = "zh-CN" as const;
export const legalSourceUpdatedAt = operationsUpdatedAt;

export const legalSourceDocuments: ProductCopy["legal"] = {
  privacy: {
    title: "隐私政策",
    summary: `说明 ${operatorLegalName} 提供 Numeron 服务时涉及的账号、游戏、充值与钱包数据，以及仍需落实的保存和权利请求安排。本文尚未完成法律审校。`,
    sections: [
      { title: "服务方与受理渠道", copy: `Numeron 的开发与服务方为 ${operatorLegalName}。隐私相关问题可通过本网站支持页列示的渠道提出。公司地址、实际处理责任分工及适用地区的补充信息将在正式收费前完善。` },
      { title: "账号与钱包认证", copy: "游戏认证涉及账号标识、关联钱包公开地址、公钥、网络以及必要的签名验证与短期会话信息，用于验证登录和维护账户安全。请勿向游戏、客服或其他玩家提供钱包私钥、助记词或密码。钱包的账户恢复与密钥处理以实际钱包服务说明为准。" },
      { title: "游戏、订单与消费记录", copy: "游戏服务处理角色、进度、物品、订单、点券余额、消费和月卡状态等记录，用于提供服务、确认权益、对账及处理付款争议。购买时付款资料在所选支付渠道页面提交；游戏订单关联渠道交易标识和核对所需信息。必要字段、处理依据和完整日志范围仍按实际启用功能核对。" },
      { title: "第三方支付与 Halliday", copy: "钱包 USDC 买卖通过 Halliday 聚合入口接入，实际兑换由交易中所选第三方服务商提供。身份核验、付款和兑换资料按该交易页面列示的服务商条款与隐私说明处理。游戏充值支付商、钱包、Halliday 及下游服务商的共享字段、处理地区和责任分工须分别核实，不能推定它们均由同一主体处理。" },
      { title: "保存、备份与跨境访问", copy: "按服务、安全、财务对账与适用法律需要分别确定保存范围。完整服务商名录、数据处理地区、保存期限、备份清除和运营人员跨境访问安排仍在核对，正式收费前应补齐。当前不以本页承诺固定天数的自动清除，也不会把所有记录永久保存作为默认规则。" },
      { title: "访问、更正、导出与删除", copy: "玩家可通过支持页提出数据问题及访问、更正、导出、删除或异议请求。受理流程、适用期限和身份验证方式将在正式服务前落实；目前未宣称已经提供自助导出或自动删除。必要财务、争议或法定记录依法单独处理，不能承诺删除公链上的交易记录。" },
      { title: "公开内容与 AI", copy: "私人聊天、未公开资产和身份信息不因参加测试而被默认授权用于公开展示或 AI 训练。规划中的 AI 功能需要另行说明实际数据用途、服务商及用户选择；当前 Atlas 原型未连接真实玩家或交易数据。" },
    ],
  },
  terms: {
    title: "用户协议",
    summary: `说明 ${operatorLegalName} 与 Numeron 游戏、点券、月卡、钱包入口及素材替换的关系。游戏尚未正式上线，当前未开放购买；本文尚未完成法律审校。`,
    sections: [
      { title: "开发与服务方", copy: `Numeron 由 ${operatorLegalName} 开发并提供游戏服务。Obelisk Labs 同时开发 Dubhe 基础设施与工具及 Dubhe Wallet；各项产品的具体服务与第三方处理分工分别说明。正式收费的合同、收据主体、公司联系资料和地区补充条款将在开放购买前明确。` },
      { title: "游戏与测试范围", copy: "Numeron 仍在开发中，已邀请外部玩家参加测试。测试开放内容、数据调整和重置范围应以对应测试公告为准；参与测试不等于全部规划功能已经上线。" },
      { title: "点券、月卡与 AI 订阅", copy: "当前公开的唯一已确认档位为 USD 1＝100 游戏点券，网页尚不接受购买。充值点券、使用点券购买游戏内月卡及规划中的 AI 订阅分别说明；充值到账不会自动开通月卡、AI 订阅或推广员资格。税费、服务地区、有效期与交付等细节将在正式购买前展示，单次充值不自动续费。" },
      { title: "钱包、Halliday 与玩家交易", copy: "钱包登录用于账户认证，资产转账应另行核对授权内容。钱包 USDC 买卖通过 Halliday 聚合入口接入，实际兑换由所选第三方服务商按其条款、费用、核验与地区条件提供。游戏点券充值、第三方兑换和玩家交易分别处理；本页不保证任何资产可变现，也不承诺所有地区或付款方式均可用。" },
      { title: "账户安全与公平游戏", copy: "玩家应保护账户凭据，不得使用外挂、利用漏洞、实施欺诈或骚扰，亦不得通过未经授权的自动化方式破坏游戏、交易或排名。请勿向客服或其他玩家提供密码、钱包私钥或助记词。" },
      { title: "素材替换与权利", copy: "现有过渡素材正在逐步替换为自有素材，部分测试画面仍含 Crystal 临时素材。素材、名称、声音、代码与衍生资源的权利和商业发行依据分别核对。目前不宣称替换或全部权属审查已经完成。" },
      { title: "正式服务前的补充规则", copy: "服务地区、年龄要求、账户限制与申诉流程、点券和可购买物品的使用/转移范围、有效期及清档时已购权益处理，需要与实际运营和渠道条件一致。适用法律、争议处理和当地发行要求应分别核对；当前官网发布不等于已获生产收款或金融业务准入。" },
    ],
  },
  refunds: {
    title: "充值与退款说明",
    summary: `说明 ${operatorLegalName} 为正式开放后的单次点券充值预定的退款规则：交付起 ${refundWindowDays} 个自然日内整笔未消费可申请原路全退，并保留法定权益。当前未开放购买，退款执行流程仍须验收。`,
    sections: [
      { title: "适用商品与当前状态", copy: "本页针对单次 Numeron 游戏点券充值，已确认档位为 USD 1＝100 点券。游戏内月卡是独立消费，规划中的 AI 订阅和第三方 USDC 买卖分别适用其商品及服务说明。当前网页不提供真实付款，下述销售规则在正式开放购买时展示；完整退款执行仍须完成验收。" },
      { title: `${refundWindowDays} 天未消费整单退款`, copy: `公开收费后，从充值点券交付起 ${refundWindowDays} 个自然日内，若该整笔充值的付费点券均未消费，玩家可申请按原支付方式整单全额退款。这项安排不压缩适用法律或支付渠道规定的申请期限，也不取代未交付、误扣等其他退款权利。` },
      { title: "未到账、重复扣款与系统错误", copy: "玩家可保留原订单号、角色、服务器、支付时间及必要的脱敏付款凭据，通过支持页请求核对。经核实的未交付、重复扣款或系统错误，按实际问题、玩家权利与渠道规则补发或退款。不要为查询原订单重复付款，也不要提交密码、私钥、助记词或完整银行卡资料。" },
      { title: "已消费、月卡与第三人权益", copy: "已消费订单按问题情况、支付渠道规则和适用法律审查，不能仅因点券已消费而一律拒绝法定退款。涉及已激活月卡、已转移物品或其他玩家权益时，需要核对对应消费与来源，采取可审计的个案处理；不会因单笔申请直接回滚整个角色存档或扣除无关物品。" },
      { title: "申请、审核与原路退款", copy: "退款问题通过支持页列示渠道提交，人工核对后告知处理结论与渠道进度。法币购买优先退回原支付方式；原渠道无法退款时采用经渠道及适用规则认可的替代流程，不擅自改成 USDC 退款。申请受理、审核通过、渠道处理中和退款成功是不同阶段，本页不承诺自动退款或固定银行到账时间。" },
      { title: "清档、停服与无法继续提供的权益", copy: "正式收费前必须明确测试清档安排及已购权益处理。如果公司清档、停服或无法提供已购服务导致付费权益无法继续使用，应按实际情况与适用义务提供对应退款方案，不能强制以赠送测试点券替代应退款项。目前不假定测试余额会永久保留或自动转入正式服。" },
      { title: "消费者权利与渠道规则", copy: "不可排除的消费者权利优先适用。十四天自愿退款窗口不限制依法或依实际支付渠道合同提出的其他申请。退款及支付业务分工以实际签约产品和订单为依据，不将某一渠道的 MoR 模板自动用于所有支付方式；当前网站更新也不表示 Waffo 审核或正式收款已经开通。" },
    ],
  },
  "ai-data": {
    title: "AI 数据与生成内容说明",
    summary: "AI 世界导演、自动切片及相关官网订阅仍在规划中。本草案说明未来服务需要确认的数据与授权边界，不表示功能已经提供。",
    sections: [
      { title: "规划中的功能", copy: "外部开发者扩展、AI 世界导演与自动切片仍属于规划内容。官网展示的服务设想和订阅页面，不构成现已可用、已收费或已有真实用户使用的证明。" },
      { title: "数据输入与授权待确认", copy: "如未来开放 AI 功能，应逐项说明需要哪些游戏事件、用户提交内容或其他数据，以及相应授权、可见范围与用户选择。不能以本草案推定私人聊天、未公开资产或身份信息已经获准用于生成内容。" },
      { title: "服务商、训练与保存待确认", copy: "实际模型和服务商、数据是否用于训练、保存期限、跨境处理与删除流程尚待确认。正式功能开放前需要按实际实现单独说明，不能将规划中的授权或开关写成已生效能力。" },
      { title: "生成内容与纠错", copy: "未来生成内容的展示、发布审核、来源说明、投诉和纠错流程需要与实际运营一致。本草案不保证生成内容准确，也不替代正式服务条款。" },
    ],
  },
};

type LegalPresentation = {
  notice: string;
  sourceLink: string;
  rechargeLink: string;
  labels: { privacy: string; terms: string; refunds: string; aiData: string };
};

// Navigation and fallback notices only; these are not translated legal policies.
const legalPresentation = {
  "zh-CN": { notice: "以下中文政策草案已按已确认业务信息更新，尚未完成法律审校。当前未开放购买，部分处理细节仍须在正式收费前落实。", sourceLink: "查看中文原文草案", rechargeLink: "查看点券充值说明", labels: { privacy: "隐私政策", terms: "用户协议", refunds: "充值与退款", aiData: "AI 数据说明" } },
  "zh-TW": { notice: "本語言的法律譯文尚未審校，以下顯示簡體中文原文草案；原文也尚未完成法律審校。", sourceLink: "查看簡體中文原文草案", rechargeLink: "查看點數儲值說明", labels: { privacy: "隱私政策", terms: "使用者協議", refunds: "儲值與退款", aiData: "AI 資料說明" } },
  en: { notice: "The legal translation for this language has not been reviewed. The Chinese source draft is shown below; the source has not completed legal review either.", sourceLink: "View the Chinese source draft", rechargeLink: "View game credit top-up information", labels: { privacy: "Privacy", terms: "Terms", refunds: "Top-ups and refunds", aiData: "AI data information" } },
  ja: { notice: "この言語の法的文書の翻訳は未校閲です。以下は簡体字中国語の原文草案で、原文も法的審査を完了していません。", sourceLink: "中国語の原文草案を見る", rechargeLink: "ゲーム内クレジットのチャージ説明", labels: { privacy: "プライバシー", terms: "利用規約", refunds: "チャージと返金", aiData: "AI データの説明" } },
  ko: { notice: "이 언어의 법률 문서 번역은 검토되지 않았습니다. 아래는 중국어 간체 원문 초안이며 원문 역시 법률 검토를 완료하지 않았습니다.", sourceLink: "중국어 원문 초안 보기", rechargeLink: "게임 크레딧 충전 안내", labels: { privacy: "개인정보", terms: "이용약관", refunds: "충전과 환불", aiData: "AI 데이터 안내" } },
  es: { notice: "La traducción jurídica a este idioma no ha sido revisada. A continuación se muestra el borrador original en chino, que tampoco ha completado la revisión jurídica.", sourceLink: "Ver el borrador original en chino", rechargeLink: "Ver información de recarga de créditos", labels: { privacy: "Privacidad", terms: "Condiciones", refunds: "Recargas y reembolsos", aiData: "Información sobre datos de IA" } },
  "pt-BR": { notice: "A tradução jurídica para este idioma não foi revisada. Abaixo está o rascunho original em chinês, que também não concluiu a revisão jurídica.", sourceLink: "Ver o rascunho original em chinês", rechargeLink: "Ver informações sobre recarga de créditos", labels: { privacy: "Privacidade", terms: "Termos", refunds: "Recargas e reembolsos", aiData: "Informações sobre dados de IA" } },
  de: { notice: "Die rechtliche Übersetzung für diese Sprache wurde nicht geprüft. Unten steht der chinesische Originalentwurf; auch dessen rechtliche Prüfung ist noch nicht abgeschlossen.", sourceLink: "Chinesischen Originalentwurf ansehen", rechargeLink: "Informationen zum Spielguthaben", labels: { privacy: "Datenschutz", terms: "Nutzungsbedingungen", refunds: "Aufladungen und Erstattungen", aiData: "Informationen zu KI-Daten" } },
  fr: { notice: "La traduction juridique dans cette langue n'a pas été révisée. Le projet original en chinois est présenté ci-dessous ; son examen juridique n'est pas non plus terminé.", sourceLink: "Voir le projet original en chinois", rechargeLink: "Voir les informations de recharge", labels: { privacy: "Confidentialité", terms: "Conditions", refunds: "Recharges et remboursements", aiData: "Informations sur les données IA" } },
  ru: { notice: "Юридический перевод на этот язык не проверен. Ниже показан исходный проект на китайском языке; его юридическая проверка также не завершена.", sourceLink: "Открыть исходный проект на китайском", rechargeLink: "Информация о пополнении кредитов", labels: { privacy: "Конфиденциальность", terms: "Условия", refunds: "Пополнения и возвраты", aiData: "Данные ИИ" } },
  pl: { notice: "Tłumaczenie prawne na ten język nie zostało sprawdzone. Poniżej znajduje się chiński projekt źródłowy, którego weryfikacja prawna również nie została zakończona.", sourceLink: "Zobacz chiński projekt źródłowy", rechargeLink: "Informacje o doładowaniu kredytów", labels: { privacy: "Prywatność", terms: "Warunki", refunds: "Doładowania i zwroty", aiData: "Informacje o danych AI" } },
  tr: { notice: "Bu dildeki hukuki çeviri incelenmemiştir. Aşağıda Çince kaynak taslak gösterilmektedir; kaynağın hukuki incelemesi de tamamlanmamıştır.", sourceLink: "Çince kaynak taslağı görüntüle", rechargeLink: "Oyun kredisi yükleme bilgileri", labels: { privacy: "Gizlilik", terms: "Koşullar", refunds: "Yüklemeler ve iadeler", aiData: "Yapay zekâ veri bilgileri" } },
  th: { notice: "คำแปลเอกสารกฎหมายภาษานี้ยังไม่ได้ตรวจทาน ด้านล่างเป็นร่างต้นฉบับภาษาจีน ซึ่งยังตรวจสอบทางกฎหมายไม่เสร็จเช่นกัน", sourceLink: "ดูร่างต้นฉบับภาษาจีน", rechargeLink: "ดูข้อมูลเติมเครดิตเกม", labels: { privacy: "ความเป็นส่วนตัว", terms: "ข้อกำหนด", refunds: "การเติมเครดิตและคืนเงิน", aiData: "ข้อมูลเกี่ยวกับข้อมูล AI" } },
  vi: { notice: "Bản dịch pháp lý cho ngôn ngữ này chưa được thẩm định. Dưới đây là bản nháp gốc tiếng Trung; bản gốc cũng chưa hoàn tất rà soát pháp lý.", sourceLink: "Xem bản nháp gốc tiếng Trung", rechargeLink: "Xem thông tin nạp điểm trò chơi", labels: { privacy: "Quyền riêng tư", terms: "Điều khoản", refunds: "Nạp điểm và hoàn tiền", aiData: "Thông tin dữ liệu AI" } },
  id: { notice: "Terjemahan hukum untuk bahasa ini belum ditinjau. Di bawah ini ditampilkan draf sumber berbahasa Mandarin; tinjauan hukumnya juga belum selesai.", sourceLink: "Lihat draf sumber berbahasa Mandarin", rechargeLink: "Lihat informasi isi ulang kredit", labels: { privacy: "Privasi", terms: "Ketentuan", refunds: "Isi ulang dan pengembalian dana", aiData: "Informasi data AI" } },
  ar: { notice: "لم تُراجع الترجمة القانونية لهذه اللغة. تظهر أدناه المسودة الأصلية بالصينية، ولم تكتمل مراجعتها القانونية أيضًا.", sourceLink: "عرض المسودة الأصلية بالصينية", rechargeLink: "عرض معلومات شحن رصيد اللعبة", labels: { privacy: "الخصوصية", terms: "الشروط", refunds: "الشحن والاسترداد", aiData: "معلومات بيانات الذكاء الاصطناعي" } },
} satisfies Record<Locale, LegalPresentation>;

export function getLegalPresentation(locale: Locale): LegalPresentation {
  return legalPresentation[locale];
}

export function getLegalSourceDocument(document: LegalDocument) {
  return legalSourceDocuments[document];
}
