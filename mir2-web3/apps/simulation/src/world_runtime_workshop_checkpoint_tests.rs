//! A real complete File checkpoint must refresh only the authenticated current
//! display. These tests never retry or re-execute an economic action to refresh.
use super::{InProcessWorldRuntime, WorldCommand, WorldRuntime};
use crate::{AccountStore, AccountStoreTransactionFault, CharacterSaveRecord, SimulationSession};
use mir2_protocol::ServerPacket;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

fn store(path: &Path) -> AccountStore {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn saved(path: &Path) -> CharacterSaveRecord {
    store(path).accounts["demo"].saves[&0].clone()
}

fn link(runtime: &InProcessWorldRuntime, prefix: &str) -> String {
    runtime
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .links
        .into_iter()
        .find(|link| link.target.starts_with(prefix))
        .unwrap()
        .target
}

fn choose(runtime: &mut InProcessWorldRuntime, prefix: &str) -> Vec<ServerPacket> {
    let target = link(runtime, prefix);
    runtime
        .execute(WorldCommand::SelectNpcDialog { target })
        .unwrap()
}

fn checkpoint_preserves_economy(runtime: &mut InProcessWorldRuntime, path: &Path) {
    let before = saved(path);
    let registry = Sha256::digest(serde_json::to_vec(&store(path).sealed_stones).unwrap());
    runtime.save_active_character().unwrap();
    let after = saved(path);
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.character, before.character);
    assert_eq!(after.gold, before.gold);
    assert_eq!(after.inventory_items_json, before.inventory_items_json);
    assert_eq!(after.belt_items_json, before.belt_items_json);
    assert_eq!(after.equipment_items_json, before.equipment_items_json);
    assert_eq!(after.npc_purchase_journal, before.npc_purchase_journal);
    assert_eq!(after.position, before.position);
    assert_eq!(after.map_file_name, before.map_file_name);
    assert_eq!(
        Sha256::digest(serde_json::to_vec(&store(path).sealed_stones).unwrap()),
        registry
    );
    let dialog = runtime.world_snapshot().active_npc_dialog.unwrap();
    for option in dialog.links {
        if [
            "@stone:v1:seal:",
            "@stone:v1:appraise:",
            "@stone:v1:cut:",
            "@atelier:v1:craft:",
            "@atelier:v1:wear:",
            "@atelier:v1:clear:",
        ]
        .iter()
        .any(|prefix| option.target.starts_with(prefix))
        {
            assert_eq!(
                option.target.rsplit(':').nth(1).unwrap().parse::<u64>(),
                Ok(after.revision)
            );
        }
    }
}

#[test]
fn workshop_checkpoint_file_stone_quotes_survive_additional_full_saves_without_reexecution() {
    let (session, path) = SimulationSession::stone_world_runtime_test_fixture();
    let mut runtime = InProcessWorldRuntime { session };
    for prefix in ["@stone:v1:home", "@stone:v1:list:ore:", "@stone:v1:ore:"] {
        choose(&mut runtime, prefix);
    }
    let initial = saved(&path);
    for (prefix, fee) in [
        ("@stone:v1:seal:", 0),
        ("@stone:v1:appraise:", 100),
        ("@stone:v1:cut:", 1500),
    ] {
        checkpoint_preserves_economy(&mut runtime, &path);
        let before = saved(&path);
        let packets = choose(&mut runtime, prefix);
        assert_eq!(saved(&path).revision, before.revision + 2);
        assert_eq!(saved(&path).gold, before.gold - fee);
        if fee > 0 {
            assert_eq!(
                packets
                    .iter()
                    .filter(|packet| matches!(packet,
                ServerPacket::LoseGold { gold } if *gold == fee))
                    .count(),
                1
            );
        }
        checkpoint_preserves_economy(&mut runtime, &path);
    }
    assert_eq!(saved(&path).gold, initial.gold - 1600);
}

#[test]
fn workshop_checkpoint_file_atelier_wear_clear_quotes_survive_additional_full_saves() {
    let (session, path) = SimulationSession::atelier_world_runtime_test_fixture();
    let mut runtime = InProcessWorldRuntime { session };
    choose(&mut runtime, "@atelier:v1:home");
    choose(&mut runtime, "@atelier:v1:recipe:traveler");
    let stats = serde_json::to_value(runtime.world_snapshot().player_crystal_stats).unwrap();
    for prefix in [
        "@atelier:v1:craft:traveler:",
        "@atelier:v1:wear:traveler:",
        "@atelier:v1:clear:",
    ] {
        checkpoint_preserves_economy(&mut runtime, &path);
        let before = saved(&path);
        choose(&mut runtime, prefix);
        assert_eq!(saved(&path).revision, before.revision + 2);
        checkpoint_preserves_economy(&mut runtime, &path);
        assert_eq!(
            serde_json::to_value(runtime.world_snapshot().player_crystal_stats).unwrap(),
            stats
        );
    }
}

#[test]
fn workshop_checkpoint_file_failure_invalidates_quotes_without_a_later_resurrection() {
    for fault in [
        AccountStoreTransactionFault::BeforeFileRename,
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    ] {
        let (session, path) = SimulationSession::atelier_world_runtime_test_fixture();
        let mut runtime = InProcessWorldRuntime { session };
        choose(&mut runtime, "@atelier:v1:home");
        choose(&mut runtime, "@atelier:v1:recipe:traveler");
        let old_quote = link(&runtime, "@atelier:v1:craft:traveler:");
        let before = saved(&path);
        let bytes = fs::read(&path).unwrap();
        runtime
            .session
            .inject_workshop_checkpoint_fault_for_test(fault);
        assert!(runtime.save_active_character().is_err());
        assert!(runtime.world_snapshot().active_npc_dialog.is_none());
        let after = saved(&path);
        let failed_bytes = fs::read(&path).unwrap();
        assert_eq!(after.gold, before.gold);
        assert_eq!(after.inventory_items_json, before.inventory_items_json);
        // A known failed save may be explicitly retried. An Unknown one stays
        // fenced. Neither may bring back an old page or execute its quote.
        let _ = runtime.save_active_character();
        assert!(runtime.world_snapshot().active_npc_dialog.is_none());
        let settled = fs::read(&path).unwrap();
        let _ = runtime.execute(WorldCommand::SelectNpcDialog { target: old_quote });
        assert_eq!(fs::read(&path).unwrap(), settled);
        assert_eq!(saved(&path).gold, before.gold);
        assert_eq!(
            saved(&path).inventory_items_json,
            before.inventory_items_json
        );
        if after.revision == before.revision {
            assert_eq!(
                serde_json::to_value(after).unwrap(),
                serde_json::to_value(before).unwrap()
            );
            assert_eq!(bytes, failed_bytes);
        } else {
            assert_eq!(after.revision, before.revision + 1);
            assert_eq!(failed_bytes, settled);
        }
    }
}

#[test]
fn workshop_checkpoint_after_exit_cannot_reopen_a_previously_published_page() {
    let (session, path) = SimulationSession::stone_world_runtime_test_fixture();
    let mut runtime = InProcessWorldRuntime { session };
    choose(&mut runtime, "@stone:v1:home");
    runtime
        .execute(WorldCommand::SelectNpcDialog {
            target: "@Exit".into(),
        })
        .unwrap();
    let gold = saved(&path).gold;
    runtime.save_active_character().unwrap();
    assert!(runtime.world_snapshot().active_npc_dialog.is_none());
    assert_eq!(saved(&path).gold, gold);
}
