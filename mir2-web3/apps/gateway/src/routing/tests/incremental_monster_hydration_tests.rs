use super::*;

fn shared_spitting_spider_at(
    state: &SharedInProcessZoneState,
    position: &Point,
) -> Option<u32> {
    state
        .map_layer(Some("0"))
        .and_then(|map| {
            map.entities.into_values().find(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && entity.name == "SpittingSpider"
                    && entity.x == position.x
                    && entity.y == position.y
            })
        })
        .map(|entity| entity.object_id)
}

fn shared_player_position(
    state: &SharedInProcessZoneState,
    session_id: &SessionId,
) -> Option<Point> {
    state
        .zone_manager
        .player_transform(session_id)
        .map(|(position, _)| position)
}

#[test]
fn async_walk_into_aoi_hydrates_actual_distant_manifest_spawn_on_serialized_drain() {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    assert!(
        shared
            .lock()
            .expect("shared state should lock")
            .zone_manager
            .install_empty_zone(ZoneRuntime::new_with_collision(
                ZoneKey::for_map("0"),
                // The target and randomized slot come from the real full-map
                // manifest/collision loader below. This bounded collider keeps
                // the routing regression focused on movement cadence and
                // hydration without mutating the process-wide collision mode
                // used by parallel Gateway tests.
                ZoneCollision::unbounded()
                    .with_bounds(mir2_simulation::ZoneBounds::new(0, 699, 0, 699)),
            )),
        "the bounded full-map fixture must be installed before Join"
    );
    let mut runtime = shared_session_runtime(shared.clone());
    runtime.inner =
        InProcessWorldRuntime::new(GatewayConfig::default().with_crystal_world_runtime());
    start_new_runtime(&mut runtime, "incremental-spider-hydration", "SpiderWalker");

    let respawn = mir2_game_data::crystal_map_respawns_by_file_name("0")
        .expect("map 0 should have Crystal respawns")
        .respawns
        .into_iter()
        .find(|respawn| respawn.respawn_index == 39 && respawn.monster_name == "SpittingSpider")
        .expect("map 0 should retain the distant SpittingSpider manifest group");
    let target = mir2_simulation::crystal_world_respawn_spawns("0", &respawn)
        .into_iter()
        .next()
        .map(|(_, position, _)| position)
        .expect("the manifest group should resolve an actual randomized walkable slot");
    let initial_position = runtime
        .inner
        .world_snapshot()
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .map(|entity| Point {
            x: entity.x,
            y: entity.y,
        })
        .expect("started runtime should expose its initial player position");
    assert!(
        initial_position.x.abs_diff(target.x)
            > u32::try_from(mir2_simulation::CRYSTAL_OBJECT_DATA_RANGE).unwrap()
            || initial_position.y.abs_diff(target.y)
                > u32::try_from(mir2_simulation::CRYSTAL_OBJECT_DATA_RANGE).unwrap(),
        "the randomized manifest slot must be outside the initial player AOI"
    );
    assert!(
        target.x >= 17,
        "the selected manifest slot must support the left-side AOI fixture"
    );
    let outside_aoi = Point {
        x: target.x - mir2_simulation::CRYSTAL_OBJECT_DATA_RANGE - 1,
        y: target.y,
    };
    let inside_aoi = Point {
        x: outside_aoi.x + 1,
        y: outside_aoi.y,
    };
    let session_id = runtime
        .current_zone_session_id()
        .expect("started runtime should have a Zone session");

    // Fixture setup changes both authorities without reconciling the dormant
    // personal spawn table. The far slot comes from the real randomized
    // manifest placement and is absent before ordinary movement reaches it.
    runtime.dispatch_zone_player_command(
        ZoneCommand::SyncPlayerTransform {
            session_id: session_id.clone(),
            position: outside_aoi.clone(),
            direction: MirDirection::Right,
        },
        false,
    );
    assert_eq!(
        runtime
            .inner
            .world_snapshot()
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .map(|entity| Point {
                x: entity.x,
                y: entity.y,
            }),
        Some(outside_aoi.clone())
    );
    assert_eq!(
        shared_spitting_spider_at(
            &shared.lock().expect("shared state should lock"),
            &target,
        ),
        None,
        "the distant slot must begin dormant instead of being assumed from a seed"
    );

    let walk = runtime
        .movement_ingress
        .try_execute(ClientPacket::Walk {
            direction: MirDirection::Right,
        })
        .expect("ordinary async movement should execute")
        .expect("the map position should be safe for movement ingress");
    let immediate_packets = format!("{:?}", walk.packets);
    let immediate_position = shared_player_position(
        &shared.lock().expect("shared state should lock"),
        &session_id,
    );
    if immediate_position.as_ref() != Some(&inside_aoi) {
        // A newly joined player may still be inside the normal walk cooldown.
        // Production completes that queued request on the Zone cadence, so
        // drive the same global cadence command instead of requiring a
        // synchronous UserLocation from ingress.
        super::super::run_shared_zone_cadence_tick(
            &shared,
            shared_gateway_now_ms().saturating_add(1_000),
        )
        .expect("the normal Zone cadence should complete the queued walk");
    }
    let authoritative_position = shared_player_position(
        &shared.lock().expect("shared state should lock"),
        &session_id,
    );
    assert_eq!(
        authoritative_position,
        Some(inside_aoi.clone()),
        "ordinary movement must enter the manifest slot AOI; target={target:?}, outside={outside_aoi:?}, immediate_position={immediate_position:?}, immediate_packets={immediate_packets}"
    );
    assert_eq!(
        shared_spitting_spider_at(
            &shared.lock().expect("shared state should lock"),
            &target,
        ),
        None,
        "the cadence worker cannot hydrate a personal dormant spawn by itself"
    );

    let drained = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 1 }))
        .expect("the next serialized packet should drain the authoritative movement");
    let spawned = drained.iter().find_map(|packet| match packet {
        ServerPacket::ObjectMonster { info }
            if info.name == "SpittingSpider" && info.location == target && !info.dead =>
        {
            Some(info.object_id)
        }
        _ => None,
    });
    assert!(
        spawned.is_some(),
        "entering the actual slot's AOI must emit its ObjectMonster on the serialized drain"
    );
    assert_eq!(
        shared_spitting_spider_at(
            &shared.lock().expect("shared state should lock"),
            &target,
        ),
        spawned,
        "the emitted monster must also be retained by the shared Zone"
    );
}
