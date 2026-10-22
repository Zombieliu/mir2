//! Native repair owner turns have their own local custody. They do not create
//! a purchase Begin, producer, ActorKey, server revision, or wallet authority.
//! Only the exclusive complete-model commit below can construct Applied.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use bevy::prelude::{Resource, World};
use serde_json::Value;

use crate::npc_purchase_economy::{
    commit_complete_owner_models, preserve_repair_hero_presentation,
    retire_purchase_source_for_repair, Decoded, NativeHeroOwnerEpoch, NativeHeroOwnerGate,
    NativeNpcEconomyError, NativeNpcEconomyProjection,
};

const MAX_BUNDLE_BYTES: usize = 64 * 1024 * 1024;
const MAX_APPLIED: usize = 32;

pub type NativeHuntingRepairError = NativeNpcEconomyError;

/// The host's actual NativeCommandStamp, captured after authenticated entry.
/// These are local lifetime counters, not server-side character identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeHuntingRepairEpoch {
    pub run: u64,
    pub connection: u64,
    pub procedure: u64,
    pub owner_epoch: u64,
    pub scene_epoch: u64,
    pub cancellation: u64,
    pub actor: u32,
    pub map: i32,
}

impl NativeHuntingRepairEpoch {
    fn valid(self) -> bool {
        self.run != 0
            && self.connection != 0
            && self.actor != 0
            && ![
                self.run,
                self.connection,
                self.procedure,
                self.owner_epoch,
                self.scene_epoch,
                self.cancellation,
            ]
            .contains(&u64::MAX)
    }

    fn hero_epoch(self) -> NativeHeroOwnerEpoch {
        NativeHeroOwnerEpoch {
            run: self.run,
            connection: self.connection,
            procedure: self.procedure,
            owner_epoch: self.owner_epoch,
            scene_epoch: self.scene_epoch,
            cancellation: self.cancellation,
            actor: self.actor,
            map: self.map,
        }
    }
}

fn valid_quote_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && value.bytes().any(|b| b != b'0')
}

fn owner_actor(owner: &Value) -> Option<u64> {
    let value = owner.get("playerObjectId")?;
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}

#[derive(Debug)]
struct Custody {
    active: bool,
    epoch: NativeHuntingRepairEpoch,
    last_prepared: u64,
    last_applied: u64,
    last_applied_quote: Option<Arc<str>>,
    applied: VecDeque<NativeHuntingRepairApplied>,
}

/// One gate per physical connection and active actor/scene lifetime. The host
/// retires it before owner changes; reset admission also retires queued gates.
#[derive(Debug, Clone)]
pub struct NativeHuntingRepairGate {
    custody: Arc<Mutex<Custody>>,
}

impl NativeHuntingRepairGate {
    pub fn new(epoch: NativeHuntingRepairEpoch) -> Result<Self, NativeHuntingRepairError> {
        if !epoch.valid() {
            return Err(NativeNpcEconomyError::Correlation);
        }
        Ok(Self {
            custody: Arc::new(Mutex::new(Custody {
                active: true,
                epoch,
                last_prepared: 0,
                last_applied: 0,
                last_applied_quote: None,
                applied: VecDeque::new(),
            })),
        })
    }

    pub fn epoch(&self) -> Option<NativeHuntingRepairEpoch> {
        self.custody
            .lock()
            .ok()
            .filter(|state| state.active)
            .map(|state| state.epoch)
    }

    pub fn retire(&self) {
        if let Ok(mut state) = self.custody.lock() {
            state.active = false;
            state.applied.clear();
        }
    }

    pub fn is_current(&self, epoch: NativeHuntingRepairEpoch) -> bool {
        self.custody
            .lock()
            .is_ok_and(|state| state.active && state.epoch == epoch)
    }

    /// Call only after strict request correlation and a Committed response
    /// with reloadRequired=false and a non-null current owner snapshot. No
    /// server authority is synthesized here. Preparation never acknowledges it.
    pub fn prepare(
        &self,
        request_id: u64,
        quote_id: &str,
        projection: NativeNpcEconomyProjection,
    ) -> Result<NativeHuntingRepairBundle, NativeHuntingRepairError> {
        if request_id == 0 || !valid_quote_id(quote_id) {
            return Err(NativeNpcEconomyError::Correlation);
        }
        if projection
            .retained_bytes()
            .saturating_add(quote_id.len())
            .saturating_add(std::mem::size_of::<NativeHuntingRepairBundle>())
            > MAX_BUNDLE_BYTES
        {
            return Err(NativeNpcEconomyError::TooLarge);
        }
        let decoded = Decoded::decode(&projection)?;
        let mut state = self
            .custody
            .lock()
            .map_err(|_| NativeNpcEconomyError::Retired)?;
        if !state.active {
            return Err(NativeNpcEconomyError::Retired);
        }
        if request_id <= state.last_prepared
            || owner_actor(&decoded.owner) != Some(u64::from(state.epoch.actor))
        {
            return Err(NativeNpcEconomyError::Correlation);
        }
        state.last_prepared = request_id;
        Ok(NativeHuntingRepairBundle {
            gate: self.clone(),
            epoch: state.epoch,
            request_id,
            quote_id: Arc::from(quote_id),
            projection: Arc::new(projection),
            hero_owner_gate: None,
        })
    }

    /// The single transport owner consumes the exact original request/quote
    /// completion only after every complete model changed in the actual World.
    pub fn try_recv_applied(&self) -> Option<NativeHuntingRepairApplied> {
        let mut state = self.custody.lock().ok()?;
        if !state.active {
            return None;
        }
        state.applied.pop_front()
    }

    /// Reauthorize a popped completion under its original custody lock. The
    /// host holds its physical-owner fence before entering this method. Reset
    /// retirement cannot interleave with the final client completion callback.
    /// The callback must not re-enter or retire this same Runtime gate.
    pub fn with_current_applied<T>(
        &self,
        applied: &NativeHuntingRepairApplied,
        complete: impl FnOnce() -> T,
    ) -> Option<T> {
        let state = self.custody.lock().ok()?;
        if !state.active || state.epoch != applied.epoch
            || state.last_applied != applied.request_id
            || state.last_applied_quote.as_deref() != Some(applied.quote_id())
        {
            return None;
        }
        let completed = complete();
        drop(state);
        Some(completed)
    }

    pub(crate) fn same_gate(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.custody, &other.custody)
    }
}

#[derive(Debug, Clone)]
pub struct NativeHuntingRepairBundle {
    gate: NativeHuntingRepairGate,
    epoch: NativeHuntingRepairEpoch,
    request_id: u64,
    quote_id: Arc<str>,
    projection: Arc<NativeNpcEconomyProjection>,
    hero_owner_gate: Option<NativeHeroOwnerGate>,
}

impl NativeHuntingRepairBundle {
    /// Presentation clocks can be preserved only from the exact same physical
    /// owner tuple. This token cannot acknowledge a repair or lend a producer.
    pub fn with_hero_owner_gate(
        mut self,
        gate: &NativeHeroOwnerGate,
    ) -> Result<Self, NativeHuntingRepairError> {
        if gate.epoch() != self.epoch.hero_epoch() {
            return Err(NativeNpcEconomyError::Correlation);
        }
        if gate.with_active(|| ()).is_none() || !self.gate.is_current(self.epoch) {
            return Err(NativeNpcEconomyError::Retired);
        }
        self.hero_owner_gate = Some(gate.clone());
        if self.retained_bytes() > MAX_BUNDLE_BYTES {
            return Err(NativeNpcEconomyError::TooLarge);
        }
        Ok(self)
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.projection
            .retained_bytes()
            .saturating_add(self.quote_id.len())
            .saturating_add(std::mem::size_of::<Self>())
            .saturating_add(
                self.hero_owner_gate
                    .as_ref()
                    .map_or(0, |_| std::mem::size_of::<NativeHeroOwnerGate>()),
            )
    }

    pub(crate) fn gate(&self) -> &NativeHuntingRepairGate {
        &self.gate
    }
    pub(crate) fn is_current(&self) -> bool {
        self.gate.is_current(self.epoch)
    }
    pub(crate) fn retire(&self) {
        self.gate.retire();
    }
}

/// No public constructor or deserializer: a local queue admission, receipt,
/// validation pass, or UI state cannot manufacture an application completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeHuntingRepairApplied {
    epoch: NativeHuntingRepairEpoch,
    request_id: u64,
    quote_id: Arc<str>,
}

impl NativeHuntingRepairApplied {
    pub fn epoch(&self) -> NativeHuntingRepairEpoch {
        self.epoch
    }
    pub fn request_id(&self) -> u64 {
        self.request_id
    }
    pub fn quote_id(&self) -> &str {
        &self.quote_id
    }
}

/// Lossless current owner source is retained independently of purchase-v1.
#[derive(Debug, Clone, Resource)]
pub struct NativeHuntingRepairSource {
    applied: NativeHuntingRepairApplied,
    projection: Arc<NativeNpcEconomyProjection>,
    owner: Arc<Value>,
    gate: NativeHuntingRepairGate,
    reset_revision: u64,
    scene_revision: u64,
}

impl NativeHuntingRepairSource {
    pub fn owner(&self) -> &Value {
        &self.owner
    }
    pub fn owner_json(&self) -> &str {
        &self.projection.owner_json
    }
    pub fn applied(&self) -> &NativeHuntingRepairApplied {
        &self.applied
    }
}

pub(crate) fn clear_native_hunting_repair_source(world: &mut World) {
    if let Some(source) = world.remove_resource::<NativeHuntingRepairSource>() {
        source.gate.retire();
    }
}

/// Invoked by the single owner-barrier consumer, never as a second competing
/// system. The ordinary typed consumers have finished the entire FIFO prefix.
pub(crate) fn apply_pending_native_hunting_repair(world: &mut World) {
    let reset = world
        .get_resource::<mir2_client_bevy::pending_operations::SessionResetRevision>()
        .map_or(0, |r| r.0);
    let scene = world
        .get_resource::<crate::SceneResetRevision>()
        .map_or(0, |r| r.0);
    if world
        .get_resource::<NativeHuntingRepairSource>()
        .is_some_and(|source| source.reset_revision != reset || source.scene_revision != scene)
    {
        clear_native_hunting_repair_source(world);
    }
    let bundle = world
        .get_resource::<crate::native_ingest::NativeInbound>()
        .and_then(|native| native.take_hunting_repair_front());
    if let Some(bundle) = bundle {
        let _ = apply_native_hunting_repair_bundle(world, bundle);
        return;
    }
    #[cfg(feature = "native-npc-economy")]
    crate::hunting_material_economy::apply_pending_native_hunting_material(world);
}

/// Exclusive application for the real main-thread World. This deliberately
/// accepts no external Applied flag or server purchase identity.
pub fn apply_native_hunting_repair_bundle(
    world: &mut World,
    bundle: NativeHuntingRepairBundle,
) -> bool {
    let Ok(mut decoded) = Decoded::decode(&bundle.projection) else {
        return false;
    };
    if owner_actor(&decoded.owner) != Some(u64::from(bundle.epoch.actor)) {
        return false;
    }
    let Ok(mut custody) = bundle.gate.custody.lock() else {
        return false;
    };
    if !custody.active
        || custody.epoch != bundle.epoch
        || bundle.request_id <= custody.last_applied
        || bundle.request_id > custody.last_prepared
        || custody.applied.len() >= MAX_APPLIED
    {
        return false;
    }
    let hero_gate = bundle.hero_owner_gate.clone();
    let commit = || {
        preserve_repair_hero_presentation(world, &mut decoded, hero_gate.as_ref());
        retire_purchase_source_for_repair(world);
        #[cfg(feature = "native-npc-economy")]
        crate::hunting_material_economy::clear_native_hunting_material_source(world);
        if let Some(prior) = world.remove_resource::<NativeHuntingRepairSource>() {
            if !prior.gate.same_gate(&bundle.gate) {
                prior.gate.retire();
            }
        }
        let owner = commit_complete_owner_models(world, decoded, hero_gate.clone());
        let applied = NativeHuntingRepairApplied {
            epoch: bundle.epoch,
            request_id: bundle.request_id,
            quote_id: Arc::clone(&bundle.quote_id),
        };
        let reset_revision = world
            .get_resource::<mir2_client_bevy::pending_operations::SessionResetRevision>()
            .map_or(0, |r| r.0);
        let scene_revision = world
            .get_resource::<crate::SceneResetRevision>()
            .map_or(0, |r| r.0);
        world.insert_resource(NativeHuntingRepairSource {
            applied: applied.clone(),
            projection: Arc::clone(&bundle.projection),
            owner: Arc::new(owner),
            gate: bundle.gate.clone(),
            reset_revision,
            scene_revision,
        });
        custody.last_applied = bundle.request_id;
        custody.last_applied_quote = Some(Arc::clone(&bundle.quote_id));
        custody.applied.push_back(applied);
        true
    };
    if let Some(gate) = hero_gate.as_ref() {
        gate.with_active(commit).unwrap_or(false)
    } else {
        commit()
    }
}

#[cfg(all(test, feature = "native-npc-economy"))]
mod tests {
    use super::*;
    use mir2_client_bevy::{hero_model::HeroModel, inventory::InventoryModel};

    fn fixture() -> (NativeHuntingRepairGate, NativeNpcEconomyProjection) {
        let (_, _, projection) = crate::npc_purchase_economy::tests::fixture();
        let owner: Value = serde_json::from_str(&projection.owner_json).unwrap();
        let gate = NativeHuntingRepairGate::new(NativeHuntingRepairEpoch {
            run: 1,
            connection: 1,
            procedure: 2,
            owner_epoch: 3,
            scene_epoch: 4,
            cancellation: 0,
            actor: u32::try_from(owner_actor(&owner).unwrap()).unwrap(),
            map: 0,
        })
        .unwrap();
        (gate, projection)
    }

    fn quote() -> String {
        "ab".repeat(32)
    }

    #[test]
    fn native_hunting_repair_prepare_never_acknowledges_or_changes_world() {
        let (gate, projection) = fixture();
        let mut world = World::new();
        let original = projection.owner_json.clone();
        let bundle = gate.prepare(1, &quote(), projection).unwrap();
        assert!(gate.try_recv_applied().is_none());
        assert!(!world.contains_resource::<InventoryModel>());
        assert!(apply_native_hunting_repair_bundle(
            &mut world,
            bundle.clone()
        ));
        let applied = gate.try_recv_applied().unwrap();
        assert_eq!(applied.epoch(), gate.epoch().unwrap());
        assert_eq!(applied.request_id(), 1);
        assert_eq!(applied.quote_id(), quote());
        assert_eq!(
            world.resource::<NativeHuntingRepairSource>().owner_json(),
            original
        );
        assert!(
            world.contains_resource::<InventoryModel>() && world.contains_resource::<HeroModel>()
        );
        assert!(!world.contains_resource::<crate::npc_purchase_economy::NativeNpcEconomySource>());
        assert!(!apply_native_hunting_repair_bundle(&mut world, bundle));
        assert!(gate.try_recv_applied().is_none());
    }

    #[test]
    fn native_hunting_repair_incomplete_wrong_actor_and_identity_reject_before_mutation() {
        let (gate, projection) = fixture();
        let mut incomplete = projection.clone();
        incomplete.mail_json = "{}".into();
        assert!(gate.prepare(1, &quote(), incomplete).is_err());
        assert!(gate.prepare(0, &quote(), projection.clone()).is_err());
        assert!(gate
            .prepare(1, &"0".repeat(64), projection.clone())
            .is_err());
        assert!(gate
            .prepare(1, &"AB".repeat(32), projection.clone())
            .is_err());
        let wrong = NativeHuntingRepairGate::new(NativeHuntingRepairEpoch {
            actor: gate.epoch().unwrap().actor.checked_add(1).unwrap(),
            ..gate.epoch().unwrap()
        })
        .unwrap();
        assert!(wrong.prepare(1, &quote(), projection.clone()).is_err());
        assert!(gate.try_recv_applied().is_none());
        let _ = gate.prepare(1, &quote(), projection.clone()).unwrap();
        assert!(gate.prepare(1, &quote(), projection).is_err());
    }

    #[test]
    fn native_hunting_repair_retirement_refuses_prepared_bundle_and_clears_completion() {
        let (gate, projection) = fixture();
        let mut world = World::new();
        let first = gate.prepare(1, &quote(), projection.clone()).unwrap();
        assert!(apply_native_hunting_repair_bundle(&mut world, first));
        let second = gate.prepare(2, &quote(), projection.clone()).unwrap();
        gate.retire();
        assert!(gate.try_recv_applied().is_none());
        let before = world
            .resource::<NativeHuntingRepairSource>()
            .owner_json()
            .to_owned();
        assert!(!apply_native_hunting_repair_bundle(&mut world, second));
        assert_eq!(
            world.resource::<NativeHuntingRepairSource>().owner_json(),
            before
        );
        assert!(gate.prepare(3, &quote(), projection).is_err());
    }

    #[test]
    fn native_hunting_repair_popped_completion_is_invalid_after_world_reset() {
        let (gate, projection) = fixture();
        let mut world = World::new();
        let bundle = gate.prepare(1, &quote(), projection).unwrap();
        assert!(apply_native_hunting_repair_bundle(&mut world, bundle));
        let applied = gate.try_recv_applied().unwrap();
        world.insert_resource(crate::SceneResetRevision(1));
        apply_pending_native_hunting_repair(&mut world);
        assert!(!world.contains_resource::<NativeHuntingRepairSource>());
        let mut completed = false;
        assert_eq!(gate.with_current_applied(&applied, || { completed = true; }), None);
        assert!(!completed);
        assert!(gate.epoch().is_none());
    }

    #[test]
    fn native_hunting_repair_final_callback_holds_custody_against_retirement() {
        use std::sync::{mpsc, TryLockError};
        use std::time::Duration;
        let (gate, projection) = fixture();
        let mut world = World::new();
        let bundle = gate.prepare(1, &quote(), projection).unwrap();
        assert!(apply_native_hunting_repair_bundle(&mut world, bundle));
        let applied = gate.try_recv_applied().unwrap();
        let (begin_tx, begin_rx) = mpsc::channel();
        let (attempt_tx, attempt_rx) = mpsc::channel();
        let (retired_tx, retired_rx) = mpsc::channel();
        std::thread::scope(|scope| {
            let retiring_gate = gate.clone();
            let retiring = scope.spawn(move || {
                begin_rx.recv_timeout(Duration::from_secs(2)).unwrap();
                attempt_tx.send(()).unwrap();
                retiring_gate.retire();
                retired_tx.send(()).unwrap();
            });
            assert_eq!(gate.with_current_applied(&applied, || {
                begin_tx.send(()).unwrap();
                attempt_rx.recv_timeout(Duration::from_secs(2)).unwrap();
                assert!(matches!(gate.custody.try_lock(), Err(TryLockError::WouldBlock)));
                assert!(matches!(retired_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
                "completed-under-original-gate"
            }), Some("completed-under-original-gate"));
            retired_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            retiring.join().unwrap();
        });
        assert!(gate.with_current_applied(&applied, || ()).is_none());
    }

    #[test]
    fn native_hunting_repair_completion_rejects_a_superseded_applied_request() {
        let (gate, projection) = fixture();
        let mut world = World::new();
        let first = gate.prepare(1, &quote(), projection.clone()).unwrap();
        assert!(apply_native_hunting_repair_bundle(&mut world, first));
        let old = gate.try_recv_applied().unwrap();
        let second = gate.prepare(2, &quote(), projection).unwrap();
        assert!(apply_native_hunting_repair_bundle(&mut world, second));
        let current = gate.try_recv_applied().unwrap();
        let mut old_completed = false;
        assert!(gate.with_current_applied(&old, || { old_completed = true; }).is_none());
        assert!(!old_completed);
        assert_eq!(gate.with_current_applied(&current, || current.request_id()), Some(2));
    }

    #[test]
    fn native_hunting_repair_display_gate_requires_entire_original_epoch() {
        let (gate, projection) = fixture();
        let epoch = gate.epoch().unwrap();
        let wrong = NativeHeroOwnerGate::new(NativeHeroOwnerEpoch {
            cancellation: 1,
            ..epoch.hero_epoch()
        })
        .unwrap();
        assert!(gate
            .prepare(1, &quote(), projection.clone())
            .unwrap()
            .with_hero_owner_gate(&wrong)
            .is_err());
        let display = NativeHeroOwnerGate::new(epoch.hero_epoch()).unwrap();
        let bundle = gate
            .prepare(2, &quote(), projection)
            .unwrap()
            .with_hero_owner_gate(&display)
            .unwrap();
        display.retire();
        let mut world = World::new();
        assert!(!apply_native_hunting_repair_bundle(&mut world, bundle));
        assert!(!world.contains_resource::<InventoryModel>());
        assert!(gate.try_recv_applied().is_none());
    }

    #[test]
    fn native_hunting_repair_preserves_only_the_actual_current_display_clock() {
        let (gate, projection) = fixture();
        let display = NativeHeroOwnerGate::new(gate.epoch().unwrap().hero_epoch()).unwrap();
        let owner: Value = serde_json::from_str(&projection.owner_json).unwrap();
        let mut world = World::new();
        world.insert_resource(HeroModel {
            session_epoch: 17,
            hero_generation: 4,
            revision: 10,
            ..Default::default()
        });
        assert!(crate::npc_purchase_economy::apply_native_hero_owner_update(
            &mut world,
            display.prepare(&owner).unwrap()
        ));
        let before = world.resource::<HeroModel>().clone();
        let bundle = gate
            .prepare(1, &quote(), projection)
            .unwrap()
            .with_hero_owner_gate(&display)
            .unwrap();
        assert!(apply_native_hunting_repair_bundle(&mut world, bundle));
        let applied = world.resource::<HeroModel>();
        assert_eq!(applied.session_epoch, before.session_epoch);
        assert_eq!(applied.hero_generation, before.hero_generation);
        assert!(applied.revision > before.revision);
        assert!(gate.try_recv_applied().is_some());
    }

    #[test]
    fn native_hunting_repair_retires_the_previous_purchase_source_without_fabricating_it() {
        let (purchase, witness, projection) = crate::npc_purchase_economy::tests::fixture();
        let mut world = World::new();
        let first = purchase.prepare(witness, projection.clone()).unwrap();
        assert!(crate::npc_purchase_economy::apply_native_npc_economy_bundle(&mut world, first));
        let late = purchase.prepare(witness, projection).unwrap();
        let (repair, projection) = fixture();
        let bundle = repair.prepare(1, &quote(), projection).unwrap();
        assert!(apply_native_hunting_repair_bundle(&mut world, bundle));
        assert!(!world.contains_resource::<crate::npc_purchase_economy::NativeNpcEconomySource>());
        assert!(!crate::npc_purchase_economy::apply_native_npc_economy_bundle(&mut world, late));
        assert!(world.contains_resource::<NativeHuntingRepairSource>());
        assert_eq!(
            world
                .resource::<NativeHuntingRepairSource>()
                .applied()
                .request_id(),
            1
        );
    }

    #[test]
    fn native_hunting_repair_completion_capacity_refuses_all_mutation() {
        let (gate, projection) = fixture();
        let mut world = World::new();
        for id in 1..=MAX_APPLIED as u64 {
            let bundle = gate.prepare(id, &quote(), projection.clone()).unwrap();
            assert!(apply_native_hunting_repair_bundle(&mut world, bundle));
        }
        let bundle = gate
            .prepare(MAX_APPLIED as u64 + 1, &quote(), projection)
            .unwrap();
        assert!(!apply_native_hunting_repair_bundle(
            &mut world,
            bundle.clone()
        ));
        assert_eq!(
            world
                .resource::<NativeHuntingRepairSource>()
                .applied()
                .request_id(),
            MAX_APPLIED as u64
        );
        assert!(gate.try_recv_applied().is_some());
        assert!(apply_native_hunting_repair_bundle(&mut world, bundle));
    }
}
