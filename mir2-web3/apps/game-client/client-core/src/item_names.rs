//! Exact authored system-name lookup; unknown text is preserved.

pub fn name(source: &str) -> String {
    known_name(source).unwrap_or(source).to_owned()
}

pub fn known_monster(source: &str) -> Option<&'static (&'static str, &'static str, &'static str)> {
    MONSTERS.iter().find(|(key, _, _)| *key == source)
}

pub fn known_name(source: &str) -> Option<&'static str> {
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


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_names_keep_known_npc_variants_and_unknown_input_bytes() {
        assert_eq!(name("Amulet"),"护身符");
        assert_eq!(known_name("amulet"),None);
        let (key,translation)=NPCS.iter().find(|(key,_)| key.contains('_')).unwrap();
        assert_eq!(name(key),*translation);
        assert_eq!(name(&key.replace('_'," ")),*translation);
        assert_eq!(name(&key.replace('_'," - ")),*translation);
        assert_eq!(name(" Unknown Item ")," Unknown Item ");
        assert!(known_monster("Scarecrow").is_some());
        assert_eq!(known_monster("scarecrow"),None);
    }
}
