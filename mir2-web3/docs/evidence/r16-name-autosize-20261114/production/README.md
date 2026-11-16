# 姓名与死亡文案发布失败后回退

2026-11-14，本轮候选 `b19ca73072db16ac7f351857aa752ecc57953591` 已尝试发布，但公网完整功能回放失败，已回退。当前 [Numeron 试玩](https://numeron.obelisk.build/game) 为 `e6c923b7c669cb061a7a03703f05fadd91211c97`／`dpl_Ae3hNyFygQuSMZDesBXCZDpUECTY`。独立 Vercel API 读回正确游戏项目、R16、READY、production；回退后新普通 Chrome 的简繁中文均完成实际二维码像素校验和 pending。所有本轮浏览器已关闭。[最终正式映射](final-production-mapping.json) · [回退出码](rollback-qr-browser.json)

发布工具提交 `e4f085e0d4` 固定候选及13个 Git blob，并把三张新姓名 PNG 加入 protected/public 完整字节与 SHA 检查，连同原11项共14项。68项发布保护通过，零 skip；包括逐项内容损坏拒绝。此次工具改动不修改游戏规则、运行包、权限或用户数据。[Stage 43550144423](https://github.com/Zombieliu/mir2/actions/runs/43550144423) 与 [Promote 43550864344](https://github.com/Zombieliu/mir2/actions/runs/43550864344) 均成功；三张 PNG 在切流前后完全一致，第一方 QR relay pending。只切换游戏项目，官网、DNS、Gateway、固定资产／运行包未重新发布。[切流与14项校验](release-sequence.json)

随后一次独立冷浏览器回放完成真实出码、官方手机尺寸钱包 Testnet 登录签名、建一个 QA 角色、进比奇、一次正常向右移动及普通注销。移动 `(288,616)` → `(289,616)` 的权威回包234毫秒，通过原8秒；113/113预取最终恢复、繁中备用任务窗和普通场景通过。但共享 HUD／Q 未就绪，六套字体缓存仅简中完整，另五套未读取到；被动网络观察见字体504。整轮明确为 false，原120秒Native冷资格也为 false，Worker停启未执行。保留199次资源HTTP错误、202次console error、零page error与290次ERR_ABORTED计数，不归零、不把备用任务窗当共享任务窗。[失败分项与原件SHA](failed-signed-browser.json) · [六字体缓存](failed-font-cache.json) · [字体504与最终运行状态](font-network-observation.json)

观察器在执行前冻结新源码／部署、完整地图碰撞和生产解析器，20份辅助文件中仅3份机械重绑定。没有事后修改已运行观察器、增加超时或把失败改成通过。公网只实走一格；同NPC对话、长路线、两后端四会话和保存来自[本地资格](../README.md)。失败后未执行依赖完整浏览器通过的四组额外静态读回，未制造 `signedBrowserPassed:true`。原发布阶段的14项字节检查不等于这四组未执行的检查。

失败定位到字体请求504及共享界面未就绪，尚不能证明姓名或死亡文案代码导致了上游失败。仍执行 [Rollback 43552001521](https://github.com/Zombieliu/mir2/actions/runs/43552001521)，恢复上一份已验证功能的正式部署。[回退写入与原件SHA](rollback-sequence.json)。旧正式版的冷加载126.060秒超原120秒问题仍见[原正式验收](../../r16-route-lookahead-20261114/production/README.md)，回退不等于消除了该性能缺口。

root已查看[失败前繁中任务画面](public-quest-before-failure.png)：备用任务窗可见，左下仍显示资源预取失败／进行中，共享HUD尚未出现。该图不能作为界面通过或最终人类Accepted的证据。[审图与SHA](visual-review.json)。手机尺寸官方钱包签名不是实体手机相机扫码，也没有支付。原始钱包、授权及会话凭据仅保留在私有回执，不提交。本轮新增姓名／死亡文案仍为本地候选；线上保留原三格寻路前视、保留目标重算和姓名命中修复。真实多人第三格占位、R15旧停步、实体Windows／手机、完整死亡复活和其余1:1开放，Goal active。
