//! Explicit, owned real-Gateway witness. No renderer, fabricated ACK, or QA API.
use super::*;
use crate::gameplay_bridge::{drain_gameplay_events, GameplayEventInbox};
use bevy::prelude::*;
use mir2_client_bevy::{
    crystal_ui::notice::NoticeDialogState,
    entities::{EntityKind, EntityModel, EntityModelSet},
    native_shell::{NativeShellModel, NativeShellScreen},
    pending_operations::{AuthoritativeModelRevisions, PendingOperations},
    quest_model::{
        CombatTargetModel, CompletedQuestTracker, GroundPickupModel, NearbyNpcModel,
        NpcDialogModel, QuestTracker,
    },
};
use std::{io::Read, path::PathBuf, sync::Mutex};

#[derive(Deserialize)]
struct OwnedRequest {
    url: String,
    account: String,
    password: String,
    name: String,
    class: String,
    map_gzip: PathBuf,
    report: PathBuf,
    attack_ack: PathBuf,
}

struct ControlledCommands {
    receiver: std::sync::mpsc::Receiver<GatewayCommand>,
    open: Arc<Mutex<bool>>,
    observed: Arc<Mutex<Vec<String>>>,
}
impl CommandSource for ControlledCommands {
    fn try_command(&mut self) -> Result<GatewayCommand, std::sync::mpsc::TryRecvError> {
        let gate = self.open.lock().unwrap();
        if !*gate {
            return Err(std::sync::mpsc::TryRecvError::Empty);
        }
        let command = self.receiver.try_recv()?;
        let label = match &command {
            GatewayCommand::Player(PlayerIntent::Walk { direction }) => {
                Some(format!("walk:{direction}"))
            }
            GatewayCommand::Player(PlayerIntent::Run { direction }) => {
                Some(format!("run:{direction}"))
            }
            GatewayCommand::Wire(NativeOutboundCommand::Attack { object_id }) => {
                Some(format!("attack:{object_id}"))
            }
            _ => None,
        };
        if let Some(label) = label {
            self.observed.lock().unwrap().push(label);
        }
        Ok(command)
    }
}

fn shell_until(
    receiver: &std::sync::mpsc::Receiver<ShellGatewayEvent>,
    predicate: impl Fn(&ShellGatewayEvent) -> bool,
) -> Result<ShellGatewayEvent, String> {
    let deadline = Instant::now() + Duration::from_secs(12);
    while Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(event) if predicate(&event) => return Ok(event),
            Ok(
                ShellGatewayEvent::LoginFailure { message }
                | ShellGatewayEvent::AccountCreationFailed { message }
                | ShellGatewayEvent::OperationFailure { message },
            ) => return Err(message),
            Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return Err("native shell channel closed".into()),
        }
    }
    Err("ordinary native lifecycle reply timed out".into())
}

fn actual_ack_until(
    receiver: &std::sync::mpsc::Receiver<NativeGameplaySnapshot>,
    forward: &std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    timeout: Duration,
    predicate: impl Fn(&NativeSelfMovementAck) -> bool,
) -> Result<NativeSelfMovementAck, String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(snapshot) => {
                let ack = snapshot.authoritative_self_movement.clone();
                forward
                    .send(snapshot)
                    .map_err(|_| "real gameplay consumer closed")?;
                if let Some(ack) = ack.filter(&predicate) {
                    return Ok(ack);
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return Err("actual native gameplay channel closed".into()),
        }
    }
    Err("command received no matching real UserLocation".into())
}

fn headless_input_app(
    sender: std::sync::mpsc::Sender<GatewayCommand>,
    receiver: std::sync::mpsc::Receiver<NativeGameplaySnapshot>,
    player: EntityModel,
) -> App {
    let mut app = App::new();
    app.insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(ButtonInput::<MouseButton>::default())
        .insert_resource(crate::input::GatewayCommands::new(sender))
        .insert_resource(GameplayEventInbox::new(receiver))
        .insert_resource(Time::<()>::default())
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        })
        .insert_resource(EntityModelSet {
            entities: vec![player],
        })
        .init_resource::<crate::input::WorldPointerMovementState>()
        .init_resource::<crate::entity_presentation::NativeEntityPresentation>()
        .init_resource::<crate::entity_overlays::NativeEntityOverlays>()
        .init_resource::<crate::effects::NativeEffects>()
        .init_resource::<QuestTracker>()
        .init_resource::<CompletedQuestTracker>()
        .init_resource::<NpcDialogModel>()
        .init_resource::<NearbyNpcModel>()
        .init_resource::<CombatTargetModel>()
        .init_resource::<GroundPickupModel>()
        .init_resource::<NoticeDialogState>()
        .init_resource::<AuthoritativeModelRevisions>()
        .init_resource::<PendingOperations>();
    app.world_mut().spawn(Window::default());
    app.add_systems(
        Update,
        (
            drain_gameplay_events,
            crate::input::mouse_world_interaction_system,
        )
            .chain(),
    );
    app
}

fn ensure(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

#[test]
#[ignore = "requires explicit loopback Gateway and fresh owned report over stdin"]
fn native_rejected_target_actual_gateway_round_trip() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    let request: OwnedRequest = serde_json::from_str(&input).expect("owned request");
    assert!(request.url.starts_with("ws://127.0.0.1:") && request.url.ends_with("/ws"));
    assert!(request.report.is_absolute() && !request.report.exists());
    assert!(request.map_gzip.is_absolute());
    let mut map_bytes = Vec::new();
    flate2::read::GzDecoder::new(std::fs::File::open(&request.map_gzip).unwrap())
        .read_to_end(&mut map_bytes)
        .unwrap();
    let map = crate::map_parser::parse_type100_map(&map_bytes)
        .or_else(|| crate::map_parser::parse_type1_map(&map_bytes))
        .expect("original Bichon collision");
    let (sender, commands) = std::sync::mpsc::channel();
    assert!(request.attack_ack.is_absolute() && !request.attack_ack.exists());
    let open = Arc::new(Mutex::new(true));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let controlled = ControlledCommands {
        receiver: commands,
        open: open.clone(),
        observed: observed.clone(),
    };
    let (shell_tx, shell_rx) = std::sync::mpsc::channel();
    let (gameplay_tx, gameplay_rx) = std::sync::mpsc::channel();
    let (forward_tx, forward_rx) = std::sync::mpsc::channel();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let url = request.url.clone();
    let worker = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(run_gateway_client_with_world_ingest(
            &url,
            controlled,
            shell_tx,
            gameplay_tx,
            NativeReconnectConfig::default(),
            |_| true,
        ));
        let _ = done_tx.send(result);
    });
    let mut admitted = false;
    let mut report = json!({"class":request.class, "rendererStarted":false,
        "humanAcceptance":false, "realGatewayAndNativeClient":true});
    let result = (|| -> Result<(), String> {
        report["phase"] = json!("ordinaryAccountLifecycle");
        shell_until(&shell_rx, |e| matches!(e, ShellGatewayEvent::Connected))?;
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::NewAccount {
                account_id: request.account.clone(),
                password: request.password.clone(),
                birth_date_binary: 0,
                user_name: String::new(),
                secret_question: String::new(),
                secret_answer: String::new(),
                email_address: String::new(),
            }))
            .map_err(|_| "registration enqueue")?;
        shell_until(&shell_rx, |e| {
            matches!(e, ShellGatewayEvent::AccountCreated)
        })?;
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::Login {
                account_id: request.account.clone(),
                password: request.password.clone(),
            }))
            .map_err(|_| "login enqueue")?;
        shell_until(&shell_rx, |e| {
            matches!(e, ShellGatewayEvent::LoginSuccess { .. })
        })?;
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::NewCharacter {
                name: request.name.clone(),
                gender: "Male".into(),
                class: request.class.clone(),
            }))
            .map_err(|_| "new-character enqueue")?;
        let ShellGatewayEvent::CharacterCreated { character } = shell_until(&shell_rx, |e| {
            matches!(e, ShellGatewayEvent::CharacterCreated { .. })
        })?
        else {
            unreachable!()
        };
        ensure(
            character.class_name.eq_ignore_ascii_case(&request.class),
            "ordinary created character must have the requested class",
        )?;
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::StartGame {
                character_index: character.index,
            }))
            .map_err(|_| "StartGame enqueue")?;
        shell_until(&shell_rx, |e| {
            matches!(e, ShellGatewayEvent::PlayerBootstrapped { .. })
        })?;
        admitted = true;
        // Two ordered real turns provide a marker newer than every bootstrap
        // location. A setup packet cannot satisfy the later rejected attack.
        report["phase"] = json!("bootstrapLeftMarker");
        let bootstrap_at = Instant::now();
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::Turn {
                direction: "left".into(),
            }))
            .map_err(|_| "first marker enqueue")?;
        actual_ack_until(&gameplay_rx, &forward_tx, Duration::from_secs(20), |a| {
            a.direction.eq_ignore_ascii_case("left")
        })?;
        report["bootstrapLeftMarkerMs"] = json!(bootstrap_at.elapsed().as_millis());
        report["phase"] = json!("bootstrapRightMarker");
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::Turn {
                direction: "right".into(),
            }))
            .map_err(|_| "second marker enqueue")?;
        let origin = actual_ack_until(&gameplay_rx, &forward_tx, Duration::from_secs(20), |a| {
            a.direction.eq_ignore_ascii_case("right")
        })?;
        report["bootstrapMarkersMs"] = json!(bootstrap_at.elapsed().as_millis());
        report["actionAckDeadlineMs"] = json!(5000);
        report["phase"] = json!("nativePendingAndRejectedTarget");
        // Consume every real setup projection before accepting local movement.
        // The recorder supplies a later command/ACK watermark independently.
        let setup_quiet = Instant::now() + Duration::from_millis(200);
        while Instant::now() < setup_quiet {
            if let Ok(snapshot) = gameplay_rx.recv_timeout(Duration::from_millis(20)) {
                forward_tx.send(snapshot).map_err(|_| "setup consumer")?;
            }
        }
        let directions = [
            ("down", 0, 1),
            ("right", 1, 0),
            ("up", 0, -1),
            ("left", -1, 0),
        ];
        let safe: Vec<_> = directions
            .into_iter()
            .filter(|(_, dx, dy)| !map.cell_blocks_movement(origin.x + dx, origin.y + dy))
            .collect();
        ensure(
            safe.len() >= 2,
            "ordinary spawn needs two source-legal neighbors",
        )?;
        let mut app = headless_input_app(
            sender.clone(),
            forward_rx,
            EntityModel {
                object_id: origin.object_id.clone(),
                kind: EntityKind::SelfPlayer,
                name: request.name.clone(),
                x: origin.x,
                y: origin.y,
                level: Some(u32::from(character.level)),
                direction: Some(origin.direction.clone()),
            },
        );
        app.update();
        // A normal new account receives the real welcome Notice packet. Use
        // the same public model action as CrystalNoticeAction::Ok; retaining
        // the modal input gate here is part of the headless fixture contract.
        let welcome_open = app.world().resource::<NoticeDialogState>().is_open();
        report["welcomeNoticeReceived"] = json!(welcome_open);
        if welcome_open {
            ensure(
                app.world_mut().resource_mut::<NoticeDialogState>().close(),
                "the ordinary Notice OK model action must dismiss the welcome modal",
            )?;
        }
        report["welcomeNoticeDismissedThroughOkModel"] = json!(welcome_open);
        let safe: Vec<_> =
            safe.into_iter()
                .filter(|(_, dx, dy)| {
                    app.world()
                        .get_resource::<crate::gameplay_bridge::NativeWorldClickState>()
                        .is_none_or(|state| {
                            !state.targets.values().any(|target| {
                                target.x == origin.x + dx && target.y == origin.y + dy
                            })
                        })
                })
                .collect();
        ensure(
            safe.len() >= 2,
            "ordinary spawn needs two currently unoccupied legal neighbors",
        )?;
        let old = safe[0];
        let fresh = safe[1];
        let cursor_for = |dx: i32, dy: i32| {
            (
                ((10 + dx * 3) * 48 + 24) as f32,
                ((11 + dy * 3) * 32 + 16) as f32,
            )
        };
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(1_000));
        *open.lock().unwrap() = false;
        app.world_mut()
            .resource_mut::<crate::entity_presentation::NativeEntityPresentation>()
            .set_hover_grid_context_for_test((origin.x, origin.y), cursor_for(old.1, old.2));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        report["pressGates"] = json!({
            "npcDialog":app.world().resource::<NpcDialogModel>().is_open,
            "notice":app.world().resource::<NoticeDialogState>().is_open(),
            "shell":format!("{:?}", app.world().resource::<NativeShellModel>().screen),
            "controllerBefore":format!("{:?}", app.world().resource::<crate::input::WorldPointerMovementState>()),
        });
        app.update();
        report["controllerAfterPress"] = json!(format!(
            "{:?}",
            app.world()
                .resource::<crate::input::WorldPointerMovementState>()
        ));
        report["selfEntitiesAfterPress"] = json!(app
            .world()
            .resource::<EntityModelSet>()
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::SelfPlayer)
            .map(|entity| json!({"id":entity.object_id,"x":entity.x,"y":entity.y}))
            .collect::<Vec<_>>());
        let accepted_pending = app
            .world()
            .resource::<crate::input::WorldPointerMovementState>()
            .pending_move_for_test();
        report["acceptedPendingBeforeSocketWrite"] = json!(accepted_pending);
        ensure(
            accepted_pending
                == Some((
                    old.0,
                    (origin.x, origin.y),
                    (origin.x + old.1, origin.y + old.2),
                )),
            "normal right press must accept the expected unsent walk before the socket gate opens",
        )?;
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::Attack {
                object_id: 260_917,
            }))
            .map_err(|_| "rejected attack enqueue")?;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Right);
        app.world_mut()
            .resource_mut::<crate::entity_presentation::NativeEntityPresentation>()
            .set_hover_grid_context_for_test((origin.x, origin.y), cursor_for(fresh.1, fresh.2));
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(600));
        app.update();
        let rejected_at = Instant::now();
        *open.lock().unwrap() = true;
        let witness_deadline = Instant::now() + Duration::from_secs(5);
        let witness: Value = loop {
            if let Some(witness) = std::fs::read(&request.attack_ack)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            {
                break witness;
            }
            ensure(
                Instant::now() < witness_deadline,
                "recorder saw no real command-following UserLocation",
            )?;
            std::thread::sleep(Duration::from_millis(10));
        };
        ensure(
            witness["ackSequence"].as_u64() > witness["commandSequence"].as_u64(),
            "the real correction must follow the attack socket write",
        )?;
        let rejected = actual_ack_until(&gameplay_rx, &forward_tx, Duration::from_secs(5), |a| {
            a.packet == "UserLocation"
                && a.object_id == origin.object_id
                && a.x == origin.x
                && a.y == origin.y
                && a.direction == origin.direction
                && witness["x"] == a.x
                && witness["y"] == a.y
                && witness["direction"] == a.direction
        })?;
        report["wireCommandSequence"] = witness["commandSequence"].clone();
        report["wireAckSequence"] = witness["ackSequence"].clone();
        report["rejectedAckMs"] = json!(rejected_at.elapsed().as_millis());
        report["origin"] = json!([origin.x, origin.y]);
        report["phase"] = json!("freshEscapeAfterRealCorrection");
        app.update();
        let accepted_before_guard = observed.lock().unwrap().clone();
        report["producerCommandsAtRealCorrection"] = json!(accepted_before_guard);
        report["pendingAtRealCorrection"] = json!(app
            .world()
            .resource::<crate::input::WorldPointerMovementState>()
            .pending_move_for_test());
        ensure(
            accepted_before_guard == vec![format!("walk:{}", old.0), "attack:260917".into()],
            "one producer-accepted movement must remain pending until the real rejected-action ACK",
        )?;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(399));
        app.update();
        ensure(
            *observed.lock().unwrap() == accepted_before_guard,
            "the correction guard must remain intact",
        )?;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(1));
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Right);
        let moved = actual_ack_until(&gameplay_rx, &forward_tx, Duration::from_secs(5), |a| {
            a.packet == "UserLocation"
                && a.object_id == origin.object_id
                && a.x == origin.x + fresh.1
                && a.y == origin.y + fresh.2
        })?;
        app.update();
        ensure(
            observed.lock().unwrap().last() == Some(&format!("walk:{}", fresh.0)),
            "the actual native sender must send fresh movement after the guard",
        )?;
        report["phase"] = json!("releasedQuietWindow");
        let quiet_until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < quiet_until {
            if let Ok(snapshot) = gameplay_rx.recv_timeout(Duration::from_millis(20)) {
                if let Some(ack) = &snapshot.authoritative_self_movement {
                    ensure(
                        ack.x == moved.x && ack.y == moved.y,
                        "a discarded movement must not execute later",
                    )?;
                }
                forward_tx.send(snapshot).map_err(|_| "quiet consumer")?;
            }
            app.update();
        }
        report["freshDirection"] = json!(fresh.0);
        report["destination"] = json!([moved.x, moved.y]);
        report["producerCommands"] = json!(*observed.lock().unwrap());
        report["guard399MsPreserved"] = json!(true);
        report["quiet5Seconds"] = json!(true);
        report["status"] = json!("passed");
        report["phase"] = json!("actionPassedBeforeNormalLogout");
        let _ = rejected;
        Ok(())
    })();
    *open.lock().unwrap() = true;
    let cleanup = if admitted {
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::LogOut))
            .unwrap();
        shell_until(&shell_rx, |e| {
            matches!(e, ShellGatewayEvent::LoggedOut { .. })
        })
        .map(|_| ())
    } else {
        Ok(())
    };
    report["normalLogoutAcknowledged"] = json!(admitted && cleanup.is_ok());
    if let Err(error) = &result {
        report["status"] = json!("failed");
        report["error"] = json!(error);
    }
    if let Err(error) = &cleanup {
        report["cleanupError"] = json!(error);
    }
    let _ = sender.send(GatewayCommand::Shutdown);
    let stopped = done_rx.recv_timeout(Duration::from_secs(10));
    if stopped.is_ok() {
        worker.join().unwrap();
    }
    report["nativeNetworkOwnerStopped"] = json!(matches!(stopped, Ok(Ok(()))));
    report["status"] = json!(
        if result.is_ok() && cleanup.is_ok() && matches!(stopped, Ok(Ok(()))) {
            "passed"
        } else {
            "failed"
        }
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&request.report)
        .unwrap();
    std::io::Write::write_all(&mut file, &serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    assert!(result.is_ok(), "{}", result.unwrap_err());
    assert!(
        cleanup.is_ok(),
        "normal logout must save the owned character"
    );
    assert!(
        matches!(stopped, Ok(Ok(()))),
        "owned native network task must stop normally"
    );
}
