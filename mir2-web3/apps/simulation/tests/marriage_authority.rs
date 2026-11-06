use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::{
    AccountRecord, CharacterRecord, EquipmentSlot, SharedMarriageMutation, SimulationConfig,
    SimulationSession, Stage5FriendIdentity,
};
use serde_json::{json, Value};
fn id(account: &str, index: i32) -> Stage5FriendIdentity {
    Stage5FriendIdentity {
        account_id: account.into(),
        character_index: index,
    }
}
fn fixture() -> SimulationConfig {
    let config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        let owner = store.accounts.get_mut("demo").unwrap();
        owner.characters[0].level = 20;
        owner.saves.get_mut(&0).unwrap().character.level = 20;
        store.accounts.insert(
            "peer".into(),
            AccountRecord::new(CharacterRecord {
                index: 1,
                name: "Partner".into(),
                level: 20,
                class: MirClass::Warrior,
                gender: MirGender::Female,
            }),
        );
        for (account, index, partner) in [("demo", 0, 1), ("peer", 1, 0)] {
            let save = store
                .accounts
                .get_mut(account)
                .unwrap()
                .saves
                .get_mut(&index)
                .unwrap();
            save.equipment_items_json.push(json!({"key":"crystal-item-416","slot":EquipmentSlot::RingLeft,
                "name":"GoldRing","icon":0,"shape":0,"description":"","durability_current":1000,"durability_max":1000,
                "attack":0,"defence":0,"user_item_unique_id":33000+index,"user_item_metadata":{"item_index":416,"wedding_ring":partner,"refined_value":9}}).to_string());
        }
    }
    config
        .commit_shared_marriage_mutation(
            &id("peer", 1),
            SharedMarriageMutation::TogglePermission,
            0,
        )
        .unwrap();
    config
}
fn enter(config: SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(session.active_identity().is_some());
    session
}
fn ring(config: &SimulationConfig, account: &str, index: i32) -> Value {
    config.account_store.lock().unwrap().accounts[account].saves[&index]
        .equipment_items_json
        .iter()
        .map(|v| serde_json::from_str::<Value>(v).unwrap())
        .find(|v| v["slot"] == json!(EquipmentSlot::RingLeft))
        .unwrap()
}
#[test]
fn divorce_clears_both_ring_bindings_and_stale_save_cannot_restore_them() {
    let config = fixture();
    let mut session = enter(config.clone());
    config
        .commit_shared_marriage_mutation(
            &id("peer", 1),
            SharedMarriageMutation::Accept {
                partner: id("demo", 0),
            },
            100,
        )
        .unwrap();
    session.refresh_shared_marriage(&Default::default(), 100, true);
    config
        .commit_shared_marriage_mutation(
            &id("peer", 1),
            SharedMarriageMutation::Divorce {
                partner: id("demo", 0),
            },
            200,
        )
        .unwrap();
    session.save_active_character().unwrap();
    for (account, index) in [("demo", 0), ("peer", 1)] {
        let item = ring(&config, account, index);
        assert_eq!(item["user_item_metadata"]["wedding_ring"], -1);
        assert_eq!(item["user_item_metadata"]["refined_value"], 9);
        assert_eq!(item["user_item_unique_id"], 33000 + index);
        assert!(config
            .shared_marriage_profile_for(&id(account, index))
            .unwrap()
            .relationship
            .partner_identity
            .is_none());
    }
    let packets = session.refresh_shared_marriage(&Default::default(), 200, false);
    assert!(packets.iter().any(|p|matches!(p,ServerPacket::RefreshItem{item} if item.unique_id==33000 && item.wedding_ring==-1)),"{packets:?}");
}
#[test]
fn marriage_reciprocity_cooldown_permission_and_live_level_are_authoritative() {
    let config = fixture();
    config
        .commit_shared_marriage_mutation(
            &id("peer", 1),
            SharedMarriageMutation::TogglePermission,
            100,
        )
        .unwrap();
    assert!(config
        .commit_shared_marriage_mutation(
            &id("peer", 1),
            SharedMarriageMutation::Accept {
                partner: id("demo", 0)
            },
            100
        )
        .is_err());
    config
        .commit_shared_marriage_mutation(
            &id("peer", 1),
            SharedMarriageMutation::TogglePermission,
            100,
        )
        .unwrap();
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("peer")
        .unwrap()
        .characters[0]
        .level = 1;
    config
        .commit_shared_marriage_mutation_with_live_levels(
            &id("peer", 1),
            SharedMarriageMutation::Accept {
                partner: id("demo", 0),
            },
            100,
            Some((20, 20)),
        )
        .unwrap();
    config
        .commit_shared_marriage_mutation(
            &id("demo", 0),
            SharedMarriageMutation::TogglePermission,
            100,
        )
        .unwrap();
    assert!(
        config
            .shared_marriage_profile_for(&id("demo", 0))
            .unwrap()
            .relationship
            .allow_lover_recall
    );
    config
        .commit_shared_marriage_mutation(
            &id("peer", 1),
            SharedMarriageMutation::Divorce {
                partner: id("demo", 0),
            },
            200,
        )
        .unwrap();
    assert!(config
        .commit_shared_marriage_mutation_with_live_levels(
            &id("peer", 1),
            SharedMarriageMutation::Accept {
                partner: id("demo", 0)
            },
            201,
            Some((20, 20))
        )
        .is_err());
    config
        .commit_shared_marriage_mutation_with_live_levels(
            &id("peer", 1),
            SharedMarriageMutation::Accept {
                partner: id("demo", 0),
            },
            7 * 24 * 60 * 60 * 1000 + 201,
            Some((20, 20)),
        )
        .unwrap();
}
