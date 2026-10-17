//! Real synchronous Session/File transactions with imported NPCs and UserItems.
//! Isolated server fixtures, not a live gateway, renderer or human acceptance.
use super::*;
use crate::{AccountStoreTransactionFault, SimulationConfig};
use super::super::components::{NpcAgent, ObjectId};
use mir2_production::{StoneError, StoneLifecycle, StoneMaterial};
use mir2_protocol::{ClientPacket, MirDirection, Point};
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture { config: SimulationConfig, session: SimulationSession, path: PathBuf, npc: u32 }
fn login(config: SimulationConfig) -> SimulationSession {
    let mut session=SimulationSession::new(config);
    assert!(session.handle_packet(ClientPacket::Login { account_id:"demo".into(), password:"demo".into() })
        .iter().any(|p| matches!(p,ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index:0 });
    assert!(session.active_identity().is_some());
    session
}
impl Fixture {
    fn new(label: &str, ore: i32) -> Self {
        let dir=std::env::temp_dir().join(format!("mir2-stones-{label}-{}-{}",std::process::id(),
            SEQUENCE.fetch_add(1,Ordering::Relaxed)));
        fs::create_dir(&dir).unwrap();
        let path=dir.join("accounts.json");
        let config=SimulationConfig::default().with_crystal_world_runtime().with_account_store_path(&path);
        let mut session=login(config.clone());
        let template=crystal_item_by_index(ore).unwrap();
        let mut item=embedded_item_state_from_template(&template,ItemContainer::Bag1,0);
        item.unique_id=120_000_001;
        {
            let mut inv=session.app.world_mut().resource_mut::<InventoryResource>();
            inv.inventory_items=vec![item]; inv.belt_items.clear(); inv.equipment_items.clear();
        }
        session.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold=100_000;
        session.force_authoritative_player_transform(Point{x:302,y:222},MirDirection::Up);
        session.save_active_character().unwrap();
        session.begin_stone_workshops().unwrap();
        let npc=session.app.world_mut().query::<(&ObjectId,&NpcAgent)>().iter(session.app.world())
            .find(|(_,a)|a.script_key.as_deref()==Some("BichonProvince/BichonWall/Blacksmith"))
            .map(|(id,_)|id.0).expect("real Bichon Bill");
        session.interact(npc);
        assert!(workshop_allowed(session.app.world()));
        Self {config,session,path,npc}
    }
    fn view(&mut self) -> StoneWorkshopView { self.session.stone_workshop_view().unwrap() }
    fn command(&mut self,id:&str,operation:Op)->StoneWorkshopCommand {
        StoneWorkshopCommand {request_id:id.into(),expected_inventory_revision:self.view().inventory_revision,operation}
    }
    fn seal(&mut self) -> u64 {
        let uid=self.session.app.world().resource::<InventoryResource>().inventory_items[0].unique_id;
        let c=self.command("seal",Op::Seal{uid});
        self.session.execute_stone_workshop(c).unwrap();
        self.view().stones[0].serial.parse().unwrap()
    }
    fn action(&mut self,id:&str,appraise:bool) -> StoneWorkshopCommand {
        let stone=self.view().stones[0].clone();
        let serial=stone.serial.parse().unwrap();let expected_stone_revision=stone.revision.parse().unwrap();
        self.command(id,if appraise {Op::Appraise{serial,expected_stone_revision}} else {Op::Cut{serial,expected_stone_revision}})
    }
    fn disk(&self)->crate::AccountStore {serde_json::from_slice(&fs::read(&self.path).unwrap()).unwrap()}
    fn live(&self)->CharacterSaveRecord {snapshot_active_character_save(self.session.app.world()).unwrap()}
    fn assert_checkpoint(&self) {
        let live=self.live();let store=self.disk();let durable=&store.accounts["demo"].saves[&0];
        assert_eq!(live.revision,durable.revision);assert_eq!(live.gold,durable.gold);
        assert_eq!(live.inventory_items_json,durable.inventory_items_json);
        assert_eq!(live.belt_items_json,durable.belt_items_json);
        assert_eq!(live.npc_purchase_journal,durable.npc_purchase_journal);
    }
}

impl SimulationSession {
    // The WorldRuntime child module can wrap this controlled, independent File
    // fixture without adding any production constructor or global opt-in.
    #[cfg(test)]
    pub(crate) fn stone_world_runtime_test_fixture() -> (Self, PathBuf) {
        let fixture = Fixture::new("world-command", 824);
        (fixture.session, fixture.path)
    }
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
        .unwrap_or_else(|| panic!("missing actual stone link {prefix}"))
        .target
        .clone()
}

fn assert_final_stone_page(session: &SimulationSession, packets: &[ServerPacket]) {
    let dialog = session
        .app
        .world()
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .as_ref()
        .unwrap();
    assert_eq!(dialog.title, "Stone workshop");
    assert!(super::super::stone_npc::approved_npc(
        session.app.world(),
        dialog.npc_object_id
    ));
    let revision = session.active_character_checkpoint().unwrap().revision;
    let mut expected = vec![dialog.title.clone()];
    expected.extend(dialog.body.iter().cloned());
    for link in &dialog.links {
        expected.push(format!("<{}/{}>", link.text, link.target));
        if ["@stone:v1:seal:", "@stone:v1:appraise:", "@stone:v1:cut:"]
            .iter()
            .any(|prefix| link.target.starts_with(prefix))
        {
            assert_eq!(
                link.target.rsplit(':').nth(1).unwrap().parse::<u64>(),
                Ok(revision)
            );
        }
        assert!(super::super::npc_input::has_menu_target(
            session.app.world(),
            &link.target
        ));
    }
    expected.push(dialog.footer.clone());
    let pages: Vec<_> = packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::NPCResponse { page } => Some(page),
            _ => None,
        })
        .collect();
    assert_eq!(pages, vec![&expected]);
}

fn open_public_ore_page(f: &mut Fixture) {
    for prefix in ["@stone:v1:home", "@stone:v1:list:ore:", "@stone:v1:ore:"] {
        let target = displayed_link(&f.session, prefix);
        let packets = f.session.select_npc_dialog_target(&target);
        assert_final_stone_page(&f.session, &packets);
        f.assert_checkpoint();
    }
}

#[test]
fn stone_public_file_source_accepts_first_seal_appraise_cut_quotes_after_final_revision() {
    let mut f = Fixture::new("public-source", 824);
    open_public_ore_page(&mut f);
    let initial = f.live();
    let actor = initial.npc_purchase_journal.as_ref().unwrap().actor;
    let mut old_seal = None;
    for (index, prefix, fee) in [
        (0, "@stone:v1:seal:", 0),
        (1, "@stone:v1:appraise:", 100),
        (2, "@stone:v1:cut:", 1500),
    ] {
        let target = displayed_link(&f.session, prefix);
        let before = f.live();
        assert_eq!(
            target.rsplit(':').nth(1).unwrap().parse::<u64>(),
            Ok(before.revision)
        );
        if index == 0 {
            old_seal = Some(target.clone());
        }
        // The exact first displayed quote must work. No refresh, retry or
        // private economic method may repair an intermediate revision.
        let packets = f.session.select_npc_dialog_target(&target);
        let after = f.live();
        assert_eq!(after.revision, before.revision + 2);
        assert_eq!(after.gold, before.gold - fee);
        assert_eq!(after.npc_purchase_journal.as_ref().unwrap().actor, actor);
        let inventory = f.session.app.world().resource::<InventoryResource>();
        assert_eq!(inventory.inventory_items.len(), 1);
        let item = &inventory.inventory_items[0];
        assert_eq!(item.unique_id, 120_000_001);
        assert_eq!(item.slot, 0);
        if index < 2 {
            assert!(item.stone_serial.is_some());
            let public = f.view();
            assert_eq!(public.stones.len(), 1);
            assert_eq!(public.stones[0].revision, index.to_string());
            assert_eq!(
                public.stones[0].lifecycle,
                if index == 0 {
                    StoneLifecycle::Sealed
                } else {
                    StoneLifecycle::Appraised
                }
            );
        } else {
            assert_eq!(item.stone_serial, None);
            assert!(packets.iter().any(|packet| matches!(packet,
                ServerPacket::GainedItem { item } if item.unique_id == 120_000_001
                    && StoneMaterial::ALL.iter().any(|material| material.template_id() == item.item_index))));
            assert!(f.view().stones.is_empty());
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
        assert_final_stone_page(&f.session, &packets);
        f.assert_checkpoint();
    }
    let committed = fs::read(&f.path).unwrap();
    let before = f.live();
    let packets = f.session.select_npc_dialog_target(&old_seal.unwrap());
    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. }
    )));
    assert_eq!(
        serde_json::to_value(f.live()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(fs::read(&f.path).unwrap(), committed);
}

#[test]
fn stone_public_known_file_failure_requotes_without_debit_and_first_retry_succeeds() {
    let mut f = Fixture::new("public-known-failure", 824);
    open_public_ore_page(&mut f);
    let target = displayed_link(&f.session, "@stone:v1:seal:");
    let before = f.live();
    let bytes = fs::read(&f.path).unwrap();
    f.config
        .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
    let packets = f.session.select_npc_dialog_target(&target);
    assert_eq!(
        serde_json::to_value(f.live()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. }
    )));
    assert_final_stone_page(&f.session, &packets);
    let retry = displayed_link(&f.session, "@stone:v1:seal:");
    assert_eq!(retry, target);
    let packets = f.session.select_npc_dialog_target(&retry);
    assert_eq!(f.live().revision, before.revision + 2);
    assert_eq!(f.live().gold, before.gold);
    assert_eq!(f.view().stones.len(), 1);
    assert_final_stone_page(&f.session, &packets);
    f.assert_checkpoint();
}

#[test]
fn stone_public_unknown_file_commit_never_publishes_new_quotes_or_retries() {
    let mut f = Fixture::new("public-unknown", 824);
    open_public_ore_page(&mut f);
    let target = displayed_link(&f.session, "@stone:v1:seal:");
    let before = f.live();
    f.config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    let packets = f.session.select_npc_dialog_target(&target);
    assert!(f.config.ensure_account_store_writable().is_err());
    assert_eq!(
        serde_json::to_value(f.live()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert!(!packets.iter().any(|packet| matches!(
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
    let committed = fs::read(&f.path).unwrap();
    let store = f.disk();
    let saved = &store.accounts["demo"].saves[&0];
    assert_eq!(saved.revision, before.revision + 1);
    assert_eq!(saved.gold, before.gold);
    assert_ne!(saved.inventory_items_json, before.inventory_items_json);
    let mut late = Vec::new();
    super::super::atelier_npc::publish_pending(&mut f.session, &mut late);
    assert!(late.is_empty());
    f.session.select_npc_dialog_target(&target);
    assert_eq!(fs::read(&f.path).unwrap(), committed);
    assert_eq!(
        serde_json::to_value(f.live()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
}

#[test]
fn stone_outer_source_failure_discards_the_nested_page_after_one_economic_commit() {
    let mut f = Fixture::new("public-outer-failure", 824);
    open_public_ore_page(&mut f);
    let target = displayed_link(&f.session, "@stone:v1:seal:");
    let before_revision = f.live().revision;
    let before = f.session.begin_npc_dialog_source_command(false).unwrap();
    assert!(before.is_some());
    let packets = f.session.select_npc_dialog_target(&target);
    assert!(super::super::shared_guild_experience::source_command_active(f.session.app.world()));
    assert!(!packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::NPCResponse { .. })));
    assert_eq!(f.live().revision, before_revision + 1);
    assert!(f
        .session
        .app
        .world()
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .as_ref()
        .unwrap()
        .links
        .iter()
        .all(|link| !link.target.starts_with("@stone:v1:appraise:")));
    let committed = fs::read(&f.path).unwrap();
    f.config
        .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
    assert!(f
        .session
        .finish_guild_experience_command(before, packets)
        .is_err());
    assert_eq!(fs::read(&f.path).unwrap(), committed);
    assert_eq!(f.live().revision, before_revision + 1);
    assert!(f
        .session
        .app
        .world()
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .is_none());
    let mut late = Vec::new();
    super::super::atelier_npc::publish_pending(&mut f.session, &mut late);
    assert!(late.is_empty());
    assert_eq!(fs::read(&f.path).unwrap(), committed);
    f.assert_checkpoint();
}

#[test]
fn stone_pending_page_cannot_reopen_after_exit_replacement_or_leaving_npc_range() {
    for change in ["exit", "replace", "range"] {
        let mut f = Fixture::new(change, 824);
        let before = f.session.begin_npc_dialog_source_command(false).unwrap();
        assert!(before.is_some());
        let mut packets = f.session.select_npc_dialog_target("@stone:v1:home");
        assert!(!packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NPCResponse { .. })));
        match change {
            "exit" => {
                f.session.select_npc_dialog_target("@Exit");
            }
            "replace" => {
                f.session.interact(f.npc);
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
            assert_ne!(dialog.as_ref().unwrap().title, "Stone workshop");
        }
        let mut late = Vec::new();
        super::super::atelier_npc::publish_pending(&mut f.session, &mut late);
        assert!(late.is_empty());
    }
}

#[test]
fn stone_real_npc_dialog_drives_seal_appraise_cut_and_rejects_stale_click() {
    let mut f=Fixture::new("npc",824);
    let next=|s:&SimulationSession,prefix:&str|s.app.world().resource::<NpcStateResource>().active_npc_dialog
        .as_ref().unwrap().links.iter().find(|l|l.target.starts_with(prefix)).unwrap().target.clone();
    for prefix in ["@stone:v1:home","@stone:v1:list:ore:","@stone:v1:ore:","@stone:v1:seal:"] {
        let t=next(&f.session,prefix);f.session.select_npc_dialog_target_impl(&t);
    }
    assert_eq!(f.view().stones.len(),1);
    for prefix in ["@stone:v1:appraise:"] {
        let t=next(&f.session,prefix);f.session.select_npc_dialog_target_impl(&t);
    }
    assert_eq!(f.view().stones[0].lifecycle,StoneLifecycle::Appraised);
    let cut=next(&f.session,"@stone:v1:cut:");
    f.session.select_npc_dialog_target_impl(&cut);
    let committed=fs::read(&f.path).unwrap();
    assert!(f.view().stones.is_empty());
    assert!(f.session.select_npc_dialog_target_impl(&cut).is_empty());
    assert_eq!(fs::read(&f.path).unwrap(),committed);
    f.assert_checkpoint();
}
#[test]
fn stone_three_grades_pay_frozen_fees_and_keep_real_uid_binding_and_slot() {
    for (ore,grade) in [(824,StoneGrade::Rough),(826,StoneGrade::Fine),(827,StoneGrade::Precious)] {
        let mut f=Fixture::new("grades",ore);
        let serial=f.seal();let before=f.view().stones[0].clone();assert_eq!(before.grade,grade);
        assert_eq!(before.possibilities.iter().map(|p|u32::from(p.basis_points)).sum::<u32>(),10_000);
        let expected=f.disk().sealed_stones.record(serial).unwrap().private_output().clone();
        let appraisal=f.action("appraise",true);f.session.execute_stone_workshop(appraisal).unwrap();
        let cut=f.action("cut",false);let out=f.session.execute_stone_workshop(cut).unwrap();
        assert!(matches!(&out.packets[0],ServerPacket::NewItemInfo {info} if info.index==expected.template_id));
        assert!(out.packets.iter().any(|p|matches!(p,ServerPacket::GainedItem {item} if item.unique_id==120_000_001 && item.item_index==expected.template_id)));
        let inv=f.session.app.world().resource::<InventoryResource>();
        assert_eq!(inv.inventory_items.len(),1);assert_eq!(inv.inventory_items[0].unique_id,120_000_001);
        assert_eq!(inv.inventory_items[0].slot,0);assert_eq!(inv.inventory_items[0].stone_serial,None);
        assert_eq!(f.live().gold,100_000-u32::try_from(before.appraisal_gold+before.cutting_gold).unwrap());
        f.assert_checkpoint();
    }
}
#[test]
fn stone_exact_cut_replay_after_inventory_and_gold_change_never_delivers_twice() {
    let mut f=Fixture::new("replay",826);f.seal();let cut=f.action("cut",false);
    f.session.execute_stone_workshop(cut.clone()).unwrap();
    f.session.app.world_mut().resource_mut::<InventoryResource>().inventory_items.clear();
    f.session.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold=0;
    f.session.save_active_character().unwrap();let committed=fs::read(&f.path).unwrap();
    let repeat=f.session.execute_stone_workshop(cut).unwrap();
    assert!(repeat.replayed);assert!(repeat.packets.is_empty());assert!(!repeat.public_result.is_empty());
    assert_eq!(fs::read(&f.path).unwrap(),committed);assert_eq!(f.live().gold,0);
}
#[test]
fn stone_changed_operation_with_original_request_id_is_rejected() {
    let mut f=Fixture::new("conflict",824);f.seal();let a=f.action("original",true);
    f.session.execute_stone_workshop(a.clone()).unwrap();let before=fs::read(&f.path).unwrap();
    let mut changed=f.action("original",false);changed.expected_inventory_revision=a.expected_inventory_revision;
    assert_eq!(f.session.execute_stone_workshop(changed).unwrap_err(),Error::Rejected(StoneError::RequestConflict));
    assert_eq!(fs::read(&f.path).unwrap(),before);
}
#[test]
fn stone_full_bag_cut_replaces_source_without_requiring_a_free_slot() {
    let mut f=Fixture::new("full",824);f.seal();
    let filler=mir2_game_data::crystal_item_manifest().items.into_iter().find(|t|t.weight==0 && t.slots==0 && t.stack_size==1).unwrap();
    {
        let mut inv=f.session.app.world_mut().resource_mut::<InventoryResource>();inv.inventory_capacity=46;
        for slot in 1..40 {let mut i=embedded_item_state_from_template(&filler,ItemContainer::Bag1,slot);i.unique_id=130_000_000+u64::from(slot);inv.inventory_items.push(i);}
        assert_eq!(free_bag_slots(&inv),0);
    }
    f.session.save_active_character().unwrap();let cut=f.action("full-cut",false);
    f.session.execute_stone_workshop(cut).unwrap();
    assert_eq!(f.session.app.world().resource::<InventoryResource>().inventory_items.len(),40);
    f.assert_checkpoint();
}
#[test]
fn stone_insufficient_gold_keeps_same_sealed_instance_and_registry() {
    let mut f=Fixture::new("gold",827);f.seal();
    f.session.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold=0;f.session.save_active_character().unwrap();
    let before=fs::read(&f.path).unwrap();let a=f.action("no-gold",false);
    assert_eq!(f.session.execute_stone_workshop(a).unwrap_err(),Error::Rejected(StoneError::InsufficientGold));
    assert_eq!(fs::read(&f.path).unwrap(),before);assert_eq!(f.view().stones[0].lifecycle,StoneLifecycle::Sealed);
}
#[test]
fn stone_locked_carrier_cannot_cut_and_raw_stone_cannot_lose_reference_on_ground() {
    let mut f=Fixture::new("custody",824);f.seal();let cut=f.action("locked",false);let before=fs::read(&f.path).unwrap();
    assert!(f.session.shared_inventory_item_drop(120_000_001,1,false).is_none());
    let item=f.session.app.world().resource::<InventoryResource>().inventory_items[0].clone();
    f.session.app.world_mut().resource_mut::<InventoryResource>().reserved_item_unique_ids.insert(item.unique_id);
    assert_eq!(f.session.execute_stone_workshop(cut).unwrap_err(),Error::Rejected(StoneError::InvalidPossession));
    assert_eq!(fs::read(&f.path).unwrap(),before);
}
#[test]
fn stone_known_file_failure_preserves_live_money_and_allows_original_retry() {
    let mut f=Fixture::new("failure",826);f.seal();let cut=f.action("persist-cut",false);
    let before=fs::read(&f.path).unwrap();let live=f.live();
    f.config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
    assert_eq!(f.session.execute_stone_workshop(cut.clone()).unwrap_err(),Error::AuthorityRejected);
    assert_eq!(fs::read(&f.path).unwrap(),before);assert_eq!(f.live().inventory_items_json,live.inventory_items_json);
    assert_eq!(f.live().gold,live.gold);assert_eq!(f.live().revision,live.revision);
    f.session.execute_stone_workshop(cut).unwrap();f.assert_checkpoint();
}
#[test]
fn stone_unknown_file_commit_preserves_durable_cut_and_fences_all_followup_writes() {
    let mut f=Fixture::new("unknown",826);let serial=f.seal();let cut=f.action("unknown-cut",false);let live=f.live();
    f.config.inject_account_store_transaction_fault(AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync);
    assert_eq!(f.session.execute_stone_workshop(cut.clone()).unwrap_err(),Error::OutcomeUnknown);
    assert!(f.config.ensure_account_store_writable().is_err());assert!(f.session.save_active_character().is_err());
    assert!(f.session.execute_stone_workshop(cut.clone()).is_err());
    let (o,_)=owner(f.session.app.world()).unwrap();let store=f.disk();
    let Op::Cut{expected_stone_revision,..}=cut.operation else {panic!()};
    let receipt=store.sealed_stones.query(&o,&StoneCommand{request_id:cut.request_id,stone_serial:serial,
        expected_revision:expected_stone_revision,action:StoneAction::Cut}).unwrap().unwrap();
    assert!(receipt.delivery.is_some());assert_eq!(store.sealed_stones.record(serial).unwrap().lifecycle(),StoneLifecycle::Cut);
    assert_eq!(f.live().gold,live.gold);assert_eq!(f.live().inventory_items_json,live.inventory_items_json);
    assert!(store.accounts["demo"].saves[&0].gold<live.gold);
}
#[test]
fn stone_save_reload_keeps_original_secret_and_physical_reference() {
    let mut f=Fixture::new("reload",827);let serial=f.seal();let frozen=f.disk().sealed_stones.record(serial).unwrap().clone();
    let path=f.path.clone();let npc=f.npc;
    // Retire every old authority handle. The next config must load the durable
    // file rather than sharing the original in-memory account store.
    drop(f);
    let config=SimulationConfig::default().with_crystal_world_runtime().with_account_store_path(&path);
    let mut fresh=login(config.clone());fresh.begin_stone_workshops().unwrap();fresh.interact(npc);
    assert_eq!(fresh.stone_workshop_view().unwrap().stones[0].serial,serial.to_string());
    let row=&fresh.app.world().resource::<InventoryResource>().inventory_items[0];assert_eq!(row.stone_serial,Some(serial));
    let public=serde_json::to_string(&fresh.stone_workshop_view().unwrap().stones).unwrap();
    assert!(!public.contains("privateOutput"));assert!(!public.contains("roll"));assert!(!public.contains("history"));
    let loaded:crate::AccountStore=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(loaded.sealed_stones.record(serial).unwrap(),&frozen);
    let view=fresh.stone_workshop_view().unwrap();
    let cut=StoneWorkshopCommand {request_id:"cut-after-reload".into(),expected_inventory_revision:view.inventory_revision,
        operation:Op::Cut {serial,expected_stone_revision:view.stones[0].revision.parse().unwrap()}};
    fresh.execute_stone_workshop(cut.clone()).unwrap();
    assert_eq!(try_user_item_from_item_state(&fresh.app.world().resource::<InventoryResource>().inventory_items[0])
        .unwrap().item_index,frozen.private_output().template_id);
    let durable=fs::read(&path).unwrap();
    drop(fresh);drop(config);
    let config=SimulationConfig::default().with_crystal_world_runtime().with_account_store_path(&path);
    let mut reloaded=login(config);reloaded.begin_stone_workshops().unwrap();reloaded.interact(npc);
    assert!(reloaded.stone_workshop_view().unwrap().stones.is_empty());
    // Ordinary StartGame commits one new Login event. Every other durable field,
    // including the complete private registry, physical item and Gold, is exact.
    let after_login = fs::read(&path).unwrap();
    let mut expected: serde_json::Value = serde_json::from_slice(&durable).unwrap();
    let save = &mut expected["accounts"]["demo"]["saves"]["0"];
    let next_revision = save["revision"].as_u64().unwrap().checked_add(1).unwrap();
    save["revision"] = serde_json::json!(next_revision);
    for field in ["committedSequence", "nextSequence"] {
        let next = save["default_npc_events"][field]
            .as_u64()
            .unwrap()
            .checked_add(1)
            .unwrap();
        save["default_npc_events"][field] = serde_json::json!(next);
    }
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&after_login).unwrap(), expected);
    let replay=reloaded.execute_stone_workshop(cut).unwrap();
    assert!(replay.replayed);assert!(replay.packets.is_empty());
    assert_eq!(fs::read(&path).unwrap(),after_login);
}
#[test]
fn stone_ordinary_account_scope_and_restore_cannot_rewind_secret_or_cut_tombstone() {
    let mut f=Fixture::new("restore",824);let old=f.path.with_file_name("before.json");f.config.backup_account_store(&old).unwrap();
    f.seal();let cut=f.action("cut",false);f.session.execute_stone_workshop(cut).unwrap();let committed=fs::read(&f.path).unwrap();
    assert!(f.config.commit_account_store_transaction(&["demo".into()],|s|{s.sealed_stones=StoneRegistry::default();Ok(())}).is_err());
    assert!(f.config.restore_account_store_from_backup(&old).is_err());assert_eq!(fs::read(&f.path).unwrap(),committed);
}
#[test]
fn stone_out_of_range_fabricated_and_parenthesized_npc_targets_cannot_mutate() {
    let mut f=Fixture::new("range",824);let before=fs::read(&f.path).unwrap();
    assert!(f.session.select_npc_dialog_target_impl("@stone:v1:seal:120000001:0:forged").is_empty());
    let home="@stone:v1:home(ignored)";assert!(f.session.select_npc_dialog_target_impl(home).is_empty());
    f.session.force_authoritative_player_transform(Point{x:200,y:200},MirDirection::Up);
    assert!(f.session.select_npc_dialog_target_impl("@stone:v1:home").is_empty());
    assert_eq!(fs::read(&f.path).unwrap(),before);
}
#[test]
fn stone_material_outputs_and_raw_templates_are_real_catalog_rows() {
    for material in mir2_production::StoneMaterial::ALL {
        let t=crystal_item_by_index(material.template_id()).unwrap();
        assert_eq!(u16::from(t.weight),material.weight());assert_eq!(t.stack_size,1);
        let mut item=embedded_item_state_from_template(&t,ItemContainer::Bag1,0);
        item.unique_id=150_000_001;
        validate_committed_item_state_carrier(&item).unwrap();
    }
    for grade in StoneGrade::ALL {
        let t=crystal_item_by_index(raw_index(grade)).unwrap();assert_eq!(t.weight,4);
        assert_eq!(t.stack_size,1);assert_eq!(t.price,0);assert!(!t.can_mine);
    }
}
