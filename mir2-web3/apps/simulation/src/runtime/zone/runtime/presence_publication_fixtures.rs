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
