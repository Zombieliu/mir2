//! Source HumanObject.ProcessBuffs social lifetime, separate from GainExp's
//! live range/group checks. An offline partner prevents creation, not retention.
use super::buffs::{client_buff_packet_for_state, crystal_buff_type_for_key, BuffState};
use super::components::{player_entity, ObjectId};
use super::resources::BuffResource;
use crate::{SimulationSession, Stage5FriendIdentity};
use bevy_ecs::prelude::World;
use mir2_protocol::{ServerPacket, UserItemStat};
use std::collections::BTreeSet;

impl SimulationSession {
    /// `online` is supplied only by the server's validated shared presences.
    /// Display names and personal RemotePlayer entities cannot grant a buff.
    pub fn refresh_shared_social_buffs(
        &mut self,
        online: &BTreeSet<(String, i32)>,
    ) -> Result<Vec<ServerPacket>, String> {
        let Some(active) = self.active_identity() else {
            return Ok(Vec::new());
        };
        let Some(config) = self.shared_mentor_config() else {
            return Ok(Vec::new());
        };
        let identity = Stage5FriendIdentity {
            account_id: active.account_id,
            character_index: active.character_index,
        };
        let marriage = config.shared_marriage_profile_for(&identity)?.relationship;
        let mentor = config.shared_mentor_profile_for(&identity)?.mentor;
        let is_online = |partner: &Option<Stage5FriendIdentity>| {
            partner.as_ref().is_some_and(|peer| {
                peer != &identity
                    && online.contains(&(peer.account_id.clone(), peer.character_index))
            })
        };
        Ok(synchronize(
            self.app.world_mut(),
            marriage.partner_identity.is_some(),
            is_online(&marriage.partner_identity),
            mentor.partner_identity.as_ref().map(|_| mentor.is_mentor),
            is_online(&mentor.partner_identity),
        ))
    }

    /// Actual source Buff presence/rates; callers must additionally validate
    /// current relationship, active peer, Zone, life, range and (mentee) group.
    pub fn social_experience_buff_rates(&self) -> (Option<i32>, Option<i32>) {
        let world = self.app.world();
        let tick = super::resources::runtime_tick(world);
        let buffs = &world.resource::<BuffResource>().buffs;
        let rate = |key: &str, stat| {
            buffs
                .iter()
                .find(|buff| buff.key == key && !buff.expired(tick))
                .map(|buff| {
                    buff.stats
                        .iter()
                        .filter(|value| value.stat == stat)
                        .fold(0i32, |sum, value| sum.saturating_add(value.value))
                })
        };
        (rate("lover", 120), rate("mentee", 123))
    }
}

fn synchronize(
    world: &mut World,
    married: bool,
    lover_online: bool,
    mentor_role: Option<bool>,
    mentor_online: bool,
) -> Vec<ServerPacket> {
    let Some(object_id) = player_entity(world)
        .and_then(|entity| world.get::<ObjectId>(entity))
        .map(|id| id.0)
    else {
        return Vec::new();
    };
    let mut packets = Vec::new();
    let removed: Vec<_> = world
        .resource::<BuffResource>()
        .buffs
        .iter()
        .filter(|buff| {
            (buff.key == "lover" && !married)
                || (matches!(buff.key.as_str(), "mentor" | "mentee") && mentor_role.is_none())
        })
        .filter_map(|buff| crystal_buff_type_for_key(&buff.key))
        .collect();
    world.resource_mut::<BuffResource>().buffs.retain(|buff| {
        !(buff.key == "lover" && !married)
            && !(matches!(buff.key.as_str(), "mentor" | "mentee") && mentor_role.is_none())
    });
    packets.extend(
        removed
            .into_iter()
            .map(|buff_type| ServerPacket::RemoveBuff {
                buff_type,
                object_id,
            }),
    );
    let has = |world: &World, keys: &[&str]| {
        world
            .resource::<BuffResource>()
            .buffs
            .iter()
            .any(|buff| keys.contains(&buff.key.as_str()))
    };
    // Exact current Crystal MarriageSystem.ini / MentorSystem.ini values.
    // Teacher damage is a separate stat; it is never added to student EXP.
    let mut additions = Vec::new();
    if married && lover_online && !has(world, &["lover"]) {
        additions.push((
            "lover",
            "Lover",
            120,
            super::stats::CRYSTAL_LOVER_EXP_RATE_PERCENT,
        ));
    }
    if mentor_online && !has(world, &["mentor", "mentee"]) {
        match mentor_role {
            Some(true) => additions.push(("mentor", "Mentor", 121, 10)),
            Some(false) => additions.push((
                "mentee",
                "Mentee",
                123,
                super::stats::CRYSTAL_MENTEE_EXP_RATE_PERCENT,
            )),
            None => {}
        }
    }
    for (key, name, stat, value) in additions {
        let buff = BuffState {
            key: key.into(),
            name: name.into(),
            description: String::new(),
            expires_at_tick: u64::MAX,
            real_time_duration: None,
            attack_bonus: 0,
            defence_bonus: 0,
            stats: vec![UserItemStat { stat, value }],
        };
        if let Some(packet) = client_buff_packet_for_state(world, &buff) {
            packets.push(packet);
        }
        world.resource_mut::<BuffResource>().buffs.push(buff);
    }
    packets
}

#[cfg(test)]
mod tests {
    use super::*;
    fn world() -> World {
        let mut world = World::new();
        world.insert_resource(BuffResource::new());
        world.insert_resource(super::super::resources::RuntimeClockResource::new());
        world.spawn((super::super::components::SelfPlayer, ObjectId(42)));
        world
    }
    #[test]
    fn shared_social_buffs_uses_durable_identity_not_legacy_display_name() {
        use crate::{SimulationConfig, Stage5SystemsState};
        use mir2_protocol::ClientPacket;
        let config = SimulationConfig::default();
        let index = config.default_character.index;
        let mut systems = Stage5SystemsState::default();
        systems.mentor.name = "Teacher".into();
        systems.relationship.partner_name = "Spouse".into();
        config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&index)
            .unwrap()
            .stage5_systems_json = Some(serde_json::to_string(&systems).unwrap());
        let mut session = SimulationSession::new(config.clone());
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into()
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        session.handle_packet(ClientPacket::StartGame {
            character_index: index,
        });
        let online = [("teacher".to_string(), 7)].into_iter().collect();
        assert!(session
            .refresh_shared_social_buffs(&online)
            .unwrap()
            .is_empty());
        assert_eq!(session.social_experience_buff_rates(), (None, None));
        systems.mentor.partner_identity = Some(Stage5FriendIdentity {
            account_id: "teacher".into(),
            character_index: 7,
        });
        config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&index)
            .unwrap()
            .stage5_systems_json = Some(serde_json::to_string(&systems).unwrap());
        assert!(session
            .refresh_shared_social_buffs(&BTreeSet::new())
            .unwrap()
            .is_empty());
        assert_eq!(
            session.refresh_shared_social_buffs(&online).unwrap().len(),
            1
        );
        assert_eq!(session.social_experience_buff_rates(), (None, Some(10)));
    }
    #[test]
    fn shared_social_buffs_online_creates_infinite_source_packets_offline_retains() {
        let mut world = world();
        assert!(synchronize(&mut world, true, false, Some(false), false).is_empty());
        let packets = synchronize(&mut world, true, true, Some(false), true);
        assert_eq!(packets.len(), 2);
        for packet in packets {
            let ServerPacket::AddBuff { buff } = packet else {
                panic!()
            };
            assert!(buff.infinite);
            assert_eq!(buff.expire_time, 0);
            assert_eq!(buff.object_id, 42);
            assert!(!buff.visible);
        }
        assert!(synchronize(&mut world, true, false, Some(false), false).is_empty());
        assert_eq!(world.resource::<BuffResource>().buffs.len(), 2);
        assert!(synchronize(&mut world, true, true, Some(false), true).is_empty());
        let removed = synchronize(&mut world, false, false, None, false);
        assert_eq!(removed.len(), 2);
        assert!(world.resource::<BuffResource>().buffs.is_empty());
    }
    #[test]
    fn shared_social_buffs_teacher_has_damage_stat_not_mentee_experience() {
        let mut world = world();
        let packets = synchronize(&mut world, false, false, Some(true), true);
        let ServerPacket::AddBuff { buff } = &packets[0] else {
            panic!()
        };
        assert_eq!(buff.buff_type, 109);
        assert_eq!(
            buff.stats,
            vec![UserItemStat {
                stat: 121,
                value: 10
            }]
        );
        assert!(synchronize(&mut world, false, false, Some(true), true).is_empty());
    }
}
