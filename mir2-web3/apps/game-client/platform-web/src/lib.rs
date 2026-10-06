//! Browser JSON/wasm-bindgen edge for renderer-independent client policies.
//!
//! This small module also serves the compatibility renderer on touch devices.
//! Importing it must not pull in Bevy, windowing, assets or server rules.

use mir2_client_core::quest::{
    evaluate_quest_action, QuestAction, QuestActionDecision, QuestActionFacts,
    QuestActionRejection, QuestActionRequest, QuestActionSource, QuestActionStatus, QuestProfile,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

pub mod equipment_pending;
pub mod auth_ui;
pub mod mail_compose;
pub mod mail_parcel;
pub mod npc_gold_buy_attempt;

#[wasm_bindgen]
pub fn client_core_abi_version() -> u32 {
    1
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Action {
    Accept,
    Finish,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Status {
    NotStarted,
    InProgress,
    ReadyToTurnIn,
    Other,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Input {
    action: Action,
    profile: String,
    status: Option<Status>,
    accept_npc_index: Option<u32>,
    finish_npc_index: Option<u32>,
    selectable_reward_indices: Vec<i32>,
    selected_reward_index: Option<i32>,
    dialog_npc_index: Option<u32>,
    dialog_action_offered: bool,
    pending: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Decision {
    eligible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    npc_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rejection: Option<&'static str>,
}

impl Decision {
    fn eligible(source: &'static str, npc_index: u32) -> Self {
        Self {
            eligible: true,
            source: Some(source),
            npc_index: Some(npc_index),
            rejection: None,
        }
    }

    fn rejected(reason: &'static str) -> Self {
        Self {
            eligible: false,
            source: None,
            npc_index: None,
            rejection: Some(reason),
        }
    }
}

fn reject(reason: QuestActionRejection) -> Decision {
    Decision::rejected(match reason {
        QuestActionRejection::Pending => "pending",
        QuestActionRejection::ProfileDisabled => "profileDisabled",
        QuestActionRejection::MissingEndpoint => "missingEndpoint",
        QuestActionRejection::ActionUnavailable => "actionUnavailable",
        QuestActionRejection::WrongStatus => "wrongStatus",
        QuestActionRejection::RewardSelectionRequired => "rewardSelectionRequired",
    })
}

fn resolve(input: Input) -> Decision {
    if input.selectable_reward_indices.len() > 128
        || input
            .selectable_reward_indices
            .iter()
            .any(|index| *index < 0)
    {
        return Decision::rejected("invalidInput");
    }
    let request = QuestActionRequest {
        action: match input.action {
            Action::Accept => QuestAction::Accept,
            Action::Finish => QuestAction::Finish,
        },
        profile: QuestProfile::from_name(&input.profile),
        source: QuestActionSource::Diary,
        facts: QuestActionFacts {
            status: input.status.map(|status| match status {
                Status::NotStarted => QuestActionStatus::NotStarted,
                Status::InProgress => QuestActionStatus::InProgress,
                Status::ReadyToTurnIn => QuestActionStatus::ReadyToTurnIn,
                Status::Other => QuestActionStatus::Other,
            }),
            accept_npc_index: input.accept_npc_index,
            finish_npc_index: input.finish_npc_index,
            selectable_reward_indices: &input.selectable_reward_indices,
        },
        pending: input.pending,
        selected_reward_index: input.selected_reward_index,
    };
    match evaluate_quest_action(request) {
        QuestActionDecision::Eligible => return Decision::eligible("diary", 0),
        // Do not route around a valid Diary action's stage, pending or reward
        // guard merely because another dialog is open.
        QuestActionDecision::Rejected(
            reason @ (QuestActionRejection::Pending
            | QuestActionRejection::WrongStatus
            | QuestActionRejection::RewardSelectionRequired),
        ) => return reject(reason),
        QuestActionDecision::Rejected(_) => {}
    }
    let npc = input.dialog_npc_index.filter(|id| *id > 0);
    let request = QuestActionRequest {
        source: QuestActionSource::NpcDialog {
            action_offered: input.dialog_action_offered && npc.is_some(),
        },
        ..request
    };
    match evaluate_quest_action(request) {
        QuestActionDecision::Eligible => {
            Decision::eligible("npcDialog", npc.expect("validated NPC"))
        }
        QuestActionDecision::Rejected(reason) => reject(reason),
    }
}

#[wasm_bindgen]
pub fn resolve_quest_action(input_json: &str) -> String {
    let decision = if input_json.len() > 16_384 {
        Decision::rejected("invalidInput")
    } else {
        serde_json::from_str::<Input>(input_json)
            .map(resolve)
            .unwrap_or_else(|_| Decision::rejected("invalidInput"))
    };
    serde_json::to_string(&decision).expect("finite decision serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn input() -> Value {
        json!({"action":"accept","profile":"newcomer-v2","status":"notStarted",
            "acceptNpcIndex":0,"finishNpcIndex":0,"selectableRewardIndices":[],
            "dialogActionOffered":false,"pending":false})
    }

    fn decide(input: Value) -> Value {
        serde_json::from_str(&resolve_quest_action(&input.to_string())).unwrap()
    }

    #[test]
    fn diary_roundtrip_uses_the_shared_policy() {
        assert_eq!(
            decide(input()),
            json!({"eligible":true,"source":"diary","npcIndex":0})
        );
        let mut finish = input();
        finish["action"] = json!("finish");
        finish["status"] = json!("readyToTurnIn");
        assert!(decide(finish.clone())["eligible"].as_bool().unwrap());
        finish.as_object_mut().unwrap().remove("finishNpcIndex");
        assert_eq!(decide(finish)["eligible"], false);
    }

    #[test]
    fn malformed_endpoints_and_unknown_states_fail_closed() {
        for endpoint in [
            json!(null),
            json!("0"),
            json!(-1),
            json!(0.5),
            json!(4294967296_u64),
        ] {
            let mut request = input();
            request["acceptNpcIndex"] = endpoint;
            assert_eq!(decide(request)["eligible"], false);
        }
        let mut request = input();
        request["status"] = json!("success");
        assert_eq!(decide(request)["rejection"], "invalidInput");
        assert_eq!(
            resolve_quest_action("not json"),
            r#"{"eligible":false,"rejection":"invalidInput"}"#
        );
    }

    #[test]
    fn real_npc_dialog_is_independent_of_template_index() {
        let mut request = input();
        request["profile"] = json!("crystal");
        request["acceptNpcIndex"] = json!(24);
        request["dialogNpcIndex"] = json!(10024);
        request["dialogActionOffered"] = json!(true);
        assert_eq!(
            decide(request.clone()),
            json!({"eligible":true,"source":"npcDialog","npcIndex":10024})
        );
        request["dialogActionOffered"] = json!(false);
        assert_eq!(decide(request)["eligible"], false);
    }

    #[test]
    fn pending_and_reward_rejection_cannot_fall_through_to_a_dialog() {
        let mut request = input();
        request["dialogNpcIndex"] = json!(7);
        request["dialogActionOffered"] = json!(true);
        request["pending"] = json!(true);
        assert_eq!(decide(request.clone())["rejection"], "pending");
        request["pending"] = json!(false);
        request["action"] = json!("finish");
        request["status"] = json!("readyToTurnIn");
        request["selectableRewardIndices"] = json!([2, 4]);
        assert_eq!(
            decide(request.clone())["rejection"],
            "rewardSelectionRequired"
        );
        request["selectedRewardIndex"] = json!(4);
        assert_eq!(decide(request)["source"], "diary");
    }
}
