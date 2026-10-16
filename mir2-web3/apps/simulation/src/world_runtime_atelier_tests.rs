//! Real WorldRuntime -> public Session -> nested Source completion. Only the
//! initial independent File fixture is controlled; every action uses the same
//! SelectNpcDialog command as the ordinary Gateway path.
use super::{InProcessWorldRuntime, WorldCommand, WorldRuntime};
use crate::{AccountStore, CharacterSaveRecord, SimulationSession, Stage5SystemsState};
use mir2_protocol::ServerPacket;
use std::{fs, path::Path};

fn durable(path: &Path) -> CharacterSaveRecord {
    let store: AccountStore = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    store.accounts["demo"].saves[&0].clone()
}

fn systems(save: &CharacterSaveRecord) -> Stage5SystemsState {
    serde_json::from_str(save.stage5_systems_json.as_deref().unwrap()).unwrap()
}

fn displayed_link(runtime: &InProcessWorldRuntime, prefix: &str) -> String {
    runtime
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .links
        .into_iter()
        .find(|link| link.target.starts_with(prefix))
        .unwrap_or_else(|| panic!("missing actual WorldRuntime link {prefix}"))
        .target
}

fn assert_final_page(runtime: &InProcessWorldRuntime, path: &Path, packets: &[ServerPacket]) {
    let snapshot = runtime.world_snapshot();
    assert_eq!(snapshot.map_file_name.as_deref(), Some("0"));
    let dialog = snapshot.active_npc_dialog.unwrap();
    assert_eq!(dialog.npc_object_id, 33);
    assert_eq!(dialog.title, "Crafting workshop");
    assert!(dialog.input.is_none());
    let live = runtime.active_character_checkpoint().unwrap();
    let saved = durable(path);
    assert_eq!(saved.revision, live.revision);
    assert_eq!(saved.character, live.character);
    assert_eq!(saved.gold, live.gold);
    assert_eq!(saved.inventory_items_json, live.inventory_items_json);
    assert_eq!(saved.equipment_items_json, live.equipment_items_json);
    assert_eq!(systems(&saved).appearance, systems(&live).appearance);
    assert_eq!(saved.npc_purchase_journal, live.npc_purchase_journal);
    let owner = runtime.active_identity().unwrap();
    assert_eq!(owner.account_id, "demo");
    assert_eq!(owner.character_index, saved.character.index);
    assert_eq!(owner.character_name, saved.character.name);

    let mut expected = vec![dialog.title];
    expected.extend(dialog.body);
    for link in dialog.links {
        expected.push(format!("<{}/{}>", link.text, link.target));
        if [
            "@atelier:v1:craft:",
            "@atelier:v1:wear:",
            "@atelier:v1:clear:",
        ]
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
fn atelier_world_command_craft_wear_clear_accepts_each_first_displayed_quote_after_outer_source() {
    let (session, path) = SimulationSession::atelier_world_runtime_test_fixture();
    let mut runtime = InProcessWorldRuntime { session };
    let initial = runtime.active_character_checkpoint().unwrap();
    let actor = initial.npc_purchase_journal.as_ref().unwrap().actor;
    let initial_stats =
        serde_json::to_value(runtime.world_snapshot().player_crystal_stats).unwrap();
    let equipment = initial.equipment_items_json;

    for target in ["@atelier:v1:home", "@atelier:v1:recipe:traveler"] {
        let packets = runtime
            .execute(WorldCommand::SelectNpcDialog {
                target: target.into(),
            })
            .unwrap();
        assert_final_page(&runtime, &path, &packets);
    }

    let mut first_craft = None;
    for (index, prefix, selected) in [
        (0, "@atelier:v1:craft:traveler:", None),
        (1, "@atelier:v1:wear:traveler:", Some("traveler")),
        (2, "@atelier:v1:clear:", None),
    ] {
        let target = displayed_link(&runtime, prefix);
        let before = runtime.active_character_checkpoint().unwrap();
        assert_eq!(
            target.rsplit(':').nth(1).unwrap().parse::<u64>(),
            Ok(before.revision)
        );
        if index == 0 {
            first_craft = Some(target.clone());
        }
        // No retry, refreshed quote, or private economic call may repair the
        // displayed target: its very first ordinary command must succeed.
        let packets = runtime
            .execute(WorldCommand::SelectNpcDialog { target })
            .unwrap();
        let saved = durable(&path);
        assert_eq!(saved.revision, before.revision + 2);
        assert_eq!(saved.gold, initial.gold - 500);
        assert_eq!(saved.equipment_items_json, equipment);
        assert_eq!(saved.npc_purchase_journal.as_ref().unwrap().actor, actor);
        let appearance = systems(&saved).appearance;
        assert!(appearance.owned_fashions.contains("traveler"));
        assert_eq!(appearance.selected_fashion.as_deref(), selected);
        assert_eq!(appearance.atelier_receipts.len(), index + 1);
        assert_eq!(
            serde_json::to_value(runtime.world_snapshot().player_crystal_stats).unwrap(),
            initial_stats
        );
        if index == 0 {
            assert!(packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 500 })));
            assert!(saved.inventory_items_json.is_empty());
        } else {
            assert!(packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::PlayerUpdate { .. })));
            assert!(!packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. }
            )));
        }
        assert_final_page(&runtime, &path, &packets);
    }

    let bytes = fs::read(&path).unwrap();
    let before = serde_json::to_value(runtime.active_character_checkpoint().unwrap()).unwrap();
    let packets = runtime
        .execute(WorldCommand::SelectNpcDialog {
            target: first_craft.unwrap(),
        })
        .unwrap();
    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. }
    )));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(
        serde_json::to_value(runtime.active_character_checkpoint().unwrap()).unwrap(),
        before
    );
}
