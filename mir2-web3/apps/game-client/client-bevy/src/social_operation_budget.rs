//! Local transport custody around the existing Native social reducer.
//! This owns no gameplay authority, wire ID, retry or generic timeout.
use crate::social::{SocialModel, SocialPendingOperation, MAX_SOCIAL_PENDING};
use crate::social_trade_projection::observe_own_offer;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SocialBudgetOwner {
    pub generation: u64,
    pub connection_generation: u64,
    pub session_generation: u64,
    pub player_object_id: u32,
    pub scene_revision: u64,
    pub map_file_name: String,
}
impl SocialBudgetOwner {
    pub fn valid(&self) -> bool {
        self.generation > 0
            && self.generation <= SAFE_INTEGER
            && self.connection_generation > 0
            && self.connection_generation <= SAFE_INTEGER
            && self.session_generation > 0
            && self.session_generation <= SAFE_INTEGER
            && self.player_object_id > 0
            && self.scene_revision <= SAFE_INTEGER
            && !self.map_file_name.is_empty()
            && self.map_file_name.len() <= 512
            && !self.map_file_name.chars().any(char::is_control)
    }
    fn physical(&self, other: &Self) -> bool {
        self.generation == other.generation
            && self.connection_generation == other.connection_generation
            && self.session_generation == other.session_generation
            && self.player_object_id == other.player_object_id
    }
    fn replaces(&self, old: &Self) -> bool {
        self.generation > old.generation
            || self.generation == old.generation
                && self.connection_generation == old.connection_generation
                && self.session_generation > old.session_generation
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SocialBudgetInvite {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Command {
    SwitchGroup {
        allow_group: bool,
    },
    AddMember {
        name: String,
    },
    DelMember {
        name: String,
    },
    GroupInvite {
        accept_invite: bool,
    },
    RequestGuildInfo {
        info_type: u8,
    },
    EditGuildMember {
        change_type: u8,
        rank_index: u8,
        name: String,
        rank_name: String,
    },
    EditGuildNotice {
        notice: Vec<String>,
    },
    GuildInvite {
        accept_invite: bool,
    },
    GuildStorageGoldChange {
        change_type: u8,
        amount: u32,
    },
    GuildStorageItemChange {
        change_type: u8,
        from: i32,
        to: i32,
    },
    TradeRequest,
    TradeReply {
        accept_invite: bool,
    },
    TradeGold {
        amount: u32,
    },
    DepositTradeItem {
        from: i32,
        to: i32,
    },
    RetrieveTradeItem {
        from: i32,
        to: i32,
    },
    TradeConfirm {
        locked: bool,
    },
    TradeCancel,
}

pub fn operation_kind(operation: &SocialPendingOperation) -> &'static str {
    use SocialPendingOperation::*;
    match operation {
        GroupSwitch { .. } => "groupSwitch",
        GroupAdd { .. } => "groupAdd",
        GroupRemove { .. } => "groupRemove",
        GroupInviteAccept { .. } => "groupInviteAccept",
        GuildInfo => "guildInfo",
        GuildRosterRead(_) => "guildRosterRead",
        GuildMember { .. } => "guildMember",
        GuildRankRename(_) => "guildRankRename",
        GuildRankPermission(_) => "guildRankPermission",
        GuildNotice { .. } => "guildNotice",
        GuildInviteAccept { .. } => "guildInviteAccept",
        GuildStorageGold { .. } => "guildStorageGold",
        GuildStorageItem { .. } => "guildStorageItem",
        TradeRequest => "tradeRequest",
        TradeReply => "tradeReply",
        TradeGold { .. } => "tradeGold",
        TradeDeposit { .. } => "tradeDeposit",
        TradeRetrieve { .. } => "tradeRetrieve",
        TradeConfirm { .. } => "tradeConfirm",
        TradeCancel => "tradeCancel",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialBudgetPending {
    pub token: u64,
    pub kind: &'static str,
    pub entered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Scope {
    Other,
    Guild(Option<String>, u64),
    Trade(Option<String>, String, u64, u64),
}
impl Scope {
    fn capture(model: &SocialModel, operation: &SocialPendingOperation) -> Self {
        if operation.is_guild() {
            Self::Guild(model.guild.name.clone(), model.guild.scope_epoch)
        } else if operation.is_trade() {
            Self::Trade(
                model.trade.partner.clone(),
                model.trade.state.clone(),
                model.trade.open_revision,
                model.trade.invite_revision,
            )
        } else {
            Self::Other
        }
    }
}

struct Row {
    token: u64,
    publication_revision: u64,
    owner: SocialBudgetOwner,
    command: Value,
    operation: SocialPendingOperation,
    scope: Scope,
    entered: bool,
}

#[derive(Default)]
pub struct SocialOperationBudget {
    owner: Option<SocialBudgetOwner>,
    retired: bool,
    next_token: u64,
    publication_revision: u64,
    clock_ms: Option<u64>,
    model: SocialModel,
    rows: Vec<Row>,
}
impl SocialOperationBudget {
    pub fn model(&self) -> &SocialModel {
        &self.model
    }
    pub fn operation_for_token(&self, token: u64) -> Option<&SocialPendingOperation> {
        self.rows
            .iter()
            .find(|row| row.token == token)
            .map(|row| &row.operation)
    }
    pub fn pending(&self) -> Vec<SocialBudgetPending> {
        self.rows
            .iter()
            .map(|row| SocialBudgetPending {
                token: row.token,
                kind: operation_kind(&row.operation),
                entered: row.entered,
            })
            .collect()
    }
    pub fn current(&self, owner: &SocialBudgetOwner) -> bool {
        owner.valid() && !self.retired && self.owner.as_ref() == Some(owner)
    }
    pub fn reset(&mut self, owner: SocialBudgetOwner) -> Result<(), &'static str> {
        if !owner.valid() {
            return Err("invalidOwner");
        }
        if let Some(old) = &self.owner {
            if old.physical(&owner) {
                if self.retired
                    || owner.scene_revision < old.scene_revision
                    || owner.scene_revision == old.scene_revision
                        && owner.map_file_name != old.map_file_name
                {
                    return Err("retiredOwner");
                }
                if old != &owner {
                    self.model.clear_scene();
                }
            } else {
                if !owner.replaces(old) {
                    return Err("retiredOwner");
                }
                self.model.clear_session();
                self.rows.clear();
                self.clock_ms = None;
            }
        }
        self.owner = Some(owner);
        self.retired = false;
        Ok(())
    }
    pub fn retire(&mut self, owner: &SocialBudgetOwner) -> Result<(), &'static str> {
        if !owner.valid() || self.owner.as_ref().is_none_or(|old| !old.physical(owner)) {
            return Err("retiredOwner");
        }
        self.model.clear_session();
        self.rows.clear();
        self.retired = true;
        Ok(())
    }
    fn require(&self, owner: &SocialBudgetOwner) -> Result<(), &'static str> {
        self.current(owner).then_some(()).ok_or("retiredOwner")
    }
    fn rebuild_pending(&mut self) {
        self.model.pending = self.rows.iter().map(|row| row.operation.clone()).collect();
        debug_assert!(self.model.pending.len() <= MAX_SOCIAL_PENDING);
    }
    fn operation(
        &self,
        wire: &Value,
        invite: Option<&SocialBudgetInvite>,
        deadline: Option<u64>,
    ) -> Result<Option<SocialPendingOperation>, &'static str> {
        let Some(kind) = wire.get("type").and_then(Value::as_str) else {
            return Err("invalidCommand");
        };
        if ![
            "switchGroup",
            "addMember",
            "delMember",
            "groupInvite",
            "requestGuildInfo",
            "editGuildMember",
            "editGuildNotice",
            "guildInvite",
            "guildStorageGoldChange",
            "guildStorageItemChange",
            "tradeRequest",
            "tradeReply",
            "tradeGold",
            "depositTradeItem",
            "retrieveTradeItem",
            "tradeConfirm",
            "tradeCancel",
        ]
        .contains(&kind)
        {
            return Ok(None);
        }
        let command: Command =
            serde_json::from_value(wire.clone()).map_err(|_| "invalidCommand")?;
        if serde_json::to_value(&command).ok().as_ref() != Some(wire) {
            return Err("invalidCommand");
        }
        let name_valid =
            |name: &str| !name.trim().is_empty() && name.len() <= 256 && !name.contains('\0');
        let invite_current = |name: &Option<String>, epoch: u64| {
            let name = name.as_ref().ok_or("missingInvite")?;
            if invite.is_some_and(|given| given.name != *name) {
                return Err("staleInvite");
            }
            Ok((name.clone(), epoch))
        };
        use SocialPendingOperation as Op;
        Ok(Some(match command {
            Command::SwitchGroup { allow_group } => Op::GroupSwitch { allow_group },
            Command::AddMember { name } if name_valid(&name) => Op::GroupAdd { name },
            Command::DelMember { name } if name_valid(&name) => Op::GroupRemove { name },
            Command::GroupInvite {
                accept_invite: true,
            } => {
                let (inviter, invite_epoch) = invite_current(
                    &self.model.group.pending_invite_from,
                    self.model.group.pending_invite_epoch,
                )?;
                Op::GroupInviteAccept {
                    inviter,
                    invite_epoch,
                }
            }
            Command::GuildInvite {
                accept_invite: true,
            } => {
                let (inviter, invite_epoch) = invite_current(
                    &self.model.guild.pending_invite_from,
                    self.model.guild.pending_invite_epoch,
                )?;
                Op::GuildInviteAccept {
                    inviter,
                    invite_epoch,
                }
            }
            Command::GroupInvite {
                accept_invite: false,
            }
            | Command::GuildInvite {
                accept_invite: false,
            } => return Ok(None),
            Command::RequestGuildInfo { info_type } => Op::GuildRosterRead(
                self.model
                    .guild_roster_read_request(info_type)
                    .ok_or("invalidContext")?,
            ),
            Command::EditGuildMember {
                change_type: 3,
                rank_index,
                name,
                rank_name,
            } if name.is_empty() => {
                let deadline = deadline
                    .filter(|ms| *ms <= SAFE_INTEGER)
                    .ok_or("invalidDeadline")?;
                let viewer = self.model.viewer.as_ref().ok_or("invalidContext")?;
                Op::GuildRankRename(
                    self.model
                        .guild_rank_rename_request(&viewer.name, rank_index, &rank_name, deadline)
                        .ok_or("invalidContext")?,
                )
            }
            Command::EditGuildMember {
                change_type: 5,
                rank_index,
                name,
                rank_name,
            } => {
                let enabled = match name.as_str() {
                    "true" => true,
                    "false" => false,
                    _ => return Err("invalidCommand"),
                };
                let option = rank_name
                    .parse::<u8>()
                    .ok()
                    .filter(|bit| *bit < 8 && rank_name == bit.to_string())
                    .ok_or("invalidCommand")?;
                let viewer = self.model.viewer.as_ref().ok_or("invalidContext")?;
                Op::GuildRankPermission(
                    self.model
                        .guild_rank_permission_request(&viewer.name, rank_index, option, enabled)
                        .ok_or("invalidContext")?,
                )
            }
            Command::EditGuildMember {
                change_type,
                rank_index,
                name,
                ..
            } if change_type <= 4 && change_type != 3 => Op::GuildMember {
                change_type,
                rank_index,
                name,
            },
            Command::EditGuildNotice { notice }
                if notice.len() <= crate::social::MAX_NOTICE_LINES
                    && notice
                        .iter()
                        .all(|line| line.len() <= 4096 && !line.contains('\0')) =>
            {
                Op::GuildNotice { notice }
            }
            Command::GuildStorageGoldChange {
                change_type,
                amount,
            } if change_type <= 1 => Op::GuildStorageGold {
                change_type,
                amount,
            },
            Command::GuildStorageItemChange { change_type: 3, .. } => return Ok(None),
            Command::GuildStorageItemChange {
                change_type,
                from,
                to,
            } if change_type <= 2 => Op::GuildStorageItem {
                change_type,
                from,
                to,
            },
            Command::TradeRequest => Op::TradeRequest,
            Command::TradeReply { .. } => Op::TradeReply,
            Command::TradeGold { amount } => Op::TradeGold { amount },
            Command::DepositTradeItem { from, to } => Op::TradeDeposit { from, to },
            Command::RetrieveTradeItem { from, to } => Op::TradeRetrieve { from, to },
            Command::TradeConfirm { locked } => Op::TradeConfirm { locked },
            Command::TradeCancel => Op::TradeCancel,
            _ => return Err("invalidCommand"),
        }))
    }
    fn reserve_operation(
        &mut self,
        owner: &SocialBudgetOwner,
        operation: SocialPendingOperation,
        wire: Value,
    ) -> Result<SocialBudgetPending, &'static str> {
        self.require(owner)?;
        if self.next_token >= SAFE_INTEGER {
            return Err("tokenExhausted");
        }
        if !self.model.begin_pending(operation.clone()) {
            return Err("budgetUnavailable");
        }
        self.next_token += 1;
        let result = SocialBudgetPending {
            token: self.next_token,
            kind: operation_kind(&operation),
            entered: false,
        };
        self.rows.push(Row {
            token: result.token,
            publication_revision: self.publication_revision,
            owner: owner.clone(),
            command: wire,
            scope: Scope::capture(&self.model, &operation),
            operation,
            entered: false,
        });
        Ok(result)
    }
    pub fn reserve(
        &mut self,
        owner: &SocialBudgetOwner,
        wire: Value,
        invite: Option<&SocialBudgetInvite>,
        deadline_ms: Option<u64>,
    ) -> Result<Option<SocialBudgetPending>, &'static str> {
        self.require(owner)?;
        let Some(operation) = self.operation(&wire, invite, deadline_ms)? else {
            return Ok(None);
        };
        self.reserve_operation(owner, operation, wire).map(Some)
    }
    pub fn validate(
        &self,
        owner: &SocialBudgetOwner,
        token: u64,
        wire: &Value,
    ) -> Result<(), &'static str> {
        self.require(owner)?;
        let row = self
            .rows
            .iter()
            .find(|row| row.token == token)
            .ok_or("unknownToken")?;
        if row.entered
            || row.publication_revision != self.publication_revision
            || row.owner != *owner
            || row.command != *wire
            || row.scope != Scope::capture(&self.model, &row.operation)
        {
            return Err("staleReservation");
        }
        let current = match &row.operation {
            SocialPendingOperation::GuildRosterRead(request) => {
                self.model.guild_roster_read_current(request)
            }
            SocialPendingOperation::GuildRankPermission(request) => {
                self.model.guild_rank_permission_current(request)
            }
            SocialPendingOperation::GuildRankRename(request) => {
                self.model.guild.rank_rename_scope_matches(request)
            }
            SocialPendingOperation::GroupInviteAccept {
                inviter,
                invite_epoch,
            } => {
                self.model.group.pending_invite_from.as_ref() == Some(inviter)
                    && self.model.group.pending_invite_epoch == *invite_epoch
            }
            SocialPendingOperation::GuildInviteAccept {
                inviter,
                invite_epoch,
            } => {
                self.model.guild.pending_invite_from.as_ref() == Some(inviter)
                    && self.model.guild.pending_invite_epoch == *invite_epoch
            }
            _ => true,
        };
        if !current {
            return Err("staleReservation");
        }
        Ok(())
    }
    pub fn enter(
        &mut self,
        owner: &SocialBudgetOwner,
        token: u64,
        wire: &Value,
    ) -> Result<(), &'static str> {
        self.validate(owner, token, wire)?;
        let index = self.rows.iter().position(|row| row.token == token).unwrap();
        if matches!(self.rows[index].operation, SocialPendingOperation::GroupAdd { .. }) {
            // Crystal may silently drop an invite during its cooldown. The
            // token authorizes one send; group membership still comes from packets.
            self.rows.remove(index);
            self.rebuild_pending();
        } else {
            self.rows[index].entered = true;
        }
        Ok(())
    }
    pub fn cancel_unsent(
        &mut self,
        owner: &SocialBudgetOwner,
        token: u64,
    ) -> Result<(), &'static str> {
        self.require(owner)?;
        let index = self
            .rows
            .iter()
            .position(|row| row.token == token)
            .ok_or("unknownToken")?;
        if self.rows[index].entered {
            return Err("alreadyEntered");
        }
        self.rows.remove(index);
        self.rebuild_pending();
        Ok(())
    }
    fn publish(&mut self, incoming: SocialModel) -> Result<(), &'static str> {
        let revision = self
            .publication_revision
            .checked_add(1)
            .ok_or("revisionExhausted")?;
        let changed_viewer = self
            .model
            .viewer
            .as_ref()
            .zip(incoming.viewer.as_ref())
            .is_some_and(|(a, b)| a != b);
        // Native completion sees entered work only. Reserved rows still count toward
        // admission and survive early/reentrant replies until cancelled as unsent.
        self.model.pending = self
            .rows
            .iter()
            .filter(|row| row.entered)
            .map(|row| row.operation.clone())
            .collect();
        self.model.apply_authoritative(incoming);
        if changed_viewer {
            self.rows.clear();
        } else {
            self.rows
                .retain(|row| !row.entered || self.model.pending.contains(&row.operation));
        }
        self.rebuild_pending();
        self.publication_revision = revision;
        Ok(())
    }
    pub fn packet(
        &mut self,
        owner: &SocialBudgetOwner,
        packet: &str,
        payload: &Value,
    ) -> Result<bool, &'static str> {
        self.require(owner)?;
        let mut incoming = self.model.clone();
        if !incoming.apply_network_packet(packet, payload) {
            return Ok(false);
        }
        self.publish(incoming)?;
        Ok(true)
    }
    pub fn snapshot(
        &mut self,
        owner: &SocialBudgetOwner,
        snapshot: &Value,
        inventory_json: &Value,
    ) -> Result<bool, &'static str> {
        self.require(owner)?;
        let mut incoming = self.model.clone();
        if !observe_own_offer(snapshot, inventory_json, &mut incoming) {
            return Ok(false);
        }
        self.publish(incoming)?;
        Ok(true)
    }
    pub fn expire(&mut self, owner: &SocialBudgetOwner, now_ms: u64) -> Result<(), &'static str> {
        self.require(owner)?;
        if now_ms > SAFE_INTEGER || self.clock_ms.is_some_and(|old| now_ms < old) {
            return Err("invalidClock");
        }
        self.clock_ms = Some(now_ms);
        self.model.expire_guild_rank_renames(now_ms);
        self.rows
            .retain(|row| self.model.pending.contains(&row.operation));
        self.rebuild_pending();
        Ok(())
    }
}

#[cfg(test)]
#[path = "social_operation_budget_tests.rs"]
mod tests;
