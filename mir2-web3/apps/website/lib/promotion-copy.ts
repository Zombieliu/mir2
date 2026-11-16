import type { Locale } from "@/lib/site-copy";

type PromotionCopy = {
  label: string;
  status: string;
  summary: string;
  sections: readonly { title: string; copy: string }[];
  notice: string;
};

// Public preparation copy only. This does not configure the game promotion,
// identity, wallet, payment or settlement services.
export const promotionCopies = {
  "zh-CN": {
    "label": "推广合作（筹备中）",
    "status": "筹备中",
    "summary": "Numeron 推广合作计划正在筹备。参与资格、支持地区、佣金与结算规则仍待审核，正式开放信息将另行公布。",
    "sections": [
      {
        "title": "单层直推",
        "copy": "拟采用单层直推，不设二级及以上分润，不因招募推广员或其业绩获得返利。不收取入会或资格费，也不要求先购买月卡取得推广资格。"
      },
      {
        "title": "资格与结算待确认",
        "copy": "支持地区、参与资格、佣金比例、计佣条件及结算方式将在审核完成后公布。当前不承诺 USDC 或任何特定付款通道，不承诺收益或开放日期。"
      },
      {
        "title": "玩家与推广员分开",
        "copy": "普通玩家与推广员分别管理。游玩、登录、钱包和月卡按各自规则及开放状态使用，购买月卡不会自动取得推广资格。"
      },
      {
        "title": "第三方钱包服务",
        "copy": "钱包中的 USDC 买卖服务由相应合作商提供，适用范围与条件以服务商实际提示为准。本页不提供兑换、承兑或汇兑服务。"
      }
    ],
    "notice": "本页仅为筹备说明，不提供推广报名、佣金领取或提现。正式活动须另行审核并公布规则。"
  },
  "zh-TW": {
    "label": "推廣合作（籌備中）",
    "status": "籌備中",
    "summary": "Numeron 推廣合作計畫正在籌備。參與資格、支援地區、佣金與結算規則仍待審核，正式開放資訊將另行公布。",
    "sections": [
      {
        "title": "單層直接推薦",
        "copy": "擬採用單層直接推薦，不設二級及以上分潤，不因招募推廣員或其業績取得返利。不收取入會或資格費，也不要求先購買月卡取得推廣資格。"
      },
      {
        "title": "資格與結算待確認",
        "copy": "支援地區、參與資格、佣金比例、計佣條件與結算方式將於審核完成後公布。目前不承諾 USDC 或任何特定付款管道，也不承諾收益或開放日期。"
      },
      {
        "title": "玩家與推廣員分開",
        "copy": "一般玩家與推廣員分別管理。遊玩、登入、錢包與月卡依各自規則及開放狀態使用，購買月卡不會自動取得推廣資格。"
      },
      {
        "title": "第三方錢包服務",
        "copy": "錢包內的 USDC 買賣服務由相關合作商提供，適用範圍與條件以服務商實際提示為準。本頁不提供兌換、承兌或匯兌服務。"
      }
    ],
    "notice": "本頁僅說明籌備狀態，不提供推廣報名、佣金領取或提領。正式活動須另行審核並公布規則。"
  },
  "en": {
    "label": "Promotional partnerships (in preparation)",
    "status": "In preparation",
    "summary": "Numeron's promotional partnership program is being prepared. Eligibility, supported regions, commissions and settlement rules remain under review. Opening information will be announced separately.",
    "sections": [
      {
        "title": "Direct referrals only",
        "copy": "The proposed program has one referral level, with no second-level or deeper revenue sharing and no rewards for recruiting promoters or their performance. There are no membership or qualification fees, and buying a monthly card is not a condition of eligibility."
      },
      {
        "title": "Eligibility and settlement under review",
        "copy": "Supported regions, eligibility, commission rates, qualifying purchases and settlement methods will be announced after review. No USDC or other specific payment channel, earnings or opening date is promised."
      },
      {
        "title": "Separate player and promoter roles",
        "copy": "Players and promoters are managed separately. Gameplay, login, wallets and monthly cards remain subject to their own rules and availability. Buying a monthly card does not automatically grant promoter eligibility."
      },
      {
        "title": "Third-party wallet services",
        "copy": "USDC purchase and sale services in the wallet are provided by the respective partners, subject to the provider's actual availability and terms. This page provides no exchange, redemption or foreign-exchange services."
      }
    ],
    "notice": "This page explains preparations only. It does not offer promoter registration, commission collection or withdrawals. A formal program requires separate review and published rules."
  },
  "ja": {
    "label": "プロモーション提携（準備中）",
    "status": "準備中",
    "summary": "Numeron のプロモーション提携プログラムは準備中です。参加資格、対象地域、紹介報酬と支払い規則は審査中で、正式な開始情報は別途お知らせします。",
    "sections": [
      {
        "title": "直接紹介の一段階のみ",
        "copy": "直接紹介の一段階のみを予定しており、二段階以上の報酬分配や、紹介者の勧誘・実績に対する報酬は設けません。入会費や資格取得費はなく、参加資格のための月額カード購入も必要ありません。"
      },
      {
        "title": "参加資格と支払いは確認中",
        "copy": "対象地域、参加資格、報酬率、対象購入と支払い方法は審査後に公表します。USDC を含む特定の支払い手段、収益や開始日は確約しません。"
      },
      {
        "title": "プレイヤーと紹介者を区別",
        "copy": "プレイヤーと紹介者は別の資格として管理します。ゲーム、ログイン、ウォレットと月額カードはそれぞれの規則と提供状況に従います。月額カードの購入で紹介者資格が自動的に付与されることはありません。"
      },
      {
        "title": "第三者のウォレットサービス",
        "copy": "ウォレット内の USDC 売買サービスは各提携事業者が提供し、対象範囲と条件は事業者の実際の案内に従います。このページは両替、償還や外国為替サービスを提供しません。"
      }
    ],
    "notice": "このページは準備状況の説明のみです。紹介者登録、報酬の受け取りや出金は行いません。正式なプログラムには別途審査と規則の公表が必要です。"
  },
  "ko": {
    "label": "홍보 제휴(준비 중)",
    "status": "준비 중",
    "summary": "Numeron 홍보 제휴 프로그램을 준비하고 있습니다. 참여 자격, 지원 지역, 수수료 및 정산 규칙은 검토 중이며, 정식 개시 정보는 별도로 안내합니다.",
    "sections": [
      {
        "title": "직접 추천 한 단계만",
        "copy": "직접 추천 한 단계만 운영할 예정이며, 2단계 이상 수익 배분이나 홍보인 모집 및 그 실적에 대한 보상은 없습니다. 가입비나 자격 취득 비용이 없으며, 참여 자격을 위해 월간 카드를 먼저 구매할 필요도 없습니다."
      },
      {
        "title": "자격과 정산은 검토 중",
        "copy": "지원 지역, 참여 자격, 수수료율, 인정 구매 및 정산 방식은 검토 후 공지합니다. USDC 등 특정 지급 수단, 수익 또는 개시일을 약속하지 않습니다."
      },
      {
        "title": "플레이어와 홍보인 구분",
        "copy": "플레이어와 홍보인 자격은 별도로 관리합니다. 게임, 로그인, 지갑과 월간 카드는 각 규칙과 제공 상태를 따릅니다. 월간 카드 구매만으로 홍보인 자격이 자동 부여되지 않습니다."
      },
      {
        "title": "제3자 지갑 서비스",
        "copy": "지갑 내 USDC 매매 서비스는 해당 협력사가 제공하며, 지원 범위와 조건은 제공사의 실제 안내를 따릅니다. 이 페이지는 교환, 상환 또는 외환 서비스를 제공하지 않습니다."
      }
    ],
    "notice": "이 페이지는 준비 상태만 설명합니다. 홍보인 등록, 수수료 수령 또는 출금 기능은 제공하지 않습니다. 정식 프로그램은 별도 검토와 규칙 공지가 필요합니다."
  },
  "es": {
    "label": "Colaboraciones promocionales (en preparación)",
    "status": "En preparación",
    "summary": "El programa de colaboraciones promocionales de Numeron está en preparación. Los requisitos, las regiones admitidas, las comisiones y las reglas de liquidación siguen en revisión. La apertura se anunciará por separado.",
    "sections": [
      {
        "title": "Solo recomendaciones directas",
        "copy": "Se propone un único nivel de recomendaciones, sin reparto de ingresos de segundo nivel o superior ni recompensas por captar promotores o por su actividad. No hay cuotas de afiliación ni de acceso, y comprar un pase mensual no es requisito para participar."
      },
      {
        "title": "Requisitos y liquidación en revisión",
        "copy": "Las regiones, los requisitos, las tasas de comisión, las compras válidas y los métodos de liquidación se anunciarán tras la revisión. No se promete USDC ni otro canal de pago concreto, ingresos o fecha de apertura."
      },
      {
        "title": "Jugadores y promotores separados",
        "copy": "Los jugadores y los promotores se gestionan por separado. El juego, el acceso, las billeteras y los pases mensuales siguen sus propias reglas y disponibilidad. Comprar un pase mensual no concede automáticamente la condición de promotor."
      },
      {
        "title": "Servicios de billetera de terceros",
        "copy": "Los servicios de compra y venta de USDC en la billetera los prestan los socios correspondientes, según su disponibilidad y condiciones reales. Esta página no ofrece servicios de cambio, reembolso de activos ni cambio de divisas."
      }
    ],
    "notice": "Esta página solo explica los preparativos. No permite registrarse como promotor, cobrar comisiones ni retirar fondos. El programa formal requiere una revisión independiente y reglas publicadas."
  },
  "pt-BR": {
    "label": "Parcerias de divulgação (em preparação)",
    "status": "Em preparação",
    "summary": "O programa de parcerias de divulgação do Numeron está em preparação. Os critérios de participação, as regiões atendidas, as comissões e as regras de liquidação seguem em análise. A abertura será anunciada separadamente.",
    "sections": [
      {
        "title": "Somente indicações diretas",
        "copy": "A proposta prevê um único nível de indicação, sem participação em receitas de segundo nível ou superior nem recompensas por recrutar divulgadores ou pelo desempenho deles. Não há taxa de adesão ou de habilitação, e comprar um passe mensal não é requisito de participação."
      },
      {
        "title": "Critérios e liquidação em análise",
        "copy": "As regiões atendidas, os critérios, os percentuais de comissão, as compras elegíveis e as formas de liquidação serão anunciados após análise. Não há promessa de USDC ou outro canal de pagamento específico, de ganhos ou de data de abertura."
      },
      {
        "title": "Jogadores e divulgadores separados",
        "copy": "Jogadores e divulgadores são gerenciados separadamente. O jogo, o login, as carteiras e os passes mensais seguem suas próprias regras e disponibilidade. A compra de um passe mensal não concede automaticamente a condição de divulgador."
      },
      {
        "title": "Serviços de carteira de terceiros",
        "copy": "Os serviços de compra e venda de USDC na carteira são prestados pelos respectivos parceiros, conforme a disponibilidade e as condições efetivamente informadas pelo prestador. Esta página não oferece serviços de conversão, resgate ou câmbio."
      }
    ],
    "notice": "Esta página apenas informa os preparativos. Não oferece cadastro de divulgadores, recebimento de comissões ou saques. O programa formal depende de análise específica e publicação das regras."
  },
  "de": {
    "label": "Werbepartnerschaften (in Vorbereitung)",
    "status": "In Vorbereitung",
    "summary": "Das Werbepartnerschaftsprogramm von Numeron ist in Vorbereitung. Teilnahmevoraussetzungen, unterstützte Regionen, Provisionen und Abrechnungsregeln werden noch geprüft. Informationen zum Start werden gesondert veröffentlicht.",
    "sections": [
      {
        "title": "Nur direkte Empfehlungen",
        "copy": "Geplant ist eine einzige Empfehlungsebene, ohne Vergütung auf zweiter oder weiteren Ebenen und ohne Prämien für die Anwerbung von Werbepartnern oder deren Leistung. Es gibt keine Beitritts- oder Qualifikationsgebühren. Der Kauf eines Monatspasses ist keine Teilnahmevoraussetzung."
      },
      {
        "title": "Teilnahme und Abrechnung in Prüfung",
        "copy": "Regionen, Teilnahmevoraussetzungen, Provisionssätze, anrechenbare Käufe und Abrechnungsmethoden werden nach der Prüfung veröffentlicht. USDC oder andere bestimmte Zahlungswege, Einnahmen und ein Starttermin werden nicht zugesagt."
      },
      {
        "title": "Getrennte Spieler- und Werberollen",
        "copy": "Spieler und Werbepartner werden getrennt verwaltet. Spiel, Anmeldung, Wallets und Monatspässe unterliegen ihren eigenen Regeln und ihrer Verfügbarkeit. Ein Monatspass begründet nicht automatisch eine Werbepartnerschaft."
      },
      {
        "title": "Wallet-Dienste Dritter",
        "copy": "USDC-Käufe und -Verkäufe im Wallet werden von den jeweiligen Partnern nach deren tatsächlicher Verfügbarkeit und Bedingungen angeboten. Diese Seite bietet keine Umtausch-, Rücknahme- oder Devisendienstleistungen."
      }
    ],
    "notice": "Diese Seite erläutert nur die Vorbereitung. Sie ermöglicht keine Werbepartner-Anmeldung, Provisionsauszahlung oder Auszahlung von Guthaben. Das formelle Programm erfordert eine gesonderte Prüfung und veröffentlichte Regeln."
  },
  "fr": {
    "label": "Partenariats promotionnels (en préparation)",
    "status": "En préparation",
    "summary": "Le programme de partenariats promotionnels de Numeron est en préparation. Les conditions, les régions prises en charge, les commissions et les règles de règlement sont encore à l'étude. L'ouverture sera annoncée séparément.",
    "sections": [
      {
        "title": "Uniquement des recommandations directes",
        "copy": "Le programme envisagé comporte un seul niveau de recommandation, sans partage de revenus de deuxième niveau ou au-delà, ni récompense pour recruter des promoteurs ou pour leurs résultats. Aucun droit d'adhésion ou d'accès n'est demandé et l'achat d'un pass mensuel n'est pas une condition de participation."
      },
      {
        "title": "Conditions et règlement à l'étude",
        "copy": "Les régions, les conditions, les taux de commission, les achats admissibles et les modes de règlement seront annoncés après examen. Aucun paiement en USDC ou par un autre canal précis, revenu ou calendrier d'ouverture n'est promis."
      },
      {
        "title": "Joueurs et promoteurs distincts",
        "copy": "Les joueurs et les promoteurs sont gérés séparément. Le jeu, la connexion, les portefeuilles et les pass mensuels suivent leurs propres règles et disponibilités. Acheter un pass mensuel ne confère pas automatiquement le statut de promoteur."
      },
      {
        "title": "Services de portefeuille tiers",
        "copy": "Les services d'achat et de vente d'USDC dans le portefeuille sont fournis par les partenaires concernés, selon leur disponibilité et leurs conditions effectives. Cette page ne propose aucun service de conversion, de remboursement d'actifs ou de change."
      }
    ],
    "notice": "Cette page présente uniquement les préparatifs. Elle ne permet ni inscription de promoteur, ni perception de commissions, ni retrait. Le programme officiel nécessite un examen distinct et la publication de ses règles."
  },
  "ru": {
    "label": "Партнёрское продвижение (в подготовке)",
    "status": "В подготовке",
    "summary": "Партнёрская программа продвижения Numeron находится в подготовке. Требования к участникам, доступные регионы, комиссии и правила расчётов ещё рассматриваются. Об открытии будет объявлено отдельно.",
    "sections": [
      {
        "title": "Только прямые рекомендации",
        "copy": "Планируется один уровень рекомендаций, без выплат за второй и последующие уровни, привлечение других партнёров или их результаты. Вступительных взносов и платы за получение статуса нет. Покупать месячный пропуск для участия не требуется."
      },
      {
        "title": "Условия и расчёты на рассмотрении",
        "copy": "Регионы, требования, ставки комиссии, учитываемые покупки и способы расчётов будут опубликованы после проверки. Выплаты в USDC или через иной конкретный канал, доход и дата открытия не обещаются."
      },
      {
        "title": "Игроки и партнёры отдельно",
        "copy": "Статусы игрока и партнёра учитываются отдельно. Игра, вход, кошельки и месячные пропуска регулируются собственными правилами и доступностью. Покупка месячного пропуска не даёт статус партнёра автоматически."
      },
      {
        "title": "Сторонние сервисы кошелька",
        "copy": "Покупку и продажу USDC в кошельке обеспечивают соответствующие партнёры на фактически доступных у них условиях. Эта страница не предоставляет услуги обмена, погашения активов или валютных операций."
      }
    ],
    "notice": "Страница описывает только подготовку. Здесь нельзя зарегистрироваться партнёром, получить комиссию или вывести средства. Для запуска нужны отдельная проверка и опубликованные правила."
  },
  "pl": {
    "label": "Współpraca promocyjna (w przygotowaniu)",
    "status": "W przygotowaniu",
    "summary": "Program współpracy promocyjnej Numeron jest w przygotowaniu. Warunki udziału, obsługiwane regiony, prowizje i zasady rozliczeń są nadal analizowane. Informacje o uruchomieniu zostaną ogłoszone osobno.",
    "sections": [
      {
        "title": "Wyłącznie bezpośrednie polecenia",
        "copy": "Planowany jest jeden poziom poleceń, bez udziału w przychodach drugiego i dalszych poziomów ani nagród za rekrutację promotorów lub ich wyniki. Nie ma opłat członkowskich ani kwalifikacyjnych, a zakup karnetu miesięcznego nie jest warunkiem udziału."
      },
      {
        "title": "Warunki i rozliczenia w analizie",
        "copy": "Regiony, warunki udziału, stawki prowizji, kwalifikujące się zakupy i metody rozliczeń zostaną ogłoszone po analizie. Nie obiecujemy USDC ani innego określonego kanału płatności, zarobków czy daty otwarcia."
      },
      {
        "title": "Oddzielne role gracza i promotora",
        "copy": "Gracze i promotorzy są zarządzani oddzielnie. Gra, logowanie, portfele i karnety miesięczne podlegają własnym zasadom i dostępności. Zakup karnetu miesięcznego nie nadaje automatycznie statusu promotora."
      },
      {
        "title": "Usługi portfela podmiotów trzecich",
        "copy": "Usługi zakupu i sprzedaży USDC w portfelu świadczą odpowiedni partnerzy zgodnie z ich rzeczywistą dostępnością i warunkami. Ta strona nie świadczy usług wymiany, wykupu aktywów ani wymiany walut."
      }
    ],
    "notice": "Strona wyłącznie opisuje przygotowania. Nie umożliwia rejestracji promotorów, odbioru prowizji ani wypłat. Formalny program wymaga osobnej analizy i publikacji zasad."
  },
  "tr": {
    "label": "Tanıtım ortaklığı (hazırlanıyor)",
    "status": "Hazırlanıyor",
    "summary": "Numeron tanıtım ortaklığı programı hazırlanıyor. Katılım koşulları, desteklenen bölgeler, komisyonlar ve ödeme kuralları inceleniyor. Açılış bilgileri ayrıca duyurulacak.",
    "sections": [
      {
        "title": "Yalnızca doğrudan yönlendirme",
        "copy": "Tek yönlendirme seviyesi planlanıyor. İkinci veya daha alt seviyelerde gelir paylaşımı, tanıtım ortağı toplama ya da onların performansı için ödül yoktur. Üyelik veya yeterlilik ücreti alınmaz; katılmak için önce aylık kart satın almak gerekmez."
      },
      {
        "title": "Koşullar ve ödemeler inceleniyor",
        "copy": "Bölgeler, katılım koşulları, komisyon oranları, uygun alışverişler ve ödeme yöntemleri inceleme sonrasında duyurulacak. USDC veya belirli başka bir ödeme kanalı, kazanç ya da açılış tarihi taahhüt edilmez."
      },
      {
        "title": "Oyuncu ve tanıtım ortağı ayrı",
        "copy": "Oyuncular ve tanıtım ortakları ayrı yönetilir. Oyun, giriş, cüzdanlar ve aylık kartlar kendi kurallarına ve kullanılabilirliklerine tabidir. Aylık kart satın almak otomatik olarak tanıtım ortağı statüsü sağlamaz."
      },
      {
        "title": "Üçüncü taraf cüzdan hizmetleri",
        "copy": "Cüzdandaki USDC alım satım hizmetleri ilgili ortaklarca, sağlayıcının gerçek kullanılabilirliği ve koşulları çerçevesinde sunulur. Bu sayfa takas, varlık itfası veya döviz hizmeti sunmaz."
      }
    ],
    "notice": "Bu sayfa yalnızca hazırlıkları açıklar. Tanıtım ortağı kaydı, komisyon tahsilatı veya para çekme sunmaz. Resmî program için ayrı inceleme ve yayımlanmış kurallar gerekir."
  },
  "th": {
    "label": "พันธมิตรส่งเสริมการขาย (อยู่ระหว่างเตรียมการ)",
    "status": "อยู่ระหว่างเตรียมการ",
    "summary": "โครงการพันธมิตรส่งเสริมการขายของ Numeron อยู่ระหว่างเตรียมการ คุณสมบัติ พื้นที่ที่รองรับ ค่าคอมมิชชัน และกฎการชำระเงินยังอยู่ระหว่างตรวจสอบ โดยจะประกาศการเปิดโครงการแยกต่างหาก",
    "sections": [
      {
        "title": "การแนะนำโดยตรงเพียงชั้นเดียว",
        "copy": "แผนงานใช้การแนะนำโดยตรงเพียงชั้นเดียว ไม่มีส่วนแบ่งชั้นที่สองหรือสูงกว่า และไม่มีรางวัลจากการรับสมัครผู้ส่งเสริมการขายหรือผลงานของบุคคลเหล่านั้น ไม่มีค่าสมัครหรือค่ารับสิทธิ์ และไม่ต้องซื้อบัตรรายเดือนก่อนเพื่อให้มีคุณสมบัติ"
      },
      {
        "title": "คุณสมบัติและการชำระเงินยังรอยืนยัน",
        "copy": "พื้นที่ คุณสมบัติ อัตราคอมมิชชัน การซื้อที่เข้าเกณฑ์ และวิธีชำระเงินจะประกาศหลังการตรวจสอบ ยังไม่รับรอง USDC หรือช่องทางชำระเงินใดโดยเฉพาะ รายได้ หรือวันเปิดโครงการ"
      },
      {
        "title": "แยกผู้เล่นและผู้ส่งเสริมการขาย",
        "copy": "ผู้เล่นและผู้ส่งเสริมการขายได้รับการจัดการแยกกัน การเล่นเกม การเข้าสู่ระบบ กระเป๋าเงิน และบัตรรายเดือนเป็นไปตามกฎและสถานะการให้บริการของแต่ละส่วน การซื้อบัตรรายเดือนไม่ทำให้ได้รับสิทธิ์ผู้ส่งเสริมการขายโดยอัตโนมัติ"
      },
      {
        "title": "บริการกระเป๋าเงินของบุคคลที่สาม",
        "copy": "บริการซื้อขาย USDC ในกระเป๋าเงินให้บริการโดยพันธมิตรที่เกี่ยวข้อง ตามขอบเขตและเงื่อนไขที่ผู้ให้บริการระบุจริง หน้านี้ไม่ให้บริการแลกเปลี่ยน ไถ่ถอน หรือแลกเปลี่ยนเงินตรา"
      }
    ],
    "notice": "หน้านี้อธิบายสถานะการเตรียมการเท่านั้น ไม่เปิดรับสมัครผู้ส่งเสริมการขาย รับค่าคอมมิชชัน หรือถอนเงิน โครงการอย่างเป็นทางการต้องผ่านการตรวจสอบและประกาศกฎแยกต่างหาก"
  },
  "vi": {
    "label": "Hợp tác quảng bá (đang chuẩn bị)",
    "status": "Đang chuẩn bị",
    "summary": "Chương trình hợp tác quảng bá Numeron đang được chuẩn bị. Điều kiện tham gia, khu vực hỗ trợ, hoa hồng và quy tắc thanh toán vẫn đang được xem xét. Thông tin mở chương trình sẽ được công bố riêng.",
    "sections": [
      {
        "title": "Chỉ giới thiệu trực tiếp một cấp",
        "copy": "Dự kiến chỉ có một cấp giới thiệu trực tiếp, không chia doanh thu từ cấp hai trở lên và không thưởng cho việc tuyển người quảng bá hoặc kết quả của họ. Không thu phí gia nhập hay phí đủ điều kiện và không yêu cầu mua thẻ tháng trước để được tham gia."
      },
      {
        "title": "Điều kiện và thanh toán đang xem xét",
        "copy": "Khu vực hỗ trợ, điều kiện tham gia, tỷ lệ hoa hồng, giao dịch đủ điều kiện và phương thức thanh toán sẽ được công bố sau khi xem xét. Hiện không cam kết USDC hay kênh thanh toán cụ thể nào, thu nhập hoặc ngày mở chương trình."
      },
      {
        "title": "Tách biệt người chơi và người quảng bá",
        "copy": "Người chơi và người quảng bá được quản lý riêng. Trò chơi, đăng nhập, ví và thẻ tháng tuân theo quy tắc và tình trạng cung cấp riêng. Mua thẻ tháng không tự động tạo tư cách người quảng bá."
      },
      {
        "title": "Dịch vụ ví của bên thứ ba",
        "copy": "Dịch vụ mua bán USDC trong ví do các đối tác tương ứng cung cấp, theo phạm vi và điều kiện thực tế do nhà cung cấp thông báo. Trang này không cung cấp dịch vụ chuyển đổi, hoàn đổi tài sản hoặc ngoại hối."
      }
    ],
    "notice": "Trang này chỉ giải thích tình trạng chuẩn bị, không cung cấp đăng ký người quảng bá, nhận hoa hồng hoặc rút tiền. Chương trình chính thức cần được xem xét riêng và công bố quy tắc."
  },
  "id": {
    "label": "Kemitraan promosi (dalam persiapan)",
    "status": "Dalam persiapan",
    "summary": "Program kemitraan promosi Numeron sedang dipersiapkan. Kelayakan, wilayah yang didukung, komisi, dan aturan penyelesaian pembayaran masih ditinjau. Informasi pembukaan akan diumumkan secara terpisah.",
    "sections": [
      {
        "title": "Hanya rujukan langsung satu tingkat",
        "copy": "Rencana program menggunakan satu tingkat rujukan langsung, tanpa pembagian pendapatan tingkat kedua atau lebih, serta tanpa imbalan atas perekrutan promotor atau kinerja mereka. Tidak ada biaya keanggotaan atau kualifikasi, dan membeli kartu bulanan bukan syarat kelayakan."
      },
      {
        "title": "Kelayakan dan pembayaran ditinjau",
        "copy": "Wilayah, kelayakan, tarif komisi, pembelian yang memenuhi syarat, dan metode pembayaran akan diumumkan setelah peninjauan. Tidak ada janji USDC atau saluran pembayaran tertentu, penghasilan, maupun tanggal pembukaan."
      },
      {
        "title": "Pemain dan promotor dipisahkan",
        "copy": "Pemain dan promotor dikelola secara terpisah. Permainan, login, dompet, dan kartu bulanan mengikuti aturan serta ketersediaannya masing-masing. Membeli kartu bulanan tidak otomatis memberikan status promotor."
      },
      {
        "title": "Layanan dompet pihak ketiga",
        "copy": "Layanan pembelian dan penjualan USDC di dompet disediakan oleh mitra terkait, sesuai ketersediaan dan ketentuan nyata dari penyedia. Halaman ini tidak menyediakan layanan penukaran, penebusan aset, atau valuta asing."
      }
    ],
    "notice": "Halaman ini hanya menjelaskan persiapan. Tidak tersedia pendaftaran promotor, penerimaan komisi, atau penarikan dana. Program resmi memerlukan peninjauan terpisah dan aturan yang dipublikasikan."
  },
  "ar": {
    "label": "شراكات الترويج (قيد الإعداد)",
    "status": "قيد الإعداد",
    "summary": "برنامج شراكات الترويج في Numeron قيد الإعداد. ما زالت شروط الأهلية والمناطق المدعومة والعمولات وقواعد التسوية قيد المراجعة. سيُعلن عن فتح البرنامج بشكل منفصل.",
    "sections": [
      {
        "title": "إحالات مباشرة بمستوى واحد فقط",
        "copy": "الخطة المقترحة تعتمد مستوى واحدًا من الإحالات المباشرة، دون تقاسم إيرادات من المستوى الثاني أو ما بعده، ودون مكافآت لتجنيد مروّجين أو على أدائهم. لا توجد رسوم انضمام أو تأهيل، ولا يُشترط شراء بطاقة شهرية للحصول على الأهلية."
      },
      {
        "title": "الأهلية والتسوية قيد المراجعة",
        "copy": "سيُعلن عن المناطق والأهلية ونسب العمولات والمشتريات المؤهلة وطرق التسوية بعد المراجعة. لا يوجد وعد بالدفع بعملة USDC أو عبر أي قناة محددة أخرى، ولا بأرباح أو بتاريخ افتتاح."
      },
      {
        "title": "فصل اللاعبين عن المروّجين",
        "copy": "تُدار صفتا اللاعب والمروّج بشكل منفصل. يخضع اللعب وتسجيل الدخول والمحافظ والبطاقات الشهرية لقواعد كل منها ومدى توفرها. شراء بطاقة شهرية لا يمنح أهلية المروّج تلقائيًا."
      },
      {
        "title": "خدمات المحافظ لدى أطراف ثالثة",
        "copy": "يقدّم الشركاء المعنيون خدمات شراء USDC وبيعه داخل المحفظة، وفق نطاق الخدمة وشروطها الفعلية لدى المزوّد. لا تقدّم هذه الصفحة خدمات تحويل أو استرداد أصول أو صرف عملات."
      }
    ],
    "notice": "تشرح هذه الصفحة مرحلة الإعداد فقط، ولا تتيح تسجيل المروّجين أو تحصيل العمولات أو السحب. يتطلب البرنامج الرسمي مراجعة مستقلة ونشر القواعد."
  }
} satisfies Record<Locale, PromotionCopy>;

export function getPromotionCopy(locale: Locale) {
  return promotionCopies[locale];
}
