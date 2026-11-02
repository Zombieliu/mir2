import type { Locale, SiteCopy } from "@/lib/site-copy";
import type { ProductCopy } from "@/lib/product-copy";
import type { DownloadCopy } from "@/lib/download-copy";

// Founder-confirmed facts, 2026-11-02. Internal validation is not an external
// playtest, a product launch, commercial revenue or evidence of retention.
export const currentProduct = {
  started: "2026-05",
  launched: false,
  externalPlaytestStarted: false,
  website: "https://numeron.obelisk.build",
  reviewed: "2026-11-03", internalValidationCompletedAt: null,
} as const;

type FactStrings = readonly [
  state: string, summary: string, world: string, planned: string,
  prototype: string, internal: string, learn: string, business: string,
  relation: string, pending: string,
];

const localized: Record<Locale, FactStrings> = {
  "zh-CN": [
    "开发中 · 外部测试准备中",
    "开发中 · 外部测试准备中",
    "故事围绕灾后世界、探索、生存与重建展开。打宝、玩家交易、三职业协作和公会竞争是设计目标；当前内部画面仍使用 Crystal 过渡素材与名称。",
    "外部开发者扩展、AI 世界导演与自动切片仍在规划中，尚未上线。",
    "界面原型 · 无真实玩家、成交或留存统计",
    "内部验证构建",
    "了解开发进度",
    "仅完成内部充值测试，尚未开始商业运营；测试资产与测试到账不代表真实收入。付费范围和价格尚未对外开放。",
    "Obelisk Labs 开发 Dubhe 基础设施与工具、Dubhe Wallet 钱包，以及开发中的 Numeron 游戏。",
    "规划中",
  ],
  en: [
    "In development · Playtesting coming soon",
    "In development · Playtesting coming soon",
    "Explore, survive and rebuild in a post-disaster world. Loot, player trading, three combat roles and guild competition are design goals. Internal footage still uses temporary Crystal assets and names.",
    "External developer extensions, AI world direction and automated highlights are planned and are not available.",
    "Interface prototype · No real player, trading or retention statistics",
    "Internal validation build",
    "Development status",
    "Only internal recharge testing has been completed. Commercial operation has not begun; test assets and test credits are not revenue. Public pricing and purchases are not open.",
    "Obelisk Labs develops Dubhe infrastructure and tools, the Dubhe Wallet product, and the Numeron game currently in development.",
    "Planned",
  ],
  ja: [
    "開発中 · 外部テスト準備中",
    "開発中 · 外部テスト準備中",
    "災害後の世界で探索し、生き延び、再建する物語です。戦利品、プレイヤー間取引、三つの戦闘役割とギルド競争は設計目標です。内部映像には Crystal の仮素材と名称を使用しています。",
    "外部開発者向け拡張、AIによる世界進行と自動ハイライトは計画段階で、未提供です。",
    "画面プロトタイプ · 実際のプレイヤー数・取引・継続率の統計なし",
    "内部検証ビルド", "開発状況を見る",
    "内部チャージテストのみ実施済みです。商用運営は始まっておらず、テスト資産やテスト残高は収益ではありません。一般向け価格と購入は未公開です。",
    "Obelisk Labs は Dubhe の基盤・ツール、Dubhe Wallet と開発中の Numeron を開発しています。", "計画中",
  ],
  ko: [
    "개발 중 · 외부 테스트 준비 중",
    "개발 중 · 외부 테스트 준비 중",
    "재난 이후 세계에서 탐험하고 생존하며 재건하는 이야기입니다. 전리품, 플레이어 거래, 세 전투 역할과 길드 경쟁은 설계 목표입니다. 내부 화면은 Crystal 임시 소재와 명칭을 사용합니다.",
    "외부 개발자 확장, AI 월드 연출과 자동 하이라이트는 계획 단계이며 아직 제공되지 않습니다.",
    "화면 프로토타입 · 실제 플레이어·거래·유지율 통계 없음",
    "내부 검증 빌드", "개발 현황",
    "내부 충전 테스트만 완료했습니다. 상용 운영은 시작하지 않았고 테스트 자산과 잔액은 매출이 아닙니다. 일반 가격과 구매는 열리지 않았습니다.",
    "Obelisk Labs는 Dubhe 인프라와 도구, Dubhe Wallet, 그리고 개발 중인 Numeron 게임을 개발합니다.", "계획 중",
  ],
  "zh-TW": [
    "開發中 · 外部測試準備中",
    "開發中 · 外部測試準備中",
    "故事圍繞災後世界、探索、生存與重建展開。打寶、玩家交易、三種戰鬥角色協作與公會競爭是設計目標；內部畫面仍使用 Crystal 過渡素材與名稱。",
    "外部開發者擴充、AI 世界導演與自動精華剪輯仍在規劃中，尚未上線。",
    "介面原型 · 無真實玩家、成交或留存統計",
    "內部驗證版本", "了解開發進度",
    "僅完成內部儲值測試，尚未開始商業營運；測試資產與測試餘額不代表收入。付費範圍與價格尚未對外開放。",
    "Obelisk Labs 開發 Dubhe 基礎設施與工具、Dubhe Wallet 錢包，以及開發中的 Numeron 遊戲。", "規劃中",
  ],
  es: [
    "En desarrollo · Pruebas externas próximamente",
    "En desarrollo · Pruebas externas próximamente",
    "Explora, sobrevive y reconstruye en un mundo tras una catástrofe. El botín, el comercio entre jugadores, tres roles de combate y la competición de gremios son objetivos de diseño. Las imágenes internas usan recursos y nombres temporales de Crystal.",
    "Las extensiones para desarrolladores externos, la dirección del mundo con IA y los resúmenes automáticos están previstos y no están disponibles.",
    "Prototipo de interfaz · Sin estadísticas reales de jugadores, comercio o retención",
    "Versión de validación interna", "Estado del desarrollo",
    "Solo se han realizado pruebas internas de recarga. No hay operación comercial; los activos y créditos de prueba no son ingresos. No hay precios ni compras públicas.",
    "Obelisk Labs desarrolla la infraestructura y las herramientas Dubhe, Dubhe Wallet y el juego Numeron, en desarrollo.", "Previsto",
  ],
  "pt-BR": [
    "Em desenvolvimento · Testes externos em preparação",
    "Em desenvolvimento · Testes externos em preparação",
    "Explore, sobreviva e reconstrua em um mundo após uma catástrofe. Saques, comércio entre jogadores, três funções de combate e competição de guildas são objetivos de design. As imagens internas usam recursos e nomes temporários de Crystal.",
    "Extensões para desenvolvedores externos, direção do mundo por IA e destaques automáticos estão planejados e não estão disponíveis.",
    "Protótipo de interface · Sem estatísticas reais de jogadores, comércio ou retenção",
    "Versão de validação interna", "Andamento do desenvolvimento",
    "Somente testes internos de recarga foram concluídos. A operação comercial não começou; ativos e créditos de teste não são receita. Preços e compras públicas não estão abertos.",
    "A Obelisk Labs desenvolve a infraestrutura e as ferramentas Dubhe, a Dubhe Wallet e o jogo Numeron, ainda em desenvolvimento.", "Planejado",
  ],
  de: [
    "In Entwicklung · Externe Spieltests in Vorbereitung",
    "In Entwicklung · Externe Spieltests in Vorbereitung",
    "Erkunden, Überleben und Wiederaufbau prägen die Welt nach einer Katastrophe. Beute, Spielerhandel, drei Kampfrollen und Gildenwettbewerb sind Entwicklungsziele. Interne Aufnahmen verwenden vorläufige Crystal-Grafiken und Namen.",
    "Erweiterungen für externe Entwickler, KI-Weltregie und automatische Highlights sind geplant und noch nicht verfügbar.",
    "Oberflächenprototyp · Keine echten Spieler-, Handels- oder Bindungsstatistiken",
    "Interner Validierungsbuild", "Entwicklungsstand",
    "Nur interne Aufladetests sind abgeschlossen. Der kommerzielle Betrieb hat nicht begonnen; Testguthaben sind kein Umsatz. Öffentliche Preise und Käufe sind nicht freigeschaltet.",
    "Obelisk Labs entwickelt Dubhe-Infrastruktur und Werkzeuge, Dubhe Wallet und das noch in Entwicklung befindliche Numeron-Spiel.", "Geplant",
  ],
  fr: [
    "En développement · Tests externes en préparation",
    "En développement · Tests externes en préparation",
    "Explorez, survivez et reconstruisez dans un monde après une catastrophe. Butin, échanges entre joueurs, trois rôles de combat et compétition de guildes sont des objectifs de conception. Les images internes utilisent des ressources et noms temporaires de Crystal.",
    "Les extensions pour développeurs externes, la direction du monde par IA et les résumés automatiques sont prévus mais indisponibles.",
    "Prototype d’interface · Aucune statistique réelle de joueurs, d’échanges ou de rétention",
    "Version de validation interne", "État du développement",
    "Seuls les tests internes de recharge sont terminés. L’exploitation commerciale n’a pas commencé ; les actifs et crédits de test ne sont pas des revenus. Les prix et achats publics ne sont pas ouverts.",
    "Obelisk Labs développe l’infrastructure et les outils Dubhe, Dubhe Wallet et le jeu Numeron, encore en développement.", "Prévu",
  ],
  ru: [
    "В разработке · Подготовка внешних тестов",
    "В разработке · Подготовка внешних тестов",
    "Исследование, выживание и восстановление мира после катастрофы — основа истории. Добыча, торговля игроков, три боевые роли и соревнование гильдий — цели разработки. Внутренние кадры используют временные материалы и названия Crystal.",
    "Расширения для внешних разработчиков, ИИ-режиссура мира и автоматические видеоподборки запланированы, но недоступны.",
    "Прототип интерфейса · Без реальной статистики игроков, торговли и удержания",
    "Внутренняя проверочная сборка", "Ход разработки",
    "Завершены только внутренние тесты пополнения. Коммерческая работа не началась; тестовые активы и баланс не являются выручкой. Публичные цены и покупки не открыты.",
    "Obelisk Labs разрабатывает инфраструктуру и инструменты Dubhe, Dubhe Wallet и находящуюся в разработке игру Numeron.", "Запланировано",
  ],
  pl: [
    "W rozwoju · Przygotowania do testów zewnętrznych",
    "W rozwoju · Przygotowania do testów zewnętrznych",
    "Eksploracja, przetrwanie i odbudowa świata po katastrofie tworzą założenia historii. Łupy, handel graczy, trzy role bojowe i rywalizacja gildii są celami projektu. Materiały wewnętrzne używają tymczasowych zasobów i nazw Crystal.",
    "Rozszerzenia dla zewnętrznych programistów, reżyseria świata przez AI i automatyczne skróty są planowane, ale niedostępne.",
    "Prototyp interfejsu · Bez rzeczywistych statystyk graczy, handlu i retencji",
    "Wewnętrzna wersja weryfikacyjna", "Postępy prac",
    "Ukończono tylko wewnętrzne testy doładowań. Działalność komercyjna nie ruszyła; aktywa i kredyty testowe nie są przychodem. Publiczne ceny i zakupy nie są dostępne.",
    "Obelisk Labs rozwija infrastrukturę i narzędzia Dubhe, Dubhe Wallet oraz tworzoną grę Numeron.", "Planowane",
  ],
  tr: [
    "Geliştirme aşamasında · Harici testler hazırlanıyor",
    "Geliştirme aşamasında · Harici testler hazırlanıyor",
    "Felaket sonrası bir dünyada keşif, hayatta kalma ve yeniden inşa hikâyenin temelidir. Ganimet, oyuncu ticareti, üç savaş rolü ve lonca rekabeti tasarım hedefleridir. Dahili görüntüler geçici Crystal varlıklarını ve adlarını kullanır.",
    "Harici geliştirici uzantıları, yapay zekâ dünya yönetimi ve otomatik özetler planlanmıştır; henüz kullanılamaz.",
    "Arayüz prototipi · Gerçek oyuncu, ticaret veya devamlılık istatistiği yok",
    "Dahili doğrulama sürümü", "Geliştirme durumu",
    "Yalnızca dahili yükleme testleri tamamlanmıştır. Ticari işletim başlamamıştır; test varlıkları ve bakiyeleri gelir değildir. Genel fiyatlar ve satın alma açık değildir.",
    "Obelisk Labs, Dubhe altyapısını ve araçlarını, Dubhe Wallet’ı ve geliştirilmekte olan Numeron oyununu geliştirir.", "Planlanıyor",
  ],
  th: [
    "กำลังพัฒนา · กำลังเตรียมทดสอบภายนอก",
    "กำลังพัฒนา · กำลังเตรียมทดสอบภายนอก",
    "เรื่องราวเกี่ยวกับการสำรวจ เอาชีวิตรอด และฟื้นฟูโลกหลังภัยพิบัติ การหาของ การค้าระหว่างผู้เล่น บทบาทการต่อสู้สามแบบและการแข่งขันกิลด์เป็นเป้าหมายการออกแบบ ภาพภายในยังใช้สื่อและชื่อชั่วคราวจาก Crystal",
    "ส่วนขยายสำหรับนักพัฒนาภายนอก ผู้กำกับโลก AI และคลิปสรุปอัตโนมัติยังเป็นแผนและยังใช้งานไม่ได้",
    "ต้นแบบหน้าจอ · ไม่มีสถิติผู้เล่น การซื้อขาย หรือการกลับมาเล่นจริง",
    "รุ่นตรวจสอบภายใน", "ความคืบหน้าการพัฒนา",
    "เสร็จเฉพาะการทดสอบเติมเงินภายใน ยังไม่ดำเนินงานเชิงพาณิชย์ สินทรัพย์และเครดิตทดสอบไม่ใช่รายได้ ยังไม่เปิดราคาและการซื้อสำหรับบุคคลทั่วไป",
    "Obelisk Labs พัฒนาโครงสร้างพื้นฐานและเครื่องมือ Dubhe, Dubhe Wallet และเกม Numeron ที่กำลังพัฒนา", "อยู่ในแผน",
  ],
  vi: [
    "Đang phát triển · Đang chuẩn bị thử nghiệm bên ngoài",
    "Đang phát triển · Đang chuẩn bị thử nghiệm bên ngoài",
    "Khám phá, sinh tồn và tái thiết thế giới sau thảm họa là nền tảng câu chuyện. Săn vật phẩm, giao dịch giữa người chơi, ba vai trò chiến đấu và cạnh tranh bang hội là mục tiêu thiết kế. Hình ảnh nội bộ còn dùng tài nguyên và tên tạm từ Crystal.",
    "Tiện ích cho nhà phát triển bên ngoài, điều phối thế giới bằng AI và video điểm nhấn tự động mới ở giai đoạn kế hoạch, chưa dùng được.",
    "Nguyên mẫu giao diện · Không có số liệu người chơi, giao dịch hay giữ chân thực tế",
    "Bản kiểm chứng nội bộ", "Tiến độ phát triển",
    "Chỉ hoàn thành thử nghiệm nạp tiền nội bộ. Chưa vận hành thương mại; tài sản và số dư thử nghiệm không phải doanh thu. Chưa mở giá và mua hàng công khai.",
    "Obelisk Labs phát triển hạ tầng và công cụ Dubhe, Dubhe Wallet và trò chơi Numeron đang được phát triển.", "Dự kiến",
  ],
  id: [
    "Dalam pengembangan · Uji eksternal sedang disiapkan",
    "Dalam pengembangan · Uji eksternal sedang disiapkan",
    "Eksplorasi, bertahan hidup dan membangun kembali dunia pascabencana menjadi dasar cerita. Rampasan, perdagangan pemain, tiga peran tempur dan persaingan guild adalah sasaran desain. Rekaman internal masih memakai aset dan nama sementara Crystal.",
    "Ekstensi pengembang eksternal, pengarahan dunia dengan AI dan sorotan otomatis masih direncanakan dan belum tersedia.",
    "Prototipe antarmuka · Tanpa statistik pemain, perdagangan atau retensi nyata",
    "Build validasi internal", "Status pengembangan",
    "Hanya pengujian isi ulang internal yang selesai. Operasi komersial belum dimulai; aset dan kredit uji bukan pendapatan. Harga dan pembelian publik belum dibuka.",
    "Obelisk Labs mengembangkan infrastruktur dan alat Dubhe, Dubhe Wallet, serta game Numeron yang masih dikembangkan.", "Direncanakan",
  ],
  ar: [
    "قيد التطوير · الاختبارات الخارجية قيد التحضير",
    "قيد التطوير · الاختبارات الخارجية قيد التحضير",
    "تدور القصة حول الاستكشاف والبقاء وإعادة البناء في عالم بعد كارثة. الغنائم وتجارة اللاعبين وثلاثة أدوار قتالية ومنافسة النقابات أهداف تصميم. تستخدم الصور الداخلية مواد وأسماء مؤقتة من Crystal.",
    "امتدادات المطورين الخارجيين وإدارة العالم بالذكاء الاصطناعي والملخصات الآلية مخططة وغير متاحة بعد.",
    "نموذج واجهة · لا توجد إحصاءات فعلية للاعبين أو التداول أو الاحتفاظ",
    "نسخة تحقق داخلية", "حالة التطوير",
    "اكتملت اختبارات الشحن الداخلية فقط. لم يبدأ التشغيل التجاري؛ أصول وأرصدة الاختبار ليست إيرادات. الأسعار والشراء العام غير مفتوحين.",
    "تطوّر Obelisk Labs بنية وأدوات Dubhe ومحفظة Dubhe Wallet ولعبة Numeron التي لا تزال قيد التطوير.", "مخطط",
  ],
};


// Dates and current results are separate: the validation completion date is unknown.
const timeline: Record<Locale, readonly [startedLabel: string, developmentStart: string, currentValidation: string, temporaryScreenshot: string]> = {
  "zh-CN": ["开始开发", "开发启动：2026 年 5 月。", "当前内部验证：游戏与内部充值测试流程已跑通；完成日期尚未确认。", "内部截图 · Crystal 临时素材"],
  en: ["Development started", "Development started: May 2026.", "Current internal validation: game and recharge test flows work; the completion date has not been confirmed.", "Internal capture · Temporary Crystal assets"],
  ja: ["開発開始", "開発開始：2026年5月。", "現在の内部検証：ゲームとチャージのテスト手順は動作確認済みですが、完了日は未確認です。", "内部画面 · Crystalの仮素材"],
  ko: ["개발 시작", "개발 시작: 2026년 5월.", "현재 내부 검증: 게임과 충전 테스트 흐름이 작동하며, 완료 날짜는 확인되지 않았습니다.", "내부 화면 · Crystal 임시 소재"],
  "zh-TW": ["開始開發", "開發啟動：2026 年 5 月。", "目前內部驗證：遊戲與內部儲值測試流程已跑通；完成日期尚未確認。", "內部截圖 · Crystal 臨時素材"],
  es: ["Inicio del desarrollo", "Inicio del desarrollo: mayo de 2026.", "Validación interna actual: funcionan los flujos de juego y recarga de prueba; la fecha de finalización no está confirmada.", "Captura interna · Recursos temporales de Crystal"],
  "pt-BR": ["Início do desenvolvimento", "Início do desenvolvimento: maio de 2026.", "Validação interna atual: os fluxos de jogo e recarga de teste funcionam; a data de conclusão não foi confirmada.", "Captura interna · Recursos temporários de Crystal"],
  de: ["Entwicklungsbeginn", "Entwicklungsbeginn: Mai 2026.", "Aktuelle interne Prüfung: Spiel- und Test-Aufladeabläufe funktionieren; das Abschlussdatum ist nicht bestätigt.", "Interne Aufnahme · Vorläufige Crystal-Assets"],
  fr: ["Début du développement", "Début du développement : mai 2026.", "Validation interne actuelle : les parcours de jeu et de recharge de test fonctionnent ; la date de fin n’est pas confirmée.", "Capture interne · Ressources temporaires Crystal"],
  ru: ["Начало разработки", "Начало разработки: май 2026 года.", "Текущая внутренняя проверка: игровые процессы и тестовое пополнение работают; дата завершения не подтверждена.", "Внутренний снимок · Временные ресурсы Crystal"],
  pl: ["Początek prac", "Początek prac: maj 2026 r.", "Obecna weryfikacja wewnętrzna: działają przebiegi gry i testowego doładowania; data zakończenia nie została potwierdzona.", "Wewnętrzny zrzut · Tymczasowe zasoby Crystal"],
  tr: ["Geliştirme başlangıcı", "Geliştirme başlangıcı: Mayıs 2026.", "Mevcut dahili doğrulama: oyun ve test yükleme akışları çalışıyor; tamamlanma tarihi doğrulanmadı.", "Dahili görüntü · Geçici Crystal varlıkları"],
  th: ["เริ่มพัฒนา", "เริ่มพัฒนา: พฤษภาคม 2026", "การตรวจสอบภายในปัจจุบัน: ขั้นตอนเกมและการเติมเงินทดสอบใช้งานได้ แต่ยังไม่ยืนยันวันที่เสร็จสิ้น", "ภาพภายใน · สื่อ Crystal ชั่วคราว"],
  vi: ["Bắt đầu phát triển", "Bắt đầu phát triển: tháng 5 năm 2026.", "Kiểm chứng nội bộ hiện tại: luồng trò chơi và nạp thử nghiệm hoạt động; ngày hoàn tất chưa được xác nhận.", "Ảnh nội bộ · Tài nguyên Crystal tạm thời"],
  id: ["Mulai dikembangkan", "Mulai dikembangkan: Mei 2026.", "Validasi internal saat ini: alur game dan isi ulang uji berfungsi; tanggal penyelesaiannya belum dikonfirmasi.", "Tangkapan internal · Aset Crystal sementara"],
  ar: ["بدء التطوير", "بدء التطوير: مايو 2026.", "التحقق الداخلي الحالي: تعمل مسارات اللعبة والشحن التجريبي؛ لم يُؤكد تاريخ اكتمالها.", "لقطة داخلية · مواد Crystal مؤقتة"],
};

export function releaseFacts(locale: Locale) {
  const [state, , world, planned, prototype, internal, learn, business, relation, pending] = localized[locale];
  const [startedLabel, developmentStart, currentValidation, temporaryScreenshot] = timeline[locale];
  return { state, world, planned, prototype, internal, learn, business, relation, pending, startedLabel, developmentStart, currentValidation, temporaryScreenshot };
}

export function applySiteFacts(locale: Locale, copy: SiteCopy): SiteCopy {
  const f = releaseFacts(locale);
  return {
    ...copy,
    hero: { ...copy.hero, description: f.state, primary: f.learn, secondary: copy.nav.world, status: f.state },
    metrics: [{ value: "2026.05", label: f.startedLabel }, { value: "PC / WEB", label: f.pending }, { value: "—", label: f.pending }],
    atlas: { ...copy.atlas, title: "NUMERON ATLAS", copy: f.prototype, live: f.prototype, metrics: [] },
    world: { ...copy.world, title: "NUMERON", copy: f.world, cards: copy.world.cards.map((card) => ({ ...card, copy: f.pending })) },
    classes: { ...copy.classes, copy: f.world, items: copy.classes.items.map((item) => ({ ...item, copy: f.internal })) },
    global: { ...copy.global, title: "NUMERON", copy: f.relation },
    chronicles: { ...copy.chronicles, items: copy.chronicles.items.map(item => ({ ...item, copy: f.pending })) },
    watch: { ...copy.watch, eyebrow: "NUMERON // PLANNED", copy: f.planned, live: f.pending, demo: f.prototype, liveTitle: "NUMERON", liveCopy: `${f.prototype} ${f.temporaryScreenshot}`, highlightsCopy: f.planned, clips: copy.watch.clips.map((clip,index) => ({ ...clip, tag: f.pending, duration: "—", title: `${f.pending} ${index+1}`, copy: f.prototype })), channelsCopy: f.planned },
    membership: { ...copy.membership, copy: `${f.planned} ${f.business}`, prototypePrice: f.pending, annualNote: f.pending, recommended: f.pending, plans: copy.membership.plans.map(plan => ({ ...plan, tagline: f.pending, monthlyPrice: "—", annualPrice: "—", features: [f.pending, f.planned] })), usageCopy: f.planned, usage: copy.membership.usage.map(item => ({ ...item, free: "—", token: "—", architect: "—" })), faq: copy.membership.faq.map(item => ({ ...item, answer: f.planned })) },
    finalCta: { ...copy.finalCta, eyebrow: "NUMERON", copy: f.state, button: f.learn },
    footer: { ...copy.footer, note: `${f.relation} ${f.state}`, enter: f.learn },
  };
}

export function applyProductFacts(locale: Locale, copy: ProductCopy): ProductCopy {
  const f = releaseFacts(locale);
  const pages = { ...copy.pages };
  for (const key of ["updates", "roadmap", "community", "press", "status"] as const) {
    const page = copy.pages[key];
    pages[key] = { ...page, copy: key === "press" ? f.relation : f.state,
      cards: page.cards.map((card,index) => ({ ...card, copy: key === "status" && index === 2 ? f.prototype : key === "status" && index === 0 ? card.copy : f.pending, label: card.label ? f.learn : undefined })),
    };
  }
  return {
    ...copy, pages,
    proof: { ...copy.proof, eyebrow: "INTERNAL DEVELOPMENT", title: f.internal, copy: f.currentValidation, verified: f.temporaryScreenshot, action: f.learn, items: copy.proof.items.map((item,index) => ({ ...item, title: /Mir|Bichon|Sabuk|比奇|沙巴克|비천|미르|ミル/iu.test(item.title) ? `${f.internal} ${index+1}` : item.title, copy: f.temporaryScreenshot })) },
    play: { ...copy.play, title: f.learn, copy: `${f.state} ${f.currentValidation} ${f.temporaryScreenshot}`, launch: f.internal, browserCopy: f.state, desktopCopy: f.state, candidate: f.internal, candidateCopy: f.state, requirementItems: copy.play.requirementItems.map(item => /\bMir\b|ミル|미르/iu.test(item) ? f.state : item) },
    membership: { ...copy.membership, founderCopy: `${f.planned} ${f.business}`, examples: copy.membership.examples.map(item => ({ ...item, copy: f.planned })), rollover: f.pending, cancellation: f.pending, refund: f.business },
    watch: { ...copy.watch, archiveCopy: f.currentValidation, still: f.temporaryScreenshot },
    account: { ...copy.account, copy: `${f.prototype} ${f.planned}`, preview: f.prototype },
    updates: copy.updates.map(entry => ({ ...entry, title: !entry.title || /\bMir\b|ミル|미르|Мир/iu.test(entry.title) ? f.learn : entry.title, summary: f.state, sections: entry.sections.map(section => ({ ...section, copy: f.pending })) })),
    common: { ...copy.common, updated: currentProduct.reviewed },
    ui: { ...copy.ui, prototypeData: f.prototype },
  };
}

export function applyDownloadFacts(locale: Locale, copy: DownloadCopy): DownloadCopy {
  const f = releaseFacts(locale);
  return { ...copy, intro: `${f.state} ${f.currentValidation} ${f.temporaryScreenshot}`, live: f.internal, candidate: f.internal, pwa: f.internal, webCopy: f.state, windowsCopy: f.state, androidCopy: f.pending, requirementsCopy: f.internal, truthCopy: f.business };
}
