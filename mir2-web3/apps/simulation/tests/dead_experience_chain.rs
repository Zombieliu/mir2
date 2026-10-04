//! Public trusted personal-session lifecycle fences. These are save/load and
//! retained-bootstrap fixtures; the actual poison/monster/death/EXP chain is
//! separately exercised through ordinary authenticated Gateway packets.
use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};
use mir2_simulation::{
    CharacterBindPoint, InProcessWorldRuntime, SimulationConfig, WorldCommand, WorldEntityKind,
    WorldRuntime,
};

fn start(runtime: &mut InProcessWorldRuntime) {
    let login = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        }))
        .unwrap();
    assert!(login
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    let started = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: 0,
        }))
        .unwrap();
    assert!(started
        .iter()
        .any(|p| matches!(p, ServerPacket::UserInformation { .. })));
}

fn own_dead(runtime: &InProcessWorldRuntime) -> bool {
    runtime
        .world_snapshot()
        .entities
        .iter()
        .find(|e| e.kind == WorldEntityKind::SelfPlayer)
        .unwrap()
        .dead
}

#[test]
fn ordinary_positive_vitals_and_retained_bootstrap_do_not_revive_online_dead() {
    let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());
    start(&mut runtime);
    let max_hp = runtime.world_snapshot().player_max_hp.unwrap();
    runtime.force_authoritative_player_vitals(Some(0), Some(0));
    assert!(own_dead(&runtime));
    runtime.force_authoritative_player_vitals(Some(max_hp), Some(0));
    assert!(
        own_dead(&runtime),
        "a trusted pool refill is still not an explicit Revive"
    );
    let before = runtime.active_character_checkpoint().unwrap();
    runtime
        .restore_active_character_checkpoint(&before)
        .unwrap();
    assert!(
        own_dead(&runtime),
        "same-session save checkpoint must preserve transient life"
    );
    runtime
        .execute(WorldCommand::ReplayRetainedStartGameBootstrap { character_index: 0 })
        .unwrap();
    assert!(
        own_dead(&runtime),
        "retained runtime bootstrap must not run new-login life reset"
    );
    let revived = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::TownRevive))
        .unwrap();
    assert_eq!(
        revived
            .iter()
            .filter(|p| matches!(p, ServerPacket::Revived))
            .count(),
        1
    );
    assert!(!own_dead(&runtime));
}

#[test]
fn genuine_new_login_from_hp_zero_revives_full_at_bind() {
    let config = SimulationConfig::default();
    let mut runtime = InProcessWorldRuntime::new(config.clone());
    start(&mut runtime);
    let bind = Point { x: 334, y: 259 };
    let mut save = runtime.active_character_checkpoint().unwrap();
    save.position = Point { x: 337, y: 270 };
    save.bind_point = Some(CharacterBindPoint {
        map_file_name: "0".into(),
        position: bind.clone(),
    });
    runtime.restore_active_character_checkpoint(&save).unwrap();
    runtime.force_authoritative_player_vitals(Some(0), Some(0));
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    let mut login = InProcessWorldRuntime::new(config);
    start(&mut login);
    let after = login.world_snapshot();
    assert_eq!(
        after.player_hp, after.player_max_hp,
        "Crystal Load HP0 refills HP"
    );
    assert_eq!(after.player_mp, after.player_max_mp);
    let self_player = after
        .entities
        .iter()
        .find(|e| e.kind == WorldEntityKind::SelfPlayer)
        .unwrap();
    assert_eq!((self_player.x, self_player.y), (bind.x, bind.y));
    assert!(!self_player.dead);
}

#[test]
fn genuine_new_login_from_full_hp_corpse_is_alive_at_saved_valid_position() {
    let config = SimulationConfig::default();
    let mut runtime = InProcessWorldRuntime::new(config.clone());
    start(&mut runtime);
    let position = Point { x: 337, y: 270 };
    runtime.force_authoritative_player_transform(position.clone(), MirDirection::Right);
    let hp = runtime.world_snapshot().player_max_hp.unwrap();
    runtime.force_authoritative_player_vitals(Some(0), Some(0));
    runtime.force_authoritative_player_vitals(Some(hp), None);
    // Preserve the source distinction: durable character saves have HP/MP, not
    // a permanent Dead flag. Online life resets only with a genuinely new login.
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    let mut login = InProcessWorldRuntime::new(config);
    start(&mut login);
    let after = login.world_snapshot();
    let player = after
        .entities
        .iter()
        .find(|e| e.kind == WorldEntityKind::SelfPlayer)
        .unwrap();
    assert!(!player.dead);
    assert_eq!((player.x, player.y), (position.x, position.y));
    assert_eq!(after.player_hp, Some(hp));
}
