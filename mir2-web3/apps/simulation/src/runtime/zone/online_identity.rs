//! Server-owned counterpart of Crystal's spawned global MapObject.Node.
//! Membership, combat life, transport credentials and this identity are distinct.
use super::types::{SessionId, ZoneJoin, ZoneKey, ZonePlayer};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OnlineOwner {
    pub namespace: String,
    pub epoch: u64,
    pub session_id: SessionId,
    pub account_id: String,
    pub character_index: i32,
    pub object_id: u32,
}
impl OnlineOwner {
    pub fn matches_join(&self, join: &ZoneJoin) -> bool {
        self.session_id == join.session_id
            && self.account_id == join.account_id
            && self.character_index == join.character_index
            && self.object_id == join.object_id
    }
    pub fn matches_player(&self, player: &ZonePlayer) -> bool {
        self.session_id == player.session_id
            && self.account_id == player.account_id
            && self.character_index == player.character_index
            && self.object_id == player.object_id
    }
    pub fn encoded(&self) -> String {
        serde_json::to_string(self).expect("online identity encodes")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OnlinePresence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owned_pet_clock: Option<super::runtime::owned_pet_combat::OwnedPetPlayerClock>,
    #[serde(default, skip_serializing_if = "online_presence_life_is_zero")]
    pub life_generation: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_profile: Option<super::ZoneChatProfile>,
    #[serde(default, skip_serializing_if = "online_presence_life_is_zero")]
    pub brown_until_ms: u64,
    #[serde(default = "online_presence_default_position", skip_serializing_if = "online_presence_position_is_zero")]
    pub position: mir2_protocol::Point,
    #[serde(default, skip_serializing_if = "online_presence_level_is_zero")]
    pub level: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experience_profile: Option<super::ZoneExperienceProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mentor_bank: Option<super::ZoneMentorBankAttribution>,
    pub owner: OnlineOwner,
    pub key: ZoneKey,
    pub dead: bool,
    pub name: String,
    pub group_members: Vec<String>,
}
pub(crate) type OnlinePresenceSnapshot = BTreeMap<SessionId, OnlinePresence>;

fn online_presence_default_position() -> mir2_protocol::Point { mir2_protocol::Point { x: 0, y: 0 } }
fn online_presence_position_is_zero(value: &mir2_protocol::Point) -> bool { value.x == 0 && value.y == 0 }
fn online_presence_level_is_zero(value: &u16) -> bool { *value == 0 }
fn online_presence_life_is_zero(value: &u64) -> bool { *value == 0 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OnlineIdentityBook {
    namespace: String,
    next_epoch: u64,
    owners: BTreeMap<SessionId, OnlineOwner>,
}
impl Default for OnlineIdentityBook {
    fn default() -> Self {
        let mut bytes = [0_u8; 16];
        OsRng.fill_bytes(&mut bytes);
        Self {
            namespace: bytes.iter().map(|b| format!("{b:02x}")).collect(),
            next_epoch: 1,
            owners: BTreeMap::new(),
        }
    }
}
impl OnlineIdentityBook {
    pub fn admit(&mut self, join: &ZoneJoin, retain: bool) -> Option<OnlineOwner> {
        if retain {
            if let Some(owner) = self
                .owners
                .get(&join.session_id)
                .filter(|o| o.matches_join(join))
            {
                return Some(owner.clone());
            }
        }
        let epoch = self.next_epoch;
        self.next_epoch = epoch.checked_add(1)?;
        let owner = OnlineOwner {
            namespace: self.namespace.clone(),
            epoch,
            session_id: join.session_id.clone(),
            account_id: join.account_id.clone(),
            character_index: join.character_index,
            object_id: join.object_id,
        };
        self.owners.insert(join.session_id.clone(), owner.clone());
        Some(owner)
    }
    pub fn revoke(&mut self, session_id: &SessionId) {
        self.owners.remove(session_id);
    }
    pub fn owner(&self, session_id: &SessionId) -> Option<&OnlineOwner> {
        self.owners.get(session_id)
    }
    pub(super) fn owners(&self) -> impl Iterator<Item = (&SessionId, &OnlineOwner)> {
        self.owners.iter()
    }
    pub fn validates_encoded(&self, encoded: &str) -> bool {
        serde_json::from_str::<OnlineOwner>(encoded)
            .ok()
            .is_some_and(|owner| self.owner(&owner.session_id) == Some(&owner))
    }
    pub fn validates_for_session(&self, encoded: &str, session_id: &SessionId) -> bool {
        serde_json::from_str::<OnlineOwner>(encoded)
            .ok()
            .is_some_and(|owner| {
                &owner.session_id == session_id && self.owner(session_id) == Some(&owner)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn online_epoch_exhaustion_fails_closed_without_recycling() {
        let mut book = OnlineIdentityBook::default();
        book.next_epoch = u64::MAX;
        let join = ZoneJoin {
            session_id: SessionId::new("exhausted"),
            account_id: "account".into(),
            character_index: 0,
            object_id: 1,
            name: "character".into(),
            class: mir2_protocol::MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            level: 1,
            hp: 10,
            max_hp: 10,
            mp: 0,
            map_file_name: "0".into(),
            position: mir2_protocol::Point { x: 1, y: 1 },
            direction: mir2_protocol::MirDirection::Up,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        };
        assert!(book.admit(&join, false).is_none());
        assert!(book.owners.is_empty());
        assert_eq!(book.next_epoch, u64::MAX);
    }
}
