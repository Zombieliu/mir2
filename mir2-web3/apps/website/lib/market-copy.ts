import type { ProductCopy } from "@/lib/product-copy";
import type { SiteCopy } from "@/lib/site-copy";

export const additionalLocales = ["zh-TW", "es", "pt-BR", "de", "fr", "ru", "pl", "tr", "th", "vi", "id", "ar"] as const;
export type AdditionalLocale = (typeof additionalLocales)[number];
export const marketLocales = ["ja", "ko", ...additionalLocales] as const;
export type MarketLocale = (typeof marketLocales)[number];

type MarketPack = {
  name: string;
  short: string;
  dir?: "rtl";
  nav: [home: string, explore: string, world: string, classes: string, regions: string, pulse: string, live: string, highlights: string, atlas: string, menu: string, locale: string];
  hero: [top: string, bottom: string, description: string, enter: string, discover: string, online: string];
  core: [atlasTitle: string, atlasCopy: string, worldTitle: string, worldCopy: string, classesTitle: string, classesCopy: string, warrior: string, sorcerer: string, taoist: string, globalTitle: string, globalCopy: string, updatesTitle: string];
  watch: [title: string, copy: string, observatory: string, privacy: string, highlights: string, channels: string];
  member: [title: string, copy: string, monthly: string, annual: string, recommended: string, coming: string, free: string, wanderer: string, architect: string, usage: string, fairness: string, faq: string];
  labels: [roadmap: string, community: string, press: string, support: string, status: string, account: string, privacy: string, terms: string, refunds: string, ai: string, founder: string, submit: string, consent: string, requirements: string, safety: string, more: string, unavailable: string, draft: string, updated: string];
  legal: [summary: string, data: string, boundary: string, rights: string];
  footer: string;
};

export const marketPacks: Record<MarketLocale, MarketPack> = {
  ja: {
    name: "日本語", short: "JA",
    nav: ["ホーム", "探索", "世界", "職業", "地域", "ワールド動向", "ライブ", "ハイライト", "天機閣", "メニュー", "言語"],
    hero: ["Legend of", "Rebirth", "ミルの大地へ帰ろう。ひとつの進化し続ける世界で仲間と戦い、あなた自身の伝説を刻もう。", "ゲームに入る", "世界を探索", "ワールドサービス稼働中"],
    core: ["世界は止まらない。\nすべては記録される。", "公開キャラクター、ギルド、装備、市場、世界イベントを確認できます。正確な位置と非公開データは保護されます。", "インスタンスではない。\n共に生きる世界。", "街、荒野、迷宮、沙巴克が、プレイヤーによって動き続けるひとつの大陸を形作ります。", "三つの道、ひとつの伝説", "力、元素、道術は手段にすぎません。誰と共に立つかが、あなたの名を決めます。", "戦士", "魔法使い", "道士", "ひとつのミルを、\nすべての地域の物語へ。", "言語、音声、地域アートは個別に提供し、ゲームプレイと公平性は世界共通に保ちます。", "世界は動き続ける"],
    watch: ["世界は動き、\n伝説は記録される。", "AIディレクターが公開イベントから実況とハイライトを制作し、Candidateコンテンツとして明示します。", "ミル・ワールド01 · 観測所", "遅延処理とプライバシー保護を施した公開イベントのみ表示します。", "今日のハイライト", "ひとつの出来事、地域ごとの表現"],
    member: ["月間報酬パックではない。\n進化するAIサービス層。", "冒険と創作のためのアカウント連携サブスクリプションです。戦闘力を販売せず、取引資産でもありません。", "月額", "年額", "おすすめ", "近日公開", "無料", "旅人", "アーキテクト", "AI利用量を明確に表示", "道具に支払い、勝利は買わない", "登録前に確認すること"],
    labels: ["ロードマップ", "コミュニティ", "メディア", "サポート", "稼働状況", "アカウント", "プライバシー", "利用規約", "購読と返金", "AIデータ方針", "Founder Access", "申請する", "製品調査と公開通知にのみデータを使用することに同意します。", "推奨環境", "安全", "詳細", "未設定", "公開草案", "2026.08.28更新"],
    legal: ["本ページはサイト、ゲーム、配信、AIサービスのデータとサービス境界を説明します。", "選択した機能に必要、またはプレイヤーが明示的に許可したデータのみ処理します。", "正確なリアルタイム位置、非公開資産、私信、未許可の本人情報は公開しません。", "正式公開前に地域ごとの法務確認が必要です。プレイヤーは自身の原作物の権利を保持します。"], footer: "プレイヤーが共に形作る永続世界。",
  },
  ko: {
    name: "한국어", short: "KO",
    nav: ["홈", "탐색", "세계", "직업", "지역", "월드 동향", "라이브", "하이라이트", "천기각", "메뉴", "언어"],
    hero: ["Legend of", "Rebirth", "미르 대륙으로 돌아오세요. 하나의 진화하는 세계에서 실제 플레이어와 함께 싸우며 나만의 전설을 쓰세요.", "게임 입장", "세계 탐험", "월드 서비스 온라인"],
    core: ["세계는 멈추지 않습니다.\n모든 순간이 기록됩니다.", "공개 캐릭터, 길드, 장비, 시장과 월드 이벤트를 확인할 수 있으며 정확한 위치와 비공개 데이터는 보호됩니다.", "인스턴스가 아닌,\n함께 살아가는 세계.", "도시, 황야, 던전과 사북이 플레이어가 움직이는 하나의 지속 대륙을 이룹니다.", "세 개의 길, 하나의 전설", "힘과 원소, 도술은 도구일 뿐입니다. 누구와 함께하는지가 당신의 이름을 결정합니다.", "전사", "마법사", "도사", "하나의 미르,\n모든 지역의 이야기로.", "언어, 음성, 지역 아트는 별도로 제공하고 게임플레이와 공정성 규칙은 전 세계에 동일하게 적용합니다.", "세계는 계속 움직입니다"],
    watch: ["세계가 움직이고,\n전설은 기록됩니다.", "AI 연출이 공개 이벤트를 해설과 하이라이트로 만들며 Candidate 콘텐츠임을 명확히 표시합니다.", "미르 월드 01 · 관측소", "지연 및 개인정보 보호 처리된 공개 이벤트만 표시합니다.", "오늘의 하이라이트", "하나의 사건, 여러 지역 표현"],
    member: ["월간 보상 묶음이 아닌,\n진화하는 AI 서비스 계층.", "모험과 창작을 위한 계정 귀속 구독입니다. 전투력을 판매하지 않으며 거래 자산도 아닙니다.", "월간", "연간", "추천", "출시 예정", "무료", "여행자", "아키텍트", "AI 사용량을 명확하게 표시", "도구에 지불하고 승리는 사지 않습니다", "구독 전에 확인할 내용"],
    labels: ["로드맵", "커뮤니티", "미디어", "지원", "서비스 상태", "계정", "개인정보", "이용약관", "구독 및 환불", "AI 데이터 정책", "Founder Access", "신청 제출", "제품 연구와 출시 알림에만 이 데이터를 사용하는 데 동의합니다.", "권장 환경", "안전", "자세히", "설정되지 않음", "공개 초안", "2026.08.28 업데이트"],
    legal: ["이 페이지는 사이트, 게임, 방송과 AI 서비스의 데이터 및 서비스 경계를 설명합니다.", "선택한 기능에 필요하거나 플레이어가 명시적으로 허용한 데이터만 처리합니다.", "정확한 실시간 위치, 비공개 자산, 개인 대화와 허가받지 않은 신원은 공개하지 않습니다.", "정식 출시 전 지역별 법률 검토가 필요하며 플레이어는 원본 창작물의 권리를 유지합니다."], footer: "플레이어가 함께 만들어 가는 지속 세계.",
  },
  "zh-TW": {
    name: "繁體中文", short: "繁中",
    nav: ["首頁", "探索", "世界", "職業", "地區", "世界動態", "直播", "精彩時刻", "天機閣", "選單", "語言"],
    hero: ["傳奇", "重生", "重返瑪法大陸，在同一個持續演化的世界與真實玩家並肩作戰，寫下屬於你的下一段傳奇。", "進入遊戲", "探索世界", "世界服務上線"],
    core: ["世界從未停下，\n每一刻都留下紀錄。", "查看公開角色、公會、裝備、市場與世界事件；精確位置與私人資料不會公開。", "不是副本，\n而是共同生活的世界。", "城鎮、荒野、迷宮與沙巴克構成由玩家推動、持續運行的同一片大陸。", "三種道路，一段傳奇", "力量、元素與道術只是工具；與誰並肩，才決定你的名字。", "戰士", "法師", "道士", "同一個瑪法，\n為每個地區重新講述。", "語言、字體、配音與地區視覺獨立交付，核心玩法與公平規則全球一致。", "世界正在發生"],
    watch: ["世界正在發生，\n傳奇正在被記錄。", "AI 導播從公開事件製作解說與精彩片段；正式頻道接入前明確標示為候選內容。", "瑪法一號 · 世界觀測台", "僅展示延遲且經過隱私處理的公開事件。", "今日精彩時刻", "同一事件，多種地區表達"],
    member: ["不是月卡獎勵，\n而是一層持續進化的 AI 服務。", "綁定帳號的 AI 訂閱服務，不販售戰鬥數值，也不是可交易資產。", "月付", "年付", "推薦", "即將開放", "免費", "旅人", "架構師", "清楚顯示 AI 使用額度", "付費獲得工具，不是勝利", "訂閱前需要說清楚的事"],
    labels: ["路線圖", "社群", "媒體中心", "支援", "服務狀態", "帳戶", "隱私政策", "使用者協議", "訂閱與退款", "AI 資料政策", "Founder Access", "提交登記", "我同意僅將資料用於產品研究與上線通知。", "建議環境", "安全須知", "查看詳情", "尚未設定", "公開草案", "更新於 2026.08.28"],
    legal: ["本頁說明網站、遊戲、直播與 AI 服務的資料和服務邊界。", "僅處理提供功能所需或玩家明確授權的資料。", "不公開即時精確位置、私人資產、私聊或未授權身分。", "正式發行前需依各地法律完成審查；玩家保留原創內容權利。"], footer: "由玩家共同塑造的持續世界。",
  },
  es: {
    name: "Español", short: "ES",
    nav: ["Inicio", "Explorar", "Mundo", "Clases", "Regiones", "Pulso", "En directo", "Momentos", "Atlas", "Menú", "Idioma"],
    hero: ["Legend of", "Rebirth", "Regresa al continente de Mir. Lucha junto a jugadores reales en un mundo compartido y en evolución, y escribe tu propia leyenda.", "Entrar al juego", "Explorar el mundo", "Servicio del mundo en línea"],
    core: ["El mundo no se detiene.\nTodo deja un registro.", "Consulta personajes, gremios, equipo, mercados y eventos públicos; la ubicación exacta y los datos privados permanecen ocultos.", "No es una instancia.\nEs un mundo compartido.", "Ciudades, tierras salvajes, mazmorras y Sabuk forman un único continente persistente impulsado por sus jugadores.", "Tres caminos. Una leyenda.", "La fuerza, los elementos y los espíritus son herramientas; tus aliados deciden en qué se convierte tu nombre.", "Guerrero", "Hechicero", "Taoísta", "Un solo Mir,\ncontado para cada región.", "Idioma, voz y arte regional se publican por separado; la jugabilidad y las reglas de equidad siguen siendo globales.", "El mundo está cambiando"],
    watch: ["El mundo se mueve.\nLas leyendas quedan grabadas.", "La dirección asistida por IA convierte eventos públicos en comentarios y momentos destacados, siempre identificados como contenido Candidate.", "Mir Mundo 01 · Observatorio", "Solo mostramos eventos públicos con retraso y protección de privacidad.", "Momentos de hoy", "Un evento, muchas expresiones locales"],
    member: ["No es un paquete mensual.\nEs una capa viva de servicios de IA.", "Suscripción ligada a la cuenta para aventura y creación; no vende poder de combate ni es un activo negociable.", "Mensual", "Anual", "Recomendado", "Próximamente", "Gratis", "Viajero", "Arquitecto", "Uso de IA claramente medido", "Paga por herramientas, nunca por ganar", "Antes de suscribirte"],
    labels: ["Hoja de ruta", "Comunidad", "Prensa", "Soporte", "Estado", "Cuenta", "Privacidad", "Términos", "Suscripción y reembolsos", "Política de datos de IA", "Acceso fundador", "Enviar solicitud", "Acepto el uso de estos datos solo para investigación y avisos de lanzamiento.", "Entorno recomendado", "Seguridad", "Más información", "No configurado", "Borrador público", "Actualizado el 28.08.2026"],
    legal: ["Este documento explica los límites de datos y servicio del sitio, el juego, las emisiones y la IA.", "Solo tratamos datos necesarios para la función elegida o autorizados expresamente por el jugador.", "No publicamos ubicaciones exactas en tiempo real, bienes privados, chat privado ni identidades no autorizadas.", "Se requiere revisión jurídica local antes del lanzamiento; el jugador conserva los derechos sobre su obra original."], footer: "Un mundo persistente creado por sus jugadores.",
  },
  "pt-BR": {
    name: "Português (Brasil)", short: "PT-BR",
    nav: ["Início", "Explorar", "Mundo", "Classes", "Regiões", "Pulso", "Ao vivo", "Destaques", "Atlas", "Menu", "Idioma"],
    hero: ["Legend of", "Rebirth", "Retorne ao continente de Mir. Lute ao lado de jogadores reais em um mundo compartilhado e em evolução e escreva sua própria lenda.", "Entrar no jogo", "Explorar o mundo", "Serviço do mundo online"],
    core: ["O mundo não para.\nTudo deixa um registro.", "Consulte personagens, guildas, equipamentos, mercados e eventos públicos; localização exata e dados privados continuam protegidos.", "Não é uma instância.\nÉ um mundo compartilhado.", "Cidades, áreas selvagens, masmorras e Sabuk formam um continente persistente movido por jogadores.", "Três caminhos. Uma lenda.", "Força, elementos e espíritos são ferramentas; seus aliados definem o destino do seu nome.", "Guerreiro", "Feiticeiro", "Taoísta", "Um único Mir,\nrecontado para cada região.", "Idioma, voz e arte regional são entregues separadamente; jogabilidade e regras de justiça permanecem globais.", "O mundo está em movimento"],
    watch: ["O mundo se move.\nAs lendas são registradas.", "A direção assistida por IA transforma eventos públicos em narração e destaques, sempre marcados como conteúdo Candidate.", "Mir Mundo 01 · Observatório", "Exibimos apenas eventos públicos com atraso e proteção de privacidade.", "Destaques de hoje", "Um evento, muitas expressões locais"],
    member: ["Não é um pacote mensal.\nÉ uma camada viva de serviços de IA.", "Assinatura vinculada à conta para aventura e criação; não vende poder de combate nem é um ativo negociável.", "Mensal", "Anual", "Recomendado", "Em breve", "Grátis", "Viajante", "Arquiteto", "Uso de IA medido com clareza", "Pague por ferramentas, nunca por vitória", "Antes de assinar"],
    labels: ["Roteiro", "Comunidade", "Imprensa", "Suporte", "Status", "Conta", "Privacidade", "Termos", "Assinatura e reembolsos", "Política de dados de IA", "Acesso de fundador", "Enviar interesse", "Concordo com o uso destes dados apenas para pesquisa e avisos de lançamento.", "Ambiente recomendado", "Segurança", "Saiba mais", "Não configurado", "Rascunho público", "Atualizado em 28.08.2026"],
    legal: ["Este documento explica os limites de dados e serviço do site, jogo, transmissões e IA.", "Processamos apenas dados necessários à função escolhida ou autorizados claramente pelo jogador.", "Não publicamos localização exata em tempo real, bens privados, conversas privadas ou identidade sem autorização.", "É necessária revisão jurídica local antes do lançamento; o jogador mantém os direitos sobre o conteúdo original."], footer: "Um mundo persistente moldado por seus jogadores.",
  },
  de: {
    name: "Deutsch", short: "DE",
    nav: ["Start", "Entdecken", "Welt", "Klassen", "Regionen", "Weltpuls", "Live", "Highlights", "Atlas", "Menü", "Sprache"],
    hero: ["Legend of", "Rebirth", "Kehre nach Mir zurück. Kämpfe mit echten Spielern in einer gemeinsamen, wachsenden Welt und schreibe deine eigene Legende.", "Spiel starten", "Welt entdecken", "Weltservice online"],
    core: ["Die Welt steht nie still.\nAlles hinterlässt Spuren.", "Öffentliche Charaktere, Gilden, Ausrüstung, Märkte und Ereignisse bleiben sichtbar; genaue Orte und private Daten bleiben geschützt.", "Keine Instanz.\nEine gemeinsame Welt.", "Städte, Wildnis, Verliese und Sabuk bilden einen beständigen, von Spielern geprägten Kontinent.", "Drei Pfade. Eine Legende.", "Stärke, Elemente und Geister sind Werkzeuge; deine Gefährten bestimmen den Klang deines Namens.", "Krieger", "Magier", "Taoist", "Ein Mir,\nfür jede Region neu erzählt.", "Sprache, Stimme und regionale Gestaltung werden getrennt ausgeliefert; Spielregeln und Fairness bleiben weltweit gleich.", "Die Welt bewegt sich"],
    watch: ["Die Welt bewegt sich.\nLegenden werden aufgezeichnet.", "KI-gestützte Regie verwandelt öffentliche Ereignisse in Kommentare und Highlights, klar als Candidate-Inhalt gekennzeichnet.", "Mir Welt 01 · Observatorium", "Nur verzögerte und datenschutzgefilterte öffentliche Ereignisse werden gezeigt.", "Highlights des Tages", "Ein Ereignis, viele lokale Ausdrucksformen"],
    member: ["Kein monatliches Belohnungspaket.\nEine lebendige KI-Serviceschicht.", "Kontogebundenes Abo für Abenteuer und Kreativität; keine Kampfkraft und kein handelbarer Vermögenswert.", "Monatlich", "Jährlich", "Empfohlen", "Demnächst", "Kostenlos", "Wanderer", "Architekt", "KI-Nutzung klar ausgewiesen", "Werkzeuge kaufen, niemals Siege", "Vor dem Abonnement"],
    labels: ["Roadmap", "Community", "Presse", "Support", "Status", "Konto", "Datenschutz", "Bedingungen", "Abos und Erstattungen", "KI-Datenrichtlinie", "Founder Access", "Interesse senden", "Ich stimme der Nutzung dieser Daten nur für Forschung und Startinformationen zu.", "Empfohlene Umgebung", "Sicherheit", "Mehr erfahren", "Nicht konfiguriert", "Öffentlicher Entwurf", "Aktualisiert am 28.08.2026"],
    legal: ["Dieses Dokument beschreibt Daten- und Servicegrenzen von Website, Spiel, Übertragungen und KI.", "Wir verarbeiten nur Daten, die für die gewählte Funktion nötig oder ausdrücklich freigegeben sind.", "Exakte Live-Positionen, private Gegenstände, private Chats und nicht freigegebene Identitäten werden nicht veröffentlicht.", "Vor Veröffentlichung ist eine lokale Rechtsprüfung nötig; Rechte an eigenen Inhalten bleiben beim Spieler."], footer: "Eine beständige Welt, gestaltet von ihren Spielern.",
  },
  fr: {
    name: "Français", short: "FR",
    nav: ["Accueil", "Explorer", "Monde", "Classes", "Régions", "Pouls", "Direct", "Temps forts", "Atlas", "Menu", "Langue"],
    hero: ["Legend of", "Rebirth", "Retournez sur le continent de Mir. Combattez avec de vrais joueurs dans un monde partagé et évolutif, et écrivez votre propre légende.", "Entrer dans le jeu", "Explorer le monde", "Service du monde en ligne"],
    core: ["Le monde ne s'arrête jamais.\nTout laisse une trace.", "Consultez personnages, guildes, équipements, marchés et événements publics ; les positions exactes et données privées restent protégées.", "Pas une instance.\nUn monde partagé.", "Villes, terres sauvages, donjons et Sabuk forment un continent persistant façonné par les joueurs.", "Trois voies. Une légende.", "Force, éléments et esprits sont des outils ; vos alliés décident de ce que devient votre nom.", "Guerrier", "Sorcier", "Taoïste", "Un seul Mir,\nraconté pour chaque région.", "Langue, voix et art régional sont livrés séparément ; le gameplay et les règles d'équité restent mondiaux.", "Le monde est en mouvement"],
    watch: ["Le monde bouge.\nLes légendes sont enregistrées.", "La réalisation assistée par IA transforme les événements publics en commentaires et temps forts, clairement marqués Candidate.", "Mir Monde 01 · Observatoire", "Seuls les événements publics différés et filtrés pour la confidentialité sont affichés.", "Temps forts du jour", "Un événement, plusieurs expressions locales"],
    member: ["Pas un pack de récompenses.\nUne couche vivante de services IA.", "Abonnement lié au compte pour l'aventure et la création ; aucune puissance de combat ni actif échangeable.", "Mensuel", "Annuel", "Recommandé", "Bientôt", "Gratuit", "Voyageur", "Architecte", "Utilisation IA clairement mesurée", "Payez des outils, jamais la victoire", "Avant de s'abonner"],
    labels: ["Feuille de route", "Communauté", "Presse", "Assistance", "État", "Compte", "Confidentialité", "Conditions", "Abonnements et remboursements", "Politique des données IA", "Accès fondateur", "Envoyer la demande", "J'accepte l'utilisation de ces données uniquement pour la recherche et les annonces de lancement.", "Configuration recommandée", "Sécurité", "En savoir plus", "Non configuré", "Projet public", "Mis à jour le 28.08.2026"],
    legal: ["Ce document explique les limites de données et de service du site, du jeu, des diffusions et de l'IA.", "Nous traitons uniquement les données nécessaires à la fonction choisie ou expressément autorisées.", "Nous ne publions ni position exacte en direct, ni objets privés, ni messages privés, ni identité non autorisée.", "Une validation juridique locale est requise avant la sortie ; les joueurs conservent leurs droits sur leurs créations."], footer: "Un monde persistant façonné par ses joueurs.",
  },
  ru: {
    name: "Русский", short: "RU",
    nav: ["Главная", "Обзор", "Мир", "Классы", "Регионы", "Пульс мира", "Эфир", "Моменты", "Атлас", "Меню", "Язык"],
    hero: ["Legend of", "Rebirth", "Вернитесь на континент Мир. Сражайтесь вместе с реальными игроками в общем развивающемся мире и напишите свою легенду.", "Войти в игру", "Исследовать мир", "Мировой сервис онлайн"],
    core: ["Мир не останавливается.\nВсё оставляет след.", "Смотрите открытых персонажей, гильдии, предметы, рынки и события; точные координаты и личные данные скрыты.", "Не отдельная сессия.\nОдин общий мир.", "Города, пустоши, подземелья и Сабук образуют постоянный континент, которым движут игроки.", "Три пути. Одна легенда.", "Сила, стихии и духи — лишь инструменты; союзники определяют судьбу вашего имени.", "Воин", "Маг", "Даос", "Один Мир,\nрассказанный для каждого региона.", "Язык, озвучка и региональная графика выпускаются отдельно; игровой процесс и честные правила едины.", "Мир продолжает меняться"],
    watch: ["Мир движется.\nЛегенды остаются в записи.", "ИИ-режиссура превращает открытые события в комментарии и лучшие моменты с честной отметкой Candidate.", "Мир 01 · Обсерватория", "Показываются только задержанные публичные события с защитой приватности.", "Лучшие моменты дня", "Одно событие — разные локальные версии"],
    member: ["Не набор ежемесячных наград.\nЖивой слой ИИ-сервисов.", "Привязанная к аккаунту подписка для приключений и творчества; без боевой силы и торговли активом.", "Ежемесячно", "Ежегодно", "Рекомендуем", "Скоро", "Бесплатно", "Странник", "Архитектор", "Прозрачный учёт ИИ", "Платите за инструменты, не за победу", "Перед подпиской"],
    labels: ["План развития", "Сообщество", "Пресса", "Поддержка", "Статус", "Аккаунт", "Конфиденциальность", "Условия", "Подписка и возвраты", "Политика ИИ-данных", "Ранний доступ", "Отправить заявку", "Я согласен на использование этих данных только для исследования и уведомлений о запуске.", "Рекомендуемая среда", "Безопасность", "Подробнее", "Не настроено", "Публичный черновик", "Обновлено 28.08.2026"],
    legal: ["Документ описывает границы данных и сервисов сайта, игры, трансляций и ИИ.", "Обрабатываются только данные, нужные выбранной функции или явно разрешённые игроком.", "Точные координаты, личные предметы, личные сообщения и неразрешённые личности не публикуются.", "До выпуска нужна местная юридическая проверка; права на оригинальные материалы остаются у игрока."], footer: "Постоянный мир, который создают игроки.",
  },
  pl: {
    name: "Polski", short: "PL",
    nav: ["Start", "Odkrywaj", "Świat", "Klasy", "Regiony", "Puls świata", "Na żywo", "Skróty", "Atlas", "Menu", "Język"],
    hero: ["Legend of", "Rebirth", "Wróć na kontynent Mir. Walcz z prawdziwymi graczami we wspólnym, rozwijającym się świecie i napisz własną legendę.", "Wejdź do gry", "Odkrywaj świat", "Usługa świata online"],
    core: ["Świat nigdy nie staje.\nWszystko zostawia zapis.", "Przeglądaj publiczne postacie, gildie, wyposażenie, rynki i wydarzenia; dokładne położenie i prywatne dane pozostają ukryte.", "To nie instancja.\nTo wspólny świat.", "Miasta, pustkowia, lochy i Sabuk tworzą trwały kontynent napędzany przez graczy.", "Trzy ścieżki. Jedna legenda.", "Siła, żywioły i duchy są narzędziami; to sojusznicy decydują, czym stanie się twoje imię.", "Wojownik", "Mag", "Taoista", "Jeden Mir,\nopowiedziany dla każdego regionu.", "Język, głos i regionalna oprawa są wydawane osobno; zasady gry i uczciwości pozostają wspólne.", "Świat jest w ruchu"],
    watch: ["Świat się porusza.\nLegendy są zapisywane.", "Reżyseria wspierana przez AI zmienia publiczne wydarzenia w komentarz i skróty, wyraźnie oznaczone jako Candidate.", "Mir Świat 01 · Obserwatorium", "Pokazujemy tylko opóźnione, publiczne wydarzenia z ochroną prywatności.", "Dzisiejsze skróty", "Jedno wydarzenie, wiele wersji lokalnych"],
    member: ["Nie miesięczna paczka nagród.\nŻywa warstwa usług AI.", "Subskrypcja przypisana do konta dla przygody i twórczości; bez siły bojowej i handlowalnego aktywa.", "Miesięcznie", "Rocznie", "Polecane", "Wkrótce", "Bezpłatnie", "Wędrowiec", "Architekt", "Jasno mierzony limit AI", "Płać za narzędzia, nigdy za zwycięstwo", "Przed subskrypcją"],
    labels: ["Plan rozwoju", "Społeczność", "Prasa", "Pomoc", "Status", "Konto", "Prywatność", "Warunki", "Subskrypcje i zwroty", "Polityka danych AI", "Dostęp założycielski", "Wyślij zgłoszenie", "Zgadzam się na użycie tych danych tylko do badań i informacji o premierze.", "Zalecane środowisko", "Bezpieczeństwo", "Dowiedz się więcej", "Nie skonfigurowano", "Publiczny projekt", "Aktualizacja 28.08.2026"],
    legal: ["Dokument opisuje granice danych i usług witryny, gry, transmisji oraz AI.", "Przetwarzamy tylko dane potrzebne wybranej funkcji lub wyraźnie zatwierdzone przez gracza.", "Nie publikujemy dokładnej pozycji na żywo, prywatnych przedmiotów, rozmów ani niezatwierdzonej tożsamości.", "Przed premierą wymagana jest lokalna kontrola prawna; prawa do własnych treści pozostają przy graczu."], footer: "Trwały świat kształtowany przez graczy.",
  },
  tr: {
    name: "Türkçe", short: "TR",
    nav: ["Ana sayfa", "Keşfet", "Dünya", "Sınıflar", "Bölgeler", "Dünya nabzı", "Canlı", "Öne çıkanlar", "Atlas", "Menü", "Dil"],
    hero: ["Legend of", "Rebirth", "Mir kıtasına dön. Ortak ve gelişen bir dünyada gerçek oyuncularla savaş, kendi efsaneni yaz.", "Oyuna gir", "Dünyayı keşfet", "Dünya hizmeti çevrimiçi"],
    core: ["Dünya hiç durmaz.\nHer şey bir kayıt bırakır.", "Herkese açık karakterleri, loncaları, eşyaları, pazarları ve olayları gör; kesin konumlar ve özel veriler korunur.", "Bir oturum değil.\nOrtak bir dünya.", "Şehirler, vahşi alanlar, zindanlar ve Sabuk oyuncuların şekillendirdiği kalıcı bir kıta oluşturur.", "Üç yol. Tek efsane.", "Güç, elementler ve ruhlar yalnızca araçtır; adının kaderini yanında duranlar belirler.", "Savaşçı", "Büyücü", "Taoist", "Tek Mir,\nher bölge için yeniden anlatılır.", "Dil, ses ve bölgesel sanat ayrı sunulur; oynanış ve adalet kuralları tüm dünyada aynıdır.", "Dünya hareket halinde"],
    watch: ["Dünya hareket ediyor.\nEfsaneler kayda geçiyor.", "Yapay zekâ destekli yönetmenlik, açık dünya olaylarını Candidate etiketiyle anlatıma ve öne çıkanlara dönüştürür.", "Mir Dünya 01 · Gözlemevi", "Yalnızca gecikmeli ve gizlilik filtresinden geçen herkese açık olaylar gösterilir.", "Günün öne çıkanları", "Tek olay, birçok yerel anlatım"],
    member: ["Aylık ödül paketi değil.\nYaşayan bir yapay zekâ hizmet katmanı.", "Macera ve üretim için hesaba bağlı abonelik; savaş gücü satmaz ve alınıp satılan bir varlık değildir.", "Aylık", "Yıllık", "Önerilen", "Yakında", "Ücretsiz", "Gezgin", "Mimar", "Açık yapay zekâ kullanım sınırı", "Araçlara ödeme yap, zafere değil", "Abone olmadan önce"],
    labels: ["Yol haritası", "Topluluk", "Basın", "Destek", "Durum", "Hesap", "Gizlilik", "Koşullar", "Abonelik ve iadeler", "Yapay zekâ veri politikası", "Kurucu erişimi", "Başvuru gönder", "Bu verilerin yalnızca araştırma ve çıkış bildirimleri için kullanılmasını kabul ediyorum.", "Önerilen ortam", "Güvenlik", "Daha fazla bilgi", "Yapılandırılmadı", "Herkese açık taslak", "28.08.2026 tarihinde güncellendi"],
    legal: ["Bu belge site, oyun, yayınlar ve yapay zekâ hizmetlerinin veri sınırlarını açıklar.", "Yalnızca seçilen işlev için gerekli veya oyuncunun açıkça izin verdiği veriler işlenir.", "Kesin canlı konum, özel eşyalar, özel sohbet ve izinsiz kimlik yayımlanmaz.", "Yayın öncesi yerel hukuk incelemesi gerekir; özgün içerik hakları oyuncuda kalır."], footer: "Oyuncuların şekillendirdiği kalıcı bir dünya.",
  },
  th: {
    name: "ไทย", short: "TH",
    nav: ["หน้าแรก", "สำรวจ", "โลก", "อาชีพ", "ภูมิภาค", "ชีพจรโลก", "ถ่ายทอดสด", "ไฮไลต์", "แอตลาส", "เมนู", "ภาษา"],
    hero: ["Legend of", "Rebirth", "กลับสู่ทวีป Mir ต่อสู้เคียงข้างผู้เล่นจริงในโลกเดียวที่พัฒนาอย่างต่อเนื่อง และเขียนตำนานของคุณเอง", "เข้าเกม", "สำรวจโลก", "บริการโลกออนไลน์"],
    core: ["โลกไม่เคยหยุด\nทุกสิ่งทิ้งบันทึกไว้", "ดูตัวละคร กิลด์ อุปกรณ์ ตลาด และเหตุการณ์สาธารณะได้ โดยตำแหน่งที่แม่นยำและข้อมูลส่วนตัวยังคงถูกปกป้อง", "ไม่ใช่อินสแตนซ์\nแต่เป็นโลกที่ใช้ร่วมกัน", "เมือง ป่า ดันเจียน และ Sabuk รวมเป็นทวีปถาวรที่ขับเคลื่อนโดยผู้เล่น", "สามเส้นทาง หนึ่งตำนาน", "พลัง ธาตุ และวิญญาณเป็นเพียงเครื่องมือ ผู้ร่วมทางต่างหากที่กำหนดชื่อของคุณ", "นักรบ", "จอมเวท", "นักพรต", "Mir เดียว\nเล่าใหม่สำหรับทุกภูมิภาค", "ภาษา เสียง และภาพภูมิภาคส่งมอบแยกกัน แต่กติกาและความยุติธรรมเหมือนกันทั่วโลก", "โลกกำลังเคลื่อนไหว"],
    watch: ["โลกกำลังเคลื่อนไหว\nตำนานกำลังถูกบันทึก", "ผู้กำกับที่ใช้ AI ช่วยเปลี่ยนเหตุการณ์สาธารณะเป็นคำบรรยายและไฮไลต์ พร้อมระบุว่าเป็น Candidate อย่างชัดเจน", "Mir World 01 · หอสังเกตการณ์", "แสดงเฉพาะเหตุการณ์สาธารณะที่หน่วงเวลาและผ่านการปกป้องความเป็นส่วนตัว", "ไฮไลต์วันนี้", "เหตุการณ์เดียว หลายรูปแบบท้องถิ่น"],
    member: ["ไม่ใช่กล่องรางวัลรายเดือน\nแต่เป็นชั้นบริการ AI ที่พัฒนาได้", "สมาชิกผูกกับบัญชีสำหรับการผจญภัยและการสร้างสรรค์ ไม่ขายพลังต่อสู้และไม่ใช่สินทรัพย์ซื้อขาย", "รายเดือน", "รายปี", "แนะนำ", "เร็ว ๆ นี้", "ฟรี", "นักเดินทาง", "สถาปนิก", "แสดงโควตา AI ชัดเจน", "จ่ายเพื่อเครื่องมือ ไม่ใช่ชัยชนะ", "ก่อนสมัครสมาชิก"],
    labels: ["แผนพัฒนา", "ชุมชน", "สื่อ", "ช่วยเหลือ", "สถานะ", "บัญชี", "ความเป็นส่วนตัว", "ข้อกำหนด", "สมาชิกและคืนเงิน", "นโยบายข้อมูล AI", "Founder Access", "ส่งความสนใจ", "ฉันยินยอมให้ใช้ข้อมูลนี้เพื่อการวิจัยและแจ้งเปิดตัวเท่านั้น", "สภาพแวดล้อมที่แนะนำ", "ความปลอดภัย", "ดูเพิ่มเติม", "ยังไม่ตั้งค่า", "ฉบับร่างสาธารณะ", "อัปเดต 28.08.2026"],
    legal: ["เอกสารนี้อธิบายขอบเขตข้อมูลและบริการของเว็บไซต์ เกม การถ่ายทอด และ AI", "ประมวลผลเฉพาะข้อมูลที่จำเป็นต่อฟังก์ชันหรือผู้เล่นอนุญาตอย่างชัดเจน", "ไม่เผยตำแหน่งสดที่แม่นยำ ทรัพย์สินส่วนตัว แชตส่วนตัว หรืออัตลักษณ์ที่ไม่ได้รับอนุญาต", "ต้องตรวจสอบกฎหมายท้องถิ่นก่อนเปิดบริการ ผู้เล่นยังคงสิทธิ์ในผลงานต้นฉบับ"], footer: "โลกถาวรที่ผู้เล่นร่วมกันสร้าง",
  },
  vi: {
    name: "Tiếng Việt", short: "VI",
    nav: ["Trang chủ", "Khám phá", "Thế giới", "Lớp nhân vật", "Khu vực", "Nhịp thế giới", "Trực tiếp", "Khoảnh khắc", "Atlas", "Menu", "Ngôn ngữ"],
    hero: ["Legend of", "Rebirth", "Trở lại lục địa Mir. Chiến đấu cùng người chơi thật trong một thế giới chung luôn phát triển và viết nên huyền thoại của riêng bạn.", "Vào game", "Khám phá thế giới", "Dịch vụ thế giới trực tuyến"],
    core: ["Thế giới không dừng lại.\nMọi thứ đều để lại dấu vết.", "Xem nhân vật, bang hội, trang bị, thị trường và sự kiện công khai; vị trí chính xác và dữ liệu riêng tư luôn được bảo vệ.", "Không phải bản riêng.\nĐây là một thế giới chung.", "Thành phố, hoang dã, hầm ngục và Sabuk tạo thành lục địa bền vững do người chơi thúc đẩy.", "Ba con đường. Một huyền thoại.", "Sức mạnh, nguyên tố và linh hồn chỉ là công cụ; đồng đội quyết định tên tuổi của bạn.", "Chiến binh", "Pháp sư", "Đạo sĩ", "Một Mir,\nđược kể lại cho từng khu vực.", "Ngôn ngữ, giọng nói và hình ảnh địa phương được phát hành riêng; lối chơi và luật công bằng vẫn thống nhất.", "Thế giới đang chuyển động"],
    watch: ["Thế giới đang chuyển động.\nHuyền thoại đang được ghi lại.", "Đạo diễn hỗ trợ bởi AI biến sự kiện công khai thành bình luận và khoảnh khắc, luôn ghi rõ nội dung Candidate.", "Mir World 01 · Đài quan sát", "Chỉ hiển thị sự kiện công khai có độ trễ và đã lọc quyền riêng tư.", "Khoảnh khắc hôm nay", "Một sự kiện, nhiều cách thể hiện địa phương"],
    member: ["Không phải gói thưởng tháng.\nĐây là lớp dịch vụ AI sống động.", "Gói đăng ký gắn với tài khoản cho phiêu lưu và sáng tạo; không bán sức mạnh chiến đấu và không phải tài sản giao dịch.", "Hàng tháng", "Hàng năm", "Đề xuất", "Sắp ra mắt", "Miễn phí", "Lữ khách", "Kiến trúc sư", "Hạn mức AI minh bạch", "Trả tiền cho công cụ, không mua chiến thắng", "Trước khi đăng ký"],
    labels: ["Lộ trình", "Cộng đồng", "Báo chí", "Hỗ trợ", "Trạng thái", "Tài khoản", "Quyền riêng tư", "Điều khoản", "Đăng ký và hoàn tiền", "Chính sách dữ liệu AI", "Founder Access", "Gửi đăng ký", "Tôi đồng ý chỉ dùng dữ liệu này cho nghiên cứu và thông báo ra mắt.", "Môi trường đề xuất", "An toàn", "Tìm hiểu thêm", "Chưa cấu hình", "Bản nháp công khai", "Cập nhật 28.08.2026"],
    legal: ["Tài liệu này giải thích ranh giới dữ liệu và dịch vụ của trang web, trò chơi, phát sóng và AI.", "Chỉ xử lý dữ liệu cần cho chức năng đã chọn hoặc được người chơi cho phép rõ ràng.", "Không công khai vị trí trực tiếp chính xác, tài sản riêng, trò chuyện riêng hay danh tính chưa được phép.", "Cần rà soát pháp lý địa phương trước khi phát hành; người chơi giữ quyền với nội dung gốc."], footer: "Một thế giới bền vững do người chơi định hình.",
  },
  id: {
    name: "Bahasa Indonesia", short: "ID",
    nav: ["Beranda", "Jelajahi", "Dunia", "Kelas", "Wilayah", "Denyut dunia", "Siaran", "Sorotan", "Atlas", "Menu", "Bahasa"],
    hero: ["Legend of", "Rebirth", "Kembali ke benua Mir. Bertarung bersama pemain nyata di satu dunia yang terus berkembang dan tulis legendamu sendiri.", "Masuk game", "Jelajahi dunia", "Layanan dunia online"],
    core: ["Dunia tidak pernah berhenti.\nSemua meninggalkan catatan.", "Lihat karakter, guild, perlengkapan, pasar, dan peristiwa publik; lokasi tepat serta data pribadi tetap terlindungi.", "Bukan instans.\nIni dunia bersama.", "Kota, alam liar, ruang bawah tanah, dan Sabuk membentuk benua persisten yang digerakkan pemain.", "Tiga jalan. Satu legenda.", "Kekuatan, elemen, dan roh hanyalah alat; rekan seperjalanan menentukan arti namamu.", "Prajurit", "Penyihir", "Taois", "Satu Mir,\ndiceritakan ulang untuk tiap wilayah.", "Bahasa, suara, dan seni regional dikirim terpisah; gameplay dan aturan keadilan tetap sama secara global.", "Dunia terus bergerak"],
    watch: ["Dunia terus bergerak.\nLegenda sedang direkam.", "Penyutradaraan berbantuan AI mengubah peristiwa publik menjadi komentar dan sorotan dengan label Candidate yang jelas.", "Mir World 01 · Observatorium", "Hanya peristiwa publik tertunda dan tersaring privasi yang ditampilkan.", "Sorotan hari ini", "Satu peristiwa, banyak ekspresi lokal"],
    member: ["Bukan paket hadiah bulanan.\nLapisan layanan AI yang hidup.", "Langganan terikat akun untuk petualangan dan kreasi; tidak menjual kekuatan tempur dan bukan aset yang dapat diperdagangkan.", "Bulanan", "Tahunan", "Direkomendasikan", "Segera hadir", "Gratis", "Pengembara", "Arsitek", "Penggunaan AI yang transparan", "Bayar alat, bukan kemenangan", "Sebelum berlangganan"],
    labels: ["Peta jalan", "Komunitas", "Pers", "Dukungan", "Status", "Akun", "Privasi", "Ketentuan", "Langganan dan pengembalian dana", "Kebijakan data AI", "Akses pendiri", "Kirim minat", "Saya setuju data ini hanya digunakan untuk riset dan pemberitahuan peluncuran.", "Lingkungan yang disarankan", "Keamanan", "Pelajari lebih lanjut", "Belum dikonfigurasi", "Draf publik", "Diperbarui 28.08.2026"],
    legal: ["Dokumen ini menjelaskan batas data dan layanan situs, game, siaran, dan AI.", "Kami hanya memproses data yang diperlukan fungsi pilihan atau disetujui jelas oleh pemain.", "Kami tidak memublikasikan lokasi langsung yang tepat, aset pribadi, chat pribadi, atau identitas tanpa izin.", "Tinjauan hukum lokal diperlukan sebelum rilis; hak atas karya asli tetap dimiliki pemain."], footer: "Dunia persisten yang dibentuk oleh para pemain.",
  },
  ar: {
    name: "العربية", short: "AR", dir: "rtl",
    nav: ["الرئيسية", "استكشاف", "العالم", "الفئات", "المناطق", "نبض العالم", "مباشر", "اللقطات", "الأطلس", "القائمة", "اللغة"],
    hero: ["Legend of", "Rebirth", "عُد إلى قارة Mir وقاتل إلى جانب لاعبين حقيقيين في عالم مشترك ومتطور، واكتب أسطورتك بنفسك.", "دخول اللعبة", "استكشاف العالم", "خدمة العالم متصلة"],
    core: ["العالم لا يتوقف.\nكل شيء يترك سجلاً.", "شاهد الشخصيات والنقابات والمعدات والأسواق والأحداث العامة، بينما تبقى المواقع الدقيقة والبيانات الخاصة محمية.", "ليست نسخة منفصلة.\nإنه عالم مشترك.", "تشكّل المدن والبراري والدهاليز وSabuk قارة دائمة يصنعها اللاعبون.", "ثلاثة مسارات. أسطورة واحدة.", "القوة والعناصر والأرواح مجرد أدوات؛ رفاقك هم من يحددون معنى اسمك.", "محارب", "ساحر", "طاوي", "عالم Mir واحد،\nيُروى لكل منطقة.", "تُقدّم اللغة والصوت والفن المحلي بصورة مستقلة، بينما تبقى قواعد اللعب والعدالة موحّدة.", "العالم يتحرك"],
    watch: ["العالم يتحرك.\nوالأساطير تُسجّل.", "يحوّل الإخراج المدعوم بالذكاء الاصطناعي الأحداث العامة إلى تعليق ولقطات مميزة مع وسم Candidate واضح.", "Mir World 01 · المرصد", "لا نعرض إلا الأحداث العامة المؤخرة والمحمية بالخصوصية.", "لقطات اليوم", "حدث واحد وتعبيرات محلية متعددة"],
    member: ["ليست حزمة مكافآت شهرية.\nبل طبقة حية من خدمات الذكاء الاصطناعي.", "اشتراك مرتبط بالحساب للمغامرة والإبداع؛ لا يبيع قوة قتالية وليس أصلاً قابلاً للتداول.", "شهري", "سنوي", "موصى به", "قريباً", "مجاني", "رحّال", "مهندس", "استخدام واضح لخدمات الذكاء الاصطناعي", "ادفع للأدوات لا للنصر", "قبل الاشتراك"],
    labels: ["خارطة الطريق", "المجتمع", "الصحافة", "الدعم", "الحالة", "الحساب", "الخصوصية", "الشروط", "الاشتراك والاسترداد", "سياسة بيانات الذكاء الاصطناعي", "وصول المؤسسين", "إرسال الطلب", "أوافق على استخدام هذه البيانات للبحث وإشعارات الإطلاق فقط.", "البيئة الموصى بها", "الأمان", "اعرف المزيد", "غير مهيأ", "مسودة عامة", "تم التحديث في 28.08.2026"],
    legal: ["تشرح هذه الوثيقة حدود البيانات والخدمة للموقع واللعبة والبث والذكاء الاصطناعي.", "نعالج فقط البيانات اللازمة للميزة المختارة أو التي يصرّح بها اللاعب بوضوح.", "لا ننشر الموقع المباشر الدقيق أو الممتلكات والمحادثات الخاصة أو الهوية غير المصرح بها.", "تلزم مراجعة قانونية محلية قبل الإطلاق، وتبقى حقوق المحتوى الأصلي للاعب."], footer: "عالم دائم يصنعه لاعبوه.",
  },
};

type MarketUi = {
  official: string;
  join: string;
  watch: string;
  email: string;
  company: string;
  submitError: string;
  copied: string;
  share: string;
  scroll: string;
  prototype: string;
  accountUsage: [companion: string, highlights: string, voice: string];
};

const marketUi: Record<MarketLocale, MarketUi> = {
  ja: { official: "公式", join: "参加する", watch: "視聴する", email: "メール", company: "会社", submitError: "送信できませんでした。後でもう一度お試しください。", copied: "コピーしました", share: "共有", scroll: "続きを見る", prototype: "プロトタイプデータ", accountUsage: ["AIコンパニオン", "ハイライト生成", "音声書き出し"] },
  ko: { official: "공식", join: "참여하기", watch: "시청하기", email: "이메일", company: "회사", submitError: "전송하지 못했습니다. 잠시 후 다시 시도해 주세요.", copied: "복사됨", share: "공유", scroll: "계속 보기", prototype: "프로토타입 데이터", accountUsage: ["AI 동료", "하이라이트 생성", "음성 내보내기"] },
  "zh-TW": { official: "官方", join: "加入", watch: "觀看", email: "電子郵件", company: "公司", submitError: "提交失敗，請稍後再試。", copied: "已複製", share: "分享", scroll: "繼續瀏覽", prototype: "原型資料", accountUsage: ["AI 夥伴", "精彩片段生成", "語音匯出"] },
  es: { official: "Oficial", join: "Unirse", watch: "Ver", email: "Correo electrónico", company: "Empresa", submitError: "No se pudo enviar. Inténtalo de nuevo más tarde.", copied: "Copiado", share: "Compartir", scroll: "Seguir explorando", prototype: "Datos de prototipo", accountUsage: ["Compañero de IA", "Generación de momentos", "Exportación de voz"] },
  "pt-BR": { official: "Oficial", join: "Participar", watch: "Assistir", email: "E-mail", company: "Empresa", submitError: "Não foi possível enviar. Tente novamente mais tarde.", copied: "Copiado", share: "Compartilhar", scroll: "Continuar explorando", prototype: "Dados de protótipo", accountUsage: ["Companheiro de IA", "Geração de destaques", "Exportação de voz"] },
  de: { official: "Offiziell", join: "Beitreten", watch: "Ansehen", email: "E-Mail", company: "Unternehmen", submitError: "Senden fehlgeschlagen. Bitte später erneut versuchen.", copied: "Kopiert", share: "Teilen", scroll: "Weiter entdecken", prototype: "Prototypdaten", accountUsage: ["KI-Begleiter", "Highlight-Erstellung", "Sprachexport"] },
  fr: { official: "Officiel", join: "Rejoindre", watch: "Regarder", email: "E-mail", company: "Entreprise", submitError: "Envoi impossible. Réessayez plus tard.", copied: "Copié", share: "Partager", scroll: "Continuer à explorer", prototype: "Données prototype", accountUsage: ["Compagnon IA", "Création de temps forts", "Export vocal"] },
  ru: { official: "Официальный", join: "Присоединиться", watch: "Смотреть", email: "Эл. почта", company: "Компания", submitError: "Не удалось отправить. Повторите попытку позже.", copied: "Скопировано", share: "Поделиться", scroll: "Продолжить", prototype: "Данные прототипа", accountUsage: ["ИИ-спутник", "Создание моментов", "Экспорт голоса"] },
  pl: { official: "Oficjalny", join: "Dołącz", watch: "Oglądaj", email: "E-mail", company: "Firma", submitError: "Nie udało się wysłać. Spróbuj ponownie później.", copied: "Skopiowano", share: "Udostępnij", scroll: "Odkrywaj dalej", prototype: "Dane prototypowe", accountUsage: ["Towarzysz AI", "Tworzenie skrótów", "Eksport głosu"] },
  tr: { official: "Resmî", join: "Katıl", watch: "İzle", email: "E-posta", company: "Şirket", submitError: "Gönderilemedi. Lütfen daha sonra tekrar deneyin.", copied: "Kopyalandı", share: "Paylaş", scroll: "Keşfetmeye devam et", prototype: "Prototip veriler", accountUsage: ["AI yoldaşı", "Öne çıkan an üretimi", "Ses dışa aktarma"] },
  th: { official: "ทางการ", join: "เข้าร่วม", watch: "รับชม", email: "อีเมล", company: "บริษัท", submitError: "ส่งไม่สำเร็จ โปรดลองอีกครั้งภายหลัง", copied: "คัดลอกแล้ว", share: "แชร์", scroll: "สำรวจต่อ", prototype: "ข้อมูลต้นแบบ", accountUsage: ["เพื่อนร่วมทาง AI", "สร้างไฮไลต์", "ส่งออกเสียง"] },
  vi: { official: "Chính thức", join: "Tham gia", watch: "Xem", email: "Email", company: "Công ty", submitError: "Không thể gửi. Vui lòng thử lại sau.", copied: "Đã sao chép", share: "Chia sẻ", scroll: "Tiếp tục khám phá", prototype: "Dữ liệu nguyên mẫu", accountUsage: ["Bạn đồng hành AI", "Tạo điểm nhấn", "Xuất giọng nói"] },
  id: { official: "Resmi", join: "Bergabung", watch: "Tonton", email: "Email", company: "Perusahaan", submitError: "Tidak dapat mengirim. Coba lagi nanti.", copied: "Disalin", share: "Bagikan", scroll: "Lanjut menjelajah", prototype: "Data prototipe", accountUsage: ["Pendamping AI", "Pembuatan sorotan", "Ekspor suara"] },
  ar: { official: "رسمي", join: "انضم", watch: "شاهد", email: "البريد الإلكتروني", company: "الشركة", submitError: "تعذر الإرسال. حاول مرة أخرى لاحقاً.", copied: "تم النسخ", share: "مشاركة", scroll: "تابع الاستكشاف", prototype: "بيانات نموذج أولي", accountUsage: ["رفيق الذكاء الاصطناعي", "إنشاء اللقطات", "تصدير الصوت"] },
};

export function localeMeta(locale: string) {
  const market = marketPacks[locale as MarketLocale];
  return market ? { name: market.name, short: market.short, dir: market.dir ?? "ltr" } : null;
}

export function createMarketSiteCopies(base: SiteCopy): Record<MarketLocale, SiteCopy> {
  return Object.fromEntries(marketLocales.map((locale) => {
    const p = marketPacks[locale];
    const [home, explore, world, classes, regions, pulse, live, highlights, atlas, menu, language] = p.nav;
    const [top, bottom, description, enter, discover, online] = p.hero;
    const [atlasTitle, atlasCopy, worldTitle, worldCopy, classesTitle, classesCopy, warrior, sorcerer, taoist, globalTitle, globalCopy, updatesTitle] = p.core;
    const [watchTitle, watchCopy, observatory, privacyCopy, highlightsTitle, channelsTitle] = p.watch;
    const [memberTitle, memberCopy, monthly, annual, recommended, coming, free, wanderer, architect, usageTitle, fairnessTitle, faqTitle] = p.member;
    const [roadmap, community, press, support, status, account, privacy, terms, refunds, ai, founder, submit, consent, requirements, safety, more, unavailable, draft, updated] = p.labels;
    const copy: SiteCopy = {
      ...base,
      languageName: p.name,
      nav: { explore, world, classes, global: regions, pulse, watch: live, highlights, chronicles: updatesTitle, atlas, token: "TOKEN", locale: language, menu, home },
      hero: { eyebrow: `${world} // ${pulse}`, titleTop: top, titleBottom: bottom, description, primary: enter, secondary: discover, status: online },
      metrics: [{ value: "463", label: world }, { value: "03", label: classes }, { value: "01", label: pulse }],
      atlas: { ...base.atlas, eyebrow: `${atlas} // ${pulse}`, title: atlasTitle, copy: atlasCopy, button: atlas, live: online, metrics: base.atlas.metrics.map((metric, index) => ({ ...metric, label: [world, regions, pulse][index] })) },
      world: { ...base.world, eyebrow: world, title: worldTitle, copy: worldCopy, cards: base.world.cards.map((card, index) => ({ ...card, title: [world, highlights, regions][index], copy: worldCopy })) },
      classes: { ...base.classes, eyebrow: classes, title: classesTitle, copy: classesCopy, items: base.classes.items.map((item, index) => ({ ...item, title: [warrior, sorcerer, taoist][index], role: [warrior, sorcerer, taoist][index], copy: classesCopy })) },
      global: { ...base.global, eyebrow: regions, title: globalTitle, copy: globalCopy, markets: marketLocales.map((item) => marketPacks[item].name).slice(0, 4) },
      chronicles: { ...base.chronicles, eyebrow: updatesTitle, title: updatesTitle, items: base.chronicles.items.map((item) => ({ ...item, title: updatesTitle, copy: worldCopy })) },
      watch: { ...base.watch, eyebrow: live, title: watchTitle, copy: watchCopy, live, demo: draft, liveTitle: observatory, liveCopy: privacyCopy, enter, openAtlas: atlas, highlightsEyebrow: highlights, highlightsTitle, highlightsCopy: watchCopy, clips: base.watch.clips.map((clip, index) => ({ ...clip, tag: [world, pulse, classes][index], title: [highlightsTitle, watchTitle, worldTitle][index], copy: watchCopy })), channelsEyebrow: regions, channelsTitle, channelsCopy: globalCopy },
      membership: { ...base.membership, eyebrow: ai, title: memberTitle, copy: memberCopy, serviceBadge: account, prototypePrice: `${draft} · ${coming}`, monthly, annual, annualNote: annual, perMonth: `/ ${monthly}`, perYear: `/ ${annual}`, recommended, unavailable: coming, plans: base.membership.plans.map((plan, index) => ({ ...plan, name: [wanderer, "Rebirth Token", architect][index], monthlyPrice: index === 0 ? free : plan.monthlyPrice, annualPrice: index === 0 ? free : plan.annualPrice, tagline: memberCopy, features: [description, atlasCopy, worldCopy, globalCopy, watchCopy, p.legal[3]].slice(0, plan.features.length) })), usageEyebrow: ai, usageTitle, usageCopy: memberCopy, usage: base.membership.usage.map((item, index) => ({ ...item, label: [account, highlights, regions, community][index] })), fairnessEyebrow: safety, fairnessTitle, fairnessCopy: p.legal[3], fairnessPoints: [p.legal[1], p.legal[2], p.legal[3], memberCopy, p.footer], faqEyebrow: refunds, faqTitle, faq: [ai, fairnessTitle, refunds, privacy].map((question, index) => ({ question, answer: [p.legal[1], p.legal[3], p.legal[0], p.legal[2]][index] })) },
      finalCta: { eyebrow: enter, title: worldTitle, copy: description, button: enter },
      footer: { ...base.footer, note: p.footer, game: home, data: atlas, watch: live, regions, enter, worldPulse: pulse },
    };
    void roadmap; void press; void support; void status; void terms; void founder; void submit; void consent; void requirements; void safety; void more; void unavailable; void updated;
    return [locale, copy];
  })) as Record<MarketLocale, SiteCopy>;
}

export function createMarketProductCopies(base: ProductCopy): Record<MarketLocale, ProductCopy> {
  return Object.fromEntries(marketLocales.map((locale) => {
    const p = marketPacks[locale];
    const utility = marketUi[locale];
    const [, explore, world, classes, regions, pulse, live, highlights, atlas] = p.nav;
    const [, , description, enter] = p.hero;
    const [atlasTitle, atlasCopy, worldTitle, worldCopy, classesTitle, classesCopy, warrior, sorcerer, taoist, globalTitle, globalCopy, updatesTitle] = p.core;
    const [watchTitle, watchCopy] = p.watch;
    const [memberTitle, memberCopy] = p.member;
    const [roadmap, community, press, support, status, account, privacy, terms, refunds, ai, founder, submit, consent, requirements, safety, more, unavailable, draft, updated] = p.labels;
    const [legalSummary, legalData, legalBoundary, legalRights] = p.legal;
    const pages: ProductCopy["pages"] = {
      updates: { eyebrow: updatesTitle, title: updatesTitle, copy: worldCopy, cards: [] },
      roadmap: { eyebrow: roadmap, title: roadmap, copy: globalCopy, cards: base.pages.roadmap.cards.map((card, index) => ({ ...card, eyebrow: [status, updatesTitle, regions][index], title: [status, updatesTitle, regions][index], copy: globalCopy, label: more })) },
      community: { eyebrow: community, title: community, copy: worldCopy, cards: base.pages.community.cards.map((card, index) => ({ ...card, eyebrow: [enter, community, press][index], title: [enter, community, press][index], copy: worldCopy, label: more })) },
      press: { eyebrow: press, title: press, copy: globalCopy, cards: base.pages.press.cards.map((card, index) => ({ ...card, eyebrow: [press, highlights, atlas][index], title: [press, highlights, atlas][index], copy: globalCopy, label: more })) },
      support: { eyebrow: support, title: support, copy: legalBoundary, cards: base.pages.support.cards.map((card, index) => ({ ...card, eyebrow: [account, support, refunds][index], title: [account, support, refunds][index], copy: legalBoundary, label: more })) },
      status: { eyebrow: status, title: status, copy: legalSummary, cards: base.pages.status.cards.map((card, index) => ({ ...card, eyebrow: [status, enter, atlas, ai][index], title: [status, enter, atlas, ai][index], copy: legalSummary, label: more })) },
    };
    const legalSections = [{ title: privacy, copy: legalData }, { title: safety, copy: legalBoundary }, { title: terms, copy: legalRights }];
    const copy: ProductCopy = {
      ...base,
      chrome: { updates: updatesTitle, roadmap, community, press, support, status, account, legal: safety, privacy, terms, refunds, aiData: ai },
      proof: { eyebrow: world, title: worldTitle, copy: worldCopy, verified: updated, action: enter, items: [{ eyebrow: classes, title: classesTitle, copy: classesCopy }, { eyebrow: world, title: world, copy: worldCopy }, { eyebrow: atlas, title: atlas, copy: atlasCopy }] },
      play: { eyebrow: enter, title: worldTitle, copy: description, launch: enter, browser: explore, browserCopy: description, desktop: regions, desktopCopy: globalCopy, candidate: draft, candidateCopy: legalSummary, requirements, requirementItems: [requirements, description, globalCopy, safety], safetyTitle: safety, safetyPoints: [legalBoundary, legalData, legalRights, support] },
      membership: { ...base.membership, founderEyebrow: founder, founderTitle: founder, founderCopy: memberCopy, examplesTitle: memberTitle, examples: base.membership.examples.map((item, index) => ({ ...item, title: [account, highlights, regions][index], copy: [memberCopy, watchCopy, globalCopy][index] })), rollover: legalSummary, cancellation: legalRights, refund: legalRights, rights: legalRights, waitlist: founder, email: utility.email, region: regions, plan: memberTitle, consent, submit, submitted: legalRights, previewSubmitted: unavailable, submitError: utility.submitError, company: utility.company },
      watch: { archiveTitle: highlights, archiveCopy: watchCopy, all: pulse, gameplay: world, systems: atlas, world, share: utility.share, copied: utility.copied, footage: highlights, still: draft },
      account: { eyebrow: account, title: account, copy: memberCopy, signIn: enter, preview: draft, usageTitle: memberTitle, contentTitle: highlights },
      pages,
      updates: base.updates.map((entry, index) => ({ ...entry, title: [worldTitle, globalTitle, updatesTitle][index], summary: worldCopy, sections: entry.sections.map((section) => ({ ...section, title: updatesTitle, copy: worldCopy })) })),
      legal: {
        privacy: { title: privacy, summary: legalSummary, sections: legalSections },
        terms: { title: terms, summary: legalSummary, sections: legalSections },
        refunds: { title: refunds, summary: legalSummary, sections: legalSections },
        "ai-data": { title: ai, summary: legalSummary, sections: legalSections },
      },
      common: { updated, draft, learnMore: more, back: p.nav[0], unavailable },
      ui: { official: utility.official, supportWarning: legalBoundary, legalDisclaimer: legalRights, join: utility.join, watch: utility.watch, founderService: founder, usageContract: ai, aiService: ai, systemSafety: safety, scroll: utility.scroll, prototypeData: utility.prototype, publicDraft: `${terms} · ${draft}`, accountUsage: utility.accountUsage },
    };
    void live; void classes; void warrior; void sorcerer; void taoist; void watchTitle;
    return [locale, copy];
  })) as Record<MarketLocale, ProductCopy>;
}
