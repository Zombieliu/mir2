//! Ordinary Gateway-shaped commands through the real WorldRuntime and nested
//! public Session Source. Only the independent initial File fixture is controlled.
use super::{InProcessWorldRuntime, WorldCommand, WorldRuntime};
use crate::{AccountStore, CharacterSaveRecord, SimulationSession};
use mir2_production::{StoneLifecycle, StoneMaterial};
use mir2_protocol::ServerPacket;
use std::{fs, path::Path};

fn durable(path: &Path) -> CharacterSaveRecord {
    let store: AccountStore = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    store.accounts["demo"].saves[&0].clone()
}

fn displayed_link(runtime: &InProcessWorldRuntime, prefix: &str) -> String {
    runtime
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .links
        .into_iter()
        .find(|link| link.target.starts_with(prefix))
        .unwrap_or_else(|| panic!("missing actual WorldRuntime stone link {prefix}"))
        .target
}

fn assert_final_page(runtime: &InProcessWorldRuntime, path: &Path, packets: &[ServerPacket]) {
    let snapshot = runtime.world_snapshot();
    assert_eq!(snapshot.map_file_name.as_deref(), Some("0"));
    let dialog = snapshot.active_npc_dialog.unwrap();
    assert_eq!(dialog.title, "Stone workshop");
    assert!(dialog.npc_object_id != 0);
    let live = runtime.active_character_checkpoint().unwrap();
    let saved = durable(path);
    assert_eq!(saved.revision, live.revision);
    assert_eq!(saved.gold, live.gold);
    assert_eq!(saved.inventory_items_json, live.inventory_items_json);
    assert_eq!(saved.belt_items_json, live.belt_items_json);
    assert_eq!(saved.npc_purchase_journal, live.npc_purchase_journal);
    assert_eq!(saved.character, live.character);
    let owner = runtime.active_identity().unwrap();
    assert_eq!(owner.account_id, "demo");
    assert_eq!(owner.character_index, saved.character.index);
    assert_eq!(owner.character_name, saved.character.name);

    let mut expected = vec![dialog.title];
    expected.extend(dialog.body);
    for link in dialog.links {
        expected.push(format!("<{}/{}>", link.text, link.target));
        if ["@stone:v1:seal:", "@stone:v1:appraise:", "@stone:v1:cut:"]
            .iter()
            .any(|prefix| link.target.starts_with(prefix))
        {
            assert_eq!(
                link.target.rsplit(':').nth(1).unwrap().parse::<u64>(),
                Ok(saved.revision)
            );
        }
    }
    expected.push(dialog.footer);
    let pages: Vec<_> = packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::NPCResponse { page } => Some(page),
            _ => None,
        })
        .collect();
    assert_eq!(pages, vec![&expected]);
}

#[test]
fn stone_world_command_file_source_accepts_first_seal_appraise_cut_quotes_after_outer_source() {
    let (session, path) = SimulationSession::stone_world_runtime_test_fixture();
    let mut runtime = InProcessWorldRuntime { session };
    let initial = runtime.active_character_checkpoint().unwrap();
    let actor = initial.npc_purchase_journal.as_ref().unwrap().actor;
    for prefix in ["@stone:v1:home", "@stone:v1:list:ore:", "@stone:v1:ore:"] {
        let target = displayed_link(&runtime, prefix);
        let packets = runtime
            .execute(WorldCommand::SelectNpcDialog { target })
            .unwrap();
        assert_final_page(&runtime, &path, &packets);
    }

    let mut old_seal = None;
    let mut serial = None;
    for (index, prefix, fee) in [
        (0, "@stone:v1:seal:", 0),
        (1, "@stone:v1:appraise:", 100),
        (2, "@stone:v1:cut:", 1500),
    ] {
        let target = displayed_link(&runtime, prefix);
        let before = runtime.active_character_checkpoint().unwrap();
        assert_eq!(
            target.rsplit(':').nth(1).unwrap().parse::<u64>(),
            Ok(before.revision)
        );
        if index == 0 {
            old_seal = Some(target.clone());
        }
        // Every link must work on its first ordinary command, with no private
        // economic call, regenerated quote or retry between these three steps.
        let packets = runtime
            .execute(WorldCommand::SelectNpcDialog { target })
            .unwrap();
        let saved = durable(&path);
        assert_eq!(saved.revision, before.revision + 2);
        assert_eq!(saved.gold, before.gold - fee);
        assert_eq!(saved.npc_purchase_journal.as_ref().unwrap().actor, actor);
        assert_eq!(saved.inventory_items_json.len(), 1);
        let item: serde_json::Value = serde_json::from_str(&saved.inventory_items_json[0]).unwrap();
        assert_eq!(item["unique_id"].as_u64(), Some(120_000_001));
        assert_eq!(item["slot"].as_u64(), Some(0));
        // Inspect only the public physical carrier and lifecycle, never the
        // sealed result before cutting.
        if index == 0 {
            serial = item["stone_serial"].as_u64();
            assert!(serial.is_some());
        }
        let store: AccountStore = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let lifecycle = store
            .sealed_stones
            .record(serial.unwrap())
            .unwrap()
            .lifecycle();
        assert_eq!(
            lifecycle,
            [
                StoneLifecycle::Sealed,
                StoneLifecycle::Appraised,
                StoneLifecycle::Cut
            ][index]
        );
        if index == 2 {
            assert!(item["stone_serial"].is_null());
            assert!(packets.iter().any(|packet| matches!(packet,
                ServerPacket::GainedItem { item } if item.unique_id == 120_000_001
                    && StoneMaterial::ALL.iter().any(|material| material.template_id() == item.item_index))));
        }
        if fee == 0 {
            assert!(!packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::LoseGold { .. })));
        } else {
            assert_eq!(
                packets
                    .iter()
                    .filter(|packet| matches!(packet,
                ServerPacket::LoseGold { gold } if *gold == fee))
                    .count(),
                1
            );
        }
        assert_final_page(&runtime, &path, &packets);
    }
    assert_eq!(durable(&path).gold, initial.gold - 1600);

    let bytes = fs::read(&path).unwrap();
    let before = runtime.active_character_checkpoint().unwrap();
    let packets = runtime
        .execute(WorldCommand::SelectNpcDialog {
            target: old_seal.unwrap(),
        })
        .unwrap();
    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. }
    )));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(runtime.active_character_checkpoint().unwrap(), before);
}
