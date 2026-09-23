//! Read-only instructions sourced from the same requirements the server checks.
use std::sync::OnceLock;

pub(crate) fn newcomer_config() -> &'static serde_json::Value {
    static CONFIG: OnceLock<serde_json::Value> = OnceLock::new();
    CONFIG.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../config/quest-guidance/newcomer-journey-v2.json"
        ))
        .expect("bundled V2 quest configuration")
    })
}

#[derive(Debug)]
pub struct PracticeGuide {
    /// Kill objectives precede flag objectives in the server packet.
    pub objective_index: usize,
    pub summary: String,
    pub instructions: Vec<String>,
}

pub fn practice_guide(quest_index: i32, class_name: &str) -> Option<PracticeGuide> {
    let definition = newcomer_config()["quests"].as_array()?.iter()
        .find(|quest| quest["id"].as_i64() == Some(i64::from(quest_index)))?;
    let (flag_index, flag) = definition["flags"].as_array()?.iter().enumerate()
        .find(|(_, flag)| flag["kind"].as_str() == Some("classPractice"))?;
    let class = class_name.to_ascii_lowercase();
    let requirements = flag["requirements"][&class].as_array()?;
    let mut summaries = Vec::new();
    let mut instructions = Vec::new();
    for requirement in requirements {
        let requirement = requirement.as_str()?;
        let (summary, instruction) = requirement_instruction(requirement)
            .unwrap_or((requirement, requirement));
        summaries.push(summary);
        instructions.push(instruction.to_owned());
    }
    if class == "warrior" && requirements.iter().any(|r| r.as_str() == Some("HalfMoon attack damage committed")) {
        instructions.push("半月与刺杀请分开开启后普攻；切换时先关闭另一个，空挥或未造成伤害不计。".into());
    }
    Some(PracticeGuide {
        objective_index: definition["kills"].as_array().map_or(0, Vec::len) + flag_index,
        summary: summaries.join("；"),
        instructions,
    })
}

fn requirement_instruction(requirement: &str) -> Option<(&str, &str)> {
    Some(match requirement {
        "Fencing learned" => ("学会基本剑术", "先学会基本剑术（Fencing），这是被动技能。"),
        "normal attack landed with Fencing learned" => ("普攻命中", "学会基本剑术后，普通攻击命中怪物。"),
        "Slaying learned" => ("学会攻杀剑术", "先学会攻杀剑术（Slaying），这是被动技能。"),
        "normal attack landed with Slaying learned" => ("普攻命中（已学攻杀）", "学会攻杀剑术（Slaying）后，普通攻击命中怪物。"),
        "FireBall learned" => ("学会火球术", "先学会火球术（FireBall）。"),
        "FireBall damage committed" => ("火球术造成伤害", "用火球术（FireBall）对怪物造成伤害，空放不计。"),
        "Healing learned" => ("学会治愈术", "先学会治愈术（Healing）。"),
        "Healing cast accepted on self" => ("对自己施放治愈术", "以自己为目标成功施放治愈术（Healing）。"),
        "GreatFireBall learned" => ("学会大火球", "先学会大火球（GreatFireBall）。"),
        "GreatFireBall damage committed" => ("大火球造成伤害", "用大火球（GreatFireBall）对怪物造成伤害。"),
        "SpiritSword learned" => ("学会精神力战法", "先学会精神力战法（SpiritSword），这是被动技能。"),
        "normal attack landed with SpiritSword learned" => ("普攻命中（已学精神力战法）", "学会精神力战法后，普通攻击命中怪物。"),
        "SoulFireBall learned" => ("学会灵魂火符", "先学会灵魂火符（SoulFireBall），装备护身符。"),
        "SoulFireBall damage committed" | "SoulFireBall damage committed with material consumption" =>
            ("灵魂火符造成伤害", "装备护身符，用灵魂火符（SoulFireBall）对怪物造成伤害；需消耗护身符。"),
        "SummonSkeleton learned" => ("学会召唤骷髅", "先学会召唤骷髅（SummonSkeleton）。"),
        "owned skeleton exists in authoritative Zone" => ("召出自己的骷髅", "装备护身符，成功召唤属于自己的骷髅。"),
        "owned skeleton damage committed" => ("自己的骷髅造成伤害", "让自己召唤的骷髅攻击怪物并造成伤害。"),
        "Thrusting learned" => ("学会刺杀剑术", "先学会刺杀剑术（Thrusting）。"),
        "Thrusting attack damage committed" => ("刺杀剑术造成伤害", "开启刺杀剑术（Thrusting）后普攻，对怪物造成伤害。"),
        "spell damage followed by legal reposition" => ("法术伤害后移动", "先用法术造成伤害，再走到可通行位置；只原地施法不计。"),
        "Poisoning learned" => ("学会施毒术", "先学会施毒术（Poisoning），准备毒粉。"),
        "owned poison effect committed" | "owned poison effect committed with material consumption" =>
            ("成功施毒", "装备毒粉，用施毒术（Poisoning）使怪物中毒；需消耗毒粉。"),
        "FireWall learned" => ("学会火墙", "先学会火墙（FireWall）。"),
        "owned FireWall damage committed" => ("自己的火墙造成伤害", "用自己施放的火墙（FireWall）灼伤怪物；只放空地不计。"),
        "HalfMoon learned" => ("学会半月弯刀", "先学会半月弯刀（HalfMoon）。"),
        "HalfMoon attack damage committed" => ("半月弯刀造成伤害", "开启半月弯刀（HalfMoon）后普攻，对怪物造成伤害。"),
        "Lightning learned" => ("学会雷电术", "先学会雷电术（Lightning）。"),
        "Lightning damage committed" => ("雷电术造成伤害", "用雷电术（Lightning）对怪物造成伤害。"),
        "legal reposition between attacks" => ("攻击之间移动", "两次攻击之间移动到可通行位置。"),
        "level-eligible weapon equipped" => ("装备可用武器", "装备一把符合自身等级要求的武器。"),
        "eligible Amulet equipped and legal poison available in bag for re-equip between casts" =>
            ("装备护身符并携带毒粉", "装备可用护身符，背包携带可用毒粉；施毒与火符之间切换材料。"),
        _ => return None,
    })
}

/// Town merchants are authored NPCs; the shop remains the authority for price,
/// stock and purchasing. This guide never grants supplies or issues purchases.
pub fn supply_instructions(quest_index: i32) -> Vec<String> {
    let Some(quest) = newcomer_config()["quests"].as_array().and_then(|quests| quests.iter()
        .find(|quest| quest["id"].as_i64() == Some(i64::from(quest_index)))) else { return Vec::new(); };
    if quest["minLevel"].as_u64().unwrap_or(0) < 16 { return Vec::new(); }
    vec![
        "药水不足时，先回城补给；进入深层前检查背包负重。".into(),
        "比奇城 Alchemist Samuel（324,291）：中型、大型红药和蓝药。".into(),
        "比奇城 Merchant Bull（374,296）：回城卷、随机卷、地牢逃脱卷。".into(),
        "回城卷返回最近经过的安全区；地图禁用的卷轴无法使用，可按入口路线步行返回。".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_authored_class_requirement_has_player_instructions() {
        for quest in newcomer_config()["quests"].as_array().unwrap() {
            for flag in quest["flags"].as_array().unwrap() {
                if flag["kind"] != "classPractice" { continue; }
                for requirements in flag["requirements"].as_object().unwrap().values() {
                    for requirement in requirements.as_array().unwrap() {
                        assert!(requirement_instruction(requirement.as_str().unwrap()).is_some(), "{requirement}");
                    }
                }
            }
        }
    }

    #[test]
    fn graduation_requires_both_warrior_skills_without_other_classes() {
        let guide = practice_guide(2110021, "Warrior").unwrap();
        assert_eq!(guide.objective_index, 1);
        assert_eq!(guide.summary, "半月弯刀造成伤害；刺杀剑术造成伤害");
        assert!(guide.instructions.last().unwrap().contains("分开开启"));
        assert!(!guide.summary.contains("火墙"));
        assert!(practice_guide(2110021, "Wizard").unwrap().summary.contains("攻击之间移动"));
        assert!(practice_guide(2110021, "Taoist").unwrap().summary.contains("自己的骷髅"));
        assert!(practice_guide(2110021, "Unknown").is_none());
        assert!(practice_guide(2110020, "Warrior").is_none());
    }
}
