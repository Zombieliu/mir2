# 字体重试已发布：功能通过，冷启仍失败

2026-11-13，正式入口 [numeron.obelisk.build/game](https://numeron.obelisk.build/game) 已运行`491819b20efd53d4cc8442cb87e8c6fc1e1a80e8`，部署`dpl_9Trr2mCdbfG79mFmgV8PGuFS6Cw9`，替换f6acaad6b0。**发布与功能回放通过，原120秒Native冷启未通过；本回执passed=false不等于发布失败。**

- [Stage43278654810](https://github.com/Zombieliu/mir2/actions/runs/43278654810) / [Promote43280166944](https://github.com/Zombieliu/mir2/actions/runs/43280166944)均成功，同一生产构建切换，target与alias一致，无回滚。辅助程序`523ee3710524378beddd52b7560231b641e89753`。
- 本地101项、58份提交/构建输入一致，70项发布保护和云端构建通过。9条公网文件完整正文及摘要与候选一致，包括Worker40071字节及六份小型客户端固定文件。[绑定](committed-source-binding.json) · [公网文件](final-public-readback.json)

## 实际通过

普通Chrome简繁出码、待确认、取消通过。官方钱包PWA在手机尺寸浏览器中用独立Testnet虚拟Passkey进行一次登录签名，普通新角色进入比奇，移动一次，113/113、共享Native HUD、繁中共享Q开关、六份原版字体全部缓存正文及SHA、Worker真实停止→重启后的相同HUD图与配置、保持Native ready、普通注销回选角通过。初次110/113的3项失败均补读成功，历史保留。钱包资金操作0，手机/游戏自建浏览器均关闭。

点击进游戏后6.808秒出现可操作画面；真实walk→UserLocation4.144秒，保留原8秒限制。六套字体共29517096字节，与Windows原版来源完全一致；没有删字、子集化或换字库。[六字体](original-fonts.json) · [Worker停启](worker-restart.json) · [实际共享HUD](public-native-hud.png) · [实际繁中共享任务窗](public-native-quest.png)

这是官方PWA设备模拟，实体摄像头未测；没有以此冒充物理手机验收。截图仍有既有NPC名字碎片和中英混合，不算全语言像素通过。

## 未通过：120秒冷启

从首个可操作画面计，共享HUD在158.084秒后才被观测到，超过原120秒标准。原始签名脚本的扩展观察记录功能passed=true，但其optional观察不是性能资格；此处性能门槛与汇总passed仍false。时间轴217.375秒时HUD/Quest都仍false，已经晚于180.116秒的冷启期限37.259秒，失败有实际采样支持。没有增加120秒标准、延后起算或覆盖旧失败。[完整分项/门槛](receipt.json)

新只读Worker上游CDP捕获22个字体请求、0观测错误，弥补上次跨域TAO时序无法区分具体状态的问题：14次ERR_ABORTED；SC首个当前源HTTP200后已收到约13.3MB编码正文，70.137秒时发生ERR_HTTP2_PROTOCOL_ERROR；另一当前源26.660秒提供完整副本，最终缓存SHA正确。此错误不能仅凭客户端归因给CDN、代理或网络任一侧。

HUD预取本轮持续61.084秒；当前Page等待该阶段后才启动独立字体和WASM读取。下一步对照这段依赖与实际正文流，优化调度/长读取，保持相同完整资源和时限，不用减少字体或关闭共享UI来制造通过。[上游无身份事件](worker-font-network.json)

## 取证与范围

执行前固定19份QA脚本和摘要，保持原授权、8秒首步、实际场景及缓存重启规则；增加只读六字体缓存SHA与Worker上游观测。观测器仅精确Worker URL和同浏览器context，隔离Chrome157实验实际捕获200/404/网络错误并排除Page请求及其他context；无headers/body/query/身份写出。CDP attach可能延长闲置生命周期，因此在显式Worker停止前已完全detach，停启证明沿原测试执行。[观测器](qa-worker-font-network.mjs) · [隔离验证](observer-isolation-receipt.json) · [帧隐私3项](frame-privacy-tests.log)

[Stage回执](stage.json) · [Promote回执](promote.json) · [发布保护70项](release-tests.log) · [收尾公网再次读回](final-check.json) · [限定部署的Function错误日志分组](runtime-error-counts.md)。完整认证记录只留本机，仓库仅保存筛选字段与原回执摘要；日志只规范换行/行尾空白。

原65份及本轮开始时134份待提交文件逐字节保全；没有改Gateway（独立health为7033a012bf）、DNS、官网、链上合约或生产存档。旧18.635/19.7秒首步、120秒Native失败和完整1:1、三职业当前1–30、技能/地图/阿洛/Hosted/Windows窗口及最终人类Accepted仍开放。
