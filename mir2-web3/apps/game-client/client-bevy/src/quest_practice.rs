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
        // Crystal ThunderBolt (雷電術) and Lightning (疾光電影) are
        // separate spells. Keep the old inactive-host copy intact, but resolve
        // these canonical requirements through their precise native keys.
        let native_keys = match requirement {
            "Lightning learned" => Some(("quest.practice.copy.53", "quest.practice.copy.54")),
            "Lightning damage committed" => Some(("quest.practice.copy.55", "quest.practice.copy.56")),
            _ => None,
        }.filter(|_| crate::native_i18n::active());
        if let Some((summary_key, instruction_key)) = native_keys {
            summaries.push(crate::native_i18n::key(summary_key, summary));
            instructions.push(crate::native_i18n::key(instruction_key, instruction));
        } else {
            summaries.push(crate::player_text::text(summary));
            instructions.push(crate::player_text::text(instruction));
        }
    }
    if class == "warrior" && requirements.iter().any(|r| r.as_str() == Some("HalfMoon attack damage committed")) {
        instructions.push(crate::player_text::text("半月与刺杀请分开开启后普攻；切换时先关闭另一个，空挥或未造成伤害不计。"));
    }
    Some(PracticeGuide {
        objective_index: definition["kills"].as_array().map_or(0, Vec::len) + flag_index,
        summary: summaries.join(&crate::player_text::format_named("quest.practice.separator", "；", &[])),
        instructions,
    })
}

fn requirement_instruction(requirement: &str) -> Option<(&str, &str)> {
    Some(match requirement {
        "Fencing learned" => ("学会基本剑术", "先学会基本剑术，这是被动技能。"),
        "normal attack landed with Fencing learned" => ("普攻命中", "学会基本剑术后，普通攻击命中怪物。"),
        "Slaying learned" => ("学会攻杀剑术", "先学会攻杀剑术，这是被动技能。"),
        "normal attack landed with Slaying learned" => ("普攻命中（已学攻杀）", "学会攻杀剑术后，普通攻击命中怪物。"),
        "FireBall learned" => ("学会火球术", "先学会火球术。"),
        "FireBall damage committed" => ("火球术造成伤害", "用火球术对怪物造成伤害，空放不计。"),
        "Healing learned" => ("学会治愈术", "先学会治愈术。"),
        "Healing cast accepted on self" => ("对自己施放治愈术", "以自己为目标成功施放治愈术。"),
        "GreatFireBall learned" => ("学会大火球", "先学会大火球。"),
        "GreatFireBall damage committed" => ("大火球造成伤害", "用大火球对怪物造成伤害。"),
        "SpiritSword learned" => ("学会精神力战法", "先学会精神力战法，这是被动技能。"),
        "normal attack landed with SpiritSword learned" => ("普攻命中（已学精神力战法）", "学会精神力战法后，普通攻击命中怪物。"),
        "SoulFireBall learned" => ("学会灵魂火符", "先学会灵魂火符，装备护身符。"),
        "SoulFireBall damage committed" | "SoulFireBall damage committed with material consumption" =>
            ("灵魂火符造成伤害", "装备护身符，用灵魂火符对怪物造成伤害；需消耗护身符。"),
        "SummonSkeleton learned" => ("学会召唤骷髅", "先学会召唤骷髅。"),
        "owned skeleton exists in authoritative Zone" => ("召出自己的骷髅", "装备护身符，成功召唤属于自己的骷髅。"),
        "owned skeleton damage committed" => ("自己的骷髅造成伤害", "让自己召唤的骷髅攻击怪物并造成伤害。"),
        "Thrusting learned" => ("学会刺杀剑术", "先学会刺杀剑术。"),
        "Thrusting attack damage committed" => ("刺杀剑术造成伤害", "开启刺杀剑术后普攻，对怪物造成伤害。"),
        "spell damage followed by legal reposition" => ("法术伤害后移动", "先用法术造成伤害，再走到可通行位置；只原地施法不计。"),
        "Poisoning learned" => ("学会施毒术", "先学会施毒术，准备毒粉。"),
        "owned poison effect committed" | "owned poison effect committed with material consumption" =>
            ("成功施毒", "用施毒术使怪物中毒；可消耗背包或腰带内毒粉，已装备毒粉优先。"),
        "FireWall learned" => ("学会火墙", "先学会火墙。"),
        "owned FireWall damage committed" => ("自己的火墙造成伤害", "用自己施放的火墙灼伤怪物；只放空地不计。"),
        "HalfMoon learned" => ("学会半月弯刀", "先学会半月弯刀。"),
        "HalfMoon attack damage committed" => ("半月弯刀造成伤害", "开启半月弯刀后普攻，对怪物造成伤害。"),
        "Lightning learned" => ("学会雷电术", "先学会雷电术。"),
        "Lightning damage committed" => ("雷电术造成伤害", "用雷电术对怪物造成伤害。"),
        "legal reposition between attacks" => ("攻击之间移动", "两次攻击之间移动到可通行位置。"),
        "level-eligible weapon equipped" => ("装备可用武器", "装备一把符合自身等级要求的武器。"),
        "eligible Amulet equipped and legal poison available in bag for re-equip between casts" =>
            ("装备护身符并携带毒粉", "装备可用护身符，背包或腰带携带可用毒粉；施毒术无需切换装备。"),
        _ => return None,
    })
}

/// Town merchants are authored NPCs; the shop remains the authority for price,
/// stock and purchasing. This guide never grants supplies or issues purchases.
pub fn supply_instructions(quest_index: i32, class_name: &str) -> Vec<String> {
    let Some(quest) = newcomer_config()["quests"].as_array().and_then(|quests| quests.iter()
        .find(|quest| quest["id"].as_i64() == Some(i64::from(quest_index)))) else { return Vec::new(); };
    let level = quest["minLevel"].as_u64().unwrap_or(1);
    let size = if level >= 25 { "大型" } else if level >= 16 { "中型" } else { "小型" };
    let mut lines = vec![
        crate::player_text::format_named("quest.supply.instruction", "本阶段建议购买{size}药水；在任务卡点“补给检查”查看库存和商店路线。", &[("size", &crate::player_text::supply_size(size))]),
        "比奇城药剂师·塞缪尔（324,291）：红药和蓝药。".into(),
        "比奇城杂货商·布尔（374,296）：回城卷、随机卷、地牢逃脱卷和护身符。".into(),
    ];
    if class_name.eq_ignore_ascii_case("Wizard") {
        lines.push("法师持续施法消耗魔法值，蓝药不足时先补给，预留红药和返程卷。".into());
    } else if class_name.eq_ignore_ascii_case("Taoist") {
        lines.push("道士施法前准备蓝药；火符和召唤骷髅消耗护身符，施毒消耗毒粉。".into());
        lines.push("毒粉商·特拉维斯在制药室（4,9）；点补给检查选择毒粉，会逐段标出店铺入口。".into());
        lines.push("火符和召唤仍需装备护身符；施毒术可直接消耗背包或腰带内毒粉，已装备毒粉优先。".into());
    }
    lines.push("回城卷回到最近经过的安全区；地图禁用卷轴时，可按入口路线步行返回。购买前检查金币和负重。".into());
    lines.into_iter().map(|line| crate::player_text::text(&line)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_lightning_practice_uses_its_own_spell_name_and_keeps_legacy_copy() {
        use crate::native_i18n::{with_locale, Locale};
        assert_eq!(requirement_instruction("Lightning learned"), Some(("学会雷电术", "先学会雷电术。")));
        for (locale, spell) in [(Locale::English, "Lightning"),
            (Locale::TraditionalChinese, "疾光電影"),
            (Locale::BrazilianPortuguese, "Relâmpago")] {
            with_locale(locale, || {
                for id in [2_110_018, 2_110_021] {
                    let guide = practice_guide(id, "Wizard").unwrap();
                    assert!(guide.summary.contains(spell));
                    assert!(guide.instructions.iter().any(|line| line.contains(spell)));
                    if locale == Locale::TraditionalChinese {
                        assert!(!guide.summary.contains("雷電術"));
                    }
                }
            });
        }
    }

    #[test]
    fn poisoning_guidance_explains_carried_supplies_and_keeps_amulets_equipped() {
        use crate::native_i18n::{with_locale, Locale};
        let (_, poison) = requirement_instruction("owned poison effect committed with material consumption").unwrap();
        assert!(poison.contains("背包或腰带") && poison.contains("已装备毒粉优先"));
        // Resolve real key overrides as well as the displayed instructions.
        // Extra locale packs previously kept equip/swap-powder copy even
        // after the canonical three-language entries had been corrected.
        for (locale, carried, priority, equipped, no_swap, compact_carried, description_carried) in [
            (Locale::English, "bag or belt", "equipped powder used first", "equipped Amulets",
                "needs no equipment swap", "carried powder", "carry poison powder"),
            (Locale::TraditionalChinese, "背包或腰帶", "已裝備毒粉優先", "裝備護身符",
                "無需切換裝備", "攜帶毒粉", "準備毒粉"),
            (Locale::BrazilianPortuguese, "bolsa ou do cinto", "priorizando o pó equipado", "Amuletos equipados",
                "não exige trocar o equipamento", "pó carregado", "leva pó de veneno"),
            (Locale::Russian, "сумки или пояса", "экипированный порошок используется первым", "экипированных талисманов",
                "менять экипировку не нужно", "переносимый порошок", "носят ядовитый порошок"),
            (Locale::Hindi, "थैले या बेल्ट", "पहना हुआ विष चूर्ण पहले", "पहने हुए ताबीज़",
                "उपकरण बदलना ज़रूरी नहीं", "साथ रखा विष चूर्ण", "विष चूर्ण साथ रखकर"),
            (Locale::Indonesian, "tas atau sabuk", "bubuk yang dikenakan digunakan lebih dahulu", "Jimat yang dikenakan",
                "tidak memerlukan pergantian", "bubuk yang dibawa", "membawa Bubuk Racun"),
            (Locale::Vietnamese, "túi hoặc thắt lưng", "bột đã trang bị được dùng trước", "Bùa đã trang bị",
                "không cần đổi trang bị", "bột mang theo", "mang Bột độc"),
            (Locale::Thai, "กระเป๋าหรือเข็มขัด", "ใช้ผงพิษที่สวมใส่ก่อน", "เครื่องรางที่สวมใส่",
                "ไม่ต้องสลับอุปกรณ์", "ผงพิษที่พกอยู่", "พกผงพิษ"),
            (Locale::Arabic, "الحقيبة أو الحزام", "المسحوق المجهّز أولًا", "التعويذات المجهّزة",
                "لا يحتاج «التسميم» إلى تبديل المعدات", "المسحوق المحمول", "يحمل الطاويون مسحوق السم"),
        ] {
            with_locale(locale, || {
                let guide = practice_guide(2_110_015, "Taoist").unwrap();
                assert!(guide.instructions.iter().any(|line| line.contains(carried) && line.contains(priority)), "{locale:?}: {:?}", guide.instructions);
                let supplies = supply_instructions(2_110_021, "Taoist");
                assert!(supplies.iter().any(|line| line.contains(carried) && line.contains(equipped) && line.contains(priority)), "{locale:?}: {supplies:?}");
                let material_requirement = crate::native_i18n::key("quest.practice.copy.62", "");
                assert!(material_requirement.contains(no_swap), "{locale:?}: {material_requirement}");
                let compact = crate::native_i18n::key("quest.supply.materials", "");
                let compact_equipped = if locale == Locale::TraditionalChinese { "裝備符" } else { equipped };
                assert!(compact.contains(compact_equipped) && compact.contains(compact_carried), "{locale:?}: {compact}");
                let description = crate::native_i18n::key("quest.2110015.description", "");
                assert!(description.contains(description_carried), "{locale:?}: {description}");
            });
        }
    }

    #[test]
    fn native_practice_and_supplies_cover_every_class_without_mutating_requirements() {
        use crate::native_i18n::{with_locale, Locale};
        let original = newcomer_config().clone();
        for locale in Locale::ALL {
            with_locale(locale, || {
                for definition in newcomer_config()["quests"].as_array().unwrap() {
                    let id = definition["id"].as_i64().unwrap() as i32;
                    for class in ["Warrior", "Wizard", "Taoist"] {
                        if let Some(guide) = practice_guide(id, class) {
                            assert!(!guide.instructions.is_empty());
                            assert!(!guide.summary.contains("committed") && !guide.summary.contains('{'));
                            if locale != Locale::TraditionalChinese {
                                assert!(!guide.instructions.iter().any(|line| line.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))));
                            }
                        }
                        let supplies = supply_instructions(id, class);
                        assert!(!supplies.is_empty());
                        assert!(!supplies.iter().any(|line| line.contains('{')));
                        if locale != Locale::TraditionalChinese {
                            assert!(!supplies.iter().any(|line| line.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))));
                        }
                    }
                }
            });
        }
        assert_eq!(newcomer_config(), &original);
    }

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
