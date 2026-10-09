//! Minimal host state for the shared Quest nodes outside the native overlay.
//! This module never installs audio, file persistence, or a native shell UI.

use bevy::prelude::{Resource, SystemSet};
use mir2_ui_core::{action::UiAction, effect::UiEffect, reducer::Transition, state::{UiPanel, UiState}};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativePlayerUiSet {
    Mutate,
    Read,
}

#[derive(Debug, Clone, Resource)]
pub struct NativePlayerUiState {
    pub core: UiState,
    pub keyboard: (),
    pub blocks_gameplay: bool,
    pub blocks_world: bool,
}

impl Default for NativePlayerUiState {
    fn default() -> Self {
        Self { core: UiState::default(), keyboard: (), blocks_gameplay: false, blocks_world: false }
    }
}

impl NativePlayerUiState {
    pub fn quest_open(&self) -> bool { self.core.panel == UiPanel::QuestLog }
    pub fn blocks_gameplay_keys(&self) -> bool { self.blocks_gameplay || self.quest_open() }
    pub fn blocks_world_click(&self) -> bool { self.blocks_world || self.quest_open() }
    pub fn blocks_world_action(&self, dialog_open: bool, dead: bool) -> bool {
        dead || dialog_open || self.blocks_world_click()
    }
}

#[derive(Debug, Default, Resource)]
pub struct UiEffectQueue {
    effects: Vec<UiEffect>,
}

impl UiEffectQueue {
    pub fn push(&mut self, effect: UiEffect) { self.effects.push(effect); }
    pub fn drain(&mut self) -> Vec<UiEffect> { std::mem::take(&mut self.effects) }
}

pub fn dispatch_ui_action(core: &mut UiState, effects: &mut UiEffectQueue, action: UiAction) -> Transition {
    if core.screen != mir2_ui_core::state::UiScreen::InGame {
        core.screen = mir2_ui_core::state::UiScreen::InGame;
    }
    let transition = mir2_ui_core::reducer::reduce(core, action);
    *core = transition.state.clone();
    for effect in transition.effects.iter().cloned() { effects.push(effect); }
    transition
}
