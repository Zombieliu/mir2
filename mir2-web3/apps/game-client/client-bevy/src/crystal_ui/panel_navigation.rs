//! Local panel state. These actions never submit a gameplay/server command.
use super::spec::CrystalRect;
use bevy::prelude::{Component, Resource};
use serde::{Deserialize, Serialize};
pub const CHARACTER_RECT: CrystalRect = CrystalRect::new(760., 0., 264., 380.);
pub const CHARACTER_PAGE_RECT: CrystalRect = CrystalRect::new(8., 90., 248., 284.);
pub const CHARACTER_CLOSE_RECT: CrystalRect = CrystalRect::new(241., 3., 24., 21.);
pub fn character_tabs() -> [(CharacterPage, CrystalRect, u16); 4] {
    [
        (
            CharacterPage::Character,
            CrystalRect::new(8., 70., 64., 20.),
            500,
        ),
        (
            CharacterPage::Stats1,
            CrystalRect::new(70., 70., 64., 20.),
            501,
        ),
        (
            CharacterPage::Stats2,
            CrystalRect::new(132., 70., 64., 20.),
            502,
        ),
        (
            CharacterPage::Spells,
            CrystalRect::new(194., 70., 64., 20.),
            503,
        ),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CharacterPage {
    #[default]
    Character,
    Stats1,
    Stats2,
    Spells,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Resource, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelNavigation {
    pub character_open: bool,
    pub character_page: CharacterPage,
    pub bag_open: bool,
    pub quest_open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Component)]
#[serde(tag = "type", content = "page", rename_all = "camelCase")]
pub enum PanelAction {
    Character,
    Bag,
    Quest,
    SelectCharacterPage(CharacterPage),
    CloseCharacter,
    CloseBag,
    OpenBag,
    CloseQuest,
    OpenQuest,
}

pub fn reduce(mut state: PanelNavigation, action: PanelAction) -> PanelNavigation {
    match action {
        PanelAction::Character => {
            state.character_open =
                !(state.character_open && state.character_page == CharacterPage::Character);
            state.character_page = CharacterPage::Character;
        }
        PanelAction::Bag => state.bag_open = !state.bag_open,
        PanelAction::Quest => state.quest_open = !state.quest_open,
        PanelAction::SelectCharacterPage(page) => {
            state.character_open = true;
            state.character_page = page;
        }
        PanelAction::CloseCharacter => state.character_open = false,
        PanelAction::CloseBag => state.bag_open = false,
        PanelAction::OpenBag => state.bag_open = true,
        PanelAction::CloseQuest => state.quest_open = false,
        PanelAction::OpenQuest => state.quest_open = true,
    }
    state
}
