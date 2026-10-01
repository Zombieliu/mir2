//! Isolated in-memory fixtures, not gameplay completion or save-edit evidence.
//! Cost agreement crosses the legacy alias, public UI snapshot, ordinary Magic
//! packet, and the shared Zone's existing Crystal attack profile.

use mir2_game_data::{crystal_item_by_name, crystal_magic_by_spell};
use mir2_protocol::{
    ClientPacket, MirClass, MirDirection, MirGender, MirGridType, ServerPacket, Spell,
};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, SimulationConfig, SimulationSession,
};
use serde_json::json;

fn skill_json(key: &str, level: u8) -> String {
    json!({
        "key": key, "name": key, "description": "isolated mana projection fixture",
        "level": level, "experience": 0, "hotkey": 0,
        "cooldown_ticks": 1, "delay_ms": 1, "cooldown_ends_at": 0, "cast_time_ms": 0
    })
    .to_string()
}

fn fixture(skill: &str, level: u8, mp: i32, book: bool) -> SimulationSession {
    let mut config = SimulationConfig::default();
    config.visible_monsters.clear();
    let character = CharacterRecord {
        index: 0,
        name: "ManaFixture".into(),
        level: 100,
        class: MirClass::Taoist,
        gender: MirGender::Male,
    };
    let mut save = CharacterSaveRecord::new(character.clone());
    save.map_file_name = config.map.file_name.clone();
    save.map_title = config.map.title.clone();
    save.position = config.spawn.clone();
    save.hp = 20;
    save.mp = mp;
    save.skill_states_json = vec![skill_json(skill, level)];
    if book {
        let template = crystal_item_by_name("Healing").expect("ordinary Healing book");
        save.inventory_items_json = vec![json!({
            "key": format!("crystal-item-{}", template.item_index),
            "name": template.name, "icon": template.image,
            "slot": 0, "unique_id": 98765, "container": "bag1", "quantity": 1,
            "description": "ordinary book fixture", "durability_current": null,
            "durability_max": null, "weight": template.weight, "equip_slot": null,
            "attack": 0, "defence": 0, "heal_hp": 0, "heal_mp": 0
        }).to_string()];
    }
    let mut account = AccountRecord::empty();
    account.characters.push(character);
    account.saves.insert(0, save);
    config
        .account_store
        .lock()
        .expect("in-memory account store")
        .accounts
        .insert("mana-fixture".into(), account);
    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "mana-fixture".into(),
        password: "demo".into(),
    });
    assert!(login.iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(start.iter().any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    session
}

fn expected_cost(spell: &str, level: u8) -> i32 {
    let metadata = crystal_magic_by_spell(spell).expect("canonical Crystal metadata");
    i32::from(metadata.base_cost) + i32::from(metadata.level_cost) * i32::from(level)
}

fn healing_packet(session: &SimulationSession) -> ClientPacket {
    let snapshot = session.world_snapshot();
    let own = snapshot.player_object_id.expect("own public object ID");
    let actor = snapshot.entities.iter().find(|entity| entity.object_id == own).unwrap();
    ClientPacket::Magic {
        object_id: own,
        spell: Spell::Healing,
        direction: MirDirection::Down,
        target_id: own,
        location: mir2_protocol::Point { x: actor.x, y: actor.y },
        spell_target_lock: true,
    }
}

fn accepted_healing(packets: &[ServerPacket]) -> bool {
    packets.iter().any(|packet| matches!(packet,
        ServerPacket::Magic { spell: Spell::Healing, cast: true, .. }))
}

#[test]
fn aliases_and_manifest_skills_project_the_zone_formula_at_levels_zero_to_three() {
    for (key, spell) in [
        ("minor-heal", Spell::Healing),
        ("battle-focus", Spell::Fury),
        ("fireball", Spell::FireBall),
        ("summonskeleton", Spell::SummonSkeleton),
    ] {
        for level in 0..=3 {
            let session = fixture(key, level, 300, false);
            let snapshot = session.world_snapshot();
            let skill = snapshot.known_skills.iter().find(|skill| skill.key == key).unwrap();
            let cost = expected_cost(&format!("{spell:?}"), level);
            let (zone_level, _, zone_cost, _) = session.zone_magic_attack_profile(spell).unwrap();
            assert_eq!(zone_level, level, "{key} level {level}");
            assert_eq!(zone_cost, cost, "existing shared Zone formula");
            assert_eq!(skill.mp_cost, Some(cost as u32), "{key} level {level}: UI versus Zone");
            assert_eq!(serde_json::to_value(skill).unwrap()["mpCost"], json!(cost));
        }
    }
}

#[test]
fn ordinary_healing_packet_spends_canonical_mana_once_and_retains_cooldown() {
    for level in 0..=3 {
        let mut session = fixture("minor-heal", level, 300, false);
        let packet = healing_packet(&session);
        let before = session.world_snapshot().player_mp.unwrap();
        let cast = session.handle_packet(packet.clone());
        assert!(accepted_healing(&cast), "Healing level {level}: {cast:?}");
        let after = session.world_snapshot().player_mp.unwrap();
        assert_eq!(before - after, expected_cost("Healing", level), "Healing level {level}");
        let repeated = session.handle_packet(packet);
        assert!(!accepted_healing(&repeated), "an immediate repeat remains blocked");
        assert_eq!(session.world_snapshot().player_mp, Some(after));
    }
}

#[test]
fn personal_healing_preflight_matches_exact_canonical_mana_boundary() {
    for level in 0..=3 {
        let cost = expected_cost("Healing", level);
        let mut sufficient = fixture("minor-heal", level, cost, false);
        let cast = sufficient.handle_packet(healing_packet(&sufficient));
        assert!(accepted_healing(&cast), "exact mana at Healing level {level}: {cast:?}");
        assert_eq!(sufficient.world_snapshot().player_mp, Some(0));
        let mut insufficient = fixture("minor-heal", level, cost - 1, false);
        let before = insufficient.world_snapshot().player_mp;
        let cast = insufficient.handle_packet(healing_packet(&insufficient));
        assert!(!accepted_healing(&cast), "one MP short at Healing level {level}");
        assert_eq!(insufficient.world_snapshot().player_mp, before);
    }
}

#[test]
fn ordinary_book_new_magic_and_known_skill_report_the_same_cost() {
    let mut session = fixture("unknown-cost-fixture", 0, 100, true);
    let before = session.world_snapshot();
    let book = before.inventory_items.iter().find(|item| item.name == "Healing").unwrap();
    let learned = session.handle_packet(ClientPacket::UseItem {
        unique_id: book.unique_id,
        grid: MirGridType::Inventory,
    });
    let magic = learned.iter().find_map(|packet| match packet {
        ServerPacket::NewMagic { magic, hero: false } if magic.spell == Spell::Healing => Some(magic),
        _ => None,
    }).expect("ordinary owned-book NewMagic receipt");
    let wire_cost = u32::from(magic.base_cost) + u32::from(magic.level_cost) * u32::from(magic.level);
    let snapshot = session.world_snapshot();
    let skill = snapshot.known_skills.iter().find(|skill| skill.spell.as_deref() == Some("Healing")).unwrap();
    assert_eq!(skill.level, 0);
    assert_eq!(skill.mp_cost, Some(wire_cost));
    assert!(!snapshot.inventory_items.iter().any(|item| item.unique_id == book.unique_id),
        "the real owned book must be consumed");
}

#[test]
fn unknown_cost_is_absent_and_cannot_be_cast_as_free_healing() {
    let mut session = fixture("unknown-cost-fixture", 0, 0, false);
    let before = session.world_snapshot();
    let skill = before.known_skills.iter().find(|skill| skill.key == "unknown-cost-fixture").unwrap();
    assert_eq!(skill.mp_cost, None);
    assert_eq!(session.zone_magic_attack_profile(Spell::Healing), None);
    assert!(!accepted_healing(&session.handle_packet(healing_packet(&session))));
    assert_eq!(session.world_snapshot().player_mp, Some(0));
}
