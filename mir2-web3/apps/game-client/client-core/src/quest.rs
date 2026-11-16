//! Presentation-only quest action policy shared by platform hosts.
//!
//! All facts here come from authoritative quest data or a host-validated,
//! currently open NPC dialog. An eligible action is only permission to submit
//! an intent; the server decides whether it succeeds.

/// Optional newcomer guidance profile selected by the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestProfile {
    Crystal,
    NewcomerV1,
    NewcomerV2,
}

impl QuestProfile {
    pub fn from_name(name: &str) -> Self {
        if name.eq_ignore_ascii_case("newcomer-v1") {
            Self::NewcomerV1
        } else if name.eq_ignore_ascii_case("newcomer-v2") {
            Self::NewcomerV2
        } else {
            Self::Crystal
        }
    }

    pub const fn enables_diary_actions(self) -> bool {
        matches!(self, Self::NewcomerV1 | Self::NewcomerV2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestAction {
    Accept,
    Finish,
}

/// The adapter maps only recognized authoritative states to actionable values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestActionStatus {
    NotStarted,
    InProgress,
    ReadyToTurnIn,
    Other,
}

/// `action_offered` means the host checked an enabled matching operation in
/// the currently open NPC dialog. Template indices are not dialog object IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestActionSource {
    Diary,
    NpcDialog { action_offered: bool },
}

/// A missing status means that a quest is not in the current read model. The
/// existing NPC path may still accept an explicitly offered quest in that case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestActionFacts<'a> {
    pub status: Option<QuestActionStatus>,
    pub accept_npc_index: Option<u32>,
    pub finish_npc_index: Option<u32>,
    pub selectable_reward_indices: &'a [i32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestActionRequest<'a> {
    pub action: QuestAction,
    pub profile: QuestProfile,
    pub source: QuestActionSource,
    pub facts: QuestActionFacts<'a>,
    pub pending: bool,
    pub selected_reward_index: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestActionRejection {
    Pending,
    ProfileDisabled,
    MissingEndpoint,
    ActionUnavailable,
    WrongStatus,
    RewardSelectionRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestActionDecision {
    Eligible,
    Rejected(QuestActionRejection),
}

impl QuestActionDecision {
    pub const fn is_eligible(self) -> bool {
        matches!(self, Self::Eligible)
    }
}

/// Only an explicitly authored zero endpoint enables a Diary action. A zero
/// finish endpoint on a normal NPC quest can mean "return to start".
pub fn diary_endpoint_authorized(
    profile: QuestProfile,
    action: QuestAction,
    accept_npc_index: Option<u32>,
    finish_npc_index: Option<u32>,
) -> bool {
    profile.enables_diary_actions()
        && accept_npc_index == Some(0)
        && (action == QuestAction::Accept || finish_npc_index == Some(0))
}

/// Evaluate a UI affordance or revalidate the same action immediately before
/// enqueueing. No completion or reward is inferred from this decision.
pub fn evaluate_quest_action(request: QuestActionRequest<'_>) -> QuestActionDecision {
    use QuestAction::{Accept, Finish};
    use QuestActionDecision::{Eligible, Rejected};
    use QuestActionRejection::{
        ActionUnavailable, MissingEndpoint, Pending, ProfileDisabled, RewardSelectionRequired,
        WrongStatus,
    };

    if request.pending {
        return Rejected(Pending);
    }

    match request.source {
        QuestActionSource::Diary => {
            if !request.profile.enables_diary_actions() {
                return Rejected(ProfileDisabled);
            }
            if !diary_endpoint_authorized(
                request.profile,
                request.action,
                request.facts.accept_npc_index,
                request.facts.finish_npc_index,
            ) {
                return Rejected(MissingEndpoint);
            }
        }
        QuestActionSource::NpcDialog { action_offered } => {
            if !action_offered {
                return Rejected(ActionUnavailable);
            }
        }
    }

    let status_allows = match (request.action, request.facts.status, request.source) {
        (Accept, Some(QuestActionStatus::NotStarted), _) => true,
        (
            Accept,
            None,
            QuestActionSource::NpcDialog {
                action_offered: true,
            },
        ) => true,
        (Finish, Some(QuestActionStatus::ReadyToTurnIn), _) => true,
        _ => false,
    };
    if !status_allows {
        return Rejected(WrongStatus);
    }

    if request.action == Finish
        && !request.facts.selectable_reward_indices.is_empty()
        && !request.selected_reward_index.is_some_and(|selected| {
            selected >= 0 && request.facts.selectable_reward_indices.contains(&selected)
        })
    {
        return Rejected(RewardSelectionRequired);
    }
    Eligible
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts<'a>(status: Option<QuestActionStatus>, rewards: &'a [i32]) -> QuestActionFacts<'a> {
        QuestActionFacts {
            status,
            accept_npc_index: Some(0),
            finish_npc_index: Some(0),
            selectable_reward_indices: rewards,
        }
    }

    fn request<'a>(
        action: QuestAction,
        source: QuestActionSource,
        facts: QuestActionFacts<'a>,
    ) -> QuestActionRequest<'a> {
        QuestActionRequest {
            action,
            profile: QuestProfile::NewcomerV2,
            source,
            facts,
            pending: false,
            selected_reward_index: None,
        }
    }

    #[test]
    fn diary_requires_enabled_profile_and_explicit_zero_endpoints() {
        let diary = QuestActionSource::Diary;
        let accept = request(
            QuestAction::Accept,
            diary,
            facts(Some(QuestActionStatus::NotStarted), &[]),
        );
        assert_eq!(evaluate_quest_action(accept), QuestActionDecision::Eligible);
        assert_eq!(
            evaluate_quest_action(QuestActionRequest {
                profile: QuestProfile::Crystal,
                ..accept
            }),
            QuestActionDecision::Rejected(QuestActionRejection::ProfileDisabled)
        );
        assert_eq!(
            evaluate_quest_action(QuestActionRequest {
                facts: QuestActionFacts {
                    accept_npc_index: None,
                    ..accept.facts
                },
                ..accept
            }),
            QuestActionDecision::Rejected(QuestActionRejection::MissingEndpoint)
        );
        let finish = request(
            QuestAction::Finish,
            diary,
            facts(Some(QuestActionStatus::ReadyToTurnIn), &[]),
        );
        assert_eq!(evaluate_quest_action(finish), QuestActionDecision::Eligible);
        assert_eq!(
            evaluate_quest_action(QuestActionRequest {
                facts: QuestActionFacts {
                    finish_npc_index: None,
                    ..finish.facts
                },
                ..finish
            }),
            QuestActionDecision::Rejected(QuestActionRejection::MissingEndpoint)
        );
        assert_eq!(
            evaluate_quest_action(QuestActionRequest {
                facts: QuestActionFacts {
                    accept_npc_index: Some(42),
                    ..finish.facts
                },
                ..finish
            }),
            QuestActionDecision::Rejected(QuestActionRejection::MissingEndpoint)
        );
    }

    #[test]
    fn npc_dialog_uses_current_operation_evidence_without_template_object_id_match() {
        let npc = QuestActionSource::NpcDialog {
            action_offered: true,
        };
        let accept = request(
            QuestAction::Accept,
            npc,
            QuestActionFacts {
                accept_npc_index: None,
                finish_npc_index: None,
                ..facts(Some(QuestActionStatus::NotStarted), &[])
            },
        );
        assert_eq!(evaluate_quest_action(accept), QuestActionDecision::Eligible);
        assert_eq!(
            evaluate_quest_action(QuestActionRequest {
                facts: QuestActionFacts {
                    status: None,
                    ..accept.facts
                },
                ..accept
            }),
            QuestActionDecision::Eligible
        );
        assert_eq!(
            evaluate_quest_action(QuestActionRequest {
                source: QuestActionSource::NpcDialog {
                    action_offered: false
                },
                ..accept
            }),
            QuestActionDecision::Rejected(QuestActionRejection::ActionUnavailable)
        );
        assert_eq!(
            evaluate_quest_action(QuestActionRequest {
                facts: QuestActionFacts {
                    status: Some(QuestActionStatus::Other),
                    ..accept.facts
                },
                ..accept
            }),
            QuestActionDecision::Rejected(QuestActionRejection::WrongStatus)
        );
        assert_eq!(
            evaluate_quest_action(QuestActionRequest {
                facts: QuestActionFacts {
                    status: None,
                    ..accept.facts
                },
                action: QuestAction::Finish,
                ..accept
            }),
            QuestActionDecision::Rejected(QuestActionRejection::WrongStatus)
        );
    }

    #[test]
    fn finish_requires_valid_selected_reward_and_pending_blocks_resubmission() {
        let finish = request(
            QuestAction::Finish,
            QuestActionSource::Diary,
            facts(Some(QuestActionStatus::ReadyToTurnIn), &[2, 4]),
        );
        assert_eq!(
            evaluate_quest_action(finish),
            QuestActionDecision::Rejected(QuestActionRejection::RewardSelectionRequired)
        );
        assert_eq!(
            evaluate_quest_action(QuestActionRequest {
                selected_reward_index: Some(1),
                ..finish
            }),
            QuestActionDecision::Rejected(QuestActionRejection::RewardSelectionRequired)
        );
        assert_eq!(
            evaluate_quest_action(QuestActionRequest {
                selected_reward_index: Some(4),
                ..finish
            }),
            QuestActionDecision::Eligible
        );
        assert_eq!(
            evaluate_quest_action(QuestActionRequest {
                pending: true,
                selected_reward_index: Some(4),
                ..finish
            }),
            QuestActionDecision::Rejected(QuestActionRejection::Pending)
        );
    }

    #[test]
    fn profile_parser_fails_closed() {
        assert_eq!(
            QuestProfile::from_name("NEWCOMER-V1"),
            QuestProfile::NewcomerV1
        );
        assert_eq!(
            QuestProfile::from_name("newcomer-v2"),
            QuestProfile::NewcomerV2
        );
        assert_eq!(QuestProfile::from_name("unknown"), QuestProfile::Crystal);
    }
}
