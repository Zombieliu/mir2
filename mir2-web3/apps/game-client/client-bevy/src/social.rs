//! Bounded, server-owned Group/Guild/Trade read models for native clients.
//!
//! This module deliberately contains no Stage5 commands and no local success
//! assumptions.  It accepts only the ordinary Crystal packet names emitted by
//! the gateway.  A UI operation may be marked pending, but it is cleared only
//! by a matching authoritative packet/state transition or by a session reset.

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::inventory::CrystalItemTooltipSourceModel;

pub const MAX_GROUP_MEMBERS: usize = 15;
pub const MAX_GUILD_MEMBERS: usize = 200;
pub const MAX_GUILD_RANKS: usize = 32;
pub const MAX_GUILD_STORAGE_ITEMS: usize = 112;
pub const MAX_NOTICE_LINES: usize = 8;
pub const MAX_TRADE_ITEMS: usize = 10;
pub const MAX_SOCIAL_PENDING: usize = 8;
pub const MAX_GUILD_PERMISSIONS: usize = 8;

/// Crystal's `GuildRankOptions` bits, in wire order.  Keep these strings in
/// the same canonical form consumed by the native guild UI and do not infer
/// permissions from rank names or local UI state.
pub const GUILD_PERMISSION_KEYS: [&str; MAX_GUILD_PERMISSIONS] = [
    "CanChangeRank",
    "CanRecruit",
    "CanKick",
    "CanStoreItem",
    "CanRetrieveItem",
    "CanAlterAlliance",
    "CanChangeNotice",
    "CanActivateBuff",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GroupMemberModel {
    pub name: String,
    pub leader: bool,
    pub online: bool,
    pub level: Option<u16>,
    pub class: Option<u8>,
    pub hp: Option<i32>,
    pub max_hp: Option<i32>,
    pub map: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GroupModel {
    pub active: bool,
    pub allow_invites: bool,
    pub leader_name: Option<String>,
    pub members: Vec<GroupMemberModel>,
    pub pending_invite_from: Option<String>,
    /// Monotonic identity for distinct authoritative invitations. Repeated
    /// delivery of the same inviter does not advance this value.
    pub pending_invite_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GuildMemberModel {
    pub name: String,
    pub id: i32,
    pub online: bool,
    pub rank_name: Option<String>,
    pub rank_index: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GuildRankModel {
    pub name: String,
    pub index: i32,
    pub options: u8,
    pub members: Vec<GuildMemberModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GuildStorageItemModel {
    pub unique_id: u64,
    pub item_index: i32,
    pub count: u16,
    pub user_id: i64,
    /// Exact `GuildStorageItem.Item` metadata used by Crystal's shared
    /// `MirItemCell.OnMouseEnter` tooltip path.
    pub tooltip_source: Option<CrystalItemTooltipSourceModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GuildModel {
    pub name: Option<String>,
    pub notice: Vec<String>,
    pub rank_name: Option<String>,
    pub level: u8,
    pub experience: i64,
    pub max_experience: i64,
    pub gold: u32,
    pub member_count: u16,
    pub max_members: u16,
    pub my_options: u8,
    pub my_rank_id: i32,
    pub permissions: Vec<String>,
    pub ranks: Vec<GuildRankModel>,
    pub members: Vec<GuildMemberModel>,
    /// Authoritative 112-slot guild storage snapshot. `None` is an empty
    /// server slot; this is intentionally separate from personal storage.
    pub storage_items: Vec<Option<GuildStorageItemModel>>,
    pub pending_invite_from: Option<String>,
    /// Monotonic identity for distinct authoritative invitations. Repeated
    /// delivery of the same inviter does not advance this value.
    pub pending_invite_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TradeItemModel {
    pub unique_id: Option<u64>,
    pub item_index: Option<i32>,
    pub name: Option<String>,
    pub count: u16,
    /// Exact offered `UserItem` metadata used by both Crystal trade grids.
    pub tooltip_source: Option<CrystalItemTooltipSourceModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TradeModel {
    /// Cursor revision for a validated trade packet, not a transaction receipt.
    /// UI-local locks must react even to repeated equal partner offers.
    pub event_revision: u64,
    /// Accepted-exchange epoch in the local packet cursor. Distinguishes two
    /// exchanges with the same partner even when their packets share a frame.
    pub open_revision: u64,
    /// Last explicit source packet that unlocked this exchange. Kept apart
    /// from last_event so a later Group/Guild packet cannot hide the unlock.
    pub unlock_revision: u64,
    pub state: String,
    pub partner: Option<String>,
    pub partner_gold: u32,
    /// Packet array positions are the original 2*x+y cell IDs. Never compact
    /// null entries or renumber an offered item into an earlier empty cell.
    pub partner_items: Vec<Option<TradeItemModel>>,
    pub partner_confirmed: bool,
    /// Exact Candidate snapshot identity. Never use an item name or a partner
    /// update as evidence for an offer belonging to this local participant.
    pub my_offer_nonce: Option<String>,
    /// The native client does not invent its own offer. These are filled only
    /// by a validated authoritative local-side snapshot, not partner packets.
    pub my_gold: u32,
    pub my_items: Vec<Option<TradeItemModel>>,
    pub my_confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialAuthoritativeEvent {
    pub packet: String,
    pub success: Option<bool>,
    #[serde(default)]
    pub subject: Option<String>,
    /// Trade item replies echo the requested inventory slots. Keep both
    /// coordinates in the event so pending operations cannot be released by
    /// a different deposit/retrieve operation.
    #[serde(default)]
    pub from: Option<i32>,
    #[serde(default)]
    pub to: Option<i32>,
    #[serde(default)]
    pub change_type: Option<u8>,
    #[serde(default)]
    pub rank_index: Option<u8>,
    #[serde(default)]
    pub amount: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum SocialPendingOperation {
    GroupSwitch {
        allow_group: bool,
    },
    GroupAdd {
        name: String,
    },
    GroupRemove {
        name: String,
    },
    GroupInviteAccept {
        inviter: String,
        invite_epoch: u64,
    },
    GuildInfo,
    GuildMember {
        change_type: u8,
        rank_index: u8,
        name: String,
    },
    GuildNotice {
        notice: Vec<String>,
    },
    GuildInviteAccept {
        inviter: String,
        invite_epoch: u64,
    },
    GuildStorageGold {
        change_type: u8,
        amount: u32,
    },
    GuildStorageItem {
        change_type: u8,
        from: i32,
        to: i32,
    },
    TradeRequest,
    TradeReply,
    TradeGold {
        amount: u32,
    },
    TradeDeposit {
        from: i32,
        to: i32,
    },
    TradeRetrieve {
        from: i32,
        to: i32,
    },
    TradeConfirm {
        locked: bool,
    },
    TradeCancel,
}

impl SocialPendingOperation {
    pub fn is_trade(&self) -> bool {
        matches!(
            self,
            Self::TradeRequest
                | Self::TradeReply
                | Self::TradeGold { .. }
                | Self::TradeDeposit { .. }
                | Self::TradeRetrieve { .. }
                | Self::TradeConfirm { .. }
                | Self::TradeCancel
        )
    }
}

#[derive(Debug, Clone, Resource, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SocialModel {
    pub group: GroupModel,
    pub guild: GuildModel,
    pub trade: TradeModel,
    #[serde(default)]
    pub pending: Vec<SocialPendingOperation>,
    #[serde(default)]
    pub last_event: Option<SocialAuthoritativeEvent>,
}

impl SocialModel {
    pub fn begin_pending(&mut self, operation: SocialPendingOperation) -> bool {
        if self.pending.iter().any(|item| item == &operation) {
            return false;
        }
        if self.pending.len() >= MAX_SOCIAL_PENDING {
            return false;
        }
        self.pending.push(operation);
        true
    }

    pub fn clear_session(&mut self) {
        *self = Self::default();
    }

    pub fn clear_scene(&mut self) {
        // Map/scene changes do not change account-scoped group or guild
        // membership, and the gateway does not provide a fresh social
        // snapshot on every map transition. Preserve all read models and
        // pending operations; only the packet cursor event is scene-local.
        self.last_event = None;
    }

    /// Replace the authoritative portion while retaining only pending work
    /// that has not been proven complete by the incoming packet/state.
    pub fn apply_authoritative(&mut self, mut incoming: SocialModel) {
        let old = self.clone();
        let pending = std::mem::take(&mut self.pending);
        incoming.normalize();
        self.group = incoming.group;
        self.guild = incoming.guild;
        self.trade = incoming.trade;
        self.last_event = incoming.last_event;
        self.pending = pending;
        self.reconcile_pending(&old);
    }

    /// Fold one ordinary gateway ServerPacket payload into the cursor model.
    /// Returns false for unknown, missing, or over-limit payloads.
    pub fn apply_packet(&mut self, packet: &str, payload: &Value) -> bool {
        let next_trade_revision = self.trade.event_revision.wrapping_add(1);
        let changed;
        let mut success = None;
        let mut from = None;
        let mut to = None;
        match packet {
            "SwitchGroup" => {
                let Some(allow) = payload.get("allowGroup").and_then(Value::as_bool) else {
                    return false;
                };
                self.group.allow_invites = allow;
                changed = true;
            }
            "DeleteGroup" => {
                self.group = GroupModel::default();
                changed = true;
            }
            "DeleteMember" => {
                let Some(name) = clean_name(payload.get("name")) else {
                    return false;
                };
                self.group.members.retain(|member| member.name != name);
                changed = true;
            }
            "GroupInvite" => {
                let Some(name) = clean_name(payload.get("name")) else {
                    return false;
                };
                if self.group.pending_invite_from.as_deref() != Some(name.as_str()) {
                    self.group.pending_invite_epoch =
                        self.group.pending_invite_epoch.saturating_add(1).max(1);
                }
                self.group.pending_invite_from = Some(name);
                changed = true;
            }
            "GroupInviteResult" => {
                let Some(result) = payload
                    .get("success")
                    .or_else(|| payload.get("accepted"))
                    .and_then(Value::as_bool)
                else {
                    return false;
                };
                if clean_name(payload.get("name")).is_none() {
                    return false;
                }
                success = Some(result);
                changed = true;
            }
            "AddMember" => {
                let Some(name) = clean_name(payload.get("name")) else {
                    return false;
                };
                let was_active = self.group.active;
                upsert_group_member(&mut self.group, name);
                if !was_active && self.group.active {
                    self.group.pending_invite_from = None;
                }
                changed = true;
            }
            "GroupMemberInfo" => {
                let Some(members) = payload.get("members").and_then(Value::as_array) else {
                    return false;
                };
                if members.len() > MAX_GROUP_MEMBERS {
                    return false;
                }
                let parsed = members
                    .iter()
                    .filter_map(parse_group_member)
                    .collect::<Vec<_>>();
                if parsed.len() != members.len() {
                    return false;
                }
                let was_active = self.group.active;
                self.group.members = parsed;
                self.group.leader_name = clean_name(payload.get("leaderName"));
                self.group.active = !self.group.members.is_empty();
                if !was_active && self.group.active {
                    self.group.pending_invite_from = None;
                }
                changed = true;
            }
            "GuildStatus" => {
                let was_in_guild = self.guild.name.is_some();
                let Some(raw_name) =
                    guild_field(payload, "guild_name", "guildName").and_then(Value::as_str)
                else {
                    return false;
                };
                let Some(raw_rank) = guild_field(payload, "guild_rank_name", "guildRankName")
                    .and_then(Value::as_str)
                else {
                    return false;
                };
                if raw_name.trim().is_empty() || raw_rank.trim().is_empty() {
                    self.guild = GuildModel::default();
                    success = Some(true);
                    self.last_event = Some(SocialAuthoritativeEvent {
                        packet: packet.to_owned(),
                        success,
                        subject: None,
                        from: None,
                        to: None,
                        change_type: None,
                        rank_index: None,
                        amount: None,
                    });
                    self.normalize();
                    return true;
                }
                let Some(name) = clean_name(Some(&Value::String(raw_name.to_owned()))) else {
                    return false;
                };
                self.guild.name = Some(name);
                self.guild.rank_name = clean_name(Some(&Value::String(raw_rank.to_owned())));
                self.guild.level = value_u8(guild_field(payload, "level", "level")).unwrap_or(0);
                self.guild.experience = value_i64(guild_field(payload, "experience", "experience"))
                    .unwrap_or(0)
                    .max(0);
                self.guild.max_experience =
                    value_i64(guild_field(payload, "max_experience", "maxExperience"))
                        .unwrap_or(0)
                        .max(0);
                self.guild.gold = value_u32(guild_field(payload, "gold", "gold")).unwrap_or(0);
                self.guild.member_count =
                    value_u16(guild_field(payload, "member_count", "memberCount")).unwrap_or(0);
                self.guild.max_members =
                    value_u16(guild_field(payload, "max_members", "maxMembers")).unwrap_or(0);
                self.guild.my_options =
                    value_u8(guild_field(payload, "my_options", "myOptions")).unwrap_or(0);
                self.guild.my_rank_id =
                    value_i32(guild_field(payload, "my_rank_id", "myRankId")).unwrap_or(-1);
                self.guild.permissions = guild_permissions_from_options(self.guild.my_options);
                if !was_in_guild {
                    self.guild.pending_invite_from = None;
                }
                changed = true;
            }
            "GuildNoticeChange" => {
                let update = value_i32(payload.get("update"));
                if update.is_some_and(|value| value < 0) {
                    // Crystal uses the negative update as the terminal reply
                    // to EditGuildNotice. It carries no replacement content.
                    success = Some(true);
                } else {
                    let Some(notice) = bounded_strings(payload.get("notice"), MAX_NOTICE_LINES)
                    else {
                        return false;
                    };
                    self.guild.notice = notice;
                }
                changed = true;
            }
            "GuildNoticeResult" => {
                let Some(result) = payload.get("success").and_then(Value::as_bool) else {
                    return false;
                };
                success = Some(result);
                changed = true;
            }
            "GuildMemberChange" => {
                let Some(ranks) = payload.get("ranks").and_then(Value::as_array) else {
                    return false;
                };
                if ranks.len() > MAX_GUILD_RANKS {
                    return false;
                }
                let mut all = Vec::new();
                let mut parsed_ranks = Vec::new();
                for rank in ranks {
                    let Some(name) = clean_name(rank.get("name")) else {
                        return false;
                    };
                    let members = rank
                        .get("members")
                        .and_then(Value::as_array)
                        .map_or(&[][..], Vec::as_slice);
                    if all.len() + members.len() > MAX_GUILD_MEMBERS {
                        return false;
                    }
                    let mut parsed_members = Vec::new();
                    for member in members {
                        let Some(mut parsed) = parse_guild_member(member) else {
                            return false;
                        };
                        parsed.rank_name = Some(name.clone());
                        parsed.rank_index = value_u8(rank.get("index"));
                        all.push(parsed.clone());
                        parsed_members.push(parsed);
                    }
                    parsed_ranks.push(GuildRankModel {
                        name,
                        index: value_i32(rank.get("index")).unwrap_or(0),
                        options: value_u8(rank.get("options")).unwrap_or(0),
                        members: parsed_members,
                    });
                }
                let had_members = !self.guild.members.is_empty();
                self.guild.ranks = parsed_ranks;
                self.guild.members = all;
                // GuildMemberChange carries the authoritative rank table. If
                // the server included the viewer's rank, refresh permissions
                // from that rank's options instead of retaining stale status
                // bits. A missing rank is intentionally left unchanged: an
                // incomplete roster is not evidence that permissions vanished.
                if let Some(my_rank) = self
                    .guild
                    .ranks
                    .iter()
                    .find(|rank| rank.index == self.guild.my_rank_id)
                {
                    self.guild.my_options = my_rank.options;
                    self.guild.permissions = guild_permissions_from_options(my_rank.options);
                }
                if !had_members && !self.guild.members.is_empty() {
                    self.guild.pending_invite_from = None;
                }
                changed = true;
            }
            "GuildStorageGoldChange" => {
                let Some(change_type) = value_u8(payload.get("changeType")) else {
                    return false;
                };
                let Some(amount) = value_u32(payload.get("amount")) else {
                    return false;
                };
                if amount == 0 || change_type > 4 {
                    return false;
                }
                match change_type {
                    0 => self.guild.gold = self.guild.gold.saturating_add(amount),
                    1 | 2 => self.guild.gold = self.guild.gold.saturating_sub(amount),
                    // 3/4 are exact failure receipts for requests 0/1. They
                    // intentionally leave the authoritative balance alone.
                    3 | 4 => {}
                    _ => unreachable!("bounded above"),
                }
                success = Some(change_type <= 2);
                changed = true;
            }
            "GuildStorageList" => {
                let Some(items) = payload.get("items").and_then(Value::as_array) else {
                    return false;
                };
                if items.len() > MAX_GUILD_STORAGE_ITEMS {
                    return false;
                }
                let mut parsed = Vec::with_capacity(items.len());
                for item in items {
                    if item.is_null() {
                        parsed.push(None);
                    } else {
                        let Some(item) = parse_guild_storage_item(item) else {
                            return false;
                        };
                        parsed.push(Some(item));
                    }
                }
                parsed.resize(MAX_GUILD_STORAGE_ITEMS, None);
                self.guild.storage_items = parsed;
                success = Some(true);
                changed = true;
            }
            "GuildStorageItemChange" => {
                let Some(change_type) = value_u8(payload.get("changeType")) else {
                    return false;
                };
                let (Some(from_slot), Some(to_slot)) =
                    (value_i32(payload.get("from")), value_i32(payload.get("to")))
                else {
                    return false;
                };
                from = Some(from_slot);
                to = Some(to_slot);
                if change_type >= 3 {
                    success = Some(false);
                    changed = true;
                } else {
                    if self.guild.storage_items.len() != MAX_GUILD_STORAGE_ITEMS {
                        self.guild
                            .storage_items
                            .resize(MAX_GUILD_STORAGE_ITEMS, None);
                    }
                    let from_index = usize::try_from(from_slot).ok();
                    let to_index = usize::try_from(to_slot).ok();
                    match change_type {
                        0 => {
                            let (Some(to_index), Some(item)) = (
                                to_index.filter(|slot| *slot < MAX_GUILD_STORAGE_ITEMS),
                                payload.get("item").and_then(parse_guild_storage_item),
                            ) else {
                                return false;
                            };
                            self.guild.storage_items[to_index] = Some(item);
                        }
                        1 => {
                            let Some(from_index) =
                                from_index.filter(|slot| *slot < MAX_GUILD_STORAGE_ITEMS)
                            else {
                                return false;
                            };
                            self.guild.storage_items[from_index] = None;
                        }
                        2 => {
                            let (Some(from_index), Some(to_index)) = (
                                from_index.filter(|slot| *slot < MAX_GUILD_STORAGE_ITEMS),
                                to_index.filter(|slot| *slot < MAX_GUILD_STORAGE_ITEMS),
                            ) else {
                                return false;
                            };
                            self.guild.storage_items.swap(from_index, to_index);
                        }
                        _ => unreachable!("change_type is bounded above"),
                    }
                    success = Some(true);
                    changed = true;
                }
            }
            "GuildInvite" => {
                let Some(name) = clean_name(payload.get("name")) else {
                    return false;
                };
                if self.guild.pending_invite_from.as_deref() != Some(name.as_str()) {
                    self.guild.pending_invite_epoch =
                        self.guild.pending_invite_epoch.saturating_add(1).max(1);
                }
                self.guild.pending_invite_from = Some(name);
                changed = true;
            }
            "GuildInviteResult" => {
                let Some(result) = payload
                    .get("success")
                    .or_else(|| payload.get("accepted"))
                    .and_then(Value::as_bool)
                else {
                    return false;
                };
                if clean_name(payload.get("name")).is_none() {
                    return false;
                }
                success = Some(result);
                changed = true;
            }
            "TradeRequest" => {
                let Some(name) =
                    clean_name(payload.get("partnerName").or_else(|| payload.get("name")))
                else {
                    return false;
                };
                self.trade = TradeModel {
                    partner: Some(name),
                    state: "requested".to_owned(),
                    ..TradeModel::default()
                };
                changed = true;
            }
            "TradeAccept" => {
                let Some(name) =
                    clean_name(payload.get("partnerName").or_else(|| payload.get("name")))
                else {
                    return false;
                };
                if self.trade.state != "open"
                    || self.trade.partner.as_deref() != Some(name.as_str())
                {
                    self.trade = TradeModel {
                        partner: Some(name),
                        state: "open".to_owned(),
                        open_revision: next_trade_revision,
                        ..TradeModel::default()
                    };
                }
                changed = true;
            }
            "TradeGold" => {
                let Some(amount) =
                    value_u32(payload.get("partnerGold").or_else(|| payload.get("amount")))
                else {
                    return false;
                };
                self.trade.partner_gold = amount;
                self.trade.my_confirmed = false;
                changed = true;
            }
            "TradeItem" => {
                let items = payload
                    .get("partnerItems")
                    .or_else(|| payload.get("tradeItems"))
                    .and_then(Value::as_array);
                let Some(items) = items else {
                    return false;
                };
                if items.len() > MAX_TRADE_ITEMS {
                    return false;
                }
                let Some(parsed) = items
                    .iter()
                    .map(|item| {
                        if item.is_null() {
                            Some(None)
                        } else {
                            parse_trade_item(item).map(Some)
                        }
                    })
                    .collect::<Option<Vec<_>>>()
                else {
                    return false;
                };
                self.trade.partner_items = parsed;
                self.trade.my_confirmed = false;
                changed = true;
            }
            "TradeConfirm" => {
                // GameScene.TradeConfirm is settlement completion, NOT a
                // partner's lock notification. Crystal resets both windows.
                self.trade = TradeModel::default();
                changed = true;
            }
            "TradeCancel" => {
                let Some(unlock) = payload.get("unlock").and_then(Value::as_bool) else {
                    return false;
                };
                if unlock {
                    self.trade.my_confirmed = false;
                } else {
                    self.trade = TradeModel::default();
                }
                changed = true;
            }
            "DepositTradeItem" | "RetrieveTradeItem" => {
                from = value_i32(payload.get("from"));
                to = value_i32(payload.get("to"));
                if from.is_none() || to.is_none() {
                    return false;
                }
                success = payload.get("success").and_then(Value::as_bool);
                changed = success.is_some();
                if changed {
                    // Both success and failure release Crystal's cell locks.
                    self.trade.my_confirmed = false;
                }
            }
            _ => return false,
        }
        if changed {
            if packet.starts_with("Trade")
                || matches!(packet, "DepositTradeItem" | "RetrieveTradeItem")
            {
                self.trade.event_revision = next_trade_revision;
                if matches!(
                    packet,
                    "TradeGold" | "TradeItem" | "DepositTradeItem" | "RetrieveTradeItem"
                ) || (packet == "TradeCancel"
                    && payload.get("unlock") == Some(&Value::Bool(true)))
                {
                    self.trade.unlock_revision = next_trade_revision;
                }
            }
            let subject = payload
                .get("name")
                .and_then(Value::as_str)
                .and_then(|name| clean_name(Some(&Value::String(name.to_owned()))));
            self.last_event = Some(SocialAuthoritativeEvent {
                packet: packet.to_owned(),
                success,
                subject,
                from,
                to,
                change_type: value_u8(payload.get("changeType")),
                rank_index: value_u8(payload.get("rankIndex")),
                amount: value_u32(payload.get("amount")),
            });
            self.normalize();
            true
        } else {
            false
        }
    }

    pub fn normalize(&mut self) {
        self.group.members.truncate(MAX_GROUP_MEMBERS);
        self.guild.members.truncate(MAX_GUILD_MEMBERS);
        self.guild.ranks.truncate(MAX_GUILD_RANKS);
        self.guild.storage_items.truncate(MAX_GUILD_STORAGE_ITEMS);
        self.guild.notice.truncate(MAX_NOTICE_LINES);
        self.guild.permissions.truncate(MAX_GUILD_PERMISSIONS);
        self.trade.partner_items.truncate(MAX_TRADE_ITEMS);
        self.trade.my_items.truncate(MAX_TRADE_ITEMS);
        self.pending.truncate(MAX_SOCIAL_PENDING);
    }

    fn reconcile_pending(&mut self, old: &SocialModel) {
        let event = self.last_event.as_ref();
        let current = self.clone();
        self.pending
            .retain(|operation| !Self::proven_complete(operation, event, old, &current));
    }

    /// `retain` keeps work that is *not* proven complete. Keeping this
    /// inversion in one helper prevents a successful packet from becoming a
    /// permanently stuck pending operation.
    fn proven_complete(
        operation: &SocialPendingOperation,
        event: Option<&SocialAuthoritativeEvent>,
        old: &SocialModel,
        current: &SocialModel,
    ) -> bool {
        // Completion/cancellation ends every request owned by this exchange.
        // An Unlock=true packet merely unlocks and retains both offers.
        if operation.is_trade()
            && event.is_some_and(|event| {
                event.packet == "TradeConfirm"
                    || (event.packet == "TradeCancel" && current.trade.state.is_empty())
                    || (event.packet == "TradeAccept"
                        && old.trade.state == "open"
                        && old.trade.open_revision != current.trade.open_revision)
            })
        {
            return true;
        }
        match operation {
            SocialPendingOperation::GroupSwitch { allow_group } => {
                event.is_some_and(|e| e.packet == "SwitchGroup")
                    && current.group.allow_invites == *allow_group
            }
            SocialPendingOperation::GroupAdd { name } => event.is_some_and(|e| {
                (e.packet == "AddMember" && e.subject.as_deref() == Some(name.as_str()))
                    || (e.packet == "GroupMemberInfo"
                        && current
                            .group
                            .members
                            .iter()
                            .any(|member| member.name == *name))
            }),
            SocialPendingOperation::GroupRemove { name } => event.is_some_and(|e| {
                (e.packet == "DeleteMember" && e.subject.as_deref() == Some(name.as_str()))
                    || (e.packet == "GroupMemberInfo"
                        && !current
                            .group
                            .members
                            .iter()
                            .any(|member| member.name == *name))
            }),
            SocialPendingOperation::GroupInviteAccept {
                inviter,
                invite_epoch,
            } => {
                let explicit_result = event.is_some_and(|event| {
                    event.packet == "GroupInviteResult"
                        && event.subject.as_deref() == Some(inviter.as_str())
                        && event.success.is_some()
                });
                let superseded = event.is_some_and(|event| event.packet == "GroupInvite")
                    && current.group.pending_invite_epoch != *invite_epoch;
                let revoked = event.is_some_and(|event| event.packet == "DeleteGroup")
                    && current.group.pending_invite_from.is_none();
                let joined = event.is_some_and(|event| {
                    matches!(event.packet.as_str(), "AddMember" | "GroupMemberInfo")
                }) && !old.group.active
                    && current.group.active;
                explicit_result || superseded || revoked || joined
            }
            SocialPendingOperation::GuildInfo => event
                .is_some_and(|e| matches!(e.packet.as_str(), "GuildStatus" | "GuildMemberChange")),
            SocialPendingOperation::GuildMember {
                change_type,
                rank_index,
                name,
            } => event.is_some_and(|event| {
                event.packet == "GuildMemberChange"
                    && event.change_type == Some(*change_type)
                    && event.rank_index == Some(*rank_index)
                    && (name.is_empty() || event.subject.as_deref() == Some(name.as_str()))
            }),
            SocialPendingOperation::GuildNotice { notice } => event.is_some_and(|event| {
                (event.packet == "GuildNoticeChange" && event.success == Some(true))
                    || (event.packet == "GuildNoticeResult" && event.success.is_some())
                    || (event.packet == "GuildNoticeChange"
                        && old.guild.notice != current.guild.notice
                        && current.guild.notice == *notice)
            }),
            SocialPendingOperation::GuildInviteAccept {
                inviter,
                invite_epoch,
            } => {
                let explicit_result = event.is_some_and(|event| {
                    event.packet == "GuildInviteResult"
                        && event.subject.as_deref() == Some(inviter.as_str())
                        && event.success.is_some()
                });
                let superseded = event.is_some_and(|event| event.packet == "GuildInvite")
                    && current.guild.pending_invite_epoch != *invite_epoch;
                let revoked = event.is_some_and(|event| event.packet == "GuildStatus")
                    && current.guild.name.is_none()
                    && current.guild.pending_invite_from.is_none();
                let joined_by_roster = event
                    .is_some_and(|event| event.packet == "GuildMemberChange")
                    && old.guild.members.is_empty()
                    && !current.guild.members.is_empty();
                let joined_by_status = event.is_some_and(|event| event.packet == "GuildStatus")
                    && old.guild.name.is_none()
                    && current.guild.name.is_some();
                explicit_result || superseded || revoked || joined_by_roster || joined_by_status
            }
            SocialPendingOperation::GuildStorageGold {
                change_type,
                amount,
            } => event.is_some_and(|event| {
                event.packet == "GuildStorageGoldChange"
                    && event.success.is_some()
                    && (event.change_type == Some(*change_type)
                        || event.change_type == Some(change_type.saturating_add(3)))
                    && event.amount == Some(*amount)
            }),
            SocialPendingOperation::GuildStorageItem {
                change_type,
                from,
                to,
            } => event.is_some_and(|event| {
                event.packet == "GuildStorageItemChange"
                    && event.from == Some(*from)
                    && event.to == Some(*to)
                    && (event.change_type == Some(*change_type)
                        || event.change_type == Some(change_type.saturating_add(3)))
            }),
            SocialPendingOperation::TradeRequest | SocialPendingOperation::TradeReply => {
                event.is_some_and(|e| matches!(e.packet.as_str(), "TradeRequest" | "TradeAccept"))
                    && old.trade.state != current.trade.state
            }
            // Crystal has no sender/request id on these packets. A partner
            // update is not proof that this client's offer/lock was accepted.
            SocialPendingOperation::TradeGold { amount } => event.is_some_and(|event| {
                // This is an internal read-model event, not a new wire packet.
                // A same-owner snapshot must prove the exact positive delta;
                // equal/stale or partner updates cannot release the request.
                event.packet == "NativeOwnTradeSnapshot"
                    && *amount > 0
                    && current.trade.state == "open"
                    && old.trade.partner == current.trade.partner
                    && old.trade.open_revision == current.trade.open_revision
                    && current.trade.my_offer_nonce.is_some()
                    && (old.trade.my_offer_nonce.is_none()
                        || old.trade.my_offer_nonce == current.trade.my_offer_nonce)
                    && old.trade.my_gold.checked_add(*amount) == Some(current.trade.my_gold)
            }),
            SocialPendingOperation::TradeConfirm { .. } => event.is_some_and(|event| {
                matches!(
                    event.packet.as_str(),
                    "TradeCancel"
                        | "TradeGold"
                        | "TradeItem"
                        | "DepositTradeItem"
                        | "RetrieveTradeItem"
                ) && !current.trade.my_confirmed
            }),
            SocialPendingOperation::TradeDeposit { from, to } => event.is_some_and(|e| {
                e.packet == "DepositTradeItem"
                    && e.success.is_some()
                    && e.from == Some(*from)
                    && e.to == Some(*to)
            }),
            SocialPendingOperation::TradeRetrieve { from, to } => event.is_some_and(|e| {
                e.packet == "RetrieveTradeItem"
                    && e.success.is_some()
                    && e.from == Some(*from)
                    && e.to == Some(*to)
            }),
            SocialPendingOperation::TradeCancel => false,
        }
    }
}

#[cfg(test)]
#[path = "social_trade_tests.rs"]
mod trade_source_tests;

fn clean_name(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?.trim();
    (!text.is_empty() && text.chars().count() <= 32).then(|| text.to_owned())
}

fn guild_permissions_from_options(options: u8) -> Vec<String> {
    GUILD_PERMISSION_KEYS
        .iter()
        .enumerate()
        .filter_map(|(bit, key)| ((options & (1 << bit)) != 0).then(|| (*key).to_owned()))
        .collect()
}

fn guild_field<'a>(payload: &'a Value, snake: &str, camel: &str) -> Option<&'a Value> {
    payload.get(snake).or_else(|| payload.get(camel))
}

fn bounded_strings(value: Option<&Value>, max: usize) -> Option<Vec<String>> {
    let values = value?.as_array()?;
    if values.len() > max {
        return None;
    }
    values.iter().map(|value| clean_name(Some(value))).collect()
}

fn parse_group_member(value: &Value) -> Option<GroupMemberModel> {
    let name = clean_name(value.get("name"))?;
    Some(GroupMemberModel {
        name,
        leader: value
            .get("leader")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        online: value
            .get("online")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        level: value_u16(value.get("level")),
        class: value_u8(value.get("class")),
        hp: value_i32(value.get("hp")),
        max_hp: value_i32(value.get("maxHp")),
        map: clean_name(value.get("map")),
    })
}

fn parse_guild_member(value: &Value) -> Option<GuildMemberModel> {
    Some(GuildMemberModel {
        name: clean_name(value.get("name"))?,
        id: value_i32(value.get("id")).unwrap_or(0),
        online: value
            .get("online")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        ..Default::default()
    })
}

fn parse_guild_storage_item(value: &Value) -> Option<GuildStorageItemModel> {
    let item = value.get("item")?;
    let unique_id = value_u64(item.get("unique_id").or_else(|| item.get("uniqueId")))?;
    let item_index = value_i32(item.get("item_index").or_else(|| item.get("itemIndex")))?;
    let count = value_u16(item.get("count")).unwrap_or(1).max(1);
    let user_id = value_i64(value.get("user_id").or_else(|| value.get("userId"))).unwrap_or(0);
    Some(GuildStorageItemModel {
        unique_id,
        item_index,
        count,
        user_id,
        tooltip_source: value
            .get("tooltipSource")
            .or_else(|| item.get("tooltipSource"))
            .and_then(|source| serde_json::from_value(source.clone()).ok()),
    })
}

fn parse_trade_item(value: &Value) -> Option<TradeItemModel> {
    value.as_object()?;
    let unique_id = value.get("uniqueId").or_else(|| value.get("unique_id"));
    let item_index = value.get("itemIndex").or_else(|| value.get("item_index"));
    let item = TradeItemModel {
        unique_id: match unique_id {
            Some(v) => Some(value_u64(Some(v))?),
            None => None,
        },
        item_index: match item_index {
            Some(v) => Some(value_i32(Some(v))?),
            None => None,
        },
        name: clean_name(value.get("name")),
        count: match value.get("count") {
            Some(v) => value_u16(Some(v))?,
            None => 1,
        },
        tooltip_source: value
            .get("tooltipSource")
            .and_then(|source| serde_json::from_value(source.clone()).ok()),
    };
    (item.unique_id.is_some()
        || item.item_index.is_some()
        || item.name.is_some()
        || item.tooltip_source.is_some())
    .then_some(item)
}

fn upsert_group_member(group: &mut GroupModel, name: String) {
    if let Some(member) = group.members.iter_mut().find(|member| member.name == name) {
        member.online = true;
    } else if group.members.len() < MAX_GROUP_MEMBERS {
        group.members.push(GroupMemberModel {
            name: name.clone(),
            online: true,
            leader: group.members.is_empty(),
            ..Default::default()
        });
    }
    group.active = !group.members.is_empty();
    if group.leader_name.is_none() {
        group.leader_name = Some(name);
    }
}

fn value_u8(value: Option<&Value>) -> Option<u8> {
    value?.as_u64().and_then(|n| u8::try_from(n).ok())
}
fn value_u16(value: Option<&Value>) -> Option<u16> {
    value?.as_u64().and_then(|n| u16::try_from(n).ok())
}
fn value_u32(value: Option<&Value>) -> Option<u32> {
    value?.as_u64().and_then(|n| u32::try_from(n).ok())
}
fn value_u64(value: Option<&Value>) -> Option<u64> {
    value?.as_u64()
}
fn value_i32(value: Option<&Value>) -> Option<i32> {
    value?.as_i64().and_then(|n| i32::try_from(n).ok())
}
fn value_i64(value: Option<&Value>) -> Option<i64> {
    value?.as_i64()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::{GuildMember, GuildRank, ServerPacket};
    use serde_json::json;

    #[test]
    fn malformed_and_oversized_social_packets_fail_closed() {
        let mut model = SocialModel::default();
        assert!(!model.apply_packet("GroupMemberInfo", &json!({"members": "bad"})));
        assert!(!model.apply_packet(
            "GroupMemberInfo",
            &json!({"members": (0..16).map(|i| json!({"name": i.to_string()})).collect::<Vec<_>>()})
        ));
        assert!(!model.apply_packet("TradeItem", &json!({"partnerItems": {}})));
        assert!(model.group.members.is_empty());
    }

    #[test]
    fn group_roster_is_bounded_and_leader_is_authoritative() {
        let mut model = SocialModel::default();
        assert!(model.apply_packet("GroupMemberInfo", &json!({"leaderName":"Alice","members":[{"name":"Alice","online":true},{"name":"Bob","online":false}]})));
        assert_eq!(model.group.leader_name.as_deref(), Some("Alice"));
        assert_eq!(model.group.members.len(), 2);
    }

    #[test]
    fn guild_packets_accept_server_packet_snake_case_and_camel_case_compatibility() {
        let rank = GuildRank {
            name: "Chief".into(),
            options: 7,
            index: 1,
            members: vec![GuildMember {
                name: "Miner".into(),
                id: 42,
                last_login_binary_datetime: 0,
                has_voted: false,
                online: true,
            }],
        };
        let status = serde_json::to_value(ServerPacket::GuildStatus {
            guild_name: "Guild".into(),
            guild_rank_name: "Chief".into(),
            level: 3,
            experience: 100,
            max_experience: 1_000,
            gold: 500,
            spare_points: 2,
            member_count: 1,
            max_members: 30,
            voting: true,
            item_count: 5,
            buff_count: 1,
            my_options: 0x7f,
            my_rank_id: 1,
        })
        .expect("GuildStatus serializes");
        let status_payload = status.get("GuildStatus").expect("variant payload");
        assert!(
            status_payload.get("guild_name").is_some() || status_payload.get("guildName").is_some(),
            "unexpected ServerPacket payload shape: {status:?}"
        );
        let mut model = SocialModel::default();
        assert!(model.apply_packet("GuildStatus", status_payload));
        assert_eq!(model.guild.name.as_deref(), Some("Guild"));
        assert_eq!(model.guild.max_experience, 1_000);

        let member_change = serde_json::to_value(ServerPacket::GuildMemberChange {
            name: "Miner".into(),
            rank_index: 1,
            status: 6,
            ranks: vec![rank],
        })
        .expect("GuildMemberChange serializes");
        let member_payload = member_change
            .get("GuildMemberChange")
            .expect("variant payload");
        assert!(model.apply_packet("GuildMemberChange", member_payload));
        assert_eq!(
            model
                .last_event
                .as_ref()
                .and_then(|event| event.subject.as_deref()),
            Some("Miner")
        );
        assert_eq!(model.guild.members[0].name, "Miner");

        let mut snake = SocialModel::default();
        assert!(snake.apply_packet(
            "GuildStatus",
            &json!({
                "guild_name": "SnakeGuild",
                "guild_rank_name": "Officer",
                "max_experience": 300,
                "member_count": 2,
                "max_members": 20,
                "my_options": 5,
                "my_rank_id": 4
            })
        ));
        assert_eq!(snake.guild.name.as_deref(), Some("SnakeGuild"));
        assert_eq!(snake.guild.max_experience, 300);
        assert_eq!(snake.guild.member_count, 2);

        let notice = serde_json::to_value(ServerPacket::GuildNoticeChange {
            update: 1,
            notice: vec!["Hello".into()],
        })
        .expect("GuildNoticeChange serializes");
        assert!(model.apply_packet(
            "GuildNoticeChange",
            notice.get("GuildNoticeChange").expect("variant payload")
        ));
        let invite = serde_json::to_value(ServerPacket::GuildInvite {
            name: "Invitee".into(),
        })
        .expect("GuildInvite serializes");
        assert!(model.apply_packet(
            "GuildInvite",
            invite.get("GuildInvite").expect("variant payload")
        ));

        let mut camel = SocialModel::default();
        assert!(camel.apply_packet(
            "GuildStatus",
            &json!({
                "guildName": "CompatGuild",
                "guildRankName": "Member",
                "maxExperience": 200,
                "memberCount": 1,
                "maxMembers": 10,
                "myOptions": 3,
                "myRankId": 2
            })
        ));
        assert_eq!(camel.guild.name.as_deref(), Some("CompatGuild"));
        assert_eq!(camel.guild.max_experience, 200);
        assert_eq!(camel.guild.my_rank_id, 2);
    }

    #[test]
    fn guild_permissions_follow_wire_bits_and_matching_rank_options() {
        let status = serde_json::to_value(ServerPacket::GuildStatus {
            guild_name: "WireGuild".into(),
            guild_rank_name: "Officer".into(),
            level: 1,
            experience: 0,
            max_experience: 100,
            gold: 0,
            spare_points: 0,
            member_count: 1,
            max_members: 20,
            voting: false,
            item_count: 0,
            buff_count: 0,
            my_options: u8::MAX,
            my_rank_id: 4,
        })
        .expect("GuildStatus serializes");
        let status_payload = status.get("GuildStatus").expect("status payload");
        let mut model = SocialModel::default();
        assert!(model.apply_packet("GuildStatus", status_payload));
        assert_eq!(
            model.guild.permissions,
            GUILD_PERMISSION_KEYS
                .iter()
                .map(|key| (*key).to_owned())
                .collect::<Vec<_>>()
        );
        for key in [
            "CanChangeNotice",
            "CanKick",
            "CanRecruit",
            "CanChangeRank",
            "CanStoreItem",
            "CanRetrieveItem",
        ] {
            assert!(
                model
                    .guild
                    .permissions
                    .iter()
                    .any(|permission| permission == key),
                "wire permission bit must expose {key}"
            );
        }

        let rank = GuildRank {
            name: "Officer".into(),
            index: 4,
            options: 0b0101_1001,
            members: vec![GuildMember {
                name: "Me".into(),
                id: 7,
                last_login_binary_datetime: 0,
                has_voted: false,
                online: true,
            }],
        };
        let member_change = serde_json::to_value(ServerPacket::GuildMemberChange {
            name: "Me".into(),
            rank_index: 4,
            status: 0,
            ranks: vec![rank],
        })
        .expect("GuildMemberChange serializes");
        let member_payload = member_change
            .get("GuildMemberChange")
            .expect("member-change payload");
        assert!(model.apply_packet("GuildMemberChange", member_payload));
        assert_eq!(model.guild.my_options, 0b0101_1001);
        assert_eq!(
            model.guild.permissions,
            vec![
                "CanChangeRank".to_owned(),
                "CanStoreItem".to_owned(),
                "CanRetrieveItem".to_owned(),
                "CanChangeNotice".to_owned(),
            ]
        );

        // A roster that omits the viewer's rank is incomplete evidence; do
        // not turn the missing row into a fabricated permission revocation.
        assert!(model.apply_packet(
            "GuildMemberChange",
            &json!({
                "name": "Other",
                "rankIndex": 3,
                "status": 0,
                "ranks": [{"name":"Other","index":3,"options":0,"members":[]}]
            })
        ));
        assert_eq!(model.guild.my_options, 0b0101_1001);
        assert_eq!(model.guild.permissions.len(), 4);
    }

    #[test]
    fn guild_storage_packets_are_bounded_correlated_and_authoritative() {
        let item = json!({
            "item": {"unique_id": 77, "item_index": 900, "count": 2},
            "user_id": 12
        });
        let mut model = SocialModel::default();
        model.guild.gold = 100;
        assert!(
            model.begin_pending(SocialPendingOperation::GuildStorageGold {
                change_type: 0,
                amount: 25,
            })
        );
        reconcile_packet(
            &mut model,
            "GuildStorageGoldChange",
            json!({"changeType":0,"amount":25,"name":"Me"}),
        );
        assert_eq!(model.guild.gold, 125);
        assert!(model.pending.is_empty());

        assert!(
            model.begin_pending(SocialPendingOperation::GuildStorageGold {
                change_type: 1,
                amount: 40,
            })
        );
        reconcile_packet(
            &mut model,
            "GuildStorageGoldChange",
            json!({"changeType":4,"amount":40,"name":"Me"}),
        );
        assert_eq!(model.guild.gold, 125, "a NACK must not mutate gold");
        assert!(model.pending.is_empty(), "a matching NACK permits retry");

        assert!(
            model.begin_pending(SocialPendingOperation::GuildStorageGold {
                change_type: 0,
                amount: 7,
            })
        );
        reconcile_packet(
            &mut model,
            "GuildStorageGoldChange",
            json!({"changeType":2,"amount":25,"name":"War"}),
        );
        assert_eq!(model.guild.gold, 100, "type 2 is authoritative guild spend");
        assert_eq!(model.pending.len(), 1, "type 2 cannot ACK a user request");
        model.pending.clear();

        assert!(model.apply_packet("GuildStorageList", &json!({"items":[item.clone(), null]})));
        assert_eq!(model.guild.storage_items.len(), MAX_GUILD_STORAGE_ITEMS);
        assert_eq!(
            model.guild.storage_items[0].as_ref().map(|item| (
                item.unique_id,
                item.item_index,
                item.count,
                item.user_id
            )),
            Some((77, 900, 2, 12))
        );

        model.begin_pending(SocialPendingOperation::GuildStorageItem {
            change_type: 2,
            from: 0,
            to: 1,
        });
        reconcile_packet(
            &mut model,
            "GuildStorageItemChange",
            json!({"changeType":2,"from":0,"to":1,"user":12,"item":item}),
        );
        assert!(model.guild.storage_items[0].is_none());
        assert_eq!(
            model.guild.storage_items[1]
                .as_ref()
                .map(|item| item.unique_id),
            Some(77)
        );
        assert!(model.pending.is_empty());

        let before = model.guild.storage_items.clone();
        model.begin_pending(SocialPendingOperation::GuildStorageItem {
            change_type: 1,
            from: 1,
            to: 4,
        });
        reconcile_packet(
            &mut model,
            "GuildStorageItemChange",
            json!({"changeType":4,"from":1,"to":4,"user":12,"item":null}),
        );
        assert_eq!(model.guild.storage_items, before);
        assert!(
            model.pending.is_empty(),
            "matching failure releases only its request"
        );

        assert!(!model.apply_packet(
            "GuildStorageList",
            &json!({"items": vec![Value::Null; MAX_GUILD_STORAGE_ITEMS + 1]})
        ));
    }

    #[test]
    fn guild_rank_change_with_empty_member_name_releases_exact_pending() {
        let mut model = SocialModel::default();
        assert!(model.begin_pending(SocialPendingOperation::GuildMember {
            change_type: 2,
            rank_index: 3,
            name: String::new(),
        }));
        reconcile_packet(
            &mut model,
            "GuildMemberChange",
            json!({
                "changeType": 2,
                "rankIndex": 2,
                "name": "",
                "ranks": [{"name":"Wrong","index":2,"options":0,"members":[]}]
            }),
        );
        assert_eq!(model.pending.len(), 1);
        reconcile_packet(
            &mut model,
            "GuildMemberChange",
            json!({
                "changeType": 2,
                "rankIndex": 3,
                "name": "",
                "ranks": [{"name":"Renamed","index":3,"options":0,"members":[]}]
            }),
        );
        assert!(model.pending.is_empty());
    }

    #[test]
    fn pending_is_not_cleared_by_unchanged_snapshot_but_is_cleared_by_matching_event() {
        let mut model = SocialModel::default();
        model.begin_pending(SocialPendingOperation::GroupAdd { name: "Bob".into() });
        let mut snapshot = model.clone();
        snapshot.pending.clear();
        model.apply_authoritative(snapshot);
        assert_eq!(model.pending.len(), 1);
        assert!(model.apply_packet("AddMember", &json!({"name":"Bob"})));
        model.apply_authoritative(model.clone());
        assert!(model.pending.is_empty());
    }

    #[test]
    fn reset_clears_models_and_pending() {
        let mut model = SocialModel::default();
        model.apply_packet("TradeRequest", &json!({"name":"Bob"}));
        model.begin_pending(SocialPendingOperation::TradeConfirm { locked: true });
        model.clear_session();
        assert_eq!(model, SocialModel::default());
    }

    fn reconcile_packet(model: &mut SocialModel, packet: &str, payload: Value) {
        let mut incoming = model.clone();
        incoming.pending.clear();
        assert!(incoming.apply_packet(packet, &payload));
        model.apply_authoritative(incoming);
    }

    #[test]
    fn every_authoritative_group_and_guild_operation_releases_only_on_matching_state() {
        let mut switch = SocialModel::default();
        switch.group.allow_invites = false;
        assert!(switch.begin_pending(SocialPendingOperation::GroupSwitch { allow_group: true }));
        reconcile_packet(&mut switch, "SwitchGroup", json!({"allowGroup": true}));
        assert!(switch.pending.is_empty());

        let mut add = SocialModel::default();
        add.begin_pending(SocialPendingOperation::GroupAdd { name: "Bob".into() });
        reconcile_packet(&mut add, "AddMember", json!({"name":"Alice"}));
        assert_eq!(add.pending.len(), 1);
        reconcile_packet(&mut add, "AddMember", json!({"name":"Bob"}));
        assert!(add.pending.is_empty());

        let mut remove = SocialModel::default();
        remove.group.members = vec![GroupMemberModel {
            name: "Bob".into(),
            ..Default::default()
        }];
        remove.begin_pending(SocialPendingOperation::GroupRemove { name: "Bob".into() });
        reconcile_packet(&mut remove, "DeleteMember", json!({"name":"Alice"}));
        assert_eq!(remove.pending.len(), 1);
        reconcile_packet(&mut remove, "DeleteMember", json!({"name":"Bob"}));
        assert!(remove.pending.is_empty());

        let mut group_invite = SocialModel::default();
        reconcile_packet(&mut group_invite, "GroupInvite", json!({"name":"Leader"}));
        group_invite.begin_pending(SocialPendingOperation::GroupInviteAccept {
            inviter: "Leader".into(),
            invite_epoch: group_invite.group.pending_invite_epoch,
        });
        reconcile_packet(&mut group_invite, "AddMember", json!({"name":"Me"}));
        assert!(
            group_invite.pending.is_empty(),
            "inactive-to-active authoritative membership transition resolves the invite"
        );

        let mut guild_info = SocialModel::default();
        guild_info.begin_pending(SocialPendingOperation::GuildInfo);
        reconcile_packet(
            &mut guild_info,
            "GuildStatus",
            json!({"guildName":"Guild","guildRankName":"Chief"}),
        );
        assert!(guild_info.pending.is_empty());

        let mut member = SocialModel::default();
        member.begin_pending(SocialPendingOperation::GuildMember {
            change_type: 4,
            rank_index: 2,
            name: "Miner".into(),
        });
        reconcile_packet(
            &mut member,
            "GuildMemberChange",
            json!({"changeType":4,"rankIndex":2,"name":"Other","ranks":[]}),
        );
        assert_eq!(member.pending.len(), 1);
        reconcile_packet(
            &mut member,
            "GuildMemberChange",
            json!({"changeType":4,"rankIndex":2,"name":"Miner","ranks":[]}),
        );
        assert!(member.pending.is_empty());

        let mut notice = SocialModel::default();
        notice.begin_pending(SocialPendingOperation::GuildNotice {
            notice: vec!["Hello".into()],
        });
        reconcile_packet(
            &mut notice,
            "GuildNoticeChange",
            json!({"notice":["Hello"]}),
        );
        assert!(notice.pending.is_empty());

        let mut invite = SocialModel::default();
        reconcile_packet(&mut invite, "GuildInvite", json!({"name":"Chief"}));
        invite.begin_pending(SocialPendingOperation::GuildInviteAccept {
            inviter: "Chief".into(),
            invite_epoch: invite.guild.pending_invite_epoch,
        });
        reconcile_packet(
            &mut invite,
            "GuildStatus",
            json!({"guildName":"Guild","guildRankName":"Member"}),
        );
        assert!(
            invite.pending.is_empty(),
            "not-in-guild to authoritative guild membership resolves the invite"
        );
    }

    #[test]
    fn group_invite_accept_is_correlated_and_allows_second_invite() {
        let mut model = SocialModel::default();
        reconcile_packet(&mut model, "GroupInvite", json!({"name":"LeaderA"}));
        let epoch_a = model.group.pending_invite_epoch;
        assert!(
            model.begin_pending(SocialPendingOperation::GroupInviteAccept {
                inviter: "LeaderA".into(),
                invite_epoch: epoch_a,
            })
        );

        reconcile_packet(&mut model, "GroupInvite", json!({"name":"LeaderA"}));
        assert_eq!(model.group.pending_invite_epoch, epoch_a);
        assert_eq!(model.pending.len(), 1, "duplicate invite is not an ACK");

        reconcile_packet(
            &mut model,
            "GroupMemberInfo",
            json!({"leaderName":"LeaderA","members":[{"name":"Me"},{"name":"LeaderA"}]}),
        );
        assert!(model.pending.is_empty());

        reconcile_packet(&mut model, "DeleteGroup", json!({}));
        reconcile_packet(&mut model, "GroupInvite", json!({"name":"LeaderB"}));
        assert!(
            model.begin_pending(SocialPendingOperation::GroupInviteAccept {
                inviter: "LeaderB".into(),
                invite_epoch: model.group.pending_invite_epoch,
            })
        );
    }

    #[test]
    fn invite_failure_is_exactly_correlated_and_can_retry() {
        let mut group = SocialModel::default();
        reconcile_packet(&mut group, "GroupInvite", json!({"name":"LeaderA"}));
        let group_epoch = group.group.pending_invite_epoch;
        let group_operation = SocialPendingOperation::GroupInviteAccept {
            inviter: "LeaderA".into(),
            invite_epoch: group_epoch,
        };
        assert!(group.begin_pending(group_operation.clone()));
        reconcile_packet(
            &mut group,
            "GroupInviteResult",
            json!({"name":"Other","success":false}),
        );
        assert_eq!(group.pending, vec![group_operation.clone()]);
        reconcile_packet(
            &mut group,
            "GroupInviteResult",
            json!({"name":"LeaderA","success":false}),
        );
        assert!(group.pending.is_empty());
        assert!(group.begin_pending(group_operation));

        let mut guild = SocialModel::default();
        reconcile_packet(&mut guild, "GuildInvite", json!({"name":"ChiefA"}));
        let guild_operation = SocialPendingOperation::GuildInviteAccept {
            inviter: "ChiefA".into(),
            invite_epoch: guild.guild.pending_invite_epoch,
        };
        assert!(guild.begin_pending(guild_operation.clone()));
        reconcile_packet(
            &mut guild,
            "GuildInviteResult",
            json!({"name":"ChiefA","accepted":false}),
        );
        assert!(guild.pending.is_empty());
        assert!(guild.begin_pending(guild_operation));
    }

    #[test]
    fn guild_invite_duplicate_and_roster_transition_are_idempotent() {
        let mut model = SocialModel::default();
        reconcile_packet(&mut model, "GuildInvite", json!({"name":"ChiefA"}));
        let epoch = model.guild.pending_invite_epoch;
        assert!(
            model.begin_pending(SocialPendingOperation::GuildInviteAccept {
                inviter: "ChiefA".into(),
                invite_epoch: epoch,
            })
        );
        reconcile_packet(&mut model, "GuildInvite", json!({"name":"ChiefA"}));
        assert_eq!(model.guild.pending_invite_epoch, epoch);
        assert_eq!(model.pending.len(), 1);
        reconcile_packet(
            &mut model,
            "GuildMemberChange",
            json!({"name":"Me","ranks":[{"name":"Member","index":0,"members":[{"name":"Me"}]}]}),
        );
        assert!(model.pending.is_empty());
        let members = model.guild.members.clone();
        reconcile_packet(
            &mut model,
            "GuildMemberChange",
            json!({"name":"Me","ranks":[{"name":"Member","index":0,"members":[{"name":"Me"}]}]}),
        );
        assert_eq!(model.guild.members, members);
        assert!(model.pending.is_empty());
    }

    #[test]
    fn a_different_inviter_supersedes_only_the_old_acceptance() {
        let mut model = SocialModel::default();
        reconcile_packet(&mut model, "GuildInvite", json!({"name":"ChiefA"}));
        assert!(
            model.begin_pending(SocialPendingOperation::GuildInviteAccept {
                inviter: "ChiefA".into(),
                invite_epoch: model.guild.pending_invite_epoch,
            })
        );
        reconcile_packet(&mut model, "GuildInvite", json!({"name":"ChiefB"}));
        assert!(model.pending.is_empty());
        assert_eq!(model.guild.pending_invite_from.as_deref(), Some("ChiefB"));
        assert!(
            model.begin_pending(SocialPendingOperation::GuildInviteAccept {
                inviter: "ChiefB".into(),
                invite_epoch: model.guild.pending_invite_epoch,
            })
        );
    }

    #[test]
    fn guild_notice_receipt_does_not_optimistically_replace_content() {
        let mut model = SocialModel::default();
        model.guild.notice = vec!["Old".into()];
        model.begin_pending(SocialPendingOperation::GuildNotice {
            notice: vec!["New".into()],
        });
        reconcile_packet(
            &mut model,
            "GuildNoticeChange",
            json!({"update":-1,"notice":[]}),
        );
        assert!(model.pending.is_empty());
        assert_eq!(model.guild.notice, vec!["Old"]);
        reconcile_packet(
            &mut model,
            "GuildNoticeChange",
            json!({"update":1,"notice":["New"]}),
        );
        assert_eq!(model.guild.notice, vec!["New"]);
    }

    #[test]
    fn failed_or_unrelated_trade_packets_do_not_fake_completion() {
        let mut trade = SocialModel::default();
        trade.trade.state = "open".into();
        trade.begin_pending(SocialPendingOperation::TradeGold { amount: 100 });
        trade.begin_pending(SocialPendingOperation::TradeConfirm { locked: true });
        reconcile_packet(&mut trade, "TradeGold", json!({"amount":100}));
        assert_eq!(
            trade.pending,
            vec![SocialPendingOperation::TradeGold { amount: 100 }]
        );
        reconcile_packet(&mut trade, "TradeConfirm", json!({}));
        assert!(trade.pending.is_empty());
        assert!(trade.trade.state.is_empty());

        let mut deposit = SocialModel::default();
        deposit.begin_pending(SocialPendingOperation::TradeDeposit { from: 2, to: 0 });
        reconcile_packet(
            &mut deposit,
            "DepositTradeItem",
            json!({"from":2,"to":0,"success":false}),
        );
        assert!(deposit.pending.is_empty()); // Failure releases this exact cell lock, not success.
        deposit.begin_pending(SocialPendingOperation::TradeDeposit { from: 2, to: 0 });
        reconcile_packet(
            &mut deposit,
            "DepositTradeItem",
            json!({"from":2,"to":0,"success":true}),
        );
        assert!(deposit.pending.is_empty());
    }

    #[test]
    fn trade_deposit_and_retrieve_ack_match_packet_and_both_slots() {
        let mut model = SocialModel::default();
        model.begin_pending(SocialPendingOperation::TradeDeposit { from: 2, to: 0 });
        model.begin_pending(SocialPendingOperation::TradeRetrieve { from: 0, to: 3 });

        // This is the review reproduction: a Deposit(2,0) and a
        // Retrieve(0,3) are simultaneously pending. A reply must release
        // exactly one matching operation.
        reconcile_packet(
            &mut model,
            "DepositTradeItem",
            json!({"from":0,"to":3,"success":true}),
        );
        assert_eq!(
            model.pending.len(),
            2,
            "wrong slots must release neither operation"
        );
        reconcile_packet(
            &mut model,
            "RetrieveTradeItem",
            json!({"from":2,"to":0,"success":true}),
        );
        assert_eq!(
            model.pending.len(),
            2,
            "wrong packet and slots must release neither operation"
        );
        reconcile_packet(
            &mut model,
            "DepositTradeItem",
            json!({"from":2,"to":0,"success":true}),
        );
        assert_eq!(
            model.pending,
            vec![SocialPendingOperation::TradeRetrieve { from: 0, to: 3 }]
        );
        reconcile_packet(
            &mut model,
            "RetrieveTradeItem",
            json!({"from":0,"to":3,"success":true}),
        );
        assert!(model.pending.is_empty());
    }

    #[test]
    fn scene_reset_preserves_social_models_but_session_reset_clears_them() {
        let mut model = SocialModel::default();
        model.group.active = true;
        model.guild.name = Some("Guild".into());
        model.trade.state = "open".into();
        model.begin_pending(SocialPendingOperation::TradeConfirm { locked: true });
        model.last_event = Some(SocialAuthoritativeEvent {
            packet: "TradeConfirm".into(),
            success: None,
            subject: None,
            from: None,
            to: None,
            change_type: None,
            rank_index: None,
            amount: None,
        });
        model.clear_scene();
        assert!(model.group.active);
        assert_eq!(model.guild.name.as_deref(), Some("Guild"));
        assert_eq!(model.trade.state, "open");
        assert_eq!(model.pending.len(), 1);
        assert!(model.last_event.is_none());
        model.clear_session();
        assert_eq!(model, SocialModel::default());
    }

    #[test]
    fn empty_guild_status_is_authoritative_leave_and_releases_guild_pending() {
        let mut model = SocialModel::default();
        model.guild.name = Some("OldGuild".into());
        model.guild.rank_name = Some("Member".into());
        model.begin_pending(SocialPendingOperation::GuildInfo);
        assert!(model.apply_packet("GuildStatus", &json!({"guildName":"","guildRankName":""})));
        let incoming = model.clone();
        model.apply_authoritative(incoming);
        assert_eq!(model.guild, GuildModel::default());
        assert!(model.pending.is_empty());
    }

    #[test]
    fn out_of_order_snapshot_does_not_release_unacknowledged_trade_or_group_work() {
        let mut model = SocialModel::default();
        model.begin_pending(SocialPendingOperation::GroupRemove { name: "Bob".into() });
        model.begin_pending(SocialPendingOperation::TradeDeposit { from: 2, to: 0 });

        let mut older = SocialModel::default();
        older.group.members.push(GroupMemberModel {
            name: "Bob".into(),
            ..Default::default()
        });
        model.apply_authoritative(older);
        assert_eq!(model.pending.len(), 2);

        assert!(model.apply_packet(
            "DepositTradeItem",
            &json!({"from": 2, "to": 0, "success": false})
        ));
        model.apply_authoritative(model.clone());
        assert_eq!(
            model.pending,
            vec![SocialPendingOperation::GroupRemove { name: "Bob".into() }]
        );
        // Failure releases the exact request without inventing item movement.
        model.begin_pending(SocialPendingOperation::TradeDeposit { from: 2, to: 0 });
        assert!(model.apply_packet(
            "DepositTradeItem",
            &json!({"from": 2, "to": 0, "success": true})
        ));
        model.apply_authoritative(model.clone());
        assert_eq!(model.pending.len(), 1);
    }
}
