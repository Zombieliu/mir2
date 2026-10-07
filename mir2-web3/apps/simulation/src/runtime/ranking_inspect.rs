//! Pure projection for ranking Inspect, whose ID is CharacterInfo.Index.
//!
//! Authentication, requester authorization, ordinary ObjectID/Hero Inspect, online
//! roster freshness, and authoritative supplemental facts belong to the caller.
//! Resolve and project from one consistent snapshot; this module never loads a
//! save, creates starter equipment, or reads the requester's personal runtime.

use std::collections::BTreeSet;

use mir2_protocol::{PlayerInspectInfo, UserItem};

use crate::config::{
    AccountStore, CharacterRecord, CharacterSaveRecord, EquipmentSlot, ItemContainer,
    Stage5FriendIdentity,
};

use super::equipment::{equipment_slot_index, item_state_from_equipment_state, EquipmentState};
use super::items::{
    crystal_template_can_equip_to_slot, item_state_can_equip_to_slot,
    try_user_item_from_item_state, validate_committed_item_state_carrier,
    validate_committed_user_item_carrier,
};

const INSPECT_EQUIPMENT_SLOTS: usize = 14;

// This narrow decode rejects duplicate authoritative fields before Value's
// last-value semantics can turn conflicting raw facts into a projection.
#[derive(serde::Deserialize)]
struct RankingInspectRawSystems {
    appearance: RankingInspectRawAppearance,
    relationship: RankingInspectRawRelationship,
}

#[derive(serde::Deserialize)]
struct RankingInspectRawAppearance {
    hair: u8,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RankingInspectRawRelationship {
    partner_identity: RankingInspectRawPartner,
}

struct RankingInspectRawPartner(Option<Stage5FriendIdentity>);

impl<'de> serde::Deserialize<'de> for RankingInspectRawPartner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PartnerVisitor;
        impl<'de> serde::de::Visitor<'de> for PartnerVisitor {
            type Value = RankingInspectRawPartner;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("explicit null or a typed partner identity")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where E: serde::de::Error,
            {
                Ok(RankingInspectRawPartner(None))
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where A: serde::de::MapAccess<'de>,
            {
                <Stage5FriendIdentity as serde::Deserialize<'de>>::deserialize(
                    serde::de::value::MapAccessDeserializer::new(map),
                ).map(|identity| RankingInspectRawPartner(Some(identity)))
            }
        }
        // deserialize_any rejects a missing field. deserialize_option would
        // incorrectly accept that missing field as an authoritative null.
        deserializer.deserialize_any(PartnerVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RankingInspectError {
    IndexOutOfRange(u32),
    TargetMissing(i32),
    TargetAmbiguous(i32),
    SaveMissing,
    SaveIdentityMismatch,
    PresenceUnknown,
    OfflineFactsMissing,
    OnlineProjectionMissing,
    ProjectionIdentityMismatch,
    FactMissing(&'static str),
    FactInvalid { fact: &'static str, reason: String },
    HeroProjection,
    EquipmentWidth(usize),
    EquipmentDecode { item: usize, reason: String },
    EquipmentSlotInvalid { item: usize },
    EquipmentSlotDuplicate { slot: usize },
    EquipmentUniqueIdMissing { slot: usize },
    EquipmentUniqueIdDuplicate(u64),
    EquipmentInvalid { slot: usize, reason: String },
}

/// An identity includes its account even though the ranking wire ID does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankingInspectIdentity {
    pub account_id: String,
    pub character_index: i32,
    pub name: String,
}

/// Created only by the full-store unique resolver. A save may be absent while
/// a matching target is online, so save validation happens on the offline path.
#[derive(Debug, Clone, Copy)]
pub struct RankingInspectTarget<'a> {
    account_id: &'a str,
    character: &'a CharacterRecord,
    save: Option<&'a CharacterSaveRecord>,
}

impl RankingInspectTarget<'_> {
    pub fn identity(&self) -> RankingInspectIdentity {
        RankingInspectIdentity {
            account_id: self.account_id.to_owned(),
            character_index: self.character.index,
            name: self.character.name.clone(),
        }
    }

    pub fn character(&self) -> &CharacterRecord {
        self.character
    }

    pub fn saved_character(&self) -> Option<&CharacterSaveRecord> {
        self.save
    }

    fn matches_identity(&self, identity: &RankingInspectIdentity) -> bool {
        self.account_id == identity.account_id
            && self.character.index == identity.character_index
            && self.character.name == identity.name
    }
}

/// Unknown facts stay None. Some(empty string)/Some(false) mean the caller has
/// positively established absence/disabled permission for this exact target.
/// Do not deserialize Stage5 defaults or reuse requester fields to fill these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankingInspectOfflineFacts {
    pub identity: RankingInspectIdentity,
    pub hair: Option<u8>,
    pub guild: Option<RankingInspectGuild>,
    pub lover_name: Option<String>,
    pub player_allows_observe: Option<bool>,
    pub server_allows_observe: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankingInspectGuild {
    pub name: String,
    pub rank: String,
}

/// The caller must obtain this from the matching current online owner, rather
/// than an AOI object projection or a stale saved/offline character snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankingInspectOnlineProjection {
    pub identity: RankingInspectIdentity,
    pub info: PlayerInspectInfo,
    /// The current owner's typed relationship fact. Names are resolved again
    /// from the query's complete account snapshot, rather than cached text.
    pub partner_identity: Option<Stage5FriendIdentity>,
}

/// Online absence/unknown presence cannot silently become OfflinePlayer.
#[derive(Debug, Clone, Copy)]
pub enum RankingInspectSource<'a> {
    OfflinePlayer { facts: Option<&'a RankingInspectOfflineFacts> },
    OnlinePlayer { projection: Option<&'a RankingInspectOnlineProjection> },
    Unknown,
}

/// Crystal ranking Inspect uses the signed CharacterInfo.Index namespace, not
/// ObjectID. Database uniqueness is account/index scoped, so count matches in
/// every account's character roster and reject duplicates even within one row.
pub fn resolve_ranking_inspect_target(
    store: &AccountStore,
    player_id: u32,
) -> Result<RankingInspectTarget<'_>, RankingInspectError> {
    let index = i32::try_from(player_id)
        .map_err(|_| RankingInspectError::IndexOutOfRange(player_id))?;
    let mut target = None;
    for (account_id, account) in &store.accounts {
        for character in &account.characters {
            if character.index != index {
                continue;
            }
            if target.is_some() {
                return Err(RankingInspectError::TargetAmbiguous(index));
            }
            target = Some(RankingInspectTarget {
                account_id,
                character,
                save: account.saves.get(&index),
            });
        }
    }
    target.ok_or(RankingInspectError::TargetMissing(index))
}

/// Read supplemental facts from the same complete authoritative store snapshot
/// that supplied `target`. Raw field presence is required: Stage5's defaults
/// cannot establish hair, marriage absence, or a personal observe permission.
pub fn saved_ranking_inspect_facts(
    store: &AccountStore,
    target: &RankingInspectTarget<'_>,
    server_allows_observe: Option<bool>,
) -> Result<RankingInspectOfflineFacts, RankingInspectError> {
    validate_target_store_snapshot(store, target)?;
    let save = validated_target_save(target)?;
    let encoded = save.stage5_systems_json.as_deref()
        .ok_or(RankingInspectError::FactMissing("stage5Systems"))?;
    let systems: serde_json::Value = serde_json::from_str(encoded).map_err(|error| {
        RankingInspectError::FactInvalid { fact: "stage5Systems", reason: error.to_string() }
    })?;
    systems.get("appearance").and_then(|appearance| appearance.get("hair"))
        .ok_or(RankingInspectError::FactMissing("hair"))?
        .as_u64().and_then(|hair| u8::try_from(hair).ok())
        .ok_or_else(|| RankingInspectError::FactInvalid {
            fact: "hair", reason: "explicit appearance.hair must be a u8".to_owned(),
        })?;
    let partner = systems.get("relationship")
        .and_then(|relationship| relationship.get("partnerIdentity"))
        .ok_or(RankingInspectError::FactMissing("loverName"))?;
    if !partner.is_null() {
        let _: Stage5FriendIdentity = serde_json::from_value(partner.clone())
            .map_err(|error| RankingInspectError::FactInvalid {
                fact: "loverName", reason: error.to_string(),
            })?;
    }
    let strict: RankingInspectRawSystems = serde_json::from_str(encoded).map_err(|error| {
        RankingInspectError::FactInvalid { fact: "stage5Systems", reason: error.to_string() }
    })?;
    let partner_identity = strict.relationship.partner_identity.0;
    Ok(RankingInspectOfflineFacts {
        identity: target.identity(), hair: Some(strict.appearance.hair),
        guild: Some(authoritative_ranking_inspect_guild(store, target)?),
        lover_name: Some(authoritative_ranking_inspect_partner_name(store, partner_identity.as_ref())?),
        // There is no persisted personal AllowObserve fact in CharacterSaveRecord.
        player_allows_observe: None,
        server_allows_observe,
    })
}

/// Typed supplemental facts from the matching current session. Unlike the save
/// reader, this does not need a save or serialize the owner's personal state.
/// The caller establishes the live hair/relationship/GM permission custody.
pub(super) fn ranking_inspect_live_facts(
    store: &AccountStore,
    target: &RankingInspectTarget<'_>,
    hair: u8,
    partner: Option<&Stage5FriendIdentity>,
    player_allows_observe: bool,
    server_allows_observe: bool,
) -> Result<RankingInspectOfflineFacts, RankingInspectError> {
    validate_target_store_snapshot(store, target)?;
    Ok(RankingInspectOfflineFacts {
        identity: target.identity(), hair: Some(hair),
        guild: Some(authoritative_ranking_inspect_guild(store, target)?),
        lover_name: Some(authoritative_ranking_inspect_partner_name(store, partner)?),
        player_allows_observe: Some(player_allows_observe),
        server_allows_observe: Some(server_allows_observe),
    })
}

fn validate_target_store_snapshot(
    store: &AccountStore,
    target: &RankingInspectTarget<'_>,
) -> Result<(), RankingInspectError> {
    let player_id = u32::try_from(target.character.index)
        .map_err(|_| RankingInspectError::ProjectionIdentityMismatch)?;
    let current = resolve_ranking_inspect_target(store, player_id)?;
    let same_save = match (target.save, current.save) {
        (Some(target), Some(current)) => std::ptr::eq(target, current),
        (None, None) => true,
        _ => false,
    };
    if !current.matches_identity(&target.identity())
        || !std::ptr::eq(target.character, current.character)
        || !same_save
    {
        return Err(RankingInspectError::ProjectionIdentityMismatch);
    }
    Ok(())
}

fn authoritative_ranking_inspect_partner_name(
    store: &AccountStore,
    partner: Option<&Stage5FriendIdentity>,
) -> Result<String, RankingInspectError> {
    let Some(identity) = partner else { return Ok(String::new()); };
    // This reference already contains the account. Unrelated accounts may
    // legitimately have the same index and do not make this exact pair ambiguous.
    let account = store.accounts.get(&identity.account_id)
        .ok_or_else(|| RankingInspectError::FactInvalid {
            fact: "loverName", reason: "partner account does not exist".to_owned(),
        })?;
    let mut matches = account.characters.iter()
        .filter(|character| character.index == identity.character_index);
    let character = matches.next().ok_or_else(|| RankingInspectError::FactInvalid {
        fact: "loverName", reason: "partner identity has no current character".to_owned(),
    })?;
    if matches.next().is_some() {
        return Err(RankingInspectError::FactInvalid {
            fact: "loverName", reason: "partner character index is ambiguous within its account".to_owned(),
        });
    }
    // Account custody and uniqueness were checked above; cached partnerName
    // never supplies the inspect name after a rename.
    Ok(character.name.clone())
}

fn authoritative_ranking_inspect_guild(
    store: &AccountStore,
    target: &RankingInspectTarget<'_>,
) -> Result<RankingInspectGuild, RankingInspectError> {
    let mut guild_projection = None;
    for (guild_id, guild) in &store.shared_guilds {
        for member in &guild.members {
            if member.identity.account_id != target.account_id
                || member.identity.character_index != target.character.index
            {
                continue;
            }
            if guild_projection.is_some() {
                return Err(RankingInspectError::FactInvalid {
                    fact: "guild", reason: "target guild membership is ambiguous".to_owned(),
                });
            }
            if guild_id != &guild.id {
                return Err(RankingInspectError::FactInvalid {
                    fact: "guild", reason: "guild storage key differs from its identity".to_owned(),
                });
            }
            let mut rank_indexes = BTreeSet::new();
            for rank in &guild.ranks {
                if !rank_indexes.insert(rank.index) {
                    return Err(RankingInspectError::FactInvalid {
                        fact: "guild", reason: "guild rank index is ambiguous".to_owned(),
                    });
                }
            }
            let rank = guild.ranks.iter().find(|rank| rank.index == member.rank_index)
                .ok_or_else(|| RankingInspectError::FactInvalid {
                    fact: "guild", reason: "target guild rank does not exist".to_owned(),
                })?;
            guild_projection = Some(RankingInspectGuild {
                name: guild.name.clone(), rank: rank.name.clone(),
            });
        }
    }
    Ok(guild_projection.unwrap_or_else(|| RankingInspectGuild {
        name: String::new(), rank: String::new(),
    }))
}

fn validated_target_save<'a>(
    target: &RankingInspectTarget<'a>,
) -> Result<&'a CharacterSaveRecord, RankingInspectError> {
    let save = target.save.ok_or(RankingInspectError::SaveMissing)?;
    // Account custody is supplied by resolver's account.saves lookup. Class and
    // gender remain the authoritative projection's values, including GM changes.
    if save.character.index != target.character.index
        || save.character.name != target.character.name
    {
        return Err(RankingInspectError::SaveIdentityMismatch);
    }
    Ok(save)
}

/// The result is a complete player projection or a rejection, never a partial
/// packet. This function does not decide whether the requester may inspect.
pub fn project_ranking_inspect(
    target: &RankingInspectTarget<'_>,
    source: RankingInspectSource<'_>,
) -> Result<PlayerInspectInfo, RankingInspectError> {
    match source {
        RankingInspectSource::Unknown => Err(RankingInspectError::PresenceUnknown),
        RankingInspectSource::OnlinePlayer { projection } => {
            let projection = projection.ok_or(RankingInspectError::OnlineProjectionMissing)?;
            if !target.matches_identity(&projection.identity)
                || projection.info.name != projection.identity.name
            {
                return Err(RankingInspectError::ProjectionIdentityMismatch);
            }
            if projection.info.is_hero {
                return Err(RankingInspectError::HeroProjection);
            }
            validate_equipment_projection(&projection.info.equipment)?;
            Ok(projection.info.clone())
        }
        RankingInspectSource::OfflinePlayer { facts } => {
            let save = validated_target_save(target)?;
            let facts = facts.ok_or(RankingInspectError::OfflineFactsMissing)?;
            if !target.matches_identity(&facts.identity) {
                return Err(RankingInspectError::ProjectionIdentityMismatch);
            }
            let hair = facts.hair.ok_or(RankingInspectError::FactMissing("hair"))?;
            let guild = facts.guild.as_ref().ok_or(RankingInspectError::FactMissing("guild"))?;
            let lover_name = facts.lover_name.as_ref()
                .ok_or(RankingInspectError::FactMissing("loverName"))?;
            let server_allows_observe = facts.server_allows_observe
                .ok_or(RankingInspectError::FactMissing("serverAllowsObserve"))?;
            let allow_observe = if server_allows_observe {
                facts.player_allows_observe
                    .ok_or(RankingInspectError::FactMissing("playerAllowsObserve"))?
            } else {
                false
            };
            let equipment = saved_equipment_projection(save)?;
            Ok(PlayerInspectInfo {
                name: save.character.name.clone(),
                guild_name: guild.name.clone(),
                guild_rank: guild.rank.clone(),
                equipment,
                class: save.character.class,
                gender: save.character.gender,
                hair,
                level: save.character.level,
                lover_name: lover_name.clone(),
                allow_observe,
                // OfflinePlayer explicitly identifies the non-Hero protocol path.
                is_hero: false,
            })
        }
    }
}

fn saved_equipment_projection(
    save: &CharacterSaveRecord,
) -> Result<Vec<Option<UserItem>>, RankingInspectError> {
    let equipment = save.equipment_items_json.iter().enumerate().map(|(item_index, encoded)| {
        serde_json::from_str(encoded).map_err(|error| {
            RankingInspectError::EquipmentDecode { item: item_index, reason: error.to_string() }
        })
    }).collect::<Result<Vec<EquipmentState>, _>>()?;
    ranking_inspect_equipment_projection(&equipment)
}

/// Strict projection shared by saved equipment and the current online owner.
/// A malformed carrier cannot silently become an empty inspect equipment cell.
pub(super) fn ranking_inspect_equipment_projection(
    items: &[EquipmentState],
) -> Result<Vec<Option<UserItem>>, RankingInspectError> {
    let mut slots = vec![None; INSPECT_EQUIPMENT_SLOTS];
    for (item_index, equipment) in items.iter().enumerate() {
        let slot = equipment_slot_index(equipment.slot)
            .filter(|slot| *slot < INSPECT_EQUIPMENT_SLOTS)
            .ok_or(RankingInspectError::EquipmentSlotInvalid { item: item_index })?;
        if slots[slot].is_some() {
            return Err(RankingInspectError::EquipmentSlotDuplicate { slot });
        }
        // Some(0) is a real UID. A legacy None would synthesize a slot UID,
        // which cannot establish a target item instance for inspect.
        let unique_id = equipment.user_item_unique_id
            .ok_or(RankingInspectError::EquipmentUniqueIdMissing { slot })?;
        let carrier = item_state_from_equipment_state(equipment.clone(), ItemContainer::Bag1, slot as u8);
        validate_committed_item_state_carrier(&carrier).map_err(|error| {
            RankingInspectError::EquipmentInvalid { slot, reason: error.to_string() }
        })?;
        if !item_state_can_equip_to_slot(&carrier, equipment.slot) {
            return Err(RankingInspectError::EquipmentInvalid {
                slot, reason: "equipment item cannot occupy its actual slot".to_owned(),
            });
        }
        let item = try_user_item_from_item_state(&carrier).map_err(|error| {
            RankingInspectError::EquipmentInvalid { slot, reason: error.to_string() }
        })?;
        if item.unique_id != unique_id {
            return Err(RankingInspectError::EquipmentInvalid {
                slot, reason: "equipment conversion changed the exact root UID".to_owned(),
            });
        }
        slots[slot] = Some(item);
    }
    validate_equipment_projection(&slots)?;
    Ok(slots)
}

fn validate_equipment_projection(
    slots: &[Option<UserItem>],
) -> Result<(), RankingInspectError> {
    if slots.len() != INSPECT_EQUIPMENT_SLOTS {
        return Err(RankingInspectError::EquipmentWidth(slots.len()));
    }
    let mut unique_ids = BTreeSet::new();
    for (slot, item) in slots.iter().enumerate() {
        let Some(item) = item else { continue; };
        validate_committed_user_item_carrier(item).map_err(|error| {
            RankingInspectError::EquipmentInvalid { slot, reason: error.to_string() }
        })?;
        let actual_slot = [
            EquipmentSlot::Weapon, EquipmentSlot::Armour, EquipmentSlot::Helmet,
            EquipmentSlot::Torch, EquipmentSlot::Necklace, EquipmentSlot::BraceletLeft,
            EquipmentSlot::BraceletRight, EquipmentSlot::RingLeft, EquipmentSlot::RingRight,
            EquipmentSlot::Amulet, EquipmentSlot::Belt, EquipmentSlot::Boots,
            EquipmentSlot::Stone, EquipmentSlot::Mount,
        ][slot];
        let template = mir2_game_data::crystal_item_by_index(item.item_index)
            .ok_or_else(|| RankingInspectError::EquipmentInvalid {
                slot, reason: "equipment template is missing".to_owned(),
            })?;
        if !crystal_template_can_equip_to_slot(&template, actual_slot) {
            return Err(RankingInspectError::EquipmentInvalid {
                slot, reason: "equipment item cannot occupy its actual slot".to_owned(),
            });
        }
        validate_equipment_tree_unique_ids(item, &mut unique_ids)?;
    }
    Ok(())
}

// Carrier validation bounds each complete tree before this recursion. Holes
// have no identity; every actual root/descendant, including UID 0, shares a set.
fn validate_equipment_tree_unique_ids(
    item: &UserItem,
    unique_ids: &mut BTreeSet<u64>,
) -> Result<(), RankingInspectError> {
    if !unique_ids.insert(item.unique_id) {
        return Err(RankingInspectError::EquipmentUniqueIdDuplicate(item.unique_id));
    }
    for embedded in item.slots.iter().flatten() {
        validate_equipment_tree_unique_ids(embedded, unique_ids)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use crate::config::{AccountRecord, SharedGuildMember, SharedGuildRank, SharedGuildRecord};
    use mir2_protocol::{MirClass, MirGender};
    use super::super::equipment::equipment_state_from_item_state;
    use super::super::items::{embedded_item_state_from_template, try_item_state_from_user_item};

    fn character(index: i32, name: &str) -> CharacterRecord {
        CharacterRecord {
            index, name: name.to_owned(), level: 48,
            class: MirClass::Taoist, gender: MirGender::Female,
        }
    }

    fn store() -> AccountStore {
        let mut store = AccountStore::new(character(7, "Target"));
        store.accounts.clear();
        store.accounts.insert("owner".into(), AccountRecord::new(character(7, "Target")));
        store.accounts.insert("requester".into(), AccountRecord::new(character(91, "Viewer")));
        store
    }

    fn facts(target: &RankingInspectTarget<'_>) -> RankingInspectOfflineFacts {
        RankingInspectOfflineFacts {
            identity: target.identity(), hair: Some(9),
            guild: Some(RankingInspectGuild { name: "TargetGuild".into(), rank: "Elder".into() }),
            lover_name: Some("TargetLover".into()),
            player_allows_observe: Some(true), server_allows_observe: Some(true),
        }
    }

    fn equipment(slot: EquipmentSlot, unique_id: u64) -> EquipmentState {
        let template = mir2_game_data::crystal_item_manifest_ref().items.iter()
            .find(|item| crystal_template_can_equip_to_slot(item, slot))
            .expect("equipment catalog fixture");
        let mut item = embedded_item_state_from_template(template, ItemContainer::Bag1, 0);
        item.unique_id = unique_id;
        equipment_state_from_item_state(&item, slot)
    }

    fn user_item_of_type(item_type: u8, unique_id: u64) -> UserItem {
        let template = mir2_game_data::crystal_item_manifest_ref().items.iter()
            .find(|item| item.item_type == item_type).expect("item catalog fixture");
        let mut item = embedded_item_state_from_template(template, ItemContainer::Bag1, 0);
        item.unique_id = unique_id;
        let mut protocol = try_user_item_from_item_state(&item).expect("complete item fixture");
        protocol.unique_id = unique_id;
        protocol
    }

    fn socket_container_child(unique_id: u64) -> UserItem {
        // "Socket" in these tests means the recursive UserItem.slots container,
        // which accepts complete catalog-backed children. The embedded catalogue
        // has real Bells (BronzeBell), but no CRYSTAL_ITEM_TYPE_SOCKET template.
        user_item_of_type(super::super::crystal_compat::CRYSTAL_ITEM_TYPE_BELLS, unique_id)
    }

    fn with_sockets(item: EquipmentState, slots: Vec<Option<UserItem>>) -> EquipmentState {
        let slot = item.slot;
        let carrier = item_state_from_equipment_state(item, ItemContainer::Bag1, 0);
        let mut protocol = try_user_item_from_item_state(&carrier).unwrap();
        protocol.slots = slots;
        let hydrated = try_item_state_from_user_item(carrier, &protocol).unwrap();
        equipment_state_from_item_state(&hydrated, slot)
    }

    fn encode_equipment(item: &EquipmentState) -> String {
        serde_json::to_string(item).expect("equipment fixture")
    }

    fn set_equipment(store: &mut AccountStore, items: Vec<String>) {
        store.accounts.get_mut("owner").unwrap().saves.get_mut(&7).unwrap()
            .equipment_items_json = items;
    }

    fn offline(store: &AccountStore) -> Result<PlayerInspectInfo, RankingInspectError> {
        let target = resolve_ranking_inspect_target(store, 7)?;
        let facts = facts(&target);
        project_ranking_inspect(&target, RankingInspectSource::OfflinePlayer { facts: Some(&facts) })
    }

    fn online_equipment(equipment: Vec<Option<UserItem>>) -> Result<PlayerInspectInfo, RankingInspectError> {
        let store = store();
        let target = resolve_ranking_inspect_target(&store, 7)?;
        let mut info = offline(&store)?;
        info.equipment = equipment;
        let projection = RankingInspectOnlineProjection { identity: target.identity(), info, partner_identity: None };
        project_ranking_inspect(&target, RankingInspectSource::OnlinePlayer { projection: Some(&projection) })
    }

    fn set_systems(store: &mut AccountStore, systems: serde_json::Value) {
        store.accounts.get_mut("owner").unwrap().saves.get_mut(&7).unwrap()
            .stage5_systems_json = Some(systems.to_string());
    }

    fn explicit_systems(partner: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "appearance": { "hair": 9 },
            "relationship": { "partnerIdentity": partner, "partnerName": "StaleCachedName" },
            "guild": { "name": "LegacyGuild", "rank": "LegacyRank" },
            "allowObserve": true
        })
    }

    fn saved_facts(store: &AccountStore, policy: Option<bool>)
        -> Result<RankingInspectOfflineFacts, RankingInspectError>
    {
        let target = resolve_ranking_inspect_target(store, 7)?;
        saved_ranking_inspect_facts(store, &target, policy)
    }

    fn guild(id: &str, account_id: &str, index: i32, name: &str) -> SharedGuildRecord {
        SharedGuildRecord {
            id: id.to_owned(), name: name.to_owned(), revision: 1,
            level: 1, experience: 0, spare_points: 0, gold: 0,
            ranks: vec![SharedGuildRank { index: 0, name: "Leader".into(), options: 0 }],
            members: vec![SharedGuildMember {
                membership_epoch: 1,
                identity: Stage5FriendIdentity { account_id: account_id.into(), character_index: index },
                name: "CachedMemberName".into(), rank_index: 0,
            }],
            notice: Vec::new(), storage: BTreeMap::new(), buffs: BTreeMap::new(),
            last_buff_tick_ms: 0, experience_receipts: BTreeSet::new(),
            experience_receipt_payloads: BTreeMap::new(),
        }
    }

    #[test]
    fn ranking_inspect_checks_signed_index_and_missing_target() {
        let store = store();
        assert!(matches!(resolve_ranking_inspect_target(&store, u32::MAX),
            Err(RankingInspectError::IndexOutOfRange(u32::MAX))));
        assert!(matches!(resolve_ranking_inspect_target(&store, i32::MAX as u32 + 1),
            Err(RankingInspectError::IndexOutOfRange(_))));
        assert!(matches!(resolve_ranking_inspect_target(&store, 0),
            Err(RankingInspectError::TargetMissing(0))));
    }

    #[test]
    fn ranking_inspect_rejects_cross_account_and_same_account_duplicates() {
        let mut store = store();
        store.accounts.insert("duplicate".into(), AccountRecord::new(character(7, "Other")));
        assert!(matches!(resolve_ranking_inspect_target(&store, 7),
            Err(RankingInspectError::TargetAmbiguous(7))));
        store.accounts.remove("duplicate");
        store.accounts.get_mut("owner").unwrap().characters.push(character(7, "Other"));
        assert!(matches!(resolve_ranking_inspect_target(&store, 7),
            Err(RankingInspectError::TargetAmbiguous(7))));
    }

    #[test]
    fn ranking_inspect_projects_target_fields_and_exact_zero_uid_with_holes() {
        let mut store = store();
        set_equipment(&mut store, vec![encode_equipment(&equipment(EquipmentSlot::Weapon, 0))]);
        let info = offline(&store).unwrap();
        assert_eq!(info.name, "Target");
        assert_eq!(info.class, MirClass::Taoist);
        assert_eq!(info.gender, MirGender::Female);
        assert_eq!(info.level, 48);
        assert_eq!(info.hair, 9);
        assert_eq!(info.guild_name, "TargetGuild");
        assert_eq!(info.guild_rank, "Elder");
        assert_eq!(info.lover_name, "TargetLover");
        assert!(info.allow_observe);
        assert!(!info.is_hero);
        assert_eq!(info.equipment.len(), 14);
        assert_eq!(info.equipment[0].as_ref().unwrap().unique_id, 0);
        assert!(info.equipment[1..].iter().all(Option::is_none));
    }

    #[test]
    fn ranking_inspect_empty_saved_equipment_stays_empty() {
        let info = offline(&store()).unwrap();
        assert_eq!(info.equipment, vec![None; 14]);
    }

    #[test]
    fn ranking_inspect_rejects_missing_save_and_mismatched_save_identity() {
        let mut store = store();
        store.accounts.get_mut("owner").unwrap().saves.remove(&7);
        assert_eq!(offline(&store), Err(RankingInspectError::SaveMissing));
        store.accounts.get_mut("owner").unwrap().saves
            .insert(7, CharacterSaveRecord::new(character(7, "Viewer")));
        assert_eq!(offline(&store), Err(RankingInspectError::SaveIdentityMismatch));
        store.accounts.get_mut("owner").unwrap().saves
            .insert(7, CharacterSaveRecord::new(character(91, "Target")));
        assert_eq!(offline(&store), Err(RankingInspectError::SaveIdentityMismatch));
    }

    #[test]
    fn ranking_inspect_rejects_duplicate_slot_and_duplicate_exact_uid() {
        let mut store = store();
        let weapon = equipment(EquipmentSlot::Weapon, 0);
        set_equipment(&mut store, vec![encode_equipment(&weapon), encode_equipment(&weapon)]);
        assert_eq!(offline(&store), Err(RankingInspectError::EquipmentSlotDuplicate { slot: 0 }));
        let other = equipment(EquipmentSlot::Armour, 0);
        set_equipment(&mut store, vec![encode_equipment(&weapon), encode_equipment(&other)]);
        assert_eq!(offline(&store), Err(RankingInspectError::EquipmentUniqueIdDuplicate(0)));
    }

    #[test]
    fn ranking_inspect_rejects_malformed_unknown_and_zero_count_items() {
        let mut store = store();
        set_equipment(&mut store, vec!["{".into()]);
        assert!(matches!(offline(&store), Err(RankingInspectError::EquipmentDecode { .. })));
        let mut item = equipment(EquipmentSlot::Weapon, 0);
        item.key = "crystal-item-2147483647".into();
        item.user_item_metadata = None;
        set_equipment(&mut store, vec![encode_equipment(&item)]);
        assert!(matches!(offline(&store), Err(RankingInspectError::EquipmentInvalid { .. })));
        item = equipment(EquipmentSlot::Weapon, 0);
        item.quantity = 0;
        set_equipment(&mut store, vec![encode_equipment(&item)]);
        assert!(matches!(offline(&store), Err(RankingInspectError::EquipmentInvalid { .. })));
    }

    #[test]
    fn ranking_inspect_rejects_legacy_synthetic_equipment_uid() {
        let mut store = store();
        let mut item = equipment(EquipmentSlot::Weapon, 0);
        item.user_item_unique_id = None;
        set_equipment(&mut store, vec![encode_equipment(&item)]);
        assert_eq!(offline(&store), Err(RankingInspectError::EquipmentUniqueIdMissing { slot: 0 }));
    }

    #[test]
    fn ranking_inspect_unknown_facts_and_foreign_facts_are_rejected() {
        let store = store();
        let target = resolve_ranking_inspect_target(&store, 7).unwrap();
        let complete = facts(&target);
        for field in ["hair", "guild", "loverName", "playerAllowsObserve", "serverAllowsObserve"] {
            let mut facts = complete.clone();
            match field {
                "hair" => facts.hair = None,
                "guild" => facts.guild = None,
                "loverName" => facts.lover_name = None,
                "playerAllowsObserve" => facts.player_allows_observe = None,
                _ => facts.server_allows_observe = None,
            }
            assert_eq!(project_ranking_inspect(&target,
                RankingInspectSource::OfflinePlayer { facts: Some(&facts) }),
                Err(RankingInspectError::FactMissing(field)));
        }
        let mut foreign = complete;
        foreign.identity.account_id = "requester".into();
        assert_eq!(project_ranking_inspect(&target,
            RankingInspectSource::OfflinePlayer { facts: Some(&foreign) }),
            Err(RankingInspectError::ProjectionIdentityMismatch));
    }

    #[test]
    fn ranking_inspect_known_absence_and_observe_policy_are_explicit() {
        let store = store();
        let target = resolve_ranking_inspect_target(&store, 7).unwrap();
        let mut facts = facts(&target);
        facts.guild = Some(RankingInspectGuild { name: String::new(), rank: String::new() });
        facts.lover_name = Some(String::new());
        for (player, server) in [(false, false), (false, true), (true, false), (true, true)] {
            facts.player_allows_observe = Some(player);
            facts.server_allows_observe = Some(server);
            let info = project_ranking_inspect(&target,
                RankingInspectSource::OfflinePlayer { facts: Some(&facts) }).unwrap();
            assert_eq!(info.allow_observe, player && server);
            assert!(info.guild_name.is_empty() && info.guild_rank.is_empty() && info.lover_name.is_empty());
        }
    }

    #[test]
    fn ranking_inspect_online_missing_and_unknown_never_fall_back() {
        let store = store();
        let target = resolve_ranking_inspect_target(&store, 7).unwrap();
        assert_eq!(project_ranking_inspect(&target, RankingInspectSource::Unknown),
            Err(RankingInspectError::PresenceUnknown));
        assert_eq!(project_ranking_inspect(&target,
            RankingInspectSource::OnlinePlayer { projection: None }),
            Err(RankingInspectError::OnlineProjectionMissing));
        assert_eq!(project_ranking_inspect(&target,
            RankingInspectSource::OfflinePlayer { facts: None }),
            Err(RankingInspectError::OfflineFactsMissing));
    }

    #[test]
    fn ranking_inspect_online_uses_authoritative_projection_and_validates_identity() {
        let mut store = store();
        let mut info = offline(&store).unwrap();
        info.level = 60;
        info.hair = 11;
        info.class = MirClass::Warrior;
        info.gender = MirGender::Male;
        store.accounts.get_mut("owner").unwrap().saves.remove(&7);
        let target = resolve_ranking_inspect_target(&store, 7).unwrap();
        let mut projection = RankingInspectOnlineProjection { identity: target.identity(), info, partner_identity: None };
        let current = project_ranking_inspect(&target,
            RankingInspectSource::OnlinePlayer { projection: Some(&projection) }).unwrap();
        assert_eq!(current.level, 60);
        assert_eq!(current.class, MirClass::Warrior);
        assert_eq!(current.gender, MirGender::Male);
        projection.identity.account_id = "requester".into();
        assert_eq!(project_ranking_inspect(&target,
            RankingInspectSource::OnlinePlayer { projection: Some(&projection) }),
            Err(RankingInspectError::ProjectionIdentityMismatch));
        projection.identity = target.identity();
        projection.info.is_hero = true;
        assert_eq!(project_ranking_inspect(&target,
            RankingInspectSource::OnlinePlayer { projection: Some(&projection) }),
            Err(RankingInspectError::HeroProjection));
        projection.info.is_hero = false;
        projection.info.equipment.pop();
        assert_eq!(project_ranking_inspect(&target,
            RankingInspectSource::OnlinePlayer { projection: Some(&projection) }),
            Err(RankingInspectError::EquipmentWidth(13)));
    }

    #[test]
    fn ranking_inspect_rejects_weapon_in_armour_offline_and_online() {
        let mut store = store();
        let weapon = equipment(EquipmentSlot::Weapon, 101);
        let mut misplaced = equipment(EquipmentSlot::Weapon, 102);
        misplaced.slot = EquipmentSlot::Armour;
        set_equipment(&mut store, vec![encode_equipment(&weapon), encode_equipment(&misplaced)]);
        assert!(matches!(offline(&store), Err(RankingInspectError::EquipmentInvalid { slot: 1, .. })));
        assert!(matches!(ranking_inspect_equipment_projection(&[weapon.clone(), misplaced]),
            Err(RankingInspectError::EquipmentInvalid { slot: 1, .. })));
        let mut slots = ranking_inspect_equipment_projection(&[weapon]).unwrap();
        let mut wrong_slot = slots[0].as_ref().unwrap().clone();
        wrong_slot.unique_id = 102;
        slots[1] = Some(wrong_slot);
        assert!(matches!(online_equipment(slots),
            Err(RankingInspectError::EquipmentInvalid { slot: 1, .. })));
    }

    #[test]
    fn ranking_inspect_accepts_right_amulet_and_paired_accessory_slots() {
        let amulet = user_item_of_type(super::super::crystal_compat::CRYSTAL_ITEM_TYPE_AMULET, 0);
        let template = mir2_game_data::crystal_item_by_index(amulet.item_index).unwrap();
        let mut carrier = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
        carrier.unique_id = 0;
        let right_amulet = equipment_state_from_item_state(&carrier, EquipmentSlot::BraceletRight);
        let items = vec![
            right_amulet,
            equipment(EquipmentSlot::BraceletLeft, 11),
            equipment(EquipmentSlot::RingLeft, 12),
            equipment(EquipmentSlot::RingRight, 13),
        ];
        let slots = ranking_inspect_equipment_projection(&items).unwrap();
        assert_eq!(slots[6].as_ref().unwrap().item_index, amulet.item_index);
        assert_eq!(slots[6].as_ref().unwrap().unique_id, 0);
        let mut store = store();
        set_equipment(&mut store, items.iter().map(encode_equipment).collect());
        assert_eq!(offline(&store).unwrap().equipment, slots);
        assert_eq!(online_equipment(slots.clone()).unwrap().equipment, slots);
    }

    #[test]
    fn ranking_inspect_rejects_duplicate_socket_uid_across_roots() {
        let socket = socket_container_child(77);
        let items = vec![
            with_sockets(equipment(EquipmentSlot::Weapon, 101), vec![None, Some(socket.clone()), None]),
            with_sockets(equipment(EquipmentSlot::Armour, 102), vec![Some(socket.clone())]),
        ];
        let mut store = store();
        set_equipment(&mut store, items.iter().map(encode_equipment).collect());
        assert_eq!(offline(&store), Err(RankingInspectError::EquipmentUniqueIdDuplicate(77)));
        assert_eq!(ranking_inspect_equipment_projection(&items),
            Err(RankingInspectError::EquipmentUniqueIdDuplicate(77)));
        // Each tree is individually valid; only their shared UID namespace conflicts.
        let mut slots = ranking_inspect_equipment_projection(&items[..1]).unwrap();
        slots[1] = ranking_inspect_equipment_projection(&items[1..]).unwrap()[1].clone();
        assert_eq!(online_equipment(slots), Err(RankingInspectError::EquipmentUniqueIdDuplicate(77)));
    }

    #[test]
    fn ranking_inspect_rejects_socket_uid_matching_own_or_other_root() {
        for root_uid in [101, 102] {
            let socket = socket_container_child(root_uid);
            let items = vec![
                with_sockets(equipment(EquipmentSlot::Weapon, 101), vec![Some(socket)]),
                equipment(EquipmentSlot::Armour, 102),
            ];
            let mut store = store();
            set_equipment(&mut store, items.iter().map(encode_equipment).collect());
            assert_eq!(offline(&store), Err(RankingInspectError::EquipmentUniqueIdDuplicate(root_uid)));
            let mut slots = vec![None; 14];
            for item in &items {
                let slot = equipment_slot_index(item.slot).unwrap();
                let carrier = item_state_from_equipment_state(item.clone(), ItemContainer::Bag1, slot as u8);
                slots[slot] = Some(try_user_item_from_item_state(&carrier).unwrap());
            }
            assert_eq!(online_equipment(slots),
                Err(RankingInspectError::EquipmentUniqueIdDuplicate(root_uid)));
        }
    }

    #[test]
    fn ranking_inspect_preserves_zero_uid_socket_and_socket_holes() {
        let mut socket = socket_container_child(77);
        let nested_zero = socket_container_child(0);
        socket.slots = vec![None, Some(nested_zero), None];
        let root = with_sockets(equipment(EquipmentSlot::Weapon, 101), vec![None, Some(socket), None]);
        let expected = ranking_inspect_equipment_projection(std::slice::from_ref(&root)).unwrap();
        let mut store = store();
        set_equipment(&mut store, vec![encode_equipment(&root)]);
        assert_eq!(offline(&store).unwrap().equipment, expected);
        assert_eq!(online_equipment(expected.clone()).unwrap().equipment, expected);
        let sockets = &expected[0].as_ref().unwrap().slots;
        assert!(sockets[0].is_none() && sockets[2].is_none());
        let nested = &sockets[1].as_ref().unwrap().slots;
        assert!(nested[0].is_none() && nested[2].is_none());
        assert_eq!(nested[1].as_ref().unwrap().unique_id, 0);
    }

    #[test]
    fn ranking_inspect_offline_keeps_authoritative_saved_class_and_gender() {
        let mut store = store();
        let save = store.accounts.get_mut("owner").unwrap().saves.get_mut(&7).unwrap();
        save.character.class = MirClass::Warrior;
        save.character.gender = MirGender::Male;
        let info = offline(&store).unwrap();
        assert_eq!(info.class, MirClass::Warrior);
        assert_eq!(info.gender, MirGender::Male);
    }

    #[test]
    fn ranking_inspect_saved_facts_require_explicit_raw_hair_and_partner_identity() {
        let mut store = store();
        assert_eq!(saved_facts(&store, Some(false)), Err(RankingInspectError::FactMissing("stage5Systems")));
        store.accounts.get_mut("owner").unwrap().saves.get_mut(&7).unwrap()
            .stage5_systems_json = Some("{".into());
        assert!(matches!(saved_facts(&store, Some(false)),
            Err(RankingInspectError::FactInvalid { fact: "stage5Systems", .. })));
        for raw in [
            serde_json::json!({ "relationship": { "partnerIdentity": null } }),
            serde_json::json!({ "appearance": {}, "relationship": { "partnerIdentity": null } }),
        ] {
            set_systems(&mut store, raw);
            assert_eq!(saved_facts(&store, Some(false)), Err(RankingInspectError::FactMissing("hair")));
        }
        for hair in [serde_json::json!(null), serde_json::json!(-1), serde_json::json!(256),
            serde_json::json!(1.5), serde_json::json!("9")]
        {
            let mut raw = explicit_systems(serde_json::Value::Null);
            raw["appearance"]["hair"] = hair;
            set_systems(&mut store, raw);
            assert!(matches!(saved_facts(&store, Some(false)),
                Err(RankingInspectError::FactInvalid { fact: "hair", .. })));
        }
        for relationship in [serde_json::json!({}), serde_json::json!({ "partnerName": "CachedOnly" })] {
            let mut raw = explicit_systems(serde_json::Value::Null);
            raw["relationship"] = relationship;
            set_systems(&mut store, raw);
            assert_eq!(saved_facts(&store, Some(false)), Err(RankingInspectError::FactMissing("loverName")));
        }
        for partner in [serde_json::json!(false), serde_json::json!({ "accountId": "requester" }),
            serde_json::json!({ "accountId": "requester", "characterIndex": "91" })]
        {
            set_systems(&mut store, explicit_systems(partner));
            assert!(matches!(saved_facts(&store, Some(false)),
                Err(RankingInspectError::FactInvalid { fact: "loverName", .. })));
        }
    }

    #[test]
    fn ranking_inspect_saved_facts_resolve_current_partner_name_and_reject_ambiguity() {
        let mut store = store();
        let identity = serde_json::json!({ "accountId": "requester", "characterIndex": 91 });
        store.accounts.get_mut("requester").unwrap().characters[0].name = "CurrentPartnerName".into();
        set_systems(&mut store, explicit_systems(identity.clone()));
        assert_eq!(saved_facts(&store, Some(false)).unwrap().lover_name, Some("CurrentPartnerName".into()));
        for bad in [serde_json::json!({ "accountId": "owner", "characterIndex": 91 }),
            serde_json::json!({ "accountId": "requester", "characterIndex": 92 })]
        {
            set_systems(&mut store, explicit_systems(bad));
            assert!(matches!(saved_facts(&store, Some(false)),
                Err(RankingInspectError::FactInvalid { fact: "loverName", .. })));
        }
        set_systems(&mut store, explicit_systems(identity));
        store.accounts.get_mut("requester").unwrap().characters.push(character(91, "Duplicate"));
        assert!(matches!(saved_facts(&store, Some(false)),
            Err(RankingInspectError::FactInvalid { fact: "loverName", .. })));
        store.accounts.get_mut("requester").unwrap().characters.pop();
        store.accounts.insert("other".into(), AccountRecord::new(character(91, "Duplicate")));
        assert_eq!(saved_facts(&store, Some(false)).unwrap().lover_name,
            Some("CurrentPartnerName".into()));
        let target = resolve_ranking_inspect_target(&store, 7).unwrap();
        let partner = Stage5FriendIdentity { account_id: "requester".into(), character_index: 91 };
        assert_eq!(ranking_inspect_live_facts(&store, &target, 9, Some(&partner), false, false)
            .unwrap().lover_name, Some("CurrentPartnerName".into()));
    }

    #[test]
    fn ranking_inspect_saved_facts_reject_duplicate_raw_authority_fields() {
        let mut store = store();
        for raw in [
            r#"{"appearance":{"hair":1,"hair":9},"relationship":{"partnerIdentity":null}}"#,
            r#"{"appearance":{"hair":1},"appearance":{"hair":9},"relationship":{"partnerIdentity":null}}"#,
            r#"{"appearance":{"hair":9},"relationship":{"partnerIdentity":{"accountId":"requester","characterIndex":91},"partnerIdentity":null}}"#,
            r#"{"appearance":{"hair":9},"relationship":{"partnerIdentity":{"accountId":"owner","accountId":"requester","characterIndex":91}}}"#,
            r#"{"appearance":{"hair":9},"relationship":{"partnerIdentity":{"accountId":"requester","characterIndex":7,"characterIndex":91}}}"#,
        ] {
            store.accounts.get_mut("owner").unwrap().saves.get_mut(&7).unwrap()
                .stage5_systems_json = Some(raw.into());
            assert!(matches!(saved_facts(&store, Some(false)),
                Err(RankingInspectError::FactInvalid { fact: "stage5Systems", .. })));
        }
    }

    #[test]
    fn ranking_inspect_saved_facts_use_target_shared_guild_and_reject_member_or_rank_ambiguity() {
        const TARGET_GUILD: &str = "0123456789abcdef0123456789abcdef";
        const OTHER_GUILD: &str = "1123456789abcdef0123456789abcdef";
        let mut store = store();
        set_systems(&mut store, explicit_systems(serde_json::Value::Null));
        store.shared_guilds.insert(OTHER_GUILD.into(), guild(OTHER_GUILD, "requester", 91, "RequesterGuild"));
        let absent = saved_facts(&store, Some(false)).unwrap();
        assert_eq!(absent.guild, Some(RankingInspectGuild { name: String::new(), rank: String::new() }));
        store.shared_guilds.insert(TARGET_GUILD.into(), guild(TARGET_GUILD, "owner", 7, "CurrentTargetGuild"));
        assert_eq!(saved_facts(&store, Some(false)).unwrap().guild,
            Some(RankingInspectGuild { name: "CurrentTargetGuild".into(), rank: "Leader".into() }));
        let member = store.shared_guilds[TARGET_GUILD].members[0].clone();
        store.shared_guilds.get_mut(TARGET_GUILD).unwrap().members.push(member.clone());
        assert!(matches!(saved_facts(&store, Some(false)),
            Err(RankingInspectError::FactInvalid { fact: "guild", .. })));
        store.shared_guilds.get_mut(TARGET_GUILD).unwrap().members.pop();
        store.shared_guilds.get_mut(OTHER_GUILD).unwrap().members.push(member);
        assert!(matches!(saved_facts(&store, Some(false)),
            Err(RankingInspectError::FactInvalid { fact: "guild", .. })));
        store.shared_guilds.get_mut(OTHER_GUILD).unwrap().members.pop();
        let rank = store.shared_guilds[TARGET_GUILD].ranks[0].clone();
        store.shared_guilds.get_mut(TARGET_GUILD).unwrap().ranks.push(rank);
        assert!(matches!(saved_facts(&store, Some(false)),
            Err(RankingInspectError::FactInvalid { fact: "guild", .. })));
        store.shared_guilds.get_mut(TARGET_GUILD).unwrap().ranks.pop();
        store.shared_guilds.get_mut(TARGET_GUILD).unwrap().members[0].rank_index = 5;
        assert!(matches!(saved_facts(&store, Some(false)),
            Err(RankingInspectError::FactInvalid { fact: "guild", .. })));
    }

    #[test]
    fn ranking_inspect_saved_facts_false_server_policy_accepts_unknown_player_observe() {
        let mut store = store();
        set_systems(&mut store, explicit_systems(serde_json::Value::Null));
        let target = resolve_ranking_inspect_target(&store, 7).unwrap();
        let facts = saved_ranking_inspect_facts(&store, &target, Some(false)).unwrap();
        assert_eq!(facts.hair, Some(9));
        assert_eq!(facts.lover_name, Some(String::new()));
        assert_eq!(facts.player_allows_observe, None);
        assert!(!project_ranking_inspect(&target,
            RankingInspectSource::OfflinePlayer { facts: Some(&facts) }).unwrap().allow_observe);
        let enabled = saved_ranking_inspect_facts(&store, &target, Some(true)).unwrap();
        assert_eq!(project_ranking_inspect(&target,
            RankingInspectSource::OfflinePlayer { facts: Some(&enabled) }),
            Err(RankingInspectError::FactMissing("playerAllowsObserve")));
        let unknown = saved_ranking_inspect_facts(&store, &target, None).unwrap();
        assert_eq!(project_ranking_inspect(&target,
            RankingInspectSource::OfflinePlayer { facts: Some(&unknown) }),
            Err(RankingInspectError::FactMissing("serverAllowsObserve")));
    }

    #[test]
    fn ranking_inspect_saved_facts_reject_a_different_store_snapshot() {
        let mut store = store();
        set_systems(&mut store, explicit_systems(serde_json::Value::Null));
        let other_snapshot = store.clone();
        let target = resolve_ranking_inspect_target(&store, 7).unwrap();
        assert_eq!(saved_ranking_inspect_facts(&other_snapshot, &target, Some(false)),
            Err(RankingInspectError::ProjectionIdentityMismatch));
    }

    #[test]
    fn ranking_inspect_live_facts_use_typed_owner_fields_without_a_save() {
        const GUILD: &str = "0123456789abcdef0123456789abcdef";
        let mut store = store();
        store.accounts.get_mut("owner").unwrap().saves.remove(&7);
        store.accounts.get_mut("requester").unwrap().characters[0].name = "CurrentLivePartner".into();
        store.shared_guilds.insert(GUILD.into(), guild(GUILD, "owner", 7, "CurrentLiveGuild"));
        let target = resolve_ranking_inspect_target(&store, 7).unwrap();
        let partner = Stage5FriendIdentity { account_id: "requester".into(), character_index: 91 };
        let facts = ranking_inspect_live_facts(&store, &target, 255, Some(&partner), true, true).unwrap();
        assert_eq!(facts.identity, target.identity());
        assert_eq!(facts.hair, Some(255));
        assert_eq!(facts.lover_name, Some("CurrentLivePartner".into()));
        assert_eq!(facts.guild,
            Some(RankingInspectGuild { name: "CurrentLiveGuild".into(), rank: "Leader".into() }));
        assert_eq!(facts.player_allows_observe, Some(true));
        assert_eq!(facts.server_allows_observe, Some(true));
        let absent = ranking_inspect_live_facts(&store, &target, 0, None, false, false).unwrap();
        assert_eq!(absent.lover_name, Some(String::new()));
        assert_eq!(absent.player_allows_observe, Some(false));
    }

    #[test]
    fn ranking_inspect_live_facts_share_snapshot_and_partner_guild_validation() {
        const GUILD: &str = "0123456789abcdef0123456789abcdef";
        let mut store = store();
        let other_snapshot = store.clone();
        let target = resolve_ranking_inspect_target(&store, 7).unwrap();
        assert_eq!(ranking_inspect_live_facts(&other_snapshot, &target, 0, None, false, false),
            Err(RankingInspectError::ProjectionIdentityMismatch));
        let missing_partner = Stage5FriendIdentity { account_id: "other".into(), character_index: 91 };
        assert!(matches!(ranking_inspect_live_facts(&store, &target, 0, Some(&missing_partner), true, true),
            Err(RankingInspectError::FactInvalid { fact: "loverName", .. })));
        store.shared_guilds.insert(GUILD.into(), guild(GUILD, "owner", 7, "CurrentLiveGuild"));
        let rank = store.shared_guilds[GUILD].ranks[0].clone();
        store.shared_guilds.get_mut(GUILD).unwrap().ranks.push(rank);
        let target = resolve_ranking_inspect_target(&store, 7).unwrap();
        assert!(matches!(ranking_inspect_live_facts(&store, &target, 0, None, true, true),
            Err(RankingInspectError::FactInvalid { fact: "guild", .. })));
    }
}
