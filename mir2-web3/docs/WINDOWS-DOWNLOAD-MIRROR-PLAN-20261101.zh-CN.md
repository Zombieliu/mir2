# Windows 下载镜像接入方案（未开通、未发布）

目标仍是：客户端空目录、Windows 无 HTTP/SOCKS 代理、4 逻辑 CPU / 8GiB Job 提交内存 / SSD、可持续至少20Mbps路径，下载完整包到真实 Healthy 登录界面不超过600秒。当前没有通过这一门槛，未切换默认官网、R2或源站频道，也未移动或推送 canonical 分支。

## 已实现与实测

运行时源码 `24a50f865679cfb81c9edf6c95632c4389187dac`；安装配方是同一提交链的后裔 `10a5c4fadec8f8911732fef2ddb7cd71df800021`。Game/engine 不因纯安装选项修复重新标记源码。两次 v4 完整资源、34包/255063逻辑资源、原始/复制12对CMS、正常PE版本和1482编译输入已通过。单页普通安装→显式默认自动本地启动→真实Ready与十秒存活→Healthy→正常退出通过；桌面选择未覆盖原文件。该本地流程从实际installer启动到Healthy界面的原钟为151.512秒，包含操作和观察，不能当成公网冷测。

安装包1040065902字节（1.040065902十进制GB），SHA256 `859f18bd68f3968fd24af3aead32a6ab53ca1d12b72fcce380a5a90782fafe8e`。路径仍采用哈希命名，包内76文件代替255079散文件。内部CMS有效；Windows Authenticode仍为NotSigned，本轮不能声称Chrome下载/SmartScreen或全新系统运行库验收通过。

R18完整冷测649.100秒，功能成功但时限失败。R19唯一冷测下载480.003秒超时，只收到400858816字节，平均6.681Mbps；没有D1、安装或U0。8并发32MiB不限速诊断8.582Mbps；每流312500B/s、合计上限20Mbps诊断11.419Mbps；HTTP2单流范围3.066Mbps；既有VPS直连8MiB探针60秒仅6995968字节。后四项仅资格诊断，不能称端到端通过。

同域Cloudflare trace将本机网络出口分类为CN，服务节点LAX；未保存出口IP。连接和TTFB并不慢，响应体低速。不能凭这些数据确定是ISP、丢包、R2还是其他环节。Windows TCP自动调优为normal，未改网络、代理或安全设置。官方说明全球网络不会自动使用大陆节点，且不能指定Anycast落点：[Cloudflare路由说明](https://developers.cloudflare.com/support/troubleshooting/general-troubleshooting/geographic-traffic-routing/)。

既有 GitHub Release 也只做了小范围资格检查，没有创建新 Release 或上传副本。github.com 直连15秒连接超时；经官方公开资产API重定向至 release-assets.githubusercontent.com 后，45.359秒仅收到1407760字节，约0.248Mbps，正常TLS、无客户端代理、exit28。该路径本次不具备替换下载入口的资格，不能把已有GitHub账号视为解决了下载性能。

## 可接入路径与依赖

当前部署的[有界源码审查](evidence/windows-cold-start-20261101/DELIVERY-PATH-AUDIT.json)未发现可证明解释低速的reader缺陷。单part、无缓存写入的范围请求也持续低速；这支持继续核实分发路径，但不能排除所有Workers运行时或上游影响。

1. 保留当前R2/Cloudflare作为现有全球路径，新增经过实测的大陆或近区文件镜像。只镜像这份相同哈希的静态安装包及校验元数据，不移动游戏账号、钱包、数据库或权威战斗。
2. 大陆CDN需要可接入域名、账号配置和备案状态；不能假设现有obelisk.build已满足条件。腾讯云的境内/全球CDN需要域名备案，纯境外区域要求不同：[官方接入要求](https://intl.cloud.tencent.com/zh/document/product/228/36178)。
3. Cloudflare China Network是Enterprise之外另订的产品，需要域名审核与ICP条件，不是当前R2缓存的免费开关；报价及功能需厂商确认：[官方开通步骤](https://developers.cloudflare.com/china-network/get-started/)。
4. 近区境外CDN可作为另一个待验证路径，不保证对大陆出口达到20Mbps。既有165.154.65.136源站本次更慢，因此不将它未经实测改成推荐入口。

待用户提供现有CDN/对象存储/镜像域名的配置位置；不要在聊天提供Secret。若没有，先确认服务、域名条件和预算，再开通账号/资源。当前未购买、升级或启用任何新付费服务，未生成生产签名或支付密钥。

## 可审核的接入合同

- 固定源文件、版本、字节数、SHA和签名，不按文件名猜版本；上传create-only，读回全文件及签名后才允许接入。
- 镜像只读HTTPS，证书正常校验；大文件Range返回206/准确Content-Range/长度，禁止压缩变换。ETag按实际提供商处理，完整SHA始终是内容依据。
- 哈希路径缓存长期immutable；官网入口与频道指针保持可原子回退。DNS及官网默认入口只在新的Windows连续实测通过后切换。
- 先用小范围不走代理的资格探针确认同路径可持续达到约定带宽，再做一次空目录完整测试；不足则不启动又一轮明知无法完成的整包冷测。
- 新镜像必须重新核实完整公开字节、CMS、实际机器路由与Job全树；不能把本机原包、服务器私有回拉或索引结果当成用户的下载证明。
- 保留R17/R18/R19原失败记录，不剔除等待、不把超时当成功、不执行partial。全新实体4核8GB与Chrome链路仍独立于当前工作站Job限制验收。

对应未启用配置：[download-mirror-plan.json](evidence/windows-cold-start-20261101/download-mirror-plan.json)。

## 费用试算（未批准）

按腾讯云国际站公开大陆CDN 0–2TB阶梯 `$0.0323/GB`，仅做CDN下行示例：[官方价格](https://intl.cloud.tencent.com/pricing/cdn)。账号地域、合同优惠、计费阶梯与实际报价仍需确认。

| 整包下载次数 | 按1.040065902GB/次的流量 | 单价示例下的CDN下行 |
|---|---:|---:|
| 1 | 1.0401GB | $0.0336 |
| 100 | 104.0066GB | $3.36 |
| 1000 | 1040.0659GB | $33.59 |

未计对象存储、首次回源、请求、日志、域名及可能的税费；未假设套餐折扣或免费额度。5000次会跨阶梯，须按厂商实际阶梯另算。Cloudflare China Network需销售报价。以上是安装下载次数的成本，不是游戏CCU/月或月卡成本。

20Mbps下完整1.04GB正文理论416.026秒，叠加本地151.512秒约567.538秒，这是算术预算，不是已完成的连续验收；当前实测路径不足以兑现该预算。

证据：[交付状态](evidence/windows-cold-start-20261101/DELIVERY-READINESS.json)、[失败冷测](evidence/windows-cold-start-20261101/r19-cold-download-failed.json)、[正常安装smoke](evidence/windows-cold-start-20261101/r19-normal-installer-smoke.json)。
