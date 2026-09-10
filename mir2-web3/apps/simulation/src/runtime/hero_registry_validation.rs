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
