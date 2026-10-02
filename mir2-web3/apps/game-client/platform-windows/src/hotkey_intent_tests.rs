use super::*;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonState, InputPlugin};
use bevy::prelude::{App, Entity, Vec2};
use bevy::time::Real;
use bevy::window::PrimaryWindow;
use mir2_client_bevy::inventory::ItemModel;
use std::sync::mpsc::Receiver;
use std::time::Duration;

fn hotkey_app(class: &str) -> (App, Receiver<GatewayCommand>, Entity) {
    let (mut app, receiver) = input_app();
    app.add_plugins(InputPlugin);
    app.init_resource::<NativePlayerUiState>();
    app.init_resource::<Time<Real>>();
    app.insert_resource(UiReadModel {
        player: mir2_client_bevy::read_model::PlayerStats {
            class_name: Some(class.into()),
            hp: 100,
            max_hp: 100,
            mp: 100,
            max_mp: 100,
            ..Default::default()
        },
        ..Default::default()
    });
    let window = app
        .world_mut()
        .spawn((stage_window(Vec2::new(480., 320.)), PrimaryWindow))
        .id();
    app.add_systems(bevy::prelude::Update, keyboard_skill_system);
    (app, receiver, window)
}

fn key_event(app: &mut App, window: Entity, code: KeyCode, pressed: bool, repeat: bool) {
    app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: match code {
            KeyCode::F1 => Key::F1,
            KeyCode::F2 => Key::F2,
            KeyCode::Digit1 | KeyCode::Numpad1 => Key::Character("1".into()),
            KeyCode::Digit2 | KeyCode::Numpad2 => Key::Character("2".into()),
            KeyCode::Digit3 | KeyCode::Numpad3 => Key::Character("3".into()),
            KeyCode::Digit4 | KeyCode::Numpad4 => Key::Character("4".into()),
            KeyCode::Digit5 | KeyCode::Numpad5 => Key::Character("5".into()),
            KeyCode::Digit6 | KeyCode::Numpad6 => Key::Character("6".into()),
            KeyCode::Digit7 | KeyCode::Numpad7 => Key::Character("7".into()),
            KeyCode::Digit8 | KeyCode::Numpad8 => Key::Character("8".into()),
            _ => unreachable!("key is outside this headless fixture"),
        },
        state: if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        },
        text: None,
        repeat,
        window,
    });
    app.update();
}

fn press(app: &mut App, window: Entity, key: KeyCode) {
    key_event(app, window, key, true, false);
}

fn release(app: &mut App, window: Entity, key: KeyCode) {
    key_event(app, window, key, false, false);
}

fn advance(app: &mut App, ms: u64) {
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_millis(ms));
}

fn skill(spell: &str, kind: &str, slot: i32, enabled: Option<bool>) -> serde_json::Value {
    serde_json::json!({
        "id": slot, "name": spell, "spell": spell, "hotkey": slot,
        "castKind": kind, "canUse": enabled, "mpCost": 20,
        "cooldownRemainingMs": 0
    })
}

fn set_skills(app: &mut App, entries: Vec<serde_json::Value>) {
    app.insert_resource(
        serde_json::from_value::<SkillModel>(serde_json::json!({
            "authority":{"sessionEpoch":1,"playerObjectId":1000,"snapshotSerial":1},
            "skills":entries
        }))
        .unwrap(),
    );
}

fn ack(app: &mut App, spell: &str, enabled: bool) {
    let mut model = app.world_mut().resource_mut::<SkillModel>();
    model.authority.snapshot_serial += 1;
    model
        .bindings
        .iter_mut()
        .find(|b| b.spell.as_deref() == Some(spell))
        .unwrap()
        .can_use = Some(enabled);
}

fn expect_toggle(receiver: &Receiver<GatewayCommand>, expected_spell: &str, expected_state: i8) {
    let GatewayCommand::Wire(command) = receiver.try_recv().expect("one toggle request") else {
        panic!("toggle escaped the wire command route");
    };
    assert_eq!(
        command.to_wire_json(),
        serde_json::json!({
            "type":"spellToggle", "spell":expected_spell, "toggleState":expected_state
        })
    );
    assert!(
        receiver.try_recv().is_err(),
        "one key must not send an unrelated mode change"
    );
}

fn belt_item(slot: u32, unique_id: Option<u64>) -> ItemModel {
    ItemModel {
        container: 1,
        slot,
        unique_id,
        quantity: 2,
        key: format!("potion-{slot}"),
        name: "Health Potion".into(),
        ..Default::default()
    }
}

#[test]
fn all_player_belt_keys_send_the_same_exact_item_identity_as_mouse_for_three_classes() {
    for class in ["Warrior", "Wizard", "Taoist"] {
        for (slot, keys) in [
            [KeyCode::Digit1, KeyCode::Numpad1],
            [KeyCode::Digit2, KeyCode::Numpad2],
            [KeyCode::Digit3, KeyCode::Numpad3],
            [KeyCode::Digit4, KeyCode::Numpad4],
            [KeyCode::Digit5, KeyCode::Numpad5],
            [KeyCode::Digit6, KeyCode::Numpad6],
        ]
        .into_iter()
        .enumerate()
        {
            for key in keys {
                let (mut app, receiver, window) = hotkey_app(class);
                // Include authoritative UID zero; every other UID is too large
                // to accidentally pass through the server's slot-as-UID fallback.
                let uid = if slot == 0 {
                    0
                } else {
                    (1u64 << 50) + slot as u64
                };
                let mut inventory = InventoryModel::default();
                inventory.items.push(belt_item(slot as u32, Some(uid)));
                inventory.items.push(ItemModel {
                    container: 0,
                    slot: slot as u32,
                    unique_id: Some(17),
                    key: "bag-decoy".into(),
                    ..Default::default()
                });
                let Some(NativePlayerUiIntent::UseItem {
                    key: mouse_key,
                    unique_id,
                    slot: mouse_slot,
                    grid,
                }) = belt_item_use_intent(&inventory, slot as u8)
                else {
                    panic!("mouse identity")
                };
                let mouse = NativeOutboundCommand::UseItem {
                    key: mouse_key,
                    unique_id,
                    slot: mouse_slot,
                    grid,
                };
                app.insert_resource(inventory);
                press(&mut app, window, key);
                let GatewayCommand::Wire(command) = receiver.try_recv().expect("belt wire request")
                else {
                    panic!("belt escaped the wire command route");
                };
                let wire = command.to_wire_json();
                assert_eq!(wire, mouse.to_wire_json(), "{class} {key:?}");
                assert_eq!(
                    wire["uniqueId"],
                    serde_json::json!(uid),
                    "slot number is not an item identity"
                );
                assert_eq!(wire["slot"], serde_json::json!(slot));
                assert_eq!(wire["grid"], "belt");
                assert!(receiver.try_recv().is_err());
            }
        }
    }
}

#[test]
fn empty_identityless_and_wrong_container_belt_slots_never_fall_back_to_slot_or_key() {
    for items in [
        vec![],
        vec![belt_item(0, None)],
        vec![belt_item(1, Some(800))],
        vec![ItemModel {
            container: 0,
            ..belt_item(0, Some(900))
        }],
    ] {
        for key in [KeyCode::Digit1, KeyCode::Numpad1] {
            let (mut app, receiver, window) = hotkey_app("Warrior");
            let mut inventory = InventoryModel::default();
            inventory.items = items.clone();
            app.insert_resource(inventory);
            press(&mut app, window, key);
            assert!(receiver.try_recv().is_err());
        }
    }
}

#[test]
fn hero_belt_seven_and_eight_never_consume_player_items() {
    for key in [
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Numpad7,
        KeyCode::Numpad8,
    ] {
        let (mut app, receiver, window) = hotkey_app("Warrior");
        let mut inventory = InventoryModel::default();
        inventory.items = (0..8)
            .map(|slot| belt_item(slot, Some(7000 + u64::from(slot))))
            .collect();
        app.insert_resource(inventory);
        press(&mut app, window, key);
        assert!(receiver.try_recv().is_err());
    }
}

#[test]
fn belt_shortcuts_honor_the_shared_item_clock_and_do_not_repeat_held_keys() {
    use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiIntentQueue;
    let (mut app, receiver, window) = hotkey_app("Taoist");
    let mut inventory = InventoryModel::default();
    inventory.items = vec![belt_item(0, Some(9000))];
    app.insert_resource(inventory);
    app.init_resource::<NativePlayerUiIntentQueue>();
    app.world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .commit_item_use(0, u64::MAX);
    press(&mut app, window, KeyCode::Digit1);
    assert!(receiver.try_recv().is_err());
    release(&mut app, window, KeyCode::Digit1);
    app.world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .commit_item_use(0, 0);
    press(&mut app, window, KeyCode::Numpad1);
    assert!(matches!(
        receiver.try_recv(),
        Ok(GatewayCommand::Wire(NativeOutboundCommand::UseItem {
            unique_id: Some(9000),
            ..
        }))
    ));
    key_event(&mut app, window, KeyCode::Numpad1, true, true);
    assert!(receiver.try_recv().is_err());
    assert!(
        !app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .push_intent(NativePlayerUiIntent::HeroPacket(
                mir2_protocol::ClientPacket::UseItem {
                    grid: mir2_protocol::MirGridType::HeroInventory,
                    unique_id: 77
                }
            )),
        "player hotkey must gate Hero item use too"
    );
}

#[test]
fn four_persistent_modes_repeatedly_toggle_at_one_second_despite_low_mp_and_cast_clock() {
    for spell in ["Thrusting", "HalfMoon", "CrossHalfMoon", "DoubleSlash"] {
        let (mut app, receiver, window) = hotkey_app("Warrior");
        let mut entry = skill(spell, "toggle", 1, Some(false));
        entry["cooldownRemainingMs"] = serde_json::json!(60_000);
        set_skills(&mut app, vec![entry]);
        app.world_mut().resource_mut::<UiReadModel>().player.mp = 0;
        press(&mut app, window, KeyCode::F1);
        expect_toggle(&receiver, spell, 1);
        key_event(&mut app, window, KeyCode::F1, true, true);
        assert!(
            receiver.try_recv().is_err(),
            "OS repeat must not flip a mode"
        );
        release(&mut app, window, KeyCode::F1);
        advance(&mut app, 900);
        ack(&mut app, spell, true);
        advance(&mut app, 99);
        press(&mut app, window, KeyCode::F1);
        assert!(
            receiver.try_recv().is_err(),
            "999ms is inside source ToggleTime"
        );
        release(&mut app, window, KeyCode::F1);
        advance(&mut app, 1);
        press(&mut app, window, KeyCode::F1);
        expect_toggle(&receiver, spell, 0);
        ack(&mut app, spell, false);
        release(&mut app, window, KeyCode::F1);
        advance(&mut app, 1000);
        press(&mut app, window, KeyCode::F1);
        expect_toggle(&receiver, spell, 1);
    }
}

#[test]
fn thrusting_and_half_moon_keep_independent_flags_and_share_only_toggle_time() {
    let (mut app, receiver, window) = hotkey_app("Warrior");
    set_skills(
        &mut app,
        vec![
            skill("Thrusting", "toggle", 1, Some(false)),
            skill("HalfMoon", "toggle", 2, Some(false)),
        ],
    );
    press(&mut app, window, KeyCode::F1);
    expect_toggle(&receiver, "Thrusting", 1);
    ack(&mut app, "Thrusting", true);
    release(&mut app, window, KeyCode::F1);
    advance(&mut app, 999);
    press(&mut app, window, KeyCode::F2);
    assert!(receiver.try_recv().is_err());
    release(&mut app, window, KeyCode::F2);
    advance(&mut app, 1);
    press(&mut app, window, KeyCode::F2);
    expect_toggle(&receiver, "HalfMoon", 1);
    ack(&mut app, "HalfMoon", true);
    release(&mut app, window, KeyCode::F2);
    advance(&mut app, 1000);
    press(&mut app, window, KeyCode::F1);
    expect_toggle(&receiver, "Thrusting", 0);
    assert_eq!(
        app.world().resource::<SkillModel>().bindings[1].can_use,
        Some(true)
    );
    ack(&mut app, "Thrusting", false);
    release(&mut app, window, KeyCode::F1);
    advance(&mut app, 1000);
    press(&mut app, window, KeyCode::F2);
    expect_toggle(&receiver, "HalfMoon", 0);
}

#[test]
fn duplicate_pointer_casts_and_authoritative_ack_do_not_restart_toggle_time() {
    let (mut app, receiver, _) = hotkey_app("Warrior");
    set_skills(&mut app, vec![skill("Thrusting", "toggle", 1, Some(false))]);
    for _ in 0..3 {
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .skill_bars
            .queue_cast(1);
    }
    app.update();
    expect_toggle(&receiver, "Thrusting", 1);
    advance(&mut app, 999);
    ack(&mut app, "Thrusting", true);
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .skill_bars
        .queue_cast(1);
    app.update();
    assert!(receiver.try_recv().is_err());
    advance(&mut app, 1);
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .skill_bars
        .queue_cast(1);
    app.update();
    expect_toggle(&receiver, "Thrusting", 0);
}

#[test]
fn preparations_always_arm_and_flame_and_twin_use_the_half_second_gate() {
    for spell in ["FlamingSword", "TwinDrakeBlade", "CounterAttack"] {
        for enabled in [None, Some(false), Some(true)] {
            let (mut app, receiver, window) = hotkey_app("Warrior");
            set_skills(&mut app, vec![skill(spell, "toggle", 1, enabled)]);
            press(&mut app, window, KeyCode::F1);
            expect_toggle(&receiver, spell, 1);
            release(&mut app, window, KeyCode::F1);
            if spell != "CounterAttack" {
                advance(&mut app, 499);
                press(&mut app, window, KeyCode::F1);
                assert!(receiver.try_recv().is_err());
                release(&mut app, window, KeyCode::F1);
                advance(&mut app, 1);
            }
            ack(&mut app, spell, true);
            press(&mut app, window, KeyCode::F1);
            expect_toggle(&receiver, spell, 1);
        }
    }
}

#[test]
fn preparations_keep_mp_and_authoritative_readiness_guards() {
    for spell in ["FlamingSword", "TwinDrakeBlade", "CounterAttack"] {
        for cooldown in [false, true] {
            let (mut app, receiver, window) = hotkey_app("Warrior");
            let mut entry = skill(spell, "toggle", 1, Some(false));
            if cooldown {
                entry["cooldownRemainingMs"] = serde_json::json!(60_000);
            } else {
                app.world_mut().resource_mut::<UiReadModel>().player.mp = 19;
            }
            set_skills(&mut app, vec![entry]);
            press(&mut app, window, KeyCode::F1);
            assert!(
                receiver.try_recv().is_err(),
                "{spell} preparation bypassed a cast guard"
            );
        }
    }
}

#[test]
fn shared_preparation_deadline_and_counter_attack_never_replace_the_attack_clock() {
    let (mut app, receiver, window) = hotkey_app("Warrior");
    set_skills(
        &mut app,
        vec![
            skill("FlamingSword", "toggle", 1, Some(true)),
            skill("Thrusting", "toggle", 2, Some(false)),
        ],
    );
    press(&mut app, window, KeyCode::F1);
    expect_toggle(&receiver, "FlamingSword", 1);
    release(&mut app, window, KeyCode::F1);
    advance(&mut app, 499);
    press(&mut app, window, KeyCode::F2);
    assert!(receiver.try_recv().is_err());
    release(&mut app, window, KeyCode::F2);
    advance(&mut app, 1);
    press(&mut app, window, KeyCode::F2);
    expect_toggle(&receiver, "Thrusting", 1);
    release(&mut app, window, KeyCode::F2);
    // CounterAttack's own readiness is independent of ToggleTime, as in Crystal.
    {
        let mut model = app.world_mut().resource_mut::<SkillModel>();
        model.bindings[0].spell = Some("CounterAttack".into());
        model.authority.snapshot_serial += 1;
    }
    press(&mut app, window, KeyCode::F1);
    expect_toggle(&receiver, "CounterAttack", 1);
}

#[test]
fn three_class_normal_casts_keep_mp_cooldown_and_dead_guards() {
    for (class, spell, kind) in [
        ("Warrior", "ShoulderDash", "direction"),
        ("Wizard", "FireBall", "target"),
        ("Taoist", "SoulFireBall", "target"),
    ] {
        for gate in ["none", "mp", "cooldown", "dead"] {
            let (mut app, receiver, window) = hotkey_app(class);
            let mut entry = skill(spell, kind, 1, Some(true));
            match gate {
                "mp" => app.world_mut().resource_mut::<UiReadModel>().player.mp = 19,
                "cooldown" => entry["cooldownRemainingMs"] = serde_json::json!(60_000),
                "dead" => app.world_mut().resource_mut::<UiReadModel>().player.hp = 0,
                _ => {}
            }
            set_skills(&mut app, vec![entry]);
            press(&mut app, window, KeyCode::F1);
            if gate == "none" {
                assert!(
                    matches!(receiver.try_recv(), Ok(GatewayCommand::Wire(
                    NativeOutboundCommand::Magic { spell: sent, object_id: 1000, .. }
                )) if sent == spell),
                    "{class} ordinary cast broke"
                );
            } else {
                assert!(
                    receiver.try_recv().is_err(),
                    "{class} {gate} no longer blocks casting"
                );
            }
        }
    }
}

#[test]
fn input_capture_focus_and_modal_gates_still_own_belt_and_weapon_hotkeys() {
    for key in [KeyCode::Digit1, KeyCode::Numpad1, KeyCode::F1] {
        for gate in [
            "chat",
            "assignment",
            "capture",
            "keyboard",
            "npc",
            "focus",
            "screen",
        ] {
            let (mut app, receiver, window) = hotkey_app("Warrior");
            set_skills(&mut app, vec![skill("Thrusting", "toggle", 1, Some(false))]);
            let mut inventory = InventoryModel::default();
            inventory.items = vec![belt_item(0, Some(4200))];
            app.insert_resource(inventory);
            match gate {
                "chat" => {
                    app.world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .core
                        .chat_focused = true
                }
                "assignment" => {
                    app.world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .skill_assign
                        .open = true
                }
                "capture" => {
                    app.world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .keyboard
                        .input_consumed = true
                }
                "keyboard" => {
                    app.world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .keyboard
                        .open = true
                }
                "npc" => {
                    app.insert_resource(NpcDialogModel {
                        is_open: true,
                        ..Default::default()
                    });
                }
                "focus" => app.world_mut().get_mut::<Window>(window).unwrap().focused = false,
                "screen" => {
                    app.world_mut().resource_mut::<NativeShellModel>().screen =
                        NativeShellScreen::Login
                }
                _ => unreachable!(),
            }
            press(&mut app, window, key);
            assert!(receiver.try_recv().is_err(), "{gate} leaked {key:?}");
        }
    }
}

#[test]
fn passive_slaying_and_unlearned_slots_never_emit_a_toggle() {
    for entries in [vec![], vec![skill("Slaying", "passive", 1, Some(true))]] {
        let (mut app, receiver, window) = hotkey_app("Warrior");
        set_skills(&mut app, entries);
        press(&mut app, window, KeyCode::F1);
        assert!(receiver.try_recv().is_err());
    }
}

#[test]
fn persistent_modes_bypass_only_mp_and_cast_readiness_while_dead_players_stay_blocked() {
    for spell in ["Thrusting", "HalfMoon", "CrossHalfMoon", "DoubleSlash"] {
        let (mut app, receiver, window) = hotkey_app("Warrior");
        let mut entry = skill(spell, "toggle", 1, Some(true));
        entry["cooldownRemainingMs"] = serde_json::json!(60_000);
        set_skills(&mut app, vec![entry]);
        app.world_mut().resource_mut::<UiReadModel>().player.hp = 0;
        app.world_mut().resource_mut::<UiReadModel>().player.mp = 0;
        press(&mut app, window, KeyCode::F1);
        assert!(receiver.try_recv().is_err(), "dead player sent {spell}");
    }
}

#[test]
fn changing_authoritative_actor_resets_only_the_local_toggle_debounce() {
    let (mut app, receiver, window) = hotkey_app("Warrior");
    set_skills(&mut app, vec![skill("Thrusting", "toggle", 1, Some(false))]);
    press(&mut app, window, KeyCode::F1);
    expect_toggle(&receiver, "Thrusting", 1);
    release(&mut app, window, KeyCode::F1);
    app.world_mut()
        .resource_mut::<SkillModel>()
        .authority
        .session_epoch += 1;
    press(&mut app, window, KeyCode::F1);
    expect_toggle(&receiver, "Thrusting", 1);
}

#[test]
fn a_failed_toggle_send_does_not_consume_the_next_press_deadline() {
    let (mut app, receiver, window) = hotkey_app("Warrior");
    set_skills(&mut app, vec![skill("Thrusting", "toggle", 1, Some(false))]);
    drop(receiver);
    press(&mut app, window, KeyCode::F1);
    release(&mut app, window, KeyCode::F1);
    let (sender, receiver) = std::sync::mpsc::channel();
    app.insert_resource(GatewayCommands::new(sender));
    press(&mut app, window, KeyCode::F1);
    expect_toggle(&receiver, "Thrusting", 1);
}
