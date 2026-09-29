//! Simplified Chinese display copy for the authored newcomer journey.
//!
//! These helpers never change a packet, quest condition, item key or actor ID.
//! Call `name` only for catalog/NPC names, never for player-chosen names or chat.
//! Unknown text is deliberately preserved; this is not a whole-game dictionary.

use std::sync::OnceLock;

struct QuestCopy {
    id: i32,
    source_title: &'static str,
    title: &'static str,
    description: &'static str,
}

const QUESTS: &[QuestCopy] = &[
    QuestCopy { id: 2110001, source_title: "Your first patrol kit", title: "首套巡逻装备", description: "前往比奇省边境村，向助理简（284,606）报告，领取巡逻装备。" },
    QuestCopy { id: 2110002, source_title: "Equip your weapon", title: "装备你的武器", description: "打开装备界面，装备一把符合自身职业和等级要求的武器。" },
    QuestCopy { id: 2110003, source_title: "Protect the village", title: "保卫村庄", description: "消灭村庄附近的 2 个稻草人，再前往村庄东北方（340,550）附近消灭 2 只钉耙猫。大地图的琥珀色方框表示狩猎区域，不是怪物的实时位置；出发前准备小型红药。" },
    QuestCopy { id: 2110004, source_title: "Your first class skill", title: "第一项职业技能", description: "消灭村庄附近的 2 个半兽人，并完成职业练习：战士使用基本剑术，法师使用火球术，道士使用治愈术。具体要求见职业练习说明。" },
    QuestCopy { id: 2110005, source_title: "Arrive safely in Bichon", title: "安全抵达比奇", description: "从边境村向北前往比奇城安全区（328,264）。打开大地图查看目的地；进入比奇城安全区后更新进度，边境村安全区不计入此任务。路上准备红药。" },
    QuestCopy { id: 2110006, source_title: "Prepare your supplies", title: "做好补给", description: "离开城镇前，从普通商店购买一瓶小型红药或小型蓝药。" },
    QuestCopy { id: 2110007, source_title: "Clear the forest path", title: "清理森林道路", description: "在比奇森林消灭 4 只森林雪人。先补充红药，并根据职业准备蓝药或护身符。" },
    QuestCopy { id: 2110008, source_title: "Show your class training", title: "展示职业训练", description: "消灭 2 个半兽人并完成职业练习，然后向比奇城墙公告板（334,259）报告。" },
    QuestCopy { id: 2110009, source_title: "Enter Oma Cave", title: "进入半兽人洞穴", description: "沿地图入口路线进入半兽人洞穴一层。出发前准备红药、蓝药；道士按技能需要携带护身符。" },
    QuestCopy { id: 2110010, source_title: "Push back the skeletons", title: "击退骷髅", description: "在半兽人洞穴一层消灭 4 只骷髅，为后续救援路线保留足够的药水。" },
    QuestCopy { id: 2110011, source_title: "Practice against the undead", title: "对抗亡灵练习", description: "在半兽人洞穴一层消灭 3 只骷髅，并完成本职业的攻杀剑术、大火球或灵魂火符练习。" },
    QuestCopy { id: 2110012, source_title: "Complete the cave patrol", title: "完成洞穴巡逻", description: "在半兽人洞穴一层消灭 1 只骷髅并完成职业练习，然后返回比奇城墙公告板（334,259）报告。" },
    QuestCopy { id: 2110013, source_title: "Enter the Dead Mine", title: "进入死亡矿区", description: "沿地图入口路线进入死亡矿区一层。进矿前准备红药、蓝药；道士按技能需要携带护身符。" },
    QuestCopy { id: 2110014, source_title: "Clear the mine entrance", title: "清理矿区入口", description: "在死亡矿区一层消灭 3 只僵尸（二型）和 2 只僵尸（三型），补足消耗品后再继续深入。" },
    QuestCopy { id: 2110015, source_title: "Fight and reposition", title: "战斗并重新站位", description: "在死亡矿区一层消灭 1 只僵尸（二型），并完成职业练习：战士使用刺杀剑术，法师造成法术伤害后移动，道士准备毒粉后施毒。" },
    // The current authoritative quest uses Zombie2; the old English hint still
    // says Zombie3. Display the current objective without modifying that data.
    QuestCopy { id: 2110016, source_title: "Complete the mine patrol", title: "完成矿区巡逻", description: "在死亡矿区一层消灭 3 只僵尸（二型）并完成职业练习，然后向比奇城墙公告板（334,259）报告。" },
    QuestCopy { id: 2110017, source_title: "Prepare for the expedition", title: "准备远征", description: "装备符合自身职业和等级要求的盔甲，并向比奇城墙公告板（334,259）报告远征准备情况。" },
    QuestCopy { id: 2110018, source_title: "Reach Wooma Temple", title: "抵达沃玛寺庙", description: "沿地图入口路线抵达沃玛寺庙入口，并完成职业练习。战士准备半月弯刀，法师准备雷电术，道士准备召唤骷髅及所需材料。" },
    QuestCopy { id: 2110019, source_title: "Clear a foothold", title: "清理前哨", description: "在沃玛寺庙一层消灭 3 只粪虫，补足消耗品后再深入。" },
    QuestCopy { id: 2110020, source_title: "Defeat Wooma Soldiers", title: "击败沃玛战士", description: "在沃玛寺庙一层消灭 3 名沃玛战士。准备好红药和本职业的战斗消耗品。" },
    QuestCopy { id: 2110021, source_title: "Prove your class tactics", title: "证明职业战术", description: "在沃玛寺庙一层消灭 3 名沃玛勇士，并按职业练习说明完成最后的战斗练习。" },
    QuestCopy { id: 2110022, source_title: "Report the expedition", title: "报告远征", description: "沿正常路线返回比奇省，向比奇城墙公告板（334,259）提交远征报告。" },
    QuestCopy { id: 2120015, source_title: "Level 15 growth reward", title: "15 级成长奖励", description: "达到 15 级并完成清理森林道路任务后，在任务日志领取成长奖励，学习奖励中的职业技能。" },
    QuestCopy { id: 2120020, source_title: "Level 20 growth reward", title: "20 级成长奖励", description: "达到 20 级并完成洞穴巡逻报告后，向比奇城墙公告板（334,259）领取成长武器。" },
    QuestCopy { id: 2120025, source_title: "Level 25 growth reward", title: "25 级成长奖励", description: "达到 25 级并完成矿区巡逻报告后，向比奇城墙公告板（334,259）领取成长盔甲和武器。" },
    QuestCopy { id: 2120030, source_title: "Level 30 growth reward", title: "30 级成长奖励", description: "达到 30 级并提交远征报告后，向比奇城墙公告板（334,259）领取成长奖励。" },
];

pub fn quest_title(id: i32, fallback: &str) -> String {
    QUESTS
        .iter()
        .find(|copy| copy.id == id)
        .map_or_else(|| text(fallback), |copy| copy.title.to_owned())
}

/// A complete, unwrapped instruction paragraph. Wrap after translation.
pub fn quest_description(id: i32, fallback: &str) -> String {
    QUESTS
        .iter()
        .find(|copy| copy.id == id)
        .map_or_else(|| text(fallback), |copy| copy.description.to_owned())
}

pub fn quest_objective(source: &str) -> String {
    text(source)
}

/// Exact system-name matching only. A catalog key is never edited in place.
pub fn name(source: &str) -> String {
    known_name(source).unwrap_or(source).to_owned()
}

pub fn text(source: &str) -> String {
    translate(source.trim()).unwrap_or_else(|| source.to_owned())
}

fn translate(source: &str) -> Option<String> {
    if let Some(copy) = QUESTS.iter().find(|copy| copy.source_title == source) {
        return Some(copy.title.to_owned());
    }
    if let Some((_, translated)) = TEXT.iter().find(|(original, _)| *original == source) {
        return Some((*translated).to_owned());
    }
    if let Some(translated) = known_name(source) {
        return Some(translated.to_owned());
    }
    if let Some(amount) = source
        .strip_prefix("Gold ")
        .filter(|value| is_unsigned(value))
    {
        return Some(format!("金币 {amount}"));
    }
    for (prefix, label) in [
        ("Price: ", "价格"),
        ("Repair: ", "修理"),
        ("Special repair: ", "特殊修理"),
        ("Sale: ", "出售"),
    ] {
        if let Some(value) = source.strip_prefix(prefix) {
            if let Some((amount, currency)) = value.split_once(' ') {
                let currency = match currency {
                    "gold" => "金币",
                    "pearl" | "pearls" => "珍珠",
                    _ => return None,
                };
                let valid_amount = is_unsigned(amount)
                    || amount.split_once('.').is_some_and(|(whole, fraction)| {
                        is_unsigned(whole) && is_unsigned(fraction)
                    });
                if valid_amount {
                    return Some(format!("{label}：{amount} {currency}"));
                }
            }
        }
    }
    for prefix in ["Chapter ", "Ch "] {
        if let Some(progress) = source
            .strip_prefix(prefix)
            .filter(|value| is_progress(value))
        {
            return Some(format!("章节 {progress}"));
        }
    }
    if let Some((chapter, progress)) = source.split_once(" · ") {
        if matches!(
            chapter,
            "Village Beginnings"
                | "Bichon Forest"
                | "Cave Rescue"
                | "Dead Mine"
                | "Wooma Expedition"
                | "Wooma Graduation"
        ) && (progress.starts_with("Chapter ") || progress.starts_with("Ch "))
        {
            return Some(format!(
                "{} · {}",
                translate(chapter)?,
                translate(progress)?
            ));
        }
    }
    if let Some(status) = source.strip_prefix("Status: ") {
        if matches!(
            status,
            "Available" | "In Progress" | "Complete" | "Completed" | "Ready to turn in"
        ) {
            return Some(format!("状态：{}", translate(status)?));
        }
    }
    // Match the original complete hint before the UI wraps it. The bundle is
    // already part of the client, and IDs keep the Chinese copy unambiguous.
    static GUIDANCE: OnceLock<serde_json::Value> = OnceLock::new();
    let guidance = GUIDANCE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../config/quest-guidance/newcomer-v2.json"
        ))
        .expect("bundled newcomer guidance")
    });
    if let Some(id) = guidance["quests"]
        .as_array()?
        .iter()
        .find(|entry| entry["hint"].as_str() == Some(source))
        .and_then(|entry| entry["id"].as_i64())
    {
        return QUESTS
            .iter()
            .find(|copy| i64::from(copy.id) == id)
            .map(|copy| copy.description.to_owned());
    }
    if let Some((body, progress)) = split_progress(source) {
        return translate(body).map(|translated| format!("{translated} {progress}"));
    }
    let sentence = source.strip_suffix('.').unwrap_or(source);
    if sentence != source {
        if let Some(translated) = translate(sentence) {
            return Some(translated);
        }
    }
    if let Some(target) = sentence.strip_prefix("Kill ") {
        return known_monster(target).map(|(_, translated, _)| format!("消灭{translated}"));
    }
    if let Some(target) = sentence.strip_prefix("Defeat ") {
        let (count, monster) = target.split_once(' ')?;
        if is_unsigned(count) {
            return known_monster(monster)
                .map(|(_, translated, unit)| format!("消灭 {count} {unit}{translated}"));
        }
    }
    if let Some(level) = sentence
        .strip_prefix("Reach level ")
        .and_then(|value| value.strip_suffix(" and claim the growth reward"))
        .filter(|value| is_unsigned(value))
    {
        return Some(format!("达到 {level} 级并领取成长奖励"));
    }
    if let Some(node) = sentence
        .strip_prefix("Newcomer V2 main node ")
        .filter(|value| value.parse::<u32>().is_ok_and(|id| (1..=22).contains(&id)))
    {
        return Some(format!("新手主线 · 第 {node} 步"));
    }
    for (prefix, chinese) in [("Return to ", "返回"), ("Report to ", "向")] {
        if let Some(target) = sentence.strip_prefix(prefix) {
            if let Some(translated) = known_name(target) {
                let suffix = if prefix == "Report to " { "报告" } else { "" };
                return Some(format!("{chinese}{translated}{suffix}"));
            }
        }
    }
    None
}

fn is_unsigned(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_progress(value: &str) -> bool {
    let value = value.trim().trim_start_matches('(').trim_end_matches(')');
    value
        .split_once('/')
        .is_some_and(|(current, total)| is_unsigned(current.trim()) && is_unsigned(total.trim()))
}

fn split_progress(source: &str) -> Option<(&str, &str)> {
    if source.ends_with(')') {
        if let Some(index) = source.rfind('(') {
            let (body, suffix) = source.split_at(index);
            if !body.trim().is_empty() && is_progress(suffix) {
                return Some((body.trim_end(), suffix));
            }
        }
    }
    let (body, suffix) = source.rsplit_once(char::is_whitespace)?;
    (!body.trim().is_empty() && is_progress(suffix)).then_some((body.trim_end(), suffix))
}

fn known_monster(source: &str) -> Option<&'static (&'static str, &'static str, &'static str)> {
    MONSTERS.iter().find(|(key, _, _)| *key == source)
}

fn known_name(source: &str) -> Option<&'static str> {
    if let Some((_, translated, _)) = known_monster(source) {
        return Some(translated);
    }
    if let Some((_, translated)) = NAMES.iter().find(|(key, _)| *key == source) {
        return Some(translated);
    }
    // The UI renders the same authored NPC key with either an underscore,
    // a space, or a spaced hyphen. Compare the entire known NPC name only.
    NPCS.iter()
        .find(|(key, _)| {
            *key == source || key.replace('_', " ") == source || key.replace('_', " - ") == source
        })
        .map(|(_, translated)| *translated)
}

const MONSTERS: &[(&str, &str, &str)] = &[
    ("Scarecrow", "稻草人", "个"),
    ("Scarecrows", "稻草人", "个"),
    ("RakingCat", "钉耙猫", "只"),
    ("Raking Cat", "钉耙猫", "只"),
    ("Raking Cats", "钉耙猫", "只"),
    ("HookingCat", "多钩猫", "只"),
    ("Hooking Cat", "多钩猫", "只"),
    ("Oma", "半兽人", "个"),
    ("Omas", "半兽人", "个"),
    ("ForestYeti", "森林雪人", "只"),
    ("Forest Yeti", "森林雪人", "只"),
    ("Forest Yetis", "森林雪人", "只"),
    ("Deer", "鹿", "只"),
    ("Hen", "鸡", "只"),
    ("Wolf", "狼", "只"),
    ("Wolves", "狼", "只"),
    ("CannibalPlant", "食人花", "株"),
    ("VenomSpider", "毒蜘蛛", "只"),
    ("ChestnutTree", "栗树", "棵"),
    ("EbonyTree", "乌木树", "棵"),
    ("CaveBat", "洞穴蝙蝠", "只"),
    ("CaveBat0", "洞穴蝙蝠", "只"),
    ("CaveMaggot", "洞蛆", "只"),
    ("Skeleton", "骷髅", "只"),
    ("Skeletons", "骷髅", "只"),
    ("Skeleton0", "骷髅", "只"),
    ("AxeSkeleton", "掷斧骷髅", "只"),
    ("BoneFighter", "骷髅战士", "只"),
    ("BoneWarrior", "骷髅战将", "只"),
    ("BoneElite", "骷髅精灵", "只"),
    ("Zombie1", "僵尸（一型）", "只"),
    ("Zombie2", "僵尸（二型）", "只"),
    ("Zombie3", "僵尸（三型）", "只"),
    ("Zombie4", "僵尸（四型）", "只"),
    ("Zombie5", "僵尸（五型）", "只"),
    ("Dung", "粪虫", "只"),
    ("Dungs", "粪虫", "只"),
    ("WoomaSoldier", "沃玛战士", "名"),
    ("Wooma Soldier", "沃玛战士", "名"),
    ("Wooma Soldiers", "沃玛战士", "名"),
    ("WoomaFighter", "沃玛勇士", "名"),
    ("Wooma Fighter", "沃玛勇士", "名"),
    ("Wooma Fighters", "沃玛勇士", "名"),
    ("WoomaWarrior", "沃玛战将", "名"),
    ("FlamingWooma", "火焰沃玛", "只"),
    ("WoomaGuardian", "沃玛卫士", "名"),
    ("WoomaTaurus", "沃玛教主", "名"),
    ("Centipede", "蜈蚣", "只"),
    ("Tongs", "钳虫", "只"),
    ("EvilCentipede", "触龙神", "只"),
    ("WhiteBoar", "白野猪", "只"),
];

const NAMES: &[(&str, &str)] = &[
    ("Warrior", "战士"),
    ("Wizard", "法师"),
    ("Taoist", "道士"),
    ("Assassin", "刺客"),
    ("Archer", "弓箭手"),
    ("BichonProvince", "比奇省"),
    ("Bichon Province", "比奇省"),
    ("BichonVillage", "边境村"),
    ("Bichon Village", "边境村"),
    ("BorderVillage", "边境村"),
    ("SerpentValley", "毒蛇山谷"),
    ("Serpent Valley", "毒蛇山谷"),
    ("WoomyonWoods(S)", "沃玛森林（南）"),
    ("WoomyonWoods(N)", "沃玛森林（北）"),
    ("WoomyonWoods", "沃玛森林"),
    ("WoomaForest", "沃玛森林"),
    ("OmaCave_1F", "半兽人洞穴一层"),
    ("OmaCave_2F", "半兽人洞穴二层"),
    ("OmaCave_3F", "半兽人洞穴三层"),
    ("Oma Cave", "半兽人洞穴"),
    ("DeadMineEntrance", "死亡矿区入口"),
    ("DeadMine_1F", "死亡矿区一层"),
    ("DeadMine_2F", "死亡矿区二层"),
    ("DeadMine_3F", "死亡矿区三层"),
    ("WoomaTempleEntrance", "沃玛寺庙入口"),
    ("WoomaTemple_1F", "沃玛寺庙一层"),
    ("WoomaTemple_2F", "沃玛寺庙二层"),
    ("WoomaTemple_3F", "沃玛寺庙三层"),
    ("Wooma Temple", "沃玛寺庙"),
    ("2FofInn", "客栈二楼"),
    ("Insect Cave N 2F", "昆虫洞穴北二层"),
    ("Insect Cave W 1F", "昆虫洞穴西一层"),
    ("Insect Cave W 2F", "昆虫洞穴西二层"),
    ("Life Death Coffin", "生死棺"),
    ("Angled Stone Tomb B4", "石墓四层"),
    ("ReagentStore", "药材店"),
    ("MedicineRoom", "制药室"),
    ("(HP)DrugSmall", "小型红药"),
    ("(MP)DrugSmall", "小型蓝药"),
    ("Small HP Drug", "小型红药"),
    ("Small MP Drug", "小型蓝药"),
    ("(HP)DrugMedium", "中型红药"),
    ("(MP)DrugMedium", "中型蓝药"),
    ("(HP)DrugLarge", "大型红药"),
    ("(MP)DrugLarge", "大型蓝药"),
    ("Amulet", "护身符"),
    ("Amulets", "护身符"),
    ("GreenPoison", "绿毒粉"),
    ("RedPoison", "红毒粉"),
    ("TownTeleport", "回城卷"),
    ("RandomTeleport", "随机传送卷"),
    ("DungeonEscape", "地牢逃脱卷"),
    ("Fencing", "基本剑术"),
    ("Slaying", "攻杀剑术"),
    ("Thrusting", "刺杀剑术"),
    ("HalfMoon", "半月弯刀"),
    ("ShoulderDash", "野蛮冲撞"),
    ("FireBall", "火球术"),
    ("GreatFireBall", "大火球"),
    ("Lightning", "雷电术"),
    ("FireWall", "火墙"),
    ("ThunderStorm", "地狱雷光"),
    ("Healing", "治愈术"),
    ("SpiritSword", "精神力战法"),
    ("SoulFireBall", "灵魂火符"),
    ("Poisoning", "施毒术"),
    ("SummonSkeleton", "召唤骷髅"),
    ("Purification", "净化术"),
    ("WoodenSword", "木剑"),
    ("Wooden Sword", "木剑"),
    ("SharpSword", "锋利长剑"),
    ("SharpTrident", "锋利三叉戟"),
    ("SharpScimitar", "锋利弯刀"),
    ("MartialSabre", "战斗马刀"),
    ("SpearWithHook", "钩镰枪"),
    ("KeenKrissSword", "锐利波刃剑"),
    ("ThickArmour(M)", "厚重盔甲（男）"),
    ("ThickArmour(F)", "厚重盔甲（女）"),
    ("FireMagicRobe(M)", "火焰法袍（男）"),
    ("FireMagicRobe(F)", "火焰法袍（女）"),
    ("TaoArmour(M)", "道士长袍（男）"),
    ("TaoArmour(F)", "道士长袍（女）"),
    ("SolidGreatAxe", "坚固巨斧"),
    ("SolidBronzeStaff", "坚固青铜法杖"),
    ("SolidSerpentSword", "坚固蛇形剑"),
    ("JudgementMace", "裁决之杖"),
    ("WarMageStaff", "战斗法师之杖"),
    ("SoulSpringWand", "灵泉法杖"),
    ("Board", "公告板"),
    ("Bichon Wall Board", "比奇城墙公告板"),
    ("the Bichon Wall Board", "比奇城墙公告板"),
    ("Quest Diary", "任务日志"),
    ("the Quest Diary", "任务日志"),
];

const NPCS: &[(&str, &str)] = &[
    ("Assistant_Jane", "助理简"),
    ("Assistant_Julie", "助理朱莉"),
    ("BorderVillage_Board", "边境村公告板"),
    ("BichonWall_Board", "比奇城墙公告板"),
    ("Alchemist_Samuel", "药剂师·塞缪尔"),
    ("Merchant_Bull", "杂货商·布尔"),
    ("Specialist_Travis", "毒粉商·特拉维斯"),
    ("Master_Wa", "战士导师瓦师傅"),
    ("Master_Joffrey", "导师乔弗里"),
    ("HighPriest_Jude", "道士导师朱德"),
    ("_Shinsu(Jude)", "朱德的神兽"),
    ("HighAssassin_Cloud", "刺客导师克劳德"),
    ("Captain_Jerald", "队长杰拉尔德"),
    ("Blacksmith_Smith", "铁匠史密斯"),
    ("Blacksmith_Bill", "铁匠比尔"),
    ("CraftsLady_Jude", "女工匠朱迪"),
    ("CraftsLady_Alice", "女工匠艾丽丝"),
    ("Merchant_John", "商人约翰"),
    ("Butcher_John", "屠夫约翰"),
    ("Merchant_Whitney", "商人惠特妮"),
    ("Merchant_Ruben", "商人鲁本"),
    ("Merchant_Scott", "商人斯科特"),
    ("Merchant_Sarah", "商人莎拉"),
    ("Merchant_Hans", "商人汉斯"),
    ("Merchant_Jang", "商人张"),
    ("Merchant_Helen", "商人海伦"),
    ("Merchant_Julia", "商人朱莉娅"),
    ("Merchant_Robert", "商人罗伯特"),
    ("Sailor_Rupert", "水手鲁珀特"),
    ("Teleport_Gilbert", "传送员吉尔伯特"),
    ("Teleporter_Gilbert", "传送员吉尔伯特"),
    ("Teleport_Jason", "传送员杰森"),
    ("Teleport_Harrison", "传送员哈里森"),
    ("Teleport_Kyle", "传送员凯尔"),
    ("InnKeeper_Brittney", "客栈老板布兰妮"),
    ("InnKeeper_Robin", "客栈老板罗宾"),
    ("Lottery_Eddie", "彩票商埃迪"),
    ("MaterialDealer_Jeffery", "材料商杰弗里"),
    ("MaterialDealer_Reece", "材料商里斯"),
    ("Material Dealer_Reece", "材料商里斯"),
    ("MirGuide_Peter", "传奇向导彼得"),
    ("Mir Guide_Peter", "传奇向导彼得"),
    ("Examiner_Tony", "考官托尼"),
    ("WiseFisherman_Albert", "渔翁阿尔伯特"),
    ("StableGirl_Mary", "马厩姑娘玛丽"),
    ("Solider_Richard", "士兵理查德"),
    ("TrustMerchant_James", "寄售商詹姆斯"),
    ("GTMerchant_Jamie", "公会领地商杰米"),
    ("SubjagationManager_James", "讨伐管理员詹姆斯"),
    ("SubjugationLead_Eric", "讨伐队长埃里克"),
    ("Premium_Elijah", "高级商人以利亚"),
    ("Commander_Luke", "指挥官卢克"),
    ("CraftingVillage_Portal", "工匠村传送门"),
    ("TravellingMerchant_Damian", "旅行商人达米安"),
    ("Master_Shok", "导师肖克"),
    ("Luke_Thompson", "卢克·汤普森"),
    ("John_Schwartz", "约翰·施瓦茨"),
    ("Royal_Archer", "皇家弓箭手"),
    ("Royal_Guard", "皇家卫兵"),
    ("ArcherGuard", "弓箭守卫"),
];

const TEXT: &[(&str, &str)] = &[
    ("Village Beginnings", "村庄起步"),
    ("Bichon Forest", "比奇森林"),
    ("Cave Rescue", "洞穴救援"),
    ("Dead Mine", "死亡矿区"),
    ("Wooma Expedition", "沃玛远征"),
    ("Wooma Graduation", "沃玛结业"),
    ("Available", "可接取"),
    ("In Progress", "进行中"),
    ("Ready to turn in", "可交付"),
    ("Complete", "可交付"),
    ("Completed", "已完成"),
    ("Done", "已完成"),
    ("Recommended", "推荐任务"),
    ("Main", "主线"),
    ("Side", "支线"),
    ("Active", "进行中"),
    ("All", "全部"),
    ("Nearby", "附近"),
    ("Accept", "接取"),
    ("ACCEPT", "接取"),
    ("Finish", "交付"),
    ("FINISH", "交付"),
    ("Claim", "领取"),
    ("CLAIM", "领取"),
    ("Cancel", "取消"),
    ("CANCEL", "取消"),
    ("Close", "关闭"),
    ("CLOSE", "关闭"),
    ("Share", "分享"),
    ("SHARE", "分享"),
    ("Track", "追踪"),
    ("Untrack", "取消追踪"),
    ("Abandon", "放弃任务"),
    ("Return to NPC", "返回任务人物处"),
    ("SELECT ITEM", "选择奖励"),
    ("Select Item", "选择奖励"),
    ("Newcomer Guide", "新手引导"),
    ("Tasks", "任务目标"),
    ("Return", "交付地点"),
    ("Time Limit", "时限"),
    ("Status", "状态"),
    ("Price", "价格"),
    ("Quote unavailable", "暂无报价"),
    ("Sale", "出售"),
    ("Repair", "修理"),
    ("Special repair", "特殊修理"),
    ("Chapter", "章节"),
    ("Ch", "章节"),
    ("Progress", "任务进度"),
    ("Rewards", "任务奖励"),
    ("Category: Recommended", "分类：推荐任务"),
    ("Category: Main", "分类：主线"),
    ("Category: Side", "分类：支线"),
    ("Gold", "金币"),
    ("Experience", "经验"),
    ("QUEST DIARY", "任务日志"),
    ("QUEST", "任务"),
    ("Equipment", "装备"),
    ("Skill", "技能"),
    ("Challenge", "挑战"),
    ("Medium", "中等"),
    ("Report to Assistant Jane", "向助理简报告"),
    ("Equip a weapon you can use", "装备一把可用武器"),
    ("Complete your class practice", "完成职业练习"),
    ("Reach a Bichon safe area", "抵达比奇城安全区"),
    (
        "Buy a small HP or MP potion from a normal shop",
        "从普通商店购买小型红药或小型蓝药",
    ),
    ("Reach the Oma Cave entrance", "抵达半兽人洞穴入口"),
    ("Reach the Dead Mine entrance", "抵达死亡矿区入口"),
    (
        "Equip armour for your class and level",
        "装备适合本职业与等级的盔甲",
    ),
    ("Report to the Bichon Wall Board", "向比奇城墙公告板报告"),
    ("Reach the Wooma Temple entrance", "抵达沃玛寺庙入口"),
    ("Return to the Bichon Wall Board", "返回比奇城墙公告板报告"),
    ("Return to the Quest Diary", "在任务日志交付"),
    (
        "Prepare your weapon and first class skill outside Bichon Village.",
        "在边境村外准备武器并学习第一项职业技能。",
    ),
    (
        "Starter weapon and first skill materials.",
        "新手武器和第一项技能所需材料。",
    ),
    (
        "Use Fencing after learning it.",
        "学习基本剑术后，用普通攻击命中怪物。",
    ),
    (
        "Use FireBall after learning it.",
        "学习火球术后，对怪物施放。",
    ),
    (
        "Use Healing after learning it and carry MP medicine.",
        "学习并使用治愈术，携带蓝药。",
    ),
    (
        "Travel safely, restock, and report the forest route.",
        "安全抵达城镇，补充物资并报告森林路线。",
    ),
    (
        "Level-15 growth weapon and class skill.",
        "15 级成长武器和职业技能。",
    ),
    (
        "Restock HP before Forest Yetis.",
        "猎杀森林雪人前补充红药。",
    ),
    (
        "Bring MP medicine for FireBall.",
        "准备蓝药，以便使用火球术。",
    ),
    ("Bring MP medicine and Amulets.", "携带蓝药和护身符。"),
    (
        "Clear the certified D001 rescue route and report to the Board.",
        "清理半兽人洞穴一层救援路线，并向公告板报告。",
    ),
    ("Level-20 growth weapon.", "20 级成长武器。"),
    (
        "Practise Slaying on Skeletons.",
        "通过攻击骷髅练习攻杀剑术。",
    ),
    (
        "Use GreatFireBall with MP supplies.",
        "补足蓝药后使用大火球。",
    ),
    (
        "Use SoulFireBall and prepare Amulets.",
        "准备护身符并使用灵魂火符。",
    ),
    (
        "Secure the D401 mine mouth and complete class combat practice.",
        "清理死亡矿区一层入口并完成职业战斗练习。",
    ),
    ("Level-25 armour and weapon.", "25 级成长盔甲和武器。"),
    ("Practise Thrusting safely.", "在安全的位置练习刺杀剑术。"),
    (
        "Use FireWall after learning it.",
        "学习火墙后，用火墙灼伤怪物。",
    ),
    (
        "Use Poisoning with legal poison supplies.",
        "装备可用毒粉后使用施毒术。",
    ),
    (
        "Prepare at the Board and scout the certified Wooma route.",
        "向公告板报告准备情况，并侦察沃玛路线。",
    ),
    (
        "Expedition supplies and safe-route practice.",
        "远征补给和安全路线练习。",
    ),
    (
        "Prepare HalfMoon and healing supplies.",
        "准备半月弯刀和红药。",
    ),
    ("Prepare Lightning and MP medicine.", "准备雷电术和蓝药。"),
    (
        "Prepare SummonSkeleton, Amulets, and poison.",
        "准备召唤骷髅、护身符和毒粉。",
    ),
    (
        "Finish the Wooma expedition and deliver its final report.",
        "完成沃玛远征并提交最终报告。",
    ),
    (
        "Level-30 growth reward and next-adventure choice.",
        "30 级成长奖励和下一阶段冒险目标。",
    ),
    (
        "Use HalfMoon and Thrusting together.",
        "完成半月弯刀和刺杀剑术练习；分开开启后普攻。",
    ),
    (
        "Use Lightning and FireWall with legal repositioning.",
        "使用雷电术和火墙，并在攻击之间移动。",
    ),
    (
        "Use your skeleton, SoulFireBall, and poison.",
        "使用自己召唤的骷髅、灵魂火符和施毒术。",
    ),
    (
        "Level 30. Compatible with Warrior, Wizard, and Taoist.",
        "需要 30 级，战士、法师和道士均可使用。",
    ),
    (
        "Equipment target for your next hunt.",
        "下一阶段狩猎的装备目标。",
    ),
    (
        "Warrior, level 30. Skill levels: 30 / 32 / 34.",
        "战士 30 级技能，技能等级要求：30 / 32 / 34。",
    ),
    (
        "Wizard, level 30. Skill levels: 30 / 32 / 34.",
        "法师 30 级技能，技能等级要求：30 / 32 / 34。",
    ),
    (
        "Taoist, level 30. Skill levels: 30 / 32 / 35.",
        "道士 30 级技能，技能等级要求：30 / 32 / 35。",
    ),
    (
        "Hunt EvilCentipede in Life Death Coffin for this book.",
        "前往生死棺猎杀触龙神，获取这本技能书。",
    ),
    (
        "Hunt WhiteBoar in Angled Stone Tomb B4 for this book.",
        "前往石墓四层猎杀白野猪，获取这本技能书。",
    ),
    ("Insect Cave N 2F sweep", "清剿昆虫洞穴北二层"),
    (
        "Reach Insect Cave N 2F from Insect Cave W 1F or W 2F.",
        "从昆虫洞穴西一层或西二层前往北二层。",
    ),
    (
        "Fight Centipede and Tongs; stay alert for a medium challenge.",
        "猎杀蜈蚣和钳虫；本次挑战难度中等，注意补给。",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_twenty_six_authored_quests_have_chinese_titles_and_guides() {
        let definitions: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../config/quest-guidance/newcomer-journey-v2.json"
        ))
        .unwrap();
        let mut ids = Vec::new();
        for list in ["quests", "growthRewards"] {
            for quest in definitions[list].as_array().unwrap() {
                let id = quest["id"].as_i64().unwrap() as i32;
                ids.push(id);
                assert!(QUESTS.iter().any(|copy| copy.id == id), "missing {id}");
                assert!(quest_title(id, "unknown").chars().any(|c| !c.is_ascii()));
                assert!(!quest_description(id, "unknown").is_ascii());
                for group in ["kills", "flags"] {
                    for objective in quest[group].as_array().into_iter().flatten() {
                        let source = objective["message"].as_str().unwrap();
                        assert_ne!(
                            quest_objective(source),
                            source,
                            "untranslated {id}: {source}"
                        );
                        assert!(
                            !quest_objective(source)
                                .chars()
                                .any(|c| c.is_ascii_alphabetic()),
                            "{source}"
                        );
                    }
                }
                for rewards in quest["rewards"].as_object().unwrap().values() {
                    for reward in rewards.as_array().unwrap() {
                        for field in ["name", "maleItem", "femaleItem"] {
                            if let Some(item) = reward[field].as_str() {
                                assert_ne!(name(item), item, "reward {item}");
                            }
                        }
                    }
                }
            }
        }
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 26);
        assert_eq!(ids.len(), QUESTS.len());
        let guidance: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../config/quest-guidance/newcomer-v2.json"
        ))
        .unwrap();
        for entry in guidance["quests"].as_array().unwrap() {
            let source = entry["hint"].as_str().unwrap();
            assert_ne!(text(source), source);
            assert!(
                !text(source).chars().any(|c| c.is_ascii_alphabetic()),
                "{source}"
            );
        }
    }

    #[test]
    fn chapter_titles_goals_rewards_and_class_hints_are_covered() {
        let ui: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../config/quest-guidance/newcomer-journey-v2-ui.json"
        ))
        .unwrap();
        assert_eq!(ui["chapters"].as_array().unwrap().len(), 6);
        for chapter in ui["chapters"].as_array().unwrap() {
            for field in ["title", "goal", "rewardSummary"] {
                let source = chapter[field].as_str().unwrap();
                assert_ne!(text(source), source, "{source}");
            }
            for hint in chapter["classHints"].as_object().unwrap().values() {
                assert_ne!(text(hint.as_str().unwrap()), hint.as_str().unwrap());
            }
        }
    }

    #[test]
    fn dynamic_kill_labels_preserve_counts_and_only_translate_known_monsters() {
        assert_eq!(text("Kill WoomaFighter"), "消灭沃玛勇士");
        assert_eq!(text("Defeat 3 WoomaFighter."), "消灭 3 名沃玛勇士");
        assert_eq!(text("Defeat 3 WoomaFighter. 1/3"), "消灭 3 名沃玛勇士 1/3");
        assert_eq!(text("Kill Skeleton (2 / 4)"), "消灭骷髅 (2 / 4)");
        assert_eq!(text("Defeat 4 Forest Yetis."), "消灭 4 只森林雪人");
        assert_eq!(
            text("Reach level 20 and claim the growth reward"),
            "达到 20 级并领取成长奖励"
        );
        assert_eq!(
            text("Reach the Wooma Temple entrance 0/1"),
            "抵达沃玛寺庙入口 0/1"
        );
        for source in [
            "Kill UnknownMonster",
            "Defeat -2 Skeleton.",
            "Defeat 2 SkeletonChampion.",
            "Kill Skeleton 1/no",
            "  unknown source  ",
        ] {
            assert_eq!(text(source), source);
        }
    }

    #[test]
    fn system_names_are_exact_and_unknown_player_like_names_remain_intact() {
        assert_eq!(name("(MP)DrugLarge"), "大型蓝药");
        assert_eq!(name("GreenPoison"), "绿毒粉");
        assert_eq!(name("Amulet"), "护身符");
        assert_eq!(name("WoomaTempleEntrance"), "沃玛寺庙入口");
        assert_eq!(name("Alchemist_Samuel"), "药剂师·塞缪尔");
        assert_eq!(name("Alchemist Samuel"), "药剂师·塞缪尔");
        assert_eq!(name("BichonWall - Board"), "比奇城墙公告板");
        assert_eq!(text("Return to BichonWall - Board"), "返回比奇城墙公告板");
        for source in [
            "a1",
            "Scout",
            "MyWoomaFighter",
            "FireBallFan",
            "Unknown_NPC",
            "Player says FireBall",
        ] {
            assert_eq!(name(source), source);
            assert_eq!(text(source), source);
        }
        assert_eq!(quest_title(999, "A custom quest"), "A custom quest");
        assert_eq!(
            quest_description(999, "A custom explanation"),
            "A custom explanation"
        );
    }

    #[test]
    fn current_mine_patrol_and_all_copy_keep_distinct_monster_types() {
        assert_eq!(name("Zombie2"), "僵尸（二型）");
        assert_eq!(name("Zombie3"), "僵尸（三型）");
        assert!(quest_description(2110016, "").contains("3 只僵尸（二型）"));
        for copy in QUESTS {
            assert!(!copy.title.chars().any(|c| c.is_ascii_alphabetic()));
            assert!(!copy.description.chars().any(|c| c.is_ascii_alphabetic()));
        }
    }

    #[test]
    fn chapter_and_status_templates_require_known_system_labels() {
        assert_eq!(text("Chapter 1/5"), "章节 1/5");
        assert_eq!(text("Ch 1/5"), "章节 1/5");
        assert_eq!(text("Cave Rescue · Chapter 1/5"), "洞穴救援 · 章节 1/5");
        assert_eq!(text("Status: In Progress"), "状态：进行中");
        assert_eq!(text("Price"), "价格");
        assert_eq!(name("ReagentStore"), "药材店");
        assert_eq!(name("MedicineRoom"), "制药室");
        assert_eq!(name("Specialist_Travis"), "毒粉商·特拉维斯");
        for source in [
            "Chapter Alice",
            "PlayerName · Chapter 1/5",
            "Status: User supplied message",
        ] {
            assert_eq!(text(source), source);
        }
    }

    #[test]
    fn price_templates_preserve_currency_amounts_and_reject_free_text() {
        assert_eq!(text("Price: 40 gold"), "价格：40 金币");
        assert_eq!(text("Price: 1 pearl"), "价格：1 珍珠");
        assert_eq!(text("Price: 50 pearls"), "价格：50 珍珠");
        assert_eq!(text("Repair: 18.8 gold"), "修理：18.8 金币");
        assert_eq!(text("Special repair: 188 gold"), "特殊修理：188 金币");
        assert_eq!(text("Sale: 100 gold"), "出售：100 金币");
        assert_eq!(text("Gold 700"), "金币 700");
        for source in [
            "Price: ask the player",
            "Price: 40 unknown",
            "Price: 4.0.0 gold",
            "Gold Farmer",
            "Sale: player name gold",
        ] {
            assert_eq!(text(source), source);
        }
    }
}
