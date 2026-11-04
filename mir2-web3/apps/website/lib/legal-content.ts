import type { LegalDocument, ProductCopy } from "@/lib/product-copy";
import type { Locale } from "@/lib/site-copy";

// These are separate Chinese source drafts, not legally approved policies.
// Do not reuse an arbitrary translated document to fill another document type.
export const legalSourceLocale = "zh-CN" as const;
export const legalSourceUpdatedAt = "2026-11-05";

export const legalSourceDocuments: ProductCopy["legal"] = {
  privacy: {
    title: "隐私政策",
    summary: "说明 Numeron 游戏、充值与钱包相关的数据类别和仍需核对的处理规则。本文是中文原文草案，尚未完成法律审校。",
    sections: [
      { title: "游戏与充值记录", copy: "游戏与充值流程涉及账号、角色、订单、点券余额和到账记录。钱包登录可能涉及公开地址与认证信息。实际采集字段、用途及必要性仍须与启用的功能逐项核对。" },
      { title: "第三方服务与数据共享", copy: "支付渠道、钱包登录及钱包中的第三方 USDC 买卖可能涉及不同服务商。服务商身份、各自处理的数据、共享范围和处理地区仍待核对，不能仅凭本页认定任何服务已获批准或在所有地区可用。" },
      { title: "保存、跨境处理与用户请求", copy: "各类数据的保存期限、跨境处理安排，以及访问、更正、导出和删除请求的受理方式仍待确认。本草案不承诺已经实现自助导出、自动删除或特定处理时限。" },
      { title: "正式政策待确认事项", copy: "运营主体的法定名称、联系方式、适用地区、实际启用的服务商与数据处理分工，需由运营方确认并按发行地区审查后写入正式政策。" },
    ],
  },
  terms: {
    title: "用户协议",
    summary: "区分 Numeron 游戏测试、点券充值、游戏内月卡和规划中的 AI 订阅，并列出正式服务条款仍待确认的事项。本文尚未完成法律审校。",
    sections: [
      { title: "游戏与测试范围", copy: "Numeron 仍在开发中，已邀请外部玩家参加测试。测试开放内容、数据调整和重置范围应以对应测试公告为准；参与测试不等于全部规划功能已经上线。" },
      { title: "点券、月卡与 AI 订阅", copy: "游戏点券充值、使用点券购买游戏内月卡，以及官网规划中的 AI 订阅是不同事项。充值到账不等于已经购买月卡，也不会自动获得 AI 订阅或推广员资格。具体商品以对应说明和购买确认步骤为准。" },
      { title: "账户安全与公平游戏", copy: "玩家应保护账户凭据，不得使用外挂、利用漏洞、实施欺诈或骚扰，亦不得通过未经授权的自动化方式破坏游戏、交易或排名。请勿向客服或其他玩家提供密码、钱包私钥或助记词。" },
      { title: "正式服务条款待确认事项", copy: "运营主体法定名称与联系方式、服务地区、年龄要求、账号处理和申诉流程、商品使用范围、点券有效期、测试清档与已购余额处理，均需依据实际运营确认。争议处理及适用规则也需按发行地区核对，本草案不替代这些确认。" },
    ],
  },
  refunds: {
    title: "充值与退款说明",
    summary: "本草案适用于单次游戏点券充值，说明充值问题核对与仍待确认的退款、清档规则。它不是 AI 订阅或自动续费政策。",
    sections: [
      { title: "适用商品", copy: "本页针对单次 Numeron 游戏点券充值。游戏内月卡需要独立消费确认，规划中的 AI 订阅另有商品范围；其续费、取消和月度额度规则不能直接用于点券充值。价格及点券数量请查看点券充值说明，当前网页不提供真实付款。" },
      { title: "未到账、重复扣款与付款争议", copy: "遇到充值问题时，可保留订单号、角色、服务器、支付时间及已脱敏的付款凭据，供核对订单与到账记录。不要为查询原订单状态而重复创建或支付新订单，也不要提交密码、私钥或助记词。" },
      { title: "退款条件与处理流程待确认", copy: "可申请退款的条件、未消费和已消费点券如何处理、原支付渠道的退款方式、受理入口和处理时限仍待运营方确认。本草案不承诺自动退款，不设未经确认的一律不退规则，也不把技术测试当作真实退款流程已经验收。" },
      { title: "测试清档与已购点券", copy: "在开放真实收费前，必须明确测试是否清档，以及已购点券、已消费权益和账户异常的处理方式。上述规则尚未确认，不能假定测试余额会永久保留、转入正式服或自动获得退款。" },
      { title: "消费者权利与渠道规则", copy: "正式退款政策需与实际支付渠道协议及销售地区的法律要求一致。适用法律赋予且不得排除的消费者权利，不因本草案而受到限制。" },
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
  "zh-CN": { notice: "以下为中文原文草案，尚未完成法律审校。待确认的商业和数据处理规则不应被视为已经生效。", sourceLink: "查看中文原文草案", rechargeLink: "查看点券充值说明", labels: { privacy: "隐私政策", terms: "用户协议", refunds: "充值与退款", aiData: "AI 数据说明" } },
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
