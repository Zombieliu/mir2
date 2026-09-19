//! Server-owned guild rules exported from Crystal's GuildSettings.ini.
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalGuildCreationCost {
    /// Empty means account gold, exactly as Crystal's GuildItemVolume.
    pub item_name: String,
    pub amount: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalGuildSettings {
    pub source: String,
    pub source_sha256: String,
    pub minimum_level: u8,
    pub experience_rate: f32,
    pub points_per_level: u8,
    pub war_time: i64,
    pub war_cost: u32,
    pub newbie_guild_buff_enabled: bool,
    pub newbie_guild_exp_buff: i32,
    pub total_buffs: u8,
    pub creation_costs: Vec<CrystalGuildCreationCost>,
    pub experience_levels: Vec<i64>,
    pub member_caps: Vec<i32>,
}

pub fn crystal_guild_settings() -> &'static CrystalGuildSettings {
    static SETTINGS: OnceLock<CrystalGuildSettings> = OnceLock::new();
    SETTINGS.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../data/generated/crystal_guild_settings.json"
        ))
        .expect("exported Crystal guild settings must be valid")
    })
}

/// Typed server definitions from the same original catalog sent to clients.
/// Callers must use these costs/stats instead of any client-provided fields.
pub fn crystal_guild_buff_definitions() -> &'static [mir2_protocol::GuildBuffInfo] {
    static BUFFS: OnceLock<Vec<mir2_protocol::GuildBuffInfo>> = OnceLock::new();
    BUFFS.get_or_init(|| {
        let payload = crate::crystal_guild_buff_list_packet_payload();
        let frame = mir2_protocol::encode_frame(
            mir2_protocol::ServerPacketId::GuildBuffList as i16,
            &payload,
        )
        .expect("exported guild buff catalog must frame");
        let packet = mir2_protocol::decode_server_packet(&frame)
            .expect("exported guild buff catalog must decode without trailing bytes");
        let mir2_protocol::ServerPacket::GuildBuffList { guild_buffs, .. } = packet else {
            unreachable!("guild buff packet ID decoded another packet")
        };
        guild_buffs
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guild_settings_preserve_source_costs_and_int64_threshold() {
        let rules = crystal_guild_settings();
        assert_eq!(rules.minimum_level, 22);
        assert_eq!(
            rules.creation_costs,
            vec![
                CrystalGuildCreationCost {
                    item_name: String::new(),
                    amount: 1_000_000
                },
                CrystalGuildCreationCost {
                    item_name: "WoomaHorn".into(),
                    amount: 1
                },
            ]
        );
        assert_eq!(rules.experience_levels.len(), 22);
        assert_eq!(rules.experience_levels[0], 45_000);
        assert_eq!(rules.experience_levels[21], 999_999_999_999_999_999);
        assert_eq!(rules.member_caps.len(), 22);
        assert_eq!(rules.member_caps[0], 5);
        assert_eq!(rules.member_caps[21], 1000);
        assert_eq!(rules.points_per_level, 1);
        assert_eq!(rules.experience_rate, 0.01_f32);
        assert_eq!(rules.total_buffs, 16);
    }

    #[test]
    fn guild_buff_catalog_is_complete_unique_and_server_owned() {
        let buffs = crystal_guild_buff_definitions();
        assert_eq!(
            buffs.len(),
            usize::from(crystal_guild_settings().total_buffs)
        );
        let ids: std::collections::BTreeSet<_> = buffs.iter().map(|buff| buff.id).collect();
        assert_eq!(ids.len(), buffs.len());
        assert_eq!(buffs[0].name, "Reputation");
        assert_eq!(buffs[0].id, 1);
        assert_eq!(buffs[0].points_requirement, 1);
        assert_eq!(buffs[0].time_limit, 60);
        assert_eq!(buffs[0].activation_cost, 0);
        assert!(buffs.iter().any(|buff| !buff.stats.is_empty()));
    }
}
