//! On-chain command injection (M4, WF-5) — the trusted seam from the Relayer to live
//! player sessions (DESIGN §4).
//!
//! The Relayer (`onchain/relayer`) POSTs chain-confirmed, normalized commands to the
//! gateway's `/onchain/inject` route. A live `GatewaySession` is owned task-locally inside
//! the per-socket loop, so there is no shared handle an HTTP handler can mutate directly.
//! `LiveSessionInjector` bridges that: each socket task registers an mpsc sender keyed by
//! its authenticated `account_id`; the HTTP handler looks the account up and hands the
//! command to that task (which exclusively owns `&mut GatewaySession` + the WS socket),
//! which applies it authoritatively and pushes the resulting packets.
//!
//! Server stays authoritative: commands are applied only via `execute_with_outcome`
//! (Direct mode) on chain-confirmed events. Idempotency place #3 lives in the sim (the
//! command's `idempotency_key` makes a duplicate a no-op), so the gateway is stateless here.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use mir2_simulation::WorldCommand;

/// A command handed to a live session task, with a oneshot channel to report the receipt.
#[derive(Debug)]
pub struct InjectionMessage {
    pub command: WorldCommand,
    pub reply: oneshot::Sender<InjectionOutcome>,
}

/// Receipt for an applied injection.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectionOutcome {
    /// Packets the command produced. 0 = idempotent no-op (already applied) or no effect.
    pub packet_count: usize,
}

/// Why an injection could not be delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionError {
    /// No live session is registered for the target account (player offline).
    NotConnected,
    /// The session task dropped the reply before responding.
    SessionGone,
}

/// Process-local registry: `account_id` -> the live socket task's command sender.
#[derive(Clone, Default)]
pub struct LiveSessionInjector {
    inner: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<InjectionMessage>>>>,
}

impl LiveSessionInjector {
    pub fn register(&self, account_id: &str, tx: mpsc::UnboundedSender<InjectionMessage>) {
        self.inner
            .lock()
            .expect("live session injector mutex should not be poisoned")
            .insert(account_id.to_string(), tx);
    }

    pub fn unregister(&self, account_id: &str) {
        self.inner
            .lock()
            .expect("live session injector mutex should not be poisoned")
            .remove(account_id);
    }

    #[cfg(test)]
    pub fn is_connected(&self, account_id: &str) -> bool {
        self.inner
            .lock()
            .expect("live session injector mutex should not be poisoned")
            .contains_key(account_id)
    }

    /// Deliver `command` to the account's live session and await its receipt.
    pub async fn dispatch(
        &self,
        account_id: &str,
        command: WorldCommand,
    ) -> Result<InjectionOutcome, InjectionError> {
        let tx = {
            let map = self
                .inner
                .lock()
                .expect("live session injector mutex should not be poisoned");
            map.get(account_id).cloned()
        };
        let tx = tx.ok_or(InjectionError::NotConnected)?;
        let (reply, reply_rx) = oneshot::channel();
        tx.send(InjectionMessage { command, reply })
            .map_err(|_| InjectionError::NotConnected)?;
        reply_rx.await.map_err(|_| InjectionError::SessionGone)
    }
}

/// RAII registration: registers on construction, unregisters on drop, so a socket task is
/// removed from the injector on EVERY exit path (the loop has many early returns).
pub struct InjectionRegistration {
    injector: LiveSessionInjector,
    account_id: String,
}

impl InjectionRegistration {
    pub fn new(
        injector: LiveSessionInjector,
        account_id: &str,
        tx: mpsc::UnboundedSender<InjectionMessage>,
    ) -> Self {
        injector.register(account_id, tx);
        Self {
            injector,
            account_id: account_id.to_string(),
        }
    }
}

impl Drop for InjectionRegistration {
    fn drop(&mut self) {
        self.injector.unregister(&self.account_id);
    }
}

/// The Relayer's normalized command JSON (matches `onchain/relayer/src/types.ts`).
/// Externally tagged on `kind`; fields are camelCase to match the relayer payload.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase")]
pub enum OnchainInjectCommand {
    GrantOnchainOre {
        account: String,
        ore_kind: String,
        amount: String,
        mine_id: String,
        #[serde(default)]
        stones_left: u32,
        idempotency_key: String,
    },
    CreditGoldFromOre {
        account: String,
        ore_amount: String,
        idempotency_key: String,
    },
    /// Render-only depletion hint — the sim has no WorldCommand for it (the GrantOnchainOre
    /// carries stones_left). The gateway accepts and ignores it. (`mineId` in the payload is
    /// ignored — serde drops unknown fields.)
    MineDepleted { idempotency_key: String },
}

impl OnchainInjectCommand {
    pub fn account(&self) -> Option<&str> {
        match self {
            Self::GrantOnchainOre { account, .. } | Self::CreditGoldFromOre { account, .. } => {
                Some(account)
            }
            Self::MineDepleted { .. } => None,
        }
    }

    pub fn idempotency_key(&self) -> &str {
        match self {
            Self::GrantOnchainOre {
                idempotency_key, ..
            }
            | Self::CreditGoldFromOre {
                idempotency_key, ..
            }
            | Self::MineDepleted {
                idempotency_key, ..
            } => idempotency_key,
        }
    }

    /// Convert to a sim `WorldCommand`. `Ok(None)` for render-only commands the sim does not
    /// model (MineDepleted) — the caller 200-accepts and ignores them.
    pub fn to_world_command(&self) -> Result<Option<WorldCommand>, String> {
        Ok(match self {
            Self::GrantOnchainOre {
                account,
                ore_kind,
                amount,
                mine_id,
                stones_left,
                idempotency_key,
                ..
            } => Some(WorldCommand::GrantOnchainOre {
                account: account.clone(),
                ore_kind: ore_kind.clone(),
                amount: amount.parse().map_err(|_| "invalid amount".to_string())?,
                mine_id: mine_id.parse().map_err(|_| "invalid mineId".to_string())?,
                stones_left: *stones_left,
                idempotency_key: idempotency_key.clone(),
            }),
            Self::CreditGoldFromOre {
                account,
                ore_amount,
                idempotency_key,
                ..
            } => {
                // M4 PLACEHOLDER: gold = oreAmount (1:1). The real ore->gold rate is an M5
                // economic decision (ROADMAP §8/§11) — applied here once the admin sets it.
                let gold = ore_amount
                    .parse::<u64>()
                    .map_err(|_| "invalid oreAmount".to_string())?
                    .min(u64::from(u32::MAX)) as u32;
                Some(WorldCommand::CreditGoldFromOre {
                    account: account.clone(),
                    gold,
                    idempotency_key: idempotency_key.clone(),
                })
            }
            Self::MineDepleted { .. } => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_grant_onchain_ore_and_maps_to_world_command() {
        let json = r#"{"kind":"GrantOnchainOre","account":"sui:0xminer","oreKind":"BlackIron",
            "amount":"5","mineId":"1","stonesLeft":5,"nonce":"1","idempotencyKey":"TX1:4"}"#;
        let cmd: OnchainInjectCommand = serde_json::from_str(json).expect("parse");
        assert_eq!(cmd.account(), Some("sui:0xminer"));
        assert_eq!(cmd.idempotency_key(), "TX1:4");
        match cmd.to_world_command().expect("map") {
            Some(WorldCommand::GrantOnchainOre {
                account,
                ore_kind,
                amount,
                mine_id,
                stones_left,
                idempotency_key,
            }) => {
                assert_eq!(account, "sui:0xminer");
                assert_eq!(ore_kind, "BlackIron");
                assert_eq!(amount, 5);
                assert_eq!(mine_id, 1);
                assert_eq!(stones_left, 5);
                assert_eq!(idempotency_key, "TX1:4");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn credit_gold_uses_ore_amount_as_placeholder_rate() {
        let json = r#"{"kind":"CreditGoldFromOre","account":"sui:0xa","oreKind":"BlackIron",
            "oreAmount":"3","idempotencyKey":"TX2:5"}"#;
        let cmd: OnchainInjectCommand = serde_json::from_str(json).expect("parse");
        match cmd.to_world_command().expect("map") {
            Some(WorldCommand::CreditGoldFromOre { gold, .. }) => assert_eq!(gold, 3),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn mine_depleted_is_accepted_but_not_a_world_command() {
        let json = r#"{"kind":"MineDepleted","mineId":"1","idempotencyKey":"TX3:6"}"#;
        let cmd: OnchainInjectCommand = serde_json::from_str(json).expect("parse");
        assert!(cmd.to_world_command().expect("map").is_none());
        assert_eq!(cmd.account(), None);
    }

    #[tokio::test]
    async fn dispatch_without_a_registered_session_is_not_connected() {
        let injector = LiveSessionInjector::default();
        assert!(!injector.is_connected("sui:0xabsent"));
        let result = injector
            .dispatch(
                "sui:0xabsent",
                WorldCommand::CreditGoldFromOre {
                    account: "sui:0xabsent".to_string(),
                    gold: 1,
                    idempotency_key: "k".to_string(),
                },
            )
            .await;
        assert_eq!(result.unwrap_err(), InjectionError::NotConnected);
    }

    #[tokio::test]
    async fn dispatch_routes_to_the_registered_session_and_returns_the_receipt() {
        let injector = LiveSessionInjector::default();
        let (tx, mut rx) = mpsc::unbounded_channel::<InjectionMessage>();
        let _reg = InjectionRegistration::new(injector.clone(), "sui:0xminer", tx);
        assert!(injector.is_connected("sui:0xminer"));

        // Simulate the live session task: receive, "apply", reply with a receipt.
        let task = tokio::spawn(async move {
            let msg = rx.recv().await.expect("message");
            assert!(matches!(msg.command, WorldCommand::GrantOnchainOre { .. }));
            let _ = msg.reply.send(InjectionOutcome { packet_count: 1 });
        });

        let outcome = injector
            .dispatch(
                "sui:0xminer",
                WorldCommand::GrantOnchainOre {
                    account: "sui:0xminer".to_string(),
                    ore_kind: "BlackIron".to_string(),
                    amount: 5,
                    mine_id: 1,
                    stones_left: 5,
                    idempotency_key: "TX1:4".to_string(),
                },
            )
            .await
            .expect("dispatch ok");
        assert_eq!(outcome.packet_count, 1);
        task.await.expect("task");
    }

    #[test]
    fn registration_guard_unregisters_on_drop() {
        let injector = LiveSessionInjector::default();
        let (tx, _rx) = mpsc::unbounded_channel::<InjectionMessage>();
        {
            let _reg = InjectionRegistration::new(injector.clone(), "sui:0xguard", tx);
            assert!(injector.is_connected("sui:0xguard"));
        }
        assert!(!injector.is_connected("sui:0xguard"));
    }
}
