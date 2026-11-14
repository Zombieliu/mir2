//! Cold, position-only preparation for a declared mechanics experiment.
//! This module is absent from normal simulation/Gateway builds and is never
//! exposed by a client command. It does not qualify pressure or alter AI clocks.
use super::*;
use serde_json::{json, Value};

const CENTRE: (i32, i32) = (59, 56);
const RING: [(i32, i32); 7] = [
    (58, 55),
    (60, 55),
    (58, 56),
    (60, 56),
    (58, 57),
    (59, 57),
    (60, 57),
];
const EXIT: [(i32, i32); 3] = [(59, 55), (59, 54), (59, 53)];

fn point((x, y): (i32, i32)) -> Point {
    Point { x, y }
}
fn aggro_union((x, y): (i32, i32)) -> bool {
    (51..=67).contains(&x) && (47..=62).contains(&y)
}

fn invariant_monsters(zone: &ZoneRuntime) -> Result<Value, String> {
    let mut value = serde_json::to_value(&zone.native_monsters).map_err(|e| e.to_string())?;
    for monster in value
        .as_object_mut()
        .ok_or("monster map must encode as an object")?
        .values_mut()
    {
        monster
            .as_object_mut()
            .ok_or("monster must encode as an object")?
            .remove("position");
    }
    Ok(value)
}
fn digest(value: &Value) -> Result<String, String> {
    Ok(
        Sha256::digest(serde_json::to_vec(value).map_err(|e| e.to_string())?)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

impl ZoneRuntime {
    /// Relocate seven existing wild CaveBat into the proposed D021 ring, and
    /// park interfering ambient actors. Must precede the first player join/tick.
    /// Caller proves original spawn/profile/map provenance separately. Every
    /// non-position monster field and complete respawn policy remain identical.
    pub fn prepare_crowded_cave_bat_fixture_for_test(&mut self) -> Result<Value, String> {
        if self.key.map_file_name != "D021" {
            return Err("fixture requires original D021".into());
        }
        if !self.players.is_empty()
            || !self.occupancy.is_empty()
            || !self.pending_native_hits.is_empty()
            || !self.pending_native_projectiles.is_empty()
            || !self.pending_native_player_hits.is_empty()
            || !self.pending_native_player_heals.is_empty()
            || !self.pending_native_summons.is_empty()
            || !self.pending_native_ground_spells.is_empty()
            || !self.dead_object_ids.is_empty()
            || !self.removed_object_ids.is_empty()
            || !self.ground_drops.is_empty()
            || !self.claimed_ground_drops.is_empty()
        {
            return Err(
                "fixture requires a cold, unoccupied world without pending gameplay".into(),
            );
        }
        let bounds = self
            .collision
            .bounds()
            .ok_or("fixture requires decoded bounded collision")?;
        let reserved: BTreeSet<_> = std::iter::once(CENTRE).chain(RING).chain(EXIT).collect();
        for tile in &reserved {
            if self.collision.is_blocked(&point(*tile))
                || self
                    .collision
                    .transfer_source_cells_for_test()
                    .contains(tile)
            {
                return Err(format!("blocked or transfer fixture tile {tile:?}"));
            }
        }
        let selected: Vec<_> = self
            .native_monsters
            .iter()
            .filter(|(_, monster)| {
                monster.name == "CaveBat"
                    && monster.ai == 0
                    && !monster.dead
                    && monster.hp > 0
                    && monster.owner_session_id.is_none()
                    && monster.master_object_id == 0
                    && zone_native_monster_is_authoritatively_hostile(monster)
            })
            .take(7)
            .map(|(id, _)| *id)
            .collect();
        if selected.len() != 7 {
            return Err("fixture needs seven existing live hostile wild CaveBat".into());
        }
        let mut occupied = reserved.clone();
        for (id, object) in &self.objects {
            if !self.native_monsters.contains_key(id) {
                let tile = (object.position.x, object.position.y);
                if reserved.contains(&tile) {
                    return Err("non-monster occupies reserved fixture tile".into());
                }
                occupied.insert(tile);
            }
        }
        let mut positions: BTreeMap<_, _> = selected.iter().copied().zip(RING).collect();
        // Reserve original legal ambient tiles before selecting parking cells.
        // A parking choice must not displace a later unchanged source actor.
        let mut to_park = Vec::new();
        for (id, monster) in &self.native_monsters {
            if monster.dead || monster.hp <= 0 || monster.owner_session_id.is_some() {
                return Err("cold fixture cannot include dead or owned monsters".into());
            }
            let object = self
                .objects
                .get(id)
                .ok_or("missing retained native object")?;
            let ServerPacket::ObjectMonster { info } = &object.packet else {
                return Err("native object has wrong retained packet kind".into());
            };
            if object.position != monster.position || info.location != monster.position {
                return Err("source native/object/packet transforms disagree".into());
            }
            if positions.contains_key(id) {
                continue;
            }
            let tile = (monster.position.x, monster.position.y);
            if aggro_union(tile)
                || self.collision.is_blocked(&monster.position)
                || self
                    .collision
                    .transfer_source_cells_for_test()
                    .contains(&tile)
                || !occupied.insert(tile)
            {
                to_park.push(*id);
            } else {
                positions.insert(*id, tile);
            }
        }
        let mut parking = (bounds.min_y..=bounds.max_y)
            .flat_map(|y| (bounds.min_x..=bounds.max_x).map(move |x| (x, y)));
        for id in to_park {
            let tile = parking
                .find(|tile| {
                    // Additional distance reduces accidental ambient entry during
                    // the prospective 30s budget. It does not suppress natural AI.
                    let distant = tile.0 < 21 || tile.0 > 97 || tile.1 < 17 || tile.1 > 92;
                    distant
                        && !aggro_union(*tile)
                        && !occupied.contains(tile)
                        && !self.collision.is_blocked(&point(*tile))
                        && !self
                            .collision
                            .transfer_source_cells_for_test()
                            .contains(tile)
                })
                .ok_or("not enough distinct source-legal parking tiles")?;
            occupied.insert(tile);
            positions.insert(id, tile);
        }
        let invariant = invariant_monsters(self)?;
        let before_root = self.canonical_state_root()?;
        let respawns =
            serde_json::to_value(&self.native_monster_respawns).map_err(|e| e.to_string())?;
        let mut prepared = self.transaction_fork();
        let mut changes = Vec::new();
        for (id, tile) in positions {
            let monster = prepared
                .native_monsters
                .get_mut(&id)
                .expect("validated monster");
            let previous = monster.position.clone();
            if previous == point(tile) {
                continue;
            }
            monster.position = point(tile);
            let object = prepared
                .objects
                .get_mut(&id)
                .expect("validated retained object");
            object.position = point(tile);
            if let ServerPacket::ObjectMonster { info } = &mut object.packet {
                info.location = point(tile);
            }
            prepared.object_grid.insert(id, &point(tile));
            changes.push(json!({ "objectId": id, "from": previous, "to": point(tile) }));
        }
        if invariant_monsters(&prepared)? != invariant
            || serde_json::to_value(&prepared.native_monster_respawns).map_err(|e| e.to_string())?
                != respawns
        {
            return Err("fixture changed non-position native/respawn state".into());
        }
        let after_root = prepared.canonical_state_root()?;
        let restored = ZoneRuntime::restore_checkpoint(&prepared.checkpoint_bytes()?)?;
        if restored.canonical_state_root()? != after_root {
            return Err("prepared fixture failed verified checkpoint roundtrip".into());
        }
        let report = json!({
            "kind": "preparedPositionOnly", "map": "D021", "centre": point(CENTRE),
            "ring": RING.map(point), "exit": EXIT.map(point), "selectedIds": selected,
            "monsterCount": prepared.native_monsters.len(), "changes": changes,
            "beforeRoot": before_root, "afterRoot": after_root,
            "unchangedNativeFieldsSha256": digest(&invariant)?,
            "unchangedRespawnPoliciesSha256": digest(&respawns)?,
            "verifiedCheckpointRoundtrip": true,
            "pressureQualified": false, "ordinaryProgressionAccepted": false,
            "nativeInputAccepted": false, "fullP1Accepted": false
        });
        *self = prepared;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        WorldEntityDisposition, ZoneBounds, ZoneMonsterDefense, ZoneMonsterRespawnPolicy,
        ZoneMonsterSpawn,
    };

    fn fixture(count: u32) -> ZoneRuntime {
        // Pure helper checks use disclosed bounded collision. Actual transport
        // preparation separately pins/decodes the original packaged D021 map.
        let mut zone = ZoneRuntime::new_with_collision(
            ZoneKey::for_map("D021"),
            super::super::ZoneCollision::unbounded().with_bounds(ZoneBounds::new(0, 100, 0, 100)),
        );
        let template = crystal_monster_by_name("CaveBat").unwrap();
        for id in 1..=count {
            let spawn = ZoneMonsterSpawn {
                crystal_drop_seed: None,
                object_id: id,
                name: "CaveBat".into(),
                name_colour_argb: -1,
                image: template.image,
                ai: template.ai,
                disposition: Some(WorldEntityDisposition::Hostile),
                level: template.level,
                max_hp: template.hp,
                hp: template.hp,
                experience: template.experience,
                move_speed_ms: u64::from(template.move_speed),
                attack_speed_ms: u64::from(template.attack_speed),
                friendly_guild: None,
                position: Point {
                    x: 54 + id as i32,
                    y: 50,
                },
                direction: MirDirection::Down,
                defense: ZoneMonsterDefense::from_crystal_template(&template),
                respawn: Some(ZoneMonsterRespawnPolicy {
                    minimum_delay_ms: 0,
                    base_delay_ms: 300_000,
                    random_delay_step_ms: 10_000,
                    random_delay_steps: 3,
                    random_delay_subtract_steps: 0,
                    rule_index: 1,
                    slot_index: id,
                }),
                drops: Vec::new(),
            };
            assert!(zone.spawn_world_event_monster(&spawn, 1_000).0);
        }
        zone
    }

    fn rejected_unchanged(mut zone: ZoneRuntime, message: &str) {
        let root = zone.canonical_state_root().unwrap();
        let bytes = zone.checkpoint_bytes().unwrap();
        let error = zone
            .prepare_crowded_cave_bat_fixture_for_test()
            .unwrap_err();
        assert!(error.contains(message), "{error}");
        assert_eq!(zone.canonical_state_root().unwrap(), root);
        assert_eq!(zone.checkpoint_bytes().unwrap(), bytes);
    }

    #[test]
    fn crowded_fixture_preserves_all_non_position_fields_and_roundtrips() {
        let mut zone = fixture(9);
        // Strict restore re-selects the signed module's collision by ZoneKey;
        // therefore this roundtrip uses that exact loader, not mock bounds.
        zone.collision = super::super::super::ZoneCollision::for_map("D021");
        let fields = invariant_monsters(&zone).unwrap();
        let policies = serde_json::to_value(&zone.native_monster_respawns).unwrap();
        let report = zone.prepare_crowded_cave_bat_fixture_for_test().unwrap();
        assert_eq!(fields, invariant_monsters(&zone).unwrap());
        assert_eq!(
            policies,
            serde_json::to_value(&zone.native_monster_respawns).unwrap()
        );
        assert_eq!(zone.native_monster_count(), 9);
        assert_eq!(report["selectedIds"], json!([1, 2, 3, 4, 5, 6, 7]));
        assert_eq!(report["pressureQualified"], false);
        for (id, expected) in (1..=7).zip(RING) {
            assert_eq!(zone.native_monsters[&id].position, point(expected));
            assert_eq!(zone.objects[&id].position, point(expected));
            let ServerPacket::ObjectMonster { info } = &zone.objects[&id].packet else {
                panic!()
            };
            assert_eq!(info.location, point(expected));
        }
        let restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(
            restored.canonical_state_root().unwrap(),
            zone.canonical_state_root().unwrap()
        );
        assert_eq!(restored.native_monster_snapshots().len(), 9);
        for tile in EXIT {
            assert!(!restored.objects.values().any(|o| o.position == point(tile)));
        }
        assert!(zone
            .native_monsters
            .iter()
            .filter(|(id, _)| **id > 7)
            .all(|(_, m)| !aggro_union((m.position.x, m.position.y))));
    }

    #[test]
    fn crowded_fixture_rejects_wrong_map_without_mutation() {
        let mut zone = fixture(7);
        zone.key.map_file_name = "D022".into();
        rejected_unchanged(zone, "original D021");
    }

    #[test]
    fn crowded_fixture_rejects_insufficient_original_bats_without_mutation() {
        rejected_unchanged(fixture(6), "seven existing");
    }

    #[test]
    fn crowded_fixture_rejects_blocked_exit_without_mutation() {
        let mut zone = fixture(7);
        zone.collision = zone.collision.with_blocked_cells([point(EXIT[1])]);
        rejected_unchanged(zone, "blocked or transfer");
    }

    #[test]
    fn crowded_fixture_rejects_inconsistent_retained_object_without_mutation() {
        let mut zone = fixture(7);
        zone.objects.get_mut(&1).unwrap().position.x += 1;
        rejected_unchanged(zone, "transforms disagree");
    }

    #[test]
    fn crowded_fixture_rejects_live_player_without_mutation() {
        let mut zone = fixture(7);
        zone.handle(ZoneCommand::Join(crate::ZoneJoin {
            session_id: SessionId::new("fixture-owner"),
            account_id: "fixture-owner".into(),
            character_index: 1,
            object_id: 9_000,
            name: "FixtureOwner".into(),
            class: MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            level: 30,
            hp: 419,
            max_hp: 419,
            mp: 116,
            map_file_name: "D021".into(),
            position: point(CENTRE),
            direction: MirDirection::Up,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        rejected_unchanged(zone, "cold, unoccupied");
    }
}
