//! Closed Android transport projection of the frozen Windows Hero requests.
//! Shared Hero UI/model own rules and pending state; a write is never an ACK.

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::android_input::{AndroidLifecycle, AndroidNetwork, AndroidShellState};
    use crate::gateway_bridge::{AndroidGatewayHostAdapter, AndroidGatewayHostWriteResult as R};
    use serde_json::{json, Value};
    use FrozenWindowsHeroCommand as C;

    // Exact Hero-used variants of the frozen Windows wire enum. Keeping a
    // test-only subset avoids pulling unrelated platform dependencies into Android.
    #[derive(Serialize)]
    #[serde(tag = "type", rename_all = "camelCase")]
    enum FrozenWindowsHeroCommand {
        MagicKey {
            #[serde(rename = "requestId")]
            request_id: u64,
            spell: String,
            key: u8,
            #[serde(rename = "oldKey")]
            old_key: u8,
        },
        ChangeHero {
            #[serde(rename = "listIndex")]
            list_index: i32,
        },
        SetHeroBehaviour {
            behaviour: u8,
        },
        SetAutoPotValue {
            stat: u8,
            value: u32,
        },
        SetAutoPotItem {
            grid: String,
            #[serde(rename = "itemIndex")]
            item_index: i32,
        },
        TransferHeroItem {
            from: i32,
            to: i32,
        },
        TakeBackHeroItem {
            from: i32,
            to: i32,
        },
        UseItem {
            #[serde(default, skip_serializing_if = "Option::is_none")]
            key: Option<String>,
            #[serde(rename = "uniqueId", default, skip_serializing_if = "Option::is_none")]
            unique_id: Option<u64>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            slot: Option<u8>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            grid: Option<String>,
        },
        MoveItem {
            grid: String,
            from: i32,
            to: i32,
        },
        EquipItem {
            #[serde(rename = "uniqueId")]
            unique_id: u64,
            grid: String,
            to: i32,
        },
        RemoveItem {
            #[serde(rename = "uniqueId")]
            unique_id: u64,
            grid: String,
            to: i32,
        },
        MergeItem {
            #[serde(rename = "gridFrom")]
            grid_from: String,
            #[serde(rename = "gridTo")]
            grid_to: String,
            #[serde(rename = "idFrom")]
            id_from: u64,
            #[serde(rename = "idTo")]
            id_to: u64,
        },
    }

    // Body copied from frozen Windows 3d735745f::hero_wire::command.
    // Only function/type aliases differ; source guard checks this independently.
    #[rustfmt::skip]
    fn frozen_windows_command(packet: &Packet) -> Option<C> {
        Some(match packet {
            Packet::MergeItem {
                grid_from,
                grid_to,
                id_from,
                id_to,
            } if matches!(grid_from, Grid::HeroInventory | Grid::HeroEquipment)
                || matches!(grid_to, Grid::HeroInventory | Grid::HeroEquipment) =>
            {
                C::MergeItem {
                    grid_from: format!("{grid_from:?}"),
                    grid_to: format!("{grid_to:?}"),
                    id_from: *id_from,
                    id_to: *id_to,
                }
            }
            Packet::ChangeHero { list_index } => C::ChangeHero {
                list_index: *list_index,
            },
            Packet::SetHeroBehaviour { behaviour } => C::SetHeroBehaviour {
                behaviour: *behaviour,
            },
            Packet::SetAutoPotValue { stat, value } => C::SetAutoPotValue {
                stat: *stat,
                value: *value,
            },
            Packet::SetAutoPotItem { grid, item_index }
                if matches!(grid, Grid::HeroHpItem | Grid::HeroMpItem) =>
            {
                C::SetAutoPotItem {
                    grid: format!("{grid:?}"),
                    item_index: *item_index,
                }
            }
            Packet::TransferHeroItem { from, to } => C::TransferHeroItem {
                from: *from,
                to: *to,
            },
            Packet::TakeBackHeroItem { from, to } => C::TakeBackHeroItem {
                from: *from,
                to: *to,
            },
            Packet::UseItem { grid, unique_id } if *grid == Grid::HeroInventory => {
                C::UseItem {
                    grid: Some("HeroInventory".into()),
                    unique_id: Some(*unique_id),
                    key: None,
                    slot: None,
                }
            }
            Packet::MoveItem { grid, from, to } if *grid == Grid::HeroInventory => {
                C::MoveItem {
                    grid: "HeroInventory".into(),
                    from: *from,
                    to: *to,
                }
            }
            Packet::EquipItem {
                grid,
                unique_id,
                to,
            } if *grid == Grid::HeroInventory => C::EquipItem {
                grid: "HeroInventory".into(),
                unique_id: *unique_id,
                to: *to,
            },
            Packet::RemoveItem {
                grid,
                unique_id,
                to,
            } if *grid == Grid::HeroInventory => C::RemoveItem {
                grid: "HeroInventory".into(),
                unique_id: *unique_id,
                to: *to,
            },
            _ => return None,
        })
    }

    pub(crate) fn fixture() -> (HeroAuthority, HeroModel, NativePlayerUiState) {
        let mut model = HeroModel {
            session_epoch: 7,
            ..Default::default()
        };
        assert!(model.apply_packet(
            "HeroInformation",
            &json!({"info":crate::hero_ingress::tests::info()})
        ));
        let authority = HeroAuthority {
            owner: 42,
            owner_name: "Fixture".into(),
            epoch: model.session_epoch,
            generation: model.hero_generation,
            actor: model.info.as_ref().unwrap().object_id,
        };
        let mut ui = NativePlayerUiState::default();
        ui.hero.observe(&model);
        (authority, model, ui)
    }

    fn item_intent(ui: &mut NativePlayerUiState) -> NativePlayerUiIntent {
        let packet = Packet::UseItem {
            grid: Grid::HeroInventory,
            unique_id: u64::MAX,
        };
        ui.hero.pending = Some(packet.clone());
        NativePlayerUiIntent::HeroPacket(packet)
    }

    pub(crate) fn key_intent(
        model: &HeroModel,
        ui: &mut NativePlayerUiState,
    ) -> NativePlayerUiIntent {
        ui.hero.assign.show(model, 0);
        ui.hero.assign.choose(18);
        let pending = ui.hero.assign.request(model).unwrap();
        let intent = NativePlayerUiIntent::MagicKey {
            request_id: pending.request_id,
            spell: format!("{:?}", pending.spell),
            key: pending.key,
            old_key: pending.old_key,
        };
        ui.hero.assign.queued(pending, true);
        intent
    }

    pub(crate) fn enqueue(
        state: &mut AndroidHeroEgressState,
        queue: &mut AndroidGatewayOutboundQueue,
        model: &HeroModel,
        ui: &NativePlayerUiState,
        intent: &NativePlayerUiIntent,
    ) -> u64 {
        let context =
            HeroDispatchContext::capture(state.authority.as_ref().unwrap(), intent, model, ui)
                .unwrap();
        let sequence = queue.enqueue_native_hero(&context.command()).unwrap();
        state.track(sequence, context);
        sequence
    }

    fn ready_state(authority: HeroAuthority) -> AndroidHeroEgressState {
        AndroidHeroEgressState {
            authority: Some(authority),
            ready: true,
            ..Default::default()
        }
    }

    #[test]
    fn hero_egress_all_frozen_packet_schemas_keep_exact_u64_and_grids() {
        let packets = [
            Packet::ChangeHero {
                list_index: i32::MAX,
            },
            Packet::SetHeroBehaviour { behaviour: u8::MAX },
            Packet::SetAutoPotValue {
                stat: u8::MAX,
                value: u32::MAX,
            },
            Packet::SetAutoPotItem {
                grid: Grid::HeroHpItem,
                item_index: 27,
            },
            Packet::SetAutoPotItem {
                grid: Grid::HeroMpItem,
                item_index: 17,
            },
            Packet::TransferHeroItem { from: -1, to: 41 },
            Packet::TakeBackHeroItem { from: 41, to: 0 },
            Packet::UseItem {
                grid: Grid::HeroInventory,
                unique_id: u64::MAX,
            },
            Packet::MoveItem {
                grid: Grid::HeroInventory,
                from: 3,
                to: 0,
            },
            Packet::EquipItem {
                grid: Grid::HeroInventory,
                unique_id: u64::MAX,
                to: 0,
            },
            Packet::RemoveItem {
                grid: Grid::HeroInventory,
                unique_id: u64::MAX,
                to: 6,
            },
            Packet::MergeItem {
                grid_from: Grid::Inventory,
                grid_to: Grid::HeroInventory,
                id_from: u64::MAX,
                id_to: u64::MAX - 1,
            },
            Packet::MergeItem {
                grid_from: Grid::HeroEquipment,
                grid_to: Grid::Inventory,
                id_from: u64::MAX,
                id_to: 70001,
            },
        ];
        for packet in packets {
            let actual = serde_json::to_value(packet_command(&packet).unwrap()).unwrap();
            let frozen = serde_json::to_value(frozen_windows_command(&packet).unwrap()).unwrap();
            assert_eq!(actual, frozen, "{packet:?}");
            assert!(actual.get("accountId").is_none() && actual.get("account_id").is_none());
            if actual["type"] == "useItem" {
                assert!(actual.get("slot").is_none() && actual.get("key").is_none());
            }
        }
    }

    #[test]
    fn hero_egress_rejects_player_auth_and_non_hero_packets() {
        for packet in [
            Packet::Login {
                account_id: "test-only".into(),
                password: "not-a-credential".into(),
            },
            Packet::StartGame { character_index: 0 },
            Packet::KeepAlive { time: 1 },
            Packet::UseItem {
                grid: Grid::Inventory,
                unique_id: 91,
            },
            Packet::MoveItem {
                grid: Grid::Inventory,
                from: 0,
                to: 1,
            },
            Packet::EquipItem {
                grid: Grid::Equipment,
                unique_id: 91,
                to: 0,
            },
            Packet::RemoveItem {
                grid: Grid::Equipment,
                unique_id: 91,
                to: 0,
            },
            Packet::SetAutoPotItem {
                grid: Grid::Inventory,
                item_index: 17,
            },
            Packet::MergeItem {
                grid_from: Grid::Inventory,
                grid_to: Grid::Equipment,
                id_from: 1,
                id_to: 2,
            },
        ] {
            assert!(packet_command(&packet).is_none());
            assert!(frozen_windows_command(&packet).is_none());
        }
    }

    #[test]
    fn hero_egress_selective_queue_preserves_player_keys_other_domains_and_two_fifos() {
        use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiIntentQueue;
        let mut queue = NativePlayerUiIntentQueue::default();
        let player = NativePlayerUiIntent::MagicKey {
            request_id: 1,
            spell: "FireBall".into(),
            key: 1,
            old_key: 0,
        };
        let noop = NativePlayerUiIntent::MagicKey {
            request_id: 2,
            spell: "FireBall".into(),
            key: 0,
            old_key: 0,
        };
        let hero_key = NativePlayerUiIntent::MagicKey {
            request_id: 3,
            spell: "FireBall".into(),
            key: 0,
            old_key: 17,
        };
        queue.push_intent(player.clone());
        for i in 0..20 {
            queue.push_intent(NativePlayerUiIntent::HeroPacket(Packet::ChangeHero {
                list_index: i,
            }));
        }
        queue.push_intent(noop.clone());
        queue.push_intent(NativePlayerUiIntent::TradeCancel);
        queue.push_intent(hero_key.clone());
        assert!(queue.drain_hero_intents_bounded(0).is_empty());
        let selected = queue.drain_hero_intents_bounded(16);
        assert_eq!(
            selected,
            (0..16)
                .map(|i| NativePlayerUiIntent::HeroPacket(Packet::ChangeHero { list_index: i }))
                .collect::<Vec<_>>()
        );
        let rest = queue.drain_hero_intents_bounded(16);
        let mut expected = (16..20)
            .map(|i| NativePlayerUiIntent::HeroPacket(Packet::ChangeHero { list_index: i }))
            .collect::<Vec<_>>();
        expected.push(hero_key);
        assert_eq!(rest, expected);
        assert_eq!(
            queue.drain_intents(),
            vec![player, noop, NativePlayerUiIntent::TradeCancel]
        );
    }

    #[test]
    fn hero_egress_sent_item_is_not_ack_or_custody_change() {
        let (authority, model, mut ui) = fixture();
        let original = serde_json::to_value(&model).unwrap();
        let intent = item_intent(&mut ui);
        let mut state = ready_state(authority);
        let mut queue = AndroidGatewayOutboundQueue::default();
        let sequence = enqueue(&mut state, &mut queue, &model, &ui, &intent);
        state.write_result(
            sequence,
            R::Sent,
            AndroidGatewayHostWriteOutcome::Sent,
            Some(&model),
            Some(&mut ui),
        );
        assert!(ui.hero.pending.is_some());
        assert_eq!(serde_json::to_value(&model).unwrap(), original);
        assert!(state.contexts.is_empty());
    }

    #[test]
    fn hero_egress_key_waits_for_actual_write_and_then_exact_shared_ack() {
        let (authority, model, mut ui) = fixture();
        let original = serde_json::to_value(&model).unwrap();
        let intent = key_intent(&model, &mut ui);
        let pending = ui.hero.assign.pending.clone();
        let mut state = ready_state(authority);
        let mut queue = AndroidGatewayOutboundQueue::default();
        let sequence = enqueue(&mut state, &mut queue, &model, &ui, &intent);
        assert!(ui.hero.assign.open && !ui.hero.assign.dispatched);
        state.write_result(
            sequence,
            R::Sent,
            AndroidGatewayHostWriteOutcome::Sent,
            Some(&model),
            Some(&mut ui),
        );
        assert!(!ui.hero.assign.open && ui.hero.assign.dispatched);
        assert_eq!(ui.hero.assign.pending, pending);
        assert_eq!(serde_json::to_value(&model).unwrap(), original);
        ui.hero.assign.reconcile(&model);
        assert_eq!(ui.hero.assign.pending, pending, "no synthetic ACK");
    }

    #[test]
    fn hero_egress_failed_matching_write_reopens_only_that_key_draft() {
        let (authority, model, mut ui) = fixture();
        let intent = key_intent(&model, &mut ui);
        let mut state = ready_state(authority);
        let mut queue = AndroidGatewayOutboundQueue::default();
        let sequence = enqueue(&mut state, &mut queue, &model, &ui, &intent);
        state.write_result(
            sequence,
            R::Failed,
            AndroidGatewayHostWriteOutcome::NonStorageFailureIgnored,
            Some(&model),
            Some(&mut ui),
        );
        assert!(
            ui.hero.assign.open && !ui.hero.assign.dispatched && ui.hero.assign.pending.is_none()
        );
        assert!(ui.hero.assign.notice.is_some());
    }

    #[test]
    fn hero_egress_failed_item_releases_matching_shared_pending_only() {
        let (authority, model, mut ui) = fixture();
        let intent = item_intent(&mut ui);
        let mut state = ready_state(authority);
        let mut queue = AndroidGatewayOutboundQueue::default();
        let sequence = enqueue(&mut state, &mut queue, &model, &ui, &intent);
        state.write_result(
            sequence,
            R::Failed,
            AndroidGatewayHostWriteOutcome::NonStorageFailureIgnored,
            Some(&model),
            Some(&mut ui),
        );
        assert!(ui.hero.pending.is_none());
        assert!(ui.hero.config_pending.is_none());
        assert_eq!(
            model.info.as_ref().unwrap().inventory.as_ref().unwrap()[3]
                .as_ref()
                .unwrap()
                .unique_id,
            u64::MAX
        );
    }

    #[test]
    fn hero_egress_late_failure_does_not_release_new_equal_domain_draft() {
        let (authority, model, mut ui) = fixture();
        let intent = key_intent(&model, &mut ui);
        let mut state = ready_state(authority);
        let mut queue = AndroidGatewayOutboundQueue::default();
        let sequence = enqueue(&mut state, &mut queue, &model, &ui, &intent);
        ui.hero.assign.pending.as_mut().unwrap().request_id += 1;
        let newer = ui.clone();
        state.write_result(
            sequence,
            R::Failed,
            AndroidGatewayHostWriteOutcome::NonStorageFailureIgnored,
            Some(&model),
            Some(&mut ui),
        );
        assert_eq!(ui, newer);
    }

    #[test]
    fn hero_egress_unknown_and_duplicate_lease_do_not_consume_current_pending() {
        let (authority, model, mut ui) = fixture();
        let intent = key_intent(&model, &mut ui);
        let mut state = ready_state(authority);
        let mut queue = AndroidGatewayOutboundQueue::default();
        let sequence = enqueue(&mut state, &mut queue, &model, &ui, &intent);
        let before = ui.clone();
        state.write_result(
            sequence,
            R::Sent,
            AndroidGatewayHostWriteOutcome::UnknownLease,
            Some(&model),
            Some(&mut ui),
        );
        assert_eq!(ui, before);
        assert_eq!(state.contexts.len(), 1);
        state.write_result(
            sequence,
            R::Sent,
            AndroidGatewayHostWriteOutcome::Sent,
            Some(&model),
            Some(&mut ui),
        );
        let sent = ui.clone();
        state.write_result(
            sequence,
            R::Failed,
            AndroidGatewayHostWriteOutcome::UnknownLease,
            Some(&model),
            Some(&mut ui),
        );
        assert_eq!(ui, sent);
    }

    #[test]
    fn hero_egress_authority_epoch_generation_actor_and_pending_are_required() {
        let (authority, model, mut ui) = fixture();
        let intent = item_intent(&mut ui);
        for field in 0..3 {
            let mut wrong = authority.clone();
            match field {
                0 => wrong.epoch += 1,
                1 => wrong.generation += 1,
                _ => wrong.actor += 1,
            }
            assert!(HeroDispatchContext::capture(&wrong, &intent, &model, &ui).is_none());
        }
        ui.hero.pending = None;
        assert!(HeroDispatchContext::capture(&authority, &intent, &model, &ui).is_none());
    }

    #[test]
    fn hero_egress_key_cannot_alias_player_or_send_none_none_or_forged_spell() {
        let (authority, model, mut ui) = fixture();
        let intent = key_intent(&model, &mut ui);
        let NativePlayerUiIntent::MagicKey { request_id, .. } = intent else {
            unreachable!()
        };
        for (key, old_key, spell) in [
            (1, 17, "FireBall"),
            (17, 8, "FireBall"),
            (0, 0, "FireBall"),
            (18, 17, "qa.giveItem"),
        ] {
            let wrong = NativePlayerUiIntent::MagicKey {
                request_id,
                key,
                old_key,
                spell: spell.into(),
            };
            assert!(HeroDispatchContext::capture(&authority, &wrong, &model, &ui).is_none());
        }
    }

    #[test]
    fn hero_egress_focus_render_deferral_preserves_other_domains_and_original_sequence() {
        let (authority, model, mut ui) = fixture();
        let intent = item_intent(&mut ui);
        let mut state = ready_state(authority);
        state.ready = false;
        let mut queue = AndroidGatewayOutboundQueue::default();
        let sequence = enqueue(&mut state, &mut queue, &model, &ui, &intent);
        queue
            .enqueue_native_social(
                &mir2_client_bevy::native_social_egress::NativeSocialCommand::TradeCancel,
            )
            .unwrap();
        let mut adapter = AndroidGatewayHostAdapter::default();
        let shell = AndroidShellState {
            lifecycle: AndroidLifecycle::Foreground,
            network: AndroidNetwork::Available,
            ..Default::default()
        };
        let deferred = state.prepare(&mut queue, Some(&model), Some(&mut ui));
        let other = adapter.drain_ready_with_hero_deferrals(&mut queue, &shell, 16, &deferred);
        assert_eq!(other.len(), 1);
        assert_eq!(
            serde_json::from_str::<Value>(&other[0].outbound().json).unwrap()["type"],
            "tradeCancel"
        );
        assert!(ui.hero.pending.is_some());
        state.ready = true;
        let deferred = state.prepare(&mut queue, Some(&model), Some(&mut ui));
        let hero = adapter.drain_ready_with_hero_deferrals(&mut queue, &shell, 16, &deferred);
        assert_eq!(hero.len(), 1);
        assert_eq!(hero[0].sequence(), sequence);
    }

    #[test]
    fn hero_egress_owner_change_discards_unsent_without_new_owner_ui_side_effects() {
        let (authority, model, mut ui) = fixture();
        let intent = item_intent(&mut ui);
        let mut state = ready_state(authority);
        let mut queue = AndroidGatewayOutboundQueue::default();
        let sequence = enqueue(&mut state, &mut queue, &model, &ui, &intent);
        state.authority.as_mut().unwrap().owner = 43;
        let before = ui.clone();
        state.prepare(&mut queue, Some(&model), Some(&mut ui));
        assert!(queue
            .drain_ready(
                &AndroidShellState {
                    lifecycle: AndroidLifecycle::Foreground,
                    network: AndroidNetwork::Available,
                    ..Default::default()
                },
                16
            )
            .is_empty());
        state.write_result(
            sequence,
            R::Sent,
            AndroidGatewayHostWriteOutcome::Sent,
            Some(&model),
            Some(&mut ui),
        );
        assert_eq!(ui, before);
    }

    #[test]
    fn hero_egress_sidecar_and_wire_are_bounded_and_reset_does_not_reuse_sequence() {
        let (authority, model, mut ui) = fixture();
        let intent = item_intent(&mut ui);
        let context = HeroDispatchContext::capture(&authority, &intent, &model, &ui).unwrap();
        let mut state = ready_state(authority);
        for id in 1..=MAX_CONTEXTS as u64 {
            state.track(id, context.clone());
        }
        assert!(!state.can_track());
        state.clear();
        assert!(state.can_track());
        let mut queue = AndroidGatewayOutboundQueue::with_capacity(1);
        let first = queue.enqueue_native_hero(&context.command()).unwrap();
        assert!(matches!(
            queue.enqueue_native_hero(&context.command()),
            Err(crate::gateway_bridge::AndroidGatewayEnqueueError::Full { .. })
        ));
        queue.mark_terminal_reset();
        let second = queue.enqueue_native_hero(&context.command()).unwrap();
        assert!(second > first);
        queue.mark_terminal_reset();
        assert!(matches!(
            queue.enqueue_native_hero(&NativeHeroCommand::MagicKey {
                request_id: 1,
                spell: "x".repeat(4096),
                key: 17,
                old_key: 0
            }),
            Err(crate::gateway_bridge::AndroidGatewayEnqueueError::OversizedNativeHero { .. })
        ));
        assert_eq!(queue.status().len, 0);
    }

    #[test]
    fn hero_egress_change_selection_can_start_without_hero_information() {
        let model = HeroModel {
            session_epoch: 7,
            ..Default::default()
        };
        let authority = HeroAuthority {
            owner: 42,
            owner_name: "Fixture".into(),
            epoch: 7,
            generation: 0,
            actor: 0,
        };
        let mut ui = NativePlayerUiState::default();
        ui.hero.observe(&model);
        let intent = NativePlayerUiIntent::HeroPacket(Packet::ChangeHero { list_index: 0 });
        assert!(HeroDispatchContext::capture(&authority, &intent, &model, &ui).is_some());
    }

    #[test]
    fn hero_egress_magic_key_exact_frozen_wire_and_full_request_id() {
        let (authority, model, mut ui) = fixture();
        key_intent(&model, &mut ui);
        ui.hero.assign.pending.as_mut().unwrap().request_id = u64::MAX;
        let intent = NativePlayerUiIntent::MagicKey {
            request_id: u64::MAX,
            spell: "FireBall".into(),
            key: 18,
            old_key: 17,
        };
        let context = HeroDispatchContext::capture(&authority, &intent, &model, &ui).unwrap();
        assert_eq!(
            serde_json::to_value(context.command()).unwrap(),
            serde_json::to_value(C::MagicKey {
                request_id: u64::MAX,
                spell: "FireBall".into(),
                key: 18,
                old_key: 17
            })
            .unwrap()
        );
    }

    #[test]
    fn hero_egress_late_equal_item_failure_after_receipt_cannot_release_new_draft() {
        let (authority, mut model, mut ui) = fixture();
        let intent = item_intent(&mut ui);
        let mut state = ready_state(authority);
        let mut queue = AndroidGatewayOutboundQueue::default();
        let sequence = enqueue(&mut state, &mut queue, &model, &ui, &intent);
        // The original reducer has processed a receipt and a new equal item
        // draft is now pending. Equality alone cannot identify the old send.
        model.item_result_serial += 1;
        ui.hero.seen_item_result = model.item_result_serial;
        let newer = ui.clone();
        state.write_result(
            sequence,
            R::Failed,
            AndroidGatewayHostWriteOutcome::NonStorageFailureIgnored,
            Some(&model),
            Some(&mut ui),
        );
        assert_eq!(ui, newer);
    }
}

use bevy::prelude::Resource;
use mir2_client_bevy::{
    crystal_ui::overlays::{
        hero_dialog, NativeHeroGrid as Grid, NativeHeroPacket as Packet, NativePlayerUiIntent,
        NativePlayerUiState,
    },
    hero_model::{HeroKeyPending, HeroModel},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

use crate::gateway_bridge::{
    AndroidGatewayHostWriteOutcome, AndroidGatewayHostWriteResult, AndroidGatewayOutboundQueue,
};

const MAX_CONTEXTS: usize = 256;

/// Only the public Hero subset. No auth, account id, debug command or raw JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NativeHeroCommand {
    ChangeHero {
        #[serde(rename = "listIndex")]
        list_index: i32,
    },
    SetHeroBehaviour {
        behaviour: u8,
    },
    SetAutoPotValue {
        stat: u8,
        value: u32,
    },
    SetAutoPotItem {
        grid: String,
        #[serde(rename = "itemIndex")]
        item_index: i32,
    },
    TransferHeroItem {
        from: i32,
        to: i32,
    },
    TakeBackHeroItem {
        from: i32,
        to: i32,
    },
    UseItem {
        grid: String,
        #[serde(rename = "uniqueId")]
        unique_id: u64,
    },
    MoveItem {
        grid: String,
        from: i32,
        to: i32,
    },
    EquipItem {
        grid: String,
        #[serde(rename = "uniqueId")]
        unique_id: u64,
        to: i32,
    },
    RemoveItem {
        grid: String,
        #[serde(rename = "uniqueId")]
        unique_id: u64,
        to: i32,
    },
    MergeItem {
        #[serde(rename = "gridFrom")]
        grid_from: String,
        #[serde(rename = "gridTo")]
        grid_to: String,
        #[serde(rename = "idFrom")]
        id_from: u64,
        #[serde(rename = "idTo")]
        id_to: u64,
    },
    MagicKey {
        #[serde(rename = "requestId")]
        request_id: u64,
        spell: String,
        key: u8,
        #[serde(rename = "oldKey")]
        old_key: u8,
    },
}

impl NativeHeroCommand {
    pub(crate) fn command_type(&self) -> &'static str {
        match self {
            Self::ChangeHero { .. } => "changeHero",
            Self::SetHeroBehaviour { .. } => "setHeroBehaviour",
            Self::SetAutoPotValue { .. } => "setAutoPotValue",
            Self::SetAutoPotItem { .. } => "setAutoPotItem",
            Self::TransferHeroItem { .. } => "transferHeroItem",
            Self::TakeBackHeroItem { .. } => "takeBackHeroItem",
            Self::UseItem { .. } => "useItem",
            Self::MoveItem { .. } => "moveItem",
            Self::EquipItem { .. } => "equipItem",
            Self::RemoveItem { .. } => "removeItem",
            Self::MergeItem { .. } => "mergeItem",
            Self::MagicKey { .. } => "magicKey",
        }
    }
}

/// Exact frozen hero_wire::command projection; not inventory/combat validation.
pub(crate) fn packet_command(packet: &Packet) -> Option<NativeHeroCommand> {
    use NativeHeroCommand as C;
    Some(match packet {
        Packet::MergeItem {
            grid_from,
            grid_to,
            id_from,
            id_to,
        } if matches!(grid_from, Grid::HeroInventory | Grid::HeroEquipment)
            || matches!(grid_to, Grid::HeroInventory | Grid::HeroEquipment) =>
        {
            C::MergeItem {
                grid_from: format!("{grid_from:?}"),
                grid_to: format!("{grid_to:?}"),
                id_from: *id_from,
                id_to: *id_to,
            }
        }
        Packet::ChangeHero { list_index } => C::ChangeHero {
            list_index: *list_index,
        },
        Packet::SetHeroBehaviour { behaviour } => C::SetHeroBehaviour {
            behaviour: *behaviour,
        },
        Packet::SetAutoPotValue { stat, value } => C::SetAutoPotValue {
            stat: *stat,
            value: *value,
        },
        Packet::SetAutoPotItem { grid, item_index }
            if matches!(grid, Grid::HeroHpItem | Grid::HeroMpItem) =>
        {
            C::SetAutoPotItem {
                grid: format!("{grid:?}"),
                item_index: *item_index,
            }
        }
        Packet::TransferHeroItem { from, to } => C::TransferHeroItem {
            from: *from,
            to: *to,
        },
        Packet::TakeBackHeroItem { from, to } => C::TakeBackHeroItem {
            from: *from,
            to: *to,
        },
        Packet::UseItem { grid, unique_id } if *grid == Grid::HeroInventory => C::UseItem {
            grid: "HeroInventory".into(),
            unique_id: *unique_id,
        },
        Packet::MoveItem { grid, from, to } if *grid == Grid::HeroInventory => C::MoveItem {
            grid: "HeroInventory".into(),
            from: *from,
            to: *to,
        },
        Packet::EquipItem {
            grid,
            unique_id,
            to,
        } if *grid == Grid::HeroInventory => C::EquipItem {
            grid: "HeroInventory".into(),
            unique_id: *unique_id,
            to: *to,
        },
        Packet::RemoveItem {
            grid,
            unique_id,
            to,
        } if *grid == Grid::HeroInventory => C::RemoveItem {
            grid: "HeroInventory".into(),
            unique_id: *unique_id,
            to: *to,
        },
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeroAuthority {
    pub(crate) owner: u32,
    pub(crate) owner_name: String,
    pub(crate) epoch: u64,
    pub(crate) generation: u64,
    pub(crate) actor: u32,
}

impl HeroAuthority {
    fn matches_model(&self, model: &HeroModel) -> bool {
        self.owner != 0
            && !self.owner_name.is_empty()
            && self.epoch != 0
            && self.epoch == model.session_epoch
            && self.generation == model.hero_generation
            && self.actor == model.info.as_ref().map_or(0, |info| info.object_id)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HeroDispatchContext {
    authority: HeroAuthority,
    intent: NativePlayerUiIntent,
    key_pending: Option<HeroKeyPending>,
    item_result_serial: u64,
}

impl HeroDispatchContext {
    pub(crate) fn capture(
        authority: &HeroAuthority,
        intent: &NativePlayerUiIntent,
        model: &HeroModel,
        ui: &NativePlayerUiState,
    ) -> Option<Self> {
        if !authority.matches_model(model)
            || ui.hero.epoch != authority.epoch
            || ui.hero.hero_generation != authority.generation
        {
            return None;
        }
        let key_pending = match intent {
            NativePlayerUiIntent::HeroPacket(packet) => {
                packet_command(packet)?;
                if !matches!(
                    packet,
                    Packet::ChangeHero { .. } | Packet::SetHeroBehaviour { .. }
                ) && ui.hero.pending.as_ref() != Some(packet)
                    && ui.hero.config_pending.as_ref() != Some(packet)
                {
                    return None;
                }
                None
            }
            NativePlayerUiIntent::MagicKey {
                request_id,
                spell,
                key,
                old_key,
            } => {
                let pending = ui.hero.assign.pending.as_ref()?;
                let hero_key = |key: u8| key == 0 || (17..=24).contains(&key);
                if *request_id == 0
                    || !hero_key(*key)
                    || !hero_key(*old_key)
                    || (*key == 0 && *old_key == 0)
                    || pending.request_id != *request_id
                    || format!("{:?}", pending.spell) != *spell
                    || pending.key != *key
                    || pending.old_key != *old_key
                    || pending.session_epoch != authority.epoch
                    || pending.hero_generation != authority.generation
                    || pending.object_id != authority.actor
                {
                    return None;
                }
                Some(pending.clone())
            }
            _ => return None,
        };
        Some(Self {
            authority: authority.clone(),
            intent: intent.clone(),
            key_pending,
            item_result_serial: model.item_result_serial,
        })
    }

    pub(crate) fn command(&self) -> NativeHeroCommand {
        match &self.intent {
            NativePlayerUiIntent::HeroPacket(packet) => {
                packet_command(packet).expect("captured typed Hero packet")
            }
            NativePlayerUiIntent::MagicKey {
                request_id,
                spell,
                key,
                old_key,
            } => NativeHeroCommand::MagicKey {
                request_id: *request_id,
                spell: spell.clone(),
                key: *key,
                old_key: *old_key,
            },
            _ => unreachable!("capture admits only Hero intents"),
        }
    }

    fn current(
        &self,
        authority: Option<&HeroAuthority>,
        model: &HeroModel,
        ui: &NativePlayerUiState,
    ) -> bool {
        authority == Some(&self.authority)
            && self.authority.matches_model(model)
            && ui.hero.epoch == self.authority.epoch
            && ui.hero.hero_generation == self.authority.generation
            && match &self.intent {
                NativePlayerUiIntent::HeroPacket(packet) => {
                    matches!(
                        packet,
                        Packet::ChangeHero { .. } | Packet::SetHeroBehaviour { .. }
                    ) || (model.item_result_serial == self.item_result_serial
                        && (ui.hero.pending.as_ref() == Some(packet)
                            || ui.hero.config_pending.as_ref() == Some(packet)))
                }
                NativePlayerUiIntent::MagicKey { .. } => {
                    ui.hero.assign.pending == self.key_pending
                        && self
                            .key_pending
                            .as_ref()
                            .is_some_and(|pending| pending.outcome(model).is_none())
                }
                _ => false,
            }
    }

    pub(crate) fn release_unsent(&self, ui: &mut NativePlayerUiState) {
        if ui.hero.epoch != self.authority.epoch
            || ui.hero.hero_generation != self.authority.generation
        {
            return;
        }
        match &self.intent {
            NativePlayerUiIntent::HeroPacket(packet) => {
                hero_dialog::host::release_unsent(&mut ui.hero, packet)
            }
            NativePlayerUiIntent::MagicKey {
                request_id,
                spell,
                key,
                old_key,
            } if ui.hero.assign.pending == self.key_pending => {
                ui.hero
                    .assign
                    .transport_result(*request_id, spell, *key, *old_key, false)
            }
            _ => {}
        }
    }
}

/// Bounded sidecar to the existing process-lifetime sequence/lease channel.
/// Authority is published from validated HostState before Update, not from JSON.
#[derive(Debug, Default, Resource)]
pub(crate) struct AndroidHeroEgressState {
    pub(crate) authority: Option<HeroAuthority>,
    pub(crate) ready: bool,
    contexts: BTreeMap<u64, HeroDispatchContext>,
}

#[derive(Default)]
pub(crate) struct HeroTransportPreparation {
    pub(crate) stale: BTreeSet<u64>,
    pub(crate) deferred: BTreeSet<u64>,
}

impl AndroidHeroEgressState {
    pub(crate) fn can_track(&self) -> bool {
        self.contexts.len() < MAX_CONTEXTS
    }
    pub(crate) fn track(&mut self, sequence: u64, context: HeroDispatchContext) {
        assert!(self.can_track() && !self.contexts.contains_key(&sequence));
        self.contexts.insert(sequence, context);
    }
    pub(crate) fn clear(&mut self) {
        self.contexts.clear();
    }
    pub(crate) fn prepare(
        &mut self,
        queue: &mut AndroidGatewayOutboundQueue,
        model: Option<&HeroModel>,
        ui: Option<&mut NativePlayerUiState>,
    ) -> BTreeSet<u64> {
        self.prepare_transport(queue, model, ui).deferred
    }
    /// Return the same exact fences for the Rust FIFO and the still-unpolled
    /// JNI FIFO. A copied/in-flight lease is not withdrawable or replayable.
    pub(crate) fn prepare_transport(
        &mut self,
        queue: &mut AndroidGatewayOutboundQueue,
        model: Option<&HeroModel>,
        ui: Option<&mut NativePlayerUiState>,
    ) -> HeroTransportPreparation {
        let mut preparation = HeroTransportPreparation::default();
        if self.authority.is_none() {
            let stale = self.contexts.keys().copied().collect();
            queue.discard_native_hero_sequences(&stale);
            preparation.stale = stale;
            self.clear();
        } else if let (Some(model), Some(ui)) = (model, ui) {
            let stale: BTreeSet<_> = self
                .contexts
                .iter()
                .filter_map(|(id, context)| {
                    (!context.current(self.authority.as_ref(), model, ui)).then_some(*id)
                })
                .collect();
            queue.discard_native_hero_sequences(&stale);
            for id in stale {
                preparation.stale.insert(id);
                self.contexts.remove(&id);
            }
            if self.ready {
                return preparation;
            }
        }
        preparation.deferred = self.contexts.keys().copied().collect();
        preparation
    }
    pub(crate) fn write_result(
        &mut self,
        sequence: u64,
        result: AndroidGatewayHostWriteResult,
        outcome: AndroidGatewayHostWriteOutcome,
        model: Option<&HeroModel>,
        ui: Option<&mut NativePlayerUiState>,
    ) {
        if outcome == AndroidGatewayHostWriteOutcome::UnknownLease {
            return;
        }
        let Some(context) = self.contexts.remove(&sequence) else {
            return;
        };
        let (Some(model), Some(ui)) = (model, ui) else {
            return;
        };
        if !context.current(self.authority.as_ref(), model, ui) {
            return;
        }
        match result {
            AndroidGatewayHostWriteResult::Failed => context.release_unsent(ui),
            AndroidGatewayHostWriteResult::Sent => {
                if let NativePlayerUiIntent::MagicKey {
                    request_id,
                    spell,
                    key,
                    old_key,
                } = &context.intent
                {
                    ui.hero
                        .assign
                        .transport_result(*request_id, spell, *key, *old_key, true);
                }
                // Shared item/config/key receipts alone settle authoritative state.
            }
        }
    }
}
