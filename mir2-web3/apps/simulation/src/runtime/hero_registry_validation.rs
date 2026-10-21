/// Validate durable Hero carriers using the same exact-carrier conversion as
/// ordinary Hero custody, including nested IDs and raw Crystal slot bounds.
pub(crate) fn validate_shared_hero_carriers(
    state: &crate::config::SharedHeroState,
) -> Result<std::collections::BTreeSet<u64>, String> {
    let decode = |values: &[String]| values.iter().map(|value| {
        serde_json::from_str::<super::items::ItemState>(value)
            .map_err(|error| format!("invalid registry Hero carrier: {error}"))
    }).collect::<Result<Vec<_>,_>>();
    let resource = super::resources::HeroInventoryResource {
        items: decode(&state.inventory_items_json)?,
        equipment: decode(&state.equipment_items_json)?,
        capacity: state.inventory_capacity,
        legacy_40: state.inventory_legacy_40,
        saved_vitals: state.vitals,
        registry_attachment: None,
    };
    super::hero_inventory::validate_hero_custody(&resource)
}

/// A seal is a transferable shell: none of its retained physical items may
/// hide economic binding. Attached custody proves account/index only; the live
/// actor still validates the original realm/incarnation before ordinary use.
pub(crate) fn validate_hunting_hero_binding_custody(
    hero: &crate::config::SharedHeroRecord,
) -> Result<(), String> {
    use crate::config::SharedHeroCustody;
    use super::items::{hunting_item_binding::has_bound, ItemState};
    if matches!(hero.custody, SharedHeroCustody::Released) {
        return Ok(());
    }
    for encoded in hero.state.inventory_items_json.iter().chain(&hero.state.equipment_items_json) {
        let item: ItemState = serde_json::from_str(encoded)
            .map_err(|error| format!("invalid registry Hero binding carrier: {error}"))?;
        if !has_bound(&item) {
            continue;
        }
        super::items::validate_committed_item_state_carrier(&item)
            .map_err(|error| format!("invalid registry Hero binding carrier: {error}"))?;
        match &hero.custody {
            SharedHeroCustody::Sealed { .. } if has_bound(&item) => {
                return Err("sealed Hero cannot contain economically bound items".into());
            }
            SharedHeroCustody::Attached { account_id, character_index, .. } => {
                let mut pending = vec![&item];
                while let Some(item) = pending.pop() {
                    if item.economic_binding.as_ref().is_some_and(|binding|
                        !binding.matches_account_character(account_id, *character_index))
                    {
                        return Err("attached Hero bound item belongs to another character".into());
                    }
                    pending.extend(&item.socketed);
                }
            }
            _ => {}
        }
    }
    Ok(())
}
