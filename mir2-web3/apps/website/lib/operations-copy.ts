import type { Locale } from "@/lib/site-copy";

export const operationsUpdatedAt = "2026-11-07";
export const commissionedDeveloperLegalName = "Obelisk Labs, Inc.";
export const refundWindowDays = 14;
export const operationSectionIds = ["refunds", "wallet", "privacy", "assets"] as const;
type OperationSection = (typeof operationSectionIds)[number];
type OperationsCopy = {
  title: string;
  operator: string;
  notice: string;
  pricingNote: string;
  sections: Record<OperationSection, { title: string; copy: string }>;
};

// Localized business summaries, not reviewed translations of the legal documents.
// Publishing these pre-sale rules does not enable payments or automated refunds.
const operationsCopies = {
  "zh-CN": {
    title: "服务方与政策摘要", operator: `Numeron 由个人运营，${commissionedDeveloperLegalName} 受托开发。`,
    notice: "当前未开放购买。以下为业务与预定销售规则摘要；完整中文政策草案和各语言法律文本仍须审校。",
    pricingNote: "目前仅确认下列档位。退款安排见本页和充值与退款说明；其他档位、税费、地区、有效期、清档与交付细节将在开放购买前确认。",
    sections: {
      refunds: { title: "充值与退款", copy: `公开收费后，充值交付起 ${refundWindowDays} 个自然日内整笔付费点券未消费，可申请原支付渠道整单全额退款。已消费订单按问题情况、渠道规则和法定权益审核；法定权利不受该窗口限制。` },
      wallet: { title: "钱包与第三方兑换", copy: "钱包 USDC 买卖通过 Halliday 聚合入口接入。实际兑换由所选第三方服务商按其条款、费用、身份核验和地区条件提供；钱包登录与游戏点券充值分别处理。" },
      privacy: { title: "数据处理", copy: "游戏账号、角色、订单、余额及消费记录用于服务交付和争议处理。完整服务商、处理地区、保存期限和数据权利请求流程仍在核对，不承诺尚未验收的自动删除或导出功能。" },
      assets: { title: "素材替换进度", copy: "现有过渡素材正逐步替换为自有素材。最终素材、代码及其商业发行依据分别核对；目前尚未宣称全部替换或权属审核已完成。" },
    },
  },
  en: {
    title: "Service and policy overview", operator: `An individual operates Numeron. ${commissionedDeveloperLegalName} develops the game under contract.`,
    notice: "Purchases remain unavailable. This summarizes the service and planned sales rules; the Chinese policy draft and legal translations still require review.",
    pricingNote: "Only the package below is confirmed. See this page and the refund information for refund arrangements. Other packages, taxes, regions, validity, resets and delivery details will be confirmed before purchases open.",
    sections: {
      refunds: { title: "Top-ups and refunds", copy: `Once paid sales open, an entirely unused top-up may be requested for a full refund to the original payment method within ${refundWindowDays} calendar days of delivery. Used credits require review of the issue, channel rules and applicable rights; statutory rights are not limited by this window.` },
      wallet: { title: "Wallet and third-party exchange", copy: "Wallet USDC buying and selling connects through the Halliday aggregation interface. The selected third party provides exchange services under its terms, fees, identity checks and regional conditions. Wallet login and game credit top-ups are separate processes." },
      privacy: { title: "Data handling", copy: "Game account, character, order, balance and spending records support service delivery and dispute handling. The full provider list, processing locations, retention and rights-request procedures remain under verification. Unverified automatic deletion or export tools are not promised." },
      assets: { title: "Asset replacement", copy: "Temporary assets are being replaced with our own assets. Final asset and code rights and commercial distribution permissions are checked separately. Replacement and rights review have not been declared complete." },
    },
  },
  "zh-TW": {
    title: "服務方與政策摘要", operator: `Numeron 由個人營運，${commissionedDeveloperLegalName} 受託開發。`,
    notice: "目前未開放購買。以下為業務與預定銷售規則摘要；完整中文政策草案及各語言法律文本仍須審校。",
    pricingNote: "目前僅確認下列方案。退款安排請見本頁與儲值退款說明；其他方案、稅費、地區、有效期、清檔與交付細節將於開放購買前確認。",
    sections: {
      refunds: { title: "儲值與退款", copy: `公開收費後，儲值交付起 ${refundWindowDays} 個日曆日內整筆付費點數未使用，可申請經原付款方式整筆全額退款。已使用訂單依問題、渠道規則與法定權益審核；法定權利不受此期限限制。` },
      wallet: { title: "錢包與第三方兌換", copy: "錢包 USDC 買賣透過 Halliday 聚合入口接入。實際兌換由所選第三方服務商依其條款、費用、身分核驗與地區條件提供；錢包登入與遊戲點數儲值分別處理。" },
      privacy: { title: "資料處理", copy: "遊戲帳號、角色、訂單、餘額與消費紀錄用於服務交付和爭議處理。完整服務商、處理地區、保存期限與資料權利申請流程仍在核對，尚未驗收的自動刪除或匯出功能不作承諾。" },
      assets: { title: "素材替換進度", copy: "現有過渡素材正逐步替換為自有素材。最終素材、程式碼與商業發行依據分別核對；目前尚未宣稱替換與權屬審核已全部完成。" },
    },
  },
  ja: {
    title: "サービスと方針の概要", operator: `Numeron は個人が運営し、${commissionedDeveloperLegalName} が委託を受けて開発しています。`,
    notice: "購入はまだ利用できません。事業と販売予定ルールの概要です。中国語の方針草案と各言語の法的文書は審査が必要です。",
    pricingNote: "確定しているのは下記プランのみです。返金は本ページと返金案内をご覧ください。他のプラン、税、地域、有効期限、リセット、提供条件は購入開始前に確定します。",
    sections: {
      refunds: { title: "チャージと返金", copy: `販売開始後、付与から ${refundWindowDays} 暦日以内で購入分が全額未使用の場合、元の支払方法による全額返金を申請できます。使用済み分は問題の内容、決済規則、法的権利に基づき審査します。法定の権利はこの期間に制限されません。` },
      wallet: { title: "ウォレットと第三者交換", copy: "ウォレットの USDC 売買は Halliday の集約インターフェースを通じて接続します。実際の交換は選択した第三者が独自の規約、手数料、本人確認、地域条件に従って提供します。ウォレット認証とゲーム内チャージは別の手続きです。" },
      privacy: { title: "データ処理", copy: "アカウント、キャラクター、注文、残高、消費記録をサービス提供と紛争対応に使用します。全サービス提供者、処理地域、保存期間、権利申請手続きは確認中です。未検証の自動削除や出力機能は約束しません。" },
      assets: { title: "素材の差し替え", copy: "仮素材を自社素材へ順次差し替えています。最終的な素材・コードの権利と商用配布の根拠を別々に確認します。差し替えや権利審査が完了したとはしていません。" },
    },
  },
  ko: {
    title: "서비스 및 정책 개요", operator: `Numeron은 개인이 운영하며 ${commissionedDeveloperLegalName}는 위탁을 받아 게임을 개발합니다.`,
    notice: "아직 구매할 수 없습니다. 서비스와 예정된 판매 규칙의 요약이며 중국어 정책 초안과 각 언어의 법률 문서는 검토가 필요합니다.",
    pricingNote: "아래 상품만 확정되었습니다. 환불은 이 페이지와 환불 안내를 확인하세요. 다른 상품, 세금, 지역, 유효기간, 초기화와 제공 조건은 구매 개시 전에 확정합니다.",
    sections: {
      refunds: { title: "충전 및 환불", copy: `유료 판매가 시작되면 지급일부터 ${refundWindowDays}일 이내에 충전분 전체가 미사용인 경우 원래 결제 수단으로 전액 환불을 신청할 수 있습니다. 사용된 충전은 문제 내용, 결제사 규칙과 법적 권리에 따라 검토하며 법정 권리는 이 기간으로 제한되지 않습니다.` },
      wallet: { title: "지갑 및 제3자 교환", copy: "지갑의 USDC 매매는 Halliday 통합 인터페이스를 통해 연결됩니다. 실제 교환은 선택한 제3자 업체가 자체 약관, 수수료, 신원 확인과 지역 조건에 따라 제공합니다. 지갑 로그인과 게임 크레딧 충전은 별개입니다." },
      privacy: { title: "데이터 처리", copy: "계정, 캐릭터, 주문, 잔액과 사용 기록은 서비스 제공 및 분쟁 처리에 사용됩니다. 전체 업체 목록, 처리 지역, 보관 기간과 권리 요청 절차는 확인 중이며 검증하지 않은 자동 삭제나 내보내기 기능을 약속하지 않습니다." },
      assets: { title: "소재 교체", copy: "임시 소재를 자체 소재로 순차 교체하고 있습니다. 최종 소재와 코드의 권리 및 상업적 배포 근거를 각각 확인합니다. 교체와 권리 검토가 모두 끝났다고 발표하지 않았습니다." },
    },
  },
  es: {
    title: "Servicio y políticas", operator: `Numeron lo opera una persona física. ${commissionedDeveloperLegalName} desarrolla el juego por encargo.`,
    notice: "Las compras siguen sin estar disponibles. Este resumen describe el servicio y las reglas de venta previstas; el borrador chino y las traducciones jurídicas requieren revisión.",
    pricingNote: "Solo está confirmado el paquete indicado. Consulte aquí y en la información de reembolsos las condiciones previstas. Otros paquetes, impuestos, regiones, vigencia, reinicios y entrega se confirmarán antes de abrir las compras.",
    sections: {
      refunds: { title: "Recargas y reembolsos", copy: `Cuando se abra la venta, podrá solicitar el reembolso íntegro de una recarga totalmente sin usar al medio de pago original durante los ${refundWindowDays} días naturales desde su entrega. Las recargas usadas se revisarán según el problema, las reglas del canal y los derechos aplicables. Este plazo no limita los derechos legales.` },
      wallet: { title: "Cartera y cambio de terceros", copy: "La compra y venta de USDC en la cartera se conecta mediante la interfaz agregadora de Halliday. El tercero elegido presta el cambio según sus términos, tarifas, verificación y condiciones regionales. El acceso con cartera y las recargas del juego son procesos separados." },
      privacy: { title: "Tratamiento de datos", copy: "Los registros de cuenta, personaje, pedido, saldo y consumo permiten prestar el servicio y resolver disputas. Se están comprobando los proveedores, ubicaciones, conservación y solicitudes de derechos. No se prometen herramientas de borrado o exportación automática sin verificar." },
      assets: { title: "Sustitución de recursos", copy: "Los recursos temporales se están sustituyendo por recursos propios. Se comprueban por separado los derechos sobre recursos y código y los permisos de distribución comercial. No se ha declarado finalizada la sustitución ni la revisión de derechos." },
    },
  },
  "pt-BR": {
    title: "Serviço e políticas", operator: `Numeron é operado por uma pessoa física. A ${commissionedDeveloperLegalName} desenvolve o jogo sob contrato.`,
    notice: "As compras continuam indisponíveis. Este é um resumo do serviço e das regras de venda previstas; o rascunho em chinês e as traduções jurídicas ainda precisam de revisão.",
    pricingNote: "Apenas o pacote abaixo está confirmado. Consulte esta página e as informações de reembolso para as regras previstas. Outros pacotes, tributos, regiões, validade, reinicializações e entrega serão confirmados antes das compras.",
    sections: {
      refunds: { title: "Recargas e reembolsos", copy: `Quando as vendas forem abertas, será possível solicitar reembolso integral, pelo meio de pagamento original, de uma recarga totalmente não utilizada em até ${refundWindowDays} dias corridos da entrega. Recargas usadas serão analisadas conforme o problema, as regras do canal e os direitos aplicáveis. Esse prazo não limita direitos legais.` },
      wallet: { title: "Carteira e câmbio de terceiros", copy: "A compra e venda de USDC na carteira conecta-se pela interface agregadora da Halliday. A prestadora terceira escolhida executa o câmbio conforme seus termos, tarifas, verificação de identidade e condições regionais. Login na carteira e recarga do jogo são processos separados." },
      privacy: { title: "Tratamento de dados", copy: "Registros de conta, personagem, pedido, saldo e consumo apoiam a prestação do serviço e a resolução de disputas. A lista de prestadores, os locais, a retenção e os pedidos de direitos estão em verificação. Não são prometidas ferramentas automáticas de exclusão ou exportação não validadas." },
      assets: { title: "Substituição de materiais", copy: "Materiais temporários estão sendo substituídos por materiais próprios. Os direitos sobre materiais e código e as permissões de distribuição comercial são verificados separadamente. A substituição e a revisão de direitos ainda não foram declaradas concluídas." },
    },
  },
  de: {
    title: "Dienst und Richtlinien", operator: `Eine Einzelperson betreibt Numeron. ${commissionedDeveloperLegalName} entwickelt das Spiel im Auftrag.`,
    notice: "Käufe sind weiterhin nicht verfügbar. Dies fasst den Dienst und die geplanten Verkaufsregeln zusammen. Der chinesische Entwurf und rechtliche Übersetzungen müssen noch geprüft werden.",
    pricingNote: "Nur das folgende Paket ist bestätigt. Erstattungsregeln stehen hier und in den Erstattungshinweisen. Weitere Pakete, Steuern, Regionen, Gültigkeit, Zurücksetzungen und Bereitstellung werden vor dem Kaufstart bestätigt.",
    sections: {
      refunds: { title: "Aufladung und Erstattung", copy: `Nach Verkaufsstart kann für eine vollständig ungenutzte Aufladung innerhalb von ${refundWindowDays} Kalendertagen ab Bereitstellung eine volle Erstattung auf das ursprüngliche Zahlungsmittel beantragt werden. Genutzte Aufladungen werden nach Problem, Kanalregeln und geltenden Rechten geprüft. Gesetzliche Rechte werden durch diese Frist nicht begrenzt.` },
      wallet: { title: "Wallet und externer Umtausch", copy: "USDC-Käufe und -Verkäufe im Wallet werden über die Aggregationsoberfläche von Halliday angebunden. Der gewählte Drittanbieter führt den Umtausch nach seinen Bedingungen, Gebühren, Identitätsprüfungen und regionalen Vorgaben durch. Wallet-Anmeldung und Spielguthaben sind getrennte Abläufe." },
      privacy: { title: "Datenverarbeitung", copy: "Konto-, Charakter-, Bestell-, Guthaben- und Verbrauchsdaten dienen der Leistung und Streitklärung. Anbieter, Verarbeitungsorte, Speicherfristen und Rechteverfahren werden noch geprüft. Nicht validierte automatische Lösch- oder Exportfunktionen werden nicht zugesagt." },
      assets: { title: "Austausch der Materialien", copy: "Vorläufige Materialien werden schrittweise durch eigene ersetzt. Rechte an Materialien und Code sowie kommerzielle Vertriebsrechte werden getrennt geprüft. Austausch und Rechteprüfung sind noch nicht als abgeschlossen erklärt." },
    },
  },
  fr: {
    title: "Service et politiques", operator: `Une personne physique exploite Numeron. ${commissionedDeveloperLegalName} développe le jeu sous contrat.`,
    notice: "Les achats restent indisponibles. Ce résumé présente le service et les règles de vente prévues ; le projet chinois et les traductions juridiques doivent encore être examinés.",
    pricingNote: "Seule l'offre ci-dessous est confirmée. Consultez cette page et les informations de remboursement. Les autres offres, taxes, régions, durées, réinitialisations et modalités de livraison seront confirmées avant les achats.",
    sections: {
      refunds: { title: "Recharges et remboursements", copy: `À l'ouverture des ventes, une recharge entièrement inutilisée pourra faire l'objet d'une demande de remboursement intégral sur le moyen de paiement d'origine dans les ${refundWindowDays} jours calendaires suivant sa livraison. Les recharges utilisées seront examinées selon le problème, les règles du canal et les droits applicables. Ce délai ne limite pas les droits légaux.` },
      wallet: { title: "Portefeuille et échange tiers", copy: "L'achat et la vente d'USDC dans le portefeuille passent par l'interface d'agrégation Halliday. Le prestataire tiers choisi réalise l'échange selon ses conditions, frais, contrôles d'identité et conditions régionales. La connexion au portefeuille et la recharge du jeu sont distinctes." },
      privacy: { title: "Traitement des données", copy: "Les données de compte, personnage, commande, solde et consommation servent à fournir le service et traiter les litiges. Prestataires, lieux, conservation et demandes de droits sont en cours de vérification. Aucune suppression ou exportation automatique non validée n'est promise." },
      assets: { title: "Remplacement des ressources", copy: "Les ressources temporaires sont progressivement remplacées par nos propres ressources. Les droits sur les ressources, le code et leur distribution commerciale sont vérifiés séparément. Le remplacement et l'examen des droits ne sont pas déclarés terminés." },
    },
  },
  ru: {
    title: "Сервис и правила", operator: `Игрой Numeron управляет частное лицо. ${commissionedDeveloperLegalName} разрабатывает игру по договору.`,
    notice: "Покупки пока недоступны. Это описание сервиса и планируемых правил продаж. Китайский проект и юридические переводы ещё требуют проверки.",
    pricingNote: "Подтверждён только указанный пакет. Правила возврата приведены здесь и в разделе о возвратах. Другие пакеты, налоги, регионы, сроки, сбросы и доставка будут уточнены до начала покупок.",
    sections: {
      refunds: { title: "Пополнение и возврат", copy: `После начала продаж можно запросить полный возврат полностью неиспользованного пополнения исходным способом оплаты в течение ${refundWindowDays} календарных дней с момента начисления. Использованные пополнения рассматриваются с учётом проблемы, правил канала и применимых прав. Этот срок не ограничивает законные права.` },
      wallet: { title: "Кошелёк и сторонний обмен", copy: "Покупка и продажа USDC в кошельке подключаются через агрегатор Halliday. Обмен выполняет выбранный сторонний поставщик по своим условиям, тарифам, проверкам личности и региональным правилам. Вход через кошелёк и пополнение игры — отдельные процессы." },
      privacy: { title: "Обработка данных", copy: "Данные аккаунта, персонажа, заказа, баланса и расходования используются для оказания услуг и разрешения споров. Поставщики, места обработки, сроки хранения и процедуры запросов ещё проверяются. Непроверенные функции автоматического удаления и экспорта не обещаются." },
      assets: { title: "Замена материалов", copy: "Временные материалы постепенно заменяются собственными. Права на материалы и код, а также коммерческое распространение проверяются отдельно. Завершение замены и проверки прав пока не заявлено." },
    },
  },
  pl: {
    title: "Usługa i zasady", operator: `Numeron prowadzi osoba fizyczna. ${commissionedDeveloperLegalName} tworzy grę na zlecenie.`,
    notice: "Zakupy pozostają niedostępne. To podsumowanie usługi i planowanych zasad sprzedaży. Chiński projekt i tłumaczenia prawne wymagają jeszcze przeglądu.",
    pricingNote: "Potwierdzony jest tylko poniższy pakiet. Zasady zwrotów opisano tutaj i w informacji o zwrotach. Inne pakiety, podatki, regiony, ważność, resety i dostawa zostaną ustalone przed zakupami.",
    sections: {
      refunds: { title: "Doładowania i zwroty", copy: `Po otwarciu sprzedaży można wnioskować o pełny zwrot całkowicie niewykorzystanego doładowania pierwotną metodą płatności w ciągu ${refundWindowDays} dni kalendarzowych od dostarczenia. Wykorzystane środki są oceniane według problemu, zasad kanału i obowiązujących praw. Termin ten nie ogranicza praw ustawowych.` },
      wallet: { title: "Portfel i wymiana zewnętrzna", copy: "Kupno i sprzedaż USDC w portfelu łączy się przez interfejs agregujący Halliday. Wybrany podmiot zewnętrzny świadczy wymianę według swoich warunków, opłat, weryfikacji tożsamości i zasad regionalnych. Logowanie portfelem i doładowania gry są odrębne." },
      privacy: { title: "Przetwarzanie danych", copy: "Dane konta, postaci, zamówień, salda i zużycia służą świadczeniu usługi i rozstrzyganiu sporów. Dostawcy, lokalizacje, retencja i procedury żądań są weryfikowane. Nie obiecujemy niezweryfikowanych automatycznych narzędzi usuwania ani eksportu." },
      assets: { title: "Wymiana materiałów", copy: "Materiały tymczasowe są stopniowo zastępowane własnymi. Prawa do materiałów i kodu oraz do dystrybucji komercyjnej sprawdzamy oddzielnie. Nie ogłoszono zakończenia wymiany ani przeglądu praw." },
    },
  },
  tr: {
    title: "Hizmet ve politika özeti", operator: `Numeron'u bir kişi işletir. ${commissionedDeveloperLegalName} oyunu sözleşme kapsamında geliştirir.`,
    notice: "Satın alma henüz açık değildir. Bu, hizmetin ve planlanan satış kurallarının özetidir. Çince taslak ve hukuki çeviriler hâlâ inceleme gerektirir.",
    pricingNote: "Yalnızca aşağıdaki paket onaylanmıştır. İade düzenlemeleri bu sayfada ve iade bilgilerindedir. Diğer paketler, vergiler, bölgeler, geçerlilik, sıfırlama ve teslim ayrıntıları satın alma açılmadan netleşecektir.",
    sections: {
      refunds: { title: "Yükleme ve iadeler", copy: `Ücretli satışlar açıldığında, tamamen kullanılmamış bir yükleme için teslimden itibaren ${refundWindowDays} takvim günü içinde ilk ödeme yöntemiyle tam iade istenebilir. Kullanılmış yüklemeler soruna, kanal kurallarına ve geçerli haklara göre incelenir. Yasal haklar bu süreyle sınırlanmaz.` },
      wallet: { title: "Cüzdan ve üçüncü taraf dönüşümü", copy: "Cüzdandaki USDC alım satımı Halliday toplama arayüzü üzerinden bağlanır. Gerçek dönüşümü seçilen üçüncü taraf kendi şartları, ücretleri, kimlik doğrulaması ve bölgesel koşullarıyla sağlar. Cüzdan girişi ve oyun kredisi yükleme ayrı işlemlerdir." },
      privacy: { title: "Veri işleme", copy: "Hesap, karakter, sipariş, bakiye ve harcama kayıtları hizmet ve uyuşmazlık işlemleri için kullanılır. Sağlayıcılar, işleme yerleri, saklama ve hak talebi süreçleri doğrulanmaktadır. Doğrulanmamış otomatik silme veya dışa aktarma araçları vaat edilmez." },
      assets: { title: "Materyal değişimi", copy: "Geçici materyaller aşamalı olarak kendi materyallerimizle değiştiriliyor. Materyal, kod ve ticari dağıtım hakları ayrı incelenir. Değişimin ve hak incelemesinin tamamlandığı ilan edilmemiştir." },
    },
  },
  th: {
    title: "ภาพรวมบริการและนโยบาย", operator: `Numeron ดำเนินงานโดยบุคคลธรรมดา ส่วน ${commissionedDeveloperLegalName} เป็นผู้รับจ้างพัฒนาเกม`,
    notice: "ยังไม่เปิดให้ซื้อ ข้อมูลนี้สรุปบริการและกฎการขายที่วางแผนไว้ ร่างภาษาจีนและคำแปลทางกฎหมายยังต้องตรวจทาน",
    pricingNote: "ยืนยันเฉพาะแพ็กเกจด้านล่าง โปรดดูการคืนเงินในหน้านี้และคำอธิบายการคืนเงิน แพ็กเกจอื่น ภาษี ภูมิภาค อายุการใช้งาน การรีเซ็ตและการส่งมอบจะยืนยันก่อนเปิดซื้อ",
    sections: {
      refunds: { title: "การเติมและคืนเงิน", copy: `เมื่อเปิดขาย สามารถขอคืนเงินเต็มจำนวนผ่านวิธีชำระเดิมสำหรับรายการเติมที่ยังไม่ได้ใช้ทั้งหมดภายใน ${refundWindowDays} วันปฏิทินนับจากส่งมอบ รายการที่ใช้แล้วจะตรวจตามปัญหา กฎช่องทางและสิทธิที่ใช้บังคับ ระยะเวลานี้ไม่จำกัดสิทธิตามกฎหมาย` },
      wallet: { title: "กระเป๋าและการแลกเปลี่ยนภายนอก", copy: "การซื้อขาย USDC ในกระเป๋าเชื่อมผ่านตัวกลาง Halliday ผู้ให้บริการภายนอกที่เลือกเป็นผู้แลกเปลี่ยนตามข้อกำหนด ค่าธรรมเนียม การยืนยันตัวตนและเงื่อนไขภูมิภาคของตน การเข้าสู่ระบบกระเป๋าและการเติมเครดิตเกมเป็นคนละกระบวนการ" },
      privacy: { title: "การประมวลผลข้อมูล", copy: "ข้อมูลบัญชี ตัวละคร คำสั่งซื้อ ยอดคงเหลือและการใช้เครดิตสนับสนุนบริการและการแก้ข้อพิพาท รายชื่อผู้ให้บริการ ที่ประมวลผล ระยะเก็บและคำขอใช้สิทธิยังอยู่ระหว่างตรวจสอบ ไม่รับรองระบบลบหรือส่งออกอัตโนมัติที่ยังไม่ผ่านการตรวจ" },
      assets: { title: "การเปลี่ยนสื่อ", copy: "กำลังทยอยเปลี่ยนสื่อชั่วคราวเป็นสื่อของเรา ตรวจสิทธิของสื่อ โค้ดและการเผยแพร่เชิงพาณิชย์แยกกัน ยังไม่ได้ประกาศว่าเปลี่ยนสื่อหรือตรวจสิทธิเสร็จทั้งหมด" },
    },
  },
  vi: {
    title: "Tổng quan dịch vụ và chính sách", operator: `Numeron do một cá nhân vận hành. ${commissionedDeveloperLegalName} phát triển trò chơi theo hợp đồng.`,
    notice: "Chưa mở mua hàng. Đây là bản tóm tắt dịch vụ và quy tắc bán hàng dự kiến; bản nháp tiếng Trung và các bản dịch pháp lý vẫn cần thẩm định.",
    pricingNote: "Chỉ gói dưới đây đã được xác nhận. Xem quy tắc hoàn tiền tại đây và trang hoàn tiền. Các gói khác, thuế, khu vực, thời hạn, đặt lại dữ liệu và giao hàng sẽ được xác nhận trước khi mở mua.",
    sections: {
      refunds: { title: "Nạp điểm và hoàn tiền", copy: `Khi mở bán, người dùng có thể yêu cầu hoàn toàn bộ một giao dịch nạp chưa sử dụng bất kỳ điểm nào về phương thức thanh toán ban đầu trong ${refundWindowDays} ngày lịch từ lúc nhận điểm. Giao dịch đã dùng được xem xét theo sự cố, quy tắc kênh và quyền áp dụng. Mốc này không hạn chế quyền theo luật.` },
      wallet: { title: "Ví và đổi tiền bên thứ ba", copy: "Mua bán USDC trong ví kết nối qua giao diện tổng hợp Halliday. Nhà cung cấp bên thứ ba được chọn thực hiện việc đổi theo điều khoản, phí, xác minh danh tính và điều kiện khu vực của họ. Đăng nhập ví và nạp điểm trò chơi là các quy trình riêng." },
      privacy: { title: "Xử lý dữ liệu", copy: "Dữ liệu tài khoản, nhân vật, đơn hàng, số dư và sử dụng điểm phục vụ cung cấp dịch vụ và xử lý tranh chấp. Danh sách nhà cung cấp, nơi xử lý, thời hạn lưu và quy trình yêu cầu quyền đang được kiểm tra. Không cam kết công cụ tự động xóa hoặc xuất dữ liệu chưa được nghiệm thu." },
      assets: { title: "Thay thế tài nguyên", copy: "Tài nguyên tạm thời đang được thay dần bằng tài nguyên của chúng tôi. Quyền đối với tài nguyên, mã và phát hành thương mại được kiểm tra riêng. Chưa tuyên bố hoàn tất thay thế hoặc thẩm tra quyền." },
    },
  },
  id: {
    title: "Ringkasan layanan dan kebijakan", operator: `Numeron dioperasikan oleh perorangan. ${commissionedDeveloperLegalName} mengembangkan gim berdasarkan kontrak.`,
    notice: "Pembelian belum tersedia. Ini merangkum layanan dan rencana aturan penjualan. Draf berbahasa Mandarin dan terjemahan hukum masih perlu ditinjau.",
    pricingNote: "Hanya paket di bawah yang dikonfirmasi. Lihat pengaturan pengembalian dana di sini dan pada informasi pengembalian dana. Paket lain, pajak, wilayah, masa berlaku, reset dan pengiriman akan ditetapkan sebelum pembelian dibuka.",
    sections: {
      refunds: { title: "Isi ulang dan pengembalian dana", copy: `Saat penjualan dibuka, isi ulang yang seluruhnya belum digunakan dapat dimintakan pengembalian penuh ke metode pembayaran awal dalam ${refundWindowDays} hari kalender sejak diterima. Isi ulang terpakai ditinjau berdasarkan masalah, aturan kanal dan hak yang berlaku. Hak menurut hukum tidak dibatasi periode ini.` },
      wallet: { title: "Dompet dan penukaran pihak ketiga", copy: "Jual beli USDC di dompet terhubung melalui antarmuka agregator Halliday. Penyedia pihak ketiga yang dipilih menjalankan penukaran menurut ketentuan, biaya, verifikasi identitas dan syarat wilayahnya. Login dompet dan isi ulang game adalah proses terpisah." },
      privacy: { title: "Pemrosesan data", copy: "Data akun, karakter, pesanan, saldo dan penggunaan mendukung layanan serta penanganan sengketa. Daftar penyedia, lokasi pemrosesan, retensi dan permintaan hak masih diverifikasi. Alat penghapusan atau ekspor otomatis yang belum divalidasi tidak dijanjikan." },
      assets: { title: "Penggantian aset", copy: "Aset sementara sedang diganti secara bertahap dengan aset kami sendiri. Hak atas aset, kode dan distribusi komersial diperiksa terpisah. Penggantian dan peninjauan hak belum dinyatakan selesai." },
    },
  },
  ar: {
    title: "ملخص الخدمة والسياسات", operator: `يتولى شخص بصفته الفردية تشغيل Numeron. وتطوّر ${commissionedDeveloperLegalName} اللعبة بموجب عقد.`,
    notice: "الشراء غير متاح حاليًا. هذا ملخص للخدمة وقواعد البيع المخططة؛ وما زالت المسودة الصينية والترجمات القانونية تحتاج إلى مراجعة.",
    pricingNote: "الحزمة أدناه وحدها مؤكدة. راجع ترتيبات الاسترداد هنا وفي صفحة الاسترداد. ستؤكد الحزم الأخرى والضرائب والمناطق والصلاحية وإعادة ضبط البيانات والتسليم قبل فتح الشراء.",
    sections: {
      refunds: { title: "الشحن والاسترداد", copy: `عند بدء البيع، يمكن طلب استرداد كامل لعملية شحن لم يُستخدم منها أي رصيد عبر وسيلة الدفع الأصلية خلال ${refundWindowDays} يومًا تقويميًا من التسليم. تُراجع العمليات المستخدمة وفق المشكلة وقواعد القناة والحقوق المعمول بها. لا تقيد هذه المدة الحقوق القانونية.` },
      wallet: { title: "المحفظة والصرف لدى طرف ثالث", copy: "يتصل شراء وبيع USDC في المحفظة عبر واجهة التجميع Halliday. يقدم الطرف الثالث المختار خدمة الصرف وفق شروطه ورسومه والتحقق من الهوية والشروط الإقليمية. تسجيل الدخول بالمحفظة وشحن رصيد اللعبة عمليتان منفصلتان." },
      privacy: { title: "معالجة البيانات", copy: "تُستخدم سجلات الحساب والشخصية والطلب والرصيد والاستهلاك لتقديم الخدمة ومعالجة النزاعات. ما زالت قائمة مقدمي الخدمة ومواقع المعالجة والاحتفاظ وطلبات الحقوق قيد التحقق. لا نعد بأدوات حذف أو تصدير آلية لم تُعتمد بعد." },
      assets: { title: "استبدال المواد", copy: "يجري استبدال المواد المؤقتة تدريجيًا بموادنا الخاصة. تُراجع حقوق المواد والشيفرة والتوزيع التجاري كل على حدة. لم يُعلن اكتمال الاستبدال أو مراجعة الحقوق." },
    },
  },
} satisfies Record<Locale, OperationsCopy>;

export function getOperationsCopy(locale: Locale): OperationsCopy {
  return operationsCopies[locale];
}
