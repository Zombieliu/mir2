use std::collections::BTreeMap;

use super::equipment::{user_item_from_equipment_state, EquipmentState};
use super::resources::{InventoryResource, Stage5SystemsResource};
use crate::config::{
    AccountStore, CharacterSaveRecord, EquipmentSlot, Stage5FriendIdentity,
    Stage5RelationshipState, Stage5SystemsState,
};
use crate::{SimulationConfig, SimulationSession};
use mir2_protocol::ServerPacket;

const MARRIAGE_COOLDOWN_MS: u64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedMarriageProfile {
    pub identity: Stage5FriendIdentity,
    pub name: String,
    pub level: u16,
    pub relationship: Stage5RelationshipState,
}
#[derive(Debug, Clone)]
pub enum SharedMarriageMutation {
    TogglePermission,
    Accept { partner: Stage5FriendIdentity },
    Divorce { partner: Stage5FriendIdentity },
}

fn profile(
    store: &AccountStore,
    id: &Stage5FriendIdentity,
) -> Result<SharedMarriageProfile, String> {
    let account = store
        .accounts
        .get(&id.account_id)
        .ok_or("marriage account missing")?;
    let character = account
        .characters
        .iter()
        .find(|c| c.index == id.character_index)
        .ok_or("marriage character missing")?;
    let save = account
        .saves
        .get(&id.character_index)
        .ok_or("marriage save missing")?;
    let systems: Stage5SystemsState = match save.stage5_systems_json.as_deref() {
        Some(value) => {
            serde_json::from_str(value).map_err(|e| format!("invalid marriage save: {e}"))?
        }
        None => Stage5SystemsState::default(),
    };
    Ok(SharedMarriageProfile {
        identity: id.clone(),
        name: character.name.clone(),
        level: character.level,
        relationship: systems.relationship,
    })
}

pub fn validate_shared_marriage_request(
    proposer: &SharedMarriageProfile,
    recipient: &SharedMarriageProfile,
    now_ms: u64,
) -> Result<(), String> {
    if proposer.identity == recipient.identity {
        return Err("server.CantMarryYourself".into());
    }
    if proposer.relationship.partner_identity.is_some() {
        return Err("server.YouAlreadyMarried".into());
    }
    if recipient.relationship.partner_identity.is_some() {
        return Err("server.PlayerAlreadyMarried".into());
    }
    if proposer.level < 10 || recipient.level < 10 {
        return Err("server.NeedLevelToMarry".into());
    }
    if proposer.relationship.cooldown_until_ms > now_ms
        || recipient.relationship.cooldown_until_ms > now_ms
    {
        return Err("server.MarriageCooldownAfterDivorce".into());
    }
    if !recipient.relationship.allow_marriage {
        return Err("server.ProposalNotAllowed".into());
    }
    Ok(())
}

fn binary_date(now_ms: u64) -> i64 {
    super::crystal_compat::DOTNET_TICKS_AT_UNIX_EPOCH.saturating_add(
        i64::try_from(now_ms)
            .unwrap_or(i64::MAX)
            .saturating_mul(10_000),
    ) | super::crystal_compat::DOTNET_DATETIME_KIND_LOCAL
}

fn advance(state: &mut Stage5RelationshipState) -> Result<(), String> {
    state.authority_revision = state
        .authority_revision
        .checked_add(1)
        .ok_or("marriage revision exhausted")?;
    state.pending_request_from = None;
    state.pending_divorce_from = None;
    Ok(())
}

pub(super) fn clear_divorced_ring_save(save: &mut CharacterSaveRecord) -> Result<(), String> {
    for encoded in &mut save.equipment_items_json {
        let mut item: EquipmentState = serde_json::from_str(encoded)
            .map_err(|e| format!("invalid marriage equipment: {e}"))?;
        if item.slot == EquipmentSlot::RingLeft {
            if let Some(metadata) = item.user_item_metadata.as_mut() {
                if metadata.wedding_ring >= 0 {
                    metadata.wedding_ring = -1;
                    *encoded = serde_json::to_string(&item).map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(())
}

fn write(
    store: &mut AccountStore,
    id: &Stage5FriendIdentity,
    relationship: Stage5RelationshipState,
    divorce: bool,
) -> Result<(), String> {
    let save = store
        .accounts
        .get_mut(&id.account_id)
        .and_then(|a| a.saves.get_mut(&id.character_index))
        .ok_or("marriage save missing")?;
    let mut systems: Stage5SystemsState = match save.stage5_systems_json.as_deref() {
        Some(value) => {
            serde_json::from_str(value).map_err(|e| format!("invalid marriage save: {e}"))?
        }
        None => Stage5SystemsState::default(),
    };
    systems.relationship = relationship;
    if divorce {
        clear_divorced_ring_save(save)?;
    }
    save.stage5_systems_json = Some(serde_json::to_string(&systems).map_err(|e| e.to_string())?);
    Ok(())
}

impl SimulationConfig {
    pub fn shared_marriage_profile_for(
        &self,
        id: &Stage5FriendIdentity,
    ) -> Result<SharedMarriageProfile, String> {
        profile(
            &*self
                .account_store
                .lock()
                .map_err(|_| "marriage account store unavailable")?,
            id,
        )
    }
    pub fn commit_shared_marriage_mutation(
        &self,
        actor: &Stage5FriendIdentity,
        mutation: SharedMarriageMutation,
        now_ms: u64,
    ) -> Result<(), String> {
        self.commit_shared_marriage_mutation_with_live_levels(actor, mutation, now_ms, None)
    }
    pub fn commit_shared_marriage_mutation_with_live_levels(
        &self,
        actor: &Stage5FriendIdentity,
        mutation: SharedMarriageMutation,
        now_ms: u64,
        live_levels: Option<(u16, u16)>,
    ) -> Result<(), String> {
        let mut accounts = vec![actor.account_id.clone()];
        if let SharedMarriageMutation::Accept { partner }
        | SharedMarriageMutation::Divorce { partner } = &mutation
        {
            accounts.push(partner.account_id.clone());
        }
        accounts.sort();
        accounts.dedup();
        self.commit_account_store_transaction(&accounts, |store| {
            let mut owner = profile(store, actor)?;
            match mutation {
                SharedMarriageMutation::TogglePermission => {
                    if owner.relationship.partner_identity.is_some() {
                        owner.relationship.allow_lover_recall =
                            !owner.relationship.allow_lover_recall;
                    } else {
                        owner.relationship.allow_marriage = !owner.relationship.allow_marriage;
                    }
                    advance(&mut owner.relationship)?;
                    write(store, actor, owner.relationship, false)
                }
                SharedMarriageMutation::Accept { partner } => {
                    let mut proposer = profile(store, &partner)?;
                    if let Some((proposer_level, recipient_level)) = live_levels {
                        proposer.level = proposer_level;
                        owner.level = recipient_level;
                    }
                    validate_shared_marriage_request(&proposer, &owner, now_ms)?;
                    owner.relationship.partner_identity = Some(partner.clone());
                    owner.relationship.partner_name = proposer.name;
                    proposer.relationship.partner_identity = Some(actor.clone());
                    proposer.relationship.partner_name = owner.name;
                    for state in [&mut owner.relationship, &mut proposer.relationship] {
                        state.married_at_ms = now_ms;
                        state.married_date_binary_datetime = binary_date(now_ms);
                        state.married_days = 0;
                        state.map_name.clear();
                        advance(state)?;
                    }
                    write(store, actor, owner.relationship, false)?;
                    write(store, &partner, proposer.relationship, false)
                }
                SharedMarriageMutation::Divorce { partner } => {
                    let mut spouse = profile(store, &partner)?;
                    if owner.relationship.partner_identity.as_ref() != Some(&partner)
                        || spouse.relationship.partner_identity.as_ref() != Some(actor)
                    {
                        return Err("server.YouNotMarried".into());
                    }
                    for state in [&mut owner.relationship, &mut spouse.relationship] {
                        state.partner_identity = None;
                        state.partner_name.clear();
                        state.map_name.clear();
                        state.married_days = 0;
                        state.married_at_ms = 0;
                        state.married_date_binary_datetime = binary_date(now_ms);
                        state.cooldown_until_ms = now_ms.saturating_add(MARRIAGE_COOLDOWN_MS);
                        advance(state)?;
                    }
                    write(store, actor, owner.relationship, true)?;
                    write(store, &partner, spouse.relationship, true)
                }
            }
        })
    }
}

impl SimulationSession {
    pub fn refresh_shared_marriage(
        &mut self,
        online: &BTreeMap<(String, i32), String>,
        now_ms: u64,
        force: bool,
    ) -> Vec<ServerPacket> {
        let Some(active) = self.active_identity() else {
            return Vec::new();
        };
        let Some(config) = self.shared_mentor_config() else {
            return Vec::new();
        };
        let id = Stage5FriendIdentity {
            account_id: active.account_id,
            character_index: active.character_index,
        };
        let Ok(durable) = config.shared_marriage_profile_for(&id) else {
            return Vec::new();
        };
        let mut relationship = durable.relationship;
        if let Some(peer) = relationship
            .partner_identity
            .as_ref()
            .and_then(|id| config.shared_marriage_profile_for(id).ok())
        {
            relationship.partner_name = peer.name;
            relationship.map_name = online
                .get(&(peer.identity.account_id, peer.identity.character_index))
                .cloned()
                .unwrap_or_default();
            relationship.married_days = i16::try_from(
                now_ms.saturating_sub(relationship.married_at_ms) / (24 * 60 * 60 * 1000),
            )
            .unwrap_or(i16::MAX);
        } else {
            relationship.partner_name.clear();
            relationship.map_name.clear();
            relationship.married_days = 0;
        }
        relationship.pending_request_from = None;
        relationship.pending_divorce_from = None;
        let previous = self
            .app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .relationship
            .clone();
        let mut packets = Vec::new();
        if relationship.authority_revision > previous.authority_revision
            && relationship.partner_identity.is_none()
            && relationship.cooldown_until_ms > 0
        {
            let mut inventory = self.app.world_mut().resource_mut::<InventoryResource>();
            if let Some(ring) = inventory
                .equipment_items
                .iter_mut()
                .find(|i| i.slot == EquipmentSlot::RingLeft)
            {
                if let Some(metadata) = ring.user_item_metadata.as_mut() {
                    if metadata.wedding_ring >= 0 {
                        metadata.wedding_ring = -1;
                        if let Some(item) = user_item_from_equipment_state(ring) {
                            packets.push(ServerPacket::RefreshItem { item });
                        }
                    }
                }
            }
        }
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .relationship = relationship.clone();
        if force || previous != relationship {
            packets.push(ServerPacket::LoverUpdate {
                name: relationship.partner_name,
                date_binary_datetime: relationship.married_date_binary_datetime,
                map_name: relationship.map_name,
                married_days: relationship.married_days,
            });
        }
        packets
    }
}
