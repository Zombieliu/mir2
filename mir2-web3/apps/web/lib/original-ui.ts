export type SpriteState = {
  base: string;
  hover?: string;
  pressed?: string;
  active?: string;
};

export type OriginalUiSpritePath = string | ((index: number) => string);

export type CharacterTabKey = "char" | "stats1" | "stats2" | "spells";
export type InventoryTabKey = "bag1" | "bag2" | "quest";
export type ClientScreen = "login" | "select" | "game";

export const ORIGINAL_UI = {
  login: {
    backgroundFrames: Array.from({ length: 19 }, (_, index) => `/original-ui/ChrSel/${index}.png`),
    dialog: "/original-ui/Prguse/1084.png",
    title: "/original-ui/Title/30.png",
    accountLabel: "/original-ui/Title/31.png",
    passwordLabel: "/original-ui/Title/32.png",
    buttons: {
      ok: sprite("/original-ui/Title/320.png", "/original-ui/Title/321.png", "/original-ui/Title/322.png"),
      newAccount: sprite("/original-ui/Title/323.png", "/original-ui/Title/324.png", "/original-ui/Title/325.png"),
      changePassword: sprite("/original-ui/Title/326.png", "/original-ui/Title/327.png", "/original-ui/Title/328.png"),
      close: sprite("/original-ui/Title/329.png", "/original-ui/Title/330.png", "/original-ui/Title/331.png"),
      viewKey: sprite("/original-ui/Title/332.png", "/original-ui/Title/333.png", "/original-ui/Title/334.png"),
    },
  },
  select: {
    background: "/original-ui/Prguse/65.png",
    title: "/original-ui/Title/40.png",
    loading: "/original-ui/Prguse/940.png",
    emptySlot: "/original-ui/Prguse/44.png",
    buttons: {
      start: sprite("/original-ui/Title/340.png", "/original-ui/Title/341.png", "/original-ui/Title/342.png"),
      newCharacter: sprite("/original-ui/Title/343.png", "/original-ui/Title/344.png", "/original-ui/Title/345.png"),
      deleteCharacter: sprite("/original-ui/Title/346.png", "/original-ui/Title/347.png", "/original-ui/Title/348.png"),
      credits: sprite("/original-ui/Title/349.png", "/original-ui/Title/350.png", "/original-ui/Title/351.png"),
      exit: sprite("/original-ui/Title/352.png", "/original-ui/Title/353.png", "/original-ui/Title/354.png"),
      createCharacter: sprite("/original-ui/Title/360.png", "/original-ui/Title/361.png", "/original-ui/Title/362.png"),
      cancel: sprite("/original-ui/Title/329.png", "/original-ui/Title/330.png", "/original-ui/Title/331.png"),
    },
    classCards: {
      warrior: { base: "/original-ui/Title/660.png", active: "/original-ui/Title/665.png" },
      wizard: { base: "/original-ui/Title/661.png", active: "/original-ui/Title/666.png" },
      taoist: { base: "/original-ui/Title/662.png", active: "/original-ui/Title/667.png" },
      assassin: { base: "/original-ui/Title/663.png", active: "/original-ui/Title/668.png" },
      archer: { base: "/original-ui/Title/664.png", active: "/original-ui/Title/669.png" },
    },
    portraits: {
      warriorMale: "/original-ui/ChrSel/20.png",
      warriorFemale: "/original-ui/ChrSel/300.png",
      wizardMale: "/original-ui/ChrSel/40.png",
      wizardFemale: "/original-ui/ChrSel/320.png",
      taoistMale: "/original-ui/ChrSel/60.png",
      taoistFemale: "/original-ui/ChrSel/340.png",
      assassinMale: "/original-ui/ChrSel/80.png",
      assassinFemale: "/original-ui/ChrSel/360.png",
      archerMale: "/original-ui/ChrSel/100.png",
      archerFemale: "/original-ui/ChrSel/140.png",
    },
  },
  game: {
    sceneWidth: 1024,
    sceneHeight: 768,
    chatDialog: "/original-ui/Prguse/2221.png",
    chatControlBar: "/original-ui/Prguse/2034.png",
    miniMap: "/original-ui/Prguse/2090.png",
    miniMapSmall: "/original-ui/Prguse/2091.png",
    duraPanel: "/original-ui/Prguse/2105.png",
    duraGray: "/original-ui/Prguse/2161.png",
    duraBg: "/original-ui/Prguse/2162.png",
    miniMapIcons: {
      light: "/original-ui/Prguse/2093.png",
      newMail: "/original-ui/Prguse/544.png",
    },
    questIcons: {
      questionWhite: ["/original-ui/Prguse/983.png", "/original-ui/Prguse/984.png"],
      exclamationYellow: ["/original-ui/Prguse/985.png", "/original-ui/Prguse/986.png"],
      questionYellow: ["/original-ui/Prguse/987.png", "/original-ui/Prguse/988.png"],
      exclamationGreen: ["/original-ui/Prguse/1085.png", "/original-ui/Prguse/1086.png"],
      questionGreen: ["/original-ui/Prguse/1087.png", "/original-ui/Prguse/1088.png"],
    },
    miniMapButtons: {
      bigMap: sprite("/original-ui/Prguse/2096.png", "/original-ui/Prguse/2097.png", "/original-ui/Prguse/2098.png"),
      mail: sprite("/original-ui/Prguse/2099.png", "/original-ui/Prguse/2100.png", "/original-ui/Prguse/2101.png"),
      toggle: sprite("/original-ui/Prguse/2102.png", "/original-ui/Prguse/2103.png", "/original-ui/Prguse/2104.png"),
      dura: sprite("/original-ui/Prguse/2113.png", "/original-ui/Prguse/2111.png", "/original-ui/Prguse/2112.png", "/original-ui/Prguse/2110.png"),
    },
    chatScrollButtons: {
      home: sprite("/original-ui/Prguse/2018.png", "/original-ui/Prguse/2019.png", "/original-ui/Prguse/2020.png"),
      up: sprite("/original-ui/Prguse/2021.png", "/original-ui/Prguse/2022.png", "/original-ui/Prguse/2023.png"),
      down: sprite("/original-ui/Prguse/2024.png", "/original-ui/Prguse/2025.png", "/original-ui/Prguse/2026.png"),
      end: sprite("/original-ui/Prguse/2027.png", "/original-ui/Prguse/2028.png", "/original-ui/Prguse/2029.png"),
      knob: sprite("/original-ui/Prguse/2015.png", "/original-ui/Prguse/2016.png", "/original-ui/Prguse/2017.png"),
    },
    chatCountBar: "/original-ui/Prguse/2012.png",
    chatFilterButtons: {
      all: sprite("/original-ui/Prguse/2036.png", "/original-ui/Prguse/2037.png", "/original-ui/Prguse/2038.png", "/original-ui/Prguse/2038.png"),
      shout: sprite("/original-ui/Prguse/2039.png", "/original-ui/Prguse/2040.png", "/original-ui/Prguse/2041.png", "/original-ui/Prguse/2041.png"),
      whisper: sprite("/original-ui/Prguse/2042.png", "/original-ui/Prguse/2043.png", "/original-ui/Prguse/2044.png", "/original-ui/Prguse/2044.png"),
      lover: sprite("/original-ui/Prguse/2045.png", "/original-ui/Prguse/2046.png", "/original-ui/Prguse/2047.png", "/original-ui/Prguse/2047.png"),
      mentor: sprite("/original-ui/Prguse/2048.png", "/original-ui/Prguse/2049.png", "/original-ui/Prguse/2050.png", "/original-ui/Prguse/2050.png"),
      group: sprite("/original-ui/Prguse/2051.png", "/original-ui/Prguse/2052.png", "/original-ui/Prguse/2053.png", "/original-ui/Prguse/2053.png"),
      guild: sprite("/original-ui/Prguse/2054.png", "/original-ui/Prguse/2055.png", "/original-ui/Prguse/2056.png", "/original-ui/Prguse/2056.png"),
      trade: sprite("/original-ui/Prguse/2004.png", "/original-ui/Prguse/2005.png", "/original-ui/Prguse/2006.png"),
      size: sprite("/original-ui/Prguse/2057.png", "/original-ui/Prguse/2058.png", "/original-ui/Prguse/2059.png"),
      settings: sprite("/original-ui/Prguse/2060.png", "/original-ui/Prguse/2061.png", "/original-ui/Prguse/2062.png"),
      report: sprite("/original-ui/Prguse/2063.png", "/original-ui/Prguse/2064.png", "/original-ui/Prguse/2065.png"),
    },
    belt: {
      horizontal: "/original-ui/Prguse/1932.png",
      horizontalOverlay: "/original-ui/Prguse/1933.png",
      vertical: "/original-ui/Prguse/1944.png",
      verticalOverlay: "/original-ui/Prguse/1945.png",
      closeHorizontal: sprite("/original-ui/Prguse/1923.png", "/original-ui/Prguse/1924.png", "/original-ui/Prguse/1925.png"),
      rotateHorizontal: sprite("/original-ui/Prguse/1926.png", "/original-ui/Prguse/1927.png", "/original-ui/Prguse/1928.png"),
      closeVertical: sprite("/original-ui/Prguse/1935.png", "/original-ui/Prguse/1936.png", "/original-ui/Prguse/1937.png"),
      rotateVertical: sprite("/original-ui/Prguse/1938.png", "/original-ui/Prguse/1939.png", "/original-ui/Prguse/1940.png"),
      slots: Array.from({ length: 6 }, (_, index) => ({
        key: `belt-${index + 1}`,
        horizontalX: index * 35 + 12,
        horizontalY: 3,
        verticalX: 3,
        verticalY: index * 35 + 12,
        labelX: 8,
        labelY: 2,
        verticalLabelX: -1,
        verticalLabelY: 11,
      })),
    },
    duraIcons: {
      necklace: {
        healthy: "/original-ui/Prguse/2122.png",
        warning: "/original-ui/Prguse/2123.png",
        danger: "/original-ui/Prguse/2124.png",
      },
      weapon: {
        healthy: "/original-ui/Prguse/2125.png",
        warning: "/original-ui/Prguse/2126.png",
        danger: "/original-ui/Prguse/2127.png",
      },
      ring: {
        healthy: "/original-ui/Prguse/2131.png",
        warning: "/original-ui/Prguse/2132.png",
        danger: "/original-ui/Prguse/2133.png",
      },
      amulet: {
        healthy: "/original-ui/Prguse/2134.png",
        warning: "/original-ui/Prguse/2135.png",
        danger: "/original-ui/Prguse/2136.png",
      },
      stone: {
        empty: "/original-ui/Prguse/2137.png",
      },
      mount: {
        healthy: "/original-ui/Prguse/2140.png",
        warning: "/original-ui/Prguse/2141.png",
        danger: "/original-ui/Prguse/2142.png",
      },
      bracelet: {
        healthy: "/original-ui/Prguse/2143.png",
        warning: "/original-ui/Prguse/2144.png",
        danger: "/original-ui/Prguse/2145.png",
      },
      torch: {
        healthy: "/original-ui/Prguse/2146.png",
        warning: "/original-ui/Prguse/2147.png",
        danger: "/original-ui/Prguse/2148.png",
      },
      armour: {
        healthy: "/original-ui/Prguse/2149.png",
        warning: "/original-ui/Prguse/2150.png",
        danger: "/original-ui/Prguse/2151.png",
      },
      boots: {
        healthy: "/original-ui/Prguse/2152.png",
        warning: "/original-ui/Prguse/2153.png",
        danger: "/original-ui/Prguse/2154.png",
      },
      helmet: {
        healthy: "/original-ui/Prguse/2155.png",
        warning: "/original-ui/Prguse/2156.png",
        danger: "/original-ui/Prguse/2157.png",
      },
      belt: {
        healthy: "/original-ui/Prguse/2158.png",
        warning: "/original-ui/Prguse/2159.png",
        danger: "/original-ui/Prguse/2160.png",
      },
    },
  },
  hud: {
    width: 1024,
    height: 152,
    base: "/original-ui/Prguse/1.png",
    healthManaOrb: "/original-ui/Prguse/4.png",
    healthOnlyOrb: "/original-ui/Prguse/6.png",
    leftCap: "/original-ui/Prguse/12.png",
    rightCap: "/original-ui/Prguse/13.png",
    experienceBar: "/original-ui/Prguse/8.png",
    weightBar: "/original-ui/Prguse/76.png",
    skillStrip: "/original-ui/Prguse/2034.png",
    buttons: {
      gameShop: sprite("/original-ui/Prguse/826.png", "/original-ui/Prguse/827.png", "/original-ui/Prguse/828.png"),
      menu: sprite("/original-ui/Prguse/1960.png", "/original-ui/Prguse/1961.png", "/original-ui/Prguse/1962.png"),
      character: sprite("/original-ui/Prguse/1900.png", "/original-ui/Prguse/1901.png", "/original-ui/Prguse/1902.png"),
      inventory: sprite("/original-ui/Prguse/1903.png", "/original-ui/Prguse/1904.png", "/original-ui/Prguse/1905.png"),
      skill: sprite("/original-ui/Prguse/1906.png", "/original-ui/Prguse/1907.png", "/original-ui/Prguse/1908.png"),
      quest: sprite("/original-ui/Prguse/1909.png", "/original-ui/Prguse/1910.png", "/original-ui/Prguse/1911.png"),
      option: sprite("/original-ui/Prguse/1912.png", "/original-ui/Prguse/1913.png", "/original-ui/Prguse/1914.png"),
    },
  },
  menu: {
    width: 36,
    height: 282,
    frame: "/original-ui/Title/567.png",
    buttons: {
      exit: { sprite: sprite("/original-ui/Title/633.png", "/original-ui/Title/634.png", "/original-ui/Title/635.png"), x: 3, y: 12 },
      logout: { sprite: sprite("/original-ui/Title/636.png", "/original-ui/Title/637.png", "/original-ui/Title/638.png"), x: 3, y: 31 },
      help: { sprite: sprite("/original-ui/Prguse/1970.png", "/original-ui/Prguse/1971.png", "/original-ui/Prguse/1972.png"), x: 3, y: 50 },
      keyboard: { sprite: sprite("/original-ui/Prguse/1973.png", "/original-ui/Prguse/1974.png", "/original-ui/Prguse/1975.png"), x: 3, y: 69 },
      ranking: { sprite: sprite("/original-ui/Prguse/2000.png", "/original-ui/Prguse/2001.png", "/original-ui/Prguse/2002.png"), x: 3, y: 88 },
      creature: { sprite: sprite("/original-ui/Prguse2/431.png", "/original-ui/Prguse2/432.png", "/original-ui/Prguse2/433.png"), x: 3, y: 126 },
      ride: { sprite: sprite("/original-ui/Prguse/1976.png", "/original-ui/Prguse/1977.png", "/original-ui/Prguse/1978.png"), x: 3, y: 145 },
      fishing: { sprite: sprite("/original-ui/Prguse/1979.png", "/original-ui/Prguse/1980.png", "/original-ui/Prguse/1981.png"), x: 3, y: 164 },
      friend: { sprite: sprite("/original-ui/Prguse/1982.png", "/original-ui/Prguse/1983.png", "/original-ui/Prguse/1984.png"), x: 3, y: 183 },
      mentor: { sprite: sprite("/original-ui/Prguse/1985.png", "/original-ui/Prguse/1986.png", "/original-ui/Prguse/1987.png"), x: 3, y: 202 },
      relationship: { sprite: sprite("/original-ui/Prguse/1988.png", "/original-ui/Prguse/1989.png", "/original-ui/Prguse/1990.png"), x: 3, y: 221 },
      group: { sprite: sprite("/original-ui/Prguse/1991.png", "/original-ui/Prguse/1992.png", "/original-ui/Prguse/1993.png"), x: 3, y: 240 },
      guild: { sprite: sprite("/original-ui/Prguse/1994.png", "/original-ui/Prguse/1995.png", "/original-ui/Prguse/1996.png"), x: 3, y: 259 },
    },
  },
  gameShop: {
    width: 696,
    height: 476,
    frame: "/original-ui/Title/749.png",
    title: "/original-ui/Title/26.png",
    cellFrame: "/original-ui/Title/750.png",
    filterBackground: "/original-ui/Title/769.png",
    closeButton: sprite("/original-ui/Prguse2/360.png", "/original-ui/Prguse2/361.png", "/original-ui/Prguse2/362.png"),
    upButton: sprite("/original-ui/Prguse2/197.png", "/original-ui/Prguse2/198.png", "/original-ui/Prguse2/199.png"),
    downButton: sprite("/original-ui/Prguse2/207.png", "/original-ui/Prguse2/208.png", "/original-ui/Prguse2/209.png"),
    positionBar: sprite("/original-ui/Prguse2/205.png", "/original-ui/Prguse2/206.png", "/original-ui/Prguse2/206.png"),
    previousButton: sprite("/original-ui/Prguse2/240.png", "/original-ui/Prguse2/241.png", "/original-ui/Prguse2/242.png"),
    nextButton: sprite("/original-ui/Prguse2/243.png", "/original-ui/Prguse2/244.png", "/original-ui/Prguse2/245.png"),
    paymentBox: {
      unchecked: "/original-ui/Prguse/2086.png",
      checked: "/original-ui/Prguse/2087.png",
    },
    buyButton: sprite("/original-ui/Title/778.png", "/original-ui/Title/779.png", "/original-ui/Title/780.png"),
    previewButton: sprite("/original-ui/Title/781.png", "/original-ui/Title/782.png", "/original-ui/Title/783.png"),
    sectionTabs: {
      all: sprite("/original-ui/Title/770.png", "/original-ui/Title/771.png", "/original-ui/Title/771.png", "/original-ui/Title/771.png"),
      deals: sprite("/original-ui/Title/772.png", "/original-ui/Title/773.png", "/original-ui/Title/773.png", "/original-ui/Title/773.png"),
      newItems: sprite("/original-ui/Title/774.png", "/original-ui/Title/775.png", "/original-ui/Title/775.png", "/original-ui/Title/775.png"),
      top: sprite("/original-ui/Title/776.png", "/original-ui/Title/777.png", "/original-ui/Title/777.png", "/original-ui/Title/777.png"),
    },
    classTabs: {
      all: sprite("/original-ui/Title/751.png", "/original-ui/Title/752.png", "/original-ui/Title/753.png", "/original-ui/Title/752.png"),
      warrior: sprite("/original-ui/Title/754.png", "/original-ui/Title/755.png", "/original-ui/Title/756.png", "/original-ui/Title/755.png"),
      assassin: sprite("/original-ui/Title/757.png", "/original-ui/Title/758.png", "/original-ui/Title/759.png", "/original-ui/Title/758.png"),
      taoist: sprite("/original-ui/Title/760.png", "/original-ui/Title/761.png", "/original-ui/Title/762.png", "/original-ui/Title/761.png"),
      wizard: sprite("/original-ui/Title/763.png", "/original-ui/Title/764.png", "/original-ui/Title/765.png", "/original-ui/Title/764.png"),
      archer: sprite("/original-ui/Title/766.png", "/original-ui/Title/767.png", "/original-ui/Title/768.png", "/original-ui/Title/767.png"),
    },
  },
  bigMap: {
    width: 760,
    height: 500,
    frame: "/original-ui/Title/820.png",
    closeButton: sprite("/original-ui/Prguse2/360.png", "/original-ui/Prguse2/361.png", "/original-ui/Prguse2/362.png"),
    upButton: sprite("/original-ui/Prguse2/197.png", "/original-ui/Prguse2/198.png", "/original-ui/Prguse2/199.png"),
    downButton: sprite("/original-ui/Prguse2/207.png", "/original-ui/Prguse2/208.png", "/original-ui/Prguse2/209.png"),
    positionBar: sprite("/original-ui/Prguse2/205.png", "/original-ui/Prguse2/206.png", "/original-ui/Prguse2/206.png"),
    worldButton: sprite("/original-ui/Title/827.png", "/original-ui/Title/828.png", "/original-ui/Title/829.png"),
    myLocationButton: sprite("/original-ui/Title/824.png", "/original-ui/Title/825.png", "/original-ui/Title/826.png"),
    teleportButton: sprite("/original-ui/Title/821.png", "/original-ui/Title/822.png", "/original-ui/Title/823.png", "/original-ui/Title/823.png"),
    searchButton: sprite("/original-ui/Prguse2/1340.png", "/original-ui/Prguse2/1341.png", "/original-ui/Prguse2/1342.png"),
    radarDot: "/original-ui/Prguse2/1350.png",
    worldMap: "/original-ui/Prguse2/1360.png",
    worldClouds: "/original-ui/Prguse2/1365.png",
    worldBorder: "/original-ui/Prguse2/1366.png",
    mapLinkIcon: (index: number) => `/original-ui/MapLinkIcon/${index}.png`,
  },
  npc: {
    // Crystal NPCDialog: Prguse frame 995 background, close (Prguse2 360-362) at
    // (413,3), help (Prguse2 257-259) at (390,3), body text at (8,34) 420px wide
    // on an 18px line stride, scrollbar column at x=417.
    frame: "/original-ui/Prguse/995.png",
    closeButton: sprite("/original-ui/Prguse2/360.png", "/original-ui/Prguse2/361.png", "/original-ui/Prguse2/362.png"),
    helpButton: sprite("/original-ui/Prguse2/257.png", "/original-ui/Prguse2/258.png", "/original-ui/Prguse2/259.png"),
  },
  trade: {
    // Crystal TradeDialog (one side): Prguse frame 389, 204x152. Item cells form
    // a 5x2 grid at (x*36+10+x, y*32+39+y); gold at (35,123); the confirm/lock
    // button is Title 520-522 at (135,120); close (Prguse2 360-362) at (181,3).
    width: 204,
    height: 152,
    frame: "/original-ui/Prguse/389.png",
    closeButton: sprite("/original-ui/Prguse2/360.png", "/original-ui/Prguse2/361.png", "/original-ui/Prguse2/362.png"),
    confirmButton: sprite("/original-ui/Title/520.png", "/original-ui/Title/521.png", "/original-ui/Title/522.png"),
    cellSize: 32,
    cellStep: 37,
    cellOriginX: 10,
    cellOriginY: 39,
  },
  mail: {
    width: 312,
    height: 444,
    frame: "/original-ui/Title/670.png",
    title: "/original-ui/Title/7.png",
    closeButton: sprite("/original-ui/Prguse2/360.png", "/original-ui/Prguse2/361.png", "/original-ui/Prguse2/362.png"),
    helpButton: sprite("/original-ui/Prguse2/257.png", "/original-ui/Prguse2/258.png", "/original-ui/Prguse2/259.png"),
    previousButton: sprite("/original-ui/Prguse2/240.png", "/original-ui/Prguse2/241.png", "/original-ui/Prguse2/242.png"),
    nextButton: sprite("/original-ui/Prguse2/243.png", "/original-ui/Prguse2/244.png", "/original-ui/Prguse2/245.png"),
    sendButton: sprite("/original-ui/Prguse/563.png", "/original-ui/Prguse/564.png", "/original-ui/Prguse/565.png"),
    replyButton: sprite("/original-ui/Prguse/569.png", "/original-ui/Prguse/570.png", "/original-ui/Prguse/571.png"),
    readButton: sprite("/original-ui/Prguse/572.png", "/original-ui/Prguse/573.png", "/original-ui/Prguse/574.png"),
    deleteButton: sprite("/original-ui/Prguse/557.png", "/original-ui/Prguse/558.png", "/original-ui/Prguse/559.png"),
    blockListButton: sprite("/original-ui/Prguse/520.png", "/original-ui/Prguse/521.png", "/original-ui/Prguse/522.png"),
    bugReportButton: sprite("/original-ui/Prguse/523.png", "/original-ui/Prguse/524.png", "/original-ui/Prguse/525.png"),
    icons: {
      letter: "/original-ui/Prguse/540.png",
      gold: "/original-ui/Prguse/541.png",
      selected: "/original-ui/Prguse/545.png",
      unread: "/original-ui/Prguse/550.png",
      locked: "/original-ui/Prguse/551.png",
      parcel: "/original-ui/Prguse/552.png",
    },
  },
  inventory: {
    width: 316,
    height: 236,
    frame: "/original-ui/Title/196.png",
    tabs: {
      bag1: { active: "/original-ui/Title/197.png", idle: "/original-ui/Title/738.png" },
      bag2: { active: "/original-ui/Title/168.png", idle: "/original-ui/Title/738.png" },
      quest: { active: "/original-ui/Title/198.png", idle: "/original-ui/Title/739.png" },
    },
    closeButton: sprite("/original-ui/Prguse2/360.png", "/original-ui/Prguse2/361.png", "/original-ui/Prguse2/362.png"),
    deleteButton: sprite("/original-ui/Prguse2/366.png", "/original-ui/Prguse2/367.png", "/original-ui/Prguse2/368.png"),
    weightBar: "/original-ui/Prguse/24.png",
    slots: buildInventorySlots(),
  },
  storage: {
    width: 384,
    height: 344,
    slots: buildStorageSlots(),
  },
  character: {
    width: 264,
    height: 380,
    frame: "/original-ui/Title/504.png",
    pages: {
      char: "/original-ui/Prguse/340.png",
      stats1: "/original-ui/Title/506.png",
      stats2: "/original-ui/Title/507.png",
      spells: "/original-ui/Title/508.png",
    },
    tabs: {
      char: "/original-ui/Title/500.png",
      stats1: "/original-ui/Title/501.png",
      stats2: "/original-ui/Title/502.png",
      spells: "/original-ui/Title/503.png",
    },
    classIcons: {
      warrior: "/original-ui/Prguse/100.png",
      wizard: "/original-ui/Prguse/101.png",
      taoist: "/original-ui/Prguse/102.png",
      assassin: "/original-ui/Prguse/103.png",
      archer: "/original-ui/Prguse/104.png",
    },
    closeButton: sprite("/original-ui/Prguse2/360.png", "/original-ui/Prguse2/361.png", "/original-ui/Prguse2/362.png"),
    prevButton: sprite("/original-ui/Prguse/398.png", "/original-ui/Prguse/399.png", "/original-ui/Prguse/399.png"),
    nextButton: sprite("/original-ui/Prguse/396.png", "/original-ui/Prguse/397.png", "/original-ui/Prguse/397.png"),
    equipmentSlots: [
      slot("Weapon", 123, 7),
      slot("Armour", 163, 7),
      slot("Helmet", 203, 7),
      slot("Mount", 203, 62),
      slot("Necklace", 203, 98),
      slot("Torch", 203, 134),
      slot("BraceletL", 8, 170),
      slot("BraceletR", 203, 170),
      slot("RingL", 8, 206),
      slot("RingR", 203, 206),
      slot("Amulet", 8, 242),
      slot("Boots", 48, 242),
      slot("Belt", 88, 242),
      slot("Stone", 128, 242),
    ],
  },
} as const;

function sprite(base: string, hover: string, pressed: string, active?: string): SpriteState {
  return { base, hover, pressed, active };
}

function slot(label: string, x: number, y: number) {
  return { label, x, y };
}

function buildInventorySlots() {
  const slots = [];

  for (let y = 0; y < 5; y += 1) {
    for (let x = 0; x < 8; x += 1) {
      slots.push({
        key: `slot-${x}-${y}`,
        x: x * 37 + 9,
        y: y * 33 + 37,
      });
    }
  }

  return slots;
}

function buildStorageSlots() {
  const slots = [];

  for (let y = 0; y < 8; y += 1) {
    for (let x = 0; x < 10; x += 1) {
      slots.push({
        key: `storage-slot-${x}-${y}`,
        x: x * 37 + 9,
        y: y * 33 + 60,
      });
    }
  }

  return slots;
}
