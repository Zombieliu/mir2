# Web R16 本地试玩安全证据（2026-11-04）

## v12-v13 共享新手与真实持久化

[当前交付及操作](../../WEB-R16-PLAYABLE-HANDOFF-20261104.zh-CN.md#v12-v13-windows-与-web-共用新手基础)已采用 Windows R16 的同一内容/任务配置与共享 Quest UI。[34份安全副本和19份源码pin](onboarding-v13/summary.json)记录来源、SHA、各组测试与原360MiB预算失败；不把不同运行拼成同一次全流程通过。

**最终前三项已闭环：** [完整分期收据](onboarding-v13/v13-complete-receipt.json)与[r11原轮](onboarding-v13/v13-r11-third-quest-complete.json)记录最后一只猫、4/4、回城Native交付、+150金币/+1Fencing、精确重登及正常退出；最后5checks通过、HTTP/实际failed request/console/driver error均0，10条warning保留，6条取消均有同路径成功。前三项由多轮正常续验合并证明，r8的partial不改写。当前后续任务是第4项职业练习；本节不宣称全部新手线通过。

[最终发布检查](onboarding-v13/final-publication-validation.json)核对48份副本/13,551,169 B、14张Root实际检视图和112个本地文档链接，当前入口health200、发行manifest未变；启动文档76/114行。运行中的私有数据、凭证和SQLite不包含在这些公开副本中。

| 证据 | 实际范围 |
| --- | --- |
| [v11真实点击回归](onboarding-v13/v11-r4-pointer-regression.json) | 原“查看任务详情”同时触发5次走路、坐标288,616→283,613；保留原失败状态 |
| [v12按钮与首任务](onboarding-v13/v12-r3-pointer-and-first-quest.json)、[任务卡](onboarding-v13/native-task-card.png)、[原生详情](onboarding-v13/native-task-detail.png) | 同帧可见性观察及本地按钮输入占用修复；实际3次开关详情、0次移动/转向、位置不变；该续验后段helper失败，整体false不改写 |
| [首任务奖励图](onboarding-v13/first-quest-reward.png)、[第二任务原轮](onboarding-v13/v12-r5-second-quest.json)、[第二任务完成图](onboarding-v13/second-quest-completed.png) | 普通接取/交付匹配ACK/CompleteQuest，100金币/木剑/10药；第二项普通装备自有木剑及另100金币；原轮随后围攻/关包诊断失败保留 |
| [v12真实续验](onboarding-v13/v12-r6-resume-revive-persistence.json)、[保存日记](onboarding-v13/saved-progress-diary.png) | HP0/30正常回城复活、8次真实用药10→2、可见X关包8/8；再普通重登保留q1/q2完成与q3稻草人2/2、资产不变，UI不重发已领奖励；两只猫未完成，整体false |
| [v13普通商店原轮](onboarding-v13/v13-r7-native-purchase.json)、[点击前原生商店图](onboarding-v13/native-shop-before-purchase.png) | 单次当前Native购买、同operation提交/完整snapshot/UID匹配，200→80金币、小药0→3；r7随后因怪物动态占位的driver判断退出，原轮false保留，不把点击前图片当已购证据 |
| [v13有界续验](onboarding-v13/v13-r8-hunt-and-persistence.json)、[分项汇总](onboarding-v13/v13-r8-scoped-receipt.json)、[实际狩猎](onboarding-v13/v13-r8-natural-hunt.png)、[重登日记](onboarding-v13/v13-r8-saved-diary.png) | 新登录确认已购3药/80金币/木剑保留，Digit1实际用药3次；猫1/2、q3达3/4，低血无药撤退中死亡，正常复活/重登精确保存、最后正常logout。该阶段仍false/partial，不改成整线通过；[6份增量副本SHA](onboarding-v13/v13-continuation-pins.json)独立于前34份 |
| [r9第二笔购买](onboarding-v13/v13-r9-second-purchase.json)、[r10路点诊断](onboarding-v13/v13-r10-waypoint-diagnostic.json) | 新operation单次2药/80金币，随后driver未关商店导致药键未发；r10额外不可达路点0步即停止。原false保留；普通可见X关店、回用已走通路线后续验，没有重买两瓶药 |
| [最终4/4待交](onboarding-v13/native-third-quest-ready.png)、[交付后章节推进](onboarding-v13/native-third-quest-completed.png)、[重登日记](onboarding-v13/native-three-quests-after-relogin.png)、[最终7份增量SHA](onboarding-v13/v13-final-pins.json) | 本人命中1122→死亡1125→4/4；回城finish1361/ACK1377/CompleteQuest；最后用药2→1/HP14→24，资产150金币/1小药/1Fencing/原木剑、HP60/60保存。图中的章节3/4指首3项已完成、下一项职业练习，不是q3仍缺一只 |
| [真实404映射](onboarding-v13/v12-r6-resource-misses.json)、[v13生成](onboarding-v13/v13-asset-generation.json)、[31项资源/打包](onboarding-v13/v13-resource-tests.log) | 4唯一缺图共16次404，补完整Monster/016 22PNG及meta；普通13类出生白名单不改，原21033有限资产pin全部保持，有限闭包21056项/281159572B |
| [v12发行](onboarding-v13/v12-package-integrity-proof.json)、[v13独立发行](onboarding-v13/v13-package-integrity-proof.json) | v12重编Next/Quest WASM；v13仅22新增PNG及2处清单/同语义meta差异，全29681项/921176670B普通文件、大小/SHA和无链接通过；所有JS/WASM/Gateway/Node/Actor保持 |
| [v12数据保全](onboarding-v13/v12-runtime-preservation-proof.json)、[v13数据保全](onboarding-v13/v13-runtime-preservation-proof.json) | 两次均正常停服再切换，v13的25private/19观战文件8174201251B逐文件相同；完整旧目录均保留 |
| [原角色保护字段](onboarding-v13/original-account-preservation-proof.json)、[原阿洛只读复核](onboarding-v13/original-alo-readonly-proof.json) | 切换前12份旧存档任务行均空、身份资产18类字段不变；普通原账号登录只读找回阿洛v2/三习惯/10金币2药/40金币一瓶返程及原教学，4批80经历，模型调用2→2、0次越界指令、正常logout |
| [阿洛习惯](onboarding-v13/original-alo-policy.png)、[既有补给](onboarding-v13/original-alo-supply.png)、[资源](onboarding-v13/original-alo-resources.png) | 三张实际面板图，固定身份行黑色遮罩；Root逐张检视，未教学、激活授权、交易或请求模型 |
| [Windows入口更新](onboarding-v13/native-baseline-update-proof.json) | 仅三份公共启动模块及BUILD索引；新默认下次正常启动生效。旧472项不变、全部475项复核；没有新EXE、私有读写或Native旧任务迁移声明 |
| [18项基线](onboarding-v13/baseline-tests.log)、[26项相邻](onboarding-v13/baseline-adjacent-tests.log)、[70项源码](onboarding-v13/v11-source-tests.log)、[91项聚焦](onboarding-v13/v12-focused-tests.log) | 共享配置、Native默认/显式历史优先、Page三处同profile、旧13卡不自动、严格world-action拒绝localUI；各组有交集不相加 |
| [Simulation V2 23项](onboarding-v13/v2-simulation-tests.log)、[旧持久化3项](onboarding-v13/legacy-persistence-tests.log) | 现有服务器任务/正确地图与接受后击杀/奖励/五职业资格及持久化回归；不是所有职业真实UI游玩 |
| [Rust红证](onboarding-v13/quest-observer-red-tests.log)、[新增4/4](onboarding-v13/quest-observer-focused-tests.log)、[完整相邻13/14](onboarding-v13/quest-observer-adjacent-tests.log) | 新回归在旧源码实红；唯一旧实体稳定性断言前后均失败，未修改或放宽；不称完整14/14 |
| [生产构建](onboarding-v13/production-build.log)、[三renderer构建](onboarding-v13/renderer-build.log) | 原ABI/字节/gzip/JS预算内；Native locked/offline host检查因既有锁漂移失败、未改锁，未重建EXE |

本次只读泄漏扫描的4个QA禁止值留在Node进程内，不参与浏览器选择器；所复制文字精确匹配为0。账号文件、角色存档、SQLite、模型凭证、owner导出和活跃二维码未收录。已公开PNG均由Root检视，保留游戏场景、遮盖固定身份与QA显示名。更早脚本选择器/超时诊断留在本机原目录，不追改为成功。共享WASM完整十语、全新手线、全职业/地图/长期稳定性、同存档跨端和人类Accepted仍开放。

## v10 语言界面与运行兼容

[当前交付](../../WEB-R16-PLAYABLE-HANDOFF-20261104.zh-CN.md#v10语言界面实际对齐与剩余差距)已更新原19041入口。[汇总与20份副本哈希](language-v10/summary.json)明确各轮边界；不把十语菜单当作全量共享WASM翻译，也不拼成新一轮战斗/交易验收。

| 证据 | 范围 |
| --- | --- |
| [十语菜单](language-v10/ten-language-menu.png) | Native顺序/标签、240宽及逐项同源字型；空凭据登录画面，非模型生成图 |
| [简中附加设置](language-v10/browser-settings-zh-CN.png)、[Hindi](language-v10/browser-settings-hi.png)、[Arabic](language-v10/browser-settings-ar.png) | 登录及附加设置实际DOM/本地字体；滑块正常显示，账号/密码黑色遮罩；阿拉伯文本不翻转游戏坐标 |
| [十语矩阵原轮](language-v10/browser-r3-ten-locale-scoped.json) | 十语登录/钱包标题/设置、俄语真实刷新保存、fresh zh-HK初选繁中及选角姓名保持均为正证；该轮整体仍为false，因为只等5s未完成冷进城，不改写状态 |
| [俄语运行原轮](language-v10/browser-r4-ru-native-scoped.json)、[Native Bag](language-v10/native-bag-ru.png)、[队伍](language-v10/group-ru.png) | 放宽冷加载观察后普通StartGame result4/MapInformation/UserInformation、实际Bag/Group/阿洛读取可用；整体false保留，未把第二次伙伴入口超时算通过 |
| [中文及清晰登录图库](language-v10/browser-r5-chinese-and-gallery.json) | 当前中文普通入城、只读阿洛及正常登出通过；前轮十语/俄语只引用命名正证，不继承前轮整体成功或后述遮罩安全声明 |
| [最终中文阿洛与退出](language-v10/browser-r6-final.json)、[该轮执行脚本](language-v10/r6-executed-browser-smoke.mjs) | 当前3项与前轮6项引用分开；真实末次刷新、同档/培养v2/3习惯/10金币2药、原采购40/1/返程及20条当前页记录保持，模型计数不变，正常LogOutSuccess；HTTP/实际failed request/pageerror/console.error均0，7条warning仅哈希保留，取消的资源路径另有成功响应 |
| [习惯](language-v10/alo-policy-zh-CN.png)、[原补给](language-v10/alo-original-supply-zh-CN.png)、[资源](language-v10/alo-resources-zh-CN.png) | 最终只读伙伴面板图；未教学、授权、消费或请求模型；稳定身份行遮罩，业务记录可读 |
| [发行完整性](language-v10/package-integrity-proof.json) | 29657文件/920318539B全大小/SHA/普通文件/nlink/ReparsePoint检查；新BuildID；原25460 public及164非Next闭包pin保持。旧v5的21019资产pin保持，仅+14字体/OFL |
| [数据保全](language-v10/runtime-preservation-proof.json) | 正常停服后25个private及9个观战文件逐字节相同，旧v9目录保留；不公布私有值/文件内容 |
| [源码与预算](language-v10/build-source-pins.json)、[构建](language-v10/production-build.log)、[65项聚焦](language-v10/focused-tests.log)、[62项相邻](language-v10/adjacent-tests.log) | 17份源码pin、Next/TS成功、WASM未改；Native词库9902×10及8个已知源slot例外、浏览器32×10。另19项资源/字体检查通过。prepared682352660B仍未过360MiB thin gate，不更名为thin成功 |

QA遮罩器的早期版本误把仅用于Node泄漏扫描的禁止值同时用于本机自有浏览器的选择器；未写入公开产物或发送网站请求。r6已将Node-only值与固定显示身份遮罩分开，去掉全DOM复杂匹配，执行脚本/公开JSON扫描无禁止值；这一安全声明只属于修正后的r6。r5执行副本及更早大小写/networkidle/冷加载/截图等待失败均在本地原目录保留，没有改写。没有读取模型API凭证或修改产品密钥配置。

上述截图由Root逐张检视，副本与源逐SHA一致；未复制SQLite、账号文件、密钥、owner导出或活跃二维码。共享WASM/原位图/服务端及部分自定义面板十语仍开放，最终人类Accepted和全局Candidate不变。

## 当前 v9 钱包扫码修复

[实机浏览器收据](wallet-login-v9/wallet-v9-browser-proof.json)：真实二维码生成/轮询200、取消停止轮询、账号入口恢复、原角色与阿洛v2/10金币2药保留；未代替用户签名钱包。[独立完整性](wallet-login-v9/wallet-v9-package-integrity-proof.json)验证29642文件及原JS/WASM/素材不变；[运行数据保全](wallet-login-v9/wallet-v9-runtime-preservation-proof.json)记录正常停止和逐文件哈希一致。二维码图只留在本机private-qa，不复制到本目录。

签名配置和代理/TLS修复范围、未验的本人签名步骤见[当前交付](../../WEB-R16-PLAYABLE-HANDOFF-20261104.zh-CN.md#v9-钱包扫码修复)。原v7收据和早期失败均保留，不能把扫码修复记作原先已经验证钱包登录。

## 主流程交付基线（v7）

[交付与操作说明](../../WEB-R16-PLAYABLE-HANDOFF-20261104.zh-CN.md)和[最终公开收据](delivery-receipts.json)记录：连续普通 UI 旅程10/10、最终生产包的真实模型教学/取消、阿洛支援、赠礼与受限采购返程、完整服务重启记忆及后续v2跟随。没有真人操作者、全局Candidate100、人类Accepted或公网发布声明。

| 证据 | 范围 |
| --- | --- |
| [阿洛最终收据](final-alo-web-proof.json) | OpenAI实际提案/确认/取消、导出SHA、组队战斗、50赠礼→40采购/1药/返程、整服重启及新v2跟随；v6缺图失败和v7修复后的0错误分开记录 |
| [最终包独立核验](final-package-integrity-v7-proof.json) | 29640文件大小/SHA/普通无链接、编译pin、旧19959资源pin、2602 raw图以及原四帧GET200；本核验自身不代替UI流程 |
| [运行数据保全](final-v7-update-preservation-proof.json) | 正常停止后复制private与观战记录，所有文件before/after哈希相同；旧目录保留 |
| [完整怪物资源](all13-resource-expansion-proof.json) | 实际single-image路径需要13类完整动作；原pins全保留。文件数保护20k→22k明确记账，字节预算未改 |
| [生产源码pin](v6-build-source-pins.json) | R16分支/WIP、BuildID及明确源文件；v6→v7原有JS/WASM未变 |
| [连续旅程：药品](journey-03-real-owned-potion-consumed.png) / [拾取](journey-05-real-monster-drop-picked-up.png) | 同一普通旅程，自有药消费/回血和天然金币拾取26 |
| [组队](journey-06-real-shared-party.png) / [报价](journey-07-real-trade-offer-before-cancel.png) / [取消](journey-08-real-trade-cancelled.png) / [成交](journey-09-real-trade-settled.png) / [重登](journey-10-saved-trade-assets-after-relogin.png) | 第一方真实浏览器，第二方普通认证GatewayClient；同UID蜡烛与金币回执、防重复及持久资产核对 |
| [模型提案确认前](final-alo-model-proposal-before-confirm.png) | 确认前原习惯不变，没有展示原始身份或凭证 |
| [采购完成](final-alo-supply-completed.png) | 实际40金币、一瓶药及返程确认；不将授权当成消费 |
| [重启后记忆](final-v7-alo-memory-fresh-connection.png) / [重启后行动](final-v7-alo-action-memory.png) | 新连接读取原教学，并继续以v2跟随；完整服务重启而非只换页面 |

新增19份安全副本逐一比对源SHA，详情在delivery-receipts。未复制账号/密码、密钥、授权grant、SQLite、模型设置或owner导出。公开截图保留游戏角色显示名，避开固定archive身份页和登录凭证输入。

## 初期快照（历史保留）

以下是[初期主题](../../WEB-R16-LOCAL-PLAYABLE-20261104.zh-CN.md)形成时的有限取证快照。当时主流程仍在验收；这些失败和独立成功轮原样保留，不转记为最终成功。

[receipts.json](receipts.json)记录原公开日志/报告 SHA-256、选定字段、各独立运行的整体结果、正式三 renderer 包的 ABI/大小、包体预算及下列副本的源哈希。源根目录为 `F:/mir2-builds/web-r16-20261104`；完整服务包 manifest 的来源另列在收据中。

| 副本 | 证据范围 |
| --- | --- |
| [登录](login-zh-r16-aligned.png) | 简体 R16 登录画面；账号/密码输入为空 |
| [选角](select-zh-r16-aligned.png) | 隔离测试角色的实际选角页面 |
| [Bag 重开](native-bag-equipped-after-reopen.png) | `qa-bag-visibility-fixed-20261104` 的限定装备与重开保留检查 |
| [NPC 售价确认](npc-owned-dress-sale-review.png) / [出售后](npc-owned-dress-sold.png) | `qa-r16-complete-20261104`，自己的衣服出售；60 金币来自服务器回执摘要 |
| [自有药水消耗](real-owned-potion-consumed.png) / [真实怪物死亡](real-monster-death.png) | `qa-r16-ordered-journey-20261104`；回血 14→15 来自服务器状态，不仅靠截图推断 |
| [阿洛重启公开收据](alo-dev-ui-restart-proof.json) | 隔离开发世界；`rules`、零模型调用、Actor 重启后培养 v2 保留；不是完整 Gateway 重启或新冒险证明 |
| [13 项 UI 补充收据](full-local-ui-supplement-proof.json) | 7,528 B 的明确本地素材清单与源哈希；不是所有素材覆盖声明 |

截图已逐张只读检视，副本与源文件逐字节 SHA-256 一致。测试套件有交集，不相加；普通物品转售、战斗用药和阿洛教学分别来自独立轮次，不能拼成同一条全流程录像。所有收录的主流程 outcome 均为 false，限定 Bag 诊断也未冒充完整旅程。

未收录账号/凭据文件、私有控制或存档配置、SQLite、模型设置、owner 导出。阿洛带原始身份信息的截图未复制入仓；只保留已匿名化公开收据及其原截图文件名。原始 events/outcome 也未整份复制，`receipts.json` 仅提取命名检查、状态、时间和必要玩法数值。

公开完整包 manifest 快照早于后续更新和 UI 补充；最终交付清单、全流程结果及重启恢复仍由主验收继续回填。原 360 MiB thin gate 的失败记录保留。
