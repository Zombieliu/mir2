//! Source GainExp social eligibility, evaluated from one authoritative Zone view.
//! Inputs are server snapshots, never client claims or cached personal AOI copies.
use super::ZoneKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExperienceCharacter<'a> {
    pub account_id: &'a str,
    pub character_index: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct ExperiencePresence<'a> {
    pub character: ExperienceCharacter<'a>,
    pub zone: &'a ZoneKey,
    pub position: [i32; 2],
    /// Excludes a leaving session and a standby replica as well as offline peers.
    pub active: bool,
    pub dead: bool,
    /// The shared group's stable identity, not its rendered member names.
    pub group_id: Option<&'a str>,
}

/// Crystal PlayerObject.GainExp requires the corresponding buff and an alive,
/// online partner in the same map within Globals.DataRange (inclusive square 16).
/// The caller resolves the durable relationship to `expected` before entering
/// the Zone read. Re-evaluate at reward capture, not when the player logs in.
pub fn crystal_social_experience_peer_eligible(
    owner: ExperiencePresence<'_>,
    expected: Option<ExperienceCharacter<'_>>,
    peer: Option<ExperiencePresence<'_>>,
    has_source_buff: bool,
    require_same_group: bool,
) -> bool {
    let (Some(expected), Some(peer)) = (expected, peer) else { return false; };
    if !has_source_buff || !owner.active || !peer.active || peer.dead
        || expected.account_id.is_empty() || expected == owner.character
        || peer.character != expected || peer.zone != owner.zone
    {
        return false;
    }
    if owner.position.iter().zip(peer.position).any(|(a, b)| a.abs_diff(b) > 16) {
        return false;
    }
    !require_same_group || owner.group_id
        .filter(|id| !id.is_empty())
        .is_some_and(|id| peer.group_id == Some(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn zone() -> ZoneKey {
        ZoneKey { shard_id: "s".into(), map_file_name: "0".into(), channel_id: 1, instance_id: "i".into() }
    }
    fn presence<'a>(zone: &'a ZoneKey, account: &'a str) -> ExperiencePresence<'a> {
        ExperiencePresence { character: ExperienceCharacter { account_id: account, character_index: 1 }, zone,
            position: [0, 0], active: true, dead: false, group_id: Some("group-1") }
    }
    #[test]
    fn social_exp_peer_uses_square_range_and_full_zone_identity() {
        let z = zone(); let owner = presence(&z, "owner"); let mut peer = presence(&z, "peer");
        peer.position = [16, -16];
        let valid = |p| crystal_social_experience_peer_eligible(owner, Some(peer.character), Some(p), true, false);
        assert!(valid(peer));
        let mut outside = peer; outside.position[0] = 17; assert!(!valid(outside));
        outside.position = [i32::MIN, i32::MAX]; assert!(!valid(outside));
        let mut other_channel = z.clone(); other_channel.channel_id += 1;
        let mut other_instance = z.clone(); other_instance.instance_id = "other".into();
        assert!(!valid(ExperiencePresence { zone: &other_channel, ..peer }));
        assert!(!valid(ExperiencePresence { zone: &other_instance, ..peer }));
    }
    #[test]
    fn social_exp_peer_requires_actual_stable_partner_and_buff() {
        let z = zone(); let owner = presence(&z, "owner"); let peer = presence(&z, "peer");
        let eligible = |expected, p, buff| crystal_social_experience_peer_eligible(owner, expected, p, buff, false);
        assert!(eligible(Some(peer.character), Some(peer), true));
        assert!(!eligible(Some(peer.character), Some(peer), false));
        assert!(!eligible(None, Some(peer), true));
        assert!(!eligible(Some(peer.character), None, true));
        assert!(!eligible(Some(owner.character), Some(peer), true));
        assert!(!eligible(Some(peer.character), Some(ExperiencePresence { character: ExperienceCharacter { account_id: "other-account", character_index: 1 }, ..peer }), true));
        assert!(!eligible(Some(peer.character), Some(ExperiencePresence { active: false, ..peer }), true));
        assert!(!eligible(Some(peer.character), Some(ExperiencePresence { dead: true, ..peer }), true));
    }
    #[test]
    fn social_exp_mentee_requires_nonempty_shared_group_but_lover_does_not() {
        let z = zone(); let owner = presence(&z, "owner"); let peer = presence(&z, "peer");
        let eligible = |o, p, group| crystal_social_experience_peer_eligible(o, Some(peer.character), Some(p), true, group);
        assert!(eligible(owner, peer, true));
        let no_group = ExperiencePresence { group_id: None, ..peer };
        assert!(!eligible(owner, no_group, true));
        assert!(eligible(owner, no_group, false));
        assert!(!eligible(ExperiencePresence { group_id: None, ..owner }, no_group, true));
        assert!(!eligible(ExperiencePresence { group_id: Some(""), ..owner }, ExperiencePresence { group_id: Some(""), ..peer }, true));
    }
}
