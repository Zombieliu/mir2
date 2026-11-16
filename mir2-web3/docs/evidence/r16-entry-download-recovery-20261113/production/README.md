# 进城下载恢复候选发布与回退

本轮新版没有通过公网完整试玩，已回退。当前 [numeron.obelisk.build/game](https://numeron.obelisk.build/game) 读回 `b2097fdfd13a2d8bc0acde435cd612fd83672cac`、部署 `dpl_3Hf6tBQxshV6qvoHR1oSakTBokMd`；该版本包含 `6997f810ed` 玩家反馈修复。回退后 11 个文件与简繁出码复验通过，本轮不据此宣布旧版完整冷加载通过。[最终状态](final-state.json) · [平台确认](restored-platform.json)

## 本轮实际执行

- 产品源 `9dc7294c0269eee93086e88ac83d2bc9d93d387e`，发布保护源 `82efae2f89760a8760a9f81dda3f8b647289b9ac`；[本地 213 项、构建和四会话](../README.md)通过后发布。运行包 `bevy-0c553a754308e5b8`、资产 `20261108-r16-08ec86d5105c` 不变。
- [Stage 43389992449](https://github.com/Zombieliu/mir2/actions/runs/43389992449) 成功，独立确认候选 `dpl_XPzRmyXdXVbC96weq8xUEeW9KCkF` READY/production/源匹配后，通过 [Promote 43391130921](https://github.com/Zombieliu/mir2/actions/runs/43391130921) 切换。正式 alias、version、Worker 与 8 个 Core 文件完整字节/SHA 对上，共 11 条读回通过。[切换回执](release-receipt.json) · [候选文件](candidate-files.json)
- 简繁各一次真实 240×240 二维码、正确待确认文案和正常取消通过，取消后零轮询。[候选扫码](candidate-qr.json)
- 默认网络 Chrome 经官方手机尺寸 PWA 创建隔离虚拟 Passkey Testnet 钱包，完成一次仅登录签名、认证选角和普通测试角色创建。没有实体手机或真实资金验收；浏览器均已关闭，钱包网络保护零越界操作。[完整尝试及原 SHA](failed-replay.json)
- 正常 StartGame 后未在原首场景观察期内获得首个可操作画面，失败 `FIRST_PLAYABLE_SCENE_NOT_OBSERVED`。随后普通注销返回选角成功。移动、Native HUD/Q、六字体和 Worker 停启均未进入，完整资格为 false，不能拼接其他会话的结果补成通过。
- [Rollback 43392450220](https://github.com/Zombieliu/mir2/actions/runs/43392450220) 成功恢复 B209；独立平台源确认、version/11 文件和简繁出码再验通过。[恢复文件](restored-files.json) · [恢复扫码](restored-qr.json)

## 已知失败边界

首次静态 scene 返回 200，但只读到 278,513 / 458,475 字节；locale 为 2,506,703 / 3,283,113，floor 为 131,071 / 439,896。随后命中无正文进展或响应头期限，后续读取亦未完整完成；动态场景回退也未完成。页面没有提交不完整场景，重试提示可见。[原始网络分项与标记](failed-entry-delivery.json)

本轮没有请求 NPC 包，Native 尚未尝试启动，所以这次失败并未进入新加的 NPC/运行包恢复路径。B209 至本候选的固定 bootstrap 字节、pin、Next 路由与域代理规则未改变；没有据此认定新逻辑导致正文中断。

失败后的 curl 诊断用 HTTP/1.1/gzip 得到三个完整 SHA；但时间与回退重叠，且继承的代理环境与 Chrome 不同，不能用来证明同候选传输正常、归因 HTTP/2，或作为公网冷加载通过。[仅诊断的读回](post-failure-curl.json) 下一轮先针对同一固定部署的单个 scene 完整读取继续定位，再决定是否值得重演登录；暂不改无关协议、缓存和时限。

所有原时限、失败和 131 份原 WIP 保留。公开旧档、当前完整三职业至 30 级、地图/技能、NPC 名称与语言像素、实体手机及最终人类 Accepted 仍开放；持续 Goal 没有完成。
