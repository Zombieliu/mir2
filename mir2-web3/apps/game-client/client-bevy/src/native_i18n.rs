//! Presentation-only localization for a single native client process.
//!
//! The Windows host activates one locale before building its App. This module
//! never changes a protocol field, player text, item identity or server language.
//! Call `tr` only for system-owned display text; form values/chat stay opaque.
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::OnceLock;

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, UiSystems};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    English,
    TraditionalChinese,
    BrazilianPortuguese,
    Russian,
    Hindi,
    Indonesian,
    Vietnamese,
    Thai,
    Arabic,
}

impl Locale {
    pub const COUNT: usize = 9;
    pub const ALL: [Self; Self::COUNT] = [
        Self::TraditionalChinese,
        Self::English,
        Self::BrazilianPortuguese,
        Self::Russian,
        Self::Hindi,
        Self::Indonesian,
        Self::Vietnamese,
        Self::Thai,
        Self::Arabic,
    ];
    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::TraditionalChinese => "zh-TW",
            Self::BrazilianPortuguese => "pt-BR",
            Self::Russian => "ru",
            Self::Hindi => "hi",
            Self::Indonesian => "id",
            Self::Vietnamese => "vi",
            Self::Thai => "th",
            Self::Arabic => "ar",
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::TraditionalChinese => "繁體中文",
            Self::BrazilianPortuguese => "Português (Brasil)",
            Self::Russian => "Русский",
            Self::Hindi => "हिन्दी",
            Self::Indonesian => "Bahasa Indonesia",
            Self::Vietnamese => "Tiếng Việt",
            Self::Thai => "ไทย",
            Self::Arabic => "العربية",
        }
    }
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "en" => Some(Self::English),
            "zh-TW" => Some(Self::TraditionalChinese),
            "pt-BR" => Some(Self::BrazilianPortuguese),
            "ru" => Some(Self::Russian),
            "hi" => Some(Self::Hindi),
            "id" => Some(Self::Indonesian),
            "vi" => Some(Self::Vietnamese),
            "th" => Some(Self::Thai),
            "ar" => Some(Self::Arabic),
            _ => None,
        }
    }
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::English => 0,
            Self::TraditionalChinese => 1,
            Self::BrazilianPortuguese => 2,
            Self::Russian => 3,
            Self::Hindi => 4,
            Self::Indonesian => 5,
            Self::Vietnamese => 6,
            Self::Thai => 7,
            Self::Arabic => 8,
        }
    }

    pub const fn is_rtl(self) -> bool {
        matches!(self, Self::Arabic)
    }

    /// All native families are bundled and pinned by the Windows host.
    pub const fn font_family(self) -> &'static str {
        match self {
            Self::TraditionalChinese => "Noto Sans TC",
            Self::Hindi => "Noto Sans Devanagari",
            Self::Thai => "Noto Sans Thai",
            Self::Arabic => "Noto Sans Arabic",
            _ => "Noto Sans",
        }
    }
}

// Zero preserves the existing renderer-neutral hosts until they opt in. The
// native Windows host always calls activate, including on the very first run.
static ACTIVE: AtomicU8 = AtomicU8::new(0);
static REVISION: AtomicU64 = AtomicU64::new(0);
thread_local! { static SCOPED_LOCALE: Cell<Option<Locale>> = const { Cell::new(None) }; }

pub fn active() -> bool {
    SCOPED_LOCALE.get().is_some() || ACTIVE.load(Ordering::Relaxed) != 0
}
pub fn locale() -> Locale {
    SCOPED_LOCALE
        .get()
        .unwrap_or_else(|| match ACTIVE.load(Ordering::Acquire) {
            2 => Locale::TraditionalChinese,
            3 => Locale::BrazilianPortuguese,
            4 => Locale::Russian,
            5 => Locale::Hindi,
            6 => Locale::Indonesian,
            7 => Locale::Vietnamese,
            8 => Locale::Thai,
            9 => Locale::Arabic,
            _ => Locale::English,
        })
}
pub fn revision() -> u64 {
    REVISION.load(Ordering::Acquire)
}
pub fn activate(value: Locale) {
    let value = value.index() as u8 + 1;
    if ACTIVE.swap(value, Ordering::AcqRel) != value {
        REVISION.fetch_add(1, Ordering::Release);
    }
}

/// Deterministic pure display fixtures; overrides only the calling thread and
/// restores the previous language even when a test unwinds.
pub fn with_locale<R>(value: Locale, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Locale>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_LOCALE.set(self.0);
        }
    }
    let _restore = Restore(SCOPED_LOCALE.replace(Some(value)));
    f()
}

#[derive(Deserialize)]
struct CatalogFile {
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    key: String,
    en: String,
    #[serde(rename = "zh-TW")]
    zh_tw: String,
    #[serde(rename = "pt-BR")]
    pt_br: String,
    #[serde(default)]
    aliases: Vec<String>,
}

/// New authored content supplies all nine locales together, independent of
/// the generated legacy overlays. Missing languages fail catalog validation.
#[derive(Deserialize)]
struct FullLocaleCatalogFile {
    entries: Vec<FullLocaleEntry>,
}
#[derive(Deserialize)]
struct FullLocaleEntry {
    key: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(flatten)]
    translations: HashMap<String, String>,
}
struct Catalog {
    values: Vec<[String; Locale::COUNT]>,
    keys: HashMap<String, usize>,
    exact: HashMap<String, usize>,
    templates: TemplateIndex,
    npc_exact: HashMap<String, usize>,
    npc_templates: TemplateIndex,
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut result = Catalog {
            values: Vec::new(),
            keys: HashMap::new(),
            exact: HashMap::new(),
            templates: TemplateIndex::default(),
            npc_exact: HashMap::new(),
            npc_templates: TemplateIndex::default(),
        };
        // Keyed overlays keep the original three-language source generators
        // unchanged. Missing keys are release validation failures, never an
        // implicit English translation for one of the six added languages.
        let expansion: [HashMap<String, String>; 6] = [
            include_str!("../../../../packages/game-data/data/native-i18n/extra/ru.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/extra/hi.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/extra/id.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/extra/vi.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/extra/th.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/extra/ar.json"),
        ]
        .map(|file| serde_json::from_str(file).expect("validated native locale overlay"));
        // Explicit native translations win over the broader legacy bundle.
        for file in [
            include_str!("../../../../packages/game-data/data/native-i18n/common.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/content.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/shell.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/quest.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/game.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/help.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/npc-menus.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/npc-prose.json"),
        ] {
            let file: CatalogFile = serde_json::from_str(file).expect("validated native catalog");
            for entry in file.entries {
                let index = result.values.len();
                let extra = expansion.each_ref().map(|values| {
                    values
                        .get(&entry.key)
                        .expect("validated native locale key")
                        .clone()
                });
                let [ru, hi, id, vi, th, ar] = extra;
                let values = [entry.en, entry.zh_tw, entry.pt_br, ru, hi, id, vi, th, ar];
                // NPC "Return" means go back; a quest's "Return" describes
                // the turn-in objective. Never let a script alias overwrite
                // the ordinary UI vocabulary.
                let npc = entry.key.starts_with("npc.");
                result.keys.insert(entry.key, index);
                for source in values.iter().cloned().chain(entry.aliases) {
                    if source.is_empty() {
                        continue;
                    }
                    if template_parts(&source)
                        .iter()
                        .any(|part| matches!(part, Part::Slot(_)))
                    {
                        let templates = if npc {
                            &mut result.npc_templates
                        } else {
                            &mut result.templates
                        };
                        templates.insert(&source, index);
                    } else {
                        if npc {
                            result.npc_exact.insert(source, index);
                        } else {
                            result.exact.insert(source, index);
                        }
                    }
                }
                result.values.push(values);
            }
        }
        for authored_source in [
            include_str!("../../../../packages/game-data/data/native-i18n/periodic.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/conquest.json"),
        ] {
            let authored: FullLocaleCatalogFile =
                serde_json::from_str(authored_source).expect("validated authored locale catalog");
            for entry in authored.entries {
                assert_eq!(
                    entry.translations.len(),
                    Locale::COUNT,
                    "periodic entry must contain exactly nine locales"
                );
                let values: [String; Locale::COUNT] =
                    ["en", "zh-TW", "pt-BR", "ru", "hi", "id", "vi", "th", "ar"].map(|code| {
                        entry
                            .translations
                            .get(code)
                            .filter(|text| !text.trim().is_empty())
                            .expect("periodic locale must be present and nonempty")
                            .clone()
                    });
                let index = result.values.len();
                let npc_caption = entry.key.starts_with("periodic.accept.")
                    || entry.key.starts_with("periodic.complete.")
                    || entry.key.starts_with("conquest.");
                assert!(
                    result.keys.insert(entry.key, index).is_none(),
                    "periodic key must not replace existing vocabulary"
                );
                for source in values.iter().cloned().chain(entry.aliases) {
                    if source.is_empty() {
                        continue;
                    }
                    // These complete authored captions contain a known quest title,
                    // not an opaque NPC/player argument captured by a legacy template.
                    if npc_caption {
                        result.npc_exact.insert(source.clone(), index);
                    }
                    if template_parts(&source)
                        .iter()
                        .any(|part| matches!(part, Part::Slot(_)))
                    {
                        result.templates.insert(&source, index);
                        if npc_caption {
                            result.npc_templates.insert(&source, index);
                        }
                    } else {
                        result.exact.insert(source, index);
                    }
                }
                result.values.push(values);
            }
        }
        for (key, translations) in BILLING_TRANSLATIONS {
            let index = result.values.len();
            assert!(result.keys.insert((*key).into(), index).is_none());
            // Stable billing keys never overwrite an ambiguous legacy alias.
            result.exact.insert((*key).into(), index);
            result.values.push(translations.map(str::to_owned));
        }
        for (source, key) in [
            (MONTHLY_CARD_ITEM_NAME, "billing.monthlyCardItemName"),
            (MONTHLY_CARD_ITEM_TOOLTIP, "billing.monthlyCardItemTooltip"),
        ] {
            let index = result.keys[key];
            assert!(result.exact.insert(source.into(), index).is_none());
        }
        result.templates.finish();
        result.npc_templates.finish();
        result
    })
}

// Authored payment vocabulary, in Locale::index order. Kept separate from
// generated aliases such as Credits (the character scene's staff credits).
const MONTHLY_CARD_ITEM_NAME: &str = "MonthlyCard30Days";
const MONTHLY_CARD_ITEM_TOOLTIP: &str = "Adds 30 days of account-wide game access when used. Active access is extended from its current expiry. Offline time counts. Single use.";
const BILLING_TRANSLATIONS: &[(&str, [&str; Locale::COUNT])] = &[
    ("billing.monthlyCardItemName", ["Monthly Card (30 Days)", "30 天月卡", "Cartão mensal (30 dias)", "Месячная карта (30 дней)", "मासिक कार्ड (30 दिन)", "Kartu Bulanan (30 Hari)", "Thẻ tháng (30 ngày)", "บัตรรายเดือน (30 วัน)", "بطاقة شهرية (30 يومًا)"]),
    ("billing.monthlyCardItemTooltip", [MONTHLY_CARD_ITEM_TOOLTIP, "使用後增加 30 天全帳號遊戲使用期限。已有使用權限時，從目前到期時間起延長。離線時間也會計入。僅可使用一次。", "Ao usar, adiciona 30 dias de acesso ao jogo para toda a conta. Se o acesso estiver ativo, o prazo é prorrogado a partir do vencimento atual. O tempo offline conta. Uso único.", "При использовании добавляет 30 дней доступа к игре для всего аккаунта. Активный доступ продлевается от текущей даты окончания. Время вне игры учитывается. Одноразовое использование.", "इस्तेमाल करने पर पूरे खाते के लिए 30 दिन की गेम पहुँच जोड़ता है। सक्रिय पहुँच की अवधि मौजूदा समाप्ति समय से आगे बढ़ती है। ऑफ़लाइन समय भी गिना जाता है। केवल एक बार इस्तेमाल किया जा सकता है।", "Saat digunakan, menambah 30 hari akses game untuk seluruh akun. Akses yang masih aktif diperpanjang dari waktu kedaluwarsa saat ini. Waktu offline tetap dihitung. Sekali pakai.", "Khi sử dụng, thêm 30 ngày truy cập trò chơi cho toàn bộ tài khoản. Quyền truy cập đang còn hạn được gia hạn từ thời điểm hết hạn hiện tại. Thời gian ngoại tuyến vẫn được tính. Chỉ dùng một lần.", "เมื่อใช้จะเพิ่มสิทธิ์เข้าเกม 30 วันสำหรับทั้งบัญชี หากสิทธิ์ยังไม่หมดอายุ จะต่อเวลาจากวันหมดอายุปัจจุบัน เวลาที่ออฟไลน์จะถูกนับด้วย ใช้ได้ครั้งเดียว", "تضيف عند استخدامها 30 يومًا من الوصول إلى اللعبة للحساب بأكمله. يُمدَّد الوصول النشط بدءًا من موعد انتهائه الحالي. يُحتسب الوقت خارج الاتصال. تُستخدم مرة واحدة."]),
    ("billing.updated", ["Account status refreshed.", "已更新帳號狀態。", "Status da conta atualizado.", "Статус аккаунта обновлён.", "खाते की स्थिति अपडेट हो गई।", "Status akun diperbarui.", "Đã cập nhật trạng thái tài khoản.", "อัปเดตสถานะบัญชีแล้ว", "تم تحديث حالة الحساب."]),
    ("billing.title", ["Credits and Monthly Card", "點數與月卡", "Créditos e cartão mensal", "Кредиты и месячная карта", "क्रेडिट और मासिक कार्ड", "Kredit dan Kartu Bulanan", "Điểm và thẻ tháng", "เครดิตและบัตรรายเดือน", "الرصيد والبطاقة الشهرية"]),
    ("billing.open", ["Recharge / buy monthly card", "充值／購買月卡", "Recarregar / comprar cartão", "Пополнить / купить карту", "रिचार्ज / मासिक कार्ड खरीदें", "Isi ulang / beli kartu", "Nạp điểm / mua thẻ tháng", "เติมเครดิต / ซื้อบัตรรายเดือน", "شحن الرصيد / شراء بطاقة"]),
    ("billing.instructions", ["Payment opens in your browser. Return here and check status to confirm Credits. Each monthly card adds 30 days.", "付款會在瀏覽器開啟。返回此處檢查狀態以確認點數入帳。每張月卡增加 30 天。", "O pagamento abre no navegador. Volte e consulte o status para confirmar os créditos. Cada cartão adiciona 30 dias.", "Оплата откроется в браузере. Вернитесь и проверьте зачисление. Каждая карта добавляет 30 дней.", "भुगतान ब्राउज़र में खुलेगा। लौटकर स्थिति जाँचें और क्रेडिट की पुष्टि करें। हर मासिक कार्ड 30 दिन जोड़ता है।", "Pembayaran dibuka di browser. Kembali dan cek status untuk memastikan kredit masuk. Setiap kartu menambah 30 hari.", "Thanh toán mở trong trình duyệt. Quay lại kiểm tra trạng thái để xác nhận điểm. Mỗi thẻ thêm 30 ngày.", "ชำระเงินในเบราว์เซอร์ แล้วกลับมาตรวจสอบเครดิต บัตรแต่ละใบเพิ่มเวลา 30 วัน", "يُفتح الدفع في المتصفح. ارجع وتحقق من الحالة لتأكيد الرصيد. تضيف كل بطاقة 30 يومًا."]),
    ("billing.balance", ["Credits: {0} · pending: {1}", "點數：{0} · 待入帳：{1}", "Créditos: {0} · pendentes: {1}", "Кредиты: {0} · ожидаются: {1}", "क्रेडिट: {0} · लंबित: {1}", "Kredit: {0} · tertunda: {1}", "Điểm: {0} · đang chờ: {1}", "เครดิต: {0} · รอดำเนินการ: {1}", "الرصيد: {0} · قيد الانتظار: {1}"]),
    ("billing.points", ["{0} Credits", "{0} 點數", "{0} créditos", "{0} кредитов", "{0} क्रेडिट", "{0} kredit", "{0} điểm", "{0} เครดิต", "{0} رصيد"]),
    ("billing.optional", ["Monthly access is optional on this server.", "此伺服器目前不要求月卡。", "O cartão mensal é opcional neste servidor.", "На этом сервере карта необязательна.", "इस सर्वर पर मासिक कार्ड वैकल्पिक है।", "Kartu bulanan opsional di server ini.", "Máy chủ này không bắt buộc thẻ tháng.", "เซิร์ฟเวอร์นี้ไม่บังคับใช้บัตรรายเดือน", "البطاقة الشهرية اختيارية في هذا الخادم."]),
    ("billing.active", ["Monthly card active", "月卡使用中", "Cartão mensal ativo", "Месячная карта активна", "मासिक कार्ड सक्रिय है", "Kartu bulanan aktif", "Thẻ tháng đang hoạt động", "บัตรรายเดือนใช้งานอยู่", "البطاقة الشهرية نشطة"]),
    ("billing.expired", ["Monthly card required or expired", "尚未啟用月卡或已到期", "Cartão mensal ausente ou vencido", "Нужна месячная карта, или срок истёк", "मासिक कार्ड आवश्यक है या समाप्त हो गया है", "Kartu bulanan diperlukan atau kedaluwarsa", "Cần thẻ tháng hoặc thẻ đã hết hạn", "ต้องใช้บัตรรายเดือนหรือบัตรหมดอายุแล้ว", "البطاقة الشهرية مطلوبة أو منتهية"]),
    ("billing.expires", ["Expires:", "有效期限：", "Validade:", "Действует до:", "समाप्ति:", "Berlaku hingga:", "Hết hạn:", "หมดอายุ:", "تنتهي في:"]),
    ("billing.unavailable", ["Billing is unavailable on this server.", "此伺服器目前未開放帳單服務。", "Pagamentos indisponíveis neste servidor.", "Платежи на этом сервере недоступны.", "इस सर्वर पर भुगतान सेवा उपलब्ध नहीं है।", "Layanan pembayaran tidak tersedia di server ini.", "Dịch vụ thanh toán chưa khả dụng trên máy chủ này.", "เซิร์ฟเวอร์นี้ยังไม่เปิดบริการชำระเงิน", "خدمة الدفع غير متاحة في هذا الخادم."]),
    ("billing.insufficientCredits", ["Not enough Credits to buy this monthly card.", "點數不足，無法購買這張月卡。", "Créditos insuficientes para comprar este cartão mensal.", "Недостаточно кредитов для покупки этой месячной карты.", "इस मासिक कार्ड को खरीदने के लिए पर्याप्त क्रेडिट नहीं हैं।", "Kredit tidak cukup untuk membeli kartu bulanan ini.", "Không đủ điểm để mua thẻ tháng này.", "เครดิตไม่เพียงพอสำหรับซื้อบัตรรายเดือนนี้", "الرصيد غير كافٍ لشراء هذه البطاقة الشهرية."]),
    ("billing.inventoryFull", ["Your bag is full. Free a slot before buying a monthly card.", "背包已滿。請先騰出一個空位，再購買月卡。", "Sua bolsa está cheia. Libere um espaço antes de comprar um cartão mensal.", "Сумка заполнена. Освободите ячейку перед покупкой месячной карты.", "आपका बैग भरा हुआ है। मासिक कार्ड खरीदने से पहले एक स्थान खाली करें।", "Tas Anda penuh. Kosongkan satu slot sebelum membeli kartu bulanan.", "Túi đã đầy. Hãy chừa một ô trống trước khi mua thẻ tháng.", "กระเป๋าเต็ม กรุณาเว้นช่องว่างหนึ่งช่องก่อนซื้อบัตรรายเดือน", "حقيبتك ممتلئة. أفرغ خانة قبل شراء بطاقة شهرية."]),
    ("billing.monthlyDisabled", ["Monthly card purchases are unavailable on this server.", "此伺服器目前未開放月卡購買。", "A compra de cartões mensais está indisponível neste servidor.", "Покупка месячных карт на этом сервере недоступна.", "इस सर्वर पर मासिक कार्ड खरीदना उपलब्ध नहीं है।", "Pembelian kartu bulanan tidak tersedia di server ini.", "Máy chủ này chưa mở bán thẻ tháng.", "เซิร์ฟเวอร์นี้ยังไม่เปิดให้ซื้อบัตรรายเดือน", "شراء البطاقات الشهرية غير متاح في هذا الخادم."]),
    ("billing.checkoutUnavailable", ["Online recharge is unavailable.", "目前未開放線上充值。", "Recarga online indisponível.", "Онлайн-пополнение недоступно.", "ऑनलाइन रिचार्ज उपलब्ध नहीं है।", "Isi ulang online tidak tersedia.", "Nạp điểm trực tuyến chưa khả dụng.", "ยังไม่เปิดเติมเครดิตออนไลน์", "شحن الرصيد عبر الإنترنت غير متاح."]),
    ("billing.offers", ["Recharge offers", "充值方案", "Opções de recarga", "Варианты пополнения", "रिचार्ज विकल्प", "Pilihan isi ulang", "Gói nạp điểm", "แพ็กเกจเติมเครดิต", "خيارات شحن الرصيد"]),
    ("billing.recharge", ["Recharge", "充值", "Recarregar", "Пополнить", "रिचार्ज", "Isi ulang", "Nạp điểm", "เติมเครดิต", "شحن الرصيد"]),
    ("billing.monthlyPrice", ["30-day monthly card: {0} Credits", "30 天月卡：{0} 點數", "Cartão de 30 dias: {0} créditos", "Карта на 30 дней: {0} кредитов", "30-दिन का मासिक कार्ड: {0} क्रेडिट", "Kartu 30 hari: {0} kredit", "Thẻ 30 ngày: {0} điểm", "บัตร 30 วัน: {0} เครดิต", "بطاقة 30 يومًا: {0} رصيد"]),
    ("billing.buyMonthly", ["Buy monthly card", "購買月卡", "Comprar cartão", "Купить карту", "मासिक कार्ड खरीदें", "Beli kartu bulanan", "Mua thẻ tháng", "ซื้อบัตรรายเดือน", "شراء بطاقة شهرية"]),
    ("billing.owned", ["Your monthly cards", "持有的月卡", "Seus cartões mensais", "Ваши месячные карты", "आपके मासिक कार्ड", "Kartu bulanan Anda", "Thẻ tháng của bạn", "บัตรรายเดือนของคุณ", "بطاقاتك الشهرية"]),
    ("billing.noCards", ["You have no monthly cards.", "目前沒有月卡。", "Você não tem cartões mensais.", "У вас нет месячных карт.", "आपके पास मासिक कार्ड नहीं हैं।", "Anda belum memiliki kartu bulanan.", "Bạn chưa có thẻ tháng.", "คุณยังไม่มีบัตรรายเดือน", "ليس لديك بطاقات شهرية."]),
    ("billing.card", ["Monthly card", "月卡", "Cartão mensal", "Месячная карта", "मासिक कार्ड", "Kartu bulanan", "Thẻ tháng", "บัตรรายเดือน", "بطاقة شهرية"]),
    ("billing.activate", ["Activate", "啟用", "Ativar", "Активировать", "सक्रिय करें", "Aktifkan", "Kích hoạt", "เปิดใช้งาน", "تفعيل"]),
    ("billing.checking", ["Checking status...", "正在查詢狀態…", "Consultando status...", "Проверка статуса…", "स्थिति जाँची जा रही है…", "Memeriksa status…", "Đang kiểm tra trạng thái…", "กำลังตรวจสอบสถานะ…", "جارٍ التحقق من الحالة…"]),
    ("billing.waiting", ["Checkout opened. Payment is not confirmed yet; return and check status.", "已開啟付款頁面，尚未確認付款。返回後請檢查狀態。", "Pagamento aberto. Ainda não confirmado; volte e consulte o status.", "Страница оплаты открыта. Оплата ещё не подтверждена; проверьте статус.", "भुगतान पृष्ठ खुल गया है। भुगतान की पुष्टि नहीं हुई है; लौटकर स्थिति जाँचें।", "Halaman pembayaran dibuka. Pembayaran belum dikonfirmasi; kembali dan cek status.", "Đã mở trang thanh toán. Chưa xác nhận thanh toán; hãy quay lại kiểm tra trạng thái.", "เปิดหน้าชำระเงินแล้ว ยังไม่ยืนยันการชำระเงิน กลับมาตรวจสอบสถานะ", "فُتحت صفحة الدفع. لم يُؤكَّد الدفع بعد؛ ارجع وتحقق من الحالة."]),
    ("billing.unconfirmed", ["Result unconfirmed. Check status or retry the same request.", "結果尚未確認。請檢查狀態，或重試同一筆請求。", "Resultado não confirmado. Consulte o status ou repita a mesma solicitação.", "Результат неизвестен. Проверьте статус или повторите тот же запрос.", "परिणाम की पुष्टि नहीं हुई। स्थिति जाँचें या वही अनुरोध फिर भेजें।", "Hasil belum dikonfirmasi. Cek status atau ulangi permintaan yang sama.", "Chưa xác nhận kết quả. Kiểm tra trạng thái hoặc thử lại cùng yêu cầu.", "ยังไม่ยืนยันผล ตรวจสอบสถานะหรือลองคำขอเดิมอีกครั้ง", "النتيجة غير مؤكدة. تحقق من الحالة أو أعد الطلب نفسه."]),
    ("billing.failed", ["Request could not be completed. Check status before retrying.", "請求未能完成。重試前請先檢查狀態。", "Não foi possível concluir. Consulte o status antes de tentar novamente.", "Не удалось выполнить запрос. Перед повтором проверьте статус.", "अनुरोध पूरा नहीं हुआ। दोबारा भेजने से पहले स्थिति जाँचें।", "Permintaan tidak selesai. Cek status sebelum mencoba lagi.", "Không thể hoàn tất yêu cầu. Kiểm tra trạng thái trước khi thử lại.", "ทำคำขอไม่สำเร็จ ตรวจสอบสถานะก่อนลองอีกครั้ง", "تعذر إتمام الطلب. تحقق من الحالة قبل إعادة المحاولة."]),
    ("billing.browserFailed", ["Could not open the payment browser. Retry the same request.", "無法開啟付款瀏覽器。請重試同一筆請求。", "Não foi possível abrir o navegador. Repita a mesma solicitação.", "Не удалось открыть браузер оплаты. Повторите тот же запрос.", "भुगतान ब्राउज़र नहीं खुल सका। वही अनुरोध फिर भेजें।", "Browser pembayaran tidak dapat dibuka. Ulangi permintaan yang sama.", "Không thể mở trình duyệt thanh toán. Thử lại cùng yêu cầu.", "เปิดเบราว์เซอร์ชำระเงินไม่ได้ ลองคำขอเดิมอีกครั้ง", "تعذر فتح متصفح الدفع. أعد الطلب نفسه."]),
    ("billing.done", ["Request confirmed. Refreshing account status...", "請求已確認，正在更新帳號狀態…", "Solicitação confirmada. Atualizando a conta...", "Запрос подтверждён. Обновление статуса…", "अनुरोध की पुष्टि हुई। खाते की स्थिति अपडेट हो रही है…", "Permintaan dikonfirmasi. Memperbarui status akun…", "Đã xác nhận yêu cầu. Đang cập nhật trạng thái tài khoản…", "ยืนยันคำขอแล้ว กำลังอัปเดตสถานะบัญชี…", "تم تأكيد الطلب. جارٍ تحديث حالة الحساب…"]),
    ("billing.refresh", ["Check status", "檢查狀態", "Consultar status", "Проверить статус", "स्थिति जाँचें", "Cek status", "Kiểm tra trạng thái", "ตรวจสอบสถานะ", "التحقق من الحالة"]),
    ("billing.retry", ["Retry same request", "重試同一筆請求", "Repetir solicitação", "Повторить запрос", "वही अनुरोध फिर भेजें", "Ulangi permintaan", "Thử lại cùng yêu cầu", "ลองคำขอเดิมอีกครั้ง", "إعادة الطلب نفسه"]),
    ("billing.close", ["Close", "關閉", "Fechar", "Закрыть", "बंद करें", "Tutup", "Đóng", "ปิด", "إغلاق"]),
    ("billing.previous", ["Previous", "上一頁", "Anterior", "Назад", "पिछला", "Sebelumnya", "Trước", "ก่อนหน้า", "السابق"]),
    ("billing.next", ["Next", "下一頁", "Próximo", "Далее", "अगला", "Berikutnya", "Sau", "ถัดไป", "التالي"]),
    ("billing.selectCharacter", ["Select a character to open billing.", "請先選擇角色，再開啟帳單。", "Selecione um personagem para abrir pagamentos.", "Выберите персонажа для оплаты.", "भुगतान खोलने के लिए पात्र चुनें।", "Pilih karakter untuk membuka pembayaran.", "Chọn nhân vật để mở thanh toán.", "เลือกตัวละครเพื่อเปิดบริการชำระเงิน", "اختر شخصية لفتح خدمة الدفع."]),
];

#[derive(Debug)]
enum Part<'a> {
    Literal(&'a str),
    Slot(&'a str),
}

#[derive(Debug)]
enum CompiledPart {
    Literal(String),
    Slot(String),
}

#[derive(Debug)]
struct CompiledTemplate {
    pattern: String,
    parts: Vec<CompiledPart>,
    prefix: String,
    suffix: String,
    literal_bytes: usize,
    entry: usize,
    trim_output: bool,
}

impl CompiledTemplate {
    fn new(pattern: &str, entry: usize, trim_output: bool) -> Option<Self> {
        let parts = template_parts(pattern);
        let literal_bytes: usize = parts
            .iter()
            .filter_map(|part| match part {
                Part::Literal(value) => Some(value.len()),
                Part::Slot(_) => None,
            })
            .sum();
        // Bare captures or only separators would treat arbitrary player text as
        // system text (for example a translated "{owner} {pet}" pattern).
        let has_words = parts.iter().any(|part| match part {
            Part::Literal(value) => value.chars().any(char::is_alphabetic),
            Part::Slot(_) => false,
        });
        if !has_words || !parts.iter().any(|part| matches!(part, Part::Slot(_))) {
            return None;
        }
        let prefix = match parts.first() {
            Some(Part::Literal(value)) => (*value).to_owned(),
            _ => String::new(),
        };
        let suffix = match parts.last() {
            Some(Part::Literal(value)) => (*value).to_owned(),
            _ => String::new(),
        };
        Some(Self {
            pattern: pattern.to_owned(),
            parts: parts
                .into_iter()
                .map(|part| match part {
                    Part::Literal(value) => CompiledPart::Literal(value.to_owned()),
                    Part::Slot(name) => CompiledPart::Slot(slot_name(name).to_owned()),
                })
                .collect(),
            prefix,
            suffix,
            literal_bytes,
            entry,
            trim_output,
        })
    }

    fn captures<'pattern, 'source>(
        &'pattern self,
        source: &'source str,
    ) -> Option<Vec<(&'pattern str, &'source str)>> {
        if !source.starts_with(self.prefix.as_str())
            || !source.ends_with(self.suffix.as_str())
            || source.len() < self.prefix.len() + self.suffix.len()
        {
            return None;
        }
        let mut rest = source;
        let mut result = Vec::<(&str, &str)>::new();
        for (index, part) in self.parts.iter().enumerate() {
            match part {
                CompiledPart::Literal(literal) => rest = rest.strip_prefix(literal.as_str())?,
                CompiledPart::Slot(name) => {
                    let following = match self.parts.get(index + 1) {
                        Some(CompiledPart::Literal(value)) => value.as_str(),
                        _ => "",
                    };
                    // {sign}{value} cannot be recovered from a flattened string
                    // without guessing. Those renderers use explicit keyed args.
                    if following.is_empty()
                        && matches!(self.parts.get(index + 2), Some(CompiledPart::Slot(_)))
                    {
                        return None;
                    }
                    let end = if following.is_empty() {
                        rest.len()
                    } else {
                        rest.find(following)?
                    };
                    let value = &rest[..end];
                    if let Some((_, old)) = result.iter().find(|(old, _)| *old == name.as_str()) {
                        if *old != value {
                            return None;
                        }
                    }
                    result.push((name.as_str(), value));
                    rest = &rest[end..];
                }
            }
        }
        rest.is_empty().then_some(result)
    }
}

#[derive(Default)]
struct TemplateIndex {
    templates: Vec<CompiledTemplate>,
    prefixes: HashMap<String, Vec<usize>>,
    without_prefix: Vec<usize>,
}

impl TemplateIndex {
    const PREFIX_CHARACTERS: usize = 8;

    fn insert(&mut self, source: &str, entry: usize) {
        if let Some(template) = CompiledTemplate::new(source, entry, false) {
            self.templates.push(template);
        }
        // Source imports sometimes include layout padding absent from the
        // native label. Normalize only catalogue edges, never captured values.
        if source.trim() != source {
            if let Some(template) = CompiledTemplate::new(source.trim(), entry, true) {
                self.templates.push(template);
            }
        }
    }

    fn finish(&mut self) {
        self.templates.sort_by_key(|template| {
            (
                std::cmp::Reverse(template.literal_bytes),
                std::cmp::Reverse(template.entry),
                template.trim_output,
            )
        });
        // Repeated en/alias strings need only one compiled candidate. The
        // sorted first occurrence is the more specific, later-domain entry.
        let mut seen = HashSet::new();
        self.templates
            .retain(|template| seen.insert(template.pattern.clone()));
        self.prefixes.clear();
        self.without_prefix.clear();
        for (index, template) in self.templates.iter().enumerate() {
            if template.prefix.is_empty() {
                self.without_prefix.push(index);
            } else {
                let prefix = template
                    .prefix
                    .chars()
                    .take(Self::PREFIX_CHARACTERS)
                    .collect::<String>();
                self.prefixes.entry(prefix).or_default().push(index);
            }
        }
    }

    fn candidates(&self, source: &str) -> Vec<usize> {
        let mut candidates = self.without_prefix.clone();
        let mut end = 0;
        for character in source.chars().take(Self::PREFIX_CHARACTERS) {
            end += character.len_utf8();
            if let Some(indices) = self.prefixes.get(&source[..end]) {
                candidates.extend_from_slice(indices);
            }
        }
        // Bucket membership only narrows lookup. Global specificity/domain
        // priority must be identical even when candidates came from 8 buckets.
        candidates.sort_unstable();
        candidates
    }
}
fn slot_name(name: &str) -> &str {
    name.split_once(':').map_or(name, |(name, _)| name)
}

fn is_slot(name: &str) -> bool {
    if let Some((number, format)) = name.split_once(':') {
        return !number.is_empty()
            && number.bytes().all(|b| b.is_ascii_digit())
            && !format.is_empty()
            && format
                .bytes()
                .all(|b| b.is_ascii_digit() || b"#,.*%+-; PpFfNnDdXxGgEeCcRr".contains(&b));
    }
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn template_parts(text: &str) -> Vec<Part<'_>> {
    let mut parts = Vec::new();
    let mut literal_start = 0;
    let mut scan = 0;
    while let Some(start) = text[scan..].find('{').map(|n| n + scan) {
        let Some(end) = text[start + 1..].find('}').map(|end| start + 1 + end) else {
            break;
        };
        let name = &text[start + 1..end];
        if !is_slot(name) {
            // Crystal colour markup is literal source syntax, not a parameter.
            // Keep scanning so {Gold/Gold}: {npc_arg0} still has one capture.
            scan = start + 1;
            continue;
        }
        parts.push(Part::Literal(&text[literal_start..start]));
        parts.push(Part::Slot(name));
        literal_start = end + 1;
        scan = literal_start;
    }
    parts.push(Part::Literal(&text[literal_start..]));
    parts
}

#[cfg(test)]
fn captures<'a>(pattern: &str, source: &'a str) -> Option<Vec<(String, &'a str)>> {
    let compiled = CompiledTemplate::new(pattern, 0, false)?;
    Some(
        compiled
            .captures(source)?
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    )
}

fn substitute(template: &str, args: &[(&str, &str)]) -> String {
    let mut text = String::with_capacity(template.len());
    for part in template_parts(template) {
        match part {
            Part::Literal(value) => text.push_str(value),
            Part::Slot(name) => {
                if let Some((_, value)) = args.iter().find(|(key, _)| *key == slot_name(name)) {
                    // One pass: braces/keywords in names and user parameters are data.
                    text.push_str(value);
                } else {
                    text.push('{');
                    text.push_str(name);
                    text.push('}');
                }
            }
        }
    }
    text
}

pub fn translate(language: Locale, source: &str) -> String {
    translate_context(language, source, false)
}

fn translate_context(language: Locale, source: &str, npc: bool) -> String {
    let c = catalog();
    let (exact, templates) = if npc {
        (&c.npc_exact, &c.npc_templates)
    } else {
        (&c.exact, &c.templates)
    };
    if let Some(index) = exact.get(source) {
        return c.values[*index][language.index()].clone();
    }
    // Preserve whitespace around system fragments; never normalize typed input.
    let trimmed = source.trim();
    if trimmed != source && !trimmed.is_empty() {
        let lead = source.len() - source.trim_start().len();
        return format!(
            "{}{}{}",
            &source[..lead],
            translate_context(language, trimmed, npc),
            &source[lead + trimmed.len()..]
        );
    }
    for index in templates.candidates(source) {
        let template = &templates.templates[index];
        if let Some(args) = template.captures(source) {
            let value = &c.values[template.entry][language.index()];
            return substitute(
                if template.trim_output {
                    value.trim()
                } else {
                    value
                },
                &args,
            );
        }
    }
    if source.contains('\n') {
        return source
            .split('\n')
            .map(|line| translate_context(language, line, npc))
            .collect::<Vec<_>>()
            .join("\n");
    }
    if npc {
        translate(language, source)
    } else {
        source.to_owned()
    }
}

/// A single parsed NPC SAY line/menu caption, before wrapping. Link targets
/// and interpolated player data never enter the translation lookup separately.
pub fn npc_text(source: &str) -> String {
    if active() {
        translate_context(locale(), source, true)
    } else {
        crate::player_text::text(source)
    }
}
pub fn tr(source: &str) -> String {
    if active() {
        translate(locale(), source)
    } else {
        source.to_owned()
    }
}
pub fn key(id: &str, fallback: &str) -> String {
    if active() {
        for_locale(locale(), id, fallback)
    } else {
        fallback.to_owned()
    }
}
pub fn for_locale(language: Locale, id: &str, fallback: &str) -> String {
    catalog()
        .keys
        .get(id)
        .map(|index| catalog().values[*index][language.index()].clone())
        .unwrap_or_else(|| translate(language, fallback))
}
pub fn format_key(id: &str, fallback: &str, args: &[(&str, &str)]) -> String {
    substitute(&key(id, fallback), args)
}
pub fn format_for(language: Locale, id: &str, fallback: &str, args: &[(&str, &str)]) -> String {
    substitute(&for_locale(language, id, fallback), args)
}

/// Opt-in marker for system text. Never attach to editable fields, player names,
/// chat, guild notices or mail bodies.
#[derive(Component, Clone)]
pub struct LocalizedText(pub String);
impl LocalizedText {
    pub fn new(source: impl Into<String>) -> Self {
        Self(source.into())
    }
}

#[derive(Component, Clone, Copy)]
pub struct LocaleChoice(pub Locale);
#[derive(Component)]
struct LocaleSelector;
#[derive(Component)]
struct KeepLanguageFont;
#[derive(Component)]
struct LocaleMenuToggle;
#[derive(Component)]
struct LocaleMenuPanel;
#[derive(Component)]
struct LocaleCurrentLabel;
#[derive(Resource, Default)]
struct LocaleMenuState {
    open: bool,
}

#[cfg(test)]
pub(crate) fn set_language_popup_for_tests(world: &mut World, open: bool) {
    world.resource_mut::<LocaleMenuState>().open = open;
}

pub struct NativeI18nPlugin;
impl Plugin for NativeI18nPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LocaleMenuState>()
            .add_systems(Startup, spawn_language_selector)
            .add_systems(
                Update,
                (
                    toggle_language_menu,
                    choose_language,
                    show_language_selector,
                )
                    .chain()
                    .before(crate::crystal_ui::overlays::NativePlayerUiSet::Mutate),
            )
            .add_systems(
                PostUpdate,
                (
                    show_language_menu,
                    refresh_localized_text,
                    refresh_native_fonts,
                    refresh_native_alignment,
                )
                    .chain()
                    .before(UiSystems::Content),
            );
    }
}

fn refresh_native_alignment(mut layouts: Query<&mut TextLayout>) {
    if !active() {
        return;
    }
    for mut layout in &mut layouts {
        // Parley resolves paragraph bidi and shapes connected scripts. Start
        // follows the text direction while centered/right-aligned counters
        // retain their explicit geometry. Never reverse text or map positions.
        if layout.justify == Justify::Left {
            layout.justify = Justify::Start;
        }
    }
}

fn refresh_native_fonts(
    mut fonts: Query<(&mut TextFont, Option<&KeepLanguageFont>)>,
    mut seen: Local<u64>,
) {
    if !active() {
        return;
    }
    let changed = *seen != revision();
    for (mut font, keep) in &mut fonts {
        if keep.is_some() {
            continue;
        }
        if changed || font.is_added() {
            // Font selection also covers opaque player text without translating
            // it. Stable family names reuse the host's pinned font identities.
            font.font = crate::crystal_ui::typography::crystal_text_font(12.0).font;
        }
    }
    *seen = revision();
}

fn refresh_localized_text(
    mut texts: Query<(&LocalizedText, &mut Text, Option<&mut TextFont>)>,
    mut seen: Local<u64>,
) {
    let changed = *seen != revision();
    for (source, mut text, font) in &mut texts {
        if changed || text.is_added() {
            **text = tr(&source.0);
            if let Some(mut font) = font {
                font.font = crate::crystal_ui::typography::crystal_text_font(12.0).font;
            }
        }
    }
    *seen = revision();
}

pub fn spawn_language_choices(parent: &mut ChildSpawnerCommands, font_size: f32) {
    parent
        .spawn((Node {
            width: percent(100.0),
            min_width: px(220.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },))
        .with_children(|root| {
            root.spawn((
                Button,
                LocaleMenuToggle,
                Node {
                    min_height: px(29.0),
                    padding: UiRect::all(px(5.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1.0)),
                    ..default()
                },
                BorderColor::all(Color::srgb(0.65, 0.50, 0.23)),
                BackgroundColor(Color::srgb(0.10, 0.08, 0.04)),
                FocusPolicy::Block,
            ))
            .with_children(|button| {
                button.spawn((
                    LocaleCurrentLabel,
                    Text::new(format!("{}  ^", locale().label())),
                    crate::crystal_ui::typography::crystal_text_font(font_size),
                    TextColor(Color::srgb(0.95, 0.91, 0.77)),
                ));
            });
            root.spawn((
                LocaleMenuPanel,
                Node {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    bottom: px(33.0),
                    left: px(0.0),
                    width: percent(100.0),
                    padding: UiRect::all(px(4.0)),
                    row_gap: px(2.0),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(px(1.0)),
                    ..default()
                },
                GlobalZIndex(25001),
                BorderColor::all(Color::srgb(0.65, 0.50, 0.23)),
                BackgroundColor(Color::srgb(0.055, 0.045, 0.03)),
                FocusPolicy::Block,
            ))
            .with_children(|choices| spawn_language_choice_buttons(choices, font_size));
        });
}

fn spawn_language_choice_buttons(parent: &mut ChildSpawnerCommands, font_size: f32) {
    for language in Locale::ALL {
        parent
            .spawn((
                Button,
                LocaleChoice(language),
                Node {
                    min_height: px(25.0),
                    padding: UiRect::axes(px(7.0), px(4.0)),
                    border: UiRect::all(px(1.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BorderColor::all(Color::srgb(0.65, 0.50, 0.23)),
                BackgroundColor(Color::srgb(0.10, 0.08, 0.04)),
                FocusPolicy::Block,
            ))
            .with_children(|button| {
                button.spawn((
                    KeepLanguageFont,
                    Text::new(language.label()),
                    TextFont {
                        font: FontSource::Family(language.font_family().into()),
                        font_size: FontSize::Px(font_size),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.91, 0.77)),
                ));
            });
    }
}
fn spawn_language_selector(mut commands: Commands) {
    commands
        .spawn((
            LocaleSelector,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(12.0),
                right: px(12.0),
                width: px(240.0),
                justify_content: JustifyContent::Center,
                column_gap: px(6.0),
                ..default()
            },
            GlobalZIndex(25000),
            FocusPolicy::Pass,
        ))
        .with_children(|root| spawn_language_choices(root, 13.0));
}
fn show_language_selector(
    shell: Option<Res<crate::native_shell::NativeShellModel>>,
    mut nodes: Query<&mut Node, With<LocaleSelector>>,
) {
    let in_game = shell.is_some_and(|s| s.screen == crate::native_shell::NativeShellScreen::InGame);
    for mut node in &mut nodes {
        node.display = if in_game {
            Display::None
        } else {
            Display::Flex
        };
    }
}
fn choose_language(
    mut buttons: Query<(&Interaction, &LocaleChoice, &mut BackgroundColor), Changed<Interaction>>,
    mut menu: ResMut<LocaleMenuState>,
) {
    for (interaction, choice, mut bg) in &mut buttons {
        if *interaction == Interaction::Pressed {
            activate(choice.0);
            menu.open = false;
        }
        bg.0 = if *interaction == Interaction::Hovered {
            Color::srgb(0.28, 0.21, 0.08)
        } else {
            Color::srgb(0.10, 0.08, 0.04)
        };
    }
}

fn toggle_language_menu(
    toggles: Query<&Interaction, (With<LocaleMenuToggle>, Changed<Interaction>)>,
    hovered: Query<&Interaction, Or<(With<LocaleMenuToggle>, With<LocaleChoice>)>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut menu: ResMut<LocaleMenuState>,
) {
    if keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape))
        || (mouse.is_some_and(|mouse| mouse.just_pressed(MouseButton::Left))
            && !hovered
                .iter()
                .any(|interaction| *interaction != Interaction::None))
    {
        menu.open = false;
    }
    for interaction in &toggles {
        if *interaction == Interaction::Pressed {
            menu.open = !menu.open;
        }
    }
}

fn show_language_menu(
    menu: Res<LocaleMenuState>,
    mut nodes: Query<(&mut Node, Ref<LocaleMenuPanel>)>,
    mut labels: Query<&mut Text, With<LocaleCurrentLabel>>,
    mut seen: Local<u64>,
) {
    // The Options panel rebuilds its children in Update. Restore the newly
    // spawned menu after those commands, even when its open state is unchanged.
    for (mut node, panel) in &mut nodes {
        if menu.is_changed() || panel.is_added() {
            node.display = if menu.open {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
    let changed = *seen != revision();
    for mut label in &mut labels {
        if changed || label.is_added() {
            **label = format!("{}  ^", locale().label());
        }
    }
    *seen = revision();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monthly_card_exact_system_name_and_tooltip_use_ordinary_item_translation() {
        assert_eq!(MONTHLY_CARD_ITEM_NAME, "MonthlyCard30Days");
        assert_eq!(MONTHLY_CARD_ITEM_TOOLTIP, "Adds 30 days of account-wide game access when used. Active access is extended from its current expiry. Offline time counts. Single use.");
        for language in Locale::ALL {
            with_locale(language, || {
                let name = crate::player_text::name(MONTHLY_CARD_ITEM_NAME);
                let tooltip = crate::player_text::text(MONTHLY_CARD_ITEM_TOOLTIP);
                assert_eq!(name, translate(language, "billing.monthlyCardItemName"));
                assert_eq!(tooltip, translate(language, "billing.monthlyCardItemTooltip"));
                assert!(name.contains("30"));
                assert!(tooltip.contains("30"));
                assert_ne!(name, MONTHLY_CARD_ITEM_NAME);
                if language != Locale::English {
                    assert_ne!(tooltip, MONTHLY_CARD_ITEM_TOOLTIP);
                }
                for opaque in ["monthlyCard30Days", "MonthlyCard30DaysFromPlayer", "MonthlyCard60Days"] {
                    assert_eq!(crate::player_text::name(opaque), opaque);
                }
            });
        }
        assert_eq!(translate(Locale::English, MONTHLY_CARD_ITEM_NAME), "Monthly Card (30 Days)");
        assert_eq!(translate(Locale::English, MONTHLY_CARD_ITEM_TOOLTIP), MONTHLY_CARD_ITEM_TOOLTIP);
    }

    #[test]
    fn billing_keys_cover_nine_locales_and_preserve_placeholders() {
        let mut keys = HashSet::new();
        for (key, values) in BILLING_TRANSLATIONS {
            assert!(key.starts_with("billing."));
            assert!(keys.insert(*key));
            for language in Locale::ALL {
                let value = &values[language.index()];
                assert!(!value.is_empty());
                assert_ne!(value, key);
                assert_eq!(translate(language, key), *value);
                for placeholder in ["{0}", "{1}"] {
                    assert_eq!(
                        values[0].matches(placeholder).count(),
                        value.matches(placeholder).count(),
                        "{key} {language:?}"
                    );
                }
            }
        }
        assert_eq!(translate(Locale::TraditionalChinese, "Credits"), "製作名單");
    }

    #[test]
    fn language_popup_survives_options_children_rebuilt_each_update() {
        fn rebuild_options(mut commands: Commands, panels: Query<Entity, With<LocaleMenuPanel>>) {
            for entity in &panels {
                commands.entity(entity).despawn();
            }
            commands.spawn((
                LocaleMenuPanel,
                Node {
                    display: Display::None,
                    ..default()
                },
            ));
        }
        let mut app = App::new();
        app.insert_resource(LocaleMenuState { open: true })
            .add_systems(Update, rebuild_options)
            .add_systems(PostUpdate, show_language_menu);
        for _ in 0..4 {
            app.update();
            let world = app.world_mut();
            assert_eq!(
                world
                    .query_filtered::<&Node, With<LocaleMenuPanel>>()
                    .single(world)
                    .unwrap()
                    .display,
                Display::Flex
            );
        }
        app.world_mut().resource_mut::<LocaleMenuState>().open = false;
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<&Node, With<LocaleMenuPanel>>()
                .single(world)
                .unwrap()
                .display,
            Display::None
        );
    }

    #[test]
    fn catalogue_padding_is_normalized_without_removing_display_whitespace() {
        for language in Locale::ALL {
            for (key, source) in [
                ("client.DropPercent", "Drop + 12%"),
                ("client.BagWeightPercent", "BagWeight + 12%"),
                ("client.ExpPercent", "Exp + 12%"),
            ] {
                let expected = format_for(language, key, "", &[("0", "12")]);
                assert_eq!(translate(language, source), expected.trim());
                assert_eq!(
                    translate(language, &format!("  {source}  ")),
                    format!("  {}  ", expected.trim())
                );
            }
        }
    }

    #[test]
    fn adjacent_capture_boundaries_are_explicit_and_values_remain_opaque() {
        assert!(captures("{owner} {pet}", "a1 says hello").is_none());
        assert!(captures("{x}/{y}", "player/name").is_none());
        for (pattern, source) in [
            ("Exp Rate: {0}{1}%", "Exp Rate: +10%"),
            (
                "Can pickup items ({0}{1}).",
                "Can pickup items (3x3 auto, 3x3 mouse).",
            ),
            ("A.Speed: {0}{1}", "A.Speed: +2"),
        ] {
            assert!(captures(pattern, source).is_none());
        }
        for language in Locale::ALL {
            let text = format_for(
                language,
                "client.CanPickupItems",
                "",
                &[("0", "Gold {1}"), ("1", " {0} 3x3")],
            );
            assert!(text.contains("Gold {1} {0} 3x3"));
        }
    }

    #[test]
    fn template_index_preserves_specificity_later_domain_and_unicode_prefixes() {
        let mut index = TemplateIndex::default();
        index.insert("Count {n}", 0);
        index.insert("Count exact {n}", 1);
        index.insert("Count {n}", 2);
        index.insert("金幣：{n}", 3);
        index.finish();
        let selected = |source: &str| {
            index.candidates(source).into_iter().find_map(|candidate| {
                let template = &index.templates[candidate];
                template.captures(source).map(|_| template.entry)
            })
        };
        assert_eq!(selected("Count 7"), Some(2));
        assert_eq!(selected("Count exact 7"), Some(1));
        assert_eq!(selected("金幣：1,234"), Some(3));
        assert_eq!(index.templates.len(), 3, "duplicate aliases compile once");
    }

    #[test]
    fn indexed_lookup_matches_full_scan_for_counters_npc_markup_and_numeric_formats() {
        let c = catalog();
        let ban = "This account is banned.\n\nReason: Gold {opaque}\nExpiryDate: 2026-12-01\nDuration: 1,234 Hours, 5 Minutes, 6 Seconds";
        for (npc, source) in [
            (false, "Counter 1234 / 5678"),
            (false, "[337, 268]"),
            (false, "HP 100/162"),
            (false, "Gold: 1,234"),
            (false, ban),
            (
                true,
                "Required {Gold/Gold}: Gold {opaque} Last's for 15 Minutes.",
            ),
        ] {
            let index = if npc { &c.npc_templates } else { &c.templates };
            for language in Locale::ALL {
                let render = |template: &CompiledTemplate| {
                    template.captures(source).map(|args| {
                        let text = &c.values[template.entry][language.index()];
                        (
                            template.entry,
                            substitute(
                                if template.trim_output {
                                    text.trim()
                                } else {
                                    text
                                },
                                &args,
                            ),
                        )
                    })
                };
                let exhaustive = index.templates.iter().find_map(render);
                if source == "Gold: 1,234" || source == ban || npc {
                    assert!(
                        exhaustive.is_some(),
                        "known fixture must match a catalogue template"
                    );
                }
                let indexed = index
                    .candidates(source)
                    .into_iter()
                    .find_map(|candidate| render(&index.templates[candidate]));
                assert_eq!(indexed, exhaustive, "indexed order for {source:?}");
            }
        }
        for language in Locale::ALL {
            let source = "Required {Gold/Gold}: Gold {opaque} Last's for 15 Minutes.";
            assert_eq!(
                translate_context(language, source, true),
                format_for(
                    language,
                    "npc.prose.39f328c1f463ddae",
                    "",
                    &[("npc_arg0", "Gold {opaque}"), ("npc_arg1", "15")]
                ),
            );
            assert!(translate(language, ban).contains("Gold {opaque}"));
            assert!(translate(language, ban).contains("1,234"));
        }
    }

    #[test]
    fn unknown_counter_lookup_has_a_bounded_candidate_set() {
        let index = &catalog().templates;
        for source in ["Counter 1234 / 5678", "[337, 268]", "HP 100/162"] {
            let candidates = index.candidates(source);
            assert!(candidates.len() <= index.without_prefix.len() + 16);
            assert!(candidates.len() * 4 < index.templates.len());
            eprintln!(
                "native-i18n candidates={}/{} prefixless={} source={source:?}",
                candidates.len(),
                index.templates.len(),
                index.without_prefix.len()
            );
        }
        // This is a structural lookup bound, not an absolute timing threshold.
        let index = &catalog().npc_templates;
        let candidates = index.candidates("Required {Gold/Gold}: 1,234 Last's for 15 Minutes.");
        assert!(candidates.len() <= index.without_prefix.len() + 8);
        assert!(candidates.len() * 4 < index.templates.len());
        eprintln!(
            "native-i18n NPC candidates={}/{} prefixless={}",
            candidates.len(),
            index.templates.len(),
            index.without_prefix.len()
        );
    }

    #[test]
    fn numeric_formats_and_npc_colour_markup_preserve_opaque_values() {
        let args = captures("Gold: {0:#,##0}", "Gold: 1,234").unwrap();
        assert_eq!(args, vec![("0".into(), "1,234")]);
        assert_eq!(
            substitute("金幣：{0:#,##0}", &[("0", "1,234")]),
            "金幣：1,234"
        );
        let args = captures(
            "Hello {{npc_arg0}/KHAKI}, {Gold/Gold}: {npc_arg1}",
            "Hello {Gold {slot}/KHAKI}, {Gold/Gold}: 99",
        )
        .unwrap();
        assert_eq!(args[0], ("npc_arg0".into(), "Gold {slot}"));
        assert_eq!(args[1], ("npc_arg1".into(), "99"));
        assert_eq!(
            substitute(
                "你好 {npc_arg0}，金幣：{npc_arg1}",
                &[("npc_arg0", args[0].1), ("npc_arg1", args[1].1)]
            ),
            "你好 Gold {slot}，金幣：99"
        );
    }

    #[test]
    fn npc_return_keeps_menu_context_separate_from_quest_heading() {
        with_locale(Locale::BrazilianPortuguese, || {
            assert_eq!(npc_text("Return"), "Voltar");
            assert_eq!(tr("Return"), "Entrega");
        });
    }

    #[test]
    fn locales_are_exactly_the_requested_nine() {
        assert_eq!(
            Locale::ALL.map(Locale::code),
            ["zh-TW", "en", "pt-BR", "ru", "hi", "id", "vi", "th", "ar"]
        );
        for language in Locale::ALL {
            assert_eq!(Locale::from_code(language.code()), Some(language));
            assert_eq!(language.is_rtl(), language == Locale::Arabic);
        }
        for unsupported in ["zh-CN", "es", "pt-PT", "pt", "../../evil", ""] {
            assert_eq!(Locale::from_code(unsupported), None);
        }
    }
    #[test]
    fn formatting_never_reinterprets_names_or_payloads() {
        assert_eq!(
            substitute(
                "{name}: {count}",
                &[("name", "Gold {count} 中文"), ("count", "3")]
            ),
            "Gold {count} 中文: 3"
        );
        assert_eq!(
            captures("Level {n} · {name}", "Level 20 · Account {x}").unwrap()[1].1,
            "Account {x}"
        );
        assert!(captures("{n} / {n}", "3 / 4").is_none());
    }
    #[test]
    fn scoped_locales_restore_and_are_thread_local() {
        with_locale(Locale::TraditionalChinese, || {
            assert_eq!(locale(), Locale::TraditionalChinese);
            with_locale(Locale::BrazilianPortuguese, || {
                assert_eq!(locale(), Locale::BrazilianPortuguese)
            });
            assert_eq!(locale(), Locale::TraditionalChinese);
            std::thread::spawn(|| {
                with_locale(Locale::English, || assert_eq!(locale(), Locale::English))
            })
            .join()
            .unwrap();
            assert_eq!(locale(), Locale::TraditionalChinese);
        });
    }
}
