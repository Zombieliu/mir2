# 全量物品资源审计 — 2026-09-21

本轮只读核验现有资源/代码，新增独立审计脚本与结果；未修改游戏资源、未重启游戏、未提交。逐帧结果见 `items-audit.json`。原版比较源由命令参数指定，本次为 `E:/mir2/Crystal/Build/Client/Debug/Data`。

## 结果与分母

| 范围 | 已解析目录物品 | Items所需帧 | DNItems问题帧 | StateItem所需帧 |
|---|---:|---:|---:|---:|
| 完整导入目录 | 1628行 | 924 | 34 | 214（421行武器/衣服/头盔） |
| platinum_176白名单 | 195 | 127 | 0 | 46（56行） |
| 所有NPC静态Trade库存 | 934名称 | 706 | 17 | 169 |
| platinum允许NPC静态Trade库存 | 96名称 | 97 | 0 | 39 |
| 所有导入掉落表 | 776名称 | 598 | 7 | 159 |
| platinum怪物同名掉落表 | 178名称 | 144 | 0 | 51 |
| platinum掉落覆盖 | 40名称 | 7 | 0 | 0 |
| 导入任务携带物/收集任务物 | 71名称 | 60 | 2 | 0 |

Items背包/商店图：1628行、913个目录Image，加11个护身符堆叠阈值Image，共924帧；全部存在、可解码、有非透明像素，并与原版Items.Lib逐像素及元数据哈希一致。现有 `verify-item-icon-closure.mjs` 通过；它确实覆盖完整目录，而非新手几件，但本身不覆盖地面图、StateItem、运行时路径或角色外观。

StateItem装备页纸娃娃图：完整目录武器/衣服/头盔421行214帧全部通过原版像素、尺寸和偏移核验。三种原版Lib的文件SHA256都与导出metadata记录一致。Crystal CharacterDialog只以StateItems画这三种纸娃娃层，其余装备格使用Items；因此未把1207行非纸娃娃物品错误计为缺失StateItem。

DNItems地面图：924物品帧，加金币112–116（其中一帧已有），共928帧全部有文件/metadata；894帧非空且逐像素匹配。33帧为原版已存在的完全透明4×1帧，导出像素仍完全一致；另外119（MysteryWater）源帧为空尺寸，当前PNG头宽度无效，sharp无法解码。这不是34帧漏导出，不能靠重导一遍获得实际图。34帧影响102个目录物品行，未命中当前195项platinum白名单。完整列表在JSON。包括任务SnakeBody/1400、AncientTree/1401，以及DragonTorch、Lantern、多种鱼、时装/变身类、药水/珍珠。原版空内容仅说明源一致，不证明策划有意让它不可见。

现有 `verify-quest-item-icon-closure.mjs` 为71任务物品/60帧通过，仅检查Items文件与metadata存在；本次额外发现上述2个地面空帧，因此任务闭包通过不等同落地显示全部通过。

## 运行时缺口与具体建议

1. **Image=0已追踪，不列为实际背包BUG。** 完整导入目录15行Image=0且Items/0.png有效。虽然 `inventory.rs:330` 的 `item_icon_path(0)` 返回None，搜索仅发现函数定义与测试调用；实际UI走 `ItemModel::user_item_image_index → concrete_item_image_index → original_item_image_bundle`，存在tooltipSource时保留Some(0)并加载Items/0.png，只有无来源的legacy零值被过滤。不能用未调用的旧helper声称15项实际不可见。15项为[H]WarGodOil、Invitation、Feather、PigEar、PigHoof、CannibalPoison、DemonLeather、UndeadLeather、MammothLeather、BeastLeather、LightLeather、BaboonLeather、RhinoLeather、AncientLeather、CookBook。
2. `platform-windows/src/atlas.rs:1440` 实际读取groundDrops.image并画DNItems；`ground_item_frame_size:1625` 对无可见像素返回None。上述34空图会没有地面精灵，名称可仍存在。建议空源帧登记为来源例外；若产品要求有图，需明确新的美术/映射策略，不能宣称原版漏导。119应在导出链避免生成非法零宽PNG，并保留可审计空帧语义。
3. 原版CharacterDialog对武器/衣服先调用GetRealItem再取Image，头盔直接Info.Image；本项目 `simulation/src/runtime/equipment.rs:164` 的state_image来自基础template.image。完整StateItem素材通过，仍不能证明class_based/level_based装备选中了正确变体；需真实不同职业/等级装备快照核对。Gateway从StateItem/meta.json取几何并传到原生纸娃娃渲染，说明这些素材确实有运行路径。
4. Items或StateItem通过不代表地图上的穿戴外观通过：bodyLibrary/weaponLibrary、shape、性别/职业映射、动作帧与方向是独立资产和代码路径，本报告未计入完成。

## 未覆盖/未解析引用（不能藏在通过率里）

- 所有NPC Trade提取1040唯一名称，其中106不能按精确名称在目录解析；platinum允许脚本97名称有1项 `BladesofVelocity` 未精确解析。需按运行时大小写/别名规则和profile过滤再验证，不能直接断言106个运行商店坏物品。
- 所有掉落表1307唯一名称中531不能精确解析，包含Gold和大小写/历史内容；platinum同名怪物掉落表179名称中仅Gold不是ItemInfo（正常独立金币包），五档金币地面帧已全部核验。未解释嵌套/随机掉落表可达性。
- 154条导入任务的carry_items/item_tasks已覆盖；二进制奖励payload、动态GIVE脚本、可选newcomer奖励叠加未在本脚本解码，不能当成所有任务奖励引用100%。完整1628目录资源检查仍覆盖其中能落到该目录的物品图。
- 未核对运行中的EXE实际加载哪一份资源包、安装目录副本或运行截图。文件及源像素一致不代表遮挡、缩放、偏移、层级和实际屏幕显示验收通过。

复现：从仓库根运行 `node apps/web/scripts/audit-all-item-assets-20260921.mjs <Crystal客户端Data目录>`；只写同目录JSON报告，不写资源。
