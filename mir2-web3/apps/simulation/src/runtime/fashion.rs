//! Display-only wardrobe projection. Fashion never becomes EquipmentState and
//! cannot participate in combat stats, item sets, weight, durability or custody.
use bevy_ecs::world::World;
use mir2_game_data::atelier::{fashion, Fashion};
use mir2_protocol::MirClass;

use crate::config::Stage5AppearanceState;

use super::components::{player_entity, CharacterBody};
use super::resources::{SessionResource, Stage5SystemsResource};
use super::save::active_session_mutating_account_id;

/// Validate only wardrobe ownership/catalogue/class invariants. The durable
/// atelier adapter separately validates its operation receipts and debits.
pub(super) fn validate_appearance(
    appearance: &Stage5AppearanceState,
    class: MirClass,
) -> Result<(), String> {
    for id in &appearance.owned_fashions {
        if fashion(id).is_none() {
            return Err(format!("unknown owned fashion: {id}"));
        }
    }
    if let Some(id) = &appearance.selected_fashion {
        let style = fashion(id).ok_or_else(|| format!("unknown selected fashion: {id}"))?;
        if !appearance.owned_fashions.contains(id) {
            return Err("selected fashion is not owned by this character".into());
        }
        if !style.supports_class(class) {
            return Err("selected fashion has no supported class artwork".into());
        }
    }
    Ok(())
}

/// Resolve only the current authenticated character. Invalid/legacy selection
/// falls back to real equipment at every caller; it never borrows another actor.
pub(super) fn selected_style(world: &World) -> Option<&'static Fashion> {
    let session = world.get_resource::<SessionResource>()?;
    active_session_mutating_account_id(session)?;
    let character = session.selected_character.as_ref()?;
    let player = player_entity(world)?;
    let body = world.entity(player).get::<CharacterBody>()?;
    if body.class != character.class || body.gender != character.gender {
        return None;
    }
    let appearance = &world
        .get_resource::<Stage5SystemsResource>()?
        .stage5_systems
        .appearance;
    let id = appearance.selected_fashion.as_deref()?;
    if !appearance.owned_fashions.contains(id) {
        return None;
    }
    fashion(id).filter(|style| style.supports_class(character.class))
}

pub(super) fn armour_shape(world: &World) -> Option<u16> {
    selected_style(world).map(|style| style.armour_shape)
}

pub(super) fn armour_image(world: &World) -> Option<u32> {
    let style = selected_style(world)?;
    let gender = world
        .get_resource::<SessionResource>()?
        .selected_character
        .as_ref()?
        .gender;
    Some(style.state_image(gender))
}

#[cfg(test)]
#[path = "fashion_tests.rs"]
mod tests;
