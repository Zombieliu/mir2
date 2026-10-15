use super::*;
use mir2_protocol::ClientPacket;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn source() -> (SimulationConfig, std::path::PathBuf, SimulationSession) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let base = std::env::temp_dir().canonicalize().unwrap();
    let path = base.join(format!(
        "mir2-character-handoff-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(path.starts_with(&base));
    std::fs::create_dir(&path).unwrap();
    let path = path.join("accounts.json");
    let mut config = SimulationConfig::default().with_account_store_path(&path);
    config.visible_monsters.clear();
    config.map_hazards.clear();
    let mut session = select(&config);
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    (config, path, session)
}

fn select(config: &SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config.clone());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into()
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session
}

fn actual(path: &std::path::Path) -> CharacterSaveRecord {
    let store: crate::config::AccountStore =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    store.accounts["demo"].saves[&0].clone()
}

fn install_foreign_refine(source: &mut SimulationSession) {
    use crate::runtime::{item_custody, items};
    let item = {
        let mut inventory = source.app.world_mut().resource_mut::<InventoryResource>();
        let index = inventory
            .inventory_items
            .iter()
            .position(|item| item.key == "dagger")
            .expect("actual carried starter weapon");
        assert!(item_custody::can_take(
            &inventory,
            &inventory.inventory_items[index]
        ));
        // Move the actual bag item, retaining its UID and complete metadata.
        // Converting an unbound legacy equipment row to Bag1/0 would instead
        // invent that slot's UID and collide with its still-carried owner.
        inventory.inventory_items.remove(index)
    };
    let unique_id = items::item_unique_id(&item);
    assert!(!crate::runtime::inventory::inventory_unique_id_is_used(
        source.app.world().resource::<InventoryResource>(),
        unique_id
    ));
    let mut wire = items::try_user_item_from_item_state(&item).unwrap();
    wire.refine_added = 1;
    let item = items::try_item_state_from_user_item(item, &wire).unwrap();
    assert_eq!(items::item_unique_id(&item), unique_id);
    {
        let mut systems = source
            .app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>();
        systems.stage5_systems.refine = crate::config::Stage5RefineState {
            current_item: Some(item.key.clone()),
            oven_item_state_json: Some(item_custody::encode(&item).unwrap()),
            remaining_ms: 60_000,
            clock_epoch: Some("another-source-process".into()),
            collect_deadline_ms: Some(90_000),
            refining: true,
            ..Default::default()
        };
    }
    item_custody::refresh(source.app.world_mut()).unwrap();
    assert!(source
        .app
        .world()
        .resource::<InventoryResource>()
        .reserved_item_unique_ids
        .contains(&unique_id));
}

#[test]
fn handoff_file_resume_uses_one_admission_and_preserves_source_tick_and_foreign_refine() {
    let (config, path, mut source) = source();
    set_runtime_tick(source.app.world_mut(), 27);
    {
        let mut skills = source.app.world_mut().resource_mut::<SkillResource>();
        let skill = skills
            .skills
            .first_mut()
            .expect("actual learned starter skill");
        skill.cooldown_ticks = 3;
        skill.cooldown_ends_at = 30;
    }
    install_foreign_refine(&mut source);
    let handoff = source.prepare_character_handoff().unwrap();
    assert!(same_record(&handoff.durable, &actual(&path)).unwrap());
    assert_eq!(
        handoff.durable.stage5_systems_json,
        handoff.runtime.stage5_systems_json
    );
    let bytes = std::fs::read(&path).unwrap();
    let mut target = select(&config);
    assert!(
        target
            .restore_active_character_checkpoint(&handoff.runtime)
            .is_err(),
        "ordinary Source restore stays closed"
    );
    assert!(
        target.apply_character_handoff_replica(&handoff).is_err(),
        "a live File Source cannot enter verified replica replay"
    );
    let committed = target.resume_character_handoff(&handoff).unwrap();
    handoff.validate_commitment(&committed).unwrap();
    assert_eq!(runtime_tick(target.app.world()), 27);
    assert_eq!(
        target.app.world().resource::<SkillResource>().skills[0].cooldown_ends_at,
        30
    );
    assert_eq!(
        committed.runtime.default_npc_events,
        handoff.durable.default_npc_events
    );
    let systems: Stage5SystemsState =
        serde_json::from_str(committed.runtime.stage5_systems_json.as_deref().unwrap()).unwrap();
    assert!(!systems.refine.ready);
    assert!(systems.refine.remaining_ms > 0 && systems.refine.remaining_ms <= 60_000);
    assert_ne!(
        systems.refine.clock_epoch.as_deref(),
        Some("another-source-process")
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let mut ordinary = select(&config);
    ordinary.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let after = actual(&path);
    assert_eq!(
        after.default_npc_events.committed_sequence,
        handoff.durable.default_npc_events.committed_sequence + 1
    );
    assert_eq!(
        after.default_npc_events.next_sequence,
        handoff.durable.default_npc_events.next_sequence + 1
    );
}

#[test]
fn handoff_rejects_unlogged_foreign_rewritten_and_early_ready_images_without_source_writes() {
    let (config, path, mut source) = source();
    install_foreign_refine(&mut source);
    let handoff = source.prepare_character_handoff().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let mut unlogged = SimulationSession::new(config.clone());
    assert!(unlogged.resume_character_handoff(&handoff).is_err());
    assert!(unlogged.active_identity().is_none());
    let mut restricted = config.clone();
    let mut profile = crate::config::ContentProfileRuntime::platinum_176();
    profile.profile.map_whitelist.clear();
    restricted.content_profile = Some(profile);
    let mut outside_profile = select(&restricted);
    assert!(outside_profile.resume_character_handoff(&handoff).is_err());
    assert!(outside_profile.active_identity().is_none());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let mut cases = Vec::new();
    let mut changed = handoff.clone();
    changed.runtime.gold += 1;
    cases.push(changed);
    let mut changed = handoff.clone();
    changed.durable.gold += 1;
    changed.runtime.gold += 1;
    cases.push(changed);
    let mut changed = handoff.clone();
    changed.durable.character.name = "AnotherActor".into();
    changed.runtime.character.name = changed.durable.character.name.clone();
    cases.push(changed);
    let mut changed = handoff.clone();
    let mut skill: serde_json::Value =
        serde_json::from_str(&changed.durable.skill_states_json[0]).unwrap();
    skill[SKILL_CLOCK_METADATA_KEY]["unexpectedField"] = serde_json::json!(1);
    changed.durable.skill_states_json[0] = skill.to_string();
    cases.push(changed);
    for field in ["remainingMs", "ready", "clockEpoch", "collectDeadlineMs"] {
        let mut changed = handoff.clone();
        let mut systems = stage5_value(&changed.runtime).unwrap();
        systems["refine"][field] = match field {
            "remainingMs" => serde_json::json!(0),
            "ready" => serde_json::json!(true),
            "clockEpoch" => serde_json::json!("forged-clock"),
            _ => serde_json::json!(0),
        };
        changed.runtime.stage5_systems_json = Some(systems.to_string());
        cases.push(changed);
    }
    for changed in cases {
        let mut target = select(&config);
        let tick = runtime_tick(target.app.world());
        assert!(target.resume_character_handoff(&changed).is_err());
        assert!(target.active_identity().is_none());
        assert_eq!(runtime_tick(target.app.world()), tick);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    let mut overflow = handoff.clone();
    for save in [&mut overflow.durable, &mut overflow.runtime] {
        let mut systems = stage5_value(save).unwrap();
        systems["refine"]["remainingMs"] = serde_json::json!(u64::MAX);
        save.stage5_systems_json = Some(systems.to_string());
    }
    // A genuine bound row can still fail the target's clock arithmetic. It must
    // leave no active identity or altered tick, and cannot write the authority.
    config
        .commit_account_store_transaction(&["demo".into()], |store| {
            store
                .accounts
                .get_mut("demo")
                .unwrap()
                .saves
                .insert(0, overflow.durable.clone());
            Ok(())
        })
        .unwrap();
    let overflow_bytes = std::fs::read(&path).unwrap();
    let mut target = select(&config);
    let tick = runtime_tick(target.app.world());
    assert!(target.resume_character_handoff(&overflow).is_err());
    assert!(target.active_identity().is_none());
    assert_eq!(runtime_tick(target.app.world()), tick);
    assert_eq!(std::fs::read(&path).unwrap(), overflow_bytes);
}

#[test]
fn handoff_prepare_returns_post_settlement_guild_checkpoint_not_the_callback_preview() {
    let (config, path, mut source) = source();
    source.enable_shared_guild_authority();
    crate::runtime::leveling::apply_level_change(source.app.world_mut(), 22);
    source
        .app
        .world_mut()
        .resource_mut::<PlayerRuntimeResource>()
        .gold = 1_000_000;
    let horn = mir2_game_data::crystal_item_by_name("WoomaHorn").unwrap();
    crate::runtime::inventory::add_or_increment_item(
        source.app.world_mut(),
        crate::config::ItemContainer::Bag1,
        &crate::runtime::items::crystal_item_key_for_template(&horn),
        &horn.name,
        "",
        20,
        2,
        u16::from(horn.weight),
    );
    source.save_active_character().unwrap();
    source.grant_shared_guild_creation_from_npc();
    source.submit_shared_guild_name("HandoffGuild").unwrap();
    let before = source.begin_guild_experience_command(false).unwrap();
    assert!(before.is_some());
    crate::runtime::npc_script::crystal_npc_give_exp(source.app.world_mut(), &["200"]);
    assert!(
        !crate::runtime::shared_guild_experience::snapshot(source.app.world())
            .pending
            .is_empty()
    );
    let handoff = source.prepare_character_handoff().unwrap();
    let durable = actual(&path);
    assert!(same_record(&handoff.durable, &durable).unwrap());
    assert!(handoff.durable.guild_experience_journal.pending.is_empty());
    assert!(!handoff
        .durable
        .guild_experience_journal
        .event_payloads
        .is_empty());
    assert!(same_record(
        &config.account_store.lock().unwrap().accounts["demo"].saves[&0],
        &durable
    )
    .unwrap());
}

#[test]
fn handoff_commitment_allows_only_skill_json_representation_changes() {
    let (config, path, mut source) = source();
    let handoff = source.prepare_character_handoff().unwrap();
    let mut target = select(&config);
    let commitment = target.resume_character_handoff(&handoff).unwrap();
    let before = std::fs::read(&path).unwrap();
    let skill: serde_json::Value =
        serde_json::from_str(&commitment.runtime.skill_states_json[0]).unwrap();
    let mut fields: Vec<_> = skill.as_object().unwrap().iter().collect();
    fields.reverse();
    let reversed = format!(
        "{{\n {}\n}}",
        fields
            .iter()
            .map(|(key, value)| format!("{}: {}", serde_json::to_string(key).unwrap(), value))
            .collect::<Vec<_>>()
            .join(",\n ")
    );
    let mut equivalent = commitment.clone();
    equivalent.runtime.skill_states_json[0] = reversed;
    assert_ne!(
        equivalent.runtime.skill_states_json[0],
        commitment.runtime.skill_states_json[0]
    );
    handoff.validate_commitment(&equivalent).unwrap();
    for mutation in ["missing", "extra", "changed"] {
        let mut changed = commitment.clone();
        let mut value = skill.clone();
        match mutation {
            // delay_ms has a typed default: a typed round trip would conceal
            // its omission, while the full raw skill comparison must not.
            "missing" => {
                assert!(value.as_object_mut().unwrap().remove("delay_ms").is_some());
            }
            "extra" => value["futureSkillField"] = serde_json::json!({"mustKeep": 1}),
            _ => {
                value["cooldown_ends_at"] =
                    serde_json::json!(value["cooldown_ends_at"].as_u64().unwrap() + 1)
            }
        }
        changed.runtime.skill_states_json[0] = value.to_string();
        assert!(
            handoff.validate_commitment(&changed).is_err(),
            "{mutation} skill field must be rejected"
        );
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn handoff_refuses_to_drop_a_bound_unknown_refine_field_in_live_or_replica_install() {
    let (config, path, mut source) = source();
    install_foreign_refine(&mut source);
    let mut handoff = source.prepare_character_handoff().unwrap();
    for save in [&mut handoff.durable, &mut handoff.runtime] {
        let mut systems = stage5_value(save).unwrap();
        systems["refine"]["futureCustodyField"] = serde_json::json!({"mustKeep": [1, 2, 3]});
        save.stage5_systems_json = Some(systems.to_string());
    }
    // Bind the complete future image to the real authority. Rejection must be
    // caused by a lossy target representation, not an unequal Source baseline.
    config
        .commit_account_store_transaction(&["demo".into()], |store| {
            store
                .accounts
                .get_mut("demo")
                .unwrap()
                .saves
                .insert(0, handoff.durable.clone());
            Ok(())
        })
        .unwrap();
    handoff.validate_source_pair().unwrap();
    assert!(same_record(&actual(&path), &handoff.durable).unwrap());
    let before = std::fs::read(&path).unwrap();
    let mut target = select(&config);
    let tick = runtime_tick(target.app.world());
    let error = target.resume_character_handoff(&handoff).unwrap_err();
    assert!(error.contains("complete Stage5 contents"), "{error}");
    assert!(target.active_identity().is_none());
    assert_eq!(runtime_tick(target.app.world()), tick);
    assert_eq!(std::fs::read(&path).unwrap(), before);

    let isolated = config.fork_for_replica_apply().unwrap();
    let image = serde_json::to_value(&*isolated.account_store.lock().unwrap()).unwrap();
    let mut replica = select(&isolated);
    let tick = runtime_tick(replica.app.world());
    let error = replica
        .apply_character_handoff_replica(&handoff)
        .unwrap_err();
    assert!(error.contains("complete Stage5 contents"), "{error}");
    assert!(replica.active_identity().is_none());
    assert_eq!(runtime_tick(replica.app.world()), tick);
    assert_eq!(
        serde_json::to_value(&*isolated.account_store.lock().unwrap()).unwrap(),
        image
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
