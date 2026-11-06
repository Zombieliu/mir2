//! Install an opaque canonical source after the Host has quiesced the shared
//! Zone. Existing Zone transforms/vitals remain authoritative; no gameplay,
//! source save, account bootstrap, or economic debit is replayed here.
use super::components::{
    current_player_is_dead, entity_facing, entity_player_vitals, entity_position, player_entity,
    Facing, Position,
};
use super::map::normalize_map_file_name;
use super::resources::{
    BuffResource, PlayerActionTimingResource, PlayerMovementTimingResource, PlayerRuntimeResource,
    PotionRecoveryResource, RuntimeConfigResource, RuntimeQueueResource, SessionResource,
    SkillResource, Stage5SystemsResource,
};
use super::session::SimulationSession;
use crate::{CharacterSaveRecord, PreparedZoneSourceRecovery, SimulationConfig};
use serde_json::Value;

fn encoded(values: &[String]) -> Result<Vec<Value>, String> {
    values
        .iter()
        .map(|value| serde_json::from_str(value).map_err(|e| e.to_string()))
        .collect()
}
fn normalized_refine(value: &mut Value) {
    if let Some(refine) = value.get_mut("refine").and_then(Value::as_object_mut) {
        for key in ["remainingMs", "clockEpoch", "collectDeadlineMs", "ready"] {
            refine.remove(key);
        }
    }
}
fn economic_state(save: &CharacterSaveRecord) -> Result<Value, String> {
    let mut systems = save
        .stage5_systems_json
        .as_deref()
        .map(serde_json::from_str::<Value>)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or(Value::Null);
    normalized_refine(&mut systems);
    Ok(serde_json::json!({
        "gold": save.gold, "credit": save.credit, "city": save.city_currencies,
        "inventoryCapacity": save.inventory_capacity,
        "equipmentExplicitEmpty": save.equipment_items_explicit_empty,
        "inventory": encoded(&save.inventory_items_json)?, "belt": encoded(&save.belt_items_json)?,
        "storage": encoded(&save.storage_items_json)?, "equipment": encoded(&save.equipment_items_json)?,
        "heroInventory": encoded(&save.hero_inventory_items_json)?, "heroEquipment": encoded(&save.hero_equipment_items_json)?,
        "heroRegistry": save.hero_registry_attachment,
        "heroCapacity": save.hero_inventory_capacity, "heroLegacy40": save.hero_inventory_legacy_40,
        "pets": save.saved_pets,
        "rentals": encoded(&save.item_rental_records_json)?, "hasRented": save.has_rented_item,
        "buyBack": encoded(&save.npc_buy_back_items_json)?, "usedGoods": encoded(&save.npc_used_goods_items_json)?,
        "systems": systems, "npcPurchase": save.npc_purchase_journal, "production": save.personal_production,
        "guildJournal": save.guild_experience_journal, "killReceipts": save.player_kill_receipts,
        "npcFlags": encoded(&save.npc_flag_states_json)?, "npcValues": encoded(&save.npc_saved_values_json)?,
        "defaultNpcEvents": save.default_npc_events,
        "quests": encoded(&save.quest_states_json)?, "level": save.character.level,
        "experience": save.experience, "maximumExperience": save.max_experience,
    }))
}
fn buff_identities(values: &[String]) -> Result<Vec<Value>, String> {
    let mut values = encoded(values)?;
    for value in &mut values {
        if let Some(fields) = value.as_object_mut() {
            fields.remove("expires_at_tick");
            if fields.get("real_time_duration").is_some() {
                fields.insert("real_time_duration".into(), Value::Bool(true));
            }
        }
    }
    Ok(values)
}

fn normalized_checkpoint_skills(save: &CharacterSaveRecord) -> Result<Value, String> {
    // Both representations use the same clock origin. The clock result is
    // deliberately excluded below; all learned-skill business fields remain.
    let skills = super::save::canonical_recovery_skill_states(save, 0)?;
    let raw = encoded(&save.skill_states_json)?;
    let normalized = skills
        .into_iter()
        .zip(raw)
        .map(|(skill, raw)| {
            let mut value = serde_json::to_value(skill).map_err(|error| error.to_string())?;
            let fields = value
                .as_object_mut()
                .ok_or("recovery skill is not an object")?;
            // The typed decoder supplies existing serde defaults, but it must not
            // silently discard a future non-clock business field from the source.
            for (key, value) in raw
                .as_object()
                .ok_or("recovery raw skill is not an object")?
            {
                if key != "_runtimeCooldownClock" && !fields.contains_key(key) {
                    fields.insert(key.clone(), value.clone());
                }
            }
            fields.remove("cooldown_ends_at");
            fields.remove("cast_time_ms");
            Ok(value)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut normalized = Value::Array(normalized);
    normalize_checkpoint_json_carriers(&mut normalized, 0)?;
    Ok(normalized)
}

fn parse_checkpoint_json_carrier(value: &Value, depth: usize) -> Result<Value, String> {
    if depth > 64 {
        return Err("recovery JSON carrier nesting exceeded".into());
    }
    match value {
        Value::String(encoded) => {
            let mut parsed = serde_json::from_str(encoded).map_err(|error| error.to_string())?;
            normalize_checkpoint_json_carriers(&mut parsed, depth + 1)?;
            Ok(parsed)
        }
        Value::Array(values) => values
            .iter()
            .map(|value| parse_checkpoint_json_carrier(value, depth + 1))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Object(values) => values
            .iter()
            .map(|(key, value)| {
                Ok((
                    key.clone(),
                    parse_checkpoint_json_carrier(value, depth + 1)?,
                ))
            })
            .collect::<Result<_, String>>()
            .map(Value::Object),
        Value::Null => Ok(Value::Null),
        _ => Err("recovery JSON carrier has an invalid shape".into()),
    }
}

fn normalize_checkpoint_json_carriers(value: &mut Value, depth: usize) -> Result<(), String> {
    if depth > 64 {
        return Err("recovery JSON carrier nesting exceeded".into());
    }
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                // Stage5 held-item maps use legacy names without a Json suffix.
                if key.ends_with("_json")
                    || key.ends_with("Json")
                    || matches!(key.as_str(), "itemStates" | "storageItemStates")
                {
                    *value = parse_checkpoint_json_carrier(value, depth + 1)?;
                } else {
                    normalize_checkpoint_json_carriers(value, depth + 1)?;
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                normalize_checkpoint_json_carriers(value, depth + 1)?;
            }
        }
        _ => (),
    }
    Ok(())
}

fn normalized_runtime_checkpoint(save: &CharacterSaveRecord) -> Result<Value, String> {
    super::save::validate_character_save_record(save)?;
    let mut value = serde_json::to_value(save).map_err(|error| error.to_string())?;
    let fields = value
        .as_object_mut()
        .ok_or("recovery checkpoint is not an object")?;
    for key in [
        "position",
        "direction",
        "hp",
        "max_hp",
        "mp",
        "max_mp",
        "hero_vitals",
    ] {
        fields.remove(key);
    }
    // Compare every encoded carrier structurally, including nested Stage5
    // custody. A typed round trip must not discard future business fields.
    normalize_checkpoint_json_carriers(&mut value, 0)?;
    let fields = value
        .as_object_mut()
        .ok_or("recovery checkpoint is not an object")?;
    if let Some(systems) = fields.get_mut("stage5_systems_json") {
        normalized_refine(systems);
    }
    fields.insert(
        "skill_states_json".into(),
        normalized_checkpoint_skills(save)?,
    );
    let mut buffs = Value::Array(buff_identities(&save.buff_states_json)?);
    normalize_checkpoint_json_carriers(&mut buffs, 0)?;
    fields.insert("buff_states_json".into(), buffs);
    Ok(value)
}

impl PreparedZoneSourceRecovery {
    /// Compare a reconstructable private image with its exact opaque PG source.
    /// This read-only check grants no authority and changes no canonical bytes.
    pub fn validate_runtime_checkpoint(
        &self,
        account: &str,
        index: i32,
        checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        if index < 0 || checkpoint.character.index != index {
            return Err("zone source recovery runtime checkpoint subject mismatch".into());
        }
        let canonical = self
            .checkpoint(account, index)
            .ok_or("zone source recovery runtime checkpoint source is missing")?;
        if normalized_runtime_checkpoint(canonical)? != normalized_runtime_checkpoint(checkpoint)? {
            return Err(
                "zone source recovery runtime checkpoint changed canonical business state".into(),
            );
        }
        Ok(())
    }
}

impl SimulationSession {
    pub fn install_zone_source_recovery(
        &mut self,
        source: &PreparedZoneSourceRecovery,
        authoritative: &SimulationConfig,
    ) -> Result<(), String> {
        source.validate_installed_cache(authoritative)?;
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if !config.has_isolated_replica_bootstrap_authority() {
            // An exact repeated installation may observe the already rebound
            // source. Any other live writer/configuration is rejected.
            source.validate_source_binding(&config)?;
        }
        let identity = self
            .active_identity()
            .ok_or("zone source recovery requires an active actor")?;
        let canonical = source
            .checkpoint(&identity.account_id, identity.character_index)
            .ok_or("zone source recovery actor is outside its canonical read set")?
            .clone();
        source.validate_active_account(&identity.account_id)?;
        super::save::validate_character_save_record(&canonical)?;
        let before = self
            .active_character_checkpoint()
            .ok_or("zone source recovery private checkpoint missing")?;
        if canonical.character.index != identity.character_index
            || canonical.character.name != identity.character_name
            || canonical.character.class != before.character.class
            || canonical.character.gender != before.character.gender
        {
            return Err("zone source recovery character incarnation mismatch".into());
        }
        if canonical.revision < before.revision {
            return Err("zone source recovery would overwrite a newer private revision".into());
        }
        if normalize_map_file_name(&canonical.map_file_name)
            != normalize_map_file_name(&before.map_file_name)
        {
            return Err("zone source recovery map differs from the current shared map".into());
        }
        if canonical.revision == before.revision
            && economic_state(&canonical)? != economic_state(&before)?
        {
            return Err("zone source recovery equal-revision economic state conflict".into());
        }
        if authoritative
            .content_profile
            .as_ref()
            .is_some_and(|profile| {
                !profile.class_is_allowed(canonical.character.class)
                    || !profile.map_is_allowed(&canonical.map_file_name)
            })
        {
            return Err("zone source recovery is outside the configured content profile".into());
        }
        let world = self.app.world();
        let player = player_entity(world).ok_or("zone source recovery player missing")?;
        let position =
            entity_position(world, player).ok_or("zone source recovery transform missing")?;
        let direction =
            entity_facing(world, player).ok_or("zone source recovery facing missing")?;
        let vitals =
            entity_player_vitals(world, player).ok_or("zone source recovery vitals missing")?;
        let dead = current_player_is_dead(world);
        let tick = super::resources::runtime_tick(world);
        let old_skills = world.resource::<SkillResource>().skills.clone();
        let player_timing = world.resource::<PlayerRuntimeResource>().clone();
        let action_timing = world.resource::<PlayerActionTimingResource>().clone();
        let movement_timing = world.resource::<PlayerMovementTimingResource>().clone();
        let old_buffs = world.resource::<BuffResource>().clone();
        let preserve_buffs = buff_identities(&before.buff_states_json)?
            == buff_identities(&canonical.buff_states_json)?;
        let old_refine = world
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .refine
            .clone();
        let mut before_refine = serde_json::json!({"refine": old_refine});
        let mut after_refine: Value = canonical
            .stage5_systems_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|e| e.to_string())?
            .unwrap_or(Value::Null);
        normalized_refine(&mut before_refine);
        normalized_refine(&mut after_refine);
        let preserve_refine = before_refine.get("refine") == after_refine.get("refine");
        let queue = world.resource::<RuntimeQueueResource>().clone();
        let potion = world.resource::<PotionRecoveryResource>().clone();
        let mut skills = super::save::canonical_recovery_skill_states(&canonical, tick)?;
        for skill in &mut skills {
            if let Some(old) = old_skills.iter().find(|old| old.key == skill.key) {
                skill.cooldown_ends_at = old.cooldown_ends_at;
                skill.cast_time_ms = old.cast_time_ms;
            }
        }
        // Account metadata (GM/storage/ban policy) must also come from the
        // canonical read, without granting a live writer during restoration.
        let install_config = authoritative.fork_for_replica_apply()?;
        self.app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config = install_config;
        // Existing private restore validates all carriers and monotonic NPC/
        // production history before it replaces source-owned personal state.
        // The replica keeps persistence disabled throughout the installation.
        if let Err(error) = self.restore_active_character_checkpoint(&canonical) {
            *self = SimulationSession::new(config);
            super::resources::set_runtime_tick(self.app.world_mut(), tick);
            return Err(error);
        }
        let restored_assets = self
            .active_character_checkpoint()
            .ok_or_else(|| "zone source recovery installed checkpoint missing".to_string())
            .and_then(|save| economic_state(&save));
        match restored_assets {
            Ok(restored) if restored == economic_state(&canonical)? => {}
            _ => {
                // Legacy repair must never silently select or mint a different
                // carrier from the complete canonical checkpoint during promotion.
                *self = SimulationSession::new(config);
                super::resources::set_runtime_tick(self.app.world_mut(), tick);
                return Err(
                    "zone source recovery changed canonical personal assets during restore".into(),
                );
            }
        }
        {
            let world = self.app.world_mut();
            world.resource_mut::<SkillResource>().skills = skills;
            if preserve_buffs {
                *world.resource_mut::<BuffResource>() = old_buffs;
            }
            if preserve_refine {
                world
                    .resource_mut::<Stage5SystemsResource>()
                    .stage5_systems
                    .refine = old_refine;
            }
            *world.resource_mut::<RuntimeQueueResource>() = queue;
            *world.resource_mut::<PotionRecoveryResource>() = potion;
            *world.resource_mut::<PlayerActionTimingResource>() = action_timing;
            *world.resource_mut::<PlayerMovementTimingResource>() = movement_timing;
        }
        {
            // These are the already-authoritative shared values, not a new
            // move/life event: callbacks would replay hooks and advance time.
            let world = self.app.world_mut();
            let player =
                player_entity(world).ok_or("zone source recovery rebuilt player missing")?;
            world.entity_mut(player).insert((
                Position(position.clone()),
                Facing(direction),
                vitals,
            ));
            let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
            runtime.player_position = position;
            runtime.player_direction = direction;
            runtime.player_vitals = vitals;
            runtime.player_dead = dead;
            runtime.chat_next_allowed_at_ms = player_timing.chat_next_allowed_at_ms;
            runtime.chat_spam_tick = player_timing.chat_spam_tick;
            runtime.last_damaged_tick = player_timing.last_damaged_tick;
            runtime.last_regen_tick = player_timing.last_regen_tick;
        }
        // Recheck the local source head after restore. A shared account may
        // advance while the private world is being rebuilt; never bind stale.
        if let Err(error) = source.validate_installed_cache(authoritative) {
            *self = SimulationSession::new(config);
            super::resources::set_runtime_tick(self.app.world_mut(), tick);
            return Err(error);
        }
        self.rebind_account_store(authoritative);
        self.bind_zone_save_fence(source.fence().clone())?;
        // Rebinding carries the original shared write freeze; recovery never
        // clears it, even when the canonical account was successfully read.
        self.app
            .world()
            .resource::<SessionResource>()
            .bind_active_save_revision(canonical.revision);
        Ok(())
    }
}

#[cfg(test)]
#[path = "zone_source_recovery_tests.rs"]
mod tests;
