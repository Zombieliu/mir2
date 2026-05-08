use crate::error::{PacketCodecError, Result};
use crate::io::{PacketReader, PacketWriter};
use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirGender {
    Male = 0,
    Female = 1,
}

impl TryFrom<u8> for MirGender {
    type Error = PacketCodecError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Male),
            1 => Ok(Self::Female),
            other => Err(PacketCodecError::InvalidEnumValue {
                type_name: "MirGender",
                value: other,
            }),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirClass {
    Warrior = 0,
    Wizard = 1,
    Taoist = 2,
    Assassin = 3,
    Archer = 4,
}

impl TryFrom<u8> for MirClass {
    type Error = PacketCodecError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Warrior),
            1 => Ok(Self::Wizard),
            2 => Ok(Self::Taoist),
            3 => Ok(Self::Assassin),
            4 => Ok(Self::Archer),
            other => Err(PacketCodecError::InvalidEnumValue {
                type_name: "MirClass",
                value: other,
            }),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirDirection {
    Up = 0,
    UpRight = 1,
    Right = 2,
    DownRight = 3,
    Down = 4,
    DownLeft = 5,
    Left = 6,
    UpLeft = 7,
}

impl TryFrom<u8> for MirDirection {
    type Error = PacketCodecError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Up),
            1 => Ok(Self::UpRight),
            2 => Ok(Self::Right),
            3 => Ok(Self::DownRight),
            4 => Ok(Self::Down),
            5 => Ok(Self::DownLeft),
            6 => Ok(Self::Left),
            7 => Ok(Self::UpLeft),
            other => Err(PacketCodecError::InvalidEnumValue {
                type_name: "MirDirection",
                value: other,
            }),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatType {
    Normal = 0,
    Shout = 1,
    System = 2,
    Hint = 3,
    Announcement = 4,
    Group = 5,
    WhisperIn = 6,
    WhisperOut = 7,
    Guild = 8,
    Trainer = 9,
}

impl TryFrom<u8> for ChatType {
    type Error = PacketCodecError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Normal),
            1 => Ok(Self::Shout),
            2 => Ok(Self::System),
            3 => Ok(Self::Hint),
            4 => Ok(Self::Announcement),
            5 => Ok(Self::Group),
            6 => Ok(Self::WhisperIn),
            7 => Ok(Self::WhisperOut),
            8 => Ok(Self::Guild),
            9 => Ok(Self::Trainer),
            other => Err(PacketCodecError::InvalidEnumValue {
                type_name: "ChatType",
                value: other,
            }),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirGridType {
    None = 0,
    Inventory = 1,
    Equipment = 2,
    Trade = 3,
    Storage = 4,
    BuyBack = 5,
    DropPanel = 6,
    Inspect = 7,
    TrustMerchant = 8,
    GuildStorage = 9,
    GuestTrade = 10,
    Mount = 11,
    Fishing = 12,
    QuestInventory = 13,
    AwakenItem = 14,
    Mail = 15,
    Refine = 16,
    Renting = 17,
    GuestRenting = 18,
    Craft = 19,
    Socket = 20,
    HeroEquipment = 21,
    HeroInventory = 22,
    HeroHpItem = 23,
    HeroMpItem = 24,
    Belt = 25,
}

impl TryFrom<u8> for MirGridType {
    type Error = PacketCodecError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Inventory),
            2 => Ok(Self::Equipment),
            3 => Ok(Self::Trade),
            4 => Ok(Self::Storage),
            5 => Ok(Self::BuyBack),
            6 => Ok(Self::DropPanel),
            7 => Ok(Self::Inspect),
            8 => Ok(Self::TrustMerchant),
            9 => Ok(Self::GuildStorage),
            10 => Ok(Self::GuestTrade),
            11 => Ok(Self::Mount),
            12 => Ok(Self::Fishing),
            13 => Ok(Self::QuestInventory),
            14 => Ok(Self::AwakenItem),
            15 => Ok(Self::Mail),
            16 => Ok(Self::Refine),
            17 => Ok(Self::Renting),
            18 => Ok(Self::GuestRenting),
            19 => Ok(Self::Craft),
            20 => Ok(Self::Socket),
            21 => Ok(Self::HeroEquipment),
            22 => Ok(Self::HeroInventory),
            23 => Ok(Self::HeroHpItem),
            24 => Ok(Self::HeroMpItem),
            25 => Ok(Self::Belt),
            other => Err(PacketCodecError::InvalidEnumValue {
                type_name: "MirGridType",
                value: other,
            }),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Spell {
    None = 0,
    Fencing = 1,
    Slaying = 2,
    Thrusting = 3,
    HalfMoon = 4,
    ShoulderDash = 5,
    TwinDrakeBlade = 6,
    Entrapment = 7,
    FlamingSword = 8,
    LionRoar = 9,
    CrossHalfMoon = 10,
    BladeAvalanche = 11,
    ProtectionField = 12,
    Rage = 13,
    CounterAttack = 14,
    SlashingBurst = 15,
    Fury = 16,
    ImmortalSkin = 17,
    FireBall = 31,
    Repulsion = 32,
    ElectricShock = 33,
    GreatFireBall = 34,
    HellFire = 35,
    ThunderBolt = 36,
    Teleport = 37,
    FireBang = 38,
    FireWall = 39,
    Lightning = 40,
    FrostCrunch = 41,
    ThunderStorm = 42,
    MagicShield = 43,
    TurnUndead = 44,
    Vampirism = 45,
    IceStorm = 46,
    FlameDisruptor = 47,
    Mirroring = 48,
    FlameField = 49,
    Blizzard = 50,
    MagicBooster = 51,
    MeteorStrike = 52,
    IceThrust = 53,
    FastMove = 54,
    StormEscape = 55,
    Healing = 61,
    SpiritSword = 62,
    Poisoning = 63,
    SoulFireBall = 64,
    SummonSkeleton = 65,
    Hiding = 67,
    MassHiding = 68,
    SoulShield = 69,
    Revelation = 70,
    BlessedArmour = 71,
    EnergyRepulsor = 72,
    TrapHexagon = 73,
    Purification = 74,
    MassHealing = 75,
    Hallucination = 76,
    UltimateEnhancer = 77,
    SummonShinsu = 78,
    Reincarnation = 79,
    SummonHolyDeva = 80,
    Curse = 81,
    Plague = 82,
    PoisonCloud = 83,
    EnergyShield = 84,
    PetEnhancer = 85,
    HealingCircle = 86,
    FatalSword = 91,
    DoubleSlash = 92,
    Haste = 93,
    FlashDash = 94,
    LightBody = 95,
    HeavenlySword = 96,
    FireBurst = 97,
    Trap = 98,
    PoisonSword = 99,
    MoonLight = 100,
    MPEater = 101,
    SwiftFeet = 102,
    DarkBody = 103,
    Hemorrhage = 104,
    CrescentSlash = 105,
    MoonMist = 106,
    CatTongue = 107,
    Focus = 121,
    StraightShot = 122,
    DoubleShot = 123,
    ExplosiveTrap = 124,
    DelayedExplosion = 125,
    Meditation = 126,
    BackStep = 127,
    ElementalShot = 128,
    Concentration = 129,
    Stonetrap = 130,
    ElementalBarrier = 131,
    SummonVampire = 132,
    VampireShot = 133,
    SummonToad = 134,
    PoisonShot = 135,
    CrippleShot = 136,
    SummonSnakes = 137,
    NapalmShot = 138,
    OneWithNature = 139,
    BindingShot = 140,
    MentalState = 141,
    Blink = 151,
    Portal = 152,
    BattleCry = 153,
    FireBounce = 154,
    MeteorShower = 155,
    DigOutZombie = 200,
    Rubble = 201,
    MapLightning = 202,
    MapLava = 203,
    MapQuake1 = 204,
    MapQuake2 = 205,
    DigOutArmadillo = 206,
    GeneralMeowMeowThunder = 207,
    StoneGolemQuake = 208,
    EarthGolemPile = 209,
    TreeQueenRoot = 210,
    TreeQueenMassRoots = 211,
    TreeQueenGroundRoots = 212,
    TucsonGeneralRock = 213,
    FlyingStatueIceTornado = 214,
    DarkOmaKingNuke = 215,
    HornedSorcererDustTornado = 216,
    HornedCommanderRockFall = 217,
    HornedCommanderRockSpike = 218,
}

impl TryFrom<u8> for Spell {
    type Error = PacketCodecError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Fencing),
            2 => Ok(Self::Slaying),
            3 => Ok(Self::Thrusting),
            4 => Ok(Self::HalfMoon),
            5 => Ok(Self::ShoulderDash),
            6 => Ok(Self::TwinDrakeBlade),
            7 => Ok(Self::Entrapment),
            8 => Ok(Self::FlamingSword),
            9 => Ok(Self::LionRoar),
            10 => Ok(Self::CrossHalfMoon),
            11 => Ok(Self::BladeAvalanche),
            12 => Ok(Self::ProtectionField),
            13 => Ok(Self::Rage),
            14 => Ok(Self::CounterAttack),
            15 => Ok(Self::SlashingBurst),
            16 => Ok(Self::Fury),
            17 => Ok(Self::ImmortalSkin),
            31 => Ok(Self::FireBall),
            32 => Ok(Self::Repulsion),
            33 => Ok(Self::ElectricShock),
            34 => Ok(Self::GreatFireBall),
            35 => Ok(Self::HellFire),
            36 => Ok(Self::ThunderBolt),
            37 => Ok(Self::Teleport),
            38 => Ok(Self::FireBang),
            39 => Ok(Self::FireWall),
            40 => Ok(Self::Lightning),
            41 => Ok(Self::FrostCrunch),
            42 => Ok(Self::ThunderStorm),
            43 => Ok(Self::MagicShield),
            44 => Ok(Self::TurnUndead),
            45 => Ok(Self::Vampirism),
            46 => Ok(Self::IceStorm),
            47 => Ok(Self::FlameDisruptor),
            48 => Ok(Self::Mirroring),
            49 => Ok(Self::FlameField),
            50 => Ok(Self::Blizzard),
            51 => Ok(Self::MagicBooster),
            52 => Ok(Self::MeteorStrike),
            53 => Ok(Self::IceThrust),
            54 => Ok(Self::FastMove),
            55 => Ok(Self::StormEscape),
            61 => Ok(Self::Healing),
            62 => Ok(Self::SpiritSword),
            63 => Ok(Self::Poisoning),
            64 => Ok(Self::SoulFireBall),
            65 => Ok(Self::SummonSkeleton),
            67 => Ok(Self::Hiding),
            68 => Ok(Self::MassHiding),
            69 => Ok(Self::SoulShield),
            70 => Ok(Self::Revelation),
            71 => Ok(Self::BlessedArmour),
            72 => Ok(Self::EnergyRepulsor),
            73 => Ok(Self::TrapHexagon),
            74 => Ok(Self::Purification),
            75 => Ok(Self::MassHealing),
            76 => Ok(Self::Hallucination),
            77 => Ok(Self::UltimateEnhancer),
            78 => Ok(Self::SummonShinsu),
            79 => Ok(Self::Reincarnation),
            80 => Ok(Self::SummonHolyDeva),
            81 => Ok(Self::Curse),
            82 => Ok(Self::Plague),
            83 => Ok(Self::PoisonCloud),
            84 => Ok(Self::EnergyShield),
            85 => Ok(Self::PetEnhancer),
            86 => Ok(Self::HealingCircle),
            91 => Ok(Self::FatalSword),
            92 => Ok(Self::DoubleSlash),
            93 => Ok(Self::Haste),
            94 => Ok(Self::FlashDash),
            95 => Ok(Self::LightBody),
            96 => Ok(Self::HeavenlySword),
            97 => Ok(Self::FireBurst),
            98 => Ok(Self::Trap),
            99 => Ok(Self::PoisonSword),
            100 => Ok(Self::MoonLight),
            101 => Ok(Self::MPEater),
            102 => Ok(Self::SwiftFeet),
            103 => Ok(Self::DarkBody),
            104 => Ok(Self::Hemorrhage),
            105 => Ok(Self::CrescentSlash),
            106 => Ok(Self::MoonMist),
            107 => Ok(Self::CatTongue),
            121 => Ok(Self::Focus),
            122 => Ok(Self::StraightShot),
            123 => Ok(Self::DoubleShot),
            124 => Ok(Self::ExplosiveTrap),
            125 => Ok(Self::DelayedExplosion),
            126 => Ok(Self::Meditation),
            127 => Ok(Self::BackStep),
            128 => Ok(Self::ElementalShot),
            129 => Ok(Self::Concentration),
            130 => Ok(Self::Stonetrap),
            131 => Ok(Self::ElementalBarrier),
            132 => Ok(Self::SummonVampire),
            133 => Ok(Self::VampireShot),
            134 => Ok(Self::SummonToad),
            135 => Ok(Self::PoisonShot),
            136 => Ok(Self::CrippleShot),
            137 => Ok(Self::SummonSnakes),
            138 => Ok(Self::NapalmShot),
            139 => Ok(Self::OneWithNature),
            140 => Ok(Self::BindingShot),
            141 => Ok(Self::MentalState),
            151 => Ok(Self::Blink),
            152 => Ok(Self::Portal),
            153 => Ok(Self::BattleCry),
            154 => Ok(Self::FireBounce),
            155 => Ok(Self::MeteorShower),
            200 => Ok(Self::DigOutZombie),
            201 => Ok(Self::Rubble),
            202 => Ok(Self::MapLightning),
            203 => Ok(Self::MapLava),
            204 => Ok(Self::MapQuake1),
            205 => Ok(Self::MapQuake2),
            206 => Ok(Self::DigOutArmadillo),
            207 => Ok(Self::GeneralMeowMeowThunder),
            208 => Ok(Self::StoneGolemQuake),
            209 => Ok(Self::EarthGolemPile),
            210 => Ok(Self::TreeQueenRoot),
            211 => Ok(Self::TreeQueenMassRoots),
            212 => Ok(Self::TreeQueenGroundRoots),
            213 => Ok(Self::TucsonGeneralRock),
            214 => Ok(Self::FlyingStatueIceTornado),
            215 => Ok(Self::DarkOmaKingNuke),
            216 => Ok(Self::HornedSorcererDustTornado),
            217 => Ok(Self::HornedCommanderRockFall),
            218 => Ok(Self::HornedCommanderRockSpike),
            other => Err(PacketCodecError::InvalidEnumValue {
                type_name: "Spell",
                value: other,
            }),
        }
    }
}

impl Spell {
    pub fn from_crystal_name(name: &str) -> Option<Self> {
        match name {
            "None" => Some(Self::None),
            "Fencing" => Some(Self::Fencing),
            "Slaying" => Some(Self::Slaying),
            "Thrusting" => Some(Self::Thrusting),
            "HalfMoon" => Some(Self::HalfMoon),
            "ShoulderDash" => Some(Self::ShoulderDash),
            "TwinDrakeBlade" => Some(Self::TwinDrakeBlade),
            "Entrapment" => Some(Self::Entrapment),
            "FlamingSword" => Some(Self::FlamingSword),
            "LionRoar" => Some(Self::LionRoar),
            "CrossHalfMoon" => Some(Self::CrossHalfMoon),
            "BladeAvalanche" => Some(Self::BladeAvalanche),
            "ProtectionField" => Some(Self::ProtectionField),
            "Rage" => Some(Self::Rage),
            "CounterAttack" => Some(Self::CounterAttack),
            "SlashingBurst" => Some(Self::SlashingBurst),
            "Fury" => Some(Self::Fury),
            "ImmortalSkin" => Some(Self::ImmortalSkin),
            "FireBall" => Some(Self::FireBall),
            "Repulsion" => Some(Self::Repulsion),
            "ElectricShock" => Some(Self::ElectricShock),
            "GreatFireBall" => Some(Self::GreatFireBall),
            "HellFire" => Some(Self::HellFire),
            "ThunderBolt" => Some(Self::ThunderBolt),
            "Teleport" => Some(Self::Teleport),
            "FireBang" => Some(Self::FireBang),
            "FireWall" => Some(Self::FireWall),
            "Lightning" => Some(Self::Lightning),
            "FrostCrunch" => Some(Self::FrostCrunch),
            "ThunderStorm" => Some(Self::ThunderStorm),
            "MagicShield" => Some(Self::MagicShield),
            "TurnUndead" => Some(Self::TurnUndead),
            "Vampirism" => Some(Self::Vampirism),
            "IceStorm" => Some(Self::IceStorm),
            "FlameDisruptor" => Some(Self::FlameDisruptor),
            "Mirroring" => Some(Self::Mirroring),
            "FlameField" => Some(Self::FlameField),
            "Blizzard" => Some(Self::Blizzard),
            "MagicBooster" => Some(Self::MagicBooster),
            "MeteorStrike" => Some(Self::MeteorStrike),
            "IceThrust" => Some(Self::IceThrust),
            "FastMove" => Some(Self::FastMove),
            "StormEscape" => Some(Self::StormEscape),
            "Healing" => Some(Self::Healing),
            "SpiritSword" => Some(Self::SpiritSword),
            "Poisoning" => Some(Self::Poisoning),
            "SoulFireBall" => Some(Self::SoulFireBall),
            "SummonSkeleton" => Some(Self::SummonSkeleton),
            "Hiding" => Some(Self::Hiding),
            "MassHiding" => Some(Self::MassHiding),
            "SoulShield" => Some(Self::SoulShield),
            "Revelation" => Some(Self::Revelation),
            "BlessedArmour" => Some(Self::BlessedArmour),
            "EnergyRepulsor" => Some(Self::EnergyRepulsor),
            "TrapHexagon" => Some(Self::TrapHexagon),
            "Purification" => Some(Self::Purification),
            "MassHealing" => Some(Self::MassHealing),
            "Hallucination" => Some(Self::Hallucination),
            "UltimateEnhancer" => Some(Self::UltimateEnhancer),
            "SummonShinsu" => Some(Self::SummonShinsu),
            "Reincarnation" => Some(Self::Reincarnation),
            "SummonHolyDeva" => Some(Self::SummonHolyDeva),
            "Curse" => Some(Self::Curse),
            "Plague" => Some(Self::Plague),
            "PoisonCloud" => Some(Self::PoisonCloud),
            "EnergyShield" => Some(Self::EnergyShield),
            "PetEnhancer" => Some(Self::PetEnhancer),
            "HealingCircle" => Some(Self::HealingCircle),
            "FatalSword" => Some(Self::FatalSword),
            "DoubleSlash" => Some(Self::DoubleSlash),
            "Haste" => Some(Self::Haste),
            "FlashDash" => Some(Self::FlashDash),
            "LightBody" => Some(Self::LightBody),
            "HeavenlySword" => Some(Self::HeavenlySword),
            "FireBurst" => Some(Self::FireBurst),
            "Trap" => Some(Self::Trap),
            "PoisonSword" => Some(Self::PoisonSword),
            "MoonLight" => Some(Self::MoonLight),
            "MPEater" => Some(Self::MPEater),
            "SwiftFeet" => Some(Self::SwiftFeet),
            "DarkBody" => Some(Self::DarkBody),
            "Hemorrhage" => Some(Self::Hemorrhage),
            "CrescentSlash" => Some(Self::CrescentSlash),
            "MoonMist" => Some(Self::MoonMist),
            "CatTongue" => Some(Self::CatTongue),
            "Focus" => Some(Self::Focus),
            "StraightShot" => Some(Self::StraightShot),
            "DoubleShot" => Some(Self::DoubleShot),
            "ExplosiveTrap" => Some(Self::ExplosiveTrap),
            "DelayedExplosion" => Some(Self::DelayedExplosion),
            "Meditation" => Some(Self::Meditation),
            "BackStep" => Some(Self::BackStep),
            "ElementalShot" => Some(Self::ElementalShot),
            "Concentration" => Some(Self::Concentration),
            "Stonetrap" => Some(Self::Stonetrap),
            "ElementalBarrier" => Some(Self::ElementalBarrier),
            "SummonVampire" => Some(Self::SummonVampire),
            "VampireShot" => Some(Self::VampireShot),
            "SummonToad" => Some(Self::SummonToad),
            "PoisonShot" => Some(Self::PoisonShot),
            "CrippleShot" => Some(Self::CrippleShot),
            "SummonSnakes" => Some(Self::SummonSnakes),
            "NapalmShot" => Some(Self::NapalmShot),
            "OneWithNature" => Some(Self::OneWithNature),
            "BindingShot" => Some(Self::BindingShot),
            "MentalState" => Some(Self::MentalState),
            "Blink" => Some(Self::Blink),
            "Portal" => Some(Self::Portal),
            "BattleCry" => Some(Self::BattleCry),
            "FireBounce" => Some(Self::FireBounce),
            "MeteorShower" => Some(Self::MeteorShower),
            "DigOutZombie" => Some(Self::DigOutZombie),
            "Rubble" => Some(Self::Rubble),
            "MapLightning" => Some(Self::MapLightning),
            "MapLava" => Some(Self::MapLava),
            "MapQuake1" => Some(Self::MapQuake1),
            "MapQuake2" => Some(Self::MapQuake2),
            "DigOutArmadillo" => Some(Self::DigOutArmadillo),
            "GeneralMeowMeowThunder" => Some(Self::GeneralMeowMeowThunder),
            "StoneGolemQuake" => Some(Self::StoneGolemQuake),
            "EarthGolemPile" => Some(Self::EarthGolemPile),
            "TreeQueenRoot" => Some(Self::TreeQueenRoot),
            "TreeQueenMassRoots" => Some(Self::TreeQueenMassRoots),
            "TreeQueenGroundRoots" => Some(Self::TreeQueenGroundRoots),
            "TucsonGeneralRock" => Some(Self::TucsonGeneralRock),
            "FlyingStatueIceTornado" => Some(Self::FlyingStatueIceTornado),
            "DarkOmaKingNuke" => Some(Self::DarkOmaKingNuke),
            "HornedSorcererDustTornado" => Some(Self::HornedSorcererDustTornado),
            "HornedCommanderRockFall" => Some(Self::HornedCommanderRockFall),
            "HornedCommanderRockSpike" => Some(Self::HornedCommanderRockSpike),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            x: reader.read_i32()?,
            y: reader.read_i32()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_i32(self.x);
        writer.write_i32(self.y);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientMagic {
    pub name: String,
    pub spell: Spell,
    pub base_cost: u8,
    pub level_cost: u8,
    pub icon: u8,
    pub level1: u8,
    pub level2: u8,
    pub level3: u8,
    pub need1: u16,
    pub need2: u16,
    pub need3: u16,
    pub level: u8,
    pub key: u8,
    pub experience: u16,
    pub delay: i64,
    pub range: u8,
    pub cast_time: i64,
}

impl ClientMagic {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            name: reader.read_string()?,
            spell: Spell::try_from(reader.read_u8()?)?,
            base_cost: reader.read_u8()?,
            level_cost: reader.read_u8()?,
            icon: reader.read_u8()?,
            level1: reader.read_u8()?,
            level2: reader.read_u8()?,
            level3: reader.read_u8()?,
            need1: reader.read_u16()?,
            need2: reader.read_u16()?,
            need3: reader.read_u16()?,
            level: reader.read_u8()?,
            key: reader.read_u8()?,
            experience: reader.read_u16()?,
            delay: reader.read_i64()?,
            range: reader.read_u8()?,
            cast_time: reader.read_i64()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_string(&self.name)?;
        writer.write_u8(self.spell as u8);
        writer.write_u8(self.base_cost);
        writer.write_u8(self.level_cost);
        writer.write_u8(self.icon);
        writer.write_u8(self.level1);
        writer.write_u8(self.level2);
        writer.write_u8(self.level3);
        writer.write_u16(self.need1);
        writer.write_u16(self.need2);
        writer.write_u16(self.need3);
        writer.write_u8(self.level);
        writer.write_u8(self.key);
        writer.write_u16(self.experience);
        writer.write_i64(self.delay);
        writer.write_u8(self.range);
        writer.write_i64(self.cast_time);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientMovementInfo {
    pub destination: i32,
    pub title: String,
    pub location: Point,
    pub icon: i32,
}

impl ClientMovementInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            destination: reader.read_i32()?,
            title: reader.read_string()?,
            location: Point::decode(reader)?,
            icon: reader.read_i32()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_i32(self.destination);
        writer.write_string(&self.title)?;
        self.location.encode(writer);
        writer.write_i32(self.icon);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientNpcInfo {
    pub index: i32,
    pub file_name: String,
    pub name: String,
    pub map_index: i32,
    pub location: Point,
    pub image: u16,
    pub rate: u16,
    pub show_on_big_map: bool,
    pub big_map_icon: i32,
    pub object_id: u32,
    pub icon: i32,
    pub can_teleport_to: bool,
}

impl ClientNpcInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            index: reader.read_i32()?,
            file_name: reader.read_string()?,
            name: reader.read_string()?,
            map_index: reader.read_i32()?,
            location: Point::decode(reader)?,
            image: reader.read_u16()?,
            rate: reader.read_u16()?,
            show_on_big_map: reader.read_bool()?,
            big_map_icon: reader.read_i32()?,
            object_id: reader.read_u32()?,
            icon: reader.read_i32()?,
            can_teleport_to: reader.read_bool()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_i32(self.index);
        writer.write_string(&self.file_name)?;
        writer.write_string(&self.name)?;
        writer.write_i32(self.map_index);
        self.location.encode(writer);
        writer.write_u16(self.image);
        writer.write_u16(self.rate);
        writer.write_bool(self.show_on_big_map);
        writer.write_i32(self.big_map_icon);
        writer.write_u32(self.object_id);
        writer.write_i32(self.icon);
        writer.write_bool(self.can_teleport_to);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientMapInfo {
    pub title: String,
    pub width: i32,
    pub height: i32,
    pub big_map: i32,
    pub movements: Vec<ClientMovementInfo>,
    pub npcs: Vec<ClientNpcInfo>,
}

impl ClientMapInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        let title = reader.read_string()?;
        let width = reader.read_i32()?;
        let height = reader.read_i32()?;
        let big_map = reader.read_i32()?;

        let movement_count = read_non_negative_count(reader, "client_map_movements")?;
        let mut movements = Vec::with_capacity(movement_count);
        for _ in 0..movement_count {
            movements.push(ClientMovementInfo::decode(reader)?);
        }

        let npc_count = read_non_negative_count(reader, "client_map_npcs")?;
        let mut npcs = Vec::with_capacity(npc_count);
        for _ in 0..npc_count {
            npcs.push(ClientNpcInfo::decode(reader)?);
        }

        Ok(Self {
            title,
            width,
            height,
            big_map,
            movements,
            npcs,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_string(&self.title)?;
        writer.write_i32(self.width);
        writer.write_i32(self.height);
        writer.write_i32(self.big_map);
        writer.write_i32(self.movements.len() as i32);
        for movement in &self.movements {
            movement.encode(writer)?;
        }
        writer.write_i32(self.npcs.len() as i32);
        for npc in &self.npcs {
            npc.encode(writer)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldMapIcon {
    pub image_index: i32,
    pub title: String,
    pub map_index: i32,
}

impl WorldMapIcon {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            image_index: reader.read_i32()?,
            title: reader.read_string()?,
            map_index: reader.read_i32()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_i32(self.image_index);
        writer.write_string(&self.title)?;
        writer.write_i32(self.map_index);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldMapSetup {
    pub enabled: bool,
    pub icons: Vec<WorldMapIcon>,
}

impl WorldMapSetup {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        let enabled = reader.read_bool()?;
        let count = read_non_negative_count(reader, "world_map_icons")?;
        let mut icons = Vec::with_capacity(count);
        for _ in 0..count {
            icons.push(WorldMapIcon::decode(reader)?);
        }
        Ok(Self { enabled, icons })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_bool(self.enabled);
        writer.write_i32(self.icons.len() as i32);
        for icon in &self.icons {
            icon.encode(writer)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectInfo {
    pub index: i32,
    pub name: String,
    pub level: u16,
    pub class: MirClass,
    pub gender: MirGender,
    pub last_access_binary_datetime: i64,
}

impl SelectInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            index: reader.read_i32()?,
            name: reader.read_string()?,
            level: reader.read_u16()?,
            class: MirClass::try_from(reader.read_u8()?)?,
            gender: MirGender::try_from(reader.read_u8()?)?,
            last_access_binary_datetime: reader.read_i64()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_i32(self.index);
        writer.write_string(&self.name)?;
        writer.write_u16(self.level);
        writer.write_u8(self.class as u8);
        writer.write_u8(self.gender as u8);
        writer.write_i64(self.last_access_binary_datetime);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInformation {
    pub object_id: u32,
    pub real_id: u32,
    pub name: String,
    pub guild_name: String,
    pub guild_rank: String,
    pub name_colour_argb: i32,
    pub class: MirClass,
    pub gender: MirGender,
    pub level: u16,
    pub location: Point,
    pub direction: MirDirection,
    pub hair: u8,
    pub hp: i32,
    pub mp: i32,
    pub experience: i64,
    pub max_experience: i64,
    pub level_effects: u16,
    pub has_hero: bool,
    pub hero_behaviour: u8,
    pub inventory_section_present: bool,
    pub inventory: Option<Vec<Option<UserItem>>>,
    pub equipment_section_present: bool,
    pub equipment: Option<Vec<Option<UserItem>>>,
    pub quest_inventory_section_present: bool,
    pub quest_inventory: Option<Vec<Option<UserItem>>>,
    pub gold: u32,
    pub credit: u32,
    pub has_expanded_storage: bool,
    pub has_storage_password: bool,
    pub require_storage_password: bool,
    pub storage_password_last_set_binary_datetime: i64,
    pub expanded_storage_expiry_time_binary_datetime: i64,
    pub magic_count: i32,
    pub intelligent_creature_count: i32,
    pub summoned_creature_type: u8,
    pub creature_summoned: bool,
    pub allow_observe: bool,
    pub observer: bool,
}

impl UserInformation {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        let object_id = reader.read_u32()?;
        let real_id = reader.read_u32()?;
        let name = reader.read_string()?;
        let guild_name = reader.read_string()?;
        let guild_rank = reader.read_string()?;
        let name_colour_argb = reader.read_i32()?;
        let class = MirClass::try_from(reader.read_u8()?)?;
        let gender = MirGender::try_from(reader.read_u8()?)?;
        let level = reader.read_u16()?;
        let location = Point::decode(reader)?;
        let direction = MirDirection::try_from(reader.read_u8()?)?;
        let hair = reader.read_u8()?;
        let hp = reader.read_i32()?;
        let mp = reader.read_i32()?;
        let experience = reader.read_i64()?;
        let max_experience = reader.read_i64()?;
        let level_effects = reader.read_u16()?;
        let has_hero = reader.read_bool()?;
        let hero_behaviour = reader.read_u8()?;
        let (inventory_section_present, inventory) =
            decode_optional_item_section(reader, "inventory_slots")?;
        let (equipment_section_present, equipment) =
            decode_optional_item_section(reader, "equipment_slots")?;
        let (quest_inventory_section_present, quest_inventory) =
            decode_optional_item_section(reader, "quest_inventory_slots")?;
        let gold = reader.read_u32()?;
        let credit = reader.read_u32()?;
        let has_expanded_storage = reader.read_bool()?;
        let has_storage_password = reader.read_bool()?;
        let require_storage_password = reader.read_bool()?;
        let storage_password_last_set_binary_datetime = reader.read_i64()?;
        let expanded_storage_expiry_time_binary_datetime = reader.read_i64()?;
        let magic_count = decode_empty_count_section(reader, "magics")?;
        let intelligent_creature_count =
            decode_empty_count_section(reader, "intelligent_creatures")?;
        let summoned_creature_type = reader.read_u8()?;
        let creature_summoned = reader.read_bool()?;
        let allow_observe = reader.read_bool()?;
        let observer = reader.read_bool()?;

        Ok(Self {
            object_id,
            real_id,
            name,
            guild_name,
            guild_rank,
            name_colour_argb,
            class,
            gender,
            level,
            location,
            direction,
            hair,
            hp,
            mp,
            experience,
            max_experience,
            level_effects,
            has_hero,
            hero_behaviour,
            inventory_section_present,
            inventory,
            equipment_section_present,
            equipment,
            quest_inventory_section_present,
            quest_inventory,
            gold,
            credit,
            has_expanded_storage,
            has_storage_password,
            require_storage_password,
            storage_password_last_set_binary_datetime,
            expanded_storage_expiry_time_binary_datetime,
            magic_count,
            intelligent_creature_count,
            summoned_creature_type,
            creature_summoned,
            allow_observe,
            observer,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_u32(self.object_id);
        writer.write_u32(self.real_id);
        writer.write_string(&self.name)?;
        writer.write_string(&self.guild_name)?;
        writer.write_string(&self.guild_rank)?;
        writer.write_i32(self.name_colour_argb);
        writer.write_u8(self.class as u8);
        writer.write_u8(self.gender as u8);
        writer.write_u16(self.level);
        self.location.encode(writer);
        writer.write_u8(self.direction as u8);
        writer.write_u8(self.hair);
        writer.write_i32(self.hp);
        writer.write_i32(self.mp);
        writer.write_i64(self.experience);
        writer.write_i64(self.max_experience);
        writer.write_u16(self.level_effects);
        writer.write_bool(self.has_hero);
        writer.write_u8(self.hero_behaviour);
        encode_optional_item_section(
            writer,
            self.inventory_section_present,
            self.inventory.as_deref(),
        )?;
        encode_optional_item_section(
            writer,
            self.equipment_section_present,
            self.equipment.as_deref(),
        )?;
        encode_optional_item_section(
            writer,
            self.quest_inventory_section_present,
            self.quest_inventory.as_deref(),
        )?;
        writer.write_u32(self.gold);
        writer.write_u32(self.credit);
        writer.write_bool(self.has_expanded_storage);
        writer.write_bool(self.has_storage_password);
        writer.write_bool(self.require_storage_password);
        writer.write_i64(self.storage_password_last_set_binary_datetime);
        writer.write_i64(self.expanded_storage_expiry_time_binary_datetime);
        encode_empty_count_section(writer, "magics", self.magic_count)?;
        encode_empty_count_section(
            writer,
            "intelligent_creatures",
            self.intelligent_creature_count,
        )?;
        writer.write_u8(self.summoned_creature_type);
        writer.write_bool(self.creature_summoned);
        writer.write_bool(self.allow_observe);
        writer.write_bool(self.observer);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserItem {
    pub unique_id: u64,
    pub item_index: i32,
    pub current_dura: u16,
    pub max_dura: u16,
    pub count: u16,
    pub soul_bound_id: i32,
    pub identified: bool,
    pub cursed: bool,
    pub slots: Vec<Option<UserItem>>,
    pub gem_count: u16,
    pub added_stats: Vec<UserItemStat>,
    pub awake_type: u8,
    pub awake_values: Vec<u8>,
    pub refined_value: u8,
    pub refine_added: u8,
    pub refine_success_chance: i32,
    pub wedding_ring: i32,
    pub expire_info: Option<UserItemExpireInfo>,
    pub rental_information: Option<UserItemRentalInformation>,
    pub is_shop_item: bool,
    pub sealed_info: Option<UserItemSealedInfo>,
    pub gm_made: bool,
}

impl UserItem {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        let unique_id = reader.read_u64()?;
        let item_index = reader.read_i32()?;
        let current_dura = reader.read_u16()?;
        let max_dura = reader.read_u16()?;
        let count = reader.read_u16()?;
        let soul_bound_id = reader.read_i32()?;
        let bools = reader.read_u8()?;
        let identified = (bools & 0x01) == 0x01;
        let cursed = (bools & 0x02) == 0x02;

        let slot_count = read_non_negative_count(reader, "user_item_slots")?;
        let mut slots = Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            let is_null = reader.read_bool()?;
            if is_null {
                slots.push(None);
            } else {
                slots.push(Some(UserItem::decode(reader)?));
            }
        }

        let gem_count = reader.read_u16()?;
        let stat_count = read_non_negative_count(reader, "user_item_added_stats")?;
        let mut added_stats = Vec::with_capacity(stat_count);
        for _ in 0..stat_count {
            added_stats.push(UserItemStat {
                stat: reader.read_u8()?,
                value: reader.read_i32()?,
            });
        }

        let awake_type = reader.read_u8()?;
        let awake_count = read_non_negative_count(reader, "user_item_awake_values")?;
        let mut awake_values = Vec::with_capacity(awake_count);
        for _ in 0..awake_count {
            awake_values.push(reader.read_u8()?);
        }

        let refined_value = reader.read_u8()?;
        let refine_added = reader.read_u8()?;
        let refine_success_chance = reader.read_i32()?;
        let wedding_ring = reader.read_i32()?;

        let expire_info = if reader.read_bool()? {
            Some(UserItemExpireInfo::decode(reader)?)
        } else {
            None
        };
        let rental_information = if reader.read_bool()? {
            Some(UserItemRentalInformation::decode(reader)?)
        } else {
            None
        };
        let is_shop_item = reader.read_bool()?;
        let sealed_info = if reader.read_bool()? {
            Some(UserItemSealedInfo::decode(reader)?)
        } else {
            None
        };
        let gm_made = reader.read_bool()?;

        Ok(Self {
            unique_id,
            item_index,
            current_dura,
            max_dura,
            count,
            soul_bound_id,
            identified,
            cursed,
            slots,
            gem_count,
            added_stats,
            awake_type,
            awake_values,
            refined_value,
            refine_added,
            refine_success_chance,
            wedding_ring,
            expire_info,
            rental_information,
            is_shop_item,
            sealed_info,
            gm_made,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_u64(self.unique_id);
        writer.write_i32(self.item_index);
        writer.write_u16(self.current_dura);
        writer.write_u16(self.max_dura);
        writer.write_u16(self.count);
        writer.write_i32(self.soul_bound_id);

        let mut bools = 0_u8;
        if self.identified {
            bools |= 0x01;
        }
        if self.cursed {
            bools |= 0x02;
        }
        writer.write_u8(bools);

        writer.write_i32(self.slots.len() as i32);
        for slot in &self.slots {
            writer.write_bool(slot.is_none());
            if let Some(item) = slot {
                item.encode(writer)?;
            }
        }

        writer.write_u16(self.gem_count);
        let mut added_stats = self.added_stats.clone();
        added_stats.sort_by_key(|stat| stat.stat);
        writer.write_i32(added_stats.len() as i32);
        for stat in added_stats {
            writer.write_u8(stat.stat);
            writer.write_i32(stat.value);
        }

        writer.write_u8(self.awake_type);
        writer.write_i32(self.awake_values.len() as i32);
        for value in &self.awake_values {
            writer.write_u8(*value);
        }

        writer.write_u8(self.refined_value);
        writer.write_u8(self.refine_added);
        writer.write_i32(self.refine_success_chance);
        writer.write_i32(self.wedding_ring);

        writer.write_bool(self.expire_info.is_some());
        if let Some(expire_info) = &self.expire_info {
            expire_info.encode(writer);
        }

        writer.write_bool(self.rental_information.is_some());
        if let Some(rental_information) = &self.rental_information {
            rental_information.encode(writer)?;
        }

        writer.write_bool(self.is_shop_item);
        writer.write_bool(self.sealed_info.is_some());
        if let Some(sealed_info) = &self.sealed_info {
            sealed_info.encode(writer);
        }
        writer.write_bool(self.gm_made);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientFriend {
    pub index: i32,
    pub name: String,
    pub memo: String,
    pub blocked: bool,
    pub online: bool,
}

impl ClientFriend {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            index: reader.read_i32()?,
            name: reader.read_string()?,
            memo: reader.read_string()?,
            blocked: reader.read_bool()?,
            online: reader.read_bool()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_i32(self.index);
        writer.write_string(&self.name)?;
        writer.write_string(&self.memo)?;
        writer.write_bool(self.blocked);
        writer.write_bool(self.online);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerInspectInfo {
    pub name: String,
    pub guild_name: String,
    pub guild_rank: String,
    pub equipment: Vec<Option<UserItem>>,
    pub class: MirClass,
    pub gender: MirGender,
    pub hair: u8,
    pub level: u16,
    pub lover_name: String,
    pub allow_observe: bool,
    pub is_hero: bool,
}

impl PlayerInspectInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        let name = reader.read_string()?;
        let guild_name = reader.read_string()?;
        let guild_rank = reader.read_string()?;
        let count = read_non_negative_count(reader, "player_inspect_equipment")?;
        let mut equipment = Vec::with_capacity(count);
        for _ in 0..count {
            if reader.read_bool()? {
                equipment.push(Some(UserItem::decode(reader)?));
            } else {
                equipment.push(None);
            }
        }

        Ok(Self {
            name,
            guild_name,
            guild_rank,
            equipment,
            class: MirClass::try_from(reader.read_u8()?)?,
            gender: MirGender::try_from(reader.read_u8()?)?,
            hair: reader.read_u8()?,
            level: reader.read_u16()?,
            lover_name: reader.read_string()?,
            allow_observe: reader.read_bool()?,
            is_hero: reader.read_bool()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_string(&self.name)?;
        writer.write_string(&self.guild_name)?;
        writer.write_string(&self.guild_rank)?;
        writer.write_i32(self.equipment.len() as i32);
        for item in &self.equipment {
            writer.write_bool(item.is_some());
            if let Some(item) = item {
                item.encode(writer)?;
            }
        }
        writer.write_u8(self.class as u8);
        writer.write_u8(self.gender as u8);
        writer.write_u8(self.hair);
        writer.write_u16(self.level);
        writer.write_string(&self.lover_name)?;
        writer.write_bool(self.allow_observe);
        writer.write_bool(self.is_hero);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientMail {
    pub mail_id: u64,
    pub sender_name: String,
    pub message: String,
    pub opened: bool,
    pub locked: bool,
    pub can_reply: bool,
    pub collected: bool,
    pub date_sent_binary_datetime: i64,
    pub gold: u32,
    pub items: Vec<UserItem>,
}

impl ClientMail {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        let mail_id = reader.read_u64()?;
        let sender_name = reader.read_string()?;
        let message = reader.read_string()?;
        let opened = reader.read_bool()?;
        let locked = reader.read_bool()?;
        let can_reply = reader.read_bool()?;
        let collected = reader.read_bool()?;
        let date_sent_binary_datetime = reader.read_i64()?;
        let gold = reader.read_u32()?;
        let item_count = read_non_negative_count(reader, "client_mail_items")?;
        let mut items = Vec::with_capacity(item_count);
        for _ in 0..item_count {
            items.push(UserItem::decode(reader)?);
        }

        Ok(Self {
            mail_id,
            sender_name,
            message,
            opened,
            locked,
            can_reply,
            collected,
            date_sent_binary_datetime,
            gold,
            items,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_u64(self.mail_id);
        writer.write_string(&self.sender_name)?;
        writer.write_string(&self.message)?;
        writer.write_bool(self.opened);
        writer.write_bool(self.locked);
        writer.write_bool(self.can_reply);
        writer.write_bool(self.collected);
        writer.write_i64(self.date_sent_binary_datetime);
        writer.write_u32(self.gold);
        writer.write_i32(self.items.len() as i32);
        for item in &self.items {
            item.encode(writer)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligentCreatureRules {
    pub minimal_fullness: i32,
    pub mouse_pickup_enabled: bool,
    pub mouse_pickup_range: i32,
    pub auto_pickup_enabled: bool,
    pub auto_pickup_range: i32,
    pub semi_auto_pickup_enabled: bool,
    pub semi_auto_pickup_range: i32,
    pub can_produce_blackstone: bool,
}

impl IntelligentCreatureRules {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            minimal_fullness: reader.read_i32()?,
            mouse_pickup_enabled: reader.read_bool()?,
            mouse_pickup_range: reader.read_i32()?,
            auto_pickup_enabled: reader.read_bool()?,
            auto_pickup_range: reader.read_i32()?,
            semi_auto_pickup_enabled: reader.read_bool()?,
            semi_auto_pickup_range: reader.read_i32()?,
            can_produce_blackstone: reader.read_bool()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_i32(self.minimal_fullness);
        writer.write_bool(self.mouse_pickup_enabled);
        writer.write_i32(self.mouse_pickup_range);
        writer.write_bool(self.auto_pickup_enabled);
        writer.write_i32(self.auto_pickup_range);
        writer.write_bool(self.semi_auto_pickup_enabled);
        writer.write_i32(self.semi_auto_pickup_range);
        writer.write_bool(self.can_produce_blackstone);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligentCreatureItemFilter {
    pub pet_pickup_all: bool,
    pub pet_pickup_gold: bool,
    pub pet_pickup_weapons: bool,
    pub pet_pickup_armours: bool,
    pub pet_pickup_helmets: bool,
    pub pet_pickup_boots: bool,
    pub pet_pickup_belts: bool,
    pub pet_pickup_accessories: bool,
    pub pet_pickup_others: bool,
}

impl IntelligentCreatureItemFilter {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            pet_pickup_all: reader.read_bool()?,
            pet_pickup_gold: reader.read_bool()?,
            pet_pickup_weapons: reader.read_bool()?,
            pet_pickup_armours: reader.read_bool()?,
            pet_pickup_helmets: reader.read_bool()?,
            pet_pickup_boots: reader.read_bool()?,
            pet_pickup_belts: reader.read_bool()?,
            pet_pickup_accessories: reader.read_bool()?,
            pet_pickup_others: reader.read_bool()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_bool(self.pet_pickup_all);
        writer.write_bool(self.pet_pickup_gold);
        writer.write_bool(self.pet_pickup_weapons);
        writer.write_bool(self.pet_pickup_armours);
        writer.write_bool(self.pet_pickup_helmets);
        writer.write_bool(self.pet_pickup_boots);
        writer.write_bool(self.pet_pickup_belts);
        writer.write_bool(self.pet_pickup_accessories);
        writer.write_bool(self.pet_pickup_others);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientIntelligentCreature {
    pub pet_type: u8,
    pub icon: i32,
    pub custom_name: String,
    pub fullness: i32,
    pub slot_index: i32,
    pub expire_binary_datetime: i64,
    pub blackstone_time: i64,
    pub pet_mode: u8,
    pub creature_rules: IntelligentCreatureRules,
    pub filter: IntelligentCreatureItemFilter,
    pub pickup_grade: u8,
    pub maintain_food_time: i64,
}

impl ClientIntelligentCreature {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            pet_type: reader.read_u8()?,
            icon: reader.read_i32()?,
            custom_name: reader.read_string()?,
            fullness: reader.read_i32()?,
            slot_index: reader.read_i32()?,
            expire_binary_datetime: reader.read_i64()?,
            blackstone_time: reader.read_i64()?,
            pet_mode: reader.read_u8()?,
            creature_rules: IntelligentCreatureRules::decode(reader)?,
            filter: IntelligentCreatureItemFilter::decode(reader)?,
            pickup_grade: reader.read_u8()?,
            maintain_food_time: reader.read_i64()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_u8(self.pet_type);
        writer.write_i32(self.icon);
        writer.write_string(&self.custom_name)?;
        writer.write_i32(self.fullness);
        writer.write_i32(self.slot_index);
        writer.write_i64(self.expire_binary_datetime);
        writer.write_i64(self.blackstone_time);
        writer.write_u8(self.pet_mode);
        self.creature_rules.encode(writer);
        self.filter.encode(writer);
        writer.write_u8(self.pickup_grade);
        writer.write_i64(self.maintain_food_time);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientHeroInformation {
    pub index: i32,
    pub name: String,
    pub level: u16,
    pub class: MirClass,
    pub gender: MirGender,
}

impl ClientHeroInformation {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            index: reader.read_i32()?,
            name: reader.read_string()?,
            level: reader.read_u16()?,
            class: MirClass::try_from(reader.read_u8()?)?,
            gender: MirGender::try_from(reader.read_u8()?)?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_i32(self.index);
        writer.write_string(&self.name)?;
        writer.write_u16(self.level);
        writer.write_u8(self.class as u8);
        writer.write_u8(self.gender as u8);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserItemStat {
    pub stat: u8,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientBuff {
    pub buff_type: u8,
    pub visible: bool,
    pub object_id: u32,
    pub expire_time: i64,
    pub infinite: bool,
    pub paused: bool,
    pub stats: Vec<UserItemStat>,
    pub values: Vec<i32>,
}

impl ClientBuff {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        let buff_type = reader.read_u8()?;
        let visible = reader.read_bool()?;
        let object_id = reader.read_u32()?;
        let expire_time = reader.read_i64()?;
        let infinite = reader.read_bool()?;
        let paused = reader.read_bool()?;
        let stats = decode_stat_values(reader, "client_buff_stats")?;
        let value_count = reader.read_i32()?;
        if value_count < 0 {
            return Err(PacketCodecError::NegativeLength {
                field: "client_buff_values",
                value: value_count,
            });
        }
        let mut values = Vec::with_capacity(value_count as usize);
        for _ in 0..value_count {
            values.push(reader.read_i32()?);
        }

        Ok(Self {
            buff_type,
            visible,
            object_id,
            expire_time,
            infinite,
            paused,
            stats,
            values,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_u8(self.buff_type);
        writer.write_bool(self.visible);
        writer.write_u32(self.object_id);
        writer.write_i64(self.expire_time);
        writer.write_bool(self.infinite);
        writer.write_bool(self.paused);
        encode_stat_values(writer, &self.stats);
        writer.write_i32(self.values.len() as i32);
        for value in &self.values {
            writer.write_i32(*value);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaseStat {
    pub stat: u8,
    pub formula_type: u8,
    pub base: i32,
    pub gain: f32,
    pub gain_rate: f32,
    pub max: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaseStats {
    pub job: MirClass,
    pub stats: Vec<BaseStat>,
    pub caps: Vec<UserItemStat>,
}

impl BaseStats {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        let job = MirClass::try_from(reader.read_u8()?)?;
        let stat_count = read_non_negative_count(reader, "base_stats")?;
        let mut stats = Vec::with_capacity(stat_count);
        for _ in 0..stat_count {
            stats.push(BaseStat {
                stat: reader.read_u8()?,
                formula_type: reader.read_u8()?,
                base: reader.read_i32()?,
                gain: reader.read_f32()?,
                gain_rate: reader.read_f32()?,
                max: reader.read_i32()?,
            });
        }
        let caps = decode_stat_values(reader, "base_stats_caps")?;
        Ok(Self { job, stats, caps })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_u8(self.job as u8);
        writer.write_i32(self.stats.len() as i32);
        for stat in &self.stats {
            writer.write_u8(stat.stat);
            writer.write_u8(stat.formula_type);
            writer.write_i32(stat.base);
            writer.write_f32(stat.gain);
            writer.write_f32(stat.gain_rate);
            writer.write_i32(stat.max);
        }
        encode_stat_values(writer, &self.caps);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroUserInformation {
    pub object_id: u32,
    pub name: String,
    pub class: MirClass,
    pub gender: MirGender,
    pub level: u16,
    pub hair: u8,
    pub hp: i32,
    pub mp: i32,
    pub experience: i64,
    pub max_experience: i64,
    pub inventory: Option<Vec<Option<UserItem>>>,
    pub equipment: Option<Vec<Option<UserItem>>>,
    pub magics: Vec<ClientMagic>,
    pub auto_pot: bool,
    pub auto_hp_percent: u8,
    pub auto_mp_percent: u8,
    pub hp_item_index: i32,
    pub mp_item_index: i32,
}

impl HeroUserInformation {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        let object_id = reader.read_u32()?;
        let name = reader.read_string()?;
        let class = MirClass::try_from(reader.read_u8()?)?;
        let gender = MirGender::try_from(reader.read_u8()?)?;
        let level = reader.read_u16()?;
        let hair = reader.read_u8()?;
        let hp = reader.read_i32()?;
        let mp = reader.read_i32()?;
        let experience = reader.read_i64()?;
        let max_experience = reader.read_i64()?;
        let (_, inventory) = decode_optional_item_section(reader, "hero_inventory")?;
        let (_, equipment) = decode_optional_item_section(reader, "hero_equipment")?;
        let magic_count = read_non_negative_count(reader, "hero_magics")?;
        let mut magics = Vec::with_capacity(magic_count);
        for _ in 0..magic_count {
            magics.push(ClientMagic::decode(reader)?);
        }
        Ok(Self {
            object_id,
            name,
            class,
            gender,
            level,
            hair,
            hp,
            mp,
            experience,
            max_experience,
            inventory,
            equipment,
            magics,
            auto_pot: reader.read_bool()?,
            auto_hp_percent: reader.read_u8()?,
            auto_mp_percent: reader.read_u8()?,
            hp_item_index: reader.read_i32()?,
            mp_item_index: reader.read_i32()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_u32(self.object_id);
        writer.write_string(&self.name)?;
        writer.write_u8(self.class as u8);
        writer.write_u8(self.gender as u8);
        writer.write_u16(self.level);
        writer.write_u8(self.hair);
        writer.write_i32(self.hp);
        writer.write_i32(self.mp);
        writer.write_i64(self.experience);
        writer.write_i64(self.max_experience);
        encode_optional_item_section(writer, self.inventory.is_some(), self.inventory.as_deref())?;
        encode_optional_item_section(writer, self.equipment.is_some(), self.equipment.as_deref())?;
        writer.write_i32(self.magics.len() as i32);
        for magic in &self.magics {
            magic.encode(writer)?;
        }
        writer.write_bool(self.auto_pot);
        writer.write_u8(self.auto_hp_percent);
        writer.write_u8(self.auto_mp_percent);
        writer.write_i32(self.hp_item_index);
        writer.write_i32(self.mp_item_index);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientAuction {
    pub auction_id: u64,
    pub item: UserItem,
    pub seller: String,
    pub price: u32,
    pub consignment_date_binary_datetime: i64,
    pub item_type: u8,
}

impl ClientAuction {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            auction_id: reader.read_u64()?,
            item: UserItem::decode(reader)?,
            seller: reader.read_string()?,
            price: reader.read_u32()?,
            consignment_date_binary_datetime: reader.read_i64()?,
            item_type: reader.read_u8()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_u64(self.auction_id);
        self.item.encode(writer)?;
        writer.write_string(&self.seller)?;
        writer.write_u32(self.price);
        writer.write_i64(self.consignment_date_binary_datetime);
        writer.write_u8(self.item_type);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameShopItem {
    pub item_index: i32,
    pub g_index: i32,
    pub info: ItemInfo,
    pub gold_price: u32,
    pub credit_price: u32,
    pub count: u16,
    pub class: String,
    pub category: String,
    pub stock: i32,
    pub i_stock: bool,
    pub deal: bool,
    pub top_item: bool,
    pub date_binary_datetime: i64,
    pub can_buy_credit: bool,
    pub can_buy_gold: bool,
}

impl GameShopItem {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            item_index: reader.read_i32()?,
            g_index: reader.read_i32()?,
            info: ItemInfo::decode(reader)?,
            gold_price: reader.read_u32()?,
            credit_price: reader.read_u32()?,
            count: reader.read_u16()?,
            class: reader.read_string()?,
            category: reader.read_string()?,
            stock: reader.read_i32()?,
            i_stock: reader.read_bool()?,
            deal: reader.read_bool()?,
            top_item: reader.read_bool()?,
            date_binary_datetime: reader.read_i64()?,
            can_buy_credit: reader.read_bool()?,
            can_buy_gold: reader.read_bool()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_i32(self.item_index);
        writer.write_i32(self.g_index);
        self.info.encode(writer)?;
        writer.write_u32(self.gold_price);
        writer.write_u32(self.credit_price);
        writer.write_u16(self.count);
        writer.write_string(&self.class)?;
        writer.write_string(&self.category)?;
        writer.write_i32(self.stock);
        writer.write_bool(self.i_stock);
        writer.write_bool(self.deal);
        writer.write_bool(self.top_item);
        writer.write_i64(self.date_binary_datetime);
        writer.write_bool(self.can_buy_credit);
        writer.write_bool(self.can_buy_gold);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuildBuff {
    pub id: i32,
    pub active: bool,
    pub active_time_remaining: i32,
}

impl GuildBuff {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            id: reader.read_i32()?,
            active: reader.read_bool()?,
            active_time_remaining: reader.read_i32()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_i32(self.id);
        writer.write_bool(self.active);
        writer.write_i32(self.active_time_remaining);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuildBuffInfo {
    pub id: i32,
    pub icon: i32,
    pub name: String,
    pub level_requirement: u8,
    pub points_requirement: u8,
    pub time_limit: i32,
    pub activation_cost: i32,
    pub stats: Vec<UserItemStat>,
}

impl GuildBuffInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            id: reader.read_i32()?,
            icon: reader.read_i32()?,
            name: reader.read_string()?,
            level_requirement: reader.read_u8()?,
            points_requirement: reader.read_u8()?,
            time_limit: reader.read_i32()?,
            activation_cost: reader.read_i32()?,
            stats: decode_stat_values(reader, "guild_buff_info_stats")?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_i32(self.id);
        writer.write_i32(self.icon);
        writer.write_string(&self.name)?;
        writer.write_u8(self.level_requirement);
        writer.write_u8(self.points_requirement);
        writer.write_i32(self.time_limit);
        writer.write_i32(self.activation_cost);
        encode_stat_values(writer, &self.stats);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildMember {
    pub name: String,
    pub id: i32,
    pub last_login_binary_datetime: i64,
    pub has_voted: bool,
    pub online: bool,
}

impl GuildMember {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            name: reader.read_string()?,
            id: reader.read_i32()?,
            last_login_binary_datetime: reader.read_i64()?,
            has_voted: reader.read_bool()?,
            online: reader.read_bool()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_string(&self.name)?;
        writer.write_i32(self.id);
        writer.write_i64(self.last_login_binary_datetime);
        writer.write_bool(self.has_voted);
        writer.write_bool(self.online);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildRank {
    pub name: String,
    pub options: u8,
    pub index: i32,
    pub members: Vec<GuildMember>,
}

impl GuildRank {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        let name = reader.read_string()?;
        let options = reader.read_u8()?;
        let index = reader.read_i32()?;
        let count = read_non_negative_count(reader, "guild_rank_members")?;
        let mut members = Vec::with_capacity(count);
        for _ in 0..count {
            members.push(GuildMember::decode(reader)?);
        }

        Ok(Self {
            name,
            options,
            index,
            members,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_string(&self.name)?;
        writer.write_u8(self.options);
        writer.write_i32(self.index);
        writer.write_i32(self.members.len() as i32);
        for member in &self.members {
            member.encode(writer)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildStorageItem {
    pub item: UserItem,
    pub user_id: i64,
}

impl GuildStorageItem {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            item: UserItem::decode(reader)?,
            user_id: reader.read_i64()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        self.item.encode(writer)?;
        writer.write_i64(self.user_id);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwakeningMaterial {
    pub item: ItemInfo,
    pub count: u8,
}

impl AwakeningMaterial {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            item: ItemInfo::decode(reader)?,
            count: reader.read_u8()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        self.item.encode(writer)?;
        writer.write_u8(self.count);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankCharacterInfo {
    pub player_id: i64,
    pub name: String,
    pub level: i32,
    pub class: MirClass,
}

impl RankCharacterInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            player_id: reader.read_i64()?,
            name: reader.read_string()?,
            level: reader.read_i32()?,
            class: MirClass::try_from(reader.read_u8()?)?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_i64(self.player_id);
        writer.write_string(&self.name)?;
        writer.write_i32(self.level);
        writer.write_u8(self.class as u8);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub title: String,
    pub message: String,
}

impl Notice {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            title: reader.read_string()?,
            message: reader.read_string()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_string(&self.title)?;
        writer.write_string(&self.message)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientGtMap {
    pub index: i32,
    pub name: String,
    pub owner: String,
    pub leader: String,
    pub leader2: String,
    pub price: i32,
    pub days: i32,
    pub begin: i32,
}

impl ClientGtMap {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            index: reader.read_i32()?,
            name: reader.read_string()?,
            owner: reader.read_string()?,
            leader: reader.read_string()?,
            leader2: reader.read_string()?,
            price: reader.read_i32()?,
            days: reader.read_i32()?,
            begin: reader.read_i32()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_i32(self.index);
        writer.write_string(&self.name)?;
        writer.write_string(&self.owner)?;
        writer.write_string(&self.leader)?;
        writer.write_string(&self.leader2)?;
        writer.write_i32(self.price);
        writer.write_i32(self.days);
        writer.write_i32(self.begin);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestItemReward {
    pub item: ItemInfo,
    pub count: u16,
}

impl QuestItemReward {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            item: ItemInfo::decode(reader)?,
            count: reader.read_u16()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        self.item.encode(writer)?;
        writer.write_u16(self.count);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientQuestInfo {
    pub index: i32,
    pub npc_index: u32,
    pub name: String,
    pub group: String,
    pub description: Vec<String>,
    pub task_description: Vec<String>,
    pub return_description: Vec<String>,
    pub completion_description: Vec<String>,
    pub min_level_needed: i32,
    pub max_level_needed: i32,
    pub quest_needed: i32,
    pub class_needed: u8,
    pub quest_type: u8,
    pub time_limit_in_seconds: i32,
    pub reward_gold: u32,
    pub reward_exp: u32,
    pub reward_credit: u32,
    pub rewards_fixed_item: Vec<QuestItemReward>,
    pub rewards_select_item: Vec<QuestItemReward>,
    pub finish_npc_index: u32,
}

impl ClientQuestInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            index: reader.read_i32()?,
            npc_index: reader.read_u32()?,
            name: reader.read_string()?,
            group: reader.read_string()?,
            description: decode_string_vec(reader, "quest_description")?,
            task_description: decode_string_vec(reader, "quest_task_description")?,
            return_description: decode_string_vec(reader, "quest_return_description")?,
            completion_description: decode_string_vec(reader, "quest_completion_description")?,
            min_level_needed: reader.read_i32()?,
            max_level_needed: reader.read_i32()?,
            quest_needed: reader.read_i32()?,
            class_needed: reader.read_u8()?,
            quest_type: reader.read_u8()?,
            time_limit_in_seconds: reader.read_i32()?,
            reward_gold: reader.read_u32()?,
            reward_exp: reader.read_u32()?,
            reward_credit: reader.read_u32()?,
            rewards_fixed_item: decode_quest_rewards(reader, "quest_fixed_rewards")?,
            rewards_select_item: decode_quest_rewards(reader, "quest_select_rewards")?,
            finish_npc_index: reader.read_u32()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_i32(self.index);
        writer.write_u32(self.npc_index);
        writer.write_string(&self.name)?;
        writer.write_string(&self.group)?;
        encode_string_vec(writer, &self.description)?;
        encode_string_vec(writer, &self.task_description)?;
        encode_string_vec(writer, &self.return_description)?;
        encode_string_vec(writer, &self.completion_description)?;
        writer.write_i32(self.min_level_needed);
        writer.write_i32(self.max_level_needed);
        writer.write_i32(self.quest_needed);
        writer.write_u8(self.class_needed);
        writer.write_u8(self.quest_type);
        writer.write_i32(self.time_limit_in_seconds);
        writer.write_u32(self.reward_gold);
        writer.write_u32(self.reward_exp);
        writer.write_u32(self.reward_credit);
        encode_quest_rewards(writer, &self.rewards_fixed_item)?;
        encode_quest_rewards(writer, &self.rewards_select_item)?;
        writer.write_u32(self.finish_npc_index);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientRecipeInfo {
    pub gold: u32,
    pub chance: u8,
    pub item: UserItem,
    pub tools: Vec<UserItem>,
    pub ingredients: Vec<UserItem>,
}

impl ClientRecipeInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            gold: reader.read_u32()?,
            chance: reader.read_u8()?,
            item: UserItem::decode(reader)?,
            tools: decode_user_item_vec(reader, "recipe_tools")?,
            ingredients: decode_user_item_vec(reader, "recipe_ingredients")?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_u32(self.gold);
        writer.write_u8(self.chance);
        self.item.encode(writer)?;
        encode_user_item_vec(writer, &self.tools)?;
        encode_user_item_vec(writer, &self.ingredients)?;
        Ok(())
    }
}

fn decode_string_vec(reader: &mut PacketReader<'_>, field: &'static str) -> Result<Vec<String>> {
    let count = read_non_negative_count(reader, field)?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(reader.read_string()?);
    }
    Ok(values)
}

fn encode_string_vec(writer: &mut PacketWriter, values: &[String]) -> Result<()> {
    writer.write_i32(values.len() as i32);
    for value in values {
        writer.write_string(value)?;
    }
    Ok(())
}

fn decode_quest_rewards(
    reader: &mut PacketReader<'_>,
    field: &'static str,
) -> Result<Vec<QuestItemReward>> {
    let count = read_non_negative_count(reader, field)?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(QuestItemReward::decode(reader)?);
    }
    Ok(values)
}

fn encode_quest_rewards(writer: &mut PacketWriter, values: &[QuestItemReward]) -> Result<()> {
    writer.write_i32(values.len() as i32);
    for value in values {
        value.encode(writer)?;
    }
    Ok(())
}

fn decode_user_item_vec(
    reader: &mut PacketReader<'_>,
    field: &'static str,
) -> Result<Vec<UserItem>> {
    let count = read_non_negative_count(reader, field)?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(UserItem::decode(reader)?);
    }
    Ok(values)
}

fn encode_user_item_vec(writer: &mut PacketWriter, values: &[UserItem]) -> Result<()> {
    writer.write_i32(values.len() as i32);
    for value in values {
        value.encode(writer)?;
    }
    Ok(())
}

fn decode_stat_values(
    reader: &mut PacketReader<'_>,
    field: &'static str,
) -> Result<Vec<UserItemStat>> {
    let count = reader.read_i32()?;
    if count < 0 {
        return Err(PacketCodecError::NegativeLength {
            field,
            value: count,
        });
    }
    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        values.push(UserItemStat {
            stat: reader.read_u8()?,
            value: reader.read_i32()?,
        });
    }
    Ok(values)
}

fn encode_stat_values(writer: &mut PacketWriter, stats: &[UserItemStat]) {
    writer.write_i32(stats.len() as i32);
    for stat in stats {
        writer.write_u8(stat.stat);
        writer.write_i32(stat.value);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserItemExpireInfo {
    pub expiry_binary_datetime: i64,
}

impl UserItemExpireInfo {
    fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            expiry_binary_datetime: reader.read_i64()?,
        })
    }

    fn encode(&self, writer: &mut PacketWriter) {
        writer.write_i64(self.expiry_binary_datetime);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserItemRentalInformation {
    pub owner_name: String,
    pub binding_flags: i16,
    pub expiry_binary_datetime: i64,
    pub rental_locked: bool,
}

impl UserItemRentalInformation {
    fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            owner_name: reader.read_string()?,
            binding_flags: reader.read_i16()?,
            expiry_binary_datetime: reader.read_i64()?,
            rental_locked: reader.read_bool()?,
        })
    }

    fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_string(&self.owner_name)?;
        writer.write_i16(self.binding_flags);
        writer.write_i64(self.expiry_binary_datetime);
        writer.write_bool(self.rental_locked);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemRentalInformation {
    pub item_id: u64,
    pub item_name: String,
    pub renting_player_name: String,
    pub item_return_date_binary_datetime: i64,
}

impl ItemRentalInformation {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            item_id: reader.read_u64()?,
            item_name: reader.read_string()?,
            renting_player_name: reader.read_string()?,
            item_return_date_binary_datetime: reader.read_i64()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_u64(self.item_id);
        writer.write_string(&self.item_name)?;
        writer.write_string(&self.renting_player_name)?;
        writer.write_i64(self.item_return_date_binary_datetime);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserItemSealedInfo {
    pub expiry_binary_datetime: i64,
    pub next_seal_binary_datetime: i64,
}

impl UserItemSealedInfo {
    fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            expiry_binary_datetime: reader.read_i64()?,
            next_seal_binary_datetime: reader.read_i64()?,
        })
    }

    fn encode(&self, writer: &mut PacketWriter) {
        writer.write_i64(self.expiry_binary_datetime);
        writer.write_i64(self.next_seal_binary_datetime);
    }
}

fn read_non_negative_count(reader: &mut PacketReader<'_>, field: &'static str) -> Result<usize> {
    let count = reader.read_i32()?;
    if count < 0 {
        Err(PacketCodecError::NegativeLength {
            field,
            value: count,
        })
    } else {
        Ok(count as usize)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemInfo {
    pub index: i32,
    pub name: String,
    pub item_type: u8,
    pub grade: u8,
    pub required_type: u8,
    pub required_class: u8,
    pub required_gender: u8,
    pub item_set: u8,
    pub shape: i16,
    pub weight: u8,
    pub light: u8,
    pub required_amount: u8,
    pub image: u16,
    pub durability: u16,
    pub stack_size: u16,
    pub price: u32,
    pub start_item: bool,
    pub effect: u8,
    pub need_identify: bool,
    pub show_group_pickup: bool,
    pub class_based: bool,
    pub level_based: bool,
    pub can_mine: bool,
    pub global_drop_notify: bool,
    pub bind: i16,
    pub unique: i16,
    pub random_stats_id: u8,
    pub can_fast_run: bool,
    pub can_awakening: bool,
    pub slots: u8,
    pub stats: Vec<UserItemStat>,
    pub tooltip: Option<String>,
}

impl ItemInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        let index = reader.read_i32()?;
        let name = reader.read_string()?;
        let item_type = reader.read_u8()?;
        let grade = reader.read_u8()?;
        let required_type = reader.read_u8()?;
        let required_class = reader.read_u8()?;
        let required_gender = reader.read_u8()?;
        let item_set = reader.read_u8()?;
        let shape = reader.read_i16()?;
        let weight = reader.read_u8()?;
        let light = reader.read_u8()?;
        let required_amount = reader.read_u8()?;
        let image = reader.read_u16()?;
        let durability = reader.read_u16()?;
        let stack_size = reader.read_u16()?;
        let price = reader.read_u32()?;
        let start_item = reader.read_bool()?;
        let effect = reader.read_u8()?;
        let bools = reader.read_u8()?;
        let need_identify = (bools & 0x01) == 0x01;
        let show_group_pickup = (bools & 0x02) == 0x02;
        let class_based = (bools & 0x04) == 0x04;
        let level_based = (bools & 0x08) == 0x08;
        let can_mine = (bools & 0x10) == 0x10;
        let global_drop_notify = (bools & 0x20) == 0x20;
        let bind = reader.read_i16()?;
        let unique = reader.read_i16()?;
        let random_stats_id = reader.read_u8()?;
        let can_fast_run = reader.read_bool()?;
        let can_awakening = reader.read_bool()?;
        let slots = reader.read_u8()?;
        let stat_count = read_non_negative_count(reader, "item_info_stats")?;
        let mut stats = Vec::with_capacity(stat_count);
        for _ in 0..stat_count {
            stats.push(UserItemStat {
                stat: reader.read_u8()?,
                value: reader.read_i32()?,
            });
        }
        let tooltip = if reader.read_bool()? {
            Some(reader.read_string()?)
        } else {
            None
        };

        Ok(Self {
            index,
            name,
            item_type,
            grade,
            required_type,
            required_class,
            required_gender,
            item_set,
            shape,
            weight,
            light,
            required_amount,
            image,
            durability,
            stack_size,
            price,
            start_item,
            effect,
            need_identify,
            show_group_pickup,
            class_based,
            level_based,
            can_mine,
            global_drop_notify,
            bind,
            unique,
            random_stats_id,
            can_fast_run,
            can_awakening,
            slots,
            stats,
            tooltip,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_i32(self.index);
        writer.write_string(&self.name)?;
        writer.write_u8(self.item_type);
        writer.write_u8(self.grade);
        writer.write_u8(self.required_type);
        writer.write_u8(self.required_class);
        writer.write_u8(self.required_gender);
        writer.write_u8(self.item_set);
        writer.write_i16(self.shape);
        writer.write_u8(self.weight);
        writer.write_u8(self.light);
        writer.write_u8(self.required_amount);
        writer.write_u16(self.image);
        writer.write_u16(self.durability);
        writer.write_u16(self.stack_size);
        writer.write_u32(self.price);
        writer.write_bool(self.start_item);
        writer.write_u8(self.effect);

        let mut bools = 0_u8;
        if self.need_identify {
            bools |= 0x01;
        }
        if self.show_group_pickup {
            bools |= 0x02;
        }
        if self.class_based {
            bools |= 0x04;
        }
        if self.level_based {
            bools |= 0x08;
        }
        if self.can_mine {
            bools |= 0x10;
        }
        if self.global_drop_notify {
            bools |= 0x20;
        }
        writer.write_u8(bools);
        writer.write_i16(self.bind);
        writer.write_i16(self.unique);
        writer.write_u8(self.random_stats_id);
        writer.write_bool(self.can_fast_run);
        writer.write_bool(self.can_awakening);
        writer.write_u8(self.slots);

        let mut stats = self.stats.clone();
        stats.sort_by_key(|stat| stat.stat);
        writer.write_i32(stats.len() as i32);
        for stat in stats {
            writer.write_u8(stat.stat);
            writer.write_i32(stat.value);
        }

        writer.write_bool(self.tooltip.is_some());
        if let Some(tooltip) = &self.tooltip {
            writer.write_string(tooltip)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectPlayerInfo {
    pub object_id: u32,
    pub name: String,
    pub guild_name: String,
    pub guild_rank_name: String,
    pub name_colour_argb: i32,
    pub class: MirClass,
    pub gender: MirGender,
    pub level: u16,
    pub location: Point,
    pub direction: MirDirection,
    pub hair: u8,
    pub light: u8,
    pub weapon: i16,
    pub weapon_effect: i16,
    pub armour: i16,
    pub poison: u16,
    pub dead: bool,
    pub hidden: bool,
    pub effect: u8,
    pub wing_effect: u8,
    pub extra: bool,
    pub mount_type: i16,
    pub riding_mount: bool,
    pub fishing: bool,
    pub transform_type: i16,
    pub element_orb_effect: u32,
    pub element_orb_level: u32,
    pub element_orb_max: u32,
    pub buffs: Vec<u8>,
    pub level_effects: u16,
}

impl ObjectPlayerInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            name: reader.read_string()?,
            guild_name: reader.read_string()?,
            guild_rank_name: reader.read_string()?,
            name_colour_argb: reader.read_i32()?,
            class: MirClass::try_from(reader.read_u8()?)?,
            gender: MirGender::try_from(reader.read_u8()?)?,
            level: reader.read_u16()?,
            location: Point::decode(reader)?,
            direction: MirDirection::try_from(reader.read_u8()?)?,
            hair: reader.read_u8()?,
            light: reader.read_u8()?,
            weapon: reader.read_i16()?,
            weapon_effect: reader.read_i16()?,
            armour: reader.read_i16()?,
            poison: reader.read_u16()?,
            dead: reader.read_bool()?,
            hidden: reader.read_bool()?,
            effect: reader.read_u8()?,
            wing_effect: reader.read_u8()?,
            extra: reader.read_bool()?,
            mount_type: reader.read_i16()?,
            riding_mount: reader.read_bool()?,
            fishing: reader.read_bool()?,
            transform_type: reader.read_i16()?,
            element_orb_effect: reader.read_u32()?,
            element_orb_level: reader.read_u32()?,
            element_orb_max: reader.read_u32()?,
            buffs: decode_u8_vec(reader, "object_player_buffs")?,
            level_effects: reader.read_u16()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_u32(self.object_id);
        writer.write_string(&self.name)?;
        writer.write_string(&self.guild_name)?;
        writer.write_string(&self.guild_rank_name)?;
        writer.write_i32(self.name_colour_argb);
        writer.write_u8(self.class as u8);
        writer.write_u8(self.gender as u8);
        writer.write_u16(self.level);
        self.location.encode(writer);
        writer.write_u8(self.direction as u8);
        writer.write_u8(self.hair);
        writer.write_u8(self.light);
        writer.write_i16(self.weapon);
        writer.write_i16(self.weapon_effect);
        writer.write_i16(self.armour);
        writer.write_u16(self.poison);
        writer.write_bool(self.dead);
        writer.write_bool(self.hidden);
        writer.write_u8(self.effect);
        writer.write_u8(self.wing_effect);
        writer.write_bool(self.extra);
        writer.write_i16(self.mount_type);
        writer.write_bool(self.riding_mount);
        writer.write_bool(self.fishing);
        writer.write_i16(self.transform_type);
        writer.write_u32(self.element_orb_effect);
        writer.write_u32(self.element_orb_level);
        writer.write_u32(self.element_orb_max);
        encode_u8_vec(writer, "object_player_buffs", &self.buffs)?;
        writer.write_u16(self.level_effects);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonsterInfo {
    pub object_id: u32,
    pub name: String,
    pub name_colour_argb: i32,
    pub location: Point,
    pub image: u16,
    pub direction: MirDirection,
    pub effect: u8,
    pub ai: u8,
    pub light: u8,
    pub dead: bool,
    pub skeleton: bool,
    pub poison: u16,
    pub hidden: bool,
    pub shock_time: i64,
    pub binding_shot_center: bool,
    pub extra: bool,
    pub extra_byte: u8,
    pub master_object_id: u32,
    pub rarity: u8,
    pub buffs: Vec<u8>,
}

impl MonsterInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            name: reader.read_string()?,
            name_colour_argb: reader.read_i32()?,
            location: Point::decode(reader)?,
            image: reader.read_u16()?,
            direction: MirDirection::try_from(reader.read_u8()?)?,
            effect: reader.read_u8()?,
            ai: reader.read_u8()?,
            light: reader.read_u8()?,
            dead: reader.read_bool()?,
            skeleton: reader.read_bool()?,
            poison: reader.read_u16()?,
            hidden: reader.read_bool()?,
            shock_time: reader.read_i64()?,
            binding_shot_center: reader.read_bool()?,
            extra: reader.read_bool()?,
            extra_byte: reader.read_u8()?,
            master_object_id: reader.read_u32()?,
            rarity: reader.read_u8()?,
            buffs: decode_u8_vec(reader, "monster_buffs")?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_u32(self.object_id);
        writer.write_string(&self.name)?;
        writer.write_i32(self.name_colour_argb);
        self.location.encode(writer);
        writer.write_u16(self.image);
        writer.write_u8(self.direction as u8);
        writer.write_u8(self.effect);
        writer.write_u8(self.ai);
        writer.write_u8(self.light);
        writer.write_bool(self.dead);
        writer.write_bool(self.skeleton);
        writer.write_u16(self.poison);
        writer.write_bool(self.hidden);
        writer.write_i64(self.shock_time);
        writer.write_bool(self.binding_shot_center);
        writer.write_bool(self.extra);
        writer.write_u8(self.extra_byte);
        writer.write_u32(self.master_object_id);
        writer.write_u8(self.rarity);
        encode_u8_vec(writer, "monster_buffs", &self.buffs)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcInfo {
    pub object_id: u32,
    pub name: String,
    pub name_colour_argb: i32,
    pub image: u16,
    pub colour_argb: i32,
    pub location: Point,
    pub direction: MirDirection,
    pub quest_ids: Vec<i32>,
}

impl NpcInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            name: reader.read_string()?,
            name_colour_argb: reader.read_i32()?,
            image: reader.read_u16()?,
            colour_argb: reader.read_i32()?,
            location: Point::decode(reader)?,
            direction: MirDirection::try_from(reader.read_u8()?)?,
            quest_ids: decode_i32_vec(reader, "npc_quest_ids")?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_u32(self.object_id);
        writer.write_string(&self.name)?;
        writer.write_i32(self.name_colour_argb);
        writer.write_u16(self.image);
        writer.write_i32(self.colour_argb);
        self.location.encode(writer);
        writer.write_u8(self.direction as u8);
        encode_i32_vec(writer, "npc_quest_ids", &self.quest_ids)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectItemInfo {
    pub object_id: u32,
    pub name: String,
    pub name_colour_argb: i32,
    pub location: Point,
    pub image: u16,
    pub grade: u8,
}

impl ObjectItemInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            name: reader.read_string()?,
            name_colour_argb: reader.read_i32()?,
            location: Point::decode(reader)?,
            image: reader.read_u16()?,
            grade: reader.read_u8()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_u32(self.object_id);
        writer.write_string(&self.name)?;
        writer.write_i32(self.name_colour_argb);
        self.location.encode(writer);
        writer.write_u16(self.image);
        writer.write_u8(self.grade);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectGoldInfo {
    pub object_id: u32,
    pub gold: u32,
    pub location: Point,
}

impl ObjectGoldInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            gold: reader.read_u32()?,
            location: Point::decode(reader)?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.object_id);
        writer.write_u32(self.gold);
        self.location.encode(writer);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectDiedInfo {
    pub object_id: u32,
    pub location: Point,
    pub direction: MirDirection,
    pub kind: u8,
}

impl ObjectDiedInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            location: Point::decode(reader)?,
            direction: MirDirection::try_from(reader.read_u8()?)?,
            kind: reader.read_u8()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.object_id);
        self.location.encode(writer);
        writer.write_u8(self.direction as u8);
        writer.write_u8(self.kind);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectRevivedInfo {
    pub object_id: u32,
    pub effect: bool,
}

impl ObjectRevivedInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            effect: reader.read_bool()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.object_id);
        writer.write_bool(self.effect);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectEffectInfo {
    pub object_id: u32,
    pub effect: u8,
    pub effect_type: u32,
    pub delay_time: u32,
    pub time: u32,
}

impl ObjectEffectInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            effect: reader.read_u8()?,
            effect_type: reader.read_u32()?,
            delay_time: reader.read_u32()?,
            time: reader.read_u32()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.object_id);
        writer.write_u8(self.effect);
        writer.write_u32(self.effect_type);
        writer.write_u32(self.delay_time);
        writer.write_u32(self.time);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectHealthInfo {
    pub object_id: u32,
    pub percent: u8,
    pub expire: u8,
}

impl ObjectHealthInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            percent: reader.read_u8()?,
            expire: reader.read_u8()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.object_id);
        writer.write_u8(self.percent);
        writer.write_u8(self.expire);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectManaInfo {
    pub object_id: u32,
    pub percent: u8,
}

impl ObjectManaInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            percent: reader.read_u8()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.object_id);
        writer.write_u8(self.percent);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectSpellInfo {
    pub object_id: u32,
    pub location: Point,
    pub spell: Spell,
    pub direction: MirDirection,
    pub param: bool,
}

impl ObjectSpellInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            location: Point::decode(reader)?,
            spell: Spell::try_from(reader.read_u8()?)?,
            direction: MirDirection::try_from(reader.read_u8()?)?,
            param: reader.read_bool()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.object_id);
        self.location.encode(writer);
        writer.write_u8(self.spell as u8);
        writer.write_u8(self.direction as u8);
        writer.write_bool(self.param);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapInformation {
    pub map_index: i32,
    pub file_name: String,
    pub title: String,
    pub mini_map: u16,
    pub big_map: u16,
    pub lights: u8,
    pub flags: u8,
    pub map_dark_light: u8,
    pub music: u16,
    pub weather_particles: u16,
}

impl MapInformation {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            map_index: reader.read_i32()?,
            file_name: reader.read_string()?,
            title: reader.read_string()?,
            mini_map: reader.read_u16()?,
            big_map: reader.read_u16()?,
            lights: reader.read_u8()?,
            flags: reader.read_u8()?,
            map_dark_light: reader.read_u8()?,
            music: reader.read_u16()?,
            weather_particles: reader.read_u16()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) -> Result<()> {
        writer.write_i32(self.map_index);
        writer.write_string(&self.file_name)?;
        writer.write_string(&self.title)?;
        writer.write_u16(self.mini_map);
        writer.write_u16(self.big_map);
        writer.write_u8(self.lights);
        writer.write_u8(self.flags);
        writer.write_u8(self.map_dark_light);
        writer.write_u16(self.music);
        writer.write_u16(self.weather_particles);
        Ok(())
    }

    pub fn has_lightning(&self) -> bool {
        self.flags & 0x01 != 0
    }

    pub fn has_fire(&self) -> bool {
        self.flags & 0x02 != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserLocation {
    pub position: Point,
    pub direction: MirDirection,
}

impl UserLocation {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            position: Point::decode(reader)?,
            direction: MirDirection::try_from(reader.read_u8()?)?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        self.position.encode(writer);
        writer.write_u8(self.direction as u8);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectMovement {
    pub object_id: u32,
    pub position: Point,
    pub direction: MirDirection,
}

impl ObjectMovement {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            position: Point::decode(reader)?,
            direction: MirDirection::try_from(reader.read_u8()?)?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.object_id);
        self.position.encode(writer);
        writer.write_u8(self.direction as u8);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectAttackInfo {
    pub object_id: u32,
    pub location: Point,
    pub direction: MirDirection,
    pub spell: u8,
    pub level: u8,
    pub attack_type: u8,
}

impl ObjectAttackInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            location: Point::decode(reader)?,
            direction: MirDirection::try_from(reader.read_u8()?)?,
            spell: reader.read_u8()?,
            level: reader.read_u8()?,
            attack_type: reader.read_u8()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.object_id);
        self.location.encode(writer);
        writer.write_u8(self.direction as u8);
        writer.write_u8(self.spell);
        writer.write_u8(self.level);
        writer.write_u8(self.attack_type);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StruckInfo {
    pub attacker_id: u32,
}

impl StruckInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            attacker_id: reader.read_u32()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.attacker_id);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectStruckInfo {
    pub object_id: u32,
    pub attacker_id: u32,
    pub location: Point,
    pub direction: MirDirection,
}

impl ObjectStruckInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            attacker_id: reader.read_u32()?,
            location: Point::decode(reader)?,
            direction: MirDirection::try_from(reader.read_u8()?)?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.object_id);
        writer.write_u32(self.attacker_id);
        self.location.encode(writer);
        writer.write_u8(self.direction as u8);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectRangeAttackInfo {
    pub object_id: u32,
    pub location: Point,
    pub direction: MirDirection,
    pub target_id: u32,
    pub target: Point,
    pub attack_type: u8,
    pub spell: u8,
    pub level: u8,
}

impl ObjectRangeAttackInfo {
    pub fn decode(reader: &mut PacketReader<'_>) -> Result<Self> {
        Ok(Self {
            object_id: reader.read_u32()?,
            location: Point::decode(reader)?,
            direction: MirDirection::try_from(reader.read_u8()?)?,
            target_id: reader.read_u32()?,
            target: Point::decode(reader)?,
            attack_type: reader.read_u8()?,
            spell: reader.read_u8()?,
            level: reader.read_u8()?,
        })
    }

    pub fn encode(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.object_id);
        self.location.encode(writer);
        writer.write_u8(self.direction as u8);
        writer.write_u32(self.target_id);
        self.target.encode(writer);
        writer.write_u8(self.attack_type);
        writer.write_u8(self.spell);
        writer.write_u8(self.level);
    }
}

fn decode_optional_item_section(
    reader: &mut PacketReader<'_>,
    section: &'static str,
) -> Result<(bool, Option<Vec<Option<UserItem>>>)> {
    let present = reader.read_bool()?;
    if !present {
        return Ok((false, None));
    }

    let count = reader.read_i32()?;
    if count < 0 {
        return Err(PacketCodecError::NegativeLength {
            field: section,
            value: count,
        });
    }

    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        if reader.read_bool()? {
            items.push(Some(UserItem::decode(reader)?));
        } else {
            items.push(None);
        }
    }

    Ok((true, Some(items)))
}

fn encode_optional_item_section(
    writer: &mut PacketWriter,
    present: bool,
    items: Option<&[Option<UserItem>]>,
) -> Result<()> {
    let present = present || items.is_some();
    writer.write_bool(present);
    if !present {
        return Ok(());
    }

    let Some(items) = items else {
        writer.write_i32(0);
        return Ok(());
    };
    writer.write_i32(
        i32::try_from(items.len())
            .map_err(|_| PacketCodecError::PacketTooLarge { size: items.len() })?,
    );
    for item in items {
        writer.write_bool(item.is_some());
        if let Some(item) = item {
            item.encode(writer)?;
        }
    }
    Ok(())
}

fn decode_empty_count_section(reader: &mut PacketReader<'_>, section: &'static str) -> Result<i32> {
    let count = reader.read_i32()?;
    if count < 0 {
        return Err(PacketCodecError::NegativeLength {
            field: section,
            value: count,
        });
    }
    if count != 0 {
        return Err(PacketCodecError::UnsupportedUserInformationSection { section, count });
    }
    Ok(count)
}

fn encode_empty_count_section(
    writer: &mut PacketWriter,
    section: &'static str,
    count: i32,
) -> Result<()> {
    if count < 0 {
        return Err(PacketCodecError::NegativeLength {
            field: section,
            value: count,
        });
    }
    if count != 0 {
        return Err(PacketCodecError::UnsupportedUserInformationSection { section, count });
    }
    writer.write_i32(count);
    Ok(())
}

fn decode_u8_vec(reader: &mut PacketReader<'_>, field: &'static str) -> Result<Vec<u8>> {
    let count = reader.read_i32()?;
    if count < 0 {
        return Err(PacketCodecError::NegativeLength {
            field,
            value: count,
        });
    }

    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        values.push(reader.read_u8()?);
    }
    Ok(values)
}

fn encode_u8_vec(writer: &mut PacketWriter, field: &'static str, values: &[u8]) -> Result<()> {
    let count = i32::try_from(values.len())
        .map_err(|_| PacketCodecError::PacketTooLarge { size: values.len() })?;
    if count < 0 {
        return Err(PacketCodecError::NegativeLength {
            field,
            value: count,
        });
    }

    writer.write_i32(count);
    for value in values {
        writer.write_u8(*value);
    }
    Ok(())
}

fn decode_i32_vec(reader: &mut PacketReader<'_>, field: &'static str) -> Result<Vec<i32>> {
    let count = reader.read_i32()?;
    if count < 0 {
        return Err(PacketCodecError::NegativeLength {
            field,
            value: count,
        });
    }

    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        values.push(reader.read_i32()?);
    }
    Ok(values)
}

fn encode_i32_vec(writer: &mut PacketWriter, field: &'static str, values: &[i32]) -> Result<()> {
    let count = i32::try_from(values.len()).map_err(|_| PacketCodecError::PacketTooLarge {
        size: values.len() * std::mem::size_of::<i32>(),
    })?;
    if count < 0 {
        return Err(PacketCodecError::NegativeLength {
            field,
            value: count,
        });
    }

    writer.write_i32(count);
    for value in values {
        writer.write_i32(*value);
    }
    Ok(())
}
