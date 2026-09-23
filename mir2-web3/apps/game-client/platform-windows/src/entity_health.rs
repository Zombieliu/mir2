//! Crystal MapObject.DrawHealth: packet-timed monster bars, separate from names.
use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use mir2_bevy_runtime::PresentationPoseBuffer;
use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
use serde_json::Value;

use crate::entity_overlays::NativeEntityOverlays;
use crate::entity_presentation::NativeEntityPresentation;

#[derive(Component)]
pub(crate) struct HealthRoot(String);
#[derive(Component)]
pub(crate) struct HealthFill(String);

#[derive(Default)]
struct HealthWindow {
    generation: u64,
    revision: u64,
    expires_at_ms: u64,
    hit_revision: u64,
    hit_expires_at_ms: u64,
}

#[derive(Default, Resource)]
pub(crate) struct MonsterHealthState {
    windows: HashMap<String, HealthWindow>,
    map: Option<String>,
    images: Option<[Handle<Image>; 2]>,
}

impl MonsterHealthState {
    fn visible(&mut self, entity: &Value, now_ms: u64) -> Option<(String, u8)> {
        let id = entity.get("objectId")?;
        let id = id
            .as_str()
            .map(str::to_owned)
            .or_else(|| id.as_u64().map(|id| id.to_string()))?;
        if entity.get("kind")?.as_str()? != "monster" {
            return None;
        }
        let revision = entity.get("_healthRevision")?.as_u64()?;
        let generation = entity
            .get("_healthGeneration")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let percent = entity.get("_healthPercent")?.as_u64()?.min(100) as u8;
        let seconds = entity.get("_healthExpireSeconds")?.as_u64()?;
        let window = self.windows.entry(id.clone()).or_default();
        if window.generation != generation {
            *window = HealthWindow::default();
        }
        if window.revision != revision || window.generation != generation {
            window.generation = generation;
            window.revision = revision;
            window.expires_at_ms = now_ms.saturating_add(seconds.saturating_mul(1000));
        }
        if let Some(hit) = entity.get("_healthHitRevision").and_then(Value::as_u64) {
            if hit != window.hit_revision {
                window.hit_revision = hit;
                window.hit_expires_at_ms = now_ms.saturating_add(5000);
            }
        }
        // Expired revisions stay recorded until the actor leaves. A repeated
        // world snapshot must not restart the server's visibility deadline.
        if entity.get("dead").and_then(Value::as_bool) == Some(true)
            || percent == 0
            || now_ms >= window.expires_at_ms.max(window.hit_expires_at_ms)
        {
            return None;
        }
        Some((id, percent))
    }
}

pub(crate) fn sync_native_monster_health(
    mut commands: Commands,
    shell: Res<NativeShellModel>,
    overlays: Res<NativeEntityOverlays>,
    presentation: Res<NativeEntityPresentation>,
    poses: Res<PresentationPoseBuffer>,
    time: Res<Time>,
    assets: Res<AssetServer>,
    mut state: ResMut<MonsterHealthState>,
    mut roots: Query<(Entity, &HealthRoot, &mut Node), Without<HealthFill>>,
    mut fills: Query<(&HealthFill, &mut Node, &mut ImageNode), Without<HealthRoot>>,
) {
    if shell.screen != NativeShellScreen::InGame || overlays.render_payload().is_none() {
        for (entity, _, _) in &roots {
            commands.entity(entity).despawn();
        }
        state.windows.clear();
        state.map = None;
        return;
    }
    let map = presentation.current_map_file_name().map(str::to_owned);
    if state.map != map {
        state.windows.clear();
        state.map = map;
    }
    let payload = overlays.render_payload().unwrap();
    let payload_center = payload
        .get("sceneView")
        .and_then(|v| v.get("center"))
        .and_then(|v| Some((v.get("x")?.as_i64()? as i32, v.get("y")?.as_i64()? as i32)));
    let shared = poses.native_overlay_active();
    let geometry_ready = !shared
        || (poses.native_overlay_center().is_some()
            && payload_center == poses.native_overlay_center());
    let center = poses
        .native_overlay_center()
        .or(payload_center)
        .unwrap_or((0, 0));
    let motion_ms = crate::entity_presentation::native_motion_clock_ms();
    let fallback_camera = presentation.camera_screen_offset(motion_ms);
    let camera = if shared {
        poses.native_overlay_camera_offset()
    } else {
        fallback_camera
    };
    let now_ms = time.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let mut present = HashSet::new();
    let mut desired = HashMap::new();
    for entity in payload
        .get("entities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(id) = entity.get("objectId").and_then(|id| {
            id.as_str()
                .map(str::to_owned)
                .or_else(|| id.as_u64().map(|id| id.to_string()))
        }) {
            present.insert(id);
        }
        let Some((id, percent)) = state.visible(entity, now_ms) else {
            continue;
        };
        let Some((x, y)) = entity
            .get("x")
            .and_then(Value::as_i64)
            .zip(entity.get("y").and_then(Value::as_i64))
        else {
            continue;
        };
        let offset = poses.native_overlay_entity_offset(&id).unwrap_or_else(|| {
            let combined = presentation.entity_screen_offset(&id, motion_ms);
            (
                combined.0 - fallback_camera.0,
                combined.1 - fallback_camera.1,
            )
        });
        // DisplayRectangle starts at DrawLocation; library offsets are NOT
        // applied by Crystal's Draw(index, x, y) or cropped Draw(..., false).
        let left = 480.0 + (x - i64::from(center.0)) as f32 * 48.0 + offset.0 + camera.0 + 8.0;
        let top = 352.0 + (y - i64::from(center.1)) as f32 * 32.0 + offset.1 + camera.1 - 64.0;
        desired.insert(id, (left, top, (32.0 * f32::from(percent) / 100.0).floor()));
    }
    state.windows.retain(|id, _| present.contains(id));
    for (fill, mut node, mut image) in &mut fills {
        if let Some((_, _, width)) = desired.get(&fill.0) {
            if node.width != Val::Px(*width) {
                node.width = Val::Px(*width);
            }
            let rect = Some(Rect::new(0.0, 0.0, *width, 4.0));
            if image.rect != rect {
                image.rect = rect;
            }
        }
    }
    for (entity, root, mut node) in &mut roots {
        // Packet lifetime is independent of render-center readiness. Keep the
        // old coordinates while the map catches up, but still remove expired
        // or absent actors and never restart their timers on the later frame.
        if !geometry_ready {
            if !desired.contains_key(&root.0) {
                commands.entity(entity).despawn();
            }
            continue;
        }
        if let Some((left, top, _)) = desired.remove(&root.0) {
            if node.left != Val::Px(left) {
                node.left = Val::Px(left);
            }
            if node.top != Val::Px(top) {
                node.top = Val::Px(top);
            }
        } else {
            commands.entity(entity).despawn();
        }
    }
    if !geometry_ready || desired.is_empty() {
        return;
    }
    let images = state
        .images
        .get_or_insert_with(|| {
            [
                assets.load("original-ui/Prguse2/0.png"),
                assets.load("original-ui/Prguse2/1.png"),
            ]
        })
        .clone();
    for (id, (left, top, width)) in desired {
        commands
            .spawn((
                Name::new(format!("MonsterHealth:{id}")),
                HealthRoot(id.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(left),
                    top: Val::Px(top),
                    width: Val::Px(32.0),
                    height: Val::Px(4.0),
                    ..default()
                },
                GlobalZIndex(850),
                ImageNode::new(images[0].clone()),
            ))
            .with_children(|root| {
                root.spawn((
                    HealthFill(id),
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(width),
                        height: Val::Px(4.0),
                        ..default()
                    },
                    ImageNode {
                        image: images[1].clone(),
                        rect: Some(Rect::new(0.0, 0.0, width, 4.0)),
                        ..default()
                    },
                ));
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn real_damage_and_zero_expiry_health_packets_reach_the_bar_projection() {
        use crate::gameplay_bridge::NativeGameplayAdapter;
        use crate::native_protocol::PacketEvent;
        let mut adapter = NativeGameplayAdapter::default();
        let base = json!({"playerObjectId":1000,"playerHp":10,"playerMaxHp":10,"sceneView":{"center":{"x":10,"y":20}},"entities":[{"objectId":1000,"kind":"selfPlayer","name":"Hero","x":10,"y":20},{"objectId":77,"kind":"monster","name":"Scarecrow","x":11,"y":20,"hp":100,"maxHp":100}]});
        for (packet, payload) in [
            (
                "DamageIndicator",
                json!({"objectId":77,"damage":7,"damageType":0}),
            ),
            (
                "ObjectHealth",
                json!({"objectId":77,"percent":93,"expire":0}),
            ),
        ] {
            assert!(adapter.observe_packet(&PacketEvent::Other {
                packet: packet.to_owned(),
                payload
            }));
        }
        let mut projection = base.clone();
        adapter.apply_authoritative_overlay(&mut projection);
        let snapshot = adapter.snapshot(&projection);
        let payload = snapshot.entity_render_payload.unwrap();
        let monster = payload["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entity| entity["objectId"] == json!(77))
            .unwrap();
        let mut state = MonsterHealthState::default();
        assert_eq!(state.visible(monster, 100), Some(("77".to_owned(), 93)));
        assert!(state.visible(monster, 5100).is_none());
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "DamageIndicator".to_owned(),
            payload: json!({"objectId":77,"damage":3,"damageType":3})
        }));
        let mut projection = base.clone();
        adapter.apply_authoritative_overlay(&mut projection);
        let healed = adapter.snapshot(&projection).entity_render_payload.unwrap();
        let monster = healed["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entity| entity["objectId"] == json!(77))
            .unwrap();
        assert!(
            state.visible(monster, 5200).is_none(),
            "healing must not invent a fresh hit"
        );
    }

    #[test]
    fn confirmed_hit_reveals_zero_expiry_health_without_snapshot_or_healing_renewal() {
        let mut state = MonsterHealthState::default();
        let mut entity = json!({"objectId":77,"kind":"monster","dead":false,"_healthGeneration":1,"_healthRevision":1,"_healthPercent":80,"_healthExpireSeconds":0});
        assert!(state.visible(&entity, 0).is_none());
        entity["_healthHitRevision"] = json!(1);
        assert!(state.visible(&entity, 100).is_some());
        entity["_healthRevision"] = json!(2);
        entity["_healthPercent"] = json!(90);
        assert!(state.visible(&entity, 5099).is_some());
        assert!(state.visible(&entity, 5100).is_none());
        entity["_healthHitRevision"] = json!(2);
        assert!(state.visible(&entity, 5200).is_some());
        assert!(state.visible(&entity, 10200).is_none());
    }

    #[test]
    fn health_ui_retains_actor_bar_crops_fill_and_expires_without_another_packet() {
        let mut app = App::new();
        app.add_plugins((
            bevy::app::TaskPoolPlugin::default(),
            bevy::asset::AssetPlugin::default(),
        ));
        app.init_asset::<Image>();
        let images = [
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .add(Image::default()),
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .add(Image::default()),
        ];
        app.insert_resource(MonsterHealthState {
            images: Some(images),
            ..default()
        });
        app.insert_resource(Time::<()>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.init_resource::<NativeEntityPresentation>();
        app.init_resource::<PresentationPoseBuffer>();
        let mut payload = json!({"sceneView":{"center":{"x":10,"y":20}},"entities":[{"objectId":77,"kind":"monster","x":11,"y":20,"dead":false,"_healthRevision":1,"_healthPercent":75,"_healthExpireSeconds":3}]});
        let mut overlays = NativeEntityOverlays::default();
        overlays.replace_payload(payload.clone());
        app.insert_resource(overlays);
        app.add_systems(Update, sync_native_monster_health);
        app.update();
        let root = app
            .world_mut()
            .query_filtered::<Entity, With<HealthRoot>>()
            .single(app.world())
            .unwrap();
        let fill = app
            .world_mut()
            .query_filtered::<Entity, With<HealthFill>>()
            .single(app.world())
            .unwrap();
        assert_eq!(app.world().get::<Node>(root).unwrap().left, Val::Px(536.0));
        assert_eq!(app.world().get::<Node>(root).unwrap().top, Val::Px(288.0));
        assert_eq!(
            app.world().get::<ImageNode>(fill).unwrap().rect,
            Some(Rect::new(0.0, 0.0, 24.0, 4.0))
        );
        for _ in 0..30 {
            app.update();
        }
        assert!(app.world().get::<HealthRoot>(root).is_some());
        payload["entities"][0]["_healthRevision"] = json!(2);
        payload["entities"][0]["_healthPercent"] = json!(25);
        payload["entities"][0]["x"] = json!(12);
        app.world_mut()
            .resource_mut::<NativeEntityOverlays>()
            .replace_payload(payload);
        app.update();
        assert_eq!(app.world().get::<Node>(root).unwrap().left, Val::Px(584.0));
        assert_eq!(app.world().get::<Node>(fill).unwrap().width, Val::Px(8.0));
        assert_eq!(
            app.world().get::<ImageNode>(fill).unwrap().rect,
            Some(Rect::new(0.0, 0.0, 8.0, 4.0))
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(3000));
        app.update();
        assert!(app.world().get_entity(root).is_err());
        assert!(app.world().get_entity(fill).is_err());
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<HealthRoot>>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn health_packet_deadline_is_not_extended_by_snapshots_but_equal_health_new_packet_renews() {
        let mut state = MonsterHealthState::default();
        let mut entity = json!({"objectId": 77, "kind": "monster", "dead": false, "_healthRevision": 1, "_healthPercent": 65, "_healthExpireSeconds": 3});
        assert_eq!(state.visible(&entity, 1000), Some(("77".to_owned(), 65)));
        assert!(state.visible(&entity, 3999).is_some());
        assert!(state.visible(&entity, 4000).is_none());
        assert!(state.visible(&entity, 9000).is_none());
        entity["_healthRevision"] = json!(2);
        assert!(state.visible(&entity, 9000).is_some());
        assert!(state.visible(&entity, 12000).is_none());
        entity["_healthGeneration"] = json!(1);
        assert!(
            state.visible(&entity, 13000).is_some(),
            "same revision on a new transport must use the new packet deadline"
        );
        entity["dead"] = json!(true);
        assert!(state.visible(&entity, 9001).is_none());
        entity["dead"] = json!(false);
        entity["_healthPercent"] = json!(0);
        assert!(state.visible(&entity, 9002).is_none());
        assert!(state
            .visible(
                &json!({"objectId": 78, "kind": "monster", "hp": 100, "maxHp": 100}),
                9002
            )
            .is_none());
    }
}
