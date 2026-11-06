# 地图装备外观静态审计（2026-09-21）

本报告只读当前仓库资源/源码与已有证据；未启动GUI、未改资源或共享源码。StateItem纸娃娃资源214帧已经独立核验通过（见同目录items-audit.md），本报告的地图body/weapon动作图未通过完整视觉验收。库存在仅指至少有一个atlas rect或有PNG及metadata，不代表完整动作、双性别、同场景绘制通过。

## 来源与映射

- 完整目录：packages/game-data/data/generated/crystal_item_manifest.json，1628项；当前platinum_176 v26白名单195名称，匹配195项。
- 运行映射：apps/simulation/src/runtime/equipment.rs:904优先template.shape；packets.rs:7995起生成sprite。衣服CArmour/{shape:02}；武器shape<100→CWeapon；100..199→AWeapon/{shape-100} R和L；>=200→ARWeapon/{shape-200}及S。
- C身体/头发男偏移0、女808；C武器男0、女416。Assassin持100..199武器时另用AArmour/AHair，偏移男0女512；Archer持>=200武器时另用ARArmour/ARHair，偏移男0女352。原生atlas.rs:798起按动作选择alt；骑乘可屏蔽武器。
- allowedClasses为Warrior/Wizard/Taoist。下面按目录本身shape统计，不把class_based/level_based实际变体选择当成已验证；变体对应实际穿戴实例、特殊外观、mount/transform完整矩阵均unknown。
- native atlas.rs:588玩家库识别明确包括C/A/AR的Armour/Hair/Weapon和Mount；:626解析PNG路径；:730起查metadata和真实文件，:738起解码standalone PNG。因此不进starter atlas不等于不支持，前提是资产根确实有文件及metadata。

## 分母与最低库存在性

|范围|武器/衣服定义数|至少所需直接库存在|缺至少一直接库|
|---|---:|---:|---:|
|platinum 武器|30|3|27|
|platinum 衣服|24|2|22|
|完整目录 武器|180|20|160|
|完整目录 衣服|180|26|154|

完整目录其余1268项不是武器/衣服，未将其计为缺bodyLibrary；头盔等纸娃娃层属于StateItem审计。A/AR衣服是职业和所持武器联合条件，不是每件衣服强制要求；下面单独列出现有alt库，不把它们错误计为普通三职业分母。

## 按实际shape归组的直接装备库

Source列来自原版快照清单presentFrameCount，属于已有源清单证据，本次未重新解码原始Lib。PNG/meta是当前仓库public物理计数；Atlas是rect索引计数，不代表PNG已逐像素核验。

|库|全目录定义数|白名单定义数|Source帧|PNG|meta帧|Atlas帧|
|---|---:|---:|---:|---:|---:|---:|
|ARWeapon/00 S|5|0|704|704|704|0|
|ARWeapon/00|5|0|832|728|704|0|
|ARWeapon/01 S|2|0|704|704|704|0|
|ARWeapon/01|2|0|832|728|704|0|
|ARWeapon/03 S|2|0|704|0|0|0|
|ARWeapon/03|2|0|832|0|0|0|
|ARWeapon/04 S|2|0|704|0|0|0|
|ARWeapon/04|2|0|832|0|0|0|
|ARWeapon/05 S|1|0|704|0|0|0|
|ARWeapon/05|1|0|832|0|0|0|
|ARWeapon/07 S|3|0|704|0|0|0|
|ARWeapon/07|3|0|832|0|0|0|
|ARWeapon/08 S|2|0|704|0|0|0|
|ARWeapon/08|2|0|832|0|0|0|
|ARWeapon/09 S|1|0|704|0|0|0|
|ARWeapon/09|1|0|832|0|0|0|
|ARWeapon/10 S|2|0|704|0|0|0|
|ARWeapon/10|2|0|832|0|0|0|
|ARWeapon/11 S|2|0|704|0|0|0|
|ARWeapon/11|2|0|832|0|0|0|
|ARWeapon/12 S|2|0|704|0|0|0|
|ARWeapon/12|2|0|832|0|0|0|
|ARWeapon/13 S|1|0|704|0|0|0|
|ARWeapon/13|1|0|832|0|0|0|
|ARWeapon/15 S|1|0|704|0|0|0|
|ARWeapon/15|1|0|832|0|0|0|
|ARWeapon/16 S|1|0|704|0|0|0|
|ARWeapon/16|1|0|832|0|0|0|
|ARWeapon/17 S|1|0|704|0|0|0|
|ARWeapon/17|1|0|832|0|0|0|
|ARWeapon/18 S|1|0|704|0|0|0|
|ARWeapon/18|1|0|832|0|0|0|
|ARWeapon/19 S|2|0|704|0|0|0|
|ARWeapon/19|2|0|832|0|0|0|
|ARWeapon/20 S|1|0|704|0|0|0|
|ARWeapon/20|1|0|832|0|0|0|
|ARWeapon/21 S|1|0|704|0|0|0|
|ARWeapon/21|1|0|832|0|0|0|
|ARWeapon/22 S|1|0|704|0|0|0|
|ARWeapon/22|1|0|832|0|0|0|
|ARWeapon/23 S|1|0|704|0|0|0|
|ARWeapon/23|1|0|832|0|0|0|
|ARWeapon/24 S|1|0|704|0|0|0|
|ARWeapon/24|1|0|832|0|0|0|
|ARWeapon/25 S|1|0|704|0|0|0|
|ARWeapon/25|1|0|832|0|0|0|
|ARWeapon/26 S|1|0|704|0|0|0|
|ARWeapon/26|1|0|832|0|0|0|
|AWeapon/00 L|6|0|1024|976|976|976|
|AWeapon/00 R|6|0|1024|976|976|976|
|AWeapon/01 L|2|0|1024|976|976|0|
|AWeapon/01 R|2|0|1024|976|976|0|
|AWeapon/02 L|3|0|1024|0|0|0|
|AWeapon/02 R|3|0|1024|0|0|0|
|AWeapon/03 L|2|0|1024|0|0|0|
|AWeapon/03 R|2|0|1024|0|0|0|
|AWeapon/04 L|2|0|1024|0|0|0|
|AWeapon/04 R|2|0|1024|0|0|0|
|AWeapon/05 L|1|0|1024|0|0|0|
|AWeapon/05 R|1|0|1024|0|0|0|
|AWeapon/06 L|2|0|1024|0|0|0|
|AWeapon/06 R|2|0|1024|0|0|0|
|AWeapon/07 L|1|0|1024|0|0|0|
|AWeapon/07 R|1|0|1024|0|0|0|
|AWeapon/08 L|1|0|1024|0|0|0|
|AWeapon/08 R|1|0|1024|0|0|0|
|AWeapon/09 L|1|0|1024|0|0|0|
|AWeapon/09 R|1|0|1024|0|0|0|
|AWeapon/10 L|2|0|1024|0|0|0|
|AWeapon/10 R|2|0|1024|0|0|0|
|AWeapon/11 L|3|0|1024|0|0|0|
|AWeapon/11 R|3|0|1024|0|0|0|
|AWeapon/13 L|1|0|1024|0|0|0|
|AWeapon/13 R|1|0|1024|0|0|0|
|AWeapon/14 L|2|0|1024|0|0|0|
|AWeapon/14 R|2|0|1024|0|0|0|
|AWeapon/15 L|1|0|1024|0|0|0|
|AWeapon/15 R|1|0|1024|0|0|0|
|AWeapon/16 L|1|0|1024|0|0|0|
|AWeapon/16 R|1|0|1024|0|0|0|
|AWeapon/17 L|1|0|1024|0|0|0|
|AWeapon/17 R|1|0|1024|0|0|0|
|AWeapon/18 L|1|0|1024|0|0|0|
|AWeapon/18 R|1|0|1024|0|0|0|
|CArmour/00|4|0|1616|1264|1264|1264|
|CArmour/01|2|2|1616|1264|1264|0|
|CArmour/02|8|4|1616|0|0|0|
|CArmour/03|8|4|1616|0|0|0|
|CArmour/04|8|4|1616|0|0|0|
|CArmour/05|8|4|1616|0|0|0|
|CArmour/06|2|2|1616|0|0|0|
|CArmour/07|2|2|1616|0|0|0|
|CArmour/08|2|2|1616|0|0|0|
|CArmour/09|8|0|1616|1616|1616|0|
|CArmour/10|12|0|1616|1616|1616|0|
|CArmour/11|12|0|1616|0|0|0|
|CArmour/12|2|0|1616|0|0|0|
|CArmour/13|2|0|1616|0|0|0|
|CArmour/14|2|0|1616|0|0|0|
|CArmour/15|2|0|1616|0|0|0|
|CArmour/16|2|0|8080|0|0|0|
|CArmour/17|2|0|8080|0|0|0|
|CArmour/18|2|0|8080|0|0|0|
|CArmour/19|12|0|8080|0|0|0|
|CArmour/20|4|0|8080|0|0|0|
|CArmour/21|4|0|8080|0|0|0|
|CArmour/22|2|0|8080|0|0|0|
|CArmour/23|2|0|8080|0|0|0|
|CArmour/24|2|0|8080|0|0|0|
|CArmour/29|12|0|8080|0|0|0|
|CArmour/30|12|0|6464|0|0|0|
|CArmour/35|4|0|1616|0|0|0|
|CArmour/36|4|0|1616|0|0|0|
|CArmour/37|2|0|1616|0|0|0|
|CArmour/38|2|0|1616|0|0|0|
|CArmour/39|2|0|1616|0|0|0|
|CArmour/41|2|0|1616|0|0|0|
|CArmour/57|12|0|1616|0|0|0|
|CArmour/58|12|0|1616|0|0|0|
|CWeapon/00|2|2|832|832|832|832|
|CWeapon/01|3|1|832|832|832|832|
|CWeapon/02|4|2|832|0|0|0|
|CWeapon/03|3|1|832|0|0|0|
|CWeapon/04|3|2|832|0|0|0|
|CWeapon/05|2|1|832|0|0|0|
|CWeapon/06|2|1|832|0|0|0|
|CWeapon/07|2|1|832|0|0|0|
|CWeapon/08|2|1|832|0|0|0|
|CWeapon/09|2|1|832|0|0|0|
|CWeapon/10|2|0|832|0|0|0|
|CWeapon/11|2|0|832|0|0|0|
|CWeapon/12|1|0|832|0|0|0|
|CWeapon/13|4|0|832|0|0|0|
|CWeapon/14|1|1|832|0|0|0|
|CWeapon/15|1|1|832|0|0|0|
|CWeapon/16|2|1|832|0|0|0|
|CWeapon/17|2|1|832|0|0|0|
|CWeapon/18|2|1|832|0|0|0|
|CWeapon/19|1|1|832|0|0|0|
|CWeapon/20|1|0|832|0|0|0|
|CWeapon/21|2|2|832|0|0|0|
|CWeapon/22|2|1|832|0|0|0|
|CWeapon/23|2|1|832|0|0|0|
|CWeapon/25|1|0|832|0|0|0|
|CWeapon/26|1|1|832|0|0|0|
|CWeapon/27|1|1|832|0|0|0|
|CWeapon/28|1|1|832|0|0|0|
|CWeapon/29|2|1|832|0|0|0|
|CWeapon/30|2|1|832|0|0|0|
|CWeapon/31|2|1|832|0|0|0|
|CWeapon/32|1|0|832|0|0|0|
|CWeapon/33|1|0|832|0|0|0|
|CWeapon/34|1|0|832|0|0|0|
|CWeapon/35|1|0|832|0|0|0|
|CWeapon/36|1|0|832|0|0|0|
|CWeapon/37|1|0|832|0|0|0|
|CWeapon/39|1|0|832|0|0|0|
|CWeapon/40|1|0|832|0|0|0|
|CWeapon/41|4|0|832|0|0|0|
|CWeapon/42|2|1|832|0|0|0|
|CWeapon/45|4|0|832|0|0|0|
|CWeapon/46|1|0|832|0|0|0|
|CWeapon/47|1|0|832|0|0|0|
|CWeapon/48|1|0|832|0|0|0|
|CWeapon/49|1|0|832|0|0|0|
|CWeapon/50|1|0|832|0|0|0|
|CWeapon/51|1|0|832|0|0|0|
|CWeapon/52|1|0|832|0|0|0|
|CWeapon/53|1|0|832|0|0|0|
|CWeapon/54|1|0|832|0|0|0|
|CWeapon/55|2|0|832|0|0|0|
|CWeapon/56|2|0|832|0|0|0|
|CWeapon/57|2|0|832|0|0|0|
|CWeapon/60|1|0|832|0|0|0|
|CWeapon/73|1|0|832|0|0|0|
|CWeapon/74|1|0|832|0|0|0|
|CWeapon/75|1|0|832|0|0|0|
|CWeapon/76|4|0|832|0|0|0|
|CWeapon/77|4|0|832|0|0|0|
|CWeapon/78|4|0|832|0|0|0|

## 当前普通装备与已知六库缺口

当前自然装备声明apps/web/scripts/asset-pipeline/natural-equipment-sprites.json及9/15历史保存证据对应如下。它不是新建角色starter发装清单，也不是当前在线角色保存快照。只有Scimitar在当前platinum白名单；其余五件历史装备不能冒充当前白名单发放。

|历史职业配置|衣服ID与名称|武器ID与名称|库|
|---|---|---|---|
|Warrior|1221 ThickArmour(M)|1216 MartialSabre|CArmour/03 + CWeapon/09|
|Wizard|1223 FireMagicRobe(M)|1217 SpearWithHook|CArmour/04 + CWeapon/10|
|Taoist|1190 SolidArmour(M)|268 Scimitar|CArmour/02 + CWeapon/07|

以上六库在当前public和starter atlas均缺失。历史9/15证据记载外部r2包曾补足7344个PNG；该报告明确写未拷回版本树。这不构成当前仓库或所有新包资源通过证明。本次未检查该外部包当前状态。

六库影响的所有目录物品（括号内为当前白名单与否）：

- CArmour/02: 84 BoneRobe(M) [非白名单]；85 BoneRobe(F) [非白名单]；319 LightArmour(M) [白名单]；320 LightArmour(F) [白名单]；321 MediumArmour(M) [白名单]；322 MediumArmour(F) [白名单]；1190 SolidArmour(M) [非白名单]；1191 SolidArmour(F) [非白名单]
- CArmour/03: 323 HeavyArmour(M) [白名单]；324 HeavyArmour(F) [白名单]；333 IronArmour(M) [白名单]；334 IronArmour(F) [白名单]；1221 ThickArmour(M) [非白名单]；1222 ThickArmour(F) [非白名单]；1246 FineIronArmour(M) [非白名单]；1247 FineIronArmour(F) [非白名单]
- CArmour/04: 325 MagicRobe(M) [白名单]；326 MagicRobe(F) [白名单]；335 WizardRobe(M) [白名单]；336 WitchRobe(F) [白名单]；1223 FireMagicRobe(M) [非白名单]；1224 FireMagicRobe(F) [非白名单]；1248 FireRobe(M) [非白名单]；1249 FireRobe(F) [非白名单]
- CWeapon/07: 268 Scimitar [白名单]；1198 SharpScimitar [非白名单]
- CWeapon/09: 239 MartialSword [白名单]；1216 MartialSabre [非白名单]
- CWeapon/10: 255 HookedSpear [非白名单]；1217 SpearWithHook [非白名单]

## 现有职业alt衣服资产

|库|Source帧|PNG|meta帧|Atlas帧|
|---|---:|---:|---:|---:|
|AArmour/00|1024|1024|1024|1024|
|AArmour/01|1024|1024|1024|0|
|ARArmour/00|704|704|704|0|
|ARArmour/01|704|704|704|0|

## 结论与边界

StateItem材料通过不等于角色地图外观通过。当前三职业白名单54件武器/衣服只有5件满足最低库存在性（WoodenSword、EbonySword、Dagger、BaseDress(M)、BaseDress(F)），49件缺直接库。地图装备补齐应按完整白名单和全目录分别建立完整方向/动作/双性别资源闭包，再做真实穿戴与同场景渲染验证。已有natural-equipment闭包脚本只覆盖声明的三个普通男角色，并非全部物品审计。
