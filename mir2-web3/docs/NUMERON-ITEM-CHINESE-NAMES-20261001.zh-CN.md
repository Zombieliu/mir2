# numeron 物品与装备中文名称对照

核对日期：2026 年 10 月 1 日。本文用于当前 Windows 版本的游戏介绍和物品目录，覆盖三职业常规白名单的 195 种物品及新手旅程额外引用的 15 种成长装备，共 **210 种模板**；其中基础装备 100 种，技能书 63 种。男女衣服分别计数。

装备条件、基础属性和配置范围见[原始物品目录](E:/mir2-player-journey/mir2-web3/docs/NUMERON-ITEM-CATALOG-20261001.zh-CN.md)。名称范围依据[内容档案](E:/mir2-player-journey/mir2-web3/packages/game-data/data/content_profiles/platinum_176.json)、[物品模板库](E:/mir2-player-journey/mir2-web3/packages/game-data/data/generated/crystal_item_manifest.json)和[新手旅程配置](E:/mir2-player-journey/mir2-web3/config/quest-guidance/newcomer-journey-v2.json)。配置收录不等于每件物品都已开放普通获取或通过实玩验收。

## 名称依据

中文名优先采用可核对的历史中英对照和项目已有译名。其他版本资料按代码含义、图标、等级和基础属性辅助核对，标为“资料推定”；名称仍随版本可能不同。没有可靠版本对应的条目标为“暂译”，可供介绍草稿使用，正式发布前应确定名称。

| 名称依据 | 模板数 | 含义 |
| --- | ---: | --- |
| 历史对照 | 96 | 历史中英清单有对应名称，常见笔误按其他资料核对 |
| 资料推定 | 16 | 其他中文版本资料与当前模板信息对应；属于跨版本推定 |
| 暂译 | 22 | 根据英文标识与项目语境拟定，待确认 |
| 项目已有 | 18 | 保留当前客户端简表中的中文名称 |
| 项目技能译名 | 58 | 取项目完整中文技能资源的首行名称 |

表中依据可点击查看来源。历史清单存在旧拼写和译名差异；只用于名称参考，外部网站的属性、职业限制及掉落来源不能替代本项目配置。

## 已核对的显示名差异

`WarMageStaff` 在历史中英资料中对应“骨玉权杖”，`SoulSpringWand` 对应“无极棍”；当前客户端简表分别写成“战斗法师之杖”和“灵泉法杖”。游戏介绍建议使用表中的经典名称。[历史中英对照][H2]

`ThunderBolt` 对应“雷电术”，`Lightning` 对应“疾光电影”。项目完整中文技能资源也区分两者，但客户端简表把 `Lightning` 写成了“雷电术”。另外，`MagicShield`、`SoulShield`、`BlessedArmour` 的经典名称分别为“魔法盾”“幽灵盾”“神圣战甲术”。[历史中英对照][H2]；[项目中文资源][P2]

36 级的 `WarSpiritBlade`、`MagicScythe`、`StoneBambooFan` 按同图标、等级和基础属性资料，分别采用“战灵之刃”“魔法镰刀”“玉竹扇”；40 级的 `SteelArmour`、`DragonRobe`、`TitanArmour` 采用“钢盔甲”“圣龙魔袍”“泰坦战衣”。这些属于当前模板的版本译名核对。[武器资料][W1]；[护甲资料][W2]

新手成长装备保留项目现用名称。即使复用了经典装备的图标，也按独立模板列出。

## 武器与矿镐 39 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 木剑 | `WoodenSword` | [历史对照][H1] |
| 匕首 | `Dagger` | [历史对照][H1] |
| 乌木剑 | `EbonySword` | [历史对照][H1] |
| 青铜剑 | `BronzeSword` | [历史对照][H1] |
| 短剑 | `ShortSword` | [历史对照][H1] |
| 铁剑 | `IronSword` | [历史对照][H1] |
| 青铜斧 | `BronzeAxe` | [历史对照][H1] |
| 钢铁斧 | `SteelAxe` | [资料推定][W1]；图标 67，16 级，攻击 3–13 |
| 凌风 | `SteelSword` | [历史对照][H1] |
| 八荒 | `HookedSword` | [历史对照][H1] |
| 斩马刀 | `MartialSword` | [历史对照][H1] |
| 修罗 | `PowerAxe` | [历史对照][H1] |
| 凝霜 | `PurifierSword` | [资料推定][W1]；图标 45，25 级，攻击 10–13；旧表有“凌霜”笔误 |
| 炼狱 | `GreatAxe` | [历史对照][H1] |
| 祖玛裁决之杖 | `ZumaJudgementMace` | [资料推定][W1]；图标 55，29 级，攻击 0–33，攻速 -2 |
| 裁决之杖 | `JudgementMace` | [历史对照][H2] |
| 井中月 | `DragonSword` | [历史对照][H2] |
| 战灵之刃 | `WarSpiritBlade` | [资料推定][W1]；图标 69，36 级，攻击 5–32 |
| 屠龙 | `DragonSlayer` | [历史对照][H2] |
| 海魂 | `Trident` | [历史对照][H1] |
| 魔杖 | `MageStaff` | [历史对照][H2] |
| 骨玉权杖 | `WarMageStaff` | [历史对照][H2]；当前客户端简表称“战斗法师之杖” |
| 魔法镰刀 | `MagicScythe` | [资料推定][W1]；图标 70，36 级，攻击 5–16、魔法 1–8 |
| 嗜魂法杖 | `DragonStaff` | [历史对照][H2] |
| 半月 | `Scimitar` | [历史对照][H1] |
| 银蛇 | `SerpentSword` | [历史对照][H2] |
| 无极棍 | `SoulSpringWand` | [历史对照][H2]；当前客户端简表称“灵泉法杖” |
| 玉竹扇 | `StoneBambooFan` | [资料推定][W1]；图标 71，36 级，攻击 2–16、道术 3–6 |
| 龙纹剑 | `SoulSabre` | [历史对照][H2] |
| 鹤嘴锄 | `PickAxe` | [历史对照][H1] |
| 锋利长剑 | `SharpSword` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |
| 锋利三叉戟 | `SharpTrident` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |
| 锋利弯刀 | `SharpScimitar` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |
| 战斗马刀 | `MartialSabre` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |
| 钩镰枪 | `SpearWithHook` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |
| 锐利波刃剑 | `KeenKrissSword` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |
| 坚固巨斧 | `SolidGreatAxe` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |
| 坚固青铜法杖 | `SolidBronzeStaff` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |
| 坚固蛇形剑 | `SolidSerpentSword` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |

## 衣服与护甲 30 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 布衣（男） | `BaseDress(M)` | [历史对照][H1] |
| 布衣（女） | `BaseDress(F)` | [历史对照][H1] |
| 轻型盔甲（男） | `LightArmour(M)` | [历史对照][H1] |
| 轻型盔甲（女） | `LightArmour(F)` | [历史对照][H1] |
| 中型盔甲（男） | `MediumArmour(M)` | 暂译；按代码含义；图标与轻型盔甲共用，不合并两种模板 |
| 中型盔甲（女） | `MediumArmour(F)` | 暂译；按代码含义；图标与轻型盔甲共用，不合并两种模板 |
| 重盔甲（男） | `HeavyArmour(M)` | [历史对照][H1] |
| 重盔甲（女） | `HeavyArmour(F)` | [历史对照][H1] |
| 魔法长袍（男） | `MagicRobe(M)` | [历史对照][H1] |
| 魔法长袍（女） | `MagicRobe(F)` | [历史对照][H1] |
| 灵魂战衣（男） | `SoulArmour(M)` | [历史对照][H1] |
| 灵魂战衣（女） | `SoulArmour(F)` | [历史对照][H1] |
| 战神盔甲（男） | `IronArmour(M)` | [历史对照][H2] |
| 战神盔甲（女） | `IronArmour(F)` | [历史对照][H2] |
| 恶魔长袍（男） | `WizardRobe(M)` | [历史对照][H2] |
| 恶魔长袍（女） | `WitchRobe(F)` | [资料推定][W2]；历史资料写作 WizardRobe(F)；按当前女性模板核对 |
| 幽灵战衣（男） | `PearlArmour(M)` | [历史对照][H2] |
| 幽灵战衣（女） | `PearlArmour(F)` | [历史对照][H2] |
| 钢盔甲（男） | `SteelArmour(M)` | [历史对照][H1] |
| 钢盔甲（女） | `SteelArmour(F)` | [历史对照][H1] |
| 圣龙魔袍（男） | `DragonRobe(M)` | [资料推定][W2]；旧中英表译“龙袍”；同图标、40 级和基础属性对应此译名 |
| 圣龙魔袍（女） | `DragonRobe(F)` | [资料推定][W2]；旧中英表译“龙袍”；同图标、40 级和基础属性对应此译名 |
| 泰坦战衣（男） | `TitanArmour(M)` | [历史对照][H1] |
| 泰坦战衣（女） | `TitanArmour(F)` | [历史对照][H1] |
| 厚重盔甲（男） | `ThickArmour(M)` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |
| 厚重盔甲（女） | `ThickArmour(F)` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |
| 火焰法袍（男） | `FireMagicRobe(M)` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |
| 火焰法袍（女） | `FireMagicRobe(F)` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |
| 道士长袍（男） | `TaoArmour(M)` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |
| 道士长袍（女） | `TaoArmour(F)` | [项目已有][P1]；新手成长奖励装备，保留项目现用名 |

## 头盔 2 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 青铜头盔 | `BronzeHelmet` | [历史对照][H1] |
| 魔法头盔 | `MagicHelmet` | [历史对照][H1] |

## 项链 5 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 金项链 | `GoldNecklace` | [历史对照][H1] |
| 传统项链 | `PrecisionNecklace` | [资料推定][W3]；图标 237，3 级，准确 +1；旧表代码为 PrecisionNeck |
| 黑水晶项链 | `BlackNecklace` | [历史对照][H1] |
| 黑檀项链 | `EbonyNecklace` | [历史对照][H1] |
| 黄水晶项链 | `YellowNecklace` | [历史对照][H1] |

## 手镯与手套 8 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 铁手镯 | `IronBracelet` | [历史对照][H1] |
| 小手镯 | `ThinBracelet` | [历史对照][H1] |
| 皮制手套 | `LeatherGlove` | [历史对照][H1] |
| 银手镯 | `SilverBracelet` | [历史对照][H1] |
| 钢手镯 | `SteelBracelet` | [历史对照][H1] |
| 大手镯 | `LargeBracelet` | [历史对照][H1] |
| 坚固手套 | `HardGlove` | [历史对照][H1] |
| 魔法手镯 | `MagicBracelet` | [历史对照][H1] |

## 戒指 14 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 古铜戒指 | `CopperRing` | [历史对照][H1] |
| 六角戒指 | `HexagonalRing` | [历史对照][H1] |
| 玻璃戒指 | `GlassRing` | [历史对照][H1] |
| 牛角戒指 | `HornRing` | [历史对照][H1] |
| 生铁戒指 | `IronRing` | [历史对照][H1] |
| 白玉戒指 | `WhiteJadeRing` | [资料推定][W5]；图标 512；与任务物品 JadeRing 分开 |
| 蓝水晶戒指 | `BlueRing` | [历史对照][H1] |
| 骷髅戒指 | `SkeletonRing` | [历史对照][H1] |
| 蛇眼戒指 | `SerpentEyeRing` | [历史对照][H1] |
| 珍珠戒指 | `PearlRing` | [历史对照][H1] |
| 珊瑚戒指 | `CoralRing` | [历史对照][H1] |
| 红宝石戒指 | `RubyRing` | [历史对照][H2] |
| 铂金戒指 | `PlatinumRing` | [历史对照][H1]；历史资料亦写“白金戒指” |
| 龙之戒指 | `DragonRing` | [历史对照][H2] |

## 腰带 1 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 皮腰带 | `LeatherBelt` | 暂译 |

## 鞋子 1 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 低帮鞋 | `LowShoes` | 暂译 |

## 道士符咒与毒药 3 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 绿毒粉 | `GreenPoison` | [项目已有][P1]；保留当前项目红、绿毒粉叫法 |
| 红毒粉 | `RedPoison` | [项目已有][P1]；保留当前项目红、绿毒粉叫法 |
| 护身符 | `Amulet` | [项目已有][P1] |

## 护身符包 1 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 护身符包 | `Amulet(Bundle)` | 暂译 |

## 照明用品 2 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 蜡烛 | `Candle` | [历史对照][H1] |
| 火把 | `Torch` | [历史对照][H1] |

## 生命与魔法药品 6 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 金创药（小量） | `(HP)DrugSmall` | [历史对照][H1]；客户端称“小型红药” |
| 魔法药（小量） | `(MP)DrugSmall` | [历史对照][H1]；客户端称“小型蓝药” |
| 金创药（中量） | `(HP)DrugMedium` | [历史对照][H1]；客户端称“中型红药” |
| 魔法药（中量） | `(MP)DrugMedium` | [历史对照][H1]；客户端称“中型蓝药” |
| 强效金创药 | `(HP)DrugLarge` | [历史对照][H1]；客户端称“大型红药” |
| 强效魔法药 | `(MP)DrugLarge` | [历史对照][H1]；客户端称“大型蓝药” |

## 卷轴与功能油 6 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 修复油 | `RepairOil` | [历史对照][H1] |
| 战神油 | `WarGodOil` | [历史对照][H1] |
| 祝福油 | `BenedictionOil` | [历史对照][H1] |
| 随机传送卷 | `RandomTeleport` | [历史对照][H1] |
| 地牢逃脱卷 | `DungeonEscape` | [历史对照][H1] |
| 回城卷 | `TownTeleport` | [历史对照][H1] |

## 肉类 4 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 鸡肉 | `Chicken` | [历史对照][H1] |
| 鹿肉 | `Venison` | 暂译；食用肉类；其他版本同图标称“干肉”，未确认相同物品 |
| 羊肉 | `Mutton` | 暂译 |
| 肉 | `Meat` | [历史对照][H1] |

## 材料与杂物 12 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 羽毛 | `Feather` | 暂译 |
| 金栗子 | `GoldChestnut` | [资料推定][W14]；图标 482；名称核对不代表外部网站效果适用于本项目 |
| 食人树叶 | `CannibalLeaf` | [历史对照][H1] |
| 食人树果实 | `CannibalFruit` | [历史对照][H1] |
| 蜘蛛牙齿 | `SpiderTeeth` | [资料推定][W14]；其他版本称“毒蜘蛛牙齿”；图标 253 |
| 蜘蛛网 | `SpiderWeb` | [资料推定][W14]；图标 854 |
| 蝎子尾巴 | `ScorpionTail` | [历史对照][H1] |
| 红丝线 | `RedThread` | [资料推定][W14]；图标 802 |
| 黑丝线 | `BlackThread` | [资料推定][W14]；图标 803 |
| 赤月碎片 | `RedMoonChip` | 暂译；其他版本同图标 444 称“血剑碎块”，版本对应待确认 |
| 幽灵油 | `EvilApeOil` | [历史对照][H2]；历史中英表译“幽灵油”；米尔资料同图标称“邪恶巨人油” |
| 幽灵心 | `EvilApeHeart` | [历史对照][H2]；历史中英表译“幽灵心”；米尔资料同图标称“邪恶巨人心脏” |

## 任务物品 12 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 鹿肉（任务） | `DeerMeat` | 暂译；与食用肉类 Venison 为不同模板 |
| 食人树毒液 | `CannibalPoison` | 暂译；任务物品 |
| 玉戒指（任务） | `JadeRing` | 暂译；与装备 WhiteJadeRing 分开 |
| 秘方 | `SecretRecipe` | 暂译；任务物品 |
| 奥莉维娅的戒指 | `OliviasRing` | 暂译；人物任务物品 |
| 失窃的金币 | `StolenGold` | 暂译；任务物品 |
| 破旧斧头 | `WornAxe` | 暂译；任务物品，不计作可穿戴武器 |
| 比奇故事·卷一 | `BichonTales(1)` | 暂译 |
| 比奇故事·卷二 | `BichonTales(2)` | 暂译 |
| 比奇故事·卷三 | `BichonTales(3)` | 暂译 |
| 尸花 | `CorpsFlower` | 暂译；按任务语境暂译；保留原始代码拼写 |
| 洁净骷髅头 | `CleanSkull` | 暂译 |

## 其他物品 1 种

| 中文名 | 代码标识 | 依据与备注 |
| --- | --- | --- |
| 石心 | `StoneHeart` | 暂译；其他类模板；不可按同图标直接认作 EvilApeHeart |

## 技能书 63 种

技能书使用技能中文名。战士 15 种、法师 23 种、道士 25 种；其中部分是扩展技能，不能把全部 63 种统称为经典国服 1.76 技能。名称取自项目中文资源，不据名称推断技能效果。

### 战士 15 种

| 技能书中文名 | 代码标识 | 学习等级 | 依据与备注 |
| --- | --- | ---: | --- |
| 基本剑术 | `Fencing` | 7 | [项目技能译名][P2] |
| 攻杀剑术 | `Slaying` | 15 | [项目技能译名][P2] |
| 刺杀剑术 | `Thrusting` | 22 | [项目技能译名][P2] |
| 半月弯刀 | `HalfMoon` | 26 | [项目技能译名][P2] |
| 野蛮冲撞 | `ShoulderDash` | 30 | [项目技能译名][P2] |
| 双龙斩 | `TwinDrakeBlade` | 32 | [项目技能译名][P2] |
| 捕蝇剑 | `Entrapment` | 32 | [项目技能译名][P2]；沿用中文资源首行“捕蝇剑”，译法较特殊，正式命名待确认 |
| 烈火剑法 | `FlamingSword` | 35 | [项目技能译名][P2] |
| 狮子吼 | `LionRoar` | 36 | [项目技能译名][P2] |
| 狂风斩 | `CrossHalfMoon` | 38 | [项目技能译名][P2] |
| 空破闪 | `BladeAvalanche` | 38 | [项目技能译名][P2] |
| 护身气幕 | `ProtectionField` | 39 | [项目技能译名][P2] |
| 狂暴 | `Rage` | 44 | [项目技能译名][P2] |
| 嗜血 | `Fury` | 45 | [项目技能译名][P2] |
| 反击 | `CounterAttack` | 47 | [项目技能译名][P2] |

### 法师 23 种

| 技能书中文名 | 代码标识 | 学习等级 | 依据与备注 |
| --- | --- | ---: | --- |
| 火球术 | `FireBall` | 7 | [项目技能译名][P2] |
| 抗拒火环 | `Repulsion` | 12 | [项目技能译名][P2] |
| 诱惑之光 | `ElectricShock` | 13 | [项目技能译名][P2] |
| 大火球 | `GreatFireBall` | 15 | [项目技能译名][P2] |
| 地狱火 | `HellFire` | 16 | [项目技能译名][P2] |
| 雷电术 | `ThunderBolt` | 17 | [项目技能译名][P2]；雷电术的代码；勿与 Lightning 混写 |
| 瞬息移动 | `Teleport` | 19 | [项目技能译名][P2] |
| 爆裂火焰 | `FireBang` | 22 | [项目技能译名][P2] |
| 火墙 | `FireWall` | 24 | [项目技能译名][P2] |
| 疾光电影 | `Lightning` | 26 | [历史对照][H2]；客户端简表译“雷电术”，完整中文资源为“疾光电影”；与 ThunderBolt 分开 |
| 寒冰掌 | `FrostCrunch` | 28 | [项目技能译名][P2] |
| 地狱雷光 | `ThunderStorm` | 30 | [项目技能译名][P2] |
| 魔法盾 | `MagicShield` | 31 | [历史对照][H2]；项目中文资源译“魔法护盾” |
| 圣言术 | `TurnUndead` | 32 | [项目技能译名][P2] |
| 噬血术 | `Vampirism` | 33 | [项目技能译名][P2] |
| 冰咆哮 | `IceStorm` | 35 | [项目技能译名][P2] |
| 灭天火 | `FlameDisruptor` | 38 | [项目技能译名][P2] |
| 分身术 | `Mirroring` | 41 | [项目技能译名][P2] |
| 闪现 | `Blink` | 41 | [项目技能译名][P2] |
| 火龙气焰 | `FlameField` | 42 | [项目技能译名][P2] |
| 天霜冰环 | `Blizzard` | 44 | [项目技能译名][P2] |
| 魔力强化 | `MagicBooster` | 47 | [项目技能译名][P2] |
| 流星打击 | `MeteorStrike` | 49 | [项目技能译名][P2] |

### 道士 25 种

| 技能书中文名 | 代码标识 | 学习等级 | 依据与备注 |
| --- | --- | ---: | --- |
| 治愈术 | `Healing` | 7 | [项目技能译名][P2] |
| 精神力战法 | `SpiritSword` | 9 | [项目技能译名][P2] |
| 施毒术 | `Poisoning` | 14 | [项目技能译名][P2] |
| 灵魂火符 | `SoulFireBall` | 18 | [项目技能译名][P2] |
| 召唤骷髅 | `SummonSkeleton` | 19 | [项目技能译名][P2] |
| 隐身术 | `Hiding` | 20 | [项目技能译名][P2] |
| 集体隐身术 | `MassHiding` | 21 | [历史对照][H2]；项目中文资源译“群体隐身术” |
| 幽灵盾 | `SoulShield` | 22 | [历史对照][H2]；项目中文资源译“灵魂护盾” |
| 心灵启示 | `Revelation` | 23 | [项目技能译名][P2] |
| 神圣战甲术 | `BlessedArmour` | 25 | [历史对照][H2]；项目中文资源译“祝福铠甲” |
| 气功波 | `EnergyRepulsor` | 27 | [项目技能译名][P2] |
| 困魔咒 | `TrapHexagon` | 28 | [项目技能译名][P2] |
| 净化术 | `Purification` | 30 | [项目技能译名][P2] |
| 群体治疗术 | `MassHealing` | 31 | [项目技能译名][P2] |
| 迷魂术 | `Hallucination` | 31 | [项目技能译名][P2] |
| 终极强化 | `UltimateEnhancer` | 33 | [项目技能译名][P2] |
| 召唤神兽 | `SummonShinsu` | 35 | [项目技能译名][P2] |
| 复活术 | `Reincarnation` | 37 | [项目技能译名][P2] |
| 召唤圣灵 | `SummonHolyDeva` | 38 | [项目技能译名][P2] |
| 诅咒术 | `Curse` | 40 | [项目技能译名][P2] |
| 瘟疫术 | `Plague` | 42 | [项目技能译名][P2] |
| 毒雾 | `PoisonCloud` | 43 | [项目技能译名][P2] |
| 宠物强化 | `PetEnhancer` | 45 | [项目技能译名][P2] |
| 阴阳五行阵 | `HealingCircle` | 45 | [项目技能译名][P2] |
| 能量护盾 | `EnergyShield` | 48 | [项目技能译名][P2] |

## 暂译与特殊译法

22 种暂译条目已在相应表格中标明。`Venison` 与 `DeerMeat` 都可译为鹿肉，但一个属于肉类模板，一个属于任务模板；`JadeRing` 也不能和装备 `WhiteJadeRing` 合并。

`RedMoonChip` 暂译“赤月碎片”，其他版本同图标称“血剑碎块”；`EvilApeOil`、`EvilApeHeart` 的历史译名为“幽灵油”“幽灵心”，其他版本同图标使用“邪恶巨人油”“邪恶巨人心脏”。图标复用不能单独证明物品身份，目录保留了这些差异。[材料资料][W14]；[历史清单][H1]

技能 `Entrapment` 沿用项目中文资源中的“捕蝇剑”，这属于特殊源译名，正式命名待确认；其他扩展技能名称也应结合本项目的技能效果审定。[项目中文资源][P2]

本次只整理介绍用名称并补充来源，没有修改客户端显示逻辑、物品代码标识、平衡数值或玩家存档。

## 来源

- [历史中英物品与技能清单][H1]、[新浪传奇专区英文名称对照][H2]：用于核对经典名称，旧表中的拼写与笔误已结合其他资料处理。
- [米尔百科武器][W1]、[护甲][W2]、[项链][W3]、[手镯][W4]、[戒指][W5]、[头盔][W6]、[消耗品][W12]、[材料][W14]：用于辅助比对图标、等级和基础属性，属于其他版本参考。
- [当前客户端中文简表][P1]、[项目完整中文技能资源][P2]：用于保留项目已有译名并核对显示名差异。

[H1]: https://www.moon-soft.com/program/bbs/readelite880263.htm "2003 年历史中英物品与技能清单"
[H2]: https://games.sina.com.cn/zhuanqu/legend/xinde/yingwenmingcheng.shtml "新浪传奇专区历史英文名称对照"
[W1]: https://mir2.xin/index.php?s=Home/Index/stuff2/id/1 "米尔百科武器资料，含后续分页"
[W2]: https://mir2.xin/index.php?s=Home/Index/stuff2/id/2 "米尔百科护甲资料，含后续分页"
[W3]: https://mir2.xin/index.php?s=Home/Index/stuff2/id/3 "米尔百科项链资料，含后续分页"
[W4]: https://mir2.xin/index.php?s=Home/Index/stuff2/id/4 "米尔百科手镯资料，含后续分页"
[W5]: https://mir2.xin/index.php?s=Home/Index/stuff2/id/5 "米尔百科戒指资料，含后续分页"
[W6]: https://mir2.xin/index.php?s=Home/Index/stuff2/id/6 "米尔百科头盔资料，含后续分页"
[W12]: https://mir2.xin/index.php?s=Home/Index/stuff2/id/12 "米尔百科消耗品资料，含后续分页"
[W14]: https://mir2.xin/index.php?s=Home/Index/stuff2/id/14 "米尔百科材料资料，含后续分页"
[P1]: E:/mir2-player-journey/mir2-web3/apps/game-client/client-bevy/src/player_text.rs:308 "当前客户端中文简表"
[P2]: E:/mir2-player-journey/mir2-web3/packages/game-data/data/generated/localization_bundle.json "项目完整中文技能资源"

外部米尔百科资料按 2026 年 10 月 1 日公开页面核对；它是其他版本的参考资料。中文名称可借鉴，具体玩法以本项目代码为准。
