# 装备、战斗、怪物追击与掉落修复（2026-10-25）

范围为玩家反馈的武器替换、双戒指、戒指拆分菜单、怪物围拢/绕障、掉落重叠，以及随后追加的半月弯刀、双击穿戴和玩家攻击入口。以下为当前源码的本地验证，不代表线上版本已经更新。

## 原因和修改

1. **武器替换与物品分类**：服务端原本支持交换已装备武器，并把旧武器返回新武器原背包格。前端兼容背包只依赖装备槽或英文名称，缺少槽位字段的中文/`crystal-item-N` 装备可能误走消耗品菜单。现复用真实物品类型分类；武器仍通过已有的实例 UID、装备 ledger 和权威快照完成交换。
2. **双戒指**：个人 UseItem 和 Web 两个自动装备入口此前固定使用默认戒指槽。现按 Crystal 选择右空槽，否则左槽；两边满时替换左槽。显式 EquipItem 仍遵从指定目标槽。
3. **戒指拆分**：装备不再出现拆分操作，也不能因同款物品 key 相同而进入合并路径。拆分确认重新校验可堆叠类型及数量范围。
4. **怪物追击**：共享 Zone 的普通玩家、诱饵、召唤物与绑定目标追击原先只尝试正前一格。现直行前最多前视8格到攻击邻格，前方受阻时做半径8、最多256候选格的确定性寻路，返回一格合法步伐；预算内无法找到路径时再按原版转向规则兜底。每步仍检查共享占用、静态碰撞与移动冷却；固定方向的特殊怪物限制保留，无新增持久状态。该有界寻路是针对玩家反馈的本服增强，区别于 Crystal 原版单纯转向；避免已复现的 L 形和半包围障碍两格往返。
5. **掉落散开**：共享 Zone 原先把所有默认位置掉落放在怪物死亡格。现复用 Crystal 的半径4扩圈顺序，先选空格，再选物品最少的合法格，每格最多5件；避开阻挡实体、墙体和传送点，保留实际载荷、归属与拾取权限。

原版依据：`Crystal/Client/MirControls/MirItemCell.cs` 自动装备选择、`Crystal/Server/MirObjects/PlayerObject.cs` 装备交换、`Crystal/Server/MirObjects/MonsterObject.cs::MoveTo`、`Crystal/Server/MirObjects/ItemObject.cs::Drop`、`Crystal/Server/Settings.cs` 的 DropRange/DropStackSize。

## 本轮验证

| 命令（在 mir2-web3 下执行） | 结果 |
| --- | --- |
| `cargo test -p mir2-simulation --lib equipment_replacement_tests -- --nocapture` | 4/4：旧武器原格返回且UID/属性保留、双戒指及第三枚替换、诅咒左戒指时填空右槽、显式右槽替换 |
| `cargo test -p mir2-simulation --lib ground_drop -- --nocapture` | 33/33：含5项新散落/碰撞测试及现有拾取、归属、UID和checkpoint回归 |
| `cargo test -p mir2-simulation --lib equip_` | 25/25：装备交换、UID保存重登、需求/绑定/诅咒、右槽及仓库装备等邻接回归 |
| `cargo test -p mir2-simulation --lib native_entity_` | 22/22：绑定目标、宠物、状态控制等邻接回归 |
| `cargo test -p mir2-simulation --test shared_zone_monster_movement` | 7/7：绕墙、L形及半包围障碍不往返、8怪围满8邻格不重叠、广播方向/冷却、全封堵、真实骷髅及野怪追宠物 |
| `cargo test -p mir2-simulation --test shared_zone` | 209/209，共享地图集成回归 |
| `cargo test -p mir2-simulation --lib inventory_use_item_equips_metadata_backed_exact_zero` | 1/1，真实UID0自动装备回归 |
| `node --no-global-search-paths --test scripts/test-inventory-equipment-actions.mjs scripts/test-equipment-gateway-adapter.mjs scripts/test-equipment-session-controller.mjs scripts/test-equipment-page-gate.mjs scripts/test-equipment-wasm.mjs`（在apps/web执行） | 51/51，实际页面函数和既有真实WASM装备ledger验证，含武器发送/ACK/交换快照与三个戒指入口 |
| `node ./node_modules/typescript/bin/tsc --project <repo>/work/gameplay-bugfix-20261025/inventory-ui/tsconfig-source-check.json`（在apps/web执行） | exit0；继承原配置，仅检查app/lib/types与next-env真实源码，含page.tsx；未修改用户tsconfig或next-env |

验证过程中修复了新增掉落模块的可见性和NPC夹具字段。新增绕墙测试最初尝试在自定义碰撞地图恢复checkpoint，被现有真实地图重建校验拒绝；已改为同地图同输入双实例比较确定性，不修改checkpoint协议。既有占用测试原断言受阻怪物原地不动，与本次绕障目标冲突；保留不重叠断言并新增合法侧移及方向广播断言后，209项通过。

独立复核发现原转向修复仍在三格L形障碍前两格往返；加入有界寻路后又发现直走快路会撤回半包围场景所需的退让。两项回归均先失败，补齐前视后7项通过，协调者重新运行最终源码的共享地图209项及新增7项全部通过。前端旧页面测试夹具缺失新增宿主依赖，补齐真实分类器和空闲领域状态；保留旧owner/storage无proof拒绝覆盖。新增native戒指夹具因缺少投影字段被正确拒绝，修正夹具字段后51项通过，没有放松产品门禁。

## 本地组件UI smoke

使用当前真实React菜单、拆分面板和InventoryWindow函数，在独立本地浏览器页操作：已持武器主点击发出weapon替换；左已戴戒指时主点击选择ringRight，右已戴时菜单穿戴选择ringLeft；戒指菜单无拆分；3瓶药拆2仅产生一次确认回调。页面不连接游戏服务器。

- [装备与戒指菜单](generated/player-qa/gameplay-bugfix-20261025/inventory-equipped-rings-fixture.jpg)
- [药水拆分面板](generated/player-qa/gameplay-bugfix-20261025/inventory-potion-split-fixture.jpg)
- [操作回调结果](generated/player-qa/gameplay-bugfix-20261025/inventory-callback-results-fixture.jpg)

## 追加：半月弯刀不可用

本轮确认并修复了实际输入链路上的多处问题：

- Web关闭技能的SpellToggle回执曾把castKind改为passive，再次开启时无法恢复。现将canUse与castKind分开保存，原生CombatHost和兼容页面都接收当前本人回执；已认证的基线缓存同步更新，控制器重绑不恢复旧开关值。
- 原生CombatHost原先忽略SpellToggle，导致快捷键每次读取旧状态、重复发送相同开关。真实技能栏三次点击现发送1→0→1，始终保留toggle类型。仅接收当前连接/角色的严格类型回执；异主、旧连接、缺字段和重复spell拒绝。
- 客户端把半月、狂风和刺杀的免费开关误套MP成本门。现这三种持久开关在0MP也能切换，烈火与双龙等预备技能的成本门保留。本轮未改半月每次攻击的MP结算；已知该结算仍需单独核对共享Zone和个人资源提交，不能据此宣称法力完全对齐。
- 服务端自动选招曾让刺杀/随机攻杀覆盖已开启的半月。现按Crystal的覆盖顺序选择当前支持的招式：烈火、双龙、狂风、半月、刺杀、攻杀；显式选择仍受已学会/已启用校验。
- 网关方向攻击原先只找正前方目标，且在寻敌后才解析已开启招式。现先解析权威招式，再按原版半月扇形（正前及旋转-1/+1/+2）寻敌，狂风使用完整邻圈，保留正前格优先。未开启的半月或普通攻击不能扩大范围；仍使用共享位置，不能退回过期个人怪物位置。
- 选中侧面怪物时，半月/狂风的侧面伤害原来会立即发生。现与其他侧面目标统一在300ms后结算，正前格结算保持原有行为。

来源核对：`Crystal/Client/MirScenes/GameScene.cs` 的技能开关、`Crystal/Client/MirObjects/PlayerObject.cs` 的普攻招式覆盖顺序，以及既有共享Zone原版攻击格定义。

| 本轮检查 | 结果 |
| --- | --- |
| `cargo test -p mir2-gateway --lib zone_halfmoon_routing_tests` | 5/5；含真实客户端Attack包在正前为空时进入共享半月攻击并收到目标伤害结算，另覆盖半月/狂风范围、正前优先和关闭后伪报拒绝。最初4项中2项先失败，修后通过 |
| `cargo test -p mir2-gateway --lib melee` | 9/9；既有寻敌、共享权威位置、坐骑、PvP及刺杀练功邻接回归 |
| `cargo test -p mir2-gateway --lib gateway_twin_drake_shared_mana_commits_once_and_survives_refresh` | 1/1，预备技能支付与刷新回归 |
| `cargo test -p mir2-simulation --lib combat_halfmoon_profile_tests` | 2/2；多开优先级与关闭后恢复刺杀，先失败后修复 |
| `cargo test -p mir2-simulation --test shared_zone_halfmoon_timing --test shared_zone_thrusting --test shared_zone` | 1/1新时序、10/10刺杀/半月邻接、209/209共享地图；侧选时序先失败后修复 |
| `npm run test:half-moon`（在apps/web执行） | 64/64；实际页面/CombatHost/回执捕获，含旧owner、畸形回执、重绑基线、被动技能保留和原有发送安全门 |
| `cargo +1.95.0 test --manifest-path apps/game-client/client-bevy/Cargo.toml --target-dir target/halfmoon-client --lib combat_input` | 17/17；含0MP持久开关双向切换及预备技能成本门保留，独立target未修改旧缓存链接 |
| Web完整源码TypeScript | exit0，未修改用户tsconfig或next-env |

新增真实React技能栏本地浏览器验证：[半月连续开关](generated/player-qa/gameplay-bugfix-20261025/halfmoon-toggle-fixture.png)。该页运行真实技能栏与当前页面开关函数，证明点击产生正确开关请求；它不是已连接游戏服务器的真人游玩证据。网关实际攻击至共享结算由上述Rust集成测试覆盖。

未执行新客户端WASM构建、发布、生产服务重启或真人完整游玩验收。客户端Rust改动须重建WASM并与Web/网关更新后才生效；不据此调整全项目百分比、Candidate100或Accepted。

## 追加：双击穿戴、攻击玩家与两端共用边界

- Web共享Bevy背包新增同一格、同一UID在500ms内的第二次点击，复用现有Use/Equip请求；首击仍可查看物品。换主、布局/物品变化、失焦、取消和实际拖动会撤销候选。先选择A移动后再双击B不会同时发送Move与Equip。
- Web兼容React背包保留原有点击入口，并处理浏览器的两次click加一次dblclick事件序列，防止重复发送；修理、仓库、出售、删除和拆分上下文继续遵从原门禁。未增加必须按U的步骤。
- 玩家攻击使用原版Shift加左键，保留近战接近与弓箭远程规则。松开Shift、失焦、退出页面或取消会停止玩家追击；不能把旧目标在再次按Shift后自动恢复。服务器继续决定模式、距离、安全区及伤害。
- Web新增可见的六种攻击模式选择及当前模式、实际配置快捷键和Shift操作提示。选择经现有共享模式控制器和发送证明，界面等待服务器ChangeAMode回执，不提前伪造切换成功。模式按钮的鼠标、方向键、Enter、Space和Escape不会误传到世界移动/攻击入口。
- Web战斗快照ABI升级为2携带forceAttack；对旧ABI1运行时仍发送原字段形状，保留原普通战斗。旧运行时选择玩家时显示更新客户端提示，不把未知字段强塞进旧WASM。
- Windows原生输入独立补齐真实窗口鼠标入口及QuestUiIntent桥接的实时Shift复核，带Alt或没有Shift的玩家攻击拒绝；松Shift清除本地追击与待发攻击，但保留已经提交、尚待服务器回执的移动。原生Ctrl+H及原有模式回执显示继续使用共享CombatModes。
- Windows原生背包另接真实WindowEvent短点击识别，使用系统双击间隔，调用与原Use按钮同一条UseInspected路径；实际拖出再返回不算双击。保留UID0、精确实例、装备pending、药品300ms冷却、邮件锁和NPC/仓库等上下文门禁。检查时发现该路径原先固定发送显式左戒指槽，因此服务器自动选槽修复不会自动覆盖它；现仅自动Use/双击在可靠戒指元数据下选择右空槽8，否则左槽7，显式EquipInspected目标保持不变。

**共用范围**：服务器权威世界、协议、物品/装备读模型及部分Rust战斗算法（世界点击解析、追击决策和攻击间隔）共用。Web的DOM/WASM宿主、便携背包，与Windows的窗口输入、原生背包和桥接分别接线。CombatSnapshot.force_attack当前由Web宿主使用，Windows不靠该宿主驱动输入；修复和验收不能凭“共用Rust”跨端推定。

| 本轮检查 | 结果 |
| --- | --- |
| `npm run test:player-controls`（apps/web） | 110/110，包含真实页面发送、Shift状态与会话退役、ABI1/2兼容、模式控件事件隔离和双击去重 |
| `node --test scripts/test-inventory-equipment-actions.mjs scripts/test-item-identity.mjs scripts/test-stage5-adapters.mjs`（apps/web） | 38项Node测试通过；Stage5另报506个内部逻辑组，不能将逻辑组当作额外Node测试累加；与控件110项存在重叠 |
| `cargo +1.95.0 test --manifest-path apps/game-client/client-bevy/Cargo.toml --features portable-quest-ui --target-dir target/halfmoon-client portable_bag_ui::tests --lib` | 37/37，含移动A后双击B只移动、触摸/拖动、UID0、物品变更和失焦 |
| 同一client-bevy命令将过滤器改为`combat_input` | 22/22，含5项新增玩家强制攻击/释放回归及既有半月等战斗检查 |
| `cargo +1.95.0 test --manifest-path apps/game-client/runtime/Cargo.toml --features webgl2-shared-ui --target-dir target/halfmoon-client combat_input_host::tests --lib` | 4通过、1项既有ignored；运行时能力声明及快照宿主检查 |
| `cargo test -p mir2-gateway --lib zone_player_attack_mode_tests`，以及`--lib pvp` | 新增3/3、既有2/2：真实双会话客户端包切全体后对方掉血并向双方广播；和平拒绝；攻击方或目标在安全区均拒绝 |
| `cargo +1.95.0 test --manifest-path apps/game-client/platform-windows/Cargo.toml --target-dir target/halfmoon-client --config 'profile.dev.opt-level=0' --config 'profile.dev.package."*".opt-level=0' native_player_ --bin mir2-platform-windows` | 6/6，其中5项新增Windows实际鼠标到桥接的玩家攻击测试，1项既有测试；无ignored |
| 运行上述本轮Windows测试binary，分别过滤`input::tests`、`gameplay_bridge::tests::attack_target_forwarding`和`gameplay_bridge::tests::crystal_click` | 138/138、2/2、2/2，无ignored；与聚焦测试存在重叠，不累计为不同测试数量 |
| `cargo +1.95.0 test --manifest-path apps/game-client/client-bevy/Cargo.toml --features native-ui --target-dir target/halfmoon-client native_inventory_double_click --lib` | 8/8，无ignored；真实玩家窗口事件到Use/Equip队列，含双击/Use按钮×槽位/tooltip元数据×四种戒指占位的16组合、pending阻重、显式槽保留及无元数据不按名字猜 |
| 运行本轮原生背包测试binary的10个聚焦邻接过滤器，详见下方日志 | 33/33，无ignored；覆盖精确装备实例/重复按键、可靠类型、共享药品冷却、背包/NPC拖放、仓库及邮件锁 |
| Web完整源码TypeScript，noEmit、pretty=false、incremental=false | exit0，最终源码零诊断；未修改用户tsconfig或next-env |

独立复核发现并修复了“移动A再双击B多发装备”和“模式按钮焦点下方向键仍移动角色”两处问题。旧测试夹具补齐真实分类器、依赖或DOM closest方法，原身份、owner与发送门禁断言保留。截图使用真实React组件的独立本地页，覆盖展开、服务器回执确认和同步中禁用；不连接游戏服务器：

- [攻击模式展开](generated/player-qa/gameplay-bugfix-20261025/combat-mode-controls-expanded.png)
- [回执确认全体模式](generated/player-qa/gameplay-bugfix-20261025/combat-mode-controls-confirmed.png)
- [来源同步中禁用](generated/player-qa/gameplay-bugfix-20261025/combat-mode-controls-syncing.png)

原始输出：[Web控件110项](generated/player-qa/gameplay-bugfix-20261025/pvp-doubleclick-20261025-web-final.log)、[物品身份与Stage5](generated/player-qa/gameplay-bugfix-20261025/pvp-doubleclick-20261025-identity-stage5-final.log)、[便携背包37项](generated/player-qa/gameplay-bugfix-20261025/pvp-doubleclick-20261025-bag-final.log)、[Windows攻击及邻接](generated/player-qa/gameplay-bugfix-20261025/native-pvp-regression.log)、[原生背包双击8项](generated/player-qa/gameplay-bugfix-20261025/native-bag-doubleclick-regression.log)、[原生背包邻接33项](generated/player-qa/gameplay-bugfix-20261025/native-bag-adjacent-regression.log)。Windows测试生成的是测试binary，尚非新游戏EXE交付或窗口实玩验收；Web也未重建新WASM/完整包或发布。只记录本轮有界验证，不调整全项目百分比。

本轮原生依赖首次编译因E盘空间耗尽失败。仅将本会话创建的`target/halfmoon-client`缓存移至C盘Temp并在原位置建立经过目标校验的junction后恢复测试；核对9614个文件、21,385,647,399字节，未移动源码或历史缓存。该路径变化只影响本机可再生编译缓存。
