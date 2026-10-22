//! Isolated real Session/File transactions. These fixtures do not claim natural
//! acquisition or GUI acceptance; no production debug or item-grant API is added.
use super::super::components::{player_entity, PlayerVitals};
use super::*;
use crate::{AccountStore, AccountStoreTransactionFault, EquipmentSlot, SimulationConfig};
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, MirGridType, Point};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const ORE_UID: u64 = 9_000_001;

fn login(config: SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into()
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })));
    session.app.world_mut().insert_resource(Enabled);
    session
}

struct Fixture {
    config: SimulationConfig,
    session: SimulationSession,
    path: PathBuf,
}
impl Fixture {
    fn new(label: &str, template: i32) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "mir2-atelier-{label}-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("accounts.json");
        let config = SimulationConfig::default()
            .with_crystal_world_runtime()
            .with_account_store_path(&path);
        let mut session = login(config.clone());
        {
            let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
            inventory.inventory_items.clear();
            inventory.belt_items.clear();
            inventory.storage_items.clear();
            inventory.equipment_items.clear();
            inventory.reserved_item_unique_ids.clear();
            let mut item = embedded_item_state_from_template(
                &crystal_item_by_index(template).unwrap(),
                ItemContainer::Bag1,
                0,
            );
            item.unique_id = ORE_UID;
            inventory.inventory_items.push(item);
        }
        session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = 10_000;
        session.force_authoritative_player_transform(Point { x: 321, y: 266 }, MirDirection::Up);
        session.save_active_character().unwrap();
        session.begin_npc_purchase_producer().unwrap();
        session.interact(33);
        assert!(allowed(session.app.world()));
        Self {
            config,
            session,
            path,
        }
    }
    fn quote(&mut self, recipe: &str) -> Command {
        self.session
            .quote_atelier(Action::Craft {
                recipe: recipe.into(),
            })
            .unwrap()
    }
    fn disk(&self) -> AccountStore {
        serde_json::from_slice(&fs::read(&self.path).unwrap()).unwrap()
    }
    fn live(&self) -> serde_json::Value {
        serde_json::to_value(snapshot_active_character_save(self.session.app.world()).unwrap())
            .unwrap()
    }
    fn assert_checkpoint(&self) {
        let store = self.disk();
        let save = &store.accounts["demo"].saves[&0];
        let live = snapshot_active_character_save(self.session.app.world()).unwrap();
        assert_eq!(save.revision, live.revision);
        assert_eq!(save.gold, live.gold);
        assert_eq!(save.inventory_items_json, live.inventory_items_json);
        assert_eq!(
            systems(save).unwrap().appearance,
            systems(&live).unwrap().appearance
        );
        validate_receipts(save).unwrap();
    }
}

impl SimulationSession {
    // Test-only access to this isolated, ordinary File-backed fixture lets the
    // WorldRuntime unit test exercise its real private Session wrapper without
    // a production constructor or a process-wide environment toggle.
    #[cfg(test)]
    pub(crate) fn atelier_world_runtime_test_fixture() -> (Self, PathBuf) {
        let fixture = Fixture::new("world-command", 824);
        (fixture.session, fixture.path)
    }
}

fn stats(session: &SimulationSession) -> Vec<i32> {
    let stats = super::super::stats::player_stats(session.app.world());
    (0..=u8::MAX).map(|id| stats.get(id)).collect()
}

fn displayed_link(session: &SimulationSession, prefix: &str) -> String {
    session
        .app
        .world()
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .as_ref()
        .unwrap()
        .links
        .iter()
        .find(|link| link.target.starts_with(prefix))
        .unwrap_or_else(|| panic!("missing displayed atelier link {prefix}"))
        .target
        .clone()
}

fn assert_final_page(session: &SimulationSession, packets: &[ServerPacket]) {
    let dialog = session
        .app
        .world()
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .as_ref()
        .unwrap();
    let pages: Vec<_> = packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::NPCResponse { page } => Some(page),
            _ => None,
        })
        .collect();
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].first(), Some(&dialog.title));
    assert_eq!(pages[0].last(), Some(&dialog.footer));
    let revision = session.active_character_checkpoint().unwrap().revision;
    for link in &dialog.links {
        assert!(pages[0].contains(&format!("<{}/{}>", link.text, link.target)));
        assert!(super::super::npc_input::has_menu_target(
            session.app.world(),
            &link.target
        ));
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
                Ok(revision)
            );
        }
    }
}

#[test]
fn atelier_public_source_craft_wear_clear_links_bind_the_final_complete_checkpoint() {
    let mut f = Fixture::new("public-source", 824);
    let before_stats = stats(&f.session);
    let before_equipment = serde_json::to_value(
        &f.session
            .app
            .world()
            .resource::<InventoryResource>()
            .equipment_items,
    )
    .unwrap();
    let packets = f.session.select_npc_dialog_target("@atelier:v1:home");
    assert_final_page(&f.session, &packets);
    let packets = f
        .session
        .select_npc_dialog_target("@atelier:v1:recipe:traveler");
    assert_final_page(&f.session, &packets);
    let craft = displayed_link(&f.session, "@atelier:v1:craft:traveler:");
    let before_revision = f.session.active_character_checkpoint().unwrap().revision;
    let packets = f.session.select_npc_dialog_target(&craft);
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 500 })));
    // The atelier transaction and the existing outer Source publication both
    // remain real commits. No future revision is guessed to build the links.
    assert_eq!(
        f.session.active_character_checkpoint().unwrap().revision,
        before_revision + 2
    );
    assert_final_page(&f.session, &packets);
    f.assert_checkpoint();
    let gold = f.session.active_character_checkpoint().unwrap().gold;
    assert_eq!(gold, 9500);
    for (prefix, selected) in [
        ("@atelier:v1:wear:traveler:", Some("traveler")),
        ("@atelier:v1:clear:", None),
    ] {
        let target = displayed_link(&f.session, prefix);
        let before_revision = f.session.active_character_checkpoint().unwrap().revision;
        let packets = f.session.select_npc_dialog_target(&target);
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::PlayerUpdate { .. })));
        assert!(!packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. }
        )));
        let save = f.session.active_character_checkpoint().unwrap();
        assert_eq!(save.revision, before_revision + 2);
        assert_eq!(save.gold, gold);
        assert_eq!(
            systems(&save)
                .unwrap()
                .appearance
                .selected_fashion
                .as_deref(),
            selected
        );
        assert_eq!(stats(&f.session), before_stats);
        assert_eq!(
            serde_json::to_value(
                &f.session
                    .app
                    .world()
                    .resource::<InventoryResource>()
                    .equipment_items
            )
            .unwrap(),
            before_equipment
        );
        assert_final_page(&f.session, &packets);
        f.assert_checkpoint();
    }
    let bytes = fs::read(&f.path).unwrap();
    let before = f.live();
    f.session.select_npc_dialog_target(&craft);
    assert_eq!(f.live(), before);
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
}

#[test]
fn atelier_outer_source_failure_discards_the_nested_pending_page_without_retry() {
    let mut f = Fixture::new("pending-save-failure", 824);
    f.session.select_npc_dialog_target("@atelier:v1:home");
    f.session
        .select_npc_dialog_target("@atelier:v1:recipe:traveler");
    let target = displayed_link(&f.session, "@atelier:v1:craft:traveler:");
    let before_revision = f.session.active_character_checkpoint().unwrap().revision;
    // This is the real nested Source shape used by WorldRuntime: its outer
    // checkpoint remains active while the public Session method finishes None.
    let before = f.session.begin_npc_dialog_source_command(false).unwrap();
    assert!(before.is_some());
    let packets = f.session.select_npc_dialog_target(&target);
    assert!(super::super::shared_guild_experience::source_command_active(f.session.app.world()));
    assert!(!packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::NPCResponse { .. })));
    assert!(!f
        .session
        .app
        .world()
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .as_ref()
        .unwrap()
        .links
        .iter()
        .any(|link| link.target.starts_with("@atelier:v1:wear:")));
    let committed = fs::read(&f.path).unwrap();
    f.config
        .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
    assert!(f
        .session
        .finish_guild_experience_command(before, packets)
        .is_err());
    assert_eq!(fs::read(&f.path).unwrap(), committed);
    let save = f.session.active_character_checkpoint().unwrap();
    assert_eq!(save.revision, before_revision + 1);
    assert_eq!(save.gold, 9500);
    let appearance = systems(&save).unwrap().appearance;
    assert!(appearance.owned_fashions.contains("traveler"));
    assert_eq!(appearance.atelier_receipts.len(), 1);
    assert!(f
        .session
        .app
        .world()
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .is_none());
    // Later successful commands cannot resurrect or auto-execute the lost page.
    let mut packets = Vec::new();
    super::super::atelier_npc::publish_pending(&mut f.session, &mut packets);
    assert!(packets.is_empty());
    assert_eq!(fs::read(&f.path).unwrap(), committed);
    f.assert_checkpoint();
}

#[test]
fn atelier_pending_page_cannot_reopen_after_exit_replacement_or_leaving_npc_range() {
    for change in ["exit", "replace", "range"] {
        let mut f = Fixture::new(change, 824);
        let before = f.session.begin_npc_dialog_source_command(false).unwrap();
        assert!(before.is_some());
        let mut packets = f.session.select_npc_dialog_target("@atelier:v1:home");
        assert!(!packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NPCResponse { .. })));
        match change {
            "exit" => {
                f.session.select_npc_dialog_target("@Exit");
            }
            "replace" => {
                // Re-opening Alice is a new ordinary NPC display, even though
                // its identity and Home navigation happen to match the old one.
                f.session.interact(33);
            }
            "range" => f
                .session
                .force_authoritative_player_transform(Point { x: 400, y: 400 }, MirDirection::Up),
            _ => unreachable!(),
        }
        packets = f
            .session
            .finish_guild_experience_command(before, packets)
            .unwrap();
        assert!(!packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NPCResponse { .. })));
        let dialog = &f
            .session
            .app
            .world()
            .resource::<NpcStateResource>()
            .active_npc_dialog;
        if change == "exit" {
            assert!(dialog.is_none());
        } else {
            assert_ne!(dialog.as_ref().unwrap().title, "Crafting workshop");
        }
        let mut late = Vec::new();
        super::super::atelier_npc::publish_pending(&mut f.session, &mut late);
        assert!(late.is_empty());
        f.assert_checkpoint();
    }
}

#[test]
fn atelier_real_alice_links_make_original_jewellery_then_normal_equip() {
    let mut f = Fixture::new("normal", 824);
    f.session.select_npc_dialog_target_impl("@atelier:v1:home");
    f.session
        .select_npc_dialog_target_impl("@atelier:v1:recipe:copper-ring");
    let target = f
        .session
        .app
        .world()
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .as_ref()
        .unwrap()
        .links
        .iter()
        .find(|l| l.target.starts_with("@atelier:v1:craft:copper-ring:"))
        .unwrap()
        .target
        .clone();
    let packets = f.session.select_npc_dialog_target_impl(&target);
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LoseGold { gold: 300 })));
    let wire = packets
        .iter()
        .find_map(|p| match p {
            ServerPacket::GainedItem { item } => Some(item),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        (wire.unique_id, wire.item_index, wire.count),
        (ORE_UID, 404, 1)
    );
    assert_eq!((wire.current_dura, wire.max_dura), (5000, 5000));
    assert!(wire.added_stats.is_empty());
    assert_eq!(
        f.session
            .app
            .world()
            .resource::<InventoryResource>()
            .inventory_items[0]
            .slot,
        0
    );
    f.assert_checkpoint();
    assert!(f
        .session
        .handle_packet(ClientPacket::EquipItem {
            grid: MirGridType::Inventory,
            unique_id: ORE_UID,
            to: 7
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::EquipItem { success: true, .. })));
    assert!(f
        .session
        .app
        .world()
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .any(|i| i.user_item_unique_id == Some(ORE_UID)));
}

#[test]
fn atelier_full_bag_reclassification_keeps_the_original_slot_uid_and_soul_binding() {
    let mut f = Fixture::new("full-bag", 824);
    let padding = mir2_game_data::crystal_item_by_name("DemonLeather").unwrap();
    assert_eq!(padding.weight, 0);
    let capacity;
    {
        let mut inventory = f
            .session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>();
        inventory.inventory_items[0].soul_bound_id = Some(0);
        capacity = crate::config::crystal_bag_slot_capacity(inventory.inventory_capacity);
        for index in 1..capacity {
            let (container, slot) =
                super::super::inventory::inventory_container_and_slot_for_index(index as u8)
                    .unwrap();
            let mut item = embedded_item_state_from_template(&padding, container, slot);
            item.unique_id = ORE_UID + 100 + u64::from(index);
            inventory.inventory_items.push(item);
        }
        assert_eq!(super::super::inventory::free_bag_slots(&inventory), 0);
    }
    f.session.save_active_character().unwrap();
    let command = f.quote("copper-ring");
    f.session.execute_atelier(command).unwrap();
    let inventory = f.session.app.world().resource::<InventoryResource>();
    assert_eq!(inventory.inventory_items.len(), usize::from(capacity));
    let item = inventory
        .inventory_items
        .iter()
        .find(|item| item.unique_id == ORE_UID)
        .unwrap();
    let wire = try_user_item_from_item_state(item).unwrap();
    assert_eq!((item.container, item.slot), (ItemContainer::Bag1, 0));
    assert_eq!(
        (wire.unique_id, wire.item_index, wire.soul_bound_id),
        (ORE_UID, 404, 0)
    );
    assert_eq!(
        crystal_item_bind_for_item_key(&item.key),
        crystal_item_by_index(824).unwrap().bind
    );
    f.assert_checkpoint();
}

#[test]
fn atelier_jade_fashion_consumes_both_exact_materials_once_without_making_armour() {
    let mut f = Fixture::new("jade", 824);
    let mut crystal = embedded_item_state_from_template(
        &crystal_item_by_index(904).unwrap(),
        ItemContainer::Bag1,
        1,
    );
    crystal.unique_id = ORE_UID + 1;
    f.session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items
        .push(crystal);
    f.session.save_active_character().unwrap();
    let command = f.quote("jade-gala");
    let execution = f.session.execute_atelier(command.clone()).unwrap();
    assert_eq!(
        execution
            .packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::DeleteItem { count: 1, .. }))
            .count(),
        2
    );
    assert!(execution
        .packets
        .iter()
        .all(|packet| !matches!(packet, ServerPacket::GainedItem { .. })));
    assert!(f
        .session
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .is_empty());
    assert_eq!(
        f.session
            .app
            .world()
            .resource::<PlayerRuntimeResource>()
            .gold,
        8500
    );
    assert!(f
        .session
        .app
        .world()
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .appearance
        .owned_fashions
        .contains("jade-gala"));
    let bytes = fs::read(&f.path).unwrap();
    assert!(f.session.execute_atelier(command).unwrap().replayed);
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
    f.assert_checkpoint();
}

#[test]
fn atelier_jade_public_file_cold_restart_keeps_cosmetic_selection_and_real_equipment() {
    // Controlled two-material File fixture; this is ordinary NPC protocol
    // verification, not natural acquisition, an actual GUI or PostgreSQL run.
    let mut f = Fixture::new("jade-public-cold", 824);
    let mut crystal = embedded_item_state_from_template(
        &crystal_item_by_index(904).unwrap(), ItemContainer::Bag1, 1,
    );
    crystal.unique_id = ORE_UID + 1;
    let armour = crystal_item_by_index(319).unwrap();
    let equipment: super::super::equipment::EquipmentState = serde_json::from_value(serde_json::json!({
        "key":"crystal-item-319","slot":EquipmentSlot::Armour,
        "name":armour.name,"icon":armour.image,"shape":armour.shape,
        "description":"Controlled owned real armour", "durability_current":armour.durability,
        "durability_max":armour.durability,"user_item_unique_id":ORE_UID+2,
        "user_item_metadata":{"item_index":319},"attack":0,"defence":0
    })).unwrap();
    {
        let mut inventory=f.session.app.world_mut().resource_mut::<InventoryResource>();
        inventory.inventory_items.push(crystal);
        inventory.equipment_items.push(equipment);
    }
    super::super::stats::refresh_player_stats(f.session.app.world_mut());
    f.session.save_active_character().unwrap();
    f.session.interact(33);
    let packets=f.session.select_npc_dialog_target("@atelier:v1:home");
    assert_final_page(&f.session,&packets);
    let recipe=displayed_link(&f.session,"@atelier:v1:recipe:jade-gala");
    let packets=f.session.select_npc_dialog_target(&recipe);
    assert_final_page(&f.session,&packets);
    let craft=displayed_link(&f.session,"@atelier:v1:craft:jade-gala:");
    let packets=f.session.select_npc_dialog_target(&craft);
    assert_final_page(&f.session,&packets);
    assert!(packets.iter().any(|p|matches!(p,ServerPacket::LoseGold{gold:1500})));
    assert_eq!(packets.iter().filter(|p|matches!(p,ServerPacket::DeleteItem{count:1,..})).count(),2);
    assert!(!packets.iter().any(|p|matches!(p,ServerPacket::GainedItem{..})));
    assert!(f.session.app.world().resource::<InventoryResource>().inventory_items.is_empty());
    f.assert_checkpoint();
    let before_stats=stats(&f.session);
    let physical = |session: &SimulationSession| {
        let s=session.world_snapshot();
        serde_json::json!({"equipment":s.equipment_items,"inventory":s.inventory_items,
            "belt":s.belt_items,"weights":s.player_weights,"weight":s.current_weight,"gold":s.gold})
    };
    let expected=physical(&f.session);
    assert_eq!(expected["gold"],8500);
    let mut selected_image=None;
    for (prefix, selected) in [("@atelier:v1:wear:jade-gala:",Some("jade-gala")),("@atelier:v1:clear:",None),("@atelier:v1:wear:jade-gala:",Some("jade-gala"))] {
        let target=displayed_link(&f.session,prefix);
        let packets=f.session.select_npc_dialog_target(&target);
        assert_final_page(&f.session,&packets);
        assert!(packets.iter().any(|p|matches!(p,ServerPacket::PlayerUpdate{..})));
        assert!(!packets.iter().any(|p|matches!(p,ServerPacket::LoseGold{..}|ServerPacket::GainedItem{..})));
        let save=f.session.active_character_checkpoint().unwrap();
        assert_eq!(systems(&save).unwrap().appearance.selected_fashion.as_deref(),selected);
        assert_eq!(stats(&f.session),before_stats);
        assert_eq!(physical(&f.session),expected);
        let image=f.session.world_snapshot().cosmetic_armour_image;
        assert_eq!(image.is_some(),selected.is_some());
        if selected.is_some(){selected_image=image;}
        f.assert_checkpoint();
    }
    let unchanged=fs::read(&f.path).unwrap();
    f.session.select_npc_dialog_target(&craft);
    assert_eq!(fs::read(&f.path).unwrap(),unchanged);
    f.session.try_handle_packet(ClientPacket::LogOut).unwrap();
    let path=f.path.clone();
    drop(f);
    let config=SimulationConfig::default().with_crystal_world_runtime().with_account_store_path(&path);
    let mut session=login(config);
    let save=session.active_character_checkpoint().unwrap();
    let appearance=systems(&save).unwrap().appearance;
    assert!(appearance.owned_fashions.contains("jade-gala"));
    assert_eq!(appearance.selected_fashion.as_deref(),Some("jade-gala"));
    assert_eq!(session.world_snapshot().cosmetic_armour_image,selected_image);
    assert_eq!(stats(&session),before_stats);
    assert_eq!(physical(&session),expected);
    session.interact(33);
    let before=fs::read(&path).unwrap();
    session.select_npc_dialog_target(&craft);
    assert_eq!(fs::read(&path).unwrap(),before);
    assert_eq!(physical(&session),expected);
}

#[test]
fn atelier_file_cold_restart_preserves_unlock_private_receipt_and_replay_without_second_debit() {
    let mut f = Fixture::new("restart", 824);
    let command = f.quote("traveler");
    assert!(!f.session.execute_atelier(command.clone()).unwrap().replayed);
    f.assert_checkpoint();
    let path = f.path.clone();
    let committed = f.disk();
    let public = serde_json::to_value(f.session.world_snapshot().client_view()).unwrap();
    assert!(public["stage5Systems"]["appearance"]
        .get("atelierReceipts")
        .is_none());
    assert!(systems(&committed.accounts["demo"].saves[&0])
        .unwrap()
        .appearance
        .owned_fashions
        .contains("traveler"));
    drop(f);
    let config = SimulationConfig::default()
        .with_crystal_world_runtime()
        .with_account_store_path(&path);
    let mut session = login(config);
    session.interact(33);
    let baseline = fs::read(&path).unwrap();
    let before =
        serde_json::to_value(snapshot_active_character_save(session.app.world()).unwrap()).unwrap();
    let replay = session.execute_atelier(command).unwrap();
    assert!(replay.replayed && replay.packets.is_empty());
    assert_eq!(fs::read(&path).unwrap(), baseline);
    assert_eq!(
        serde_json::to_value(snapshot_active_character_save(session.app.world()).unwrap()).unwrap(),
        before
    );
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        9500
    );
}

#[test]
fn atelier_old_revision_changed_request_and_fabricated_recipe_never_consume() {
    let mut f = Fixture::new("stale", 824);
    let old = f.quote("copper-ring");
    f.session.save_active_character().unwrap();
    let bytes = fs::read(&f.path).unwrap();
    let before = f.live();
    assert_eq!(f.session.execute_atelier(old).unwrap_err(), Error::Stale);
    let mut forged = f.quote("copper-ring");
    forged.request_id = "0".repeat(64);
    assert_eq!(
        f.session.execute_atelier(forged).unwrap_err(),
        Error::InvalidRequest
    );
    let mut forged = f.quote("copper-ring");
    forged.action = Action::Craft {
        recipe: "free-dragon-armour".into(),
    };
    assert_eq!(
        f.session.execute_atelier(forged).unwrap_err(),
        Error::InvalidRequest
    );
    assert_eq!(f.live(), before);
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
}

#[test]
fn atelier_same_request_replays_but_cross_character_cannot_spend_or_recover_it() {
    let mut f = Fixture::new("owner", 824);
    let command = f.quote("copper-ring");
    f.session.execute_atelier(command.clone()).unwrap();
    let baseline = fs::read(&f.path).unwrap();
    let replay = f.session.execute_atelier(command.clone()).unwrap();
    assert!(replay.replayed && replay.packets.is_empty());
    assert_eq!(fs::read(&f.path).unwrap(), baseline);
    let mut other = SimulationSession::new(f.config.clone());
    other.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    other.handle_packet(ClientPacket::NewCharacter {
        name: "AtelierOther".into(),
        gender: MirGender::Male,
        class: MirClass::Warrior,
    });
    let index = f.config.account_store.lock().unwrap().accounts["demo"]
        .characters
        .iter()
        .find(|c| c.name == "AtelierOther")
        .unwrap()
        .index;
    other.handle_packet(ClientPacket::StartGame {
        character_index: index,
    });
    other.app.world_mut().insert_resource(Enabled);
    other.force_authoritative_player_transform(Point { x: 321, y: 266 }, MirDirection::Up);
    other.save_active_character().unwrap();
    other.begin_npc_purchase_producer().unwrap();
    other.interact(33);
    let bytes = fs::read(&f.path).unwrap();
    let before =
        serde_json::to_value(snapshot_active_character_save(other.app.world()).unwrap()).unwrap();
    assert!(other.execute_atelier(command).is_err());
    assert_eq!(
        serde_json::to_value(snapshot_active_character_save(other.app.world()).unwrap()).unwrap(),
        before
    );
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
}

#[test]
fn atelier_locked_rented_sealed_or_foreign_bound_materials_are_not_offered() {
    for variant in 0..5 {
        let mut f = Fixture::new(&format!("plain-{variant}"), 824);
        {
            let mut inventory = f
                .session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>();
            match variant {
                0 => {
                    inventory.reserved_item_unique_ids.insert(ORE_UID);
                }
                1 => inventory.inventory_items[0].rental_locked = true,
                2 => inventory.inventory_items[0].sealed_next_time_binary_datetime = i64::MAX,
                3 => inventory.inventory_items[0].soul_bound_id = Some(99999),
                _ => inventory.inventory_items[0].stone_serial = Some(1),
            }
        }
        let before = f.live();
        let bytes = fs::read(&f.path).unwrap();
        assert!(f
            .session
            .quote_atelier(Action::Craft {
                recipe: "copper-ring".into()
            })
            .is_err());
        assert_eq!(f.live(), before);
        assert_eq!(fs::read(&f.path).unwrap(), bytes);
    }
}

#[test]
fn atelier_wrong_npc_out_of_range_and_dead_owners_cannot_execute_displayed_quote() {
    for variant in 0..3 {
        let mut f = Fixture::new(&format!("admission-{variant}"), 824);
        let command = f.quote("copper-ring");
        match variant {
            0 => {
                f.session
                    .app
                    .world_mut()
                    .resource_mut::<NpcStateResource>()
                    .active_npc_dialog
                    .as_mut()
                    .unwrap()
                    .npc_object_id = 19
            }
            1 => f
                .session
                .force_authoritative_player_transform(Point { x: 100, y: 100 }, MirDirection::Up),
            _ => {
                f.session
                    .app
                    .world_mut()
                    .resource_mut::<PlayerRuntimeResource>()
                    .player_dead = true;
                let player = player_entity(f.session.app.world()).unwrap();
                f.session
                    .app
                    .world_mut()
                    .entity_mut(player)
                    .get_mut::<PlayerVitals>()
                    .unwrap()
                    .hp = 0;
            }
        }
        let before = f.live();
        let bytes = fs::read(&f.path).unwrap();
        assert_eq!(
            f.session.execute_atelier(command).unwrap_err(),
            Error::Unavailable
        );
        assert_eq!(f.live(), before);
        assert_eq!(fs::read(&f.path).unwrap(), bytes);
    }
}

#[test]
fn atelier_gold_or_second_material_shortage_has_no_partial_debit() {
    let mut f = Fixture::new("short", 824);
    let before = f.live();
    let bytes = fs::read(&f.path).unwrap();
    assert_eq!(
        f.session
            .quote_atelier(Action::Craft {
                recipe: "jade-gala".into()
            })
            .unwrap_err(),
        Error::Materials
    );
    assert_eq!(f.live(), before);
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
    f.session
        .app
        .world_mut()
        .resource_mut::<PlayerRuntimeResource>()
        .gold = 1;
    f.session.save_active_character().unwrap();
    let bytes = fs::read(&f.path).unwrap();
    assert_eq!(
        f.session
            .quote_atelier(Action::Craft {
                recipe: "copper-ring".into()
            })
            .unwrap_err(),
        Error::Gold
    );
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
}

#[test]
fn atelier_failed_file_commit_keeps_whole_checkpoint_then_original_request_can_succeed() {
    let mut f = Fixture::new("failure", 824);
    let command = f.quote("copper-ring");
    let bytes = fs::read(&f.path).unwrap();
    let before = f.live();
    f.config
        .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
    assert_eq!(
        f.session.execute_atelier(command.clone()).unwrap_err(),
        Error::AuthorityRejected
    );
    assert_eq!(f.live(), before);
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
    f.session.execute_atelier(command).unwrap();
    f.assert_checkpoint();
    assert_eq!(
        f.session
            .app
            .world()
            .resource::<PlayerRuntimeResource>()
            .gold,
        9700
    );
}

#[test]
fn atelier_ambiguous_file_commit_fences_writes_without_live_success_or_automatic_retry() {
    let mut f = Fixture::new("unknown", 824);
    let command = f.quote("copper-ring");
    let before = f.live();
    f.config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    assert_eq!(
        f.session.execute_atelier(command.clone()).unwrap_err(),
        Error::OutcomeUnknown
    );
    assert_eq!(f.live(), before);
    let committed = fs::read(&f.path).unwrap();
    let disk = f.disk();
    assert_eq!(disk.accounts["demo"].saves[&0].gold, 9700);
    assert!(f.config.ensure_account_store_writable().is_err());
    assert!(f.session.execute_atelier(command).is_err());
    assert!(f.session.save_active_character().is_err());
    assert_eq!(fs::read(&f.path).unwrap(), committed);
}

#[test]
fn atelier_fashion_unlock_duplicate_wear_and_clear_never_change_equipment_or_stats() {
    let mut f = Fixture::new("fashion", 824);
    let command = f.quote("traveler");
    f.session.execute_atelier(command).unwrap();
    let bytes = fs::read(&f.path).unwrap();
    assert_eq!(
        f.session
            .quote_atelier(Action::Craft {
                recipe: "traveler".into()
            })
            .unwrap_err(),
        Error::AlreadyOwned
    );
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
    let before_stats = stats(&f.session);
    let before_equipment = serde_json::to_value(
        &f.session
            .app
            .world()
            .resource::<InventoryResource>()
            .equipment_items,
    )
    .unwrap();
    let gold = f
        .session
        .app
        .world()
        .resource::<PlayerRuntimeResource>()
        .gold;
    for action in [
        Action::Wear {
            fashion: "traveler".into(),
        },
        Action::Clear,
    ] {
        let command = f.session.quote_atelier(action).unwrap();
        let execution = f.session.execute_atelier(command).unwrap();
        assert!(execution
            .packets
            .iter()
            .any(|p| matches!(p, ServerPacket::PlayerUpdate { .. })));
        assert!(execution.packets.iter().all(|p| !matches!(
            p,
            ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. }
        )));
        assert_eq!(stats(&f.session), before_stats);
        assert_eq!(
            serde_json::to_value(
                &f.session
                    .app
                    .world()
                    .resource::<InventoryResource>()
                    .equipment_items
            )
            .unwrap(),
            before_equipment
        );
        assert_eq!(
            f.session
                .app
                .world()
                .resource::<PlayerRuntimeResource>()
                .gold,
            gold
        );
        f.assert_checkpoint();
    }
}

#[test]
fn atelier_receipts_reject_tampering_and_eviction_never_revives_an_old_quote() {
    let mut f = Fixture::new("retention", 824);
    let original = f.quote("traveler");
    f.session.execute_atelier(original.clone()).unwrap();
    let mut forged = f.disk().accounts["demo"].saves[&0].clone();
    let mut state = systems(&forged).unwrap();
    let encoded = state
        .appearance
        .atelier_receipts
        .get_mut(&original.request_id)
        .unwrap();
    let mut receipt: serde_json::Value = serde_json::from_str(encoded).unwrap();
    receipt["canonical"]["gold"] = serde_json::json!(1);
    *encoded = serde_json::to_string(&receipt).unwrap();
    forged.stage5_systems_json = Some(serde_json::to_string(&state).unwrap());
    assert!(validate_receipts(&forged).is_err());
    for _ in 0..catalog::RECEIPT_LIMIT + 1 {
        let selected = f
            .session
            .app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .appearance
            .selected_fashion
            .is_some();
        let action = if selected {
            Action::Clear
        } else {
            Action::Wear {
                fashion: "traveler".into(),
            }
        };
        let command = f.session.quote_atelier(action).unwrap();
        f.session.execute_atelier(command).unwrap();
    }
    let appearance = &f
        .session
        .app
        .world()
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .appearance;
    assert_eq!(appearance.atelier_receipts.len(), catalog::RECEIPT_LIMIT);
    assert!(!appearance
        .atelier_receipts
        .contains_key(&original.request_id));
    let bytes = fs::read(&f.path).unwrap();
    let before = f.live();
    assert_eq!(
        f.session.execute_atelier(original).unwrap_err(),
        Error::Stale
    );
    assert_eq!(f.live(), before);
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
    f.assert_checkpoint();
}

#[test]
fn atelier_requires_the_exact_current_displayed_link_even_for_an_otherwise_valid_quote() {
    let mut f = Fixture::new("displayed", 824);
    let command = f.quote("copper-ring");
    let target = format!(
        "@atelier:v1:craft:copper-ring:{}:{}",
        command.expected_revision, command.request_id
    );
    let bytes = fs::read(&f.path).unwrap();
    let before = f.live();
    // Alice's initial page offers Home, not this unshown action.
    assert!(f.session.select_npc_dialog_target_impl(&target).is_empty());
    assert_eq!(f.live(), before);
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
    f.session.select_npc_dialog_target_impl("@atelier:v1:home");
    f.session
        .select_npc_dialog_target_impl("@atelier:v1:recipe:copper-ring");
    assert!(f
        .session
        .select_npc_dialog_target_impl(&format!("{target}(forged)"))
        .is_empty());
    assert_eq!(f.live(), before);
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
}

#[test]
fn atelier_unknown_dialog_has_only_exit_and_no_success_or_replacement_quote() {
    let mut f = Fixture::new("unknown-dialog", 824);
    f.session.select_npc_dialog_target_impl("@atelier:v1:home");
    f.session
        .select_npc_dialog_target_impl("@atelier:v1:recipe:copper-ring");
    let target = f
        .session
        .app
        .world()
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .as_ref()
        .unwrap()
        .links
        .iter()
        .find(|link| link.target.starts_with("@atelier:v1:craft:"))
        .unwrap()
        .target
        .clone();
    let before = f.live();
    f.config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    let packets = f.session.select_npc_dialog_target_impl(&target);
    assert!(packets.iter().all(|packet| !matches!(
        packet,
        ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. }
    )));
    let dialog = f
        .session
        .app
        .world()
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .as_ref()
        .unwrap();
    assert_eq!(dialog.links.len(), 1);
    assert_eq!(dialog.links[0].target, "@Exit");
    assert!(!dialog.body.is_empty());
    assert_eq!(f.live(), before);
    assert!(f.config.ensure_account_store_writable().is_err());
}
