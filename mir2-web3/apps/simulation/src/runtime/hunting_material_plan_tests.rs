//! Pure main-server planning checks after ordinary in-memory Login/StartGame.
//! The Bound marker below is explicit synthetic setup. No PostgreSQL source,
//! fee, permit transaction, gateway or Windows acceptance is claimed here.
use super::*;
use crate::config::{EquipmentSlot, ItemContainer, SimulationConfig};
use crate::runtime::equipment::equipment_state_from_item_state;
use crate::runtime::inventory::empty_slots_for_inventory_container;
use crate::runtime::items::hunting_item_binding::HuntingItemBinding;
use crate::runtime::items::{
    crystal_item_template_for_item_key, embedded_item_state_from_template,
    try_item_state_from_user_item,
};
use crate::runtime::resources::{RuntimeConfigResource, SessionResource};
use crate::runtime::session::SimulationSession;
use mir2_protocol::{ClientPacket, ServerPacket};
use serde_json::json;

const REALM: &str = "material-plan-fixture";
const INCARNATION: &str = "pg-character-v1:12345678-1234-4234-9234-123456789abc";
const OPERATION: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const QUOTE: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const VALUATION: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const LOT: &str = "4444444444444444444444444444444444444444444444444444444444444444";

fn identity() -> ConversionIdentity<'static> {
    ConversionIdentity {
        operation_id: OPERATION,
        quote_id: QUOTE,
        valuation_version: "synthetic-plan-prices-v1",
        valuation_digest: VALUATION,
        lot_id: LOT,
    }
}

fn started(account: &str) -> SimulationSession {
    let config = SimulationConfig::default();
    if account != "demo" {
        let mut store = config.account_store.lock().unwrap();
        let account_record = store.accounts.get("demo").unwrap().clone();
        assert!(store
            .accounts
            .insert(account.into(), account_record)
            .is_none());
    }
    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login {
        account_id: account.into(),
        password: "demo".into(),
    });
    assert!(
        login
            .iter()
            .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })),
        "{login:?}"
    );
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(
        start
            .iter()
            .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })),
        "{start:?}"
    );
    assert!(session
        .active_zone_join_snapshot("material-plan-fixture")
        .is_some());
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = Some(REALM.into());
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation = Some(INCARNATION.into());
    install(&mut session, Vec::new());
    session
}

fn install(session: &mut SimulationSession, items: Vec<ItemState>) {
    let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
    inventory.inventory_items = items;
    inventory.belt_items.clear();
    inventory.storage_items.clear();
    inventory.equipment_items.clear();
    inventory.reserved_item_unique_ids.clear();
    drop(inventory);
    let mut hero = session
        .app
        .world_mut()
        .resource_mut::<HeroInventoryResource>();
    hero.items.clear();
    hero.equipment.clear();
}

fn plain(key: &str, uid: u64, slot: u8, quantity: u32) -> ItemState {
    let template = crystal_item_template_for_item_key(key).unwrap();
    let mut base = embedded_item_state_from_template(&template, ItemContainer::Bag1, slot);
    base.unique_id = uid;
    base.quantity = quantity;
    let mut wire = try_user_item_from_item_state(&base).unwrap();
    assert_eq!(wire.item_index, template.item_index);
    wire.awake_type = 2;
    wire.awake_values = vec![7, 11];
    wire.refined_value = 3;
    wire.refine_added = 4;
    wire.refine_success_chance = 17;
    wire.gm_made = true;
    let mut hydrated = try_item_state_from_user_item(base, &wire).unwrap();
    hydrated.name = format!("Fixture {key}");
    hydrated.description = "complete server description survives conversion".into();
    assert!(hydrated.user_item_metadata.is_some());
    assert_eq!(
        crystal_item_index_for_item_state(&hydrated),
        template.item_index
    );
    assert_eq!(try_user_item_from_item_state(&hydrated).unwrap(), wire);
    validate_committed_item_state_carrier(&hydrated).unwrap();
    hydrated
}

fn bound(session: &SimulationSession, key: &str, uid: u64, slot: u8, quantity: u32) -> ItemState {
    let mut item = plain(key, uid, slot, quantity);
    item.economic_binding = Some(
        HuntingItemBinding::for_current_owner(
            session.app.world(),
            "quest-once-v1:2110001:synthetic-material-plan",
        )
        .unwrap(),
    );
    item
}

fn inventory_debug(session: &SimulationSession) -> String {
    format!("{:?}", session.app.world().resource::<InventoryResource>())
}

fn checkpoint(session: &SimulationSession) -> CharacterSaveRecord {
    session.active_character_checkpoint().unwrap()
}

fn decoded_items(save: &CharacterSaveRecord) -> Vec<ItemState> {
    save.inventory_items_json
        .iter()
        .map(|value| serde_json::from_str(value).unwrap())
        .collect()
}

fn assert_converted_clone(output: &ItemState, original: &ItemState) {
    assert!(output.economic_binding.is_none());
    let provenance = serde_json::to_value(output.economic_conversion.as_ref().unwrap()).unwrap();
    assert_eq!(provenance["operationId"], OPERATION);
    assert_eq!(provenance["quoteId"], QUOTE);
    assert_eq!(provenance["inputUniqueId"], json!(original.unique_id));
    assert_eq!(
        provenance["sourceBindingDigest"],
        source_binding_digest(original).unwrap()
    );
    let mut expected = serde_json::to_value(original).unwrap();
    expected.as_object_mut().unwrap().remove("economic_binding");
    expected["economic_conversion"] = provenance;
    expected["unique_id"] = json!(output.unique_id);
    expected["quantity"] = json!(output.quantity);
    expected["container"] = serde_json::to_value(output.container).unwrap();
    expected["slot"] = json!(output.slot);
    assert_eq!(serde_json::to_value(output).unwrap(), expected);
}

#[test]
fn hunting_material_plan_whole_preserves_full_carrier_and_uid_without_live_mutation() {
    let mut session = started("demo");
    let item = bound(&session, "red-potion", (i64::MAX as u64) + 77, 8, 7);
    install(&mut session, vec![item.clone()]);
    let live_before = inventory_debug(&session);
    let before = checkpoint(&session);
    let source = MaterialItemSource::capture(session.app.world(), item.unique_id, 7).unwrap();
    let quote = source.quote_source(before.revision);
    assert_eq!(quote.item_index, crystal_item_index_for_item_state(&item));
    assert!(quote.item_index > 0);
    assert_eq!(quote.input_count, 7);
    assert_eq!(quote.quantity, 7);
    let plan = source.plan(session.app.world(), identity()).unwrap();
    assert_eq!(plan.output_unique_id(), item.unique_id);
    let after = plan.checkpoint(&before).unwrap();
    let items = decoded_items(&after);
    assert_eq!(items.len(), 1);
    assert_converted_clone(&items[0], &item);
    assert_eq!(items[0].quantity, 7);
    assert_eq!(items[0].unique_id, item.unique_id);
    let mut restored = after.clone();
    restored.inventory_items_json = before.inventory_items_json.clone();
    assert_eq!(
        serde_json::to_value(restored).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(inventory_debug(&session), live_before);
    let mut published = session.app.world().resource::<InventoryResource>().clone();
    assert!(plan.apply_to_inventory(&mut published));
    assert_eq!(
        encode_state_vec(&published.inventory_items),
        after.inventory_items_json
    );
    let published_before_retry = format!("{published:?}");
    assert!(!plan.apply_to_inventory(&mut published));
    assert_eq!(format!("{published:?}"), published_before_retry);
    assert_eq!(inventory_debug(&session), live_before);
}

#[test]
fn hunting_material_plan_partial_retains_bound_remainder_and_allocates_separate_full_output() {
    let mut session = started("demo");
    let item = bound(&session, "red-potion", 201, 8, 7);
    let legacy = plain("red-potion", 202, 9, 2);
    install(&mut session, vec![item.clone(), legacy.clone()]);
    let before = checkpoint(&session);
    let live_before = inventory_debug(&session);
    let source = MaterialItemSource::capture(session.app.world(), item.unique_id, 3).unwrap();
    let plan = source.plan(session.app.world(), identity()).unwrap();
    assert_ne!(plan.output_unique_id(), 0);
    assert_ne!(plan.output_unique_id(), item.unique_id);
    assert_ne!(plan.output_unique_id(), legacy.unique_id);
    let after = plan.checkpoint(&before).unwrap();
    let items = decoded_items(&after);
    assert_eq!(
        items.len(),
        3,
        "converted output must not merge into legacy space"
    );
    let remaining = items
        .iter()
        .find(|entry| entry.unique_id == item.unique_id)
        .unwrap();
    let mut expected_remaining = item.clone();
    expected_remaining.quantity = 4;
    assert_eq!(
        serde_json::to_value(remaining).unwrap(),
        serde_json::to_value(expected_remaining).unwrap()
    );
    let old = items
        .iter()
        .find(|entry| entry.unique_id == legacy.unique_id)
        .unwrap();
    assert_eq!(
        serde_json::to_value(old).unwrap(),
        serde_json::to_value(&legacy).unwrap()
    );
    let output = items
        .iter()
        .find(|entry| entry.unique_id == plan.output_unique_id())
        .unwrap();
    assert_eq!(output.quantity, 3);
    assert_ne!((output.container, output.slot), (item.container, item.slot));
    assert_ne!(
        (output.container, output.slot),
        (legacy.container, legacy.slot)
    );
    assert_converted_clone(output, &item);
    assert_eq!(output.user_item_metadata, item.user_item_metadata);
    assert_eq!(try_user_item_from_item_state(output).unwrap().count, 3);
    assert_eq!(items.iter().map(|entry| entry.quantity).sum::<u32>(), 9);
    assert_eq!(inventory_debug(&session), live_before);
}

#[test]
fn hunting_material_plan_full_actual_bag_allows_whole_and_refuses_partial_without_merging() {
    let mut session = started("demo");
    let item = bound(&session, "red-potion", 301, 8, 5);
    install(&mut session, vec![item.clone()]);
    let inventory = session.app.world().resource::<InventoryResource>();
    let empty = empty_slots_for_inventory_container(
        &inventory.inventory_items,
        ItemContainer::Bag1,
        inventory.inventory_capacity,
    );
    assert!(!empty.is_empty());
    let mut full = inventory.inventory_items.clone();
    for (index, (container, slot)) in empty.into_iter().enumerate() {
        let mut filler = plain("red-potion", 10_000 + index as u64, slot, 1);
        filler.container = container;
        full.push(filler);
    }
    install(&mut session, full);
    let inventory = session.app.world().resource::<InventoryResource>();
    assert!(empty_slots_for_inventory_container(
        &inventory.inventory_items,
        ItemContainer::Bag1,
        inventory.inventory_capacity
    )
    .is_empty());
    let live_before = inventory_debug(&session);
    let whole = MaterialItemSource::capture(session.app.world(), item.unique_id, 5).unwrap();
    assert_eq!(
        whole
            .plan(session.app.world(), identity())
            .unwrap()
            .output_unique_id(),
        item.unique_id
    );
    let partial = MaterialItemSource::capture(session.app.world(), item.unique_id, 2).unwrap();
    assert!(partial.plan(session.app.world(), identity()).is_err());
    assert_eq!(inventory_debug(&session), live_before);
}

#[test]
fn hunting_material_plan_refuses_legacy_equipment_other_restrictions_children_and_wrong_owner() {
    let mut session = started("demo");
    let source = bound(&session, "red-potion", 401, 8, 3);
    let foreign = started("material-plan-foreign");
    let mut rental = source.clone();
    rental.rental_locked = true;
    rental.rental_owner_name = "Another".into();
    let mut soul = source.clone();
    soul.soul_bound_id = Some(42);
    let mut sealed = source.clone();
    sealed.sealed_next_time_binary_datetime = 1;
    let mut nested_base = source.clone();
    nested_base.economic_binding = None;
    let mut nested_wire = try_user_item_from_item_state(&nested_base).unwrap();
    nested_wire.slots = vec![Some(
        try_user_item_from_item_state(&plain("red-potion", 499, 0, 1)).unwrap(),
    )];
    let mut nested = try_item_state_from_user_item(nested_base, &nested_wire).unwrap();
    nested.economic_binding = source.economic_binding.clone();
    assert!(!nested.socketed.is_empty());
    for (label, invalid) in [
        ("legacy", plain("red-potion", 401, 8, 3)),
        (
            "already converted",
            convert_bound_root(session.app.world(), &source, identity()).unwrap(),
        ),
        ("equipment", bound(&session, "dagger", 401, 8, 1)),
        ("rental", rental),
        ("soul", soul),
        ("sealed", sealed),
        ("occupied child", nested),
        ("foreign owner", bound(&foreign, "red-potion", 401, 8, 3)),
    ] {
        install(&mut session, vec![invalid]);
        let before = inventory_debug(&session);
        assert!(
            MaterialItemSource::capture(session.app.world(), 401, 1).is_err(),
            "{label}"
        );
        assert_eq!(inventory_debug(&session), before, "{label}");
    }
    install(&mut session, vec![source]);
    for (uid, quantity) in [(0, 1), (401, 0), (401, 4), (401, u64::MAX)] {
        assert!(MaterialItemSource::capture(session.app.world(), uid, quantity).is_err());
    }
}

#[test]
fn hunting_material_plan_refuses_uid_and_slot_ambiguity_across_custody() {
    let mut session = started("demo");
    let source = bound(&session, "red-potion", 501, 8, 3);
    let same_slot = plain("red-potion", 502, 8, 1);
    let same_uid = plain("red-potion", 501, 9, 1);
    for duplicate in [same_slot, same_uid] {
        install(&mut session, vec![source.clone(), duplicate]);
        let before = inventory_debug(&session);
        assert!(MaterialItemSource::capture(session.app.world(), 501, 1).is_err());
        assert_eq!(inventory_debug(&session), before);
    }
    install(&mut session, vec![source.clone()]);
    let mut storage = plain("red-potion", 501, 0, 1);
    storage.container = ItemContainer::Storage;
    session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .storage_items
        .push(storage);
    assert!(MaterialItemSource::capture(session.app.world(), 501, 1).is_err());
    install(&mut session, vec![source]);
    session
        .app
        .world_mut()
        .resource_mut::<HeroInventoryResource>()
        .items
        .push(plain("red-potion", 501, 0, 1));
    assert!(MaterialItemSource::capture(session.app.world(), 501, 1).is_err());
    for target_is_alias in [true, false] {
        let mut target = bound(&session, "red-potion", 501, 0, 3);
        let mut other = plain("red-potion", 502, 0, 1);
        if target_is_alias {
            target.container = ItemContainer::Bag1;
            target.slot = 40;
            other.container = ItemContainer::Bag2;
            other.slot = 0;
        } else {
            target.container = ItemContainer::Bag2;
            target.slot = 0;
            other.container = ItemContainer::Bag1;
            other.slot = 40;
        }
        install(&mut session, vec![target.clone(), other]);
        session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_capacity = crate::config::CRYSTAL_MAX_INVENTORY_CAPACITY;
        let live_before = inventory_debug(&session);
        assert!(
            MaterialItemSource::capture(session.app.world(), 501, 1).is_err(),
            "logical slot alias must refuse, target_is_alias={target_is_alias}"
        );
        let before = checkpoint(&session);
        let input = MaterialConversionQuoteSource {
            item_index: crystal_item_index_for_item_state(&target),
            input_unique_id: 501,
            input_count: 3,
            quantity: 1,
            input_save_revision: before.revision,
            input_item_digest: item_digest(&target).unwrap(),
            source_binding_digest: source_binding_digest(&target).unwrap(),
        };
        assert!(
            validate_hunting_material_quote_source(
                REALM,
                "demo",
                INCARNATION,
                before.character.index,
                &before,
                &input
            )
            .is_err(),
            "a correct digest must not authorize a noncanonical/ambiguous slot"
        );
        assert_eq!(inventory_debug(&session), live_before);
    }
}

#[test]
fn hunting_material_plan_revalidates_full_preimage_and_typed_authority_before_publish() {
    let mut session = started("demo");
    let item = bound(&session, "red-potion", 601, 8, 3);
    let neighbour = plain("red-potion", 602, 9, 1);
    install(&mut session, vec![item.clone(), neighbour]);
    let before = checkpoint(&session);
    let source = MaterialItemSource::capture(session.app.world(), item.unique_id, 1).unwrap();
    let input = source.quote_source(before.revision);
    validate_hunting_material_quote_source(
        REALM,
        "demo",
        INCARNATION,
        before.character.index,
        &before,
        &input,
    )
    .unwrap();
    for mutation in 0..6 {
        let mut bad = source.quote_source(before.revision);
        match mutation {
            0 => bad.item_index = crystal_item_index_for_item_state(&plain("dagger", 700, 0, 1)),
            1 => bad.input_item_digest = "sha256:client-authored".into(),
            2 => bad.source_binding_digest = "9".repeat(64),
            3 => bad.input_count += 1,
            4 => bad.input_save_revision += 1,
            5 => bad.input_unique_id += 1,
            _ => unreachable!(),
        }
        assert!(validate_hunting_material_quote_source(
            REALM,
            "demo",
            INCARNATION,
            before.character.index,
            &before,
            &bad
        )
        .is_err());
    }
    for (realm, account, incarnation, index) in [
        ("wrong-realm", "demo", INCARNATION, before.character.index),
        (REALM, "wrong-account", INCARNATION, before.character.index),
        (
            REALM,
            "demo",
            "pg-character-v1:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            before.character.index,
        ),
        (REALM, "demo", INCARNATION, before.character.index + 1),
    ] {
        assert!(validate_hunting_material_quote_source(
            realm,
            account,
            incarnation,
            index,
            &before,
            &input
        )
        .is_err());
    }
    let plan = source.plan(session.app.world(), identity()).unwrap();
    for mutation in 0..6 {
        let mut changed = session.app.world().resource::<InventoryResource>().clone();
        match mutation {
            0 => changed.inventory_items[1].description.push_str(" changed"),
            1 => {
                let mut stored = plain("red-potion", 703, 0, 1);
                stored.container = ItemContainer::Storage;
                changed.storage_items.push(stored);
            }
            2 => {
                let mut belted = plain("red-potion", 704, 0, 1);
                belted.container = ItemContainer::Belt;
                changed.belt_items.push(belted);
            }
            3 => {
                changed.reserved_item_unique_ids.insert(705);
            }
            4 => {
                changed.inventory_capacity = (1..=crate::config::CRYSTAL_MAX_INVENTORY_CAPACITY)
                    .find(|capacity| {
                        *capacity != changed.inventory_capacity
                            && crate::config::is_valid_crystal_inventory_capacity(*capacity)
                    })
                    .unwrap()
            }
            5 => changed
                .equipment_items
                .push(equipment_state_from_item_state(
                    &plain("dagger", 706, 0, 1),
                    EquipmentSlot::Weapon,
                )),
            _ => unreachable!(),
        }
        let unchanged = format!("{changed:?}");
        assert!(
            !plan.apply_to_inventory(&mut changed),
            "preimage mutation {mutation}"
        );
        assert_eq!(format!("{changed:?}"), unchanged);
    }
    let mut changed_checkpoint = before.clone();
    let mut changed_items = decoded_items(&changed_checkpoint);
    changed_items[1].description.push_str(" changed");
    changed_checkpoint.inventory_items_json = encode_state_vec(&changed_items);
    assert!(plan.checkpoint(&changed_checkpoint).is_err());
    session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items[0]
        .user_item_metadata
        .as_mut()
        .unwrap()
        .refined_value += 1;
    assert!(source.plan(session.app.world(), identity()).is_err());
}
