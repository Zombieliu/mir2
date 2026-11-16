# NPC 完整姓名与原定位框

2026-11-14，R16 候选 `b19ca73072db16ac7f351857aa752ecc57953591` 已提交推送。此前 `Assistant Jane` 的 GDI 测量宽79，却被画进50×15位图；普通长名字也被48px定位框挤成多行。修复沿用原48/50px定位框和运动驱动，按各行 AutoSize 选择字图，整像素居中；普通文字保持完整一行，NPC原始下划线分行仍按12px排列，副名用白色。点击文字伸出定位框的部分仍能通过真实画布命中同一对象。

依据是固定 Crystal `NPCObject.CreateNPCLabel/DrawName`、`MapObject.CreateLabel/DrawName` 与 `MirLabel.GetSize`：定位框不是文字最大宽度，描边在测量结果上各加2px。保留原8张固定／自动字图，新增 `Assistant Jane` 81×16、`Merchant` 54×16、白色 `Ruben` 40×16三张。两次独立Windows GDI导出完全一致，原8张PNG逐字节未变。[字图及字体来源](asset-preservation.json) · [修前画面](names-before.png) · [修后画面](names-after.png)

导出机当前 Arial 文件 SHA 与旧夹具不同，最初相同字体文件检查失败；在8张原PNG逐字节一致后显式采用当前来源，未冒充同一字体文件。原始导出清单为CRLF，提交输入遵守仓库LF规范，JSON值完全一致。工具提交 `b2dd6ac70b` 同时固定清单及所有权标记的LF；Windows PowerShell5.1自检通过11张字图、两次重复导出和8个非法输入拒绝。PS7旧Add-Type依赖失败保留。[清单／标记来源与转换](manifest-provenance.json) · [导出自检](gdi-self-test.txt) · [Node字节检查](gdi-node-after-lf.txt)

39项相关检查通过。初次相邻检查37/40，三项失败在提交 `6e5abd6ddd` 的独立输入中同样复现：两个聊天检查缺少既有Tooltip导入的测试替身，已修复该检查；钓鱼导出范围 `[0,351]` 与旧断言 `[0,175]` 仍未通过，没有改写或忽略。[相关检查](focused-tests.txt) · [初次相邻检查](initial-adjacent-tests.txt) · [独立基线](baseline-tests.txt) · [基线输入](baseline-test-source.json)

独立生产构建通过，BUILD_ID `twLw4KivOhoiCO5yJSu1j`。2,771项输入逐字节对照提交，包括所有非public Web源文件、12项GDI public文件和共享键盘文件，仅保留具名的固定运行包清单转换。原Shell动画和Native未提交工作均未混入。真实整个场景组件加原版源码画布目标选择器的4个伸出框外点击通过；后续在精确构建源复跑的图片与root已审图逐字节相同。[构建与源码闭包](source-qualification.json) · [修前组件](component-before.json) · [精确源修后组件](component-after.json)

## 完整游戏四会话

默认渲染英文 `en-US`、真实共享WebGL2繁中 `zh-TW` 均完成冷启动、完整关闭所有该浏览器进程、同角色重登。两路都正常点NPC1、收到同ID权威对话、点原生X产生唯一 `@Exit` 并收到关闭状态；狩猎区内零步，选择补给商人实际目标 `(324,291)`，各189次正常走跑到 `(322,293)`，原8秒逐段前进、停止、注销回选角及落盘坐标通过。冷场保存 `(289,616)` 与暖场权威初始位置相等；原30级毕业夹具保全通过，不能算本轮新手至30级验收。

原120秒Native门槛未改：默认冷／暖54.545／1.298秒，共享WebGL2冷／暖52.166／1.315秒。每会话113/113、六字体完整读取／SHA、Q及原输入检查通过，Worker缓存重启通过；浏览器、代理、Next、Gateway均关闭。这是固定QA Gateway与受控资源／网络条件下的本地结果，不能替代公网冷加载。[四会话回执](local-four-sessions.json)

英文实际观察9个NPC，其中7个名字部件超出定位框；未换行，并实际读取新Merchant/Ruben字图。root已审两路的对话和商人到达画面。[英文对话](default-en-npc-dialog.png) · [英文到达](default-en-route-arrival.png) · [共享WebGL2繁中对话](shared-gl2-tw-npc-dialog.png) · [繁中到达](shared-gl2-tw-route-arrival.png) · [审图与SHA](visual-review.json)

首次英文整轮错误期待HTML语言 `en`，第二次错误等待硬编码繁中寻路提示；两次完整结果都保留false，均完成清理且未改产品。最终观察器按共享元数据 `en-US` 和精确构建源词库校验对应文案，原坐标、范围、步数、超时与权威门槛不变。[保留的失败](retained-failures.json) · [日志原件SHA及显示转换](log-provenance.json)

## 当前边界

本候选已包含[死亡／复活文案十语](../r16-death-locale-20261114/README.md)，该文案只有实际组件检查，不由上述普通冒险替代真实死亡／复活验收。新截图仍见英文攻击模式与部分货币行夹中文、原生NPC标题保留原始下划线；繁中别名还可能合并原始分行。继续与已提交Windows核对这些语言边界。三张字图之外的浏览器文字像素也未声称与Windows GDI全部一致。

2026-11-14 02:46 UTC 的[原正式映射](production-readback.json)保留。后续已尝试发布b19，Stage／Promote及三张新PNG切流前后完整校验通过；公网出码、官方手机尺寸Testnet确认、进城首步234ms、113项与正常退出通过，但字体504、五套缓存缺失及共享HUD失败，整轮false，已回退。03:34 UTC独立API与后续简繁实际出码pending均通过，正式恢复 `e6c923b7c6`／`dpl_Ae3hNyFygQuSMZDesBXCZDpUECTY` READY；候选未留正式。[失败、回退与全部边界](production/README.md)。真实多人第三格占位、R15旧战后停步、实体Windows／手机、全部场景GPU、当前三职业完整新手线和整体1:1继续开放，Goal active；不计最终人类Accepted。
