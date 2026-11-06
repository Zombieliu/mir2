import type { Locale } from "@/lib/site-copy";
import { getOperationsCopy } from "@/lib/operations-copy";

type RechargeCopy = {
  label: string; aiLabel: string; status: string; intro: string;
  pricingTitle: string; pricingNote: string;
  columns: readonly [string, string, string, string];
  pending: string; sections: readonly { title: string; copy: string }[];
};

// Only the first public price was confirmed by the owner on 2026-11-05.
// Publishing this informational offer never enables a payment provider.
export const publicRechargeOffer = { currency: "USD", amountMinor: 100, credits: 100, purchasesEnabled: false } as const;

export const rechargeCopies = {
  "zh-CN": {
    "label": "游戏点券充值",
    "aiLabel": "AI 订阅（规划中）",
    "status": "筹备中 · 尚未开放购买",
    "intro": "本页公开已确认的 Numeron 游戏点券档位。游戏内月卡与计划中的 AI 订阅分别提供说明，当前不接受购买。",
    "pricingTitle": "充值档位与价格",
    "pricingNote": "目前仅确认下列档位。其他档位、税费、服务地区、有效期、测试清档、交付时间和退款条件仍待确认。测试数据不作为其他正式售价的依据。",
    "columns": [
      "商品",
      "支付金额",
      "到账点券",
      "状态"
    ],
    "pending": "待确认",
    "sections": [
      {
        "title": "点券用途",
        "copy": "付费点券记入充值账号的游戏余额，仅用于 Numeron 内消费。点券余额不可在玩家之间转让、提现或兑换法币和加密资产。充值不会自动开通月卡。玩家可用自己的点券在官方商城为指定角色购买月卡作为赠礼。"
      },
      {
        "title": "支付与钱包",
        "copy": "钱包登录、点券充值和钱包内的第三方 USDC 买卖是不同流程。USDC 买卖由相应合作商提供；可用支付渠道以正式结算页实际启用为准。"
      },
      {
        "title": "订单与交付",
        "copy": "正式购买时应核对账号、角色、服务器、金额与点券数量，付款结果须经服务端确认。当前网页不接受付款。"
      }
    ]
  },
  "en": {
    "label": "Game credit top-ups",
    "aiLabel": "AI subscriptions (planned)",
    "status": "In preparation · Purchases unavailable",
    "intro": "This page lists the confirmed Numeron game credit package. In-game monthly cards and planned AI subscriptions are explained separately. Purchases are currently unavailable.",
    "pricingTitle": "Top-up packages and prices",
    "pricingNote": "Only the package below is confirmed. Other packages, taxes, regions, validity, test resets, delivery times and refund conditions remain unconfirmed. Test data does not establish any other production price.",
    "columns": [
      "Product",
      "Payment amount",
      "Credits received",
      "Status"
    ],
    "pending": "To be confirmed",
    "sections": [
      {
        "title": "Using credits",
        "copy": "Paid credits go to the chosen game account for use within Numeron. Players cannot transfer their credit balance to each other, cash it out or exchange it for fiat currency or cryptoassets. A top-up does not activate a monthly card. Players can spend their own credits in the official shop to gift a monthly card to a selected character."
      },
      {
        "title": "Payments and wallets",
        "copy": "Wallet login, credit top-ups and third-party USDC trading in the wallet are separate flows. USDC trading is provided by the respective partners. Available payment methods will be those actually enabled at production checkout."
      },
      {
        "title": "Orders and delivery",
        "copy": "For a production purchase, check the account, character, server, amount and credit quantity. Payment results require server confirmation. This website currently accepts no payments."
      }
    ]
  },
  "pt-BR": {
    "label": "Recarga de créditos do jogo",
    "aiLabel": "Assinaturas de IA (planejadas)",
    "status": "Em preparação · Compras indisponíveis",
    "intro": "Esta página apresenta o pacote confirmado de créditos Numeron. Passes mensais no jogo e assinaturas de IA planejadas têm explicações separadas. As compras ainda estão indisponíveis.",
    "pricingTitle": "Pacotes e preços de recarga",
    "pricingNote": "Somente o pacote abaixo está confirmado. Outros pacotes, impostos, regiões, validade, redefinições de teste, prazos de entrega e reembolsos ainda precisam de confirmação. Dados de teste não definem outros preços oficiais.",
    "columns": [
      "Produto",
      "Valor do pagamento",
      "Créditos recebidos",
      "Status"
    ],
    "pending": "A confirmar",
    "sections": [
      {
        "title": "Uso dos créditos",
        "copy": "Os créditos pagos são lançados na conta do jogo escolhida e usados apenas no Numeron. O saldo não pode ser transferido entre jogadores, sacado nem trocado por moeda fiduciária ou criptoativos. A recarga não ativa um passe mensal. Na loja oficial, os jogadores podem usar os próprios créditos para presentear um personagem escolhido com um passe mensal."
      },
      {
        "title": "Pagamentos e carteiras",
        "copy": "Login por carteira, recarga e negociação de USDC por terceiros na carteira são processos distintos. A negociação de USDC é fornecida pelos respectivos parceiros. Os meios disponíveis serão os efetivamente habilitados no checkout oficial."
      },
      {
        "title": "Pedidos e entrega",
        "copy": "Em uma compra oficial, confira conta, personagem, servidor, valor e quantidade de créditos. O resultado do pagamento depende da confirmação do servidor. Este site ainda não recebe pagamentos."
      }
    ]
  },
  "vi": {
    "label": "Nạp điểm trong trò chơi",
    "aiLabel": "Gói AI (dự kiến)",
    "status": "Đang chuẩn bị · Chưa mở mua",
    "intro": "Trang này công bố gói điểm Numeron đã xác nhận. Thẻ tháng trong trò chơi và gói AI dự kiến được giải thích riêng. Hiện chưa mở mua.",
    "pricingTitle": "Gói nạp và giá",
    "pricingNote": "Chỉ gói bên dưới đã được xác nhận. Các gói khác, thuế, khu vực, thời hạn, xóa dữ liệu thử nghiệm, thời gian giao và điều kiện hoàn tiền còn chờ xác nhận. Dữ liệu thử nghiệm không xác lập giá chính thức khác.",
    "columns": [
      "Sản phẩm",
      "Số tiền thanh toán",
      "Điểm nhận được",
      "Trạng thái"
    ],
    "pending": "Chờ xác nhận",
    "sections": [
      {
        "title": "Sử dụng điểm",
        "copy": "Điểm nạp được ghi vào tài khoản đã chọn và chỉ dùng trong Numeron. Không thể chuyển số dư giữa người chơi, rút tiền hoặc đổi sang tiền pháp định hay tài sản mã hóa. Nạp điểm không tự kích hoạt thẻ tháng. Người chơi có thể dùng điểm của mình tại cửa hàng chính thức để tặng thẻ tháng cho nhân vật đã chọn."
      },
      {
        "title": "Thanh toán và ví",
        "copy": "Đăng nhập bằng ví, nạp điểm và mua bán USDC qua bên thứ ba trong ví là các quy trình riêng. Mua bán USDC do đối tác tương ứng cung cấp. Kênh thanh toán tùy thuộc các phương thức thực sự được bật tại trang thanh toán chính thức."
      },
      {
        "title": "Đơn hàng và giao điểm",
        "copy": "Khi mua chính thức, hãy kiểm tra tài khoản, nhân vật, máy chủ, số tiền và số điểm. Kết quả thanh toán cần máy chủ xác nhận. Hiện trang này không nhận thanh toán."
      }
    ]
  },
  "zh-TW": {
    "label": "遊戲點數儲值",
    "aiLabel": "AI 訂閱（規劃中）",
    "status": "籌備中 · 尚未開放購買",
    "intro": "本頁公開已確認的 Numeron 遊戲點數方案。遊戲內月卡與規劃中的 AI 訂閱分別說明，目前不接受購買。",
    "pricingTitle": "儲值方案與價格",
    "pricingNote": "目前僅確認下列方案。其他方案、稅費、服務地區、期限、測試清檔、交付時間及退款條件仍待確認。測試資料不作為其他正式售價依據。",
    "columns": [
      "商品",
      "付款金額",
      "入帳點數",
      "狀態"
    ],
    "pending": "待確認",
    "sections": [
      {
        "title": "點數用途",
        "copy": "付費點數記入儲值帳號的遊戲餘額，僅供 Numeron 內消費。點數餘額不可在玩家之間轉讓、提領或兌換法幣及加密資產。儲值不會自動開通月卡。玩家可使用自己的點數在官方商城購買月卡，贈送給指定角色。"
      },
      {
        "title": "付款與錢包",
        "copy": "錢包登入、點數儲值與錢包內第三方 USDC 買賣是不同流程。USDC 買賣由相關合作商提供；可用付款管道以正式結帳頁實際啟用為準。"
      },
      {
        "title": "訂單與交付",
        "copy": "正式購買時應核對帳號、角色、伺服器、金額及點數，付款結果須經伺服器確認。目前本頁不接受付款。"
      }
    ]
  },
  "ja": {
    "label": "ゲーム内クレジットのチャージ",
    "aiLabel": "AIサブスクリプション（計画中）",
    "status": "準備中 · 購入は未開始",
    "intro": "確認済みの Numeron ゲーム内クレジット商品を掲載しています。月額カードと計画中の AI サブスクリプションは別途説明します。現在は購入できません。",
    "pricingTitle": "チャージ内容と価格",
    "pricingNote": "確定しているのは下記商品のみです。他の商品、税金、対象地域、有効期限、テストリセット、付与時期と返金条件は未確定です。テストデータで他の正式価格は決まりません。",
    "columns": [
      "商品",
      "支払額",
      "付与クレジット",
      "状況"
    ],
    "pending": "確認待ち",
    "sections": [
      {
        "title": "クレジットの用途",
        "copy": "購入したクレジットは指定したゲームアカウントに付与され、Numeron 内でのみ使えます。プレイヤー間での残高の移転、出金、法定通貨や暗号資産への交換はできません。チャージだけでは月額カードは有効になりません。公式ショップでは自分のクレジットを使い、指定したキャラクターに月額カードを贈れます。"
      },
      {
        "title": "決済とウォレット",
        "copy": "ウォレットログイン、チャージ、ウォレット内の第三者による USDC 売買は別の手続きです。USDC 売買は各提携事業者が提供します。利用可能な支払方法は正式な決済画面で実際に有効なものに限ります。"
      },
      {
        "title": "注文と付与",
        "copy": "正式購入ではアカウント、キャラクター、サーバー、金額と付与数を確認してください。支払結果はサーバーによる確認が必要です。現在このサイトは支払いを受け付けていません。"
      }
    ]
  },
  "ko": {
    "label": "게임 크레딧 충전",
    "aiLabel": "AI 구독(계획 중)",
    "status": "준비 중 · 구매 미개시",
    "intro": "확인된 Numeron 게임 크레딧 상품을 안내합니다. 게임 내 월간 카드와 계획 중인 AI 구독은 별도로 설명합니다. 현재 구매할 수 없습니다.",
    "pricingTitle": "충전 상품과 가격",
    "pricingNote": "아래 상품만 확인되었습니다. 다른 상품, 세금, 지역, 유효기간, 테스트 초기화, 지급 시간과 환불 조건은 미정입니다. 테스트 데이터로 다른 정식 가격을 정하지 않습니다.",
    "columns": [
      "상품",
      "결제 금액",
      "지급 크레딧",
      "상태"
    ],
    "pending": "확인 예정",
    "sections": [
      {
        "title": "크레딧 사용",
        "copy": "유료 크레딧은 선택한 게임 계정에 적립되며 Numeron 안에서만 사용할 수 있습니다. 잔액을 다른 플레이어에게 이전하거나 출금하거나 법정화폐 및 암호화폐로 교환할 수 없습니다. 충전만으로 월간 카드는 활성화되지 않습니다. 공식 상점에서 자신의 크레딧으로 지정한 캐릭터에게 월간 카드를 선물할 수 있습니다."
      },
      {
        "title": "결제와 지갑",
        "copy": "지갑 로그인, 크레딧 충전, 지갑 내 제3자 USDC 매매는 별도 절차입니다. USDC 매매는 해당 협력사가 제공합니다. 결제 수단은 정식 결제 화면에서 실제 활성화된 항목을 따릅니다."
      },
      {
        "title": "주문과 지급",
        "copy": "정식 구매 시 계정, 캐릭터, 서버, 금액과 크레딧 수량을 확인해야 합니다. 결제 결과는 서버 확인이 필요합니다. 현재 이 사이트는 결제를 받지 않습니다."
      }
    ]
  },
  "es": {
    "label": "Recarga de créditos del juego",
    "aiLabel": "Suscripciones de IA (previstas)",
    "status": "En preparación · Compras no disponibles",
    "intro": "Esta página muestra el paquete confirmado de créditos Numeron. Los pases mensuales del juego y las futuras suscripciones de IA se explican por separado. Las compras no están disponibles.",
    "pricingTitle": "Paquetes y precios",
    "pricingNote": "Solo el paquete siguiente está confirmado. Otros paquetes, impuestos, regiones, vigencia, reinicios de prueba, entrega y condiciones de reembolso están por confirmar. Los datos de prueba no fijan otros precios oficiales.",
    "columns": [
      "Producto",
      "Importe",
      "Créditos recibidos",
      "Estado"
    ],
    "pending": "Por confirmar",
    "sections": [
      {
        "title": "Uso de créditos",
        "copy": "Los créditos de pago se abonan en la cuenta de juego elegida y solo se usan en Numeron. El saldo no se puede transferir entre jugadores, retirar ni cambiar por dinero de curso legal o criptoactivos. La recarga no activa un pase mensual. En la tienda oficial puedes usar tus créditos para regalar un pase mensual a un personaje elegido."
      },
      {
        "title": "Pagos y billeteras",
        "copy": "El acceso con billetera, las recargas y la compraventa de USDC por terceros son procesos distintos. La compraventa la prestan los socios correspondientes. Los medios disponibles serán los realmente habilitados en el pago oficial."
      },
      {
        "title": "Pedidos y entrega",
        "copy": "En una compra oficial, revise cuenta, personaje, servidor, importe y créditos. El servidor debe confirmar el pago. Este sitio aún no acepta pagos."
      }
    ]
  },
  "de": {
    "label": "Spielguthaben aufladen",
    "aiLabel": "KI-Abonnements (geplant)",
    "status": "In Vorbereitung · Kauf nicht verfügbar",
    "intro": "Hier steht das bestätigte Numeron-Guthabenpaket. Monatspässe im Spiel und geplante KI-Abonnements werden getrennt erläutert. Käufe sind derzeit nicht möglich.",
    "pricingTitle": "Aufladepakete und Preise",
    "pricingNote": "Nur das folgende Paket ist bestätigt. Weitere Pakete, Steuern, Regionen, Gültigkeit, Testzurücksetzungen, Bereitstellungszeiten und Erstattungsbedingungen sind noch offen. Testdaten legen keine weiteren offiziellen Preise fest.",
    "columns": [
      "Produkt",
      "Zahlbetrag",
      "Erhaltenes Guthaben",
      "Status"
    ],
    "pending": "Noch zu bestätigen",
    "sections": [
      {
        "title": "Guthaben verwenden",
        "copy": "Gekauftes Guthaben wird dem gewählten Spielkonto gutgeschrieben und kann nur in Numeron verwendet werden. Es lässt sich weder zwischen Spielern übertragen noch auszahlen oder gegen gesetzliche Währungen oder Kryptowerte tauschen. Eine Aufladung aktiviert keinen Monatspass. Im offiziellen Shop können Spieler mit ihrem eigenen Guthaben einem gewählten Charakter einen Monatspass schenken."
      },
      {
        "title": "Zahlungen und Wallets",
        "copy": "Wallet-Anmeldung, Aufladen und USDC-Handel durch Dritte im Wallet sind getrennte Vorgänge. Den USDC-Handel bieten die jeweiligen Partner an. Verfügbar sind die tatsächlich im offiziellen Checkout aktivierten Zahlungsmethoden."
      },
      {
        "title": "Bestellung und Bereitstellung",
        "copy": "Prüfen Sie beim offiziellen Kauf Konto, Charakter, Server, Betrag und Guthabenmenge. Das Zahlungsergebnis muss vom Server bestätigt werden. Diese Website nimmt derzeit keine Zahlungen an."
      }
    ]
  },
  "fr": {
    "label": "Recharge de crédits du jeu",
    "aiLabel": "Abonnements IA (prévus)",
    "status": "En préparation · Achats indisponibles",
    "intro": "Cette page présente le lot de crédits Numeron confirmé. Les pass mensuels du jeu et les abonnements IA prévus sont expliqués séparément. Les achats sont indisponibles.",
    "pricingTitle": "Offres et prix de recharge",
    "pricingNote": "Seul le lot ci-dessous est confirmé. Les autres lots, taxes, régions, validité, réinitialisations de test, délais et conditions de remboursement restent à confirmer. Les données de test ne fixent aucun autre prix officiel.",
    "columns": [
      "Produit",
      "Montant payé",
      "Crédits reçus",
      "Statut"
    ],
    "pending": "À confirmer",
    "sections": [
      {
        "title": "Utilisation des crédits",
        "copy": "Les crédits payants sont ajoutés au compte de jeu choisi et utilisables uniquement dans Numeron. Le solde ne peut être ni transféré entre joueurs, ni retiré, ni échangé contre de la monnaie légale ou des cryptoactifs. Une recharge n’active pas le pass mensuel. Dans la boutique officielle, les joueurs peuvent utiliser leurs crédits pour offrir un pass mensuel au personnage de leur choix."
      },
      {
        "title": "Paiements et portefeuilles",
        "copy": "La connexion par portefeuille, la recharge et les échanges d'USDC assurés par des tiers sont des opérations distinctes. Les partenaires concernés fournissent les échanges d'USDC. Les moyens disponibles seront ceux réellement activés au paiement officiel."
      },
      {
        "title": "Commandes et livraison",
        "copy": "Lors d'un achat officiel, vérifiez compte, personnage, serveur, montant et quantité de crédits. Le serveur doit confirmer le paiement. Ce site n'accepte actuellement aucun paiement."
      }
    ]
  },
  "ru": {
    "label": "Пополнение игровых кредитов",
    "aiLabel": "Подписки ИИ (планируются)",
    "status": "Подготовка · Покупки недоступны",
    "intro": "Здесь указан подтверждённый пакет кредитов Numeron. Игровые месячные пропуски и планируемые подписки ИИ описываются отдельно. Покупки пока недоступны.",
    "pricingTitle": "Пакеты и цены",
    "pricingNote": "Подтверждён только указанный ниже пакет. Другие пакеты, налоги, регионы, срок действия, тестовые сбросы, сроки начисления и условия возврата ещё не определены. Тестовые данные не устанавливают другие официальные цены.",
    "columns": [
      "Продукт",
      "Сумма оплаты",
      "Начисляемые кредиты",
      "Статус"
    ],
    "pending": "Требует подтверждения",
    "sections": [
      {
        "title": "Использование кредитов",
        "copy": "Платные кредиты зачисляются на выбранный игровой аккаунт и используются только в Numeron. Баланс нельзя передавать другим игрокам, выводить или обменивать на фиатные деньги и криптоактивы. Пополнение не активирует месячный пропуск. В официальном магазине можно потратить свои кредиты, чтобы подарить пропуск выбранному персонажу."
      },
      {
        "title": "Платежи и кошельки",
        "copy": "Вход через кошелёк, пополнение и обмен USDC сторонними поставщиками — отдельные процессы. Обмен USDC предоставляют соответствующие партнёры. Доступны будут только способы, включённые в официальной форме оплаты."
      },
      {
        "title": "Заказы и начисление",
        "copy": "При официальной покупке проверяйте аккаунт, персонажа, сервер, сумму и количество кредитов. Оплату должен подтвердить сервер. Сейчас сайт не принимает платежи."
      }
    ]
  },
  "pl": {
    "label": "Doładowanie kredytów w grze",
    "aiLabel": "Subskrypcje AI (planowane)",
    "status": "W przygotowaniu · Zakupy niedostępne",
    "intro": "Strona przedstawia potwierdzony pakiet kredytów Numeron. Miesięczne karnety w grze i planowane subskrypcje AI opisano osobno. Zakupy są obecnie niedostępne.",
    "pricingTitle": "Pakiety i ceny doładowań",
    "pricingNote": "Potwierdzony jest tylko poniższy pakiet. Inne pakiety, podatki, regiony, ważność, reset testów, terminy dostawy i warunki zwrotu wymagają potwierdzenia. Dane testowe nie ustalają innych oficjalnych cen.",
    "columns": [
      "Produkt",
      "Kwota płatności",
      "Otrzymane kredyty",
      "Status"
    ],
    "pending": "Do potwierdzenia",
    "sections": [
      {
        "title": "Używanie kredytów",
        "copy": "Płatne kredyty trafiają na wybrane konto gry i można ich używać tylko w Numeron. Salda nie można przekazywać innym graczom, wypłacać ani wymieniać na waluty lub kryptoaktywa. Doładowanie nie aktywuje karnetu miesięcznego. W oficjalnym sklepie można wydać własne kredyty, aby podarować karnet wybranej postaci."
      },
      {
        "title": "Płatności i portfele",
        "copy": "Logowanie portfelem, doładowanie i handel USDC przez podmioty trzecie to odrębne procesy. Handel USDC zapewniają odpowiedni partnerzy. Dostępne będą metody faktycznie włączone w oficjalnej kasie."
      },
      {
        "title": "Zamówienia i dostarczenie",
        "copy": "Przy oficjalnym zakupie sprawdź konto, postać, serwer, kwotę i liczbę kredytów. Wynik płatności wymaga potwierdzenia serwera. Obecnie witryna nie przyjmuje płatności."
      }
    ]
  },
  "tr": {
    "label": "Oyun kredisi yükleme",
    "aiLabel": "YZ abonelikleri (planlanan)",
    "status": "Hazırlanıyor · Satın alma kapalı",
    "intro": "Bu sayfa onaylanan Numeron kredi paketini gösterir. Oyun içi aylık kartlar ve planlanan YZ abonelikleri ayrı açıklanır. Satın alma şu anda kapalıdır.",
    "pricingTitle": "Yükleme paketleri ve fiyatlar",
    "pricingNote": "Yalnızca aşağıdaki paket kesinleşmiştir. Diğer paketler, vergiler, bölgeler, geçerlilik, test sıfırlamaları, teslim süreleri ve iade koşulları henüz belirlenmedi. Test verileri başka resmî fiyatları belirlemez.",
    "columns": [
      "Ürün",
      "Ödeme tutarı",
      "Yüklenen kredi",
      "Durum"
    ],
    "pending": "Onay bekliyor",
    "sections": [
      {
        "title": "Kredilerin kullanımı",
        "copy": "Ücretli krediler seçilen oyun hesabına yüklenir ve yalnızca Numeron içinde kullanılır. Kredi bakiyesi oyuncular arasında aktarılamaz, nakit olarak çekilemez ve itibari paraya veya kripto varlığa çevrilemez. Yükleme aylık kartı etkinleştirmez. Oyuncular resmî mağazada kendi kredileriyle seçtikleri bir karaktere aylık kart hediye edebilir."
      },
      {
        "title": "Ödemeler ve cüzdanlar",
        "copy": "Cüzdanla giriş, kredi yükleme ve cüzdandaki üçüncü taraf USDC alım satımı ayrı süreçlerdir. USDC işlemlerini ilgili ortaklar sağlar. Kullanılabilir ödeme yöntemleri resmî kasada gerçekten etkinleştirilenlerle sınırlıdır."
      },
      {
        "title": "Sipariş ve teslimat",
        "copy": "Resmî satın alımda hesabı, karakteri, sunucuyu, tutarı ve kredi miktarını kontrol edin. Ödeme sonucunu sunucu doğrulamalıdır. Bu site şu anda ödeme kabul etmiyor."
      }
    ]
  },
  "th": {
    "label": "เติมเครดิตเกม",
    "aiLabel": "สมาชิก AI (อยู่ในแผน)",
    "status": "อยู่ระหว่างเตรียมการ · ยังไม่เปิดซื้อ",
    "intro": "หน้านี้แสดงแพ็กเกจเครดิต Numeron ที่ยืนยันแล้ว บัตรรายเดือนในเกมและสมาชิก AI ที่วางแผนไว้มีคำอธิบายแยกกัน ขณะนี้ยังไม่เปิดซื้อ",
    "pricingTitle": "แพ็กเกจและราคาเติมเครดิต",
    "pricingNote": "ยืนยันเฉพาะแพ็กเกจด้านล่าง แพ็กเกจอื่น ภาษี ภูมิภาค อายุใช้งาน การล้างข้อมูลทดสอบ เวลาส่งมอบและเงื่อนไขคืนเงินยังรอยืนยัน ข้อมูลทดสอบไม่กำหนดราคาจริงอื่น",
    "columns": [
      "สินค้า",
      "จำนวนเงิน",
      "เครดิตที่ได้รับ",
      "สถานะ"
    ],
    "pending": "รอยืนยัน",
    "sections": [
      {
        "title": "การใช้เครดิต",
        "copy": "เครดิตที่ซื้อจะเข้าบัญชีเกมที่เลือกและใช้ได้เฉพาะใน Numeron ไม่สามารถโอนยอดเครดิตระหว่างผู้เล่น ถอนเงิน หรือแลกเป็นเงินตราหรือสินทรัพย์ดิจิทัลได้ การเติมเครดิตไม่เปิดใช้บัตรรายเดือน ผู้เล่นใช้เครดิตของตนในร้านค้าทางการเพื่อส่งบัตรรายเดือนเป็นของขวัญให้ตัวละครที่เลือกได้"
      },
      {
        "title": "การชำระเงินและกระเป๋า",
        "copy": "การเข้าสู่ระบบด้วยกระเป๋า การเติมเครดิต และการซื้อขาย USDC โดยบุคคลที่สามเป็นคนละขั้นตอน คู่ค้าที่เกี่ยวข้องเป็นผู้ให้บริการซื้อขาย USDC วิธีชำระเงินจะเป็นวิธีที่เปิดใช้จริงในหน้าชำระเงินอย่างเป็นทางการ"
      },
      {
        "title": "คำสั่งซื้อและการส่งมอบ",
        "copy": "เมื่อซื้อจริง ให้ตรวจสอบบัญชี ตัวละคร เซิร์ฟเวอร์ จำนวนเงินและเครดิต ผลการชำระเงินต้องผ่านการยืนยันจากเซิร์ฟเวอร์ ขณะนี้เว็บไซต์ไม่รับชำระเงิน"
      }
    ]
  },
  "id": {
    "label": "Isi ulang kredit game",
    "aiLabel": "Langganan AI (direncanakan)",
    "status": "Dalam persiapan · Pembelian belum dibuka",
    "intro": "Halaman ini menampilkan paket kredit Numeron yang telah dikonfirmasi. Kartu bulanan dalam game dan rencana langganan AI dijelaskan terpisah. Pembelian belum dibuka.",
    "pricingTitle": "Paket dan harga isi ulang",
    "pricingNote": "Hanya paket berikut yang dikonfirmasi. Paket lain, pajak, wilayah, masa berlaku, reset uji, waktu pengiriman dan ketentuan pengembalian dana belum ditentukan. Data uji tidak menetapkan harga resmi lain.",
    "columns": [
      "Produk",
      "Jumlah pembayaran",
      "Kredit diterima",
      "Status"
    ],
    "pending": "Menunggu konfirmasi",
    "sections": [
      {
        "title": "Penggunaan kredit",
        "copy": "Kredit berbayar masuk ke akun gim yang dipilih dan hanya dapat digunakan di Numeron. Saldo tidak dapat dipindahkan antar pemain, dicairkan, atau ditukar dengan uang fiat maupun aset kripto. Isi ulang tidak mengaktifkan kartu bulanan. Di toko resmi, pemain dapat memakai kredit sendiri untuk menghadiahkan kartu bulanan kepada karakter yang dipilih."
      },
      {
        "title": "Pembayaran dan dompet",
        "copy": "Login dompet, isi ulang kredit, dan jual beli USDC oleh pihak ketiga adalah proses terpisah. Jual beli USDC disediakan mitra terkait. Metode pembayaran mengikuti yang benar-benar diaktifkan pada checkout resmi."
      },
      {
        "title": "Pesanan dan pengiriman",
        "copy": "Saat membeli secara resmi, periksa akun, karakter, server, nominal dan jumlah kredit. Hasil pembayaran perlu dikonfirmasi server. Situs ini saat ini tidak menerima pembayaran."
      }
    ]
  },
  "ar": {
    "label": "شحن رصيد اللعبة",
    "aiLabel": "اشتراكات الذكاء الاصطناعي (مخططة)",
    "status": "قيد الإعداد · الشراء غير متاح",
    "intro": "تعرض هذه الصفحة باقة رصيد Numeron المؤكدة. تُشرح بطاقات اللعبة الشهرية واشتراكات الذكاء الاصطناعي المخططة بشكل منفصل. الشراء غير متاح حاليًا.",
    "pricingTitle": "باقات الشحن والأسعار",
    "pricingNote": "الباقة التالية فقط مؤكدة. الباقات الأخرى والضرائب والمناطق والصلاحية وإعادة ضبط الاختبار ومدة التسليم وشروط الاسترداد لم تتأكد بعد. لا تحدد بيانات الاختبار أسعارًا رسمية أخرى.",
    "columns": [
      "المنتج",
      "مبلغ الدفع",
      "الرصيد المستلم",
      "الحالة"
    ],
    "pending": "بانتظار التأكيد",
    "sections": [
      {
        "title": "استخدام الرصيد",
        "copy": "يُضاف الرصيد المدفوع إلى حساب اللعبة المحدد ويُستخدم داخل Numeron فقط. لا يمكن تحويل الرصيد بين اللاعبين أو سحبه أو استبداله بعملة ورقية أو أصول مشفرة. لا يفعّل الشحن البطاقة الشهرية. يمكن للاعبين استخدام رصيدهم في المتجر الرسمي لإهداء بطاقة شهرية إلى شخصية يحددونها."
      },
      {
        "title": "الدفع والمحافظ",
        "copy": "تسجيل الدخول بالمحفظة والشحن وتداول USDC عبر جهات خارجية عمليات منفصلة. يقدم الشركاء المعنيون تداول USDC. تتحدد وسائل الدفع بما يُفعّل فعليًا في صفحة الدفع الرسمية."
      },
      {
        "title": "الطلبات والتسليم",
        "copy": "عند الشراء الرسمي، تحقق من الحساب والشخصية والخادم والمبلغ وكمية الرصيد. تتطلب نتيجة الدفع تأكيد الخادم. لا يقبل هذا الموقع مدفوعات حاليًا."
      }
    ]
  }
} satisfies Record<Locale, RechargeCopy>;

const billingLabels: Record<Locale, string> = {
  "zh-CN": "单次充值 · 不自动续费", "zh-TW": "單次儲值 · 不自動續訂", en: "One-time top-up · No automatic renewal", ja: "都度チャージ · 自動更新なし", ko: "일회성 충전 · 자동 갱신 없음",
  es: "Recarga puntual · Sin renovación automática", "pt-BR": "Recarga avulsa · Sem renovação automática", de: "Einmalige Aufladung · Keine automatische Verlängerung", fr: "Recharge ponctuelle · Sans renouvellement automatique",
  ru: "Разовое пополнение · Без автопродления", pl: "Jednorazowe doładowanie · Bez automatycznego odnawiania", tr: "Tek seferlik yükleme · Otomatik yenileme yok", th: "เติมครั้งเดียว · ไม่ต่ออายุอัตโนมัติ", vi: "Nạp một lần · Không tự gia hạn", id: "Isi ulang sekali bayar · Tanpa perpanjangan otomatis", ar: "شحن لمرة واحدة · دون تجديد تلقائي",
};

export function getRechargeCopy(locale: Locale): RechargeCopy & { billing: string } {
  return { ...rechargeCopies[locale], pricingNote: getOperationsCopy(locale).pricingNote, billing: billingLabels[locale] };
}
