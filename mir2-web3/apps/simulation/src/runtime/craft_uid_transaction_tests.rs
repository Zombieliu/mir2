//! Actual CraftItem/Session paths with an observable UID store and File source.
//! Backend faults are explicit mocks, not PostgreSQL/network fault evidence.
use super::*;
use crate::{
    SimulationConfig, UserItemUid, UserItemUidAllocator, UserItemUidError, UserItemUidReason,
    UserItemUidStore,
};
use mir2_protocol::ClientPacket;
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(in crate::runtime) struct ObservedStore {
    pub(in crate::runtime) high: AtomicU64,
    calls: AtomicUsize,
    fail_at: AtomicUsize,
    unknown: AtomicBool,
    issued: Mutex<Vec<(u64, UserItemUidReason)>>,
}
impl ObservedStore {
    pub(in crate::runtime) fn arm(&self, nth: usize, unknown: bool) {
        assert!(nth > 0);
        self.unknown.store(unknown, Ordering::SeqCst);
        self.fail_at
            .store(self.calls.load(Ordering::SeqCst) + nth, Ordering::SeqCst);
    }
    pub(in crate::runtime) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    pub(in crate::runtime) fn issued(&self) -> Vec<(u64, UserItemUidReason)> {
        self.issued.lock().unwrap().clone()
    }
}
impl UserItemUidStore for ObservedStore {
    fn issue_one(&self, reason: UserItemUidReason) -> Result<UserItemUid, UserItemUidError> {
        let ordinal = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if ordinal == self.fail_at.load(Ordering::SeqCst) && !self.unknown.load(Ordering::SeqCst) {
            return Err(UserItemUidError::DatabaseAuthorityUnavailable {
                operation: "owned craft/mining mock",
            });
        }
        let old = self.high.load(Ordering::SeqCst);
        let uid = crate::player_drop::player_drop_next_ordinary_item_uid(old).map_err(|_| {
            UserItemUidError::Exhausted {
                issued_through: old,
                generation: 0,
            }
        })?;
        self.high.store(uid, Ordering::SeqCst);
        self.issued.lock().unwrap().push((uid, reason));
        if ordinal == self.fail_at.load(Ordering::SeqCst) {
            return Err(UserItemUidError::DatabaseCommitOutcomeUnknown);
        }
        UserItemUid::new(uid)
    }
    fn issued_through(&self) -> Result<u64, UserItemUidError> {
        Ok(self.high.load(Ordering::SeqCst))
    }
    fn ensure_issued_through_at_least(&self, floor: u64) -> Result<u64, UserItemUidError> {
        Ok(self.high.fetch_max(floor, Ordering::SeqCst).max(floor))
    }
}

pub(in crate::runtime) struct Fixture {
    pub(in crate::runtime) session: SimulationSession,
    pub(in crate::runtime) config: SimulationConfig,
    pub(in crate::runtime) store: Arc<ObservedStore>,
    pub(in crate::runtime) path: PathBuf,
}
impl Fixture {
    pub(in crate::runtime) fn new(label: &str) -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap();
        let directory = root.join(format!(
            "mir2-craft-uid-{label}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(directory.starts_with(root));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("accounts.json");
        let store = Arc::new(ObservedStore {
            high: AtomicU64::new(5_000_000),
            calls: AtomicUsize::new(0),
            fail_at: AtomicUsize::new(0),
            unknown: AtomicBool::new(false),
            issued: Mutex::new(Vec::new()),
        });
        let config = SimulationConfig::default()
            .with_account_store_path(&path)
            .with_item_uid_allocator(UserItemUidAllocator::from_store(store.clone()))
            .unwrap();
        let mut session = SimulationSession::new(config.clone());
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into()
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert!(session.active_identity().is_some());
        let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
        inventory.inventory_capacity = 86;
        inventory.inventory_items.clear();
        inventory.belt_items.clear();
        inventory.storage_items.clear();
        drop(inventory);
        Self {
            session,
            config,
            store,
            path,
        }
    }
    fn recipe(
        &mut self,
        recipe_id: u64,
        succeeds: bool,
    ) -> (mir2_game_data::CrystalRecipe, Vec<i32>) {
        let recipe = crystal_recipes()
            .into_iter()
            .find(|recipe| recipe.output_unique_id == recipe_id)
            .unwrap();
        let mut slots = Vec::new();
        {
            let mut inventory = self
                .session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>();
            inventory.inventory_items.clear();
            inventory.belt_items.clear();
            for (ordinal, required) in recipe
                .tools
                .iter()
                .chain(recipe.ingredients.iter())
                .enumerate()
            {
                let template = crystal_item_by_index(required.item_index).unwrap();
                let slot = ordinal as u8;
                let mut item =
                    embedded_item_state_from_template(&template, ItemContainer::Bag1, slot);
                item.unique_id = 700_000 + ordinal as u64;
                item.quantity = u32::from(required.count.max(1));
                if ordinal < recipe.tools.len() {
                    item.durability_max = Some(item.durability_max.unwrap_or(0).max(3000));
                    item.durability_current = Some(3000);
                }
                inventory.inventory_items.push(item);
                slots.push(i32::from(slot));
            }
        }
        self.session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = 50_000;
        let tick = (0..10_000)
            .find(|tick| {
                (deterministic_roll(
                    *tick,
                    recipe.output.item_index.max(0) as usize,
                    recipe.output_unique_id as usize,
                    100,
                ) < u64::from(recipe.chance))
                    == succeeds
            })
            .expect("bounded deterministic recipe branch exists");
        self.session
            .app
            .world_mut()
            .resource_mut::<super::super::resources::RuntimeClockResource>()
            .tick = tick;
        (recipe, slots)
    }
    fn craft(
        &mut self,
        recipe: &mir2_game_data::CrystalRecipe,
        slots: Vec<i32>,
    ) -> Vec<ServerPacket> {
        self.session.handle_packet(ClientPacket::CraftItem {
            unique_id: recipe.output_unique_id,
            count: 1,
            slots,
        })
    }
}
pub(in crate::runtime) fn inventory_image(session: &SimulationSession) -> String {
    let inventory = session.app.world().resource::<InventoryResource>();
    format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}",
        inventory.inventory_items,
        inventory.belt_items,
        inventory.storage_items,
        inventory.equipment_items,
        inventory.reserved_item_unique_ids
    )
}

#[test]
fn craft_uid_failure_preserves_recipe_materials_gold_and_tool_durability() {
    let mut fixture = Fixture::new("known-failure");
    let (recipe, slots) = fixture.recipe(17, true);
    assert!(!recipe.tools.is_empty());
    fixture.session.save_active_character().unwrap();
    let before = inventory_image(&fixture.session);
    let file = fs::read(&fixture.path).unwrap();
    let high = fixture.store.high.load(Ordering::SeqCst);
    fixture.store.arm(1, false);
    let packets = fixture.craft(&recipe, slots);
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::CraftItem { success: false })));
    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. }
    )));
    assert_eq!(inventory_image(&fixture.session), before);
    assert_eq!(
        fixture
            .session
            .app
            .world()
            .resource::<PlayerRuntimeResource>()
            .gold,
        50_000
    );
    assert_eq!(fixture.store.high.load(Ordering::SeqCst), high);
    assert_eq!(fs::read(&fixture.path).unwrap(), file);
}

#[test]
fn craft_uid_unknown_burns_mock_identity_but_keeps_assets_and_freezes_source_clones() {
    let mut fixture = Fixture::new("unknown");
    let (recipe, slots) = fixture.recipe(17, true);
    let before = inventory_image(&fixture.session);
    let high = fixture.store.high.load(Ordering::SeqCst);
    fixture.store.arm(1, true);
    let packets = fixture.craft(&recipe, slots);
    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. }
    )));
    assert_eq!(inventory_image(&fixture.session), before);
    assert_eq!(
        fixture
            .session
            .app
            .world()
            .resource::<PlayerRuntimeResource>()
            .gold,
        50_000
    );
    assert_eq!(fixture.store.high.load(Ordering::SeqCst), high + 1);
    assert!(fixture.config.ensure_account_store_writable().is_err());
    assert!(fixture
        .config
        .clone()
        .ensure_account_store_writable()
        .is_err());
}

#[test]
fn craft_uid_missed_chance_keeps_original_attempt_cost_and_never_calls_issuer() {
    let mut fixture = Fixture::new("miss");
    let (recipe, slots) = fixture.recipe(17, false);
    let calls = fixture.store.calls();
    fixture.store.arm(1, false);
    let packets = fixture.craft(&recipe, slots);
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::CraftItem { success: true })));
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoseGold { gold } if *gold == recipe.gold)));
    assert!(!packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
    assert_eq!(fixture.store.calls(), calls);
    assert_eq!(
        fixture
            .session
            .app
            .world()
            .resource::<PlayerRuntimeResource>()
            .gold,
        50_000 - recipe.gold
    );
    let inventory = fixture.session.app.world().resource::<InventoryResource>();
    assert_eq!(inventory.inventory_items.len(), recipe.tools.len());
    assert!(inventory
        .inventory_items
        .iter()
        .all(|item| item.durability_current == Some(2000)));
}

#[test]
fn craft_uid_success_uses_new_identity_and_publishes_original_costs_once() {
    let mut fixture = Fixture::new("success");
    let (recipe, slots) = fixture.recipe(32, true);
    let high = fixture.store.high.load(Ordering::SeqCst);
    let packets = fixture.craft(&recipe, slots);
    let gained: Vec<_> = packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::GainedItem { item } => Some(item),
            _ => None,
        })
        .collect();
    assert_eq!(gained.len(), 1);
    assert_eq!(
        (gained[0].unique_id, gained[0].item_index, gained[0].count),
        (high + 1, recipe.output.item_index, recipe.output.count)
    );
    assert_eq!(
        fixture
            .session
            .app
            .world()
            .resource::<PlayerRuntimeResource>()
            .gold,
        50_000 - recipe.gold
    );
    assert_eq!(
        packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::LoseGold { .. }))
            .count(),
        1
    );
    assert_eq!(
        fixture.store.issued().last(),
        Some(&(high + 1, UserItemUidReason::Crafting))
    );
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::CraftItem { success: true })));
}

#[test]
fn craft_output_batch_second_issue_failure_burns_first_without_partial_source_publication() {
    let fixture = Fixture::new("multi-root-plan");
    let template = crystal_item_by_name("GoldOre").unwrap();
    assert_eq!(template.stack_size, 1);
    let inventory = fixture
        .session
        .app
        .world()
        .resource::<InventoryResource>()
        .clone();
    let before = inventory_image(&fixture.session);
    let high = fixture.store.high.load(Ordering::SeqCst);
    fixture.store.arm(2, false);
    // Actual CraftItem validates output <= one stack. This separately verifies
    // its output planner's complete multi-root transaction/failure boundary.
    assert!(plan_crystal_craft_output(&inventory, &template, 3).is_err());
    assert_eq!(inventory_image(&fixture.session), before);
    assert_eq!(fixture.store.high.load(Ordering::SeqCst), high + 1);
    let plan = plan_crystal_craft_output(&inventory, &template, 3).unwrap();
    assert_eq!(plan.changed.len(), 3);
    let ids: std::collections::BTreeSet<_> =
        plan.changed.iter().map(|item| item.unique_id).collect();
    assert_eq!(ids, (high + 2..=high + 4).collect());
    assert!(plan.changed.iter().all(|item| item.quantity == 1));
    assert_eq!(
        inventory_image(&fixture.session),
        before,
        "planning alone never publishes inventory"
    );
}
