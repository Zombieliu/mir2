//! Local item-mutation evidence for a future repair quote boundary.
//!
//! The two inventory resources own their item data, without interior mutation.
//! HeadlessRuntime observes Bevy's dirty markers between exclusive mutable World
//! borrows. A same-value write, ABA replacement or checkpoint rollback therefore
//! advances a separate, checked u64 even if the serialized item is unchanged.
//! Bevy's wrapping u32 is only a sensor for one borrow interval, never identity.
//!
//! No quote, protocol, debit or repair is authorized here. External account/DB
//! changes require an incarnation witness, canonical checkpoint CAS and the real
//! transaction boundary. A future caller must also recheck NPC/service/range and
//! consume its exact server-issued quote. Never reconstruct this witness from
//! saved state or client input, or bypass Bevy's inventory change detection.

use bevy_ecs::{component::Tick, prelude::World};
use rand_core::{OsRng, RngCore};

use super::super::resources::{HeroInventoryResource, InventoryResource};

/// Opaque, process-local evidence; deliberately has no serialization or public
/// constructor. A new HeadlessRuntime gets an independent random scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runtime) struct RepairMutationWitness {
    scope: [u8; 32],
    revision: u64,
}

#[derive(Debug)]
pub(super) struct RepairMutationTracker {
    scope: Option<[u8; 32]>,
    revision: Option<u64>,
    observed_tick: Tick,
    expected_tick: Tick,
    inventory_present: bool,
    hero_inventory_present: bool,
}

impl RepairMutationTracker {
    pub(super) fn new(world: &World) -> Self {
        Self::with_entropy(world, |bytes| OsRng.try_fill_bytes(bytes).is_ok())
    }

    fn with_entropy(world: &World, entropy: impl FnOnce(&mut [u8; 32]) -> bool) -> Self {
        let mut scope = [0; 32];
        let scope = (entropy(&mut scope) && scope != [0; 32]).then_some(scope);
        let tick = world.read_change_tick();
        Self {
            scope,
            revision: Some(0),
            observed_tick: tick,
            expected_tick: tick,
            inventory_present: world.contains_resource::<InventoryResource>(),
            hero_inventory_present: world.contains_resource::<HeroInventoryResource>(),
        }
    }

    /// Finish observing the preceding borrow, then give the next borrow its own
    /// Bevy tick. This uses the owned World directly, not HeadlessRuntime's gate.
    pub(super) fn before_mutable_access(&mut self, world: &mut World) {
        self.flush(world);
        world.increment_change_tick();
        self.expected_tick = world.read_change_tick();
    }

    pub(super) fn witness(&mut self, world: &World) -> Option<RepairMutationWitness> {
        self.flush(world);
        if !self.inventory_present {
            return None;
        }
        Some(RepairMutationWitness {
            scope: self.scope?,
            revision: self.revision?,
        })
    }

    fn flush(&mut self, world: &World) {
        let now = world.read_change_tick();
        // The gate owns change-tick progression. Unexpected direct progression
        // breaks the short-interval proof and permanently disables witnesses.
        if now != self.expected_tick {
            self.revision = None;
        }
        let inventory = world.get_resource_change_ticks::<InventoryResource>();
        let hero_inventory = world.get_resource_change_ticks::<HeroInventoryResource>();
        let changed = inventory.is_some() != self.inventory_present
            || hero_inventory.is_some() != self.hero_inventory_present
            || inventory.is_some_and(|ticks| ticks.is_changed(self.observed_tick, now))
            || hero_inventory.is_some_and(|ticks| ticks.is_changed(self.observed_tick, now));
        if changed {
            self.revision = self.revision.and_then(|revision| revision.checked_add(1));
        }
        self.inventory_present = inventory.is_some();
        self.hero_inventory_present = hero_inventory.is_some();
        self.observed_tick = now;
        self.expected_tick = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ItemContainer, SimulationConfig};
    use crate::runtime::equipment::{item_state_from_equipment_state, seed_equipment_items};
    use crate::runtime::resources::{set_runtime_tick, RuntimeClockResource};
    use crate::runtime::session::{HeadlessRuntime, SimulationSession};
    use mir2_protocol::ClientPacket;

    fn item() -> crate::runtime::items::ItemState {
        let mut item = item_state_from_equipment_state(
            seed_equipment_items()[0].clone(),
            ItemContainer::Bag1,
            30,
        );
        item.unique_id = 70_030;
        item.quantity = 1;
        item.durability_current = Some(100);
        item.durability_max = Some(1_000);
        item
    }

    fn runtime() -> HeadlessRuntime {
        let mut app = HeadlessRuntime::new();
        let mut inventory = InventoryResource::new(80);
        inventory.inventory_items.push(item());
        app.insert_resource(inventory);
        app.insert_resource(HeroInventoryResource::new());
        app.insert_resource(RuntimeClockResource::new());
        assert!(app.repair_mutation_witness().is_some());
        app
    }

    fn witness(app: &mut HeadlessRuntime) -> RepairMutationWitness {
        app.repair_mutation_witness().expect("local witness")
    }

    #[test]
    fn repair_mutation_idle_reads_and_clock_writes_preserve_witness() {
        let mut app = runtime();
        let before = witness(&mut app);
        for tick in 1..=16 {
            assert_eq!(
                app.world()
                    .resource::<InventoryResource>()
                    .inventory_items
                    .len(),
                1
            );
            let world = app.world_mut();
            assert_eq!(
                world
                    .resource_mut::<InventoryResource>()
                    .inventory_items
                    .len(),
                1
            );
            set_runtime_tick(world, tick);
        }
        assert_eq!(witness(&mut app), before);
        assert!(app.repair_mutation_witness_matches(&before));
    }

    #[test]
    fn repair_mutation_last_borrow_is_flushed_before_witness_or_match() {
        let mut app = runtime();
        let before = witness(&mut app);
        app.world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items[0]
            .quantity += 1;
        assert!(!app.repair_mutation_witness_matches(&before));
        let after = witness(&mut app);
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(witness(&mut app), after, "one write is counted once");
    }

    #[test]
    fn repair_mutation_same_value_item_write_invalidates_witness() {
        let mut app = runtime();
        let before = witness(&mut app);
        let quantity = app.world().resource::<InventoryResource>().inventory_items[0].quantity;
        app.world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items[0]
            .quantity = quantity;
        assert_ne!(witness(&mut app), before);
    }

    #[test]
    fn repair_mutation_same_value_hero_write_invalidates_witness() {
        let mut app = runtime();
        app.world_mut()
            .resource_mut::<HeroInventoryResource>()
            .items
            .push(item());
        let before = witness(&mut app);
        let quantity = app.world().resource::<HeroInventoryResource>().items[0].quantity;
        app.world_mut()
            .resource_mut::<HeroInventoryResource>()
            .items[0]
            .quantity = quantity;
        assert_ne!(witness(&mut app), before);
    }

    #[test]
    fn repair_mutation_same_borrow_aba_is_visible_despite_identical_items() {
        let mut app = runtime();
        let before = witness(&mut app);
        let encoded =
            serde_json::to_value(&app.world().resource::<InventoryResource>().inventory_items)
                .unwrap();
        let world = app.world_mut();
        let mut inventory = world.resource_mut::<InventoryResource>();
        let removed = inventory.inventory_items.remove(0);
        inventory.inventory_items.push(removed);
        drop(inventory);
        assert_eq!(
            serde_json::to_value(&world.resource::<InventoryResource>().inventory_items).unwrap(),
            encoded
        );
        assert_ne!(witness(&mut app), before);
    }

    #[test]
    fn repair_mutation_multiple_borrows_count_each_dirty_interval() {
        let mut app = runtime();
        let before = witness(&mut app);
        let original = app.world().resource::<InventoryResource>().inventory_items[0].clone();
        app.world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items[0]
            .quantity = 99;
        app.world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items[0] = original;
        let after = witness(&mut app);
        assert_eq!(after.revision, before.revision + 2);
    }

    #[test]
    fn repair_mutation_checkpoint_resource_restore_cannot_rewind_witness() {
        let mut app = runtime();
        let saved_inventory = app.world().resource::<InventoryResource>().clone();
        let saved_tick = app.world().resource::<RuntimeClockResource>().tick;
        let before = witness(&mut app);
        app.world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items
            .clear();
        let world = app.world_mut();
        *world.resource_mut::<InventoryResource>() = saved_inventory;
        set_runtime_tick(world, saved_tick);
        let after = witness(&mut app);
        assert_eq!(after.revision, before.revision + 2);
        assert_eq!(
            app.world().resource::<RuntimeClockResource>().tick,
            saved_tick
        );
        assert!(!app.repair_mutation_witness_matches(&before));
    }

    #[test]
    fn repair_mutation_inserting_identical_resource_invalidates_witness() {
        let mut app = runtime();
        let before = witness(&mut app);
        let inventory = app.world().resource::<InventoryResource>().clone();
        app.insert_resource(inventory);
        assert_ne!(witness(&mut app), before);
    }

    #[test]
    fn repair_mutation_missing_inventory_fails_closed_and_reinsert_retires_old_witness() {
        let mut app = runtime();
        let before = witness(&mut app);
        let inventory = app
            .world_mut()
            .remove_resource::<InventoryResource>()
            .unwrap();
        assert!(app.repair_mutation_witness().is_none());
        assert!(!app.repair_mutation_witness_matches(&before));
        app.insert_resource(inventory);
        assert_ne!(witness(&mut app), before);
        assert!(!app.repair_mutation_witness_matches(&before));
    }

    #[test]
    fn repair_mutation_remove_and_reinsert_in_same_borrow_invalidates_witness() {
        let mut app = runtime();
        let before = witness(&mut app);
        let world = app.world_mut();
        let inventory = world.remove_resource::<InventoryResource>().unwrap();
        world.insert_resource(inventory);
        assert_ne!(witness(&mut app), before);
    }

    #[test]
    fn repair_mutation_hero_resource_presence_changes_invalidate_witness() {
        let mut app = runtime();
        let before = witness(&mut app);
        let hero = app
            .world_mut()
            .remove_resource::<HeroInventoryResource>()
            .unwrap();
        let absent = witness(&mut app);
        assert_ne!(absent, before);
        app.insert_resource(hero);
        assert_ne!(witness(&mut app), absent);
    }

    #[test]
    fn repair_mutation_new_empty_runtime_cannot_issue_witness() {
        let mut app = HeadlessRuntime::new();
        assert!(app.repair_mutation_witness().is_none());
        app.insert_resource(InventoryResource::new(80));
        assert!(app.repair_mutation_witness().is_some());
    }

    #[test]
    fn repair_mutation_separate_runtime_scopes_reject_each_others_witnesses() {
        let mut first = runtime();
        let mut second = runtime();
        let before = witness(&mut first);
        assert_ne!(witness(&mut second), before);
        assert!(!second.repair_mutation_witness_matches(&before));
    }

    #[test]
    fn repair_mutation_exhaustion_cannot_be_repaired_by_restore_or_insert() {
        let mut app = runtime();
        app.repair_mutation.revision = Some(u64::MAX);
        let before = witness(&mut app);
        let inventory = app.world().resource::<InventoryResource>().clone();
        app.world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items
            .clear();
        assert!(app.repair_mutation_witness().is_none());
        app.insert_resource(inventory);
        set_runtime_tick(app.world_mut(), 0);
        assert!(app.repair_mutation_witness().is_none());
        assert!(!app.repair_mutation_witness_matches(&before));
    }

    #[test]
    fn repair_mutation_entropy_failure_and_zero_scope_stay_unavailable() {
        for produce_zero in [false, true] {
            let mut app = runtime();
            app.repair_mutation =
                RepairMutationTracker::with_entropy(app.world(), |_| produce_zero);
            assert!(app.repair_mutation_witness().is_none());
            let inventory = app.world().resource::<InventoryResource>().clone();
            app.insert_resource(inventory);
            assert!(app.repair_mutation_witness().is_none());
        }
    }

    #[test]
    fn repair_mutation_unexpected_bevy_tick_progression_permanently_fails_closed() {
        let mut app = runtime();
        let before = witness(&mut app);
        app.world_mut().increment_change_tick();
        assert!(app.repair_mutation_witness().is_none());
        let inventory = app.world().resource::<InventoryResource>().clone();
        app.insert_resource(inventory);
        assert!(!app.repair_mutation_witness_matches(&before));
        assert!(app.repair_mutation_witness().is_none());
    }

    #[test]
    fn repair_mutation_real_shared_personal_idle_ticks_preserve_witness() {
        let mut session = SimulationSession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert!(session.active_identity().is_some());
        let before = witness(&mut session.app);
        for _ in 0..5 {
            session.tick_shared_zone_personal_state();
        }
        assert_eq!(witness(&mut session.app), before);
    }

    #[test]
    fn repair_mutation_real_checkpoint_restore_retires_witness_without_changing_clock() {
        let mut session = SimulationSession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let checkpoint = session.active_character_checkpoint().expect("active save");
        let tick = session.app.world().resource::<RuntimeClockResource>().tick;
        let before = witness(&mut session.app);
        session
            .restore_active_character_checkpoint(&checkpoint)
            .unwrap();
        assert_eq!(
            session.app.world().resource::<RuntimeClockResource>().tick,
            tick
        );
        assert!(!session.app.repair_mutation_witness_matches(&before));
    }
}
