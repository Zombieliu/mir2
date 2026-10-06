//! Closed shared Group/Guild/Trade intent-to-wire projection.
//! No membership, permission, custody, wallet, lock or settlement authority.
use crate::crystal_ui::overlays::{
    NativePlayerUiIntent, NativePlayerUiIntentQueue, NativePlayerUiState,
};
use crate::social::{SocialModel, SocialPendingOperation};
use mir2_ui_core::effect::{valid_guild_storage_gold_change, valid_guild_storage_item_change};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NativeSocialCommand {
    SwitchGroup {
        #[serde(rename = "allowGroup")]
        allow_group: bool,
    },
    AddMember {
        name: String,
    },
    DelMember {
        name: String,
    },
    GroupInvite {
        #[serde(rename = "acceptInvite")]
        accept_invite: bool,
    },
    RequestGuildInfo {
        #[serde(rename = "infoType")]
        info_type: u8,
    },
    EditGuildMember {
        #[serde(rename = "changeType")]
        change_type: u8,
        #[serde(rename = "rankIndex")]
        rank_index: u8,
        name: String,
        #[serde(rename = "rankName")]
        rank_name: String,
    },
    EditGuildNotice {
        notice: Vec<String>,
    },
    GuildInvite {
        #[serde(rename = "acceptInvite")]
        accept_invite: bool,
    },
    GuildStorageGoldChange {
        #[serde(rename = "changeType")]
        change_type: u8,
        amount: u32,
    },
    GuildStorageItemChange {
        #[serde(rename = "changeType")]
        change_type: u8,
        from: i32,
        to: i32,
    },
    TradeRequest,
    TradeReply {
        #[serde(rename = "acceptInvite")]
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

impl NativeSocialCommand {
    pub fn command_type(&self) -> &'static str {
        match self {
            Self::SwitchGroup { .. } => "switchGroup",
            Self::AddMember { .. } => "addMember",
            Self::DelMember { .. } => "delMember",
            Self::GroupInvite { .. } => "groupInvite",
            Self::RequestGuildInfo { .. } => "requestGuildInfo",
            Self::EditGuildMember { .. } => "editGuildMember",
            Self::EditGuildNotice { .. } => "editGuildNotice",
            Self::GuildInvite { .. } => "guildInvite",
            Self::GuildStorageGoldChange { .. } => "guildStorageGoldChange",
            Self::GuildStorageItemChange { .. } => "guildStorageItemChange",
            Self::TradeRequest => "tradeRequest",
            Self::TradeReply { .. } => "tradeReply",
            Self::TradeGold { .. } => "tradeGold",
            Self::DepositTradeItem { .. } => "depositTradeItem",
            Self::RetrieveTradeItem { .. } => "retrieveTradeItem",
            Self::TradeConfirm { .. } => "tradeConfirm",
            Self::TradeCancel => "tradeCancel",
        }
    }

    pub fn valid_shared_bounds(&self) -> bool {
        match self {
            Self::GuildStorageGoldChange {
                change_type,
                amount,
            } => valid_guild_storage_gold_change(*change_type, *amount),
            Self::GuildStorageItemChange {
                change_type,
                from,
                to,
            } => valid_guild_storage_item_change(*change_type, *from, *to),
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSocialProjectionError {
    NotSocial,
    InvalidGuildStorage,
}

pub fn is_native_social_intent(intent: &NativePlayerUiIntent) -> bool {
    matches!(
        intent,
        NativePlayerUiIntent::GroupSwitch { .. }
            | NativePlayerUiIntent::GroupAddMember { .. }
            | NativePlayerUiIntent::GroupRemoveMember { .. }
            | NativePlayerUiIntent::GroupInvite { .. }
            | NativePlayerUiIntent::GuildRequestInfo { .. }
            | NativePlayerUiIntent::GuildEditMember { .. }
            | NativePlayerUiIntent::GuildEditNotice { .. }
            | NativePlayerUiIntent::GuildInvite { .. }
            | NativePlayerUiIntent::GuildStorageGoldChange { .. }
            | NativePlayerUiIntent::GuildStorageItemChange { .. }
            | NativePlayerUiIntent::TradeRequest
            | NativePlayerUiIntent::TradeReply { .. }
            | NativePlayerUiIntent::TradeGold { .. }
            | NativePlayerUiIntent::TradeDepositItem { .. }
            | NativePlayerUiIntent::TradeRetrieveItem { .. }
            | NativePlayerUiIntent::TradeConfirm { .. }
            | NativePlayerUiIntent::TradeCancel
    )
}

/// Exact public BrowserCommand field mapping, as on frozen Windows.
/// Only the existing shared guild-storage coordinate validators are reused.
pub fn project_native_social_intent(
    intent: &NativePlayerUiIntent,
) -> Result<NativeSocialCommand, NativeSocialProjectionError> {
    let command = match intent {
        NativePlayerUiIntent::GroupSwitch { allow_group } => NativeSocialCommand::SwitchGroup {
            allow_group: *allow_group,
        },
        NativePlayerUiIntent::GroupAddMember { name } => {
            NativeSocialCommand::AddMember { name: name.clone() }
        }
        NativePlayerUiIntent::GroupRemoveMember { name } => {
            NativeSocialCommand::DelMember { name: name.clone() }
        }
        NativePlayerUiIntent::GroupInvite { accept_invite } => NativeSocialCommand::GroupInvite {
            accept_invite: *accept_invite,
        },
        NativePlayerUiIntent::GuildRequestInfo { info_type } => {
            NativeSocialCommand::RequestGuildInfo {
                info_type: *info_type,
            }
        }
        NativePlayerUiIntent::GuildEditMember {
            change_type,
            rank_index,
            name,
            rank_name,
        } => NativeSocialCommand::EditGuildMember {
            change_type: *change_type,
            rank_index: *rank_index,
            name: name.clone(),
            rank_name: rank_name.clone(),
        },
        NativePlayerUiIntent::GuildEditNotice { notice } => NativeSocialCommand::EditGuildNotice {
            notice: notice.clone(),
        },
        NativePlayerUiIntent::GuildInvite { accept_invite } => NativeSocialCommand::GuildInvite {
            accept_invite: *accept_invite,
        },
        NativePlayerUiIntent::GuildStorageGoldChange {
            change_type,
            amount,
        } => NativeSocialCommand::GuildStorageGoldChange {
            change_type: *change_type,
            amount: *amount,
        },
        NativePlayerUiIntent::GuildStorageItemChange {
            change_type,
            from,
            to,
        } => NativeSocialCommand::GuildStorageItemChange {
            change_type: *change_type,
            from: *from,
            to: *to,
        },
        NativePlayerUiIntent::TradeRequest => NativeSocialCommand::TradeRequest,
        NativePlayerUiIntent::TradeReply { accept_invite } => NativeSocialCommand::TradeReply {
            accept_invite: *accept_invite,
        },
        NativePlayerUiIntent::TradeGold { amount } => {
            NativeSocialCommand::TradeGold { amount: *amount }
        }
        NativePlayerUiIntent::TradeDepositItem { from, to } => {
            NativeSocialCommand::DepositTradeItem {
                from: *from,
                to: *to,
            }
        }
        NativePlayerUiIntent::TradeRetrieveItem { from, to } => {
            NativeSocialCommand::RetrieveTradeItem {
                from: *from,
                to: *to,
            }
        }
        NativePlayerUiIntent::TradeConfirm { locked } => {
            NativeSocialCommand::TradeConfirm { locked: *locked }
        }
        NativePlayerUiIntent::TradeCancel => NativeSocialCommand::TradeCancel,
        _ => return Err(NativeSocialProjectionError::NotSocial),
    };
    if !command.valid_shared_bounds() {
        return Err(NativeSocialProjectionError::InvalidGuildStorage);
    }
    Ok(command)
}

/// Capture identity before a synchronous local enqueue. A later socket failure
/// is unknown outcome and belongs to the existing connection/DataReset owner;
/// this context must never be used to "undo" a command accepted by the queue.
pub struct NativeSocialDispatchContext {
    operation: Option<SocialPendingOperation>,
    group_identity: Option<(String, u64)>,
    guild_identity: Option<(String, u64)>,
    group_reply: bool,
    guild_reply: bool,
}

impl NativeSocialDispatchContext {
    pub fn capture(
        intent: &NativePlayerUiIntent,
        social: &SocialModel,
        ui: &NativePlayerUiState,
    ) -> Self {
        let group_reply = matches!(intent, NativePlayerUiIntent::GroupInvite { .. });
        let guild_reply = matches!(intent, NativePlayerUiIntent::GuildInvite { .. });
        let group_identity = group_reply
            .then(|| ui.group_dialog.answered.clone())
            .flatten();
        let guild_identity = guild_reply
            .then(|| ui.guild_panel.answered_invite.clone())
            .flatten();
        // Derive the exact pending operation through the original shared UI
        // queue, not a second platform-owned pending/receipt rule table.
        let mut probe = social.clone();
        probe.pending.clear();
        if group_reply {
            probe.group.pending_invite_from = group_identity.as_ref().map(|id| id.0.clone());
            probe.group.pending_invite_epoch = group_identity.as_ref().map_or(0, |id| id.1);
        }
        if guild_reply {
            probe.guild.pending_invite_from = guild_identity.as_ref().map(|id| id.0.clone());
            probe.guild.pending_invite_epoch = guild_identity.as_ref().map_or(0, |id| id.1);
        }
        let mut queue = NativePlayerUiIntentQueue::default();
        let operation = queue
            .push_social_pending(&mut probe, intent.clone())
            .then(|| probe.pending.into_iter().next())
            .flatten();
        Self {
            operation,
            group_identity,
            guild_identity,
            group_reply,
            guild_reply,
        }
    }

    pub fn correlation_known(&self) -> bool {
        (!self.group_reply || self.group_identity.is_some())
            && (!self.guild_reply || self.guild_identity.is_some())
    }

    pub fn invitation_current(&self, social: &SocialModel) -> bool {
        self.group_identity.as_ref().is_none_or(|id| {
            social.group.pending_invite_from.as_deref() == Some(id.0.as_str())
                && social.group.pending_invite_epoch == id.1
        }) && self.guild_identity.as_ref().is_none_or(|id| {
            social.guild.pending_invite_from.as_deref() == Some(id.0.as_str())
                && social.guild.pending_invite_epoch == id.1
        })
    }

    /// Definitely unsent only: release one exact shared pending identity and
    /// restore existing draft/focus state, preserving newer drafts/invitations.
    /// No authoritative social fields, wallet, inventory or receipts change.
    pub fn release_unsent(
        &self,
        intent: &NativePlayerUiIntent,
        social: &mut SocialModel,
        ui: &mut NativePlayerUiState,
    ) {
        if let Some(operation) = &self.operation {
            social.pending.retain(|entry| entry != operation);
        }
        match intent {
            NativePlayerUiIntent::GroupAddMember { name } => {
                ui.group_dialog.restore_unsent(false, name)
            }
            NativePlayerUiIntent::GroupRemoveMember { name } => {
                ui.group_dialog.restore_unsent(true, name)
            }
            NativePlayerUiIntent::GroupInvite { .. } => {
                if let Some(identity) = &self.group_identity {
                    ui.group_dialog.restore_invitation(identity, &social.group);
                }
            }
            NativePlayerUiIntent::GuildInvite { .. } => {
                if let Some(identity) = &self.guild_identity {
                    if ui.guild_panel.answered_invite.as_ref() == Some(identity) {
                        ui.guild_panel.answered_invite = None;
                        if ui.guild_panel.invite.is_none() && self.invitation_current(social) {
                            ui.guild_panel.invite = Some(identity.clone());
                        }
                    }
                }
            }
            NativePlayerUiIntent::GuildEditNotice { notice } => {
                if ui.guild_notice_submission.as_ref() == Some(notice) {
                    ui.guild_notice_submission = None;
                    ui.guild_notice_editing = true;
                }
            }
            NativePlayerUiIntent::GuildEditMember {
                change_type,
                rank_index,
                name,
                rank_name,
            } => {
                ui.guild_panel.create_ready_ms = 0;
                match *change_type {
                    0 if ui.guild_recruit_draft.is_empty() => {
                        ui.guild_recruit_draft = name.clone();
                        ui.guild_recruit_focused = true;
                        ui.guild_panel.sync_recruit(name);
                    }
                    3 if ui.selected_guild_rank == Some(*rank_index)
                        && ui.guild_rank_name_draft == *rank_name =>
                    {
                        ui.guild_panel.rank_name_ready_ms = 0;
                        ui.guild_rank_name_focused = true;
                        ui.guild_panel.rank_editor.editor_focused = true;
                    }
                    5 => ui.guild_panel.rank_option_ready_ms = 0,
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn cases() -> Vec<(NativePlayerUiIntent, Value)> {
        vec![
            (
                NativePlayerUiIntent::GroupSwitch { allow_group: true },
                json!({"type":"switchGroup","allowGroup":true}),
            ),
            (
                NativePlayerUiIntent::GroupAddMember {
                    name: "Alice".into(),
                },
                json!({"type":"addMember","name":"Alice"}),
            ),
            (
                NativePlayerUiIntent::GroupRemoveMember { name: "Bob".into() },
                json!({"type":"delMember","name":"Bob"}),
            ),
            (
                NativePlayerUiIntent::GroupInvite {
                    accept_invite: false,
                },
                json!({"type":"groupInvite","acceptInvite":false}),
            ),
            (
                NativePlayerUiIntent::GuildRequestInfo { info_type: 2 },
                json!({"type":"requestGuildInfo","infoType":2}),
            ),
            (
                NativePlayerUiIntent::GuildEditMember {
                    change_type: 3,
                    rank_index: 255,
                    name: "Alice".into(),
                    rank_name: "Officer".into(),
                },
                json!({"type":"editGuildMember","changeType":3,"rankIndex":255,"name":"Alice","rankName":"Officer"}),
            ),
            (
                NativePlayerUiIntent::GuildEditNotice {
                    notice: vec!["one".into(), "二".into()],
                },
                json!({"type":"editGuildNotice","notice":["one","二"]}),
            ),
            (
                NativePlayerUiIntent::GuildInvite {
                    accept_invite: true,
                },
                json!({"type":"guildInvite","acceptInvite":true}),
            ),
            (
                NativePlayerUiIntent::GuildStorageGoldChange {
                    change_type: 0,
                    amount: u32::MAX,
                },
                json!({"type":"guildStorageGoldChange","changeType":0,"amount":u32::MAX}),
            ),
            (
                NativePlayerUiIntent::GuildStorageItemChange {
                    change_type: 0,
                    from: 255,
                    to: 111,
                },
                json!({"type":"guildStorageItemChange","changeType":0,"from":255,"to":111}),
            ),
            (
                NativePlayerUiIntent::TradeRequest,
                json!({"type":"tradeRequest"}),
            ),
            (
                NativePlayerUiIntent::TradeReply {
                    accept_invite: false,
                },
                json!({"type":"tradeReply","acceptInvite":false}),
            ),
            (
                NativePlayerUiIntent::TradeGold { amount: u32::MAX },
                json!({"type":"tradeGold","amount":u32::MAX}),
            ),
            (
                NativePlayerUiIntent::TradeDepositItem { from: 79, to: 9 },
                json!({"type":"depositTradeItem","from":79,"to":9}),
            ),
            (
                NativePlayerUiIntent::TradeRetrieveItem { from: 9, to: 79 },
                json!({"type":"retrieveTradeItem","from":9,"to":79}),
            ),
            (
                NativePlayerUiIntent::TradeConfirm { locked: false },
                json!({"type":"tradeConfirm","locked":false}),
            ),
            (
                NativePlayerUiIntent::TradeCancel,
                json!({"type":"tradeCancel"}),
            ),
        ]
    }

    #[test]
    fn social_egress_all_17_intents_have_exact_closed_wire_fields() {
        let rows = cases();
        assert_eq!(rows.len(), 17);
        for (intent, expected) in rows {
            assert!(is_native_social_intent(&intent));
            let command = project_native_social_intent(&intent).unwrap();
            assert_eq!(serde_json::to_value(&command).unwrap(), expected);
            assert_eq!(command.command_type(), expected["type"].as_str().unwrap());
            assert!(!matches!(
                command.command_type(),
                "login" | "passkeyLogin" | "startGame" | "stage5Command" | "moveTo"
            ));
        }
        for intent in [
            NativePlayerUiIntent::ReadMail { mail_id: 1 },
            NativePlayerUiIntent::ClaimMail { mail_id: 2 },
            NativePlayerUiIntent::GuildBuffUpdate(
                crate::crystal_ui::overlays::guild_buff_dialog::GuildBuffRequest {
                    action: 0,
                    id: 7,
                },
            ),
        ] {
            assert!(!is_native_social_intent(&intent));
            assert_eq!(
                project_native_social_intent(&intent),
                Err(NativeSocialProjectionError::NotSocial)
            );
        }
    }

    #[test]
    fn social_egress_uses_original_shared_guild_storage_bounds_including_refresh() {
        for intent in [
            NativePlayerUiIntent::GuildStorageGoldChange {
                change_type: 2,
                amount: 1,
            },
            NativePlayerUiIntent::GuildStorageGoldChange {
                change_type: 0,
                amount: 0,
            },
            NativePlayerUiIntent::GuildStorageItemChange {
                change_type: 0,
                from: 256,
                to: 0,
            },
            NativePlayerUiIntent::GuildStorageItemChange {
                change_type: 1,
                from: 112,
                to: 0,
            },
            NativePlayerUiIntent::GuildStorageItemChange {
                change_type: 2,
                from: -1,
                to: 0,
            },
            NativePlayerUiIntent::GuildStorageItemChange {
                change_type: 3,
                from: 1,
                to: 0,
            },
        ] {
            assert_eq!(
                project_native_social_intent(&intent),
                Err(NativeSocialProjectionError::InvalidGuildStorage)
            );
        }
        let refresh = NativePlayerUiIntent::GuildStorageItemChange {
            change_type: 3,
            from: 0,
            to: 0,
        };
        let command = project_native_social_intent(&refresh).unwrap();
        assert_eq!(
            serde_json::to_value(command).unwrap(),
            json!({"type":"guildStorageItemChange","changeType":3,"from":0,"to":0})
        );
        let social = SocialModel::default();
        let mut ui = NativePlayerUiState::default();
        let context = NativeSocialDispatchContext::capture(&refresh, &social, &ui);
        let mut after = social.clone();
        context.release_unsent(&refresh, &mut after, &mut ui);
        assert_eq!(after, social, "a read-only refresh cannot clear a mutation");
    }

    #[test]
    fn social_egress_selective_bounded_drain_keeps_both_fifo_streams() {
        let mail1 = NativePlayerUiIntent::ReadMail { mail_id: 1 };
        let mail2 = NativePlayerUiIntent::ReadMail { mail_id: 2 };
        let switch = NativePlayerUiIntent::GroupSwitch { allow_group: true };
        let gold = NativePlayerUiIntent::TradeGold { amount: 125 };
        let info = NativePlayerUiIntent::GuildRequestInfo { info_type: 0 };
        let buff = NativePlayerUiIntent::GuildBuffUpdate(
            crate::crystal_ui::overlays::guild_buff_dialog::GuildBuffRequest { action: 0, id: 7 },
        );
        let mut queue = NativePlayerUiIntentQueue::default();
        for intent in [&mail1, &switch, &buff, &gold, &mail2, &info] {
            assert!(queue.push_transient_unique(intent.clone()));
        }
        assert!(queue.drain_social_intents_bounded(0).is_empty());
        assert_eq!(queue.drain_social_intents_bounded(2), vec![switch, gold]);
        assert_eq!(queue.drain_social_intents_bounded(2), vec![info]);
        assert_eq!(queue.drain_intents(), vec![mail1, buff, mail2]);
    }

    #[test]
    fn social_egress_unsent_group_operation_restores_matching_draft_only() {
        let intent = NativePlayerUiIntent::GroupAddMember {
            name: "Alice".into(),
        };
        let mut social = SocialModel::default();
        let mut queue = NativePlayerUiIntentQueue::default();
        assert!(queue.push_social_pending(&mut social, intent.clone()));
        assert!(social.begin_pending(SocialPendingOperation::TradeGold { amount: 125 }));
        social.trade.partner_gold = 17;
        let mut ui = NativePlayerUiState::default();
        ui.group_dialog.submitted = Some((false, "Alice".into()));
        let context = NativeSocialDispatchContext::capture(&intent, &social, &ui);
        let mut expected = social.clone();
        expected.pending.retain(
            |op| !matches!(op, SocialPendingOperation::GroupAdd { name } if name == "Alice"),
        );
        context.release_unsent(&intent, &mut social, &mut ui);
        assert_eq!(social, expected);
        assert_eq!(
            ui.group_dialog.editor.editor.as_ref().unwrap().text(),
            "Alice"
        );
        assert!(ui.group_dialog.editor.edit_notice.is_some());
        assert_eq!(social.trade.partner_gold, 17);
        ui.group_dialog.submitted = Some((false, "Bob".into()));
        let saved = ui.group_dialog.clone();
        context.release_unsent(&intent, &mut social, &mut ui);
        assert_eq!(ui.group_dialog, saved, "never overwrite a later draft");
    }

    #[test]
    fn social_egress_invitation_epochs_never_answer_or_resurrect_a_new_inviter() {
        let intent = NativePlayerUiIntent::GroupInvite {
            accept_invite: true,
        };
        let mut social = SocialModel::default();
        assert!(social.apply_packet("GroupInvite", &json!({"name":"Alice"})));
        let identity = ("Alice".into(), social.group.pending_invite_epoch);
        let mut ui = NativePlayerUiState::default();
        ui.group_dialog.answered = Some(identity.clone());
        let mut queue = NativePlayerUiIntentQueue::default();
        assert!(queue.push_social_pending(&mut social, intent.clone()));
        let context = NativeSocialDispatchContext::capture(&intent, &social, &ui);
        assert!(context.correlation_known() && context.invitation_current(&social));
        assert!(social.apply_packet("GroupInvite", &json!({"name":"Bob"})));
        ui.group_dialog.invitation = Some(("Bob".into(), social.group.pending_invite_epoch));
        assert!(!context.invitation_current(&social));
        context.release_unsent(&intent, &mut social, &mut ui);
        assert_eq!(ui.group_dialog.invitation.as_ref().unwrap().0, "Bob");
        assert!(social.pending.iter().all(|op| !matches!(op, SocialPendingOperation::GroupInviteAccept { inviter, invite_epoch } if inviter == "Alice" && *invite_epoch == identity.1)));
        let missing =
            NativeSocialDispatchContext::capture(&intent, &social, &NativePlayerUiState::default());
        assert!(!missing.correlation_known());

        let intent = NativePlayerUiIntent::GuildInvite {
            accept_invite: true,
        };
        assert!(social.apply_packet("GuildInvite", &json!({"name":"Alice"})));
        let identity = ("Alice".into(), social.guild.pending_invite_epoch);
        ui.guild_panel.answered_invite = Some(identity.clone());
        assert!(queue.push_social_pending(&mut social, intent.clone()));
        let context = NativeSocialDispatchContext::capture(&intent, &social, &ui);
        assert!(social.apply_packet("GuildInvite", &json!({"name":"Bob"})));
        assert!(!context.invitation_current(&social));
        context.release_unsent(&intent, &mut social, &mut ui);
        assert!(
            ui.guild_panel.invite.is_none(),
            "do not resurrect Alice after Bob's new invitation"
        );
        assert!(social.pending.iter().all(|op| !matches!(op, SocialPendingOperation::GuildInviteAccept { inviter, invite_epoch } if inviter == "Alice" && *invite_epoch == identity.1)));
    }

    #[test]
    fn social_egress_unsent_notice_and_member_recovery_preserve_new_edits() {
        let intent = NativePlayerUiIntent::GuildEditNotice {
            notice: vec!["old".into()],
        };
        let mut social = SocialModel::default();
        let mut queue = NativePlayerUiIntentQueue::default();
        assert!(queue.push_social_pending(&mut social, intent.clone()));
        let mut ui = NativePlayerUiState::default();
        ui.guild_notice_submission = Some(vec!["old".into()]);
        let context = NativeSocialDispatchContext::capture(&intent, &social, &ui);
        context.release_unsent(&intent, &mut social, &mut ui);
        assert!(ui.guild_notice_submission.is_none() && ui.guild_notice_editing);
        assert!(social.pending.is_empty());
        ui.guild_notice_submission = Some(vec!["new".into()]);
        ui.guild_notice_editing = false;
        context.release_unsent(&intent, &mut social, &mut ui);
        assert_eq!(ui.guild_notice_submission, Some(vec!["new".into()]));
        assert!(!ui.guild_notice_editing);

        let recruit = NativePlayerUiIntent::GuildEditMember {
            change_type: 0,
            rank_index: 0,
            name: "Alice".into(),
            rank_name: "".into(),
        };
        assert!(queue.push_social_pending(&mut social, recruit.clone()));
        let context = NativeSocialDispatchContext::capture(&recruit, &social, &ui);
        context.release_unsent(&recruit, &mut social, &mut ui);
        assert_eq!(ui.guild_recruit_draft, "Alice");
        assert!(ui.guild_recruit_focused);
        ui.guild_recruit_draft = "Bob".into();
        context.release_unsent(&recruit, &mut social, &mut ui);
        assert_eq!(ui.guild_recruit_draft, "Bob");
    }

    #[test]
    fn social_egress_all_trade_pending_recovery_uses_original_shared_queue_rules() {
        let mut social = SocialModel::default();
        assert!(social.apply_packet("TradeAccept", &json!({"name":"Alice"})));
        social.trade.my_gold = 50;
        social.trade.partner_gold = 17;
        let mut ui = NativePlayerUiState::default();
        for intent in [
            NativePlayerUiIntent::TradeRequest,
            NativePlayerUiIntent::TradeReply {
                accept_invite: true,
            },
            NativePlayerUiIntent::TradeGold { amount: 125 },
            NativePlayerUiIntent::TradeDepositItem { from: 79, to: 9 },
            NativePlayerUiIntent::TradeRetrieveItem { from: 9, to: 79 },
            NativePlayerUiIntent::TradeConfirm { locked: true },
            NativePlayerUiIntent::TradeCancel,
        ] {
            let mut current = social.clone();
            let mut queue = NativePlayerUiIntentQueue::default();
            assert!(queue.push_social_pending(&mut current, intent.clone()));
            let before = current.clone();
            let context = NativeSocialDispatchContext::capture(&intent, &current, &ui);
            assert_eq!(
                current, before,
                "context capture cannot acknowledge or settle"
            );
            context.release_unsent(&intent, &mut current, &mut ui);
            assert_eq!(
                current, social,
                "only the definitely-unsent pending operation may be released"
            );
        }
    }
}
