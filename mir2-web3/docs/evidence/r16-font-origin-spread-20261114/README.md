# R16 完整字库分源候选

2026-11-14。源码 `7140aae96c41986968aadf040436e0ab396e1fdb` 已推送到
`codex/windows-player-journey`。本轮已尝试发布，但公网首步确认和共享HUD／字体门槛失败，已回退；正式入口保留
`e6c923b7c669cb061a7a03703f05fadd91211c97`，不覆盖其已发布的寻路修复。
[本轮发布失败与已核对的回退](production/README.md)。前次字体通道发布的失败与回退仍见[原回执](../r16-font-lanes-20261114/production/README.md)。

## 寻路反馈与本轮范围

“目标不对或到不了”的三格前视、保留原任务目标重算及 NPC 姓名命中修复已经在
E6 正式版；[实际路线和边界](../r16-route-lookahead-20261114/README.md)保留。
本轮新鲜执行任务导航、纠正和权威等待三个测试文件，59/59 通过。
NPC 目标是其坐标，合法终点可以在旁边两格内；例如补给目标 (324,291)
的既有实走终点是 (322,293)，不是要求进入 NPC 占用格。
新的真实多人第三格占位、跨图／重连和 R15 旧停步仍未由这些测试解决。

新候选发布先被共享 HUD 的冷资源加载挡住。本轮只调整完整字体的来源，
保留原六套字库、SHA、原 7／8 秒响应头、15 秒正字节停顿、32 MiB 上限、
两路正文预算及 Rust 六语言字体映射的完整就绪条件；没有放宽原 Native 120 秒门槛。

## 八组实际公开资源隔离测量

每组均使用新 Chrome 上下文，不登录、不启动游戏、不修改公网。
原始结果及生产者 SHA 见[汇总](network-summary.json)和[逐文件校验](network-artifacts.json)。
普通 fetch 使用 `cache: reload`；直接读只观察原头部／停顿预算，不提前按停顿中止，
各资源总测量上限仍为 120 秒。Worker 会等完整正文才返回给消费者，
消费者的几十秒头部时间不能当成上游 7 秒失败。汇总删除了旧生产者在失败路径
留下的这两项消费者衍生布尔值，原始 JSON 保留原样。

| 条件 | 实际结果 | 资格边界 |
| --- | --- | --- |
| [直接读 TC＋Latin](network/direct-two.json) | 完整两套 SHA 通过；TC 89.684 秒 | 只测这两条资源 |
| [正式 E6 Worker 读 TC＋Latin](network/worker-two.json) | 完整两套，TC 65.281 秒 | 上游和消费者计时分别保留 |
| [正式 Worker 两字体＋实际 CDN WASM](network/worker-two-runtime.json) | 三条完整 SHA 通过，WASM 67.221 秒 | 没有实例化 WASM，不是完整游戏竞争 |
| [直接读六字体](network/direct-six.json) | SC／TC 到 120 秒分别只有 8,841,602／11,344,182 解码字节；小字体四套完整 | 末尾版本读回未完成，整轮仍 false |
| [正式 Worker 读六字体](network/worker-six.json) | 小字体四套完整，两个大字库未在消费者 120 秒内返回；TC 上游继续到约 135 秒才结束 | 末尾版本未确认，整轮仍 false；0 消费字节不等于 0 上游字节 |
| [直接读六字体，SC 分到当前 CDN](network/direct-six-spread.json) | 六套完整 SHA，SC 57.409 秒、TC 39.714 秒 | Arabic 一次停顿 15.129 秒，原停顿预算仍 false |
| [同一自有 Worker 内注入候选函数](network/worker-six-spread-injected.json) | 六套完整 SHA；SC 111.115 秒、TC 80.883 秒；上游每套单一完整 200 | 仅诊断候选函数，不是已发布脚本或完整 Native 冷启动 |
| [候选函数六字体＋实际 CDN WASM 同时读取](network/worker-six-spread-runtime-injected.json) | 七条完整 SHA；SC 28.212 秒、TC 45.459 秒，WASM 20.791 秒，最后字体约 46.056 秒收齐 | WASM 未实例化；不同时间的速度变化不当成严格因果对照 |

这些顺序进行的观测缩小了问题范围：不经过 Worker、不运行游戏时，两套大字体
同时走应用源仍能超过测量上限；分源方案有改善，但不能据此断言 HTTP/2、代理、
CDN 或具体网络层机制已经定因，也不能把不同时间的用时差当成严格吞吐对照。
浏览器沿用本机网络设置，与之前清理部分代理环境变量的正式回放条件不完全相同。

候选诊断的函数注入只发生在本轮新建、最终关闭的 Worker：保留正式 E6 的 fetch
事件和缓存入口，将 `fetchStaticAsset` 换成完整候选代码生成的闭包，使用真实
fetch、CacheStorage、原定时器及两路正文队列。公开服务没有写入候选代码。
源 SHA `94796b40517c7fd1ca7914ebaeb7a99d6ebd4a3546e2fa28d1c9d75abe836218`
与提交的 Worker 完全一致。首次尝试通过 Playwright route 替换 Worker 脚本没有
命中请求，候选未安装，[false 原件](failed-worker-substitution.json)保留。
所有组均确认关闭其自有浏览器。

`producers/` 是原执行文件的逐字节快照，保留历史机器路径和原导入位置，供审阅，
不是新的可移植产品命令；不包含钱包或账户凭证。

## 实现与定向回归

仅 `NotoSansCJKsc-Regular.otf` 默认优先使用配置中当前 immutable release 的
主 CDN；TC 和其他字体保持应用源优先。只选择已有允许源，不造新 URL、
不把旧版本提前；显式全字体 mirror-first、Range 和未固定版本的语义保留。
CDN 出错后仍用完整回退正文，前缀不入缓存、不与后续来源拼接。

[新鲜测试原件](tests/receipt.json)逐字节绑定提交后的 Worker 和测试输入：

- 同六项新回归对旧 6ae：2 过／4 失败，完整退出；[旧代码日志](tests/before.log)。
- Worker、预热、生命周期、瞬时缺失和运行包初始化：155/155，无 skip；[日志](tests/after.log)。
- 任务路线、纠正和权威等待：59/59，无 skip；[日志](tests/routes.log)。
- 发布、域名保留和工作流入口保护：68/68，无 skip；[回执](release-tests.json)及[日志](release-tests.log)。初次单文件实际35项通过，统计脚本误用68预期返回1，[单文件原统计](initial-release-test-count.json)保留；最终68来自三个既有文件。

前三次旧代码对照的清理钩子因未消费响应／空正文触发后续源而挂住，30 秒子进程
上限结束；未计作“旧代码完整失败”。其 JSON 和日志以 `failed-test-cleanup-r1` 至
`r3` 原样保留。最终先送非空前缀，并在失败清理时关闭流、消费响应，才得到上面的
完整 2／4 结果；没有改变产品断言或预算。

## 构建、四会话及下一门槛

独立生产构建已通过：`7140aae96c41-font-origin-spread-5feeb428`，
BUILD_ID `NoSF23HrNg6Fu9FfUOXIC`，35.899 秒。
2,773 项 Web／GDI／Worker／共享键盘输入逐字节绑定提交，只保留固定已发布
`bevy-0c553a754308e5b8` 清单转换；原 Shell／Native WIP 未混入。
[源码与构建](source-qualification.json)及[候选资格](candidate-qualification.json)分别保留。

默认英文与真实共享 WebGL2 繁中均通过冷开、彻底关闭后同角重登，共四会话：
原 Native 120 秒内分别为 54.562／1.254 秒及 52.171／1.281 秒；六套完整字体
SHA、113/113、共享 HUD／Q、同 ID NPC／可见 X、Worker 停启、1024 字节前缀
停顿后的完整恢复与正常注销／保存通过。两次补给路线各189次普通走跑到 (322,293)，
狩猎区零步反馈与商人终点分别核对。冷场存档 (289,616) 与热场权威起点相同。
[四会话回执](local-four-sessions.json) · [冻结检查器绑定](local-preparation.json)。

已逐张查看四张[图片及 SHA](visual-review.json)。共享界面、英／繁任务和商人到达
画面通过本轮范围；英文攻击模式／货币行夹中文、NPC raw 标题及繁中别名分行仍
需 Windows 对照。既有毕业档保持不变，不算新做三职业1到30级。

真实公网完整回放、原首帧后 120 秒、实体手机／Windows 与整体 1:1 仍开放；
八组静态资源测量不替代这些门槛。发布助手固定本候选的两个 Git 文件、44059
字节 Worker 及原15项静态门槛后，Stage／Promote及15项完整读回通过；新普通
公网回放首步13.286秒、共享HUD未就绪、字体4/6、运行包首帧后153.493秒，整轮false。
已回退E6并复验简繁扫码及独立API。[完整结果与下一步](production/README.md)。
