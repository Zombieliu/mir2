//! Pending runtime work for the manager's live differential tests.
use super::*;

fn spawn(object_id: u32, name: &str, ai: u8, position: Point) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id,
        name: name.into(),
        name_colour_argb: -1,
        image: 359,
        ai,
        disposition: Some(crate::config::WorldEntityDisposition::Friendly),
        level: 10,
        max_hp: 200,
        hp: 200,
        experience: 0,
        move_speed_ms: 300,
        attack_speed_ms: 2500,
        friendly_guild: None,
        position,
        direction: MirDirection::Right,
        defense: Default::default(),
        respawn: None,
        drops: Vec::new(),
    }
}

impl ZoneRuntime {
    pub(crate) fn retained_presence_publication_snapshot(&self) -> serde_json::Value {
        serde_json::to_value(&self.online_presence).unwrap()
    }

    pub(crate) fn set_presence_publication_managed(&mut self, managed: bool) {
        self.managed_online_identity = managed;
    }

    pub(crate) fn presence_publication_is_managed(&self) -> bool {
        self.managed_online_identity
    }

    pub(crate) fn change_retained_presence_publication_field(
        &mut self,
        session: &SessionId,
        field: &str,
    ) {
        if field == "session_key" {
            let presence = self.online_presence.remove(session).unwrap();
            self.online_presence
                .insert(SessionId::new("wrong-map-key"), presence);
            return;
        }
        let presence = self.online_presence.get_mut(session).unwrap();
        match field {
            "owner" => presence.owner.epoch += 1,
            "key" => presence.key.map_file_name.push_str("-wrong"),
            "dead" => presence.dead = !presence.dead,
            "owned_pet_clock" => {
                let mut clock =
                    serde_json::to_value(presence.owned_pet_clock.as_ref().unwrap()).unwrap();
                clock["last_hit_until_ms"] = serde_json::json!(99_999);
                presence.owned_pet_clock = Some(serde_json::from_value(clock).unwrap());
            }
            "life_generation" => presence.life_generation += 1,
            "guild_name" => presence.guild_name = Some("wrong guild".into()),
            "chat_profile" => presence.chat_profile.as_mut().unwrap().attack_mode = 0,
            "brown_until_ms" => presence.brown_until_ms += 1,
            "position" => presence.position.x += 1,
            "level" => presence.level += 1,
            "experience_profile" => {
                presence
                    .experience_profile
                    .as_mut()
                    .unwrap()
                    .experience_rate_percent += 1
            }
            "name" => presence.name.push_str("-wrong"),
            "group_members" => presence.group_members.push("wrong member".into()),
            "mentor_bank" => {
                presence.mentor_bank = Some(crate::runtime::zone::ZoneMentorBankAttribution {
                    pupil: crate::Stage5FriendIdentity {
                        account_id: "pupil".into(),
                        character_index: 1,
                    },
                    teacher: crate::Stage5FriendIdentity {
                        account_id: "teacher".into(),
                        character_index: 2,
                    },
                    relationship_epoch: 1,
                })
            }
            _ => panic!("unknown retained projection field"),
        }
    }

    pub(crate) fn prepare_presence_publication_threat_clock(&mut self, session: &SessionId) {
        let owner_id = self.players[session].object_id;
        self.record_owned_pet_player_hit(owner_id, &SessionId::new("peer"), 100);
        self.record_owned_pet_player_hit(102, session, 101);
    }

    pub(crate) fn duplicate_presence_publication_player(
        &mut self,
        source: &ZoneRuntime,
        session: &SessionId,
    ) {
        let player = source.players[session].clone();
        self.ecs
            .upsert_player(session, player.object_id, &player.position);
        self.occupancy
            .insert(tile_key(&player.position), session.clone());
        self.player_grid.insert(session.clone(), &player.position);
        self.players.insert(session.clone(), player);
    }

    pub(crate) fn prepare_presence_publication_foreign_owners(&mut self, session: &SessionId) {
        let monster = spawn(9003, "Scarecrow", 0, Point { x: 334, y: 270 });
        assert!(self.spawn_authoritative_monster(&monster, 0).0);
        let mut foreign = self.online_presence[session].owner.clone();
        foreign.epoch += 100;
        let monster = self.native_monsters.get_mut(&9003).unwrap();
        monster.experience_owner = Some(ZoneExperienceOwner {
            session_id: session.clone(),
            account_id: foreign.account_id.clone(),
            character_index: foreign.character_index,
            object_id: foreign.object_id,
            expires_at_ms: 10_000,
            online_owner: Some(foreign.clone()),
        });
        monster.damage_poison_owner_session_id = Some(session.clone());
        monster.damage_poison_owner_object_id = foreign.object_id + 1;
        let drop = crate::GroundDropSnapshot {
            object_id: 8001,
            name: "Gold".into(),
            name_colour_argb: -1,
            icon: 0,
            x: 334,
            y: 270,
            quantity: 1,
            source_monster: "Scarecrow".into(),
            owner_object_id: Some(foreign.object_id),
            ownership_remaining_ticks: Some(30),
            loot: crate::GroundDropLootSnapshot::Gold { amount: 1 },
        };
        let mut stored = self.new_zone_ground_drop(drop, Some(10_000)).unwrap();
        stored.native_owner = Some(crate::runtime::zone::types::NativeGroundOwner {
            owner: foreign,
            source: self.key.clone(),
            born_at_ms: 0,
            expires_at_ms: 10_000,
        });
        self.ground_drops.insert(8001, stored);
    }

    pub(crate) fn assert_presence_publication_foreign_owners_pruned(&self) {
        let monster = &self.native_monsters[&9003];
        assert!(monster.experience_owner.is_none());
        assert!(monster.damage_poison_owner_session_id.is_none());
        assert_eq!(monster.damage_poison_owner_object_id, 0);
        let stored = &self.ground_drops[&8001];
        assert!(stored.owner_expires_at_ms.is_none());
        assert!(stored.drop.ownership_remaining_ticks.is_none());
        assert!(stored.native_owner.is_some());
    }

    pub(crate) fn prepare_presence_publication_health_and_poison(&mut self, session: &SessionId) {
        self.players.get_mut(session).unwrap().change_owner_mp(-1);
        self.players.get_mut(session).unwrap().change_owner_mp(-1);
        let monster = spawn(9001, "Scarecrow", 0, Point { x: 334, y: 270 });
        assert!(self.spawn_authoritative_monster(&monster, 0).0);
        // A cleared lease can leave the retained mask awaiting the next handle.
        let object = self.objects.get_mut(&9001).unwrap();
        let ServerPacket::ObjectMonster { info } = &mut object.packet else {
            panic!("fixture monster must have its ordinary retained spawn");
        };
        info.poison = CRYSTAL_POISON_GREEN;
    }

    pub(crate) fn prepare_presence_publication_pet_target(&mut self, session: &SessionId) {
        let pet = spawn(701, "BoneFamiliar", 0, Point { x: 333, y: 270 });
        assert!(self.spawn_authoritative_monster(&pet, 0).0);
        let owner_id = self.players[session].object_id;
        let monster = self.native_monsters.get_mut(&701).unwrap();
        monster.owner_session_id = Some(session.clone());
        monster.master_object_id = owner_id;
        monster.owner_player_object_id = owner_id;
        self.register_owned_pet_life(701);
        let mut target = spawn(9002, "Scarecrow", 0, Point { x: 334, y: 271 });
        target.disposition = Some(crate::config::WorldEntityDisposition::Hostile);
        assert!(self.spawn_authoritative_monster(&target, 0).0);
        // Retain a focus assigned before the mode projection being tested.
        // Repeating an unchanged 1/3 profile must still clear that live target.
        let mode = self.players[session].chat_profile.pet_mode;
        self.players.get_mut(session).unwrap().chat_profile.pet_mode = 4;
        self.focus_owned_pets(session, 9002, 100);
        self.players.get_mut(session).unwrap().chat_profile.pet_mode = mode;
    }

    pub(crate) fn prepare_presence_publication_vampire_death(&mut self, session: &SessionId) {
        // Same Master-null death source as vampire_ai::death_tests. Keep the
        // source pending across the exact manager projection boundary.
        let spider = spawn(60, "VampireSpider", 60, Point { x: 333, y: 270 });
        assert!(self.spawn_authoritative_monster(&spider, 0).0);
        let owner_id = self.players[session].object_id;
        let monster = self.native_monsters.get_mut(&60).unwrap();
        monster.owner_session_id = Some(session.clone());
        monster.master_object_id = owner_id;
        monster.owner_player_object_id = owner_id;
        monster.summon_skill_level = 2;
        super::super::vampire_ai::initialize_vampire(monster);
        assert!(
            self.apply_native_monster_direct_damage(60, 200, None, 100)
                .unwrap()
                .2
        );
        assert!(self.vampire_death_pending(60));
    }

    pub(crate) fn prepare_presence_publication_pet_death(&mut self, session: &SessionId) {
        let pet = spawn(701, "BoneFamiliar", 0, Point { x: 333, y: 270 });
        assert!(self.spawn_authoritative_monster(&pet, 0).0);
        let owner_id = self.players[session].object_id;
        let monster = self.native_monsters.get_mut(&701).unwrap();
        monster.owner_session_id = Some(session.clone());
        monster.master_object_id = owner_id;
        monster.owner_player_object_id = owner_id;
        self.register_owned_pet_life(701);
        self.players.get_mut(session).unwrap().dead = true;
        self.clear_owned_pet_life(owner_id);
        assert!(self.native_monsters[&701].dead);
    }
}
