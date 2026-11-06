# 充值入口 UI 验收记录（2026-10-29）

**本轮 UI 技术目标已完成**：法币充值、USDC 链上充值和月卡三个入口持续展示，真实 Windows 客户端逐页切换与重启通过。充值逻辑专项 19 通过，英/繁/简独立 GPU 共 39 图零缺字、溢出和越界；已有 100 点券、10000 金币和三张历史订单保持一致。`complete=true` 仅指本轮 UI 目标，不代表完整发布或真人 Accepted。

三个入口各有独立页面；切页不购买，隐藏页面旧按钮不能跨页购买，未知结果重试复用原请求；已有月卡的展示/使用独立于新销售开关。收费配置、汇率、余额和后端支付规则没有变化。

首次简体实机启动曾因旧显示标签表 len 9 / index 9 越界失败，未点击游戏控件；[失败和修复记录](../native-zh-cn-20261029/windows-live/evidence/first-simplified-startup-failure.json)保留。显示路径修复后 7 个专项、三张最终游戏图及真实窗口十图补验通过，不将此前编译和离线截图写成当时实机已通过。

| 检查 | 结果 |
| --- | --- |
| 充值逻辑 | [19 通过，1 个 GPU 另跑](native-mock/focused-result.json) |
| 离线充值 GPU | [3 语言 × 13 状态，39 图](native-mock/gpu/report.json) |
| 生产字体 | [16 通过；十语独立 GPU 见简体包](native-mock/font-result.json) |
| 最终修复构建 | [149412864 字节](native-mock/windows-build.json)，SHA-256 `abd92be30f421328c3309eac66bbafcdd542df4117ed24ced872098e9b9b64ff`；[旧 5dc223…](native-mock/windows-build-before-startup-fix.json)仅保留失败历史 |
| 真实窗口 / 重启 | [10 图操作回执](../native-zh-cn-20261029/windows-live/navigation-result.json) |
| 经济字段 | [导航前 / 三页后 / 重启后独立比较一致](../native-zh-cn-20261029/windows-live/economy-preservation.json) |

39 张离线图早于两处 NPC/任务标题补充和显示表修复，此后充值代码/页面文案未变；最终生产字体、游戏标题和实际启动路径独立补验。不能称所有旧 suite 在最终源码全部重跑。阶段见 [index](evidence-index.json)。忽略的 GPU 不计为逻辑专项第 20 个通过项。

**真实 Windows 页面**只在同轮简体包存一份，避免重复证据：

| 页面 | 截图 |
| --- | --- |
| 法币充值 | [07：入口可见、当前服务器未开放](../native-zh-cn-20261029/windows-live/screenshots/07-zh-cn-fiat.jpg) |
| USDC 链上充值 | [08：入口可见、三张历史单状态](../native-zh-cn-20261029/windows-live/screenshots/08-zh-cn-usdc.jpg) |
| 独立月卡 | [09：月卡页面与购买/持有区](../native-zh-cn-20261029/windows-live/screenshots/09-zh-cn-monthly.jpg) |
| 重启仍为简体 | [10：相反 en 种子下恢复已存 zh-CN](../native-zh-cn-20261029/windows-live/screenshots/10-zh-cn-after-restart.jpg) |

这些真实窗口连接隔离的文件权威 Gateway，新法币/USDC 销售关闭。没有新付款、新订单、月卡购买或使用。100 点券源于既有测试网付款，本轮只验证保持不变；[旧实机基线](windows-live/evidence/before-ui-change.json)与[此次最终回执](../native-zh-cn-20261029/windows-live/evidence/final-ui-result.json)均为公开投影。保存 revision 增长不表示经济字段改变。

以下为**离线测试数据**，不代表真实余额或新交易：

| 页面 | English | 繁體中文 | 简体中文 |
| --- | --- | --- | --- |
| 法币充值 | [图](native-mock/gpu/screenshots/en-fiat-1024x768.png) | [图](native-mock/gpu/screenshots/zh-TW-fiat-1024x768.png) | [图](native-mock/gpu/screenshots/zh-CN-fiat-1024x768.png) |
| USDC | [图](native-mock/gpu/screenshots/en-usdc-confirmation-1024x768.png) | [图](native-mock/gpu/screenshots/zh-TW-usdc-confirmation-1024x768.png) | [图](native-mock/gpu/screenshots/zh-CN-usdc-confirmation-1024x768.png) |
| 月卡 | [图](native-mock/gpu/screenshots/en-monthly-1024x768.png) | [图](native-mock/gpu/screenshots/zh-TW-monthly-1024x768.png) | [图](native-mock/gpu/screenshots/zh-CN-monthly-1024x768.png) |

[简体/十语完整记录](../native-zh-cn-20261029/README.md)说明 9769 键离线生成、玩家参数保持原样及 1996 字符字体覆盖。完整 SC 主字体与原 TC 内嵌；早期“币、链”两字字体仅为小型回退，不能代表全游戏简体覆盖。

[安装打包专项](../native-zh-cn-20261029/packaging/static-focused-validation.json)通过十语配方、11 字体 pin 与精确许可文件检查，两个包固定同样 43 个当前源码。完整 package/verify selftest 因既有 `Title/meta.json` 身份 pin 不一致提前受阻；可信 Inno 未安装，完整翻译闭包、安装器编译/安装/签名未验。此处完成 UI 验收不关闭这些发布门限，也不代表 9769 条均经过人工语言审校。最终玩家前端 Accepted 仍待真人确认。

[旧 USDC 包](../sui-usdc-recharge-v1-20261029/README.md)的 manifest、源码 pins 和付款/恢复记录保持原样，本轮未重测付款。二进制、完整账号库、APPDATA、原始偏好、密钥、临时工具与大段构建日志不收入此包。运行 `node generate-manifest.mjs --verify --require-complete` 校验本轮范围；提交前另核暂存区 Git blob。
