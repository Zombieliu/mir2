import { displayCopy, getDisplayMetrics } from "./atlas-display.mjs";
// Presentation data recovered from the public deployed bundle; see RECOVERY-PROVENANCE.json.
// No player, revenue, market, activity, capacity or verification data is connected.
export const locales = [
  "zh-CN",
  "en",
  "ja",
  "ko",
  "zh-TW",
  "es",
  "pt-BR",
  "de",
  "fr",
  "ru",
  "pl",
  "tr",
  "th",
  "vi",
  "id",
  "ar",
];
export const copy = {
  "zh-CN": {
    "name": "简体中文",
    "short": "简中",
    "dir": "ltr",
    "nav": [
      "总览",
      "世界",
      "角色",
      "公会",
      "图鉴",
      "市场"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "原型 · 暂无数据",
      "Numeron 于 2026 年 5 月开始开发，已邀请外部玩家测试，尚未正式上线。此 Atlas 是界面原型，未连接真实玩家、交易或世界事件数据。",
      "当前",
      "数据视图"
    ],
    "ui": [
      "探索传奇重生世界",
      "输入角色、公会、装备、地图、事件编号…",
      "匹配结果",
      "暂时没有找到相关记录。",
      "热门搜索",
      "在线冒险者",
      "活跃区域",
      "今日公开事件",
      "市场成交",
      "世界纪元",
      "当前进度",
      "下一阶段",
      "运行状态",
      "世界热度",
      "查看档案",
      "下一世界事件",
      "最新世界记录",
      "世界关注趋势",
      "市场热度",
      "数据来源与隐私",
      "原型 · 暂无数据",
      "概览",
      "关联档案"
    ],
    "kinds": [
      "角色",
      "公会",
      "装备",
      "物品",
      "地图",
      "怪物",
      "事件",
      "世界",
      "市场"
    ],
    "record": [
      "公开档案",
      "原型 · 暂无数据",
      "原型 · 暂无数据",
      "原型 · 暂无数据",
      "精确位置、私人资产、私聊和未授权身份不会公开。",
      "关键指标",
      "档案详情",
      "公开时间线",
      "返回世界总览",
      "原型 · 暂无数据"
    ]
  },
  "en": {
    "name": "English",
    "short": "EN",
    "dir": "ltr",
    "nav": [
      "Overview",
      "World",
      "Characters",
      "Guilds",
      "Codex",
      "Market"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "Prototype · No data",
      "Numeron development began in May 2026. External players have been invited to test; the game has not formally launched. This Atlas is an interface prototype with no connected player, trade or world-event data.",
      "Current",
      "data view"
    ],
    "ui": [
      "Explore Legend of Rebirth",
      "Search characters, guilds, gear, maps or event IDs…",
      "Matches",
      "No matching public record was found.",
      "Popular searches",
      "Adventurers online",
      "Active regions",
      "Public events today",
      "Market volume",
      "World era",
      "Current progress",
      "Next stage",
      "Service status",
      "World activity",
      "View record",
      "Next world event",
      "Latest world records",
      "Discovery trends",
      "Market pulse",
      "Sources and privacy",
      "Prototype · No data",
      "Overview",
      "Related records"
    ],
    "kinds": [
      "Character",
      "Guild",
      "Equipment",
      "Item",
      "Map",
      "Monster",
      "Event",
      "World",
      "Market"
    ],
    "record": [
      "Public record",
      "Prototype · No data",
      "Prototype · No data",
      "Prototype · No data",
      "Exact locations, private assets, private chat and unauthorized identities stay private.",
      "Key metrics",
      "Record details",
      "Public timeline",
      "Back to world overview",
      "Prototype · No data"
    ]
  },
  "ja": {
    "name": "日本語",
    "short": "JA",
    "dir": "ltr",
    "nav": [
      "概要",
      "世界",
      "キャラクター",
      "ギルド",
      "図鑑",
      "市場"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "試作 · データなし",
      "Numeron は2026年5月に開発を開始し、外部プレイヤーをテストに招待しています。正式公開はまだです。この Atlas は画面の試作で、実際のプレイヤー、取引、世界イベントのデータには接続していません。",
      "現在",
      "データ表示"
    ],
    "ui": [
      "Legend of Rebirthを探索",
      "キャラクター、ギルド、装備、マップ、イベントIDを検索…",
      "検索結果",
      "一致する公開記録はありません。",
      "人気の検索",
      "オンライン冒険者",
      "活動地域",
      "本日の公開イベント",
      "市場取引",
      "ワールド紀元",
      "現在の進行",
      "次の段階",
      "稼働状況",
      "ワールド動向",
      "記録を見る",
      "次のワールドイベント",
      "最新ワールド記録",
      "注目トレンド",
      "市場動向",
      "データソースとプライバシー",
      "試作 · データなし",
      "概要",
      "関連記録"
    ],
    "kinds": [
      "キャラクター",
      "ギルド",
      "装備",
      "アイテム",
      "マップ",
      "モンスター",
      "イベント",
      "世界",
      "市場"
    ],
    "record": [
      "公開記録",
      "試作 · データなし",
      "試作 · データなし",
      "試作 · データなし",
      "正確な位置、非公開資産、私信、未許可の本人情報は公開しません。",
      "主要指標",
      "記録詳細",
      "公開タイムライン",
      "世界概要へ戻る",
      "試作 · データなし"
    ]
  },
  "ko": {
    "name": "한국어",
    "short": "KO",
    "dir": "ltr",
    "nav": [
      "개요",
      "세계",
      "캐릭터",
      "길드",
      "도감",
      "시장"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "프로토타입 · 데이터 없음",
      "Numeron은 2026년 5월 개발을 시작했고 외부 플레이어를 테스트에 초대했습니다. 아직 정식 출시되지 않았습니다. 이 Atlas는 실제 플레이어, 거래 또는 월드 이벤트 데이터에 연결되지 않은 화면 프로토타입입니다.",
      "현재",
      "데이터 보기"
    ],
    "ui": [
      "Legend of Rebirth 탐색",
      "캐릭터, 길드, 장비, 지도 또는 이벤트 ID 검색…",
      "검색 결과",
      "일치하는 공개 기록이 없습니다.",
      "인기 검색",
      "온라인 모험가",
      "활성 지역",
      "오늘의 공개 이벤트",
      "시장 거래",
      "월드 시대",
      "현재 진행도",
      "다음 단계",
      "서비스 상태",
      "월드 활동",
      "기록 보기",
      "다음 월드 이벤트",
      "최신 월드 기록",
      "관심 동향",
      "시장 동향",
      "데이터 출처와 개인정보",
      "프로토타입 · 데이터 없음",
      "개요",
      "관련 기록"
    ],
    "kinds": [
      "캐릭터",
      "길드",
      "장비",
      "아이템",
      "지도",
      "몬스터",
      "이벤트",
      "세계",
      "시장"
    ],
    "record": [
      "공개 기록",
      "프로토타입 · 데이터 없음",
      "프로토타입 · 데이터 없음",
      "프로토타입 · 데이터 없음",
      "정확한 위치, 비공개 자산, 개인 대화와 허가받지 않은 신원은 공개하지 않습니다.",
      "주요 지표",
      "기록 상세",
      "공개 타임라인",
      "월드 개요로 돌아가기",
      "프로토타입 · 데이터 없음"
    ]
  },
  "zh-TW": {
    "name": "繁體中文",
    "short": "繁中",
    "dir": "ltr",
    "nav": [
      "總覽",
      "世界",
      "角色",
      "公會",
      "圖鑑",
      "市場"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "原型 · 暫無資料",
      "Numeron 於 2026 年 5 月開始開發，已邀請外部玩家測試，尚未正式上線。此 Atlas 為介面原型，未連接真實玩家、交易或世界事件資料。",
      "目前",
      "資料檢視"
    ],
    "ui": [
      "探索傳奇重生世界",
      "輸入角色、公會、裝備、地圖、事件編號…",
      "符合結果",
      "暫時沒有找到相關公開紀錄。",
      "熱門搜尋",
      "線上冒險者",
      "活躍地區",
      "今日公開事件",
      "市場成交",
      "世界紀元",
      "目前進度",
      "下一階段",
      "運作狀態",
      "世界熱度",
      "查看檔案",
      "下一個世界事件",
      "最新世界紀錄",
      "世界關注趨勢",
      "市場熱度",
      "資料來源與隱私",
      "原型 · 暫無資料",
      "概覽",
      "關聯檔案"
    ],
    "kinds": [
      "角色",
      "公會",
      "裝備",
      "物品",
      "地圖",
      "怪物",
      "事件",
      "世界",
      "市場"
    ],
    "record": [
      "公開檔案",
      "原型 · 暫無資料",
      "原型 · 暫無資料",
      "原型 · 暫無資料",
      "精確位置、私人資產、私聊和未授權身分不會公開。",
      "關鍵指標",
      "檔案詳情",
      "公開時間軸",
      "返回世界總覽",
      "原型 · 暫無資料"
    ]
  },
  "es": {
    "name": "Español",
    "short": "ES",
    "dir": "ltr",
    "nav": [
      "Resumen",
      "Mundo",
      "Personajes",
      "Gremios",
      "Códice",
      "Mercado"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "Prototipo · Sin datos",
      "Numeron comenzó a desarrollarse en mayo de 2026. Se ha invitado a jugadores externos a probarlo; aún no se ha lanzado oficialmente. Este Atlas es un prototipo sin conexión a datos de jugadores, operaciones o eventos del mundo.",
      "Actual",
      "vista de datos"
    ],
    "ui": [
      "Explorar Legend of Rebirth",
      "Buscar personajes, gremios, equipo, mapas o eventos…",
      "Resultados",
      "No se encontró ningún registro público.",
      "Búsquedas populares",
      "Aventureros en línea",
      "Regiones activas",
      "Eventos públicos hoy",
      "Volumen de mercado",
      "Era mundial",
      "Progreso actual",
      "Siguiente etapa",
      "Estado del servicio",
      "Actividad mundial",
      "Ver registro",
      "Próximo evento mundial",
      "Últimos registros",
      "Tendencias",
      "Pulso del mercado",
      "Fuentes y privacidad",
      "Prototipo · Sin datos",
      "Resumen",
      "Registros relacionados"
    ],
    "kinds": [
      "Personaje",
      "Gremio",
      "Equipo",
      "Objeto",
      "Mapa",
      "Monstruo",
      "Evento",
      "Mundo",
      "Mercado"
    ],
    "record": [
      "Registro público",
      "Prototipo · Sin datos",
      "Prototipo · Sin datos",
      "Prototipo · Sin datos",
      "Las ubicaciones exactas, activos privados, chat privado e identidades no autorizadas permanecen ocultos.",
      "Métricas clave",
      "Detalles",
      "Cronología pública",
      "Volver al resumen",
      "Prototipo · Sin datos"
    ]
  },
  "pt-BR": {
    "name": "Português (Brasil)",
    "short": "PT-BR",
    "dir": "ltr",
    "nav": [
      "Visão geral",
      "Mundo",
      "Personagens",
      "Guildas",
      "Códice",
      "Mercado"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "Protótipo · Sem dados",
      "O desenvolvimento do Numeron começou em maio de 2026. Jogadores externos foram convidados para testes; o jogo ainda não foi lançado oficialmente. Este Atlas é um protótipo sem conexão a dados de jogadores, negociações ou eventos do mundo.",
      "Atual",
      "visão de dados"
    ],
    "ui": [
      "Explorar Legend of Rebirth",
      "Buscar personagens, guildas, itens, mapas ou eventos…",
      "Resultados",
      "Nenhum registro público correspondente foi encontrado.",
      "Buscas populares",
      "Aventureiros online",
      "Regiões ativas",
      "Eventos públicos hoje",
      "Volume de mercado",
      "Era mundial",
      "Progresso atual",
      "Próxima etapa",
      "Status do serviço",
      "Atividade mundial",
      "Ver registro",
      "Próximo evento mundial",
      "Registros mais recentes",
      "Tendências",
      "Pulso do mercado",
      "Fontes e privacidade",
      "Protótipo · Sem dados",
      "Visão geral",
      "Registros relacionados"
    ],
    "kinds": [
      "Personagem",
      "Guilda",
      "Equipamento",
      "Item",
      "Mapa",
      "Monstro",
      "Evento",
      "Mundo",
      "Mercado"
    ],
    "record": [
      "Registro público",
      "Protótipo · Sem dados",
      "Protótipo · Sem dados",
      "Protótipo · Sem dados",
      "Localizações exatas, ativos privados, chat privado e identidades não autorizadas permanecem privados.",
      "Métricas principais",
      "Detalhes",
      "Linha do tempo pública",
      "Voltar à visão geral",
      "Protótipo · Sem dados"
    ]
  },
  "de": {
    "name": "Deutsch",
    "short": "DE",
    "dir": "ltr",
    "nav": [
      "Übersicht",
      "Welt",
      "Charaktere",
      "Gilden",
      "Kodex",
      "Markt"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "Prototyp · Keine Daten",
      "Die Entwicklung von Numeron begann im Mai 2026. Externe Spieler wurden zu Tests eingeladen; ein offizieller Start steht noch aus. Dieser Atlas ist ein Oberflächenprototyp ohne Verbindung zu Spieler-, Handels- oder Weltereignisdaten.",
      "Aktuell",
      "Datenansicht"
    ],
    "ui": [
      "Legend of Rebirth erkunden",
      "Charaktere, Gilden, Ausrüstung, Karten oder Ereignisse suchen…",
      "Ergebnisse",
      "Kein passender öffentlicher Eintrag gefunden.",
      "Beliebte Suchen",
      "Abenteurer online",
      "Aktive Regionen",
      "Öffentliche Ereignisse heute",
      "Marktvolumen",
      "Weltära",
      "Aktueller Fortschritt",
      "Nächste Phase",
      "Dienststatus",
      "Weltaktivität",
      "Eintrag öffnen",
      "Nächstes Weltereignis",
      "Neueste Welteinträge",
      "Entdeckungstrends",
      "Marktpuls",
      "Quellen und Datenschutz",
      "Prototyp · Keine Daten",
      "Übersicht",
      "Verknüpfte Einträge"
    ],
    "kinds": [
      "Charakter",
      "Gilde",
      "Ausrüstung",
      "Gegenstand",
      "Karte",
      "Monster",
      "Ereignis",
      "Welt",
      "Markt"
    ],
    "record": [
      "Öffentlicher Eintrag",
      "Prototyp · Keine Daten",
      "Prototyp · Keine Daten",
      "Prototyp · Keine Daten",
      "Genaue Orte, private Güter, private Chats und nicht freigegebene Identitäten bleiben privat.",
      "Kennzahlen",
      "Details",
      "Öffentliche Zeitleiste",
      "Zur Weltübersicht",
      "Prototyp · Keine Daten"
    ]
  },
  "fr": {
    "name": "Français",
    "short": "FR",
    "dir": "ltr",
    "nav": [
      "Vue d’ensemble",
      "Monde",
      "Personnages",
      "Guildes",
      "Codex",
      "Marché"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "Prototype · Aucune donnée",
      "Le développement de Numeron a commencé en mai 2026. Des joueurs externes ont été invités aux tests ; le jeu n'est pas encore officiellement lancé. Cet Atlas est un prototype sans connexion aux données de joueurs, de transactions ou d'événements du monde.",
      "Actuel",
      "vue des données"
    ],
    "ui": [
      "Explorer Legend of Rebirth",
      "Rechercher personnages, guildes, équipement, cartes ou événements…",
      "Résultats",
      "Aucun dossier public correspondant.",
      "Recherches populaires",
      "Aventuriers en ligne",
      "Régions actives",
      "Événements publics aujourd’hui",
      "Volume du marché",
      "Ère du monde",
      "Progression actuelle",
      "Étape suivante",
      "État du service",
      "Activité du monde",
      "Voir le dossier",
      "Prochain événement mondial",
      "Derniers registres",
      "Tendances",
      "Pouls du marché",
      "Sources et confidentialité",
      "Prototype · Aucune donnée",
      "Vue d’ensemble",
      "Dossiers liés"
    ],
    "kinds": [
      "Personnage",
      "Guilde",
      "Équipement",
      "Objet",
      "Carte",
      "Monstre",
      "Événement",
      "Monde",
      "Marché"
    ],
    "record": [
      "Dossier public",
      "Prototype · Aucune donnée",
      "Prototype · Aucune donnée",
      "Prototype · Aucune donnée",
      "Les positions exactes, biens privés, messages privés et identités non autorisées restent privés.",
      "Indicateurs clés",
      "Détails",
      "Chronologie publique",
      "Retour à la vue mondiale",
      "Prototype · Aucune donnée"
    ]
  },
  "ru": {
    "name": "Русский",
    "short": "RU",
    "dir": "ltr",
    "nav": [
      "Обзор",
      "Мир",
      "Персонажи",
      "Гильдии",
      "Справочник",
      "Рынок"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "Прототип · Нет данных",
      "Разработка Numeron началась в мае 2026 года. Внешние игроки приглашены на тесты; официального запуска ещё не было. Этот Атлас — прототип интерфейса без подключения к данным игроков, торговли или мировых событий.",
      "Текущий",
      "режим данных"
    ],
    "ui": [
      "Исследовать Legend of Rebirth",
      "Поиск персонажей, гильдий, предметов, карт или событий…",
      "Результаты",
      "Подходящая открытая запись не найдена.",
      "Популярные запросы",
      "Искатели онлайн",
      "Активные регионы",
      "Открытые события сегодня",
      "Объём рынка",
      "Эра мира",
      "Текущий прогресс",
      "Следующий этап",
      "Статус сервиса",
      "Активность мира",
      "Открыть запись",
      "Следующее событие мира",
      "Последние записи",
      "Тренды",
      "Пульс рынка",
      "Источники и приватность",
      "Прототип · Нет данных",
      "Обзор",
      "Связанные записи"
    ],
    "kinds": [
      "Персонаж",
      "Гильдия",
      "Снаряжение",
      "Предмет",
      "Карта",
      "Монстр",
      "Событие",
      "Мир",
      "Рынок"
    ],
    "record": [
      "Открытая запись",
      "Прототип · Нет данных",
      "Прототип · Нет данных",
      "Прототип · Нет данных",
      "Точные координаты, личные вещи, личный чат и неподтверждённые личности скрыты.",
      "Ключевые показатели",
      "Подробности",
      "Открытая хронология",
      "Назад к обзору мира",
      "Прототип · Нет данных"
    ]
  },
  "pl": {
    "name": "Polski",
    "short": "PL",
    "dir": "ltr",
    "nav": [
      "Przegląd",
      "Świat",
      "Postacie",
      "Gildie",
      "Kodeks",
      "Rynek"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "Prototyp · Brak danych",
      "Prace nad Numeron rozpoczęły się w maju 2026 r. Zaproszono zewnętrznych graczy do testów; oficjalna premiera jeszcze się nie odbyła. Ten Atlas to prototyp interfejsu bez połączenia z danymi graczy, handlu i wydarzeń świata.",
      "Bieżący",
      "widok danych"
    ],
    "ui": [
      "Odkrywaj Legend of Rebirth",
      "Szukaj postaci, gildii, sprzętu, map lub wydarzeń…",
      "Wyniki",
      "Nie znaleziono pasującego publicznego wpisu.",
      "Popularne wyszukiwania",
      "Poszukiwacze online",
      "Aktywne regiony",
      "Dzisiejsze wydarzenia",
      "Wolumen rynku",
      "Era świata",
      "Bieżący postęp",
      "Następny etap",
      "Stan usługi",
      "Aktywność świata",
      "Zobacz wpis",
      "Następne wydarzenie",
      "Najnowsze wpisy",
      "Trendy",
      "Puls rynku",
      "Źródła i prywatność",
      "Prototyp · Brak danych",
      "Przegląd",
      "Powiązane wpisy"
    ],
    "kinds": [
      "Postać",
      "Gildia",
      "Wyposażenie",
      "Przedmiot",
      "Mapa",
      "Potwór",
      "Wydarzenie",
      "Świat",
      "Rynek"
    ],
    "record": [
      "Publiczny wpis",
      "Prototyp · Brak danych",
      "Prototyp · Brak danych",
      "Prototyp · Brak danych",
      "Dokładne pozycje, prywatne zasoby, rozmowy i nieautoryzowane tożsamości pozostają prywatne.",
      "Kluczowe metryki",
      "Szczegóły",
      "Publiczna oś czasu",
      "Wróć do świata",
      "Prototyp · Brak danych"
    ]
  },
  "tr": {
    "name": "Türkçe",
    "short": "TR",
    "dir": "ltr",
    "nav": [
      "Genel bakış",
      "Dünya",
      "Karakterler",
      "Loncalar",
      "Rehber",
      "Pazar"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "Prototip · Veri yok",
      "Numeron'un geliştirilmesine Mayıs 2026'da başlandı. Dış oyuncular teste davet edildi; oyun henüz resmen yayımlanmadı. Bu Atlas oyuncu, ticaret veya dünya olayı verilerine bağlı olmayan bir arayüz prototipidir.",
      "Geçerli",
      "veri görünümü"
    ],
    "ui": [
      "Legend of Rebirth’i keşfet",
      "Karakter, lonca, ekipman, harita veya etkinlik ara…",
      "Sonuçlar",
      "Eşleşen açık kayıt bulunamadı.",
      "Popüler aramalar",
      "Çevrimiçi maceracılar",
      "Etkin bölgeler",
      "Bugünkü açık etkinlikler",
      "Pazar hacmi",
      "Dünya çağı",
      "Geçerli ilerleme",
      "Sonraki aşama",
      "Hizmet durumu",
      "Dünya etkinliği",
      "Kaydı aç",
      "Sonraki dünya etkinliği",
      "Son dünya kayıtları",
      "Keşif eğilimleri",
      "Pazar nabzı",
      "Kaynaklar ve gizlilik",
      "Prototip · Veri yok",
      "Genel bakış",
      "İlgili kayıtlar"
    ],
    "kinds": [
      "Karakter",
      "Lonca",
      "Ekipman",
      "Eşya",
      "Harita",
      "Canavar",
      "Etkinlik",
      "Dünya",
      "Pazar"
    ],
    "record": [
      "Açık kayıt",
      "Prototip · Veri yok",
      "Prototip · Veri yok",
      "Prototip · Veri yok",
      "Kesin konumlar, özel varlıklar, özel sohbet ve izinsiz kimlikler gizli kalır.",
      "Temel ölçümler",
      "Ayrıntılar",
      "Açık zaman çizelgesi",
      "Dünya görünümüne dön",
      "Prototip · Veri yok"
    ]
  },
  "th": {
    "name": "ไทย",
    "short": "TH",
    "dir": "ltr",
    "nav": [
      "ภาพรวม",
      "โลก",
      "ตัวละคร",
      "กิลด์",
      "สารานุกรม",
      "ตลาด"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "ต้นแบบ · ยังไม่มีข้อมูล",
      "Numeron เริ่มพัฒนาในเดือนพฤษภาคม 2026 และได้เชิญผู้เล่นภายนอกเข้าทดสอบแล้ว แต่ยังไม่เปิดตัวอย่างเป็นทางการ Atlas นี้เป็นต้นแบบหน้าจอที่ไม่ได้เชื่อมต่อข้อมูลผู้เล่น การซื้อขาย หรือเหตุการณ์โลกจริง",
      "ปัจจุบัน",
      "มุมมองข้อมูล"
    ],
    "ui": [
      "สำรวจ Legend of Rebirth",
      "ค้นหาตัวละคร กิลด์ อุปกรณ์ แผนที่ หรือเหตุการณ์…",
      "ผลลัพธ์",
      "ไม่พบระเบียนสาธารณะที่ตรงกัน",
      "การค้นหายอดนิยม",
      "นักผจญภัยออนไลน์",
      "พื้นที่ที่ใช้งาน",
      "เหตุการณ์สาธารณะวันนี้",
      "ปริมาณตลาด",
      "ยุคของโลก",
      "ความคืบหน้าปัจจุบัน",
      "ขั้นถัดไป",
      "สถานะบริการ",
      "กิจกรรมโลก",
      "ดูระเบียน",
      "เหตุการณ์โลกถัดไป",
      "ระเบียนโลกล่าสุด",
      "แนวโน้ม",
      "ชีพจรตลาด",
      "แหล่งข้อมูลและความเป็นส่วนตัว",
      "ต้นแบบ · ยังไม่มีข้อมูล",
      "ภาพรวม",
      "ระเบียนที่เกี่ยวข้อง"
    ],
    "kinds": [
      "ตัวละคร",
      "กิลด์",
      "อุปกรณ์",
      "ไอเทม",
      "แผนที่",
      "มอนสเตอร์",
      "เหตุการณ์",
      "โลก",
      "ตลาด"
    ],
    "record": [
      "ระเบียนสาธารณะ",
      "ต้นแบบ · ยังไม่มีข้อมูล",
      "ต้นแบบ · ยังไม่มีข้อมูล",
      "ต้นแบบ · ยังไม่มีข้อมูล",
      "ตำแหน่งที่แน่นอน ทรัพย์สินส่วนตัว แชตส่วนตัว และตัวตนที่ไม่ได้รับอนุญาตจะไม่ถูกเปิดเผย",
      "ตัวชี้วัดหลัก",
      "รายละเอียด",
      "ไทม์ไลน์สาธารณะ",
      "กลับสู่ภาพรวมโลก",
      "ต้นแบบ · ยังไม่มีข้อมูล"
    ]
  },
  "vi": {
    "name": "Tiếng Việt",
    "short": "VI",
    "dir": "ltr",
    "nav": [
      "Tổng quan",
      "Thế giới",
      "Nhân vật",
      "Bang hội",
      "Cẩm nang",
      "Thị trường"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "Nguyên mẫu · Chưa có dữ liệu",
      "Numeron bắt đầu phát triển vào tháng 5 năm 2026. Người chơi bên ngoài đã được mời thử nghiệm; trò chơi chưa chính thức ra mắt. Atlas này là nguyên mẫu giao diện, chưa kết nối dữ liệu người chơi, giao dịch hoặc sự kiện thế giới thực.",
      "Hiện tại",
      "chế độ dữ liệu"
    ],
    "ui": [
      "Khám phá Legend of Rebirth",
      "Tìm nhân vật, bang hội, trang bị, bản đồ hoặc sự kiện…",
      "Kết quả",
      "Không tìm thấy hồ sơ công khai phù hợp.",
      "Tìm kiếm phổ biến",
      "Nhà thám hiểm trực tuyến",
      "Khu vực hoạt động",
      "Sự kiện công khai hôm nay",
      "Khối lượng thị trường",
      "Kỷ nguyên thế giới",
      "Tiến độ hiện tại",
      "Giai đoạn tiếp theo",
      "Trạng thái dịch vụ",
      "Hoạt động thế giới",
      "Xem hồ sơ",
      "Sự kiện thế giới tiếp theo",
      "Hồ sơ mới nhất",
      "Xu hướng",
      "Nhịp thị trường",
      "Nguồn dữ liệu và quyền riêng tư",
      "Nguyên mẫu · Chưa có dữ liệu",
      "Tổng quan",
      "Hồ sơ liên quan"
    ],
    "kinds": [
      "Nhân vật",
      "Bang hội",
      "Trang bị",
      "Vật phẩm",
      "Bản đồ",
      "Quái vật",
      "Sự kiện",
      "Thế giới",
      "Thị trường"
    ],
    "record": [
      "Hồ sơ công khai",
      "Nguyên mẫu · Chưa có dữ liệu",
      "Nguyên mẫu · Chưa có dữ liệu",
      "Nguyên mẫu · Chưa có dữ liệu",
      "Vị trí chính xác, tài sản riêng, trò chuyện riêng và danh tính chưa được phép luôn được bảo vệ.",
      "Chỉ số chính",
      "Chi tiết",
      "Dòng thời gian công khai",
      "Về tổng quan thế giới",
      "Nguyên mẫu · Chưa có dữ liệu"
    ]
  },
  "id": {
    "name": "Bahasa Indonesia",
    "short": "ID",
    "dir": "ltr",
    "nav": [
      "Ringkasan",
      "Dunia",
      "Karakter",
      "Guild",
      "Kodeks",
      "Pasar"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "Prototipe · Belum ada data",
      "Pengembangan Numeron dimulai pada Mei 2026. Pemain eksternal telah diundang untuk menguji; game belum diluncurkan resmi. Atlas ini adalah prototipe antarmuka tanpa koneksi ke data pemain, perdagangan, atau peristiwa dunia nyata.",
      "Saat ini",
      "tampilan data"
    ],
    "ui": [
      "Jelajahi Legend of Rebirth",
      "Cari karakter, guild, perlengkapan, peta, atau peristiwa…",
      "Hasil",
      "Tidak ada catatan publik yang cocok.",
      "Pencarian populer",
      "Petualang online",
      "Wilayah aktif",
      "Peristiwa publik hari ini",
      "Volume pasar",
      "Era dunia",
      "Progres saat ini",
      "Tahap berikutnya",
      "Status layanan",
      "Aktivitas dunia",
      "Lihat catatan",
      "Peristiwa dunia berikutnya",
      "Catatan dunia terbaru",
      "Tren penemuan",
      "Denyut pasar",
      "Sumber dan privasi",
      "Prototipe · Belum ada data",
      "Ringkasan",
      "Catatan terkait"
    ],
    "kinds": [
      "Karakter",
      "Guild",
      "Perlengkapan",
      "Item",
      "Peta",
      "Monster",
      "Peristiwa",
      "Dunia",
      "Pasar"
    ],
    "record": [
      "Catatan publik",
      "Prototipe · Belum ada data",
      "Prototipe · Belum ada data",
      "Prototipe · Belum ada data",
      "Lokasi tepat, aset pribadi, chat pribadi, dan identitas tanpa izin tetap privat.",
      "Metrik utama",
      "Detail",
      "Linimasa publik",
      "Kembali ke ringkasan dunia",
      "Prototipe · Belum ada data"
    ]
  },
  "ar": {
    "name": "العربية",
    "short": "AR",
    "dir": "rtl",
    "nav": [
      "نظرة عامة",
      "العالم",
      "الشخصيات",
      "النقابات",
      "الدليل",
      "السوق"
    ],
    "hero": [
      "NUMERON ATLAS",
      "NUMERON",
      "نموذج أولي · لا بيانات",
      "بدأ تطوير Numeron في مايو 2026. تمت دعوة لاعبين خارجيين للاختبار ولم تُطلق اللعبة رسميًا بعد. هذا الأطلس نموذج واجهة غير متصل ببيانات اللاعبين أو التداول أو أحداث العالم الحقيقية.",
      "الحالي",
      "عرض البيانات"
    ],
    "ui": [
      "استكشف Legend of Rebirth",
      "ابحث عن شخصية أو نقابة أو معدات أو خريطة أو حدث…",
      "النتائج",
      "لم يتم العثور على سجل عام مطابق.",
      "عمليات البحث الشائعة",
      "المغامرون المتصلون",
      "المناطق النشطة",
      "الأحداث العامة اليوم",
      "حجم السوق",
      "حقبة العالم",
      "التقدم الحالي",
      "المرحلة التالية",
      "حالة الخدمة",
      "نشاط العالم",
      "عرض السجل",
      "الحدث العالمي التالي",
      "أحدث سجلات العالم",
      "الاتجاهات",
      "نبض السوق",
      "المصادر والخصوصية",
      "نموذج أولي · لا بيانات",
      "نظرة عامة",
      "السجلات المرتبطة"
    ],
    "kinds": [
      "شخصية",
      "نقابة",
      "معدات",
      "عنصر",
      "خريطة",
      "وحش",
      "حدث",
      "عالم",
      "سوق"
    ],
    "record": [
      "سجل عام",
      "نموذج أولي · لا بيانات",
      "نموذج أولي · لا بيانات",
      "نموذج أولي · لا بيانات",
      "تبقى المواقع الدقيقة والأصول والمحادثات الخاصة والهويات غير المصرح بها محمية.",
      "المقاييس الرئيسية",
      "التفاصيل",
      "الخط الزمني العام",
      "العودة إلى نظرة العالم",
      "نموذج أولي · لا بيانات"
    ]
  }
};
export const facts = Object.fromEntries(locales.map(locale => [locale, [copy[locale].hero[2], displayCopy[locale].summary]]));
for (const locale of locales) copy[locale].hero[3] = facts[locale][1];
export const archiveKinds = [
  "character",
  "guild",
  "equipment",
  "item",
  "map",
  "monster",
  "event",
  "world",
  "market",
];
export const archiveRecords = [
  {
    kind: "equipment",
    slug: "dragon-slayer",
    tone: "gold",
    related: [
      {
        kind: "market",
        slug: "world-01",
      },
      {
        kind: "monster",
        slug: "zuma-leader",
      },
      {
        kind: "character",
        slug: "cangyue-lone-wolf",
      },
    ],
  },
  {
    kind: "equipment",
    slug: "judgement-mace",
    tone: "gold",
    related: [
      {
        kind: "monster",
        slug: "zuma-leader",
      },
      {
        kind: "character",
        slug: "cangyue-lone-wolf",
      },
      {
        kind: "market",
        slug: "world-01",
      },
    ],
  },
  {
    kind: "character",
    slug: "cangyue-lone-wolf",
    tone: "cyan",
    related: [
      {
        kind: "guild",
        slug: "dragon-soul-hall",
      },
      {
        kind: "event",
        slug: "sabuk-0826",
      },
      {
        kind: "equipment",
        slug: "judgement-mace",
      },
    ],
  },
  {
    kind: "character",
    slug: "qingfeng-taoist",
    tone: "green",
    related: [
      {
        kind: "map",
        slug: "red-moon-gorge",
      },
      {
        kind: "monster",
        slug: "red-moon-demon",
      },
      {
        kind: "world",
        slug: "world-01",
      },
    ],
  },
  {
    kind: "guild",
    slug: "dragon-soul-hall",
    tone: "red",
    related: [
      {
        kind: "character",
        slug: "cangyue-lone-wolf",
      },
      {
        kind: "map",
        slug: "sabuk",
      },
      {
        kind: "event",
        slug: "sabuk-0826",
      },
    ],
  },
  {
    kind: "map",
    slug: "red-moon-gorge",
    tone: "red",
    related: [
      {
        kind: "monster",
        slug: "red-moon-demon",
      },
      {
        kind: "character",
        slug: "qingfeng-taoist",
      },
      {
        kind: "world",
        slug: "world-01",
      },
    ],
  },
  {
    kind: "map",
    slug: "sabuk",
    tone: "red",
    related: [
      {
        kind: "event",
        slug: "sabuk-0826",
      },
      {
        kind: "guild",
        slug: "dragon-soul-hall",
      },
      {
        kind: "character",
        slug: "cangyue-lone-wolf",
      },
    ],
  },
  {
    kind: "map",
    slug: "bichon-province",
    tone: "green",
    related: [
      {
        kind: "world",
        slug: "world-01",
      },
      {
        kind: "market",
        slug: "world-01",
      },
      {
        kind: "map",
        slug: "mengzhong-province",
      },
    ],
  },
  {
    kind: "map",
    slug: "mengzhong-province",
    tone: "gold",
    related: [
      {
        kind: "monster",
        slug: "zuma-leader",
      },
      {
        kind: "equipment",
        slug: "judgement-mace",
      },
      {
        kind: "market",
        slug: "world-01",
      },
    ],
  },
  {
    kind: "monster",
    slug: "red-moon-demon",
    tone: "red",
    related: [
      {
        kind: "map",
        slug: "red-moon-gorge",
      },
      {
        kind: "character",
        slug: "qingfeng-taoist",
      },
      {
        kind: "world",
        slug: "world-01",
      },
    ],
  },
  {
    kind: "monster",
    slug: "zuma-leader",
    tone: "gold",
    related: [
      {
        kind: "equipment",
        slug: "judgement-mace",
      },
      {
        kind: "character",
        slug: "cangyue-lone-wolf",
      },
      {
        kind: "map",
        slug: "mengzhong-province",
      },
    ],
  },
  {
    kind: "event",
    slug: "sabuk-0826",
    tone: "red",
    related: [
      {
        kind: "guild",
        slug: "dragon-soul-hall",
      },
      {
        kind: "map",
        slug: "sabuk",
      },
      {
        kind: "character",
        slug: "cangyue-lone-wolf",
      },
    ],
  },
  {
    kind: "item",
    slug: "paralysis-ring",
    tone: "cyan",
    related: [
      {
        kind: "market",
        slug: "world-01",
      },
      {
        kind: "world",
        slug: "world-01",
      },
      {
        kind: "equipment",
        slug: "dragon-slayer",
      },
    ],
  },
  {
    kind: "item",
    slug: "black-iron-ore",
    tone: "cyan",
    related: [
      {
        kind: "market",
        slug: "world-01",
      },
      {
        kind: "map",
        slug: "mengzhong-province",
      },
      {
        kind: "item",
        slug: "blessing-oil",
      },
    ],
  },
  {
    kind: "item",
    slug: "blessing-oil",
    tone: "green",
    related: [
      {
        kind: "market",
        slug: "world-01",
      },
      {
        kind: "equipment",
        slug: "judgement-mace",
      },
      {
        kind: "item",
        slug: "black-iron-ore",
      },
    ],
  },
  {
    kind: "item",
    slug: "zuma-portrait",
    tone: "gold",
    related: [
      {
        kind: "monster",
        slug: "zuma-leader",
      },
      {
        kind: "event",
        slug: "sabuk-0826",
      },
      {
        kind: "market",
        slug: "world-01",
      },
    ],
  },
  {
    kind: "world",
    slug: "world-01",
    tone: "cyan",
    related: [
      {
        kind: "event",
        slug: "sabuk-0826",
      },
      {
        kind: "map",
        slug: "red-moon-gorge",
      },
      {
        kind: "market",
        slug: "world-01",
      },
    ],
  },
  {
    kind: "market",
    slug: "world-01",
    tone: "green",
    related: [
      {
        kind: "item",
        slug: "black-iron-ore",
      },
      {
        kind: "item",
        slug: "paralysis-ring",
      },
      {
        kind: "equipment",
        slug: "dragon-slayer",
      },
    ],
  },
];
export const searchRecords = [
  ["equipment", "dragon-slayer"],
  ["character", "cangyue-lone-wolf"],
  ["guild", "dragon-soul-hall"],
  ["map", "red-moon-gorge"],
  ["monster", "red-moon-demon"],
];
export const ledgerRecords = [
  ["equipment", "judgement-mace", "gold"],
  ["event", "sabuk-0826", "red"],
  ["item", "paralysis-ring", "cyan"],
  ["character", "qingfeng-taoist", "green"],
  ["map", "red-moon-gorge", "red"],
];
export const trendRecords = [
  [
    ["equipment", "judgement-mace"],
    ["equipment", "dragon-slayer"],
    ["item", "paralysis-ring"],
  ],
  [
    ["guild", "dragon-soul-hall"],
    ["event", "sabuk-0826"],
    ["map", "sabuk"],
  ],
  [
    ["character", "cangyue-lone-wolf"],
    ["character", "qingfeng-taoist"],
    ["world", "world-01"],
  ],
  [
    ["map", "sabuk"],
    ["map", "red-moon-gorge"],
    ["map", "mengzhong-province"],
  ],
];
export const marketRows = [
  ["item", "black-iron-ore", "—", "—"],
  ["item", "blessing-oil", "—", "—"],
  ["item", "zuma-portrait", "—", "—"],
  ["equipment", "judgement-mace", "—", "—"],
];
export const mapPoints = [
  ["map", "sabuk", "—", "47%", "43%", "safe"],
  ["map", "bichon-province", "—", "28%", "31%", "safe"],
  ["map", "mengzhong-province", "—", "65%", "59%", "safe"],
  ["map", "red-moon-gorge", "—", "77%", "27%", "safe"],
];

export function isLocale(value) {
  return locales.includes(value);
}
export function archiveHref(record, locale = "zh-CN") {
  return `/${locale}/archive/${record.kind}/${record.slug}`;
}
export function getArchiveRecord(locale, kind, slug) {
  const record = archiveRecords.find(
    (item) => item.kind === kind && item.slug === slug,
  );
  if (!record || !isLocale(locale)) return undefined;
  const text = copy[locale];
  const [state] = facts[locale];
  const kindLabel = text.kinds[archiveKinds.indexOf(kind)];
  const name = `${kindLabel} · ${state}`;
  return {
    ...record,
    kindLabel,
    name,
    englishName: "NUMERON ATLAS · PROTOTYPE",
    eyebrow: "INTERFACE PROTOTYPE",
    summary: state,
    status: state,
    emblem: "◇",
    metrics: getDisplayMetrics(locale, state),
    overview: [state],
    facts: [{ label: text.ui[12], value: state }],
    timelineTitle: text.record[7],
    timeline: [{ time: "—", title: state, detail: state }],
  };
}
export function requireArchiveRecord(locale, kind, slug) {
  const record = getArchiveRecord(locale, kind, slug);
  if (!record) throw new Error("Missing prototype route");
  return record;
}
