//! Durable parent permissions for one finite Bichon life session.
//!
//! These records share the supply journal's authenticated publication and its
//! single writer. The parent has one permanent child slot, not a replenishing
//! allowance. A child created before a runtime restart is recovery-only.
use super::*;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeGrant {
    pub life_permit_id: String,
    pub world_epoch: String,
    pub actor_character_index: i32,
    pub map_file_name: String,
    pub started_at_ms: u64,
    pub expires_at_ms: u64,
    pub duration_seconds: u32,
    pub budget_gold: u32,
    pub max_purchased_quantity: u16,
    pub bounds: SupplyBounds,
    pub home: SupplyPoint,
    pub route_id: String,
    pub waypoints: Vec<SupplyPoint>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifeScopeStatus {
    Active,
    Paused,
    Revoked,
}

/// Non-secret, in-memory authority projection. The consumer also checks the
/// trusted current clock against expires_at_ms; it must never call the ledger
/// while holding this read guard, or mutate this projection itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LifeScope {
    pub life_permit_id: String,
    pub map_file_name: String,
    pub bounds: SupplyBounds,
    pub home: SupplyPoint,
    pub expires_at_ms: u64,
    pub status: LifeScopeStatus,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LifeScopeProjection {
    pub poisoned: bool,
    pub actors: BTreeMap<CharacterBinding, LifeScope>,
}
pub type LifeProjectionHandle = Arc<RwLock<LifeScopeProjection>>;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LifeRecord {
    owner_request_key: String,
    owner: CharacterBinding,
    actor: CharacterBinding,
    actor_admission_id: String,
    resident_id: String,
    archive_id: String,
    grant: LifeGrant,
    activated: bool,
    prepared_boot_id: String,
    active_boot_id: Option<String>,
    paused_at_ms: Option<u64>,
    revoked_at_ms: Option<u64>,
    child_permit_id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LifeResumeRecord {
    owner_request_key: String,
    owner: CharacterBinding,
    actor: CharacterBinding,
    actor_admission_id: String,
    resident_id: String,
    archive_id: String,
    life_permit_id: String,
    requested_at_ms: u64,
    prepared_boot_id: String,
    activated: bool,
    abandoned_at_ms: Option<u64>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LifeRevocationRecord {
    owner_request_key: String,
    owner: CharacterBinding,
    resident_id: String,
    recorded_at_ms: u64,
    revoked_life_permit_ids: Vec<String>,
    revoked_child_permit_ids: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestDomain {
    Supply,
    SupplyCancel,
    Life,
    LifeResume,
    LifeCancel,
}

fn life_bounds() -> SupplyBounds {
    SupplyBounds {
        min_x: 270,
        min_y: 595,
        max_x: 310,
        max_y: 645,
    }
}
fn home() -> SupplyPoint {
    SupplyPoint { x: 288, y: 616 }
}
fn waypoints() -> Vec<SupplyPoint> {
    vec![
        SupplyPoint { x: 286, y: 628 },
        SupplyPoint { x: 294, y: 633 },
        SupplyPoint { x: 301, y: 626 },
    ]
}
pub(super) fn record_count(document: &Document) -> usize {
    document
        .records
        .len()
        .saturating_add(document.revocations.len())
        .saturating_add(document.lives.len())
        .saturating_add(document.life_resumptions.len())
        .saturating_add(document.life_revocations.len())
}
fn request_domain(
    document: &Document,
    owner: &CharacterBinding,
    key: &str,
) -> Option<RequestDomain> {
    if document
        .records
        .iter()
        .any(|r| &r.owner == owner && r.owner_request_key == key)
    {
        Some(RequestDomain::Supply)
    } else if document
        .revocations
        .iter()
        .any(|r| &r.owner == owner && r.owner_request_key == key)
    {
        Some(RequestDomain::SupplyCancel)
    } else if document
        .lives
        .iter()
        .any(|r| &r.owner == owner && r.owner_request_key == key)
    {
        Some(RequestDomain::Life)
    } else if document
        .life_resumptions
        .iter()
        .any(|r| &r.owner == owner && r.owner_request_key == key)
    {
        Some(RequestDomain::LifeResume)
    } else if document
        .life_revocations
        .iter()
        .any(|r| &r.owner == owner && r.owner_request_key == key)
    {
        Some(RequestDomain::LifeCancel)
    } else {
        None
    }
}
fn valid_grant(grant: &LifeGrant, epoch: &str, actor: &CharacterBinding) -> bool {
    reference(&grant.life_permit_id)
        && grant.world_epoch == epoch
        && grant.actor_character_index == actor.character_index
        && grant.map_file_name == "0"
        && grant.started_at_ms <= MAX_SAFE_MS
        && matches!(grant.duration_seconds, 1200 | 1800)
        && grant
            .started_at_ms
            .checked_add(u64::from(grant.duration_seconds) * 1000)
            == Some(grant.expires_at_ms)
        && grant.expires_at_ms <= MAX_SAFE_MS
        && (1..=50).contains(&grant.budget_gold)
        && grant.max_purchased_quantity == 1
        && grant.bounds == life_bounds()
        && grant.home == home()
        && grant.route_id == "bichon-south-v1"
        && grant.waypoints == waypoints()
}
fn same_enrollment(record: &LifeRecord, enrollment: &Enrollment) -> bool {
    record.owner == enrollment.owner
        && record.actor == enrollment.actor
        && record.resident_id == enrollment.resident_id
        && record.archive_id == enrollment.archive_id
}
fn valid_enrollment(enrollment: &Enrollment) -> bool {
    character(&enrollment.owner)
        && character(&enrollment.actor)
        && enrollment.owner != enrollment.actor
        && reference(&enrollment.resident_id)
        && reference(&enrollment.archive_id)
}
fn timed(grant: &LifeGrant, now: u64) -> Result<()> {
    if now > MAX_SAFE_MS || now < grant.started_at_ms {
        Err(SupplyError::Invalid)
    } else if now >= grant.expires_at_ms {
        Err(SupplyError::Expired)
    } else {
        Ok(())
    }
}
fn current_life(record: &LifeRecord, admission: &str, boot: &str, now: u64) -> Result<()> {
    timed(&record.grant, now)?;
    if !record.activated {
        return Err(SupplyError::Missing);
    }
    if record.revoked_at_ms.is_some() {
        return Err(SupplyError::Revoked);
    }
    if record.actor_admission_id != admission
        || record.active_boot_id.as_deref() != Some(boot)
        || record.paused_at_ms.is_some()
    {
        return Err(SupplyError::AdmissionChanged);
    }
    Ok(())
}

pub(super) fn validate_life_document(document: &Document) -> Result<()> {
    let mut keys = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for record in &document.records {
        if !keys.insert((record.owner.clone(), record.owner_request_key.clone()))
            || !ids.insert(record.grant.permit_id.clone())
        {
            return Err(SupplyError::Integrity);
        }
        if record.parent_life_permit_id.is_none()
            && (record.life_execution_boot_id.is_some() || record.life_execution_retired)
        {
            return Err(SupplyError::Integrity);
        }
    }
    for record in &document.revocations {
        if !keys.insert((record.owner.clone(), record.owner_request_key.clone())) {
            return Err(SupplyError::Integrity);
        }
    }
    for record in &document.lives {
        if !valid_enrollment(&Enrollment {
            owner: record.owner.clone(),
            actor: record.actor.clone(),
            resident_id: record.resident_id.clone(),
            archive_id: record.archive_id.clone(),
            display_name: String::new(),
        }) || !private_reference(&record.owner_request_key)
            || !keys.insert((record.owner.clone(), record.owner_request_key.clone()))
            || !ids.insert(record.grant.life_permit_id.clone())
            || !private_reference(&record.actor_admission_id)
            || !reference(&record.prepared_boot_id)
            || !valid_grant(&record.grant, &document.world_epoch, &record.actor)
            || record.activated != record.active_boot_id.is_some()
            || record
                .active_boot_id
                .as_ref()
                .is_some_and(|id| !reference(id))
            || !record.activated
                && (record.paused_at_ms.is_some() || record.child_permit_id.is_some())
            || record
                .paused_at_ms
                .into_iter()
                .chain(record.revoked_at_ms)
                .any(|time| time < record.grant.started_at_ms || time > MAX_SAFE_MS)
        {
            return Err(SupplyError::Integrity);
        }
        let children = document
            .records
            .iter()
            .filter(|child| {
                child.parent_life_permit_id.as_deref() == Some(record.grant.life_permit_id.as_str())
            })
            .collect::<Vec<_>>();
        if children.len() > 1 || children.len() != usize::from(record.child_permit_id.is_some()) {
            return Err(SupplyError::Integrity);
        }
        if let Some(child) = children.first() {
            if record.child_permit_id.as_ref() != Some(&child.grant.permit_id)
                || child.owner != record.owner
                || child.actor != record.actor
                || child.resident_id != record.resident_id
                || child.archive_id != record.archive_id
                || !child.activated
                || child.grant.max_gold != record.grant.budget_gold
                || child.grant.quantity != record.grant.max_purchased_quantity
                || child.authorized_at_ms < record.grant.started_at_ms
                || child.grant.expires_at_ms > record.grant.expires_at_ms
                || child.grant.return_anchor != record.grant.home
                || child.owner_request_key != format!("life-child:{}", record.grant.life_permit_id)
                || child
                    .life_execution_boot_id
                    .as_ref()
                    .is_none_or(|id| !reference(id))
                || record.revoked_at_ms.is_some() && child.revoked_at_ms.is_none()
                || record.paused_at_ms.is_some() && !child.life_execution_retired
            {
                return Err(SupplyError::Integrity);
            }
        }
    }
    for child in document
        .records
        .iter()
        .filter(|record| record.parent_life_permit_id.is_some())
    {
        if !document.lives.iter().any(|record| {
            Some(record.grant.life_permit_id.as_str()) == child.parent_life_permit_id.as_deref()
        }) {
            return Err(SupplyError::Integrity);
        }
    }
    for record in &document.life_resumptions {
        let Some(parent) = document
            .lives
            .iter()
            .find(|life| life.grant.life_permit_id == record.life_permit_id)
        else {
            return Err(SupplyError::Integrity);
        };
        if !private_reference(&record.owner_request_key)
            || !keys.insert((record.owner.clone(), record.owner_request_key.clone()))
            || record.owner != parent.owner
            || record.actor != parent.actor
            || record.resident_id != parent.resident_id
            || record.archive_id != parent.archive_id
            || !private_reference(&record.actor_admission_id)
            || !reference(&record.prepared_boot_id)
            || record.requested_at_ms < parent.grant.started_at_ms
            || record.requested_at_ms >= parent.grant.expires_at_ms
            || record.activated && record.abandoned_at_ms.is_some()
            || record
                .abandoned_at_ms
                .is_some_and(|time| time < record.requested_at_ms || time > MAX_SAFE_MS)
        {
            return Err(SupplyError::Integrity);
        }
    }
    let mut revoked_lives = BTreeSet::new();
    let mut revoked_children = BTreeSet::new();
    for record in &document.life_revocations {
        if !private_reference(&record.owner_request_key)
            || !character(&record.owner)
            || !reference(&record.resident_id)
            || record.recorded_at_ms > MAX_SAFE_MS
            || !keys.insert((record.owner.clone(), record.owner_request_key.clone()))
        {
            return Err(SupplyError::Integrity);
        }
        for id in &record.revoked_life_permit_ids {
            let Some(parent) = document
                .lives
                .iter()
                .find(|life| &life.grant.life_permit_id == id)
            else {
                return Err(SupplyError::Integrity);
            };
            if !revoked_lives.insert(id)
                || parent.owner != record.owner
                || parent.resident_id != record.resident_id
                || parent.revoked_at_ms != Some(record.recorded_at_ms)
            {
                return Err(SupplyError::Integrity);
            }
        }
        for id in &record.revoked_child_permit_ids {
            let Some(child) = document
                .records
                .iter()
                .find(|child| &child.grant.permit_id == id)
            else {
                return Err(SupplyError::Integrity);
            };
            if !revoked_children.insert(id)
                || child.owner != record.owner
                || child.resident_id != record.resident_id
                || child.parent_life_permit_id.is_none()
                || child.revoked_at_ms != Some(record.recorded_at_ms)
                || !record
                    .revoked_life_permit_ids
                    .iter()
                    .any(|life_id| Some(life_id) == child.parent_life_permit_id.as_ref())
            {
                return Err(SupplyError::Integrity);
            }
        }
    }
    Ok(())
}

fn projection(document: &Document, boot: &str) -> LifeScopeProjection {
    let mut actors = BTreeMap::new();
    // A draft has never granted authority and must not impose a new boundary
    // either. Only previously activated lives become paused on a cold open.
    for record in &document.lives {
        if !record.activated {
            continue;
        }
        let status = if record.revoked_at_ms.is_some() {
            LifeScopeStatus::Revoked
        } else if record.activated
            && record.active_boot_id.as_deref() == Some(boot)
            && record.paused_at_ms.is_none()
        {
            LifeScopeStatus::Active
        } else {
            LifeScopeStatus::Paused
        };
        actors.insert(
            record.actor.clone(),
            LifeScope {
                life_permit_id: record.grant.life_permit_id.clone(),
                map_file_name: record.grant.map_file_name.clone(),
                bounds: record.grant.bounds,
                home: record.grant.home,
                expires_at_ms: record.grant.expires_at_ms,
                status,
            },
        );
    }
    LifeScopeProjection {
        poisoned: false,
        actors,
    }
}
pub(super) fn new_projection(document: &Document, boot: &str) -> LifeProjectionHandle {
    Arc::new(RwLock::new(projection(document, boot)))
}

impl SupplyLedger {
    pub fn projection_handle(&self) -> LifeProjectionHandle {
        self.projection.clone()
    }
    pub(super) fn poison(&self) {
        self.poisoned.set(true);
        self.projection
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .poisoned = true;
    }
    pub(super) fn refresh_projection(&self) -> Result<()> {
        match self.projection.write() {
            Ok(mut state) => {
                *state = projection(&self.document, &self.boot_id);
                Ok(())
            }
            Err(poisoned) => {
                poisoned.into_inner().poisoned = true;
                self.poisoned.set(true);
                Err(SupplyError::Unavailable)
            }
        }
    }
    pub(super) fn reject_life_request_key(
        &self,
        owner: &CharacterBinding,
        key: &str,
    ) -> Result<()> {
        if matches!(
            request_domain(&self.document, owner, key),
            Some(RequestDomain::Life | RequestDomain::LifeResume | RequestDomain::LifeCancel)
        ) {
            Err(SupplyError::Conflict)
        } else {
            Ok(())
        }
    }
    pub(super) fn ensure_standalone_allowed(
        &self,
        actor: &CharacterBinding,
        now: u64,
    ) -> Result<()> {
        if self.document.lives.iter().any(|record| {
            &record.actor == actor
                && record.revoked_at_ms.is_none()
                && now < record.grant.expires_at_ms
        }) {
            return Err(SupplyError::Conflict);
        }
        if self.document.records.iter().any(|record| {
            &record.actor == actor
                && record.parent_life_permit_id.is_some()
                && record.operation.is_some()
                && record.outcome.is_none()
        }) {
            return Err(SupplyError::Unresolved);
        }
        Ok(())
    }
    pub(super) fn current_life_child(
        &self,
        child: &PermitRecord,
        admission: &str,
        now: u64,
    ) -> Result<()> {
        let Some(parent_id) = &child.parent_life_permit_id else {
            return Ok(());
        };
        Self::current(child, admission, now)?;
        if child.life_execution_retired
            || child.life_execution_boot_id.as_deref() != Some(self.boot_id.as_str())
        {
            return Err(SupplyError::AdmissionChanged);
        }
        let parent = self
            .document
            .lives
            .iter()
            .find(|record| &record.grant.life_permit_id == parent_id)
            .ok_or(SupplyError::Integrity)?;
        current_life(parent, admission, &self.boot_id, now)
    }
    fn unresolved_for(&self, actor: &CharacterBinding) -> bool {
        self.document.records.iter().any(|record| {
            &record.actor == actor && record.operation.is_some() && record.outcome.is_none()
        })
    }
    fn life_index(&self, actor: &CharacterBinding, id: &str) -> Result<usize> {
        let index = self
            .document
            .lives
            .iter()
            .position(|record| record.grant.life_permit_id == id)
            .ok_or(SupplyError::Missing)?;
        if &self.document.lives[index].actor != actor {
            return Err(SupplyError::Conflict);
        }
        Ok(index)
    }

    pub fn prepare_life(
        &mut self,
        key: &str,
        enrollment: &Enrollment,
        admission: &str,
        now: u64,
        budget: u32,
        duration: u32,
    ) -> Result<LifeGrant> {
        self.ready()?;
        if !private_reference(key)
            || !valid_enrollment(enrollment)
            || !private_reference(admission)
            || !(1..=50).contains(&budget)
            || !matches!(duration, 1200 | 1800)
            || now
                .checked_add(u64::from(duration) * 1000)
                .is_none_or(|end| end > MAX_SAFE_MS)
        {
            return Err(SupplyError::Invalid);
        }
        match request_domain(&self.document, &enrollment.owner, key) {
            Some(RequestDomain::Life) => {
                let prior = self
                    .document
                    .lives
                    .iter()
                    .find(|record| {
                        record.owner == enrollment.owner && record.owner_request_key == key
                    })
                    .ok_or(SupplyError::Integrity)?;
                if !same_enrollment(prior, enrollment)
                    || prior.grant.budget_gold != budget
                    || prior.grant.duration_seconds != duration
                {
                    return Err(SupplyError::Conflict);
                }
                return Ok(prior.grant.clone());
            }
            Some(_) => return Err(SupplyError::Conflict),
            None => {}
        }
        if record_count(&self.document) >= RECORD_CAP {
            return Err(SupplyError::Capacity);
        }
        if self.unresolved_for(&enrollment.actor) {
            return Err(SupplyError::Unresolved);
        }
        let grant = LifeGrant {
            life_permit_id: random_reference("life:")?,
            world_epoch: self.document.world_epoch.clone(),
            actor_character_index: enrollment.actor.character_index,
            map_file_name: "0".into(),
            started_at_ms: now,
            expires_at_ms: now + u64::from(duration) * 1000,
            duration_seconds: duration,
            budget_gold: budget,
            max_purchased_quantity: 1,
            bounds: life_bounds(),
            home: home(),
            route_id: "bichon-south-v1".into(),
            waypoints: waypoints(),
        };
        let mut next = self.document.clone();
        next.lives.push(LifeRecord {
            owner_request_key: key.into(),
            owner: enrollment.owner.clone(),
            actor: enrollment.actor.clone(),
            actor_admission_id: admission.into(),
            resident_id: enrollment.resident_id.clone(),
            archive_id: enrollment.archive_id.clone(),
            grant: grant.clone(),
            activated: false,
            prepared_boot_id: self.boot_id.clone(),
            active_boot_id: None,
            paused_at_ms: None,
            revoked_at_ms: None,
            child_permit_id: None,
        });
        self.commit(next)?;
        Ok(grant)
    }

    pub fn activate_life(
        &mut self,
        key: &str,
        actor: &CharacterBinding,
        admission: &str,
        id: &str,
        now: u64,
    ) -> Result<LifeGrant> {
        self.ready()?;
        if !private_reference(key)
            || !character(actor)
            || !private_reference(admission)
            || !reference(id)
        {
            return Err(SupplyError::Invalid);
        }
        let index = self.life_index(actor, id)?;
        let record = &self.document.lives[index];
        if record.owner_request_key != key {
            return Err(SupplyError::Conflict);
        }
        if record.actor_admission_id != admission || record.prepared_boot_id != self.boot_id {
            return Err(SupplyError::AdmissionChanged);
        }
        timed(&record.grant, now)?;
        if record.revoked_at_ms.is_some() {
            return Err(SupplyError::Revoked);
        }
        if record.activated {
            current_life(record, admission, &self.boot_id, now)?;
            return Ok(record.grant.clone());
        }
        if self.unresolved_for(actor) {
            return Err(SupplyError::Unresolved);
        }
        if self.document.lives[index + 1..]
            .iter()
            .any(|later| &later.actor == actor && later.activated)
        {
            return Err(SupplyError::Conflict);
        }
        let grant = record.grant.clone();
        let mut next = self.document.clone();
        for (prior_index, prior) in next.lives.iter_mut().enumerate() {
            if prior_index != index && &prior.actor == actor && prior.revoked_at_ms.is_none() {
                prior.revoked_at_ms = Some(now.max(prior.grant.started_at_ms));
            }
        }
        for prior in &mut next.records {
            if &prior.actor == actor && prior.revoked_at_ms.is_none() {
                prior.revoked_at_ms = Some(now.max(prior.authorized_at_ms));
                if prior.parent_life_permit_id.is_some() {
                    prior.life_execution_retired = true;
                }
            }
        }
        next.lives[index].activated = true;
        next.lives[index].active_boot_id = Some(self.boot_id.clone());
        self.commit(next)?;
        Ok(grant)
    }

    /// Abandon either the initial life draft or a matching resume draft. This
    /// cannot revoke the earlier active life when a policy check fails.
    pub fn abandon_prepared_life(
        &mut self,
        key: &str,
        actor: &CharacterBinding,
        id: &str,
        now: u64,
    ) -> Result<bool> {
        self.ready()?;
        if !private_reference(key) || !character(actor) || !reference(id) || now > MAX_SAFE_MS {
            return Err(SupplyError::Invalid);
        }
        if let Some(index) = self
            .document
            .life_resumptions
            .iter()
            .position(|record| record.owner_request_key == key && &record.actor == actor)
        {
            let record = &self.document.life_resumptions[index];
            if record.life_permit_id != id {
                return Err(SupplyError::Conflict);
            }
            if record.activated || record.abandoned_at_ms.is_some() {
                return Ok(false);
            }
            if now < record.requested_at_ms {
                return Err(SupplyError::Invalid);
            }
            let mut next = self.document.clone();
            next.life_resumptions[index].abandoned_at_ms = Some(now);
            self.commit(next)?;
            return Ok(true);
        }
        let index = self.life_index(actor, id)?;
        let record = &self.document.lives[index];
        if record.owner_request_key != key {
            return Err(SupplyError::Conflict);
        }
        if record.activated || record.revoked_at_ms.is_some() {
            return Ok(false);
        }
        if now < record.grant.started_at_ms {
            return Err(SupplyError::Invalid);
        }
        let mut next = self.document.clone();
        next.lives[index].revoked_at_ms = Some(now);
        self.commit(next)?;
        Ok(true)
    }

    pub fn active_life(
        &self,
        actor: &CharacterBinding,
        admission: &str,
        now: u64,
    ) -> Result<Option<LifeGrant>> {
        self.ready()?;
        if !character(actor) || !private_reference(admission) {
            return Err(SupplyError::Invalid);
        }
        let Some(record) = self
            .document
            .lives
            .iter()
            .rev()
            .find(|record| &record.actor == actor && record.activated)
        else {
            return Ok(None);
        };
        current_life(record, admission, &self.boot_id, now)?;
        Ok(Some(record.grant.clone()))
    }

    /// Called on the enrolled actor admission's detach, never the owner's
    /// detach. Pausing also permanently retires an unattempted old child.
    pub fn pause_life(
        &mut self,
        actor: &CharacterBinding,
        admission: &str,
        now: u64,
    ) -> Result<bool> {
        self.ready()?;
        if !character(actor) || !private_reference(admission) || now > MAX_SAFE_MS {
            return Err(SupplyError::Invalid);
        }
        let Some(index) = self
            .document
            .lives
            .iter()
            .rposition(|record| &record.actor == actor && record.activated)
        else {
            return Ok(false);
        };
        let record = &self.document.lives[index];
        if record.actor_admission_id != admission
            || record.revoked_at_ms.is_some()
            || record.paused_at_ms.is_some()
        {
            return Ok(false);
        }
        if now < record.grant.started_at_ms {
            return Err(SupplyError::Invalid);
        }
        let child = record.child_permit_id.clone();
        let mut next = self.document.clone();
        next.lives[index].paused_at_ms = Some(now);
        if let Some(child) = child {
            let record = next
                .records
                .iter_mut()
                .find(|record| record.grant.permit_id == child)
                .ok_or(SupplyError::Integrity)?;
            record.life_execution_retired = true;
        }
        self.commit(next)?;
        Ok(true)
    }

    pub fn resume_life_prepare(
        &mut self,
        key: &str,
        enrollment: &Enrollment,
        admission: &str,
        id: &str,
        now: u64,
    ) -> Result<LifeGrant> {
        self.ready()?;
        if !private_reference(key)
            || !valid_enrollment(enrollment)
            || !private_reference(admission)
            || !reference(id)
        {
            return Err(SupplyError::Invalid);
        }
        let index = self.life_index(&enrollment.actor, id)?;
        let parent = &self.document.lives[index];
        if !same_enrollment(parent, enrollment) {
            return Err(SupplyError::Conflict);
        }
        match request_domain(&self.document, &enrollment.owner, key) {
            Some(RequestDomain::LifeResume) => {
                let prior = self
                    .document
                    .life_resumptions
                    .iter()
                    .find(|record| {
                        record.owner == enrollment.owner && record.owner_request_key == key
                    })
                    .ok_or(SupplyError::Integrity)?;
                if prior.life_permit_id != id
                    || prior.actor != enrollment.actor
                    || prior.resident_id != enrollment.resident_id
                    || prior.archive_id != enrollment.archive_id
                {
                    return Err(SupplyError::Conflict);
                }
                return Ok(parent.grant.clone());
            }
            Some(_) => return Err(SupplyError::Conflict),
            None => {}
        }
        timed(&parent.grant, now)?;
        if !parent.activated {
            return Err(SupplyError::Missing);
        }
        if parent.revoked_at_ms.is_some() {
            return Err(SupplyError::Revoked);
        }
        if record_count(&self.document) >= RECORD_CAP {
            return Err(SupplyError::Capacity);
        }
        let grant = parent.grant.clone();
        let mut next = self.document.clone();
        next.life_resumptions.push(LifeResumeRecord {
            owner_request_key: key.into(),
            owner: enrollment.owner.clone(),
            actor: enrollment.actor.clone(),
            actor_admission_id: admission.into(),
            resident_id: enrollment.resident_id.clone(),
            archive_id: enrollment.archive_id.clone(),
            life_permit_id: id.into(),
            requested_at_ms: now,
            prepared_boot_id: self.boot_id.clone(),
            activated: false,
            abandoned_at_ms: None,
        });
        self.commit(next)?;
        Ok(grant)
    }

    pub fn resume_life_activate(
        &mut self,
        key: &str,
        actor: &CharacterBinding,
        admission: &str,
        id: &str,
        now: u64,
    ) -> Result<LifeGrant> {
        self.ready()?;
        if !private_reference(key)
            || !character(actor)
            || !private_reference(admission)
            || !reference(id)
        {
            return Err(SupplyError::Invalid);
        }
        let life_index = self.life_index(actor, id)?;
        let parent = &self.document.lives[life_index];
        let index = self
            .document
            .life_resumptions
            .iter()
            .position(|record| record.owner == parent.owner && record.owner_request_key == key)
            .ok_or(SupplyError::Missing)?;
        let resume = &self.document.life_resumptions[index];
        if &resume.actor != actor || resume.life_permit_id != id {
            return Err(SupplyError::Conflict);
        }
        if resume.actor_admission_id != admission || resume.prepared_boot_id != self.boot_id {
            return Err(SupplyError::AdmissionChanged);
        }
        if now < resume.requested_at_ms {
            return Err(SupplyError::Invalid);
        }
        timed(&parent.grant, now)?;
        if resume.abandoned_at_ms.is_some() || parent.revoked_at_ms.is_some() {
            return Err(SupplyError::Revoked);
        }
        if !parent.activated {
            return Err(SupplyError::Missing);
        }
        if resume.activated {
            current_life(parent, admission, &self.boot_id, now)?;
            return Ok(parent.grant.clone());
        }
        if self.document.life_resumptions[index + 1..]
            .iter()
            .any(|later| later.life_permit_id == id && later.activated)
        {
            return Err(SupplyError::Conflict);
        }
        let grant = parent.grant.clone();
        let child = parent.child_permit_id.clone();
        let mut next = self.document.clone();
        next.lives[life_index].actor_admission_id = admission.into();
        next.lives[life_index].active_boot_id = Some(self.boot_id.clone());
        next.lives[life_index].paused_at_ms = None;
        next.life_resumptions[index].activated = true;
        if let Some(child) = child {
            let record = next
                .records
                .iter_mut()
                .find(|record| record.grant.permit_id == child)
                .ok_or(SupplyError::Integrity)?;
            record.life_execution_retired = true;
        }
        self.commit(next)?;
        Ok(grant)
    }

    /// There is exactly one child key and slot per parent. A duplicate returns
    /// the original live child; expiry, cancellation or restart never renews it.
    pub fn claim_life_supply(
        &mut self,
        actor: &CharacterBinding,
        admission: &str,
        id: &str,
        now: u64,
    ) -> Result<SupplyGrant> {
        self.ready()?;
        if !character(actor) || !private_reference(admission) || !reference(id) {
            return Err(SupplyError::Invalid);
        }
        let index = self.life_index(actor, id)?;
        let parent = &self.document.lives[index];
        current_life(parent, admission, &self.boot_id, now)?;
        if let Some(child_id) = &parent.child_permit_id {
            let child = self
                .document
                .records
                .iter()
                .find(|record| &record.grant.permit_id == child_id)
                .ok_or(SupplyError::Integrity)?;
            self.current_life_child(child, admission, now)?;
            return Ok(child.grant.clone());
        }
        if parent.grant.expires_at_ms - now < 300_000 {
            return Err(SupplyError::Expired);
        }
        if record_count(&self.document) >= RECORD_CAP {
            return Err(SupplyError::Capacity);
        }
        if self.unresolved_for(actor) {
            return Err(SupplyError::Unresolved);
        }
        let key = format!("life-child:{id}");
        if request_domain(&self.document, &parent.owner, &key).is_some() {
            return Err(SupplyError::Conflict);
        }
        let grant = SupplyGrant {
            permit_id: random_reference("supply:")?,
            world_epoch: self.document.world_epoch.clone(),
            actor_character_index: actor.character_index,
            map_file_name: "0".into(),
            npc_index: 9,
            npc_name: "Merchant_Ruben".into(),
            npc_position: SupplyPoint { x: 288, y: 608 },
            item_index: 658,
            item_name: "(HP)DrugSmall".into(),
            quantity: 1,
            max_gold: parent.grant.budget_gold,
            expires_at_ms: now + 300_000,
            return_anchor: parent.grant.home,
            bounds: SupplyBounds::village(),
        };
        let mut next = self.document.clone();
        next.records.push(PermitRecord {
            owner_request_key: key,
            owner: parent.owner.clone(),
            actor: parent.actor.clone(),
            actor_admission_id: admission.into(),
            resident_id: parent.resident_id.clone(),
            archive_id: parent.archive_id.clone(),
            authorized_at_ms: now,
            grant: grant.clone(),
            revoked_at_ms: None,
            reserved_gold: 0,
            operation: None,
            outcome: None,
            activated: true,
            prepared_boot_id: None,
            parent_life_permit_id: Some(id.into()),
            life_execution_boot_id: Some(self.boot_id.clone()),
            life_execution_retired: false,
        });
        next.lives[index].child_permit_id = Some(grant.permit_id.clone());
        self.commit(next)?;
        Ok(grant)
    }

    pub fn life_revocation_seen(
        &self,
        key: &str,
        owner: &CharacterBinding,
        resident: &str,
    ) -> Result<bool> {
        self.ready()?;
        if !private_reference(key) || !character(owner) || !reference(resident) {
            return Err(SupplyError::Invalid);
        }
        match request_domain(&self.document, owner, key) {
            Some(RequestDomain::LifeCancel) => {
                let record = self
                    .document
                    .life_revocations
                    .iter()
                    .find(|record| &record.owner == owner && record.owner_request_key == key)
                    .ok_or(SupplyError::Integrity)?;
                if record.resident_id != resident {
                    Err(SupplyError::Conflict)
                } else {
                    Ok(true)
                }
            }
            Some(_) => Err(SupplyError::Conflict),
            None => Ok(false),
        }
    }

    /// A durable cancellation records only its original targets. Retrying it
    /// after a newer owner grant must not cancel that grant or its runner.
    pub fn revoke_life_request(
        &mut self,
        key: &str,
        owner: &CharacterBinding,
        resident: &str,
        now: u64,
    ) -> Result<usize> {
        self.ready()?;
        if now > MAX_SAFE_MS {
            return Err(SupplyError::Invalid);
        }
        if self.life_revocation_seen(key, owner, resident)? {
            return Ok(self
                .document
                .life_revocations
                .iter()
                .find(|record| &record.owner == owner && record.owner_request_key == key)
                .ok_or(SupplyError::Integrity)?
                .revoked_life_permit_ids
                .len());
        }
        if record_count(&self.document) >= RECORD_CAP {
            // A cancellation that cannot be durably remembered must not leave
            // the live projection permitting more work. Reopen is paused too.
            self.poison();
            return Err(SupplyError::Capacity);
        }
        let mut next = self.document.clone();
        let mut revoked_life_permit_ids = Vec::new();
        let mut revoked_child_permit_ids = Vec::new();
        for record in &mut next.lives {
            if &record.owner == owner
                && record.resident_id == resident
                && record.revoked_at_ms.is_none()
            {
                if now < record.grant.started_at_ms {
                    return Err(SupplyError::Invalid);
                }
                record.revoked_at_ms = Some(now);
                revoked_life_permit_ids.push(record.grant.life_permit_id.clone());
            }
        }
        for child in &mut next.records {
            if child
                .parent_life_permit_id
                .as_ref()
                .is_some_and(|id| revoked_life_permit_ids.contains(id))
            {
                child.life_execution_retired = true;
                if child.revoked_at_ms.is_none() {
                    if now < child.authorized_at_ms {
                        return Err(SupplyError::Invalid);
                    }
                    child.revoked_at_ms = Some(now);
                    revoked_child_permit_ids.push(child.grant.permit_id.clone());
                }
            }
        }
        for resume in &mut next.life_resumptions {
            if revoked_life_permit_ids.contains(&resume.life_permit_id)
                && !resume.activated
                && resume.abandoned_at_ms.is_none()
            {
                if now < resume.requested_at_ms {
                    return Err(SupplyError::Invalid);
                }
                resume.abandoned_at_ms = Some(now);
            }
        }
        let count = revoked_life_permit_ids.len();
        next.life_revocations.push(LifeRevocationRecord {
            owner_request_key: key.into(),
            owner: owner.clone(),
            resident_id: resident.into(),
            recorded_at_ms: now,
            revoked_life_permit_ids,
            revoked_child_permit_ids,
        });
        self.commit(next)?;
        Ok(count)
    }
}

#[cfg(test)]
#[path = "companion_life_tests.rs"]
mod tests;
