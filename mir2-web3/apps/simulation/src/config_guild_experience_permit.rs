//! Ephemeral authorization for one already-verified shared Zone reward.
//! The pending Zone/outbox envelope is durable; this capability is recreated by
//! its trusted consumer and is never deserialized from a character checkpoint.
use super::*;
use crate::runtime::zone::ZoneExperienceSelection;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub(crate) struct GuildExperienceCommitPermit {
    pub(crate) identity: Stage5FriendIdentity,
    pub(crate) kill_key: String,
    pub(crate) kill_payload_hash: String,
    pub(crate) selection: Option<ZoneExperienceSelection>,
    pub(crate) selection_hash: Option<String>,
}

impl GuildExperienceCommitPermit {
    /// Only the server-owned shared reward entry point calls this, after
    /// verifying the Zone/outbox owner against the live authenticated character.
    /// It does not turn an imported `pending` journal into an authorized award.
    pub(crate) fn from_verified_shared_kill(
        identity: Stage5FriendIdentity,
        kill_key: String,
        kill_payload_hash: String,
        selection: Option<ZoneExperienceSelection>,
    ) -> Result<Self, String> {
        if identity.account_id.is_empty()
            || kill_key.is_empty()
            || kill_key.len() > 4096
            || kill_payload_hash.len() != 64
            || !kill_payload_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("invalid shared kill authorization identity or receipt".into());
        }
        if let Some(selection) = &selection {
            if selection.account_id != identity.account_id
                || selection.character_index != identity.character_index
                || selection.source_zone.is_empty()
                || selection
                    .guild
                    .as_ref()
                    .is_some_and(|guild| guild.guild_id.is_empty())
            {
                return Err(
                    "shared kill selection does not belong to the authenticated recipient".into(),
                );
            }
        }
        let selection_hash = selection
            .as_ref()
            .map(|selection| {
                serde_json::to_vec(selection)
                    .map(|bytes| {
                        Sha256::digest(bytes)
                            .iter()
                            .map(|byte| format!("{byte:02x}"))
                            .collect::<String>()
                    })
                    .map_err(|error| error.to_string())
            })
            .transpose()?;
        Ok(Self {
            identity,
            kill_key,
            kill_payload_hash,
            selection,
            selection_hash,
        })
    }
    pub(super) fn authorizes(&self, event: &GuildExperienceEvent, selection_hash: &str) -> bool {
        self.identity == event.identity
            && self.selection_hash.as_deref() == Some(selection_hash)
            && self.selection.as_ref().is_some_and(|selection| {
                selection.final_amount == event.final_amount
                    && selection.guild.as_ref().is_some_and(|guild| {
                        guild.guild_id == event.guild_id
                            && guild.membership_epoch == event.membership_epoch
                    })
            })
    }
}
