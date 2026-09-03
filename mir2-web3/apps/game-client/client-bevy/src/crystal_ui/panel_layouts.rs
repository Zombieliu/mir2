//! Pure Crystal panel layout data.
//!
//! This file deliberately contains no Bevy, renderer, input, network, or game
//! logic.  Values are copied from the Crystal client source listed by each
//! [`SourceRef`].  An absent value is represented by `None`; it is never
//! inferred from an atlas image or from a sibling panel.

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Library {
    Title,
    Prguse,
    Prguse2,
    MagIcon2,
    Items,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SpriteRef {
    pub library: Library,
    pub index: u16,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SpriteTriple {
    pub normal: Option<u16>,
    pub hover: Option<u16>,
    pub pressed: Option<u16>,
}

impl SpriteTriple {
    pub const NONE: Self = Self {
        normal: None,
        hover: None,
        pressed: None,
    };

    pub const fn new(normal: u16, hover: u16, pressed: u16) -> Self {
        Self {
            normal: Some(normal),
            hover: Some(hover),
            pressed: Some(pressed),
        }
    }

    pub const fn with_unknown_hover(normal: u16, pressed: u16) -> Self {
        Self {
            normal: Some(normal),
            hover: None,
            pressed: Some(pressed),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Confirmed,
    Option(&'static str),
    Unsupported(&'static str),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Position {
    Absolute(Point),
    Center,
    Formula(&'static str),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RectSpec {
    pub position: Position,
    pub size: Option<Size>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RegionSpec {
    pub name: &'static str,
    pub rect: RectSpec,
    pub sprite: Option<SpriteRef>,
    pub status: Status,
    pub source: SourceRef,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SourceRef {
    pub file: &'static str,
    pub lines: Option<LineRange>,
}

impl SourceRef {
    pub const fn new(file: &'static str, start: u32, end: u32) -> Self {
        Self {
            file,
            lines: Some(LineRange { start, end }),
        }
    }

    pub const fn no_declaration(file: &'static str) -> Self {
        Self { file, lines: None }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ButtonSpec {
    pub name: &'static str,
    pub rect: RectSpec,
    pub library: Library,
    /// The static/default triple.  `None` means Crystal assigns it later or
    /// does not assign that state in the cited source.
    pub sprites: SpriteTriple,
    /// State-dependent triples, in source order.  This is data only; callers
    /// must interpret the state labels in `variant_names`.
    pub variants: &'static [SpriteTriple],
    pub variant_names: &'static [&'static str],
    pub status: Status,
    pub source: SourceRef,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GridSpec {
    pub name: &'static str,
    pub origin: Position,
    pub columns: Option<u16>,
    pub rows: Option<u16>,
    pub capacity: Option<u16>,
    pub visible_per_page: Option<u16>,
    pub pages: Option<u16>,
    pub cell_size: Option<Size>,
    pub step: Option<Point>,
    pub coordinate_formula: &'static str,
    pub status: Status,
    pub source: SourceRef,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PaginationSpec {
    pub name: &'static str,
    pub page_size: Option<u16>,
    pub page_count: Option<u16>,
    pub label: Option<RectSpec>,
    pub previous: Option<Position>,
    pub next: Option<Position>,
    pub source: SourceRef,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PanelSpec {
    pub name: &'static str,
    pub origin: Position,
    pub size: Option<Size>,
    pub background: Option<SpriteRef>,
    pub buttons: &'static [ButtonSpec],
    pub regions: &'static [RegionSpec],
    pub grids: &'static [GridSpec],
    pub pagination: &'static [PaginationSpec],
    pub notes: &'static [&'static str],
    pub status: Status,
    pub source: SourceRef,
}

const MAIN: &str = "../Crystal/Client/MirScenes/Dialogs/MainDialogs.cs";
const MAIL_FILE: &str = "../Crystal/Client/MirScenes/Dialogs/MailDialogs.cs";
const SHOP: &str = "../Crystal/Client/MirScenes/Dialogs/GameshopDialog.cs";
const NPC: &str = "../Crystal/Client/MirScenes/Dialogs/NPCDialogs.cs";
const INVENTORY_FILE: &str = "../Crystal/Client/MirScenes/Dialogs/InventoryDialog.cs";
const CHARACTER_FILE: &str = "../Crystal/Client/MirScenes/Dialogs/CharacterDialog.cs";

const OPTION_SOURCE: SourceRef = SourceRef::new(MAIN, 2527, 3004);
const MENU_SOURCE: SourceRef = SourceRef::new(MAIN, 3007, 3266);
const ASSIGN_SOURCE: SourceRef = SourceRef::new(MAIN, 3784, 3902);
const MAIL_SOURCE: SourceRef = SourceRef::new(MAIL_FILE, 9, 319);
const MAIL_ROW_SOURCE: SourceRef = SourceRef::new(MAIL_FILE, 425, 520);
const SHOP_SOURCE: SourceRef = SourceRef::new(SHOP, 7, 468);
const SHOP_UPDATE_SOURCE: SourceRef = SourceRef::new(SHOP, 729, 758);
const NPC_GOODS_SOURCE: SourceRef = SourceRef::new(NPC, 1051, 1197);
const STORAGE_SOURCE: SourceRef = SourceRef::new(NPC, 2798, 3298);
const BIG_MAP_SOURCE: SourceRef = SourceRef::new(
    "../Crystal/Client/MirScenes/Dialogs/BigMapDialog.cs",
    12,
    832,
);
const BIG_MAP_CONSTRUCTOR_SOURCE: SourceRef = SourceRef::new(
    "../Crystal/Client/MirScenes/Dialogs/BigMapDialog.cs",
    93,
    254,
);
const BIG_MAP_VIEWPORT_SOURCE: SourceRef = SourceRef::new(
    "../Crystal/Client/MirScenes/Dialogs/BigMapDialog.cs",
    544,
    780,
);
const BIG_MAP_WORLD_SOURCE: SourceRef = SourceRef::new(
    "../Crystal/Client/MirScenes/Dialogs/BigMapDialog.cs",
    468,
    527,
);
const BIG_MAP_NPC_ROW_SOURCE: SourceRef = SourceRef::new(
    "../Crystal/Client/MirScenes/Dialogs/BigMapDialog.cs",
    782,
    832,
);

/// Native render bounds for the Crystal inventory. Crystal exposes eighty
/// bag cells as two pages of 8 x 5 cells; keeping 46 server slots does not
/// change that presentation contract.
pub const INVENTORY_PANEL_SIZE: Size = Size {
    width: 316,
    height: 236,
};
/// `GameScene` constructs the ordinary InventoryDialog without overriding
/// `MirControl.Location`, whose source default is `(0,0)`. Service/trade
/// dialogs may move it later; the direct I/F9/HUD path starts here.
pub const INVENTORY_PANEL_ORIGIN: Point = Point { x: 0, y: 0 };
pub const INVENTORY_PAGE_COLUMNS: usize = 8;
pub const INVENTORY_PAGE_ROWS: usize = 5;
pub const INVENTORY_PAGE_SIZE: usize = INVENTORY_PAGE_COLUMNS * INVENTORY_PAGE_ROWS;
pub const INVENTORY_GRID_ORIGIN: Point = Point { x: 9, y: 37 };
pub const INVENTORY_GRID_STEP: Point = Point { x: 37, y: 33 };
pub const INVENTORY_CELL_SIZE: Size = Size {
    width: 36,
    height: 32,
};
/// Crystal `InventoryDialog` footer controls.  These are independent of the
/// selected bag page and remain visible while the dialog is open.
pub const INVENTORY_GOLD_LABEL_ORIGIN: Point = Point { x: 40, y: 212 };
pub const INVENTORY_GOLD_LABEL_SIZE: Size = Size {
    width: 111,
    height: 14,
};
pub const INVENTORY_WEIGHT_BAR_ORIGIN: Point = Point { x: 182, y: 217 };
pub const INVENTORY_WEIGHT_BAR_SIZE: Size = Size {
    width: 84,
    height: 6,
};
pub const INVENTORY_FREE_SLOT_LABEL_ORIGIN: Point = Point { x: 268, y: 212 };
pub const INVENTORY_FREE_SLOT_LABEL_SIZE: Size = Size {
    width: 26,
    height: 14,
};
pub const INVENTORY_DELETE_BUTTON_ORIGIN: Point = Point { x: 291, y: 212 };
pub const INVENTORY_DELETE_BUTTON_SIZE: Size = Size {
    width: 16,
    height: 15,
};

/// CharacterDialog owns the Crystal skill page. It uses Title/504 as the
/// outer frame, Title/508 as the page, seven 33px rows and pager controls at
/// y=340 in outer-panel coordinates.
pub const SKILL_PANEL_SIZE: Size = Size {
    width: 264,
    height: 380,
};
pub const SKILL_PAGE_SIZE: usize = 7;
pub const SKILL_PAGE_ORIGIN: Point = Point { x: 8, y: 90 };
pub const SKILL_ROW_ORIGIN: Point = Point { x: 16, y: 98 };
pub const SKILL_ROW_SIZE: Size = Size {
    width: 210,
    height: 28,
};
pub const SKILL_ROW_STEP_Y: i32 = 33;

/// GameShopDialog has a fixed 696 x 476 background and an authoritative 4 x 2
/// page. The catalog may contain any bounded number of rows; pagination must
/// expose every row eight at a time rather than expanding the panel.
pub const GAME_SHOP_PANEL_SIZE: Size = Size {
    width: 696,
    height: 476,
};
pub const GAME_SHOP_PAGE_COLUMNS: usize = 4;
pub const GAME_SHOP_PAGE_ROWS: usize = 2;
pub const GAME_SHOP_PAGE_SIZE: usize = GAME_SHOP_PAGE_COLUMNS * GAME_SHOP_PAGE_ROWS;
pub const GAME_SHOP_GRID_ORIGIN: Point = Point { x: 152, y: 115 };
pub const GAME_SHOP_CELL_SIZE: Size = Size {
    width: 125,
    height: 146,
};
pub const GAME_SHOP_COLUMN_STEP: i32 = 132;
pub const GAME_SHOP_ROW_STEP: i32 = 160;

pub const INVENTORY_LAYOUT_SOURCE: SourceRef = SourceRef::new(INVENTORY_FILE, 148, 181);
pub const INVENTORY_FOOTER_SOURCE: SourceRef = SourceRef::new(INVENTORY_FILE, 23, 195);
pub const SKILL_LAYOUT_SOURCE: SourceRef = SourceRef::new(CHARACTER_FILE, 136, 143);
pub const SKILL_ROWS_SOURCE: SourceRef = SourceRef::new(CHARACTER_FILE, 550, 593);

const OPTION_SKILL_MODE: &[SpriteTriple] = &[
    SpriteTriple::with_unknown_hover(452, 451),
    SpriteTriple::with_unknown_hover(450, 451),
    SpriteTriple::with_unknown_hover(453, 454),
    SpriteTriple::with_unknown_hover(455, 454),
];
const OPTION_STANDARD_ON: &[SpriteTriple] = &[
    SpriteTriple::with_unknown_hover(458, 457),
    SpriteTriple::with_unknown_hover(456, 457),
];
const OPTION_STANDARD_OFF: &[SpriteTriple] = &[
    SpriteTriple::with_unknown_hover(459, 460),
    SpriteTriple::with_unknown_hover(461, 460),
];
const OPTION_HP_ON: &[SpriteTriple] = &[
    SpriteTriple::with_unknown_hover(464, 463),
    SpriteTriple::with_unknown_hover(462, 463),
];
const OPTION_HP_OFF: &[SpriteTriple] = &[
    SpriteTriple::with_unknown_hover(465, 466),
    SpriteTriple::with_unknown_hover(467, 466),
];
const OPTION_NEW_MOVE_ON: &[SpriteTriple] = &[
    SpriteTriple::with_unknown_hover(853, 853),
    SpriteTriple::with_unknown_hover(851, 853),
];
const OPTION_NEW_MOVE_OFF: &[SpriteTriple] = &[
    SpriteTriple::with_unknown_hover(848, 850),
    SpriteTriple::with_unknown_hover(850, 850),
];
const OPTION_TRUE_FALSE: &[&str] = &["true", "false"];
const OPTION_SKILL_MODE_NAMES: &[&str] = &[
    "skill_mode_on_true",
    "skill_mode_on_false",
    "skill_mode_off_true",
    "skill_mode_off_false",
];

const OPTION_BUTTONS: &[ButtonSpec] = &[
    ButtonSpec {
        name: "close",
        rect: RectSpec {
            position: Position::Formula("x = panel.width - 26; y = 5"),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(360, 361, 362),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "skill_mode_on",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 159, y: 68 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::NONE,
        variants: OPTION_SKILL_MODE,
        variant_names: OPTION_SKILL_MODE_NAMES,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "skill_mode_off",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 201, y: 68 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::NONE,
        variants: OPTION_SKILL_MODE,
        variant_names: OPTION_SKILL_MODE_NAMES,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "skill_bar_on",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 159, y: 93 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::NONE,
        variants: OPTION_STANDARD_ON,
        variant_names: OPTION_TRUE_FALSE,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "skill_bar_off",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 201, y: 93 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::NONE,
        variants: OPTION_STANDARD_OFF,
        variant_names: OPTION_TRUE_FALSE,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "effect_on",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 159, y: 118 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::NONE,
        variants: OPTION_STANDARD_ON,
        variant_names: OPTION_TRUE_FALSE,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "effect_off",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 201, y: 118 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::NONE,
        variants: OPTION_STANDARD_OFF,
        variant_names: OPTION_TRUE_FALSE,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "drop_view_on",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 159, y: 143 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::NONE,
        variants: OPTION_STANDARD_ON,
        variant_names: OPTION_TRUE_FALSE,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "drop_view_off",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 201, y: 143 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::NONE,
        variants: OPTION_STANDARD_OFF,
        variant_names: OPTION_TRUE_FALSE,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "name_view_on",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 159, y: 168 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::NONE,
        variants: OPTION_STANDARD_ON,
        variant_names: OPTION_TRUE_FALSE,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "name_view_off",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 201, y: 168 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::NONE,
        variants: OPTION_STANDARD_OFF,
        variant_names: OPTION_TRUE_FALSE,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "hp_view_on",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 159, y: 193 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::NONE,
        variants: OPTION_HP_ON,
        variant_names: OPTION_TRUE_FALSE,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "hp_view_off",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 201, y: 193 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::NONE,
        variants: OPTION_HP_OFF,
        variant_names: OPTION_TRUE_FALSE,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "new_move_on",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 159, y: 296 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Title,
        sprites: SpriteTriple::NONE,
        variants: OPTION_NEW_MOVE_ON,
        variant_names: OPTION_TRUE_FALSE,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "new_move_off",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 201, y: 296 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Title,
        sprites: SpriteTriple::NONE,
        variants: OPTION_NEW_MOVE_OFF,
        variant_names: OPTION_TRUE_FALSE,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "observe_on",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 159, y: 271 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::NONE,
        variants: OPTION_STANDARD_ON,
        variant_names: OPTION_TRUE_FALSE,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
    ButtonSpec {
        name: "observe_off",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 201, y: 271 }),
            size: Some(Size {
                width: 36,
                height: 17,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::NONE,
        variants: OPTION_STANDARD_OFF,
        variant_names: OPTION_TRUE_FALSE,
        status: Status::Confirmed,
        source: OPTION_SOURCE,
    },
];

const MENU_BUTTONS: &[ButtonSpec] = &[
    ButtonSpec {
        name: "exit",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 3, y: 12 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(633, 634, 635),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MENU_SOURCE,
    },
    ButtonSpec {
        name: "logout",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 3, y: 31 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(636, 637, 638),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MENU_SOURCE,
    },
    ButtonSpec {
        name: "help",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 3, y: 50 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(1970, 1971, 1972),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MENU_SOURCE,
    },
    ButtonSpec {
        name: "keyboard_layout",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 3, y: 69 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(1973, 1974, 1975),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MENU_SOURCE,
    },
    ButtonSpec {
        name: "ranking",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 3, y: 88 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(2000, 2001, 2002),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MENU_SOURCE,
    },
    ButtonSpec {
        name: "crafting",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 3, y: 107 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(2000, 2001, 2002),
        variants: &[],
        variant_names: &[],
        status: Status::Option("source sets Visible=false and has an empty click handler"),
        source: MENU_SOURCE,
    },
    ButtonSpec {
        name: "intelligent_creature",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 3, y: 126 }),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(431, 432, 433),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MENU_SOURCE,
    },
    ButtonSpec {
        name: "ride",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 3, y: 145 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(1976, 1977, 1978),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MENU_SOURCE,
    },
    ButtonSpec {
        name: "fishing",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 3, y: 164 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(1979, 1980, 1981),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MENU_SOURCE,
    },
    ButtonSpec {
        name: "friend",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 3, y: 183 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(1982, 1983, 1984),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MENU_SOURCE,
    },
    ButtonSpec {
        name: "mentor",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 3, y: 202 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(1985, 1986, 1987),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MENU_SOURCE,
    },
    ButtonSpec {
        name: "relationship",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 3, y: 221 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(1988, 1989, 1990),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MENU_SOURCE,
    },
    ButtonSpec {
        name: "group",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 3, y: 240 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(1991, 1992, 1993),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MENU_SOURCE,
    },
    ButtonSpec {
        name: "guild",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 3, y: 259 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(1994, 1995, 1996),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MENU_SOURCE,
    },
];

const ASSIGN_BUTTONS: &[ButtonSpec] = &[
    ButtonSpec {
        name: "none",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 284, y: 64 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(287, 288, 289),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: ASSIGN_SOURCE,
    },
    ButtonSpec {
        name: "save",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 284, y: 101 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(156, 157, 158),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: ASSIGN_SOURCE,
    },
    ButtonSpec {
        name: "f_key_cells",
        rect: RectSpec {
            position: Position::Formula("x = 17 + 32*(i%8) + 5*(i%8/4); y = 58 + 37*(i/8)"),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(1656, 1657, 1658),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: ASSIGN_SOURCE,
    },
];

const MAIL_BUTTONS: &[ButtonSpec] = &[
    ButtonSpec {
        name: "close",
        rect: RectSpec {
            position: Position::Formula("x = panel.width - 24; y = 3"),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(360, 361, 362),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MAIL_SOURCE,
    },
    ButtonSpec {
        name: "help",
        rect: RectSpec {
            position: Position::Formula("x = panel.width - 50; y = 3"),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(257, 258, 259),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MAIL_SOURCE,
    },
    ButtonSpec {
        name: "previous",
        rect: RectSpec {
            position: Position::Formula("x = 102; y = panel.height - 55"),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(240, 241, 242),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MAIL_SOURCE,
    },
    ButtonSpec {
        name: "next",
        rect: RectSpec {
            position: Position::Formula("x = 192; y = panel.height - 55"),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(243, 244, 245),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MAIL_SOURCE,
    },
    ButtonSpec {
        name: "send",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 75, y: 414 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(563, 564, 565),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MAIL_SOURCE,
    },
    ButtonSpec {
        name: "reply",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 102, y: 414 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(569, 570, 571),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MAIL_SOURCE,
    },
    ButtonSpec {
        name: "read",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 129, y: 414 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(572, 573, 574),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MAIL_SOURCE,
    },
    ButtonSpec {
        name: "delete",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 156, y: 414 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(557, 558, 559),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: MAIL_SOURCE,
    },
    ButtonSpec {
        name: "block_list",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 183, y: 414 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(520, 521, 522),
        variants: &[],
        variant_names: &[],
        status: Status::Option("source sets GrayScale=true and Enabled=false"),
        source: MAIL_SOURCE,
    },
    ButtonSpec {
        name: "bug_report",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 210, y: 414 }),
            size: None,
        },
        library: Library::Prguse,
        sprites: SpriteTriple::new(523, 524, 525),
        variants: &[],
        variant_names: &[],
        status: Status::Option("source sets GrayScale=true and Enabled=false"),
        source: MAIL_SOURCE,
    },
];

const SHOP_BUTTONS: &[ButtonSpec] = &[
    ButtonSpec {
        name: "close",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 671, y: 4 }),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(360, 361, 362),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "category_up",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 120, y: 103 }),
            size: Some(Size {
                width: 16,
                height: 14,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(197, 198, 199),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "category_down",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 120, y: 421 }),
            size: Some(Size {
                width: 16,
                height: 14,
            }),
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(207, 208, 209),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "category_position_bar",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 120, y: 117 }),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(205, 206, 206),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "section_all",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 138, y: 68 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple {
            normal: Some(770),
            hover: None,
            pressed: None,
        },
        variants: &[],
        variant_names: &[],
        status: Status::Option("source does not assign HoverIndex or PressedIndex"),
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "section_top",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 209, y: 68 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple {
            normal: Some(776),
            hover: None,
            pressed: None,
        },
        variants: &[],
        variant_names: &[],
        status: Status::Option("source does not assign HoverIndex or PressedIndex"),
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "section_deals",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 280, y: 68 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple {
            normal: Some(772),
            hover: None,
            pressed: None,
        },
        variants: &[],
        variant_names: &[],
        status: Status::Option("source does not assign HoverIndex or PressedIndex"),
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "section_new",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 351, y: 68 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple {
            normal: Some(774),
            hover: None,
            pressed: None,
        },
        variants: &[],
        variant_names: &[],
        status: Status::Option("source starts Visible=false"),
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "class_all",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 539, y: 37 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(751, 752, 753),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "class_warrior",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 568, y: 38 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(754, 755, 756),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "class_assassin",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 591, y: 38 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(757, 758, 759),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "class_taoist",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 614, y: 38 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(760, 761, 762),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "class_wizard",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 637, y: 38 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(763, 764, 765),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "class_archer",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 660, y: 38 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(766, 767, 768),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "previous",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 600, y: 448 }),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(240, 241, 242),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: SHOP_SOURCE,
    },
    ButtonSpec {
        name: "next",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 660, y: 448 }),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(243, 244, 245),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: SHOP_SOURCE,
    },
];

const NPC_GOODS_BUTTONS: &[ButtonSpec] = &[
    ButtonSpec {
        name: "close",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 217, y: 3 }),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(360, 361, 362),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: NPC_GOODS_SOURCE,
    },
    ButtonSpec {
        name: "buy",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 77, y: 304 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(312, 313, 314),
        variants: &[],
        variant_names: &[],
        status: Status::Option("hidden for PanelType.Craft"),
        source: NPC_GOODS_SOURCE,
    },
    ButtonSpec {
        name: "up",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 219, y: 35 }),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(197, 198, 199),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: NPC_GOODS_SOURCE,
    },
    ButtonSpec {
        name: "down",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 219, y: 284 }),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(207, 208, 209),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: NPC_GOODS_SOURCE,
    },
    ButtonSpec {
        name: "position_bar",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 219, y: 49 }),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(205, 206, 206),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: NPC_GOODS_SOURCE,
    },
];

const STORAGE_BUTTONS: &[ButtonSpec] = &[
    ButtonSpec {
        name: "storage_1",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 8, y: 36 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(743, 743, 744),
        variants: &[
            SpriteTriple::new(743, 743, 744),
            SpriteTriple::new(744, 744, 744),
        ],
        variant_names: &["storage_1_active", "storage_2_active"],
        status: Status::Confirmed,
        source: STORAGE_SOURCE,
    },
    ButtonSpec {
        name: "storage_2",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 80, y: 36 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(746, 746, 746),
        variants: &[
            SpriteTriple::new(746, 746, 746),
            SpriteTriple::new(745, 745, 745),
        ],
        variant_names: &["storage_1_active", "storage_2_active"],
        status: Status::Confirmed,
        source: STORAGE_SOURCE,
    },
    ButtonSpec {
        name: "rent",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 283, y: 33 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(483, 484, 485),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: STORAGE_SOURCE,
    },
    ButtonSpec {
        name: "protect",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 328, y: 33 }),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(113, 114, 115),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: STORAGE_SOURCE,
    },
    ButtonSpec {
        name: "close",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 363, y: 3 }),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(360, 361, 362),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: STORAGE_SOURCE,
    },
];

const BIG_MAP_BUTTONS: &[ButtonSpec] = &[
    ButtonSpec {
        name: "scroll_up",
        rect: RectSpec {
            position: Position::Formula("x = panel.width - 21; y = 48"),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(197, 198, 199),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: BIG_MAP_CONSTRUCTOR_SOURCE,
    },
    ButtonSpec {
        name: "scroll_down",
        rect: RectSpec {
            position: Position::Formula("x = panel.width - 21; y = 417"),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(207, 208, 209),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: BIG_MAP_CONSTRUCTOR_SOURCE,
    },
    ButtonSpec {
        name: "scroll_bar",
        rect: RectSpec {
            position: Position::Formula("x = panel.width - 21; y = scroll_up.y + 13"),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(205, 206, 206),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: BIG_MAP_CONSTRUCTOR_SOURCE,
    },
    ButtonSpec {
        name: "world",
        rect: RectSpec {
            position: Position::Formula("x = 250; y = panel.height - 33"),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(827, 828, 829),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: BIG_MAP_CONSTRUCTOR_SOURCE,
    },
    ButtonSpec {
        name: "my_location",
        rect: RectSpec {
            position: Position::Formula("x = 400; y = panel.height - 33"),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(824, 825, 826),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: BIG_MAP_CONSTRUCTOR_SOURCE,
    },
    ButtonSpec {
        name: "teleport_to_npc",
        rect: RectSpec {
            position: Position::Formula("x = panel.width - 122; y = 432"),
            size: None,
        },
        library: Library::Title,
        sprites: SpriteTriple::new(821, 822, 823),
        variants: &[],
        variant_names: &[],
        status: Status::Option(
            "DisabledIndex is also 823; enabled only for selected same-map teleportable NPC",
        ),
        source: BIG_MAP_CONSTRUCTOR_SOURCE,
    },
    ButtonSpec {
        name: "search",
        rect: RectSpec {
            position: Position::Formula("x = 23; y = panel.height - 36"),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(1340, 1341, 1342),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: BIG_MAP_CONSTRUCTOR_SOURCE,
    },
    ButtonSpec {
        name: "close",
        rect: RectSpec {
            position: Position::Formula("x = panel.width - 25; y = 3"),
            size: None,
        },
        library: Library::Prguse2,
        sprites: SpriteTriple::new(360, 361, 362),
        variants: &[],
        variant_names: &[],
        status: Status::Confirmed,
        source: BIG_MAP_CONSTRUCTOR_SOURCE,
    },
];

const BIG_MAP_REGIONS: &[RegionSpec] = &[
    RegionSpec {
        name: "viewport",
        rect: RectSpec {
            position: Position::Formula(
                "x = 14 + (568 - view.width)/2; y = 52 + (380 - view.height)/2",
            ),
            size: Some(Size {
                width: 568,
                height: 380,
            }),
        },
        sprite: None,
        status: Status::Confirmed,
        source: BIG_MAP_VIEWPORT_SOURCE,
    },
    RegionSpec {
        name: "world_map_image",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 10, y: 0 }),
            size: None,
        },
        sprite: Some(SpriteRef {
            library: Library::Prguse2,
            index: 1360,
        }),
        status: Status::Confirmed,
        source: BIG_MAP_WORLD_SOURCE,
    },
    RegionSpec {
        name: "world_map_clouds",
        rect: RectSpec {
            position: Position::Formula("child origin"),
            size: None,
        },
        sprite: Some(SpriteRef {
            library: Library::Prguse2,
            index: 1365,
        }),
        status: Status::Confirmed,
        source: BIG_MAP_WORLD_SOURCE,
    },
    RegionSpec {
        name: "world_map_border",
        rect: RectSpec {
            position: Position::Formula("child origin"),
            size: None,
        },
        sprite: Some(SpriteRef {
            library: Library::Prguse2,
            index: 1366,
        }),
        status: Status::Confirmed,
        source: BIG_MAP_WORLD_SOURCE,
    },
    RegionSpec {
        name: "map_title_current_record",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 19, y: 6 }),
            size: Some(Size {
                width: 699,
                height: 20,
            }),
        },
        sprite: None,
        status: Status::Confirmed,
        source: BIG_MAP_CONSTRUCTOR_SOURCE,
    },
    RegionSpec {
        name: "coordinate_label",
        rect: RectSpec {
            position: Position::Absolute(Point { x: 519, y: 435 }),
            size: None,
        },
        sprite: None,
        status: Status::Option("AutoSize=true; exact text bounds depend on font/text"),
        source: BIG_MAP_CONSTRUCTOR_SOURCE,
    },
    RegionSpec {
        name: "search_text_box",
        rect: RectSpec {
            position: Position::Formula("x = 59; y = panel.height - 27"),
            size: Some(Size {
                width: 130,
                height: 10,
            }),
        },
        sprite: None,
        status: Status::Confirmed,
        source: BIG_MAP_CONSTRUCTOR_SOURCE,
    },
    RegionSpec {
        name: "selected_npc_icon",
        rect: RectSpec {
            position: Position::Formula("map position scaled into viewport"),
            size: None,
        },
        sprite: None,
        status: Status::Option(
            "library is MapLinkIcon and index is selected NPC info.Icon, not a fixed constant",
        ),
        source: BIG_MAP_VIEWPORT_SOURCE,
    },
    RegionSpec {
        name: "my_location_radar_dot",
        rect: RectSpec {
            position: Position::Formula("current player map position scaled into viewport"),
            size: None,
        },
        sprite: Some(SpriteRef {
            library: Library::Prguse2,
            index: 1350,
        }),
        status: Status::Confirmed,
        source: BIG_MAP_VIEWPORT_SOURCE,
    },
];

const BIG_MAP_GRIDS: &[GridSpec] = &[GridSpec {
    name: "npc_rows",
    origin: Position::Absolute(Point { x: 590, y: 50 }),
    columns: Some(1),
    rows: Some(18),
    capacity: Some(18),
    visible_per_page: Some(18),
    pages: None,
    cell_size: Some(Size {
        width: 140,
        height: 25,
    }),
    step: Some(Point { x: 0, y: 21 }),
    coordinate_formula: "x = 590; y = 50 + (i - scroll_offset)*21; MaximumRows = 18",
    status: Status::Confirmed,
    source: BIG_MAP_NPC_ROW_SOURCE,
}];

const BIG_MAP_PAGINATION: &[PaginationSpec] = &[PaginationSpec {
    name: "npc_scroll",
    page_size: Some(18),
    page_count: None,
    label: None,
    previous: Some(Position::Formula("scroll_up at panel.width - 21, y=48")),
    next: Some(Position::Formula("scroll_down at panel.width - 21, y=417")),
    source: BIG_MAP_SOURCE,
}];

const MAIL_GRIDS: &[GridSpec] = &[GridSpec {
    name: "mail_rows",
    origin: Position::Absolute(Point { x: 10, y: 55 }),
    columns: Some(1),
    rows: Some(10),
    capacity: Some(10),
    visible_per_page: Some(10),
    pages: None,
    cell_size: Some(Size {
        width: 290,
        height: 33,
    }),
    step: Some(Point { x: 0, y: 33 }),
    coordinate_formula: "x = 10; y = 55 + i*33",
    status: Status::Confirmed,
    source: MAIL_ROW_SOURCE,
}];

const ASSIGN_GRIDS: &[GridSpec] = &[GridSpec {
    name: "assign_key_cells",
    origin: Position::Absolute(Point { x: 17, y: 58 }),
    columns: Some(8),
    rows: Some(2),
    capacity: Some(16),
    visible_per_page: Some(16),
    pages: None,
    cell_size: None,
    step: Some(Point { x: 32, y: 37 }),
    coordinate_formula:
        "x = 17 + 32*(i%8) + 5*(i%8/4); y = 58 + 37*(i/8); keyStrings.Length is 8 or 16",
    status: Status::Confirmed,
    source: ASSIGN_SOURCE,
}];

const SHOP_GRIDS: &[GridSpec] = &[
    GridSpec {
        name: "shop_items",
        origin: Position::Absolute(Point { x: 152, y: 115 }),
        columns: Some(4),
        rows: Some(2),
        capacity: Some(8),
        visible_per_page: Some(8),
        pages: None,
        cell_size: Some(Size {
            width: 125,
            height: 146,
        }),
        step: Some(Point { x: 132, y: 160 }),
        coordinate_formula: "x = 152 + 132*(i%4); y = i<4 ? 115 : 275",
        status: Status::Confirmed,
        source: SHOP_UPDATE_SOURCE,
    },
    GridSpec {
        name: "category_filters",
        origin: Position::Absolute(Point { x: 15, y: 103 }),
        columns: Some(1),
        rows: Some(22),
        capacity: Some(22),
        visible_per_page: Some(22),
        pages: None,
        cell_size: Some(Size {
            width: 90,
            height: 20,
        }),
        step: Some(Point { x: 0, y: 15 }),
        coordinate_formula: "x = 15; y = 103 + 15*i",
        status: Status::Confirmed,
        source: SHOP_SOURCE,
    },
];

const NPC_GOODS_GRIDS: &[GridSpec] = &[GridSpec {
    name: "npc_goods",
    origin: Position::Absolute(Point { x: 10, y: 34 }),
    columns: Some(1),
    rows: Some(8),
    capacity: Some(8),
    visible_per_page: Some(8),
    pages: None,
    cell_size: None,
    step: Some(Point { x: 0, y: 33 }),
    coordinate_formula: "x = 10; y = 34 + i*33; DisplayGoods is scrolled by StartIndex",
    status: Status::Confirmed,
    source: NPC_GOODS_SOURCE,
}];

const STORAGE_GRIDS: &[GridSpec] = &[GridSpec {
    name: "storage",
    origin: Position::Absolute(Point { x: 9, y: 60 }),
    columns: Some(10),
    rows: Some(16),
    capacity: Some(160),
    visible_per_page: Some(80),
    pages: Some(2),
    cell_size: None,
    step: Some(Point { x: 37, y: 33 }),
    coordinate_formula:
        "idx = 10*y+x; x = 9 + 37*x; y = 60 + 33*(y%8); Storage1/Storage2 controls visibility",
    status: Status::Confirmed,
    source: STORAGE_SOURCE,
}];

const MAIL_PAGINATION: &[PaginationSpec] = &[PaginationSpec {
    name: "mail_pages",
    page_size: Some(10),
    page_count: None,
    label: Some(RectSpec {
        position: Position::Formula("x = 120; y = panel.height - 55"),
        size: Some(Size {
            width: 67,
            height: 15,
        }),
    }),
    previous: Some(Position::Formula("x = 102; y = panel.height - 55")),
    next: Some(Position::Formula("x = 192; y = panel.height - 55")),
    source: MAIL_SOURCE,
}];

const SHOP_PAGINATION: &[PaginationSpec] = &[PaginationSpec {
    name: "shop_pages",
    page_size: Some(8),
    page_count: None,
    label: Some(RectSpec {
        position: Position::Absolute(Point { x: 597, y: 446 }),
        size: Some(Size {
            width: 83,
            height: 17,
        }),
    }),
    previous: Some(Position::Absolute(Point { x: 600, y: 448 })),
    next: Some(Position::Absolute(Point { x: 660, y: 448 })),
    source: SHOP_SOURCE,
}];

const OPTION_PANELS: &[GridSpec] = &[];
const EMPTY_PAGINATION: &[PaginationSpec] = &[];

pub const SYSTEM_MENU: PanelSpec = PanelSpec {
    name: "system_menu",
    origin: Position::Formula(
        "x = Settings.ScreenWidth - panel.width; y = MainDialog.Location.Y - panel.height + 15",
    ),
    size: None,
    background: Some(SpriteRef {
        library: Library::Title,
        index: 567,
    }),
    buttons: MENU_BUTTONS,
    regions: &[],
    grids: &[],
    pagination: EMPTY_PAGINATION,
    notes: &[
        "Panel size is inherited from Title/567 and is not numerically declared in MainDialogs.cs.",
    ],
    status: Status::Confirmed,
    source: MENU_SOURCE,
};

pub const OPTION: PanelSpec = PanelSpec {
    name: "options",
    origin: Position::Center,
    size: None,
    background: Some(SpriteRef {
        library: Library::Title,
        index: 411,
    }),
    buttons: OPTION_BUTTONS,
    regions: &[],
    grids: OPTION_PANELS,
    pagination: EMPTY_PAGINATION,
    notes: &[
        "Close hides the dialog; Observe is an authoritative @ALLOWOBSERVE request, not a local-only toggle.",
    ],
    status: Status::Confirmed,
    source: OPTION_SOURCE,
};

pub const SKILL_ASSIGN_KEY: PanelSpec = PanelSpec {
    name: "skill_assign_key",
    origin: Position::Center,
    size: None,
    background: Some(SpriteRef {
        library: Library::Prguse,
        index: 710,
    }),
    buttons: ASSIGN_BUTTONS,
    regions: &[],
    grids: ASSIGN_GRIDS,
    pagination: EMPTY_PAGINATION,
    notes: &[
        "MagicImage is MagIcon2 index = magic.Icon*2 at (16,16); FKeys has 8 or 16 entries depending on keyStrings.Length.",
    ],
    status: Status::Confirmed,
    source: ASSIGN_SOURCE,
};

pub const MAIL: PanelSpec = PanelSpec {
    name: "mail_list",
    origin: Position::Formula("x = Settings.ScreenWidth - panel.width - 150; y = 5"),
    size: Some(Size {
        width: 312,
        height: 444,
    }),
    background: Some(SpriteRef {
        library: Library::Title,
        index: 670,
    }),
    buttons: MAIL_BUTTONS,
    regions: &[],
    grids: MAIL_GRIDS,
    pagination: MAIL_PAGINATION,
    notes: &[
        "Rows array length is 10; PageCount is computed from user mail count with page size 10.",
    ],
    status: Status::Confirmed,
    source: MAIL_SOURCE,
};

pub const GAME_SHOP: PanelSpec = PanelSpec {
    name: "game_shop",
    origin: Position::Center,
    size: None,
    background: Some(SpriteRef {
        library: Library::Title,
        index: 749,
    }),
    buttons: SHOP_BUTTONS,
    regions: &[],
    grids: SHOP_GRIDS,
    pagination: SHOP_PAGINATION,
    notes: &[
        "Panel size is inherited from Title/749; the source only gives Center, not numeric dimensions.",
    ],
    status: Status::Confirmed,
    source: SHOP_SOURCE,
};

pub const NPC_SHOP: PanelSpec = PanelSpec {
    name: "npc_goods_shop",
    origin: Position::Absolute(Point { x: 0, y: 224 }),
    size: None,
    background: Some(SpriteRef {
        library: Library::Prguse,
        index: 1000,
    }),
    buttons: NPC_GOODS_BUTTONS,
    regions: &[],
    grids: NPC_GOODS_GRIDS,
    pagination: EMPTY_PAGINATION,
    notes: &[
        "NPCGoodsDialog has eight visible goods cells; BuyButton is hidden for PanelType.Craft. Source does not declare panel/cell dimensions.",
    ],
    status: Status::Confirmed,
    source: NPC_GOODS_SOURCE,
};

pub const WAREHOUSE: PanelSpec = PanelSpec {
    name: "warehouse_storage",
    origin: Position::Absolute(Point { x: 0, y: 0 }),
    size: None,
    background: Some(SpriteRef {
        library: Library::Prguse,
        index: 586,
    }),
    buttons: STORAGE_BUTTONS,
    regions: &[],
    grids: STORAGE_GRIDS,
    pagination: EMPTY_PAGINATION,
    notes: &[
        "Crystal allocates 10*16 cells; each Storage1/Storage2 view exposes 80 cells. Cell size is inherited by MirItemCell and is not declared here.",
    ],
    status: Status::Confirmed,
    source: STORAGE_SOURCE,
};

pub const BIG_MAP: PanelSpec = PanelSpec {
    name: "big_map",
    origin: Position::Center,
    size: None,
    background: Some(SpriteRef {
        library: Library::Title,
        index: 820,
    }),
    buttons: BIG_MAP_BUTTONS,
    regions: BIG_MAP_REGIONS,
    grids: BIG_MAP_GRIDS,
    pagination: BIG_MAP_PAGINATION,
    notes: &[
        "Panel size is inherited from Title/820 and is not numerically declared in BigMapDialog.cs.",
        "MaximumRows is exactly 18; Crystal has no Zoom control.",
        "World/current state is represented by WorldButton, CurrentRecord-driven title/radar, and MyLocationButton; no fake teleport success is encoded here.",
    ],
    status: Status::Confirmed,
    source: BIG_MAP_SOURCE,
};

pub const CREDITS: PanelSpec = PanelSpec {
    name: "credits",
    origin: Position::Center,
    size: None,
    background: None,
    buttons: &[],
    regions: &[],
    grids: &[],
    pagination: EMPTY_PAGINATION,
    notes: &[
        "No Credits dialog, background, or button declaration was found in the four requested Crystal source files.",
    ],
    status: Status::Unsupported(
        "not declared in MainDialogs.cs, MailDialogs.cs, GameshopDialog.cs, or NPCDialogs.cs",
    ),
    source: SourceRef::no_declaration(MAIN),
};

pub const ALL_PANELS: &[&PanelSpec] = &[
    &SYSTEM_MENU,
    &OPTION,
    &SKILL_ASSIGN_KEY,
    &MAIL,
    &GAME_SHOP,
    &NPC_SHOP,
    &WAREHOUSE,
    &BIG_MAP,
    &CREDITS,
];

#[cfg(test)]
fn absolute_rect(spec: &RectSpec) -> Option<(Point, Size)> {
    match (spec.position, spec.size) {
        (Position::Absolute(point), Some(size)) => Some((point, size)),
        _ => None,
    }
}

#[cfg(test)]
fn triple_is_complete(triple: SpriteTriple) -> bool {
    triple.normal.is_some() && triple.hover.is_some() && triple.pressed.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_rects_do_not_escape_explicit_mail_panel() {
        let panel = MAIL.size.expect("mail size is source-confirmed");
        let mut checked = 0;
        for button in MAIL.buttons {
            if let Some((point, size)) = absolute_rect(&button.rect) {
                assert!(
                    point.x >= 0 && point.y >= 0,
                    "{} has negative origin",
                    button.name
                );
                assert!(
                    point.x as u32 + size.width <= panel.width,
                    "{} exceeds width",
                    button.name
                );
                assert!(
                    point.y as u32 + size.height <= panel.height,
                    "{} exceeds height",
                    button.name
                );
                checked += 1;
            }
        }
        for grid in MAIL.grids {
            let point = match grid.origin {
                Position::Absolute(point) => point,
                _ => panic!("mail grid must have an absolute origin"),
            };
            let cell = grid.cell_size.expect("mail row size is source-confirmed");
            let step = grid.step.expect("mail row step is source-confirmed");
            let rows = grid.rows.expect("mail row count is source-confirmed");
            let last_x = point.x + step.x * (grid.columns.unwrap() as i32 - 1);
            let last_y = point.y + step.y * (rows as i32 - 1);
            assert!(last_x >= 0 && last_y >= 0);
            assert!(last_x as u32 + cell.width <= panel.width);
            assert!(last_y as u32 + cell.height <= panel.height);
            checked += 1;
        }
        assert!(checked > 0);
    }

    #[test]
    fn source_confirmed_fixed_triples_are_complete() {
        for button in SYSTEM_MENU.buttons {
            assert!(
                triple_is_complete(button.sprites),
                "{} is not a triple",
                button.name
            );
        }
        for button in SKILL_ASSIGN_KEY.buttons {
            assert!(
                triple_is_complete(button.sprites),
                "{} is not a triple",
                button.name
            );
        }
        for button in MAIL.buttons {
            assert!(
                triple_is_complete(button.sprites),
                "{} is not a triple",
                button.name
            );
        }
        for button in BIG_MAP.buttons {
            assert!(
                triple_is_complete(button.sprites),
                "{} is not a triple",
                button.name
            );
        }
        for button in NPC_SHOP.buttons {
            assert!(
                triple_is_complete(button.sprites),
                "{} is not a triple",
                button.name
            );
        }
        for button in WAREHOUSE.buttons {
            assert!(
                triple_is_complete(button.sprites),
                "{} is not a triple",
                button.name
            );
        }
    }

    #[test]
    fn sprite_variant_counts_match_source_state_matrices() {
        assert_eq!(OPTION.buttons.len(), 17);
        for button in OPTION.buttons.iter().skip(1) {
            assert_eq!(button.variants.len(), button.variant_names.len());
            assert!(
                !button.variants.is_empty(),
                "{} lost its state variants",
                button.name
            );
            for triple in button.variants {
                assert!(triple_is_complete(*triple) || triple.hover.is_none());
                assert!(triple.normal.is_some());
                assert!(triple.pressed.is_some());
            }
        }
        assert_eq!(SYSTEM_MENU.buttons.len(), 14);
        assert_eq!(MAIL.buttons.len(), 10);
        assert_eq!(GAME_SHOP.grids[0].capacity, Some(8));
        assert_eq!(NPC_SHOP.grids[0].visible_per_page, Some(8));
        assert_eq!(WAREHOUSE.grids[0].capacity, Some(160));
    }

    #[test]
    fn grid_quantities_are_bounded_and_consistent() {
        for panel in ALL_PANELS {
            for grid in panel.grids {
                if let (Some(columns), Some(rows), Some(capacity)) =
                    (grid.columns, grid.rows, grid.capacity)
                {
                    assert_eq!(
                        columns as u32 * rows as u32,
                        capacity as u32,
                        "{} capacity",
                        grid.name
                    );
                }
                if let (Some(capacity), Some(visible)) = (grid.capacity, grid.visible_per_page) {
                    assert!(visible <= capacity, "{} visible count", grid.name);
                }
                if let Some(pages) = grid.pages {
                    assert!(pages > 0);
                }
            }
        }
        assert_eq!(ASSIGN_GRIDS[0].capacity, Some(16));
        assert_eq!(MAIL_GRIDS[0].capacity, Some(10));
        assert_eq!(SHOP_GRIDS[1].capacity, Some(22));
    }

    #[test]
    fn big_map_npc_rows_fit_the_declared_list_lane() {
        let grid = BIG_MAP_GRIDS[0];
        let origin = match grid.origin {
            Position::Absolute(point) => point,
            _ => panic!("NPC rows need an absolute origin"),
        };
        let step = grid.step.expect("NPC row step is source-confirmed");
        let cell = grid.cell_size.expect("NPC row size is source-confirmed");
        let rows = grid.rows.expect("MaximumRows is source-confirmed");
        let last_y = origin.y + step.y * (rows as i32 - 1);
        assert_eq!(rows, 18);
        assert!(origin.x >= 0 && last_y >= 0);
        assert_eq!(origin.x + cell.width as i32, 730);
        assert_eq!(last_y + cell.height as i32, 432);
        assert!(!BIG_MAP.buttons.iter().any(|button| button.name == "zoom"));
    }

    #[test]
    fn native_inventory_skill_and_shop_viewports_are_bounded() {
        let inventory_last = Point {
            x: INVENTORY_GRID_ORIGIN.x
                + INVENTORY_GRID_STEP.x * (INVENTORY_PAGE_COLUMNS as i32 - 1),
            y: INVENTORY_GRID_ORIGIN.y + INVENTORY_GRID_STEP.y * (INVENTORY_PAGE_ROWS as i32 - 1),
        };
        assert!(inventory_last.x as u32 + INVENTORY_CELL_SIZE.width <= INVENTORY_PANEL_SIZE.width);
        assert!(
            inventory_last.y as u32 + INVENTORY_CELL_SIZE.height <= INVENTORY_PANEL_SIZE.height
        );
        assert_eq!(INVENTORY_PAGE_SIZE, 40);
        assert_eq!(INVENTORY_GOLD_LABEL_ORIGIN, Point { x: 40, y: 212 });
        assert_eq!(
            INVENTORY_GOLD_LABEL_SIZE,
            Size {
                width: 111,
                height: 14
            }
        );
        assert_eq!(INVENTORY_WEIGHT_BAR_ORIGIN, Point { x: 182, y: 217 });
        assert_eq!(
            INVENTORY_WEIGHT_BAR_SIZE,
            Size {
                width: 84,
                height: 6
            }
        );
        assert_eq!(INVENTORY_FREE_SLOT_LABEL_ORIGIN, Point { x: 268, y: 212 });
        assert_eq!(
            INVENTORY_FREE_SLOT_LABEL_SIZE,
            Size {
                width: 26,
                height: 14
            }
        );
        assert_eq!(INVENTORY_DELETE_BUTTON_ORIGIN, Point { x: 291, y: 212 });
        assert_eq!(
            INVENTORY_DELETE_BUTTON_SIZE,
            Size {
                width: 16,
                height: 15
            }
        );
        assert_eq!(
            INVENTORY_FOOTER_SOURCE,
            SourceRef::new(INVENTORY_FILE, 23, 195)
        );

        let skill_last_y = SKILL_ROW_ORIGIN.y + SKILL_ROW_STEP_Y * (SKILL_PAGE_SIZE as i32 - 1);
        assert!(SKILL_ROW_ORIGIN.x as u32 + SKILL_ROW_SIZE.width <= SKILL_PANEL_SIZE.width);
        assert!(skill_last_y as u32 + SKILL_ROW_SIZE.height <= SKILL_PANEL_SIZE.height);

        let shop_last_x =
            GAME_SHOP_GRID_ORIGIN.x + GAME_SHOP_COLUMN_STEP * (GAME_SHOP_PAGE_COLUMNS as i32 - 1);
        let shop_last_y =
            GAME_SHOP_GRID_ORIGIN.y + GAME_SHOP_ROW_STEP * (GAME_SHOP_PAGE_ROWS as i32 - 1);
        assert!(shop_last_x as u32 + GAME_SHOP_CELL_SIZE.width <= GAME_SHOP_PANEL_SIZE.width);
        assert!(shop_last_y as u32 + GAME_SHOP_CELL_SIZE.height <= GAME_SHOP_PANEL_SIZE.height);
        assert_eq!(GAME_SHOP_PAGE_SIZE, 8);
    }

    #[test]
    fn unknown_geometry_is_explicitly_unresolved() {
        for panel in [
            &SYSTEM_MENU,
            &OPTION,
            &SKILL_ASSIGN_KEY,
            &GAME_SHOP,
            &NPC_SHOP,
            &WAREHOUSE,
        ] {
            assert!(panel.size.is_none(), "{} guessed a panel size", panel.name);
        }
        assert!(matches!(CREDITS.status, Status::Unsupported(_)));
        assert!(CREDITS.background.is_none());
    }
}
