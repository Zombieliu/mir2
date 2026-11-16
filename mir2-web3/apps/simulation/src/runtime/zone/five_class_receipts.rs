//! The source-defined Assassin/Archer LevelMagic boundary. This is a trusted
//! Zone outcome, never a client command or a replacement for a learned spell.
use super::{SessionId, ZoneKey};
use mir2_protocol::{MirClass, Spell};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZoneFiveClassPracticeSpell {
    FatalSword,
    DoubleSlash,
    Haste,
    FlashDash,
    LightBody,
    HeavenlySword,
    FireBurst,
    Trap,
    PoisonSword,
    MoonLight,
    MPEater,
    SwiftFeet,
    DarkBody,
    Hemorrhage,
    CatTongue,
    CrescentSlash,
    Focus,
    StraightShot,
    MentalState,
    DoubleShot,
    ExplosiveTrap,
    Meditation,
    ElementalShot,
    BackStep,
    Concentration,
    DelayedExplosion,
    SummonVampire,
    BindingShot,
    ElementalBarrier,
    Stonetrap,
    VampireShot,
    SummonSnakes,
    PoisonShot,
    CrippleShot,
}

impl ZoneFiveClassPracticeSpell {
    pub fn from_spell(spell: Spell) -> Option<Self> {
        Some(match spell {
            Spell::FatalSword => Self::FatalSword,
            Spell::DoubleSlash => Self::DoubleSlash,
            Spell::Haste => Self::Haste,
            Spell::FlashDash => Self::FlashDash,
            Spell::LightBody => Self::LightBody,
            Spell::HeavenlySword => Self::HeavenlySword,
            Spell::FireBurst => Self::FireBurst,
            Spell::Trap => Self::Trap,
            Spell::PoisonSword => Self::PoisonSword,
            Spell::MoonLight => Self::MoonLight,
            Spell::MPEater => Self::MPEater,
            Spell::SwiftFeet => Self::SwiftFeet,
            Spell::DarkBody => Self::DarkBody,
            Spell::Hemorrhage => Self::Hemorrhage,
            Spell::CatTongue => Self::CatTongue,
            Spell::CrescentSlash => Self::CrescentSlash,
            Spell::Focus => Self::Focus,
            Spell::StraightShot => Self::StraightShot,
            Spell::MentalState => Self::MentalState,
            Spell::DoubleShot => Self::DoubleShot,
            Spell::ExplosiveTrap => Self::ExplosiveTrap,
            Spell::Meditation => Self::Meditation,
            Spell::ElementalShot => Self::ElementalShot,
            Spell::BackStep => Self::BackStep,
            Spell::Concentration => Self::Concentration,
            Spell::DelayedExplosion => Self::DelayedExplosion,
            Spell::SummonVampire => Self::SummonVampire,
            Spell::BindingShot => Self::BindingShot,
            Spell::ElementalBarrier => Self::ElementalBarrier,
            Spell::Stonetrap => Self::Stonetrap,
            Spell::VampireShot => Self::VampireShot,
            Spell::SummonSnakes => Self::SummonSnakes,
            Spell::PoisonShot => Self::PoisonShot,
            Spell::CrippleShot => Self::CrippleShot,
            _ => return None,
        })
    }

    pub fn spell(self) -> Spell {
        match self {
            Self::FatalSword => Spell::FatalSword,
            Self::DoubleSlash => Spell::DoubleSlash,
            Self::Haste => Spell::Haste,
            Self::FlashDash => Spell::FlashDash,
            Self::LightBody => Spell::LightBody,
            Self::HeavenlySword => Spell::HeavenlySword,
            Self::FireBurst => Spell::FireBurst,
            Self::Trap => Spell::Trap,
            Self::PoisonSword => Spell::PoisonSword,
            Self::MoonLight => Spell::MoonLight,
            Self::MPEater => Spell::MPEater,
            Self::SwiftFeet => Spell::SwiftFeet,
            Self::DarkBody => Spell::DarkBody,
            Self::Hemorrhage => Spell::Hemorrhage,
            Self::CatTongue => Spell::CatTongue,
            Self::CrescentSlash => Spell::CrescentSlash,
            Self::Focus => Spell::Focus,
            Self::StraightShot => Spell::StraightShot,
            Self::MentalState => Spell::MentalState,
            Self::DoubleShot => Spell::DoubleShot,
            Self::ExplosiveTrap => Spell::ExplosiveTrap,
            Self::Meditation => Spell::Meditation,
            Self::ElementalShot => Spell::ElementalShot,
            Self::BackStep => Spell::BackStep,
            Self::Concentration => Spell::Concentration,
            Self::DelayedExplosion => Spell::DelayedExplosion,
            Self::SummonVampire => Spell::SummonVampire,
            Self::BindingShot => Spell::BindingShot,
            Self::ElementalBarrier => Spell::ElementalBarrier,
            Self::Stonetrap => Spell::Stonetrap,
            Self::VampireShot => Spell::VampireShot,
            Self::SummonSnakes => Spell::SummonSnakes,
            Self::PoisonShot => Spell::PoisonShot,
            Self::CrippleShot => Spell::CrippleShot,
        }
    }

    pub fn skill_key(self) -> &'static str {
        match self {
            Self::FatalSword => "fatalsword",
            Self::DoubleSlash => "doubleslash",
            Self::Haste => "haste",
            Self::FlashDash => "flashdash",
            Self::LightBody => "lightbody",
            Self::HeavenlySword => "heavenlysword",
            Self::FireBurst => "fireburst",
            Self::Trap => "trap",
            Self::PoisonSword => "poisonsword",
            Self::MoonLight => "moonlight",
            Self::MPEater => "mpeater",
            Self::SwiftFeet => "swiftfeet",
            Self::DarkBody => "darkbody",
            Self::Hemorrhage => "hemorrhage",
            Self::CatTongue => "cattongue",
            Self::CrescentSlash => "crescentslash",
            Self::Focus => "focus",
            Self::StraightShot => "straightshot",
            Self::MentalState => "mentalstate",
            Self::DoubleShot => "doubleshot",
            Self::ExplosiveTrap => "explosivetrap",
            Self::Meditation => "meditation",
            Self::ElementalShot => "elementalshot",
            Self::BackStep => "backstep",
            Self::Concentration => "concentration",
            Self::DelayedExplosion => "delayedexplosion",
            Self::SummonVampire => "summonvampire",
            Self::BindingShot => "bindingshot",
            Self::ElementalBarrier => "elementalbarrier",
            Self::Stonetrap => "stonetrap",
            Self::VampireShot => "vampireshot",
            Self::SummonSnakes => "summonsnakes",
            Self::PoisonShot => "poisonshot",
            Self::CrippleShot => "crippleshot",
        }
    }

    pub fn class(self) -> MirClass {
        match self {
            Self::FatalSword
            | Self::DoubleSlash
            | Self::Haste
            | Self::FlashDash
            | Self::LightBody
            | Self::HeavenlySword
            | Self::FireBurst
            | Self::Trap
            | Self::PoisonSword
            | Self::MoonLight
            | Self::MPEater
            | Self::SwiftFeet
            | Self::DarkBody
            | Self::Hemorrhage
            | Self::CatTongue
            | Self::CrescentSlash => MirClass::Assassin,
            _ => MirClass::Archer,
        }
    }

    /// Stable server-only learned identity. Append future variants after these
    /// bits; never derive authority from the client's selected spell or level.
    pub fn learned_bit(self) -> u64 {
        let bit = match self {
            Self::FatalSword => 0,
            Self::DoubleSlash => 1,
            Self::Haste => 2,
            Self::FlashDash => 3,
            Self::LightBody => 4,
            Self::HeavenlySword => 5,
            Self::FireBurst => 6,
            Self::Trap => 7,
            Self::PoisonSword => 8,
            Self::MoonLight => 9,
            Self::MPEater => 10,
            Self::SwiftFeet => 11,
            Self::DarkBody => 12,
            Self::Hemorrhage => 13,
            Self::CatTongue => 14,
            Self::CrescentSlash => 15,
            Self::Focus => 16,
            Self::StraightShot => 17,
            Self::MentalState => 18,
            Self::DoubleShot => 19,
            Self::ExplosiveTrap => 20,
            Self::Meditation => 21,
            Self::ElementalShot => 22,
            Self::BackStep => 23,
            Self::Concentration => 24,
            Self::DelayedExplosion => 25,
            Self::SummonVampire => 26,
            Self::BindingShot => 27,
            Self::ElementalBarrier => 28,
            Self::Stonetrap => 29,
            Self::VampireShot => 30,
            Self::SummonSnakes => 31,
            Self::PoisonShot => 32,
            Self::CrippleShot => 33,
        };
        1_u64 << bit
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneFiveClassPracticeReceipt {
    pub spell: ZoneFiveClassPracticeSpell,
    pub session_id: SessionId,
    pub account_id: String,
    pub character_index: i32,
    pub object_id: u32,
    pub life_generation: u64,
    pub zone_key: ZoneKey,
    pub sequence: u64,
    pub committed_at_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_book_whitelist_is_bijective_by_shape_including_misspelled_book_names() {
        let manifest = mir2_game_data::crystal_item_manifest();
        let mut seen = std::collections::BTreeSet::new();
        for book in &manifest.items {
            if book.item_type != 20
                || !matches!(book.required_class, 8 | 16)
                || book.required_type != 0
                || book.required_amount > 50
            {
                continue;
            }
            let magic = mir2_game_data::crystal_magic_for_book(book).expect("actual Shape magic");
            let spell = Spell::from_crystal_name(&magic.spell).expect("native source spell");
            let typed = ZoneFiveClassPracticeSpell::from_spell(spell)
                .expect("bounded source book whitelist");
            assert_eq!(typed.spell(), spell);
            assert_eq!(
                typed.class(),
                if book.required_class == 8 {
                    MirClass::Assassin
                } else {
                    MirClass::Archer
                }
            );
            assert_eq!(
                typed.skill_key(),
                crate::runtime::skills::normalize_crystal_skill_key(&magic.spell)
            );
            assert!(seen.insert(typed));
        }
        assert_eq!(seen.len(), 34);
        assert_eq!(
            seen.iter()
                .fold(0_u64, |mask, spell| mask | spell.learned_bit()),
            (1_u64 << 34) - 1
        );
        assert!(ZoneFiveClassPracticeSpell::from_spell(Spell::FireBall).is_none());
        assert!(ZoneFiveClassPracticeSpell::from_spell(Spell::OneWithNature).is_none());
    }
}
