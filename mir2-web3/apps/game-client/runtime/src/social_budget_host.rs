//! JSON/instance boundary for the shared Native social transport budget.
use mir2_client_bevy::social_operation_budget::{
    SocialBudgetInvite, SocialBudgetOwner, SocialOperationBudget,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::cell::RefCell;

const MAX_INPUT: usize = 8 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
enum Input {
    Reset {
        version: u8,
        owner: SocialBudgetOwner,
    },
    Retire {
        version: u8,
        owner: SocialBudgetOwner,
    },
    SetViewer {
        version: u8,
        owner: SocialBudgetOwner,
        payload: Value,
    },
    Packet {
        version: u8,
        owner: SocialBudgetOwner,
        packet: String,
        payload: Value,
    },
    Snapshot {
        version: u8,
        owner: SocialBudgetOwner,
        snapshot: Value,
        #[serde(rename = "inventoryJson")]
        inventory_json: Value,
    },
    Reserve {
        version: u8,
        owner: SocialBudgetOwner,
        command: Value,
        #[serde(default)]
        invite: Option<SocialBudgetInvite>,
        #[serde(default, rename = "deadlineMs")]
        deadline_ms: Option<u64>,
    },
    Validate {
        version: u8,
        owner: SocialBudgetOwner,
        token: u64,
        command: Value,
    },
    Enter {
        version: u8,
        owner: SocialBudgetOwner,
        token: u64,
        command: Value,
    },
    CancelUnsent {
        version: u8,
        owner: SocialBudgetOwner,
        token: u64,
    },
    Expire {
        version: u8,
        owner: SocialBudgetOwner,
        #[serde(rename = "nowMs")]
        now_ms: u64,
    },
    Inspect {
        version: u8,
        owner: SocialBudgetOwner,
    },
}
impl Input {
    fn version(&self) -> u8 {
        match self {
            Self::Reset { version, .. }
            | Self::Retire { version, .. }
            | Self::SetViewer { version, .. }
            | Self::Packet { version, .. }
            | Self::Snapshot { version, .. }
            | Self::Reserve { version, .. }
            | Self::Validate { version, .. }
            | Self::Enter { version, .. }
            | Self::CancelUnsent { version, .. }
            | Self::Expire { version, .. }
            | Self::Inspect { version, .. } => *version,
        }
    }
}

#[derive(Default)]
struct Host {
    budget: SocialOperationBudget,
}
impl Host {
    fn process(&mut self, input: &str) -> String {
        let before: Vec<_> = self.budget.pending().iter().map(|row| row.token).collect();
        let result = if input.len() > MAX_INPUT {
            Err("inputTooLarge")
        } else {
            serde_json::from_str::<Input>(input)
                .map_err(|_| "invalidInput")
                .and_then(|input| self.run(input))
        };
        let pending = self.budget.pending();
        let released: Vec<_> = before
            .into_iter()
            .filter(|token| !pending.iter().any(|row| row.token == *token))
            .collect();
        let mut output = match result {
            Ok((handled, token, kind)) => {
                json!({"version":1,"ok":true,"handled":handled,"token":token,"kind":kind})
            }
            Err(error) => {
                json!({"version":1,"ok":false,"handled":true,"token":null,"kind":null,"error":error})
            }
        };
        output["pendingCount"] = json!(pending.len());
        output["pending"] = json!(pending);
        output["released"] = json!(released);
        output.to_string()
    }
    fn run(
        &mut self,
        input: Input,
    ) -> Result<(bool, Option<u64>, Option<&'static str>), &'static str> {
        if input.version() != 1 {
            return Err("unsupportedVersion");
        }
        let mut handled = true;
        match input {
            Input::Reset { owner, .. } => self.budget.reset(owner)?,
            Input::Retire { owner, .. } => self.budget.retire(&owner)?,
            Input::SetViewer { owner, payload, .. } => {
                handled = self.budget.packet(&owner, "UserInformation", &payload)?
            }
            Input::Packet {
                owner,
                packet,
                payload,
                ..
            } => handled = self.budget.packet(&owner, &packet, &payload)?,
            Input::Snapshot {
                owner,
                snapshot,
                inventory_json,
                ..
            } => handled = self.budget.snapshot(&owner, &snapshot, &inventory_json)?,
            Input::Reserve {
                owner,
                command,
                invite,
                deadline_ms,
                ..
            } => {
                return Ok(
                    match self
                        .budget
                        .reserve(&owner, command, invite.as_ref(), deadline_ms)?
                    {
                        Some(row) => (true, Some(row.token), Some(row.kind)),
                        None => (false, None, None),
                    },
                );
            }
            Input::Validate {
                owner,
                token,
                command,
                ..
            } => self.budget.validate(&owner, token, &command)?,
            Input::Enter {
                owner,
                token,
                command,
                ..
            } => self.budget.enter(&owner, token, &command)?,
            Input::CancelUnsent { owner, token, .. } => self.budget.cancel_unsent(&owner, token)?,
            Input::Expire { owner, now_ms, .. } => self.budget.expire(&owner, now_ms)?,
            Input::Inspect { owner, .. } => {
                if !self.budget.current(&owner) {
                    return Err("retiredOwner");
                }
            }
        }
        Ok((handled, None, None))
    }
}

thread_local! { static HOST: RefCell<Host> = RefCell::new(Host::default()); }
pub(super) fn process_json(input: &str) -> String {
    HOST.with(|host| match host.try_borrow_mut() {
        Ok(mut host) => host.process(input),
        Err(_) => {
            json!({"version":1,"ok":false,"handled":true,"error":"busy","token":null,"kind":null,
            "pendingCount":0,"pending":[],"released":[]})
            .to_string()
        }
    })
}

#[cfg(test)]
#[path = "social_budget_host_tests.rs"]
mod tests;
