//! Crystal GameScene direct modes and source cycle throttles.
use std::time::{Duration, Instant};
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CombatModes {
    pub attack: u8,
    pub pet: u8,
    attack_ready: Option<Instant>,
    pet_ready: Option<Instant>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeRequest {
    Attack(u8),
    Pet(u8),
}
impl CombatModes {
    pub fn request(&mut self, function: &str, now: Instant) -> Option<ModeRequest> {
        let attack = [
            "AttackmodePeace",
            "AttackmodeGroup",
            "AttackmodeGuild",
            "AttackmodeEnemyguild",
            "AttackmodeRedbrown",
            "AttackmodeAll",
        ];
        let pet = [
            "PetmodeBoth",
            "PetmodeMoveonly",
            "PetmodeAttackonly",
            "PetmodeNone",
            "PetmodeFocusMasterTarget",
        ];
        if let Some(index) = attack.iter().position(|s| *s == function) {
            return Some(ModeRequest::Attack(index as u8));
        }
        if let Some(index) = pet.iter().position(|s| *s == function) {
            return Some(ModeRequest::Pet(index as u8));
        }
        match function {
            "ChangeAttackmode" if self.attack_ready.is_none_or(|ready| now > ready) => {
                self.attack_ready = Some(now + Duration::from_millis(300));
                Some(ModeRequest::Attack((self.attack + 1) % 6))
            }
            "ChangePetmode" if self.pet_ready.is_none_or(|ready| now > ready) => {
                self.pet_ready = Some(now + Duration::from_millis(500));
                Some(ModeRequest::Pet((self.pet + 1) % 5))
            }
            _ => None,
        }
    }
    pub fn failed_cycle(&mut self, function: &str) {
        match function {
            "ChangeAttackmode" => self.attack_ready = None,
            "ChangePetmode" => self.pet_ready = None,
            _ => {}
        }
    }
    pub fn observe(&mut self, packet: &mir2_protocol::ServerPacket) -> Option<&'static str> {
        match packet {
            mir2_protocol::ServerPacket::ChangeAMode { mode } if *mode < 6 => {
                self.attack = *mode;
                Some(
                    [
                        "[Mode: Peaceful]",
                        "[Mode: Group]",
                        "[Mode: Guild]",
                        "[Mode: Enemy Guild]",
                        "[Mode: Red/Brown]",
                        "[Mode: Attack All]",
                    ][*mode as usize],
                )
            }
            mir2_protocol::ServerPacket::ChangePMode { mode } if *mode < 5 => {
                self.pet = *mode;
                Some(
                    [
                        "[Pet: Attack and Move]",
                        "[Pet: Do Not Attack]",
                        "[Pet: Do Not Move]",
                        "[Pet: Do Not Attack or Move]",
                        "[Pet: Focus Master Target]",
                    ][*mode as usize],
                )
            }
            _ => None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cycles_wait_for_server_modes_and_use_independent_source_delays() {
        let mut m = CombatModes::default();
        let now = Instant::now();
        assert_eq!(
            m.request("ChangeAttackmode", now),
            Some(ModeRequest::Attack(1))
        );
        assert_eq!(m.attack, 0);
        assert_eq!(
            m.request("ChangeAttackmode", now + Duration::from_millis(300)),
            None
        );
        assert_eq!(m.request("ChangePetmode", now), Some(ModeRequest::Pet(1)));
        assert_eq!(
            m.request("ChangePetmode", now + Duration::from_millis(400)),
            None
        );
        m.observe(&mir2_protocol::ServerPacket::ChangeAMode { mode: 5 });
        assert_eq!(
            m.request("ChangeAttackmode", now + Duration::from_millis(301)),
            Some(ModeRequest::Attack(0))
        );
    }
    #[test]
    fn direct_modes_are_unthrottled_and_invalid_packets_never_change_state() {
        let mut m = CombatModes::default();
        let now = Instant::now();
        assert_eq!(
            m.request("AttackmodeEnemyguild", now),
            Some(ModeRequest::Attack(3))
        );
        assert_eq!(
            m.request("PetmodeFocusMasterTarget", now),
            Some(ModeRequest::Pet(4))
        );
        assert_eq!(m.pet, 0);
        assert!(m
            .observe(&mir2_protocol::ServerPacket::ChangePMode { mode: 99 })
            .is_none());
        assert_eq!(m.pet, 0);
        assert_eq!(
            m.observe(&mir2_protocol::ServerPacket::ChangePMode { mode: 4 }),
            Some("[Pet: Focus Master Target]")
        );
    }
}
