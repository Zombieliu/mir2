# 成对试发、原门槛失败与回退

**当前正式仍是 e0／Fo，306 候选已从两端回退。**
R1 通过原验收，R2 的 Native 在首帧后 128,080 ms 才就绪，超过原 120,000 ms。
本轮没有放宽门槛，未恢复数据库，保留了新增 QA 角色与进度。Goal active，整体 1:1 未完成。
[最终结果与独立读回](outcome.json)。

## 发行身份与资格

| 项目 | 已核对的事实 |
| --- | --- |
| 应用源码 | `306066f29590d1e2e14155d83644fbc19f11a4c3` |
| 发布保护配置源码 | `b87aa402a38aeb70f30e5afe285862cd8ecf7a46`；88 项保护测试通过 |
| 尝试的 Web 部署 | `dpl_9txRbTXzvDryXkCGH1hHtPn1Gssn`，独立核对为目标游戏项目、精确源码、READY |
| 已恢复 Web | `dpl_FoNUgjSRpw56hwUFDNrSs7LZZQFN` |
| 已恢复应用／网关 | `e0ceea22964c03041d45e49e506b3a8badd6984e`，网关 PID `1729017`（本次读回时） |

[同源 Linux／PG CI](https://github.com/Zombieliu/mir2/actions/runs/43782096198) 已完成：
310 项共享世界相关检查、216 项原生 Postgres 检查及包内 1,841 个文件 SHA 均独立核对通过。
216 项没有忽略或 Docker 替代；工作流中与该构建模式无关的分支仍正常跳过，不能称所有工作流分支都执行过。
ZIP 95,700,093 字节，SHA `b7c195e051d169a6726888d5297fcac3f222161162acc260c4d7fd64e3978538`。
网关 ELF 93,576,560 字节，SHA `22bb54130b2084a34c31d7b0b7bd604cc99a260a08e8d924a5513b329ee4ec96`。
它与旧版本 ELF 字节相同，但仍先验证了本次独立 CI 包，再按完整压缩 SHA 复用服务器传输缓存。
[包验证](backend-artifact.json)。

独立克隆原库的 318／230／230 行后，普通注册、密码登录、创角、移动、两次普通注销、
同角色落点重登和一条自然中立怪移动通过；没有 QA／管理员命令，未改变正式进程。
[隔离协议回执](canary-protocol.json)。隔离网关与 Redis 均在切流前停止。

Web [候选构建](https://github.com/Zombieliu/mir2/actions/runs/43782847319)、
[检查](https://github.com/Zombieliu/mir2/actions/runs/43783855827)、
[切换](https://github.com/Zombieliu/mir2/actions/runs/43786660426)及
[回退](https://github.com/Zombieliu/mir2/actions/runs/43788334107)的精确回执分别见
[stage](web-stage.json)、[check](web-check.json)、[promote](web-promote.json)、[rollback](web-rollback.json)。
发行步骤成功不等于游戏原门槛通过。

## 原两场公网结果

| 原场次 | 实际按键到权威 ACK | 运行包正文首帧后 | Native 首帧后 | 原判定 |
| --- | ---: | ---: | ---: | --- |
| R1 | 226 ms | 10,137 ms | 51,176 ms | 通过 |
| R2 | 131 ms | 1,911 ms | **128,080 ms** | **失败：Native 超过 120 秒** |

[R1 原分类器输出](public-r1-acceptance.json) · [R2 原分类器输出](public-r2-acceptance.json)。
两场功能运行器都 exit 0；R2 原分类器 exit 1，这个失败不能被最终完整加载或 180 秒观察窗口覆盖。
原分类器 SHA、8／65／120 秒、两次 20 秒的资源流程、完整六字体和 113 项均保持。

两场都是真实公网、官方 Testnet 钱包正常确认一次、普通创角进比奇、一格权威移动、
完整字体／HUD／行走包、Worker 重启和正常注销。两场共享门槛后的 Q 都实际由 Native／bevy 接管，
没有 Q 世界写入；两场公众渲染路径均为 WebGPU，不能扩称公网 WebGL2 或完整战斗／三职业 1–30 验收。
未使用实体手机相机扫码；手机尺寸浏览器的官方钱包操作与实体相机验收分开。

首次 Q 两场仍是 React 备用。R1 PNG 为 3,938 字节的全黑图，SHA 与之前 e0／6ba 的黑图相同；
R2 有实际备用画面，其布局与 Native 不同。原始截图与像素观察均保留，不列为视觉通过。
[像素结果](public-q-pixels.json) · [R1 黑图](r1-first-q-black.png) ·
[R2 备用窗](r2-first-q-fallback.png) · [R1 Native](r1-native-q.png) · [R2 Native](r2-native-q.png)。

## 回退和数据保全

网关切换前在无在线会话时制作同一导出快照的备份：
`/var/backups/mir2-playtest/r16-cjk-progress-306066f295-20261116T005843Z`。
dump SHA `7b96212c37444c477e729c4e30e214923a038d2f0e3ec3e03870b975b33a9149`。
实际切网关时只改变版本环境变量，原有 318 个账号、230 个角色和存档以及 24 项配置保持。
[切换回执](backend-rollout.json)。

失败后仅删除本轮有精确 SHA 的版本覆盖文件，恢复 e0 网关和 Fo 的目标／域名映射；
**没有恢复数据库**。当前为 320 个账号、232 个角色／存档，原 318／230／230 行逐字节保持，
24 项原配置 SHA 保持。两场新 QA 进度保留。
[网关回退](backend-rollback.json) · [只读保全核验](preservation.json)。

独立 alias、Web `/version`、网关 `/playtest/health` 均读回 e0／Fo；
简体、繁体真实二维码矩阵、待手机确认及取消后零轮询通过。所有自有浏览器、SSH 和本地测试进程关闭，
15 个私有构建覆盖恢复，原 29 份内容 WIP 的原始 SHA 保持。
[独立 alias](alias-final.json) · [最终读回、简繁和关闭状态](outcome.json) · [证据 SHA](evidence-index.json)。

## 下一项的依据

R2 的运行包在首帧后 1.9 秒已完整，最后 SC 字体完成与 Native 观察仅相隔 394 ms。
字体时间线包含初始合并范围正文到响应头后 15,002 ms 被取消、重复当前 CDN 的 8 秒等头、
多个整源 7 秒失败，以及后续再试范围，最后才完成独立 app 的 SC 整包。
这说明该场被字体交付拖住；不能把不同公网场次的差异全部归因于本次改动。
[完整白名单时间线与边界](font-tail-diagnosis.json)。

下一项做受控复现：分段正文持续有进展却碰到总计时，以及已失败来源的重复等待／后备次序。
保留完整字节、配置／取消边界和原公网门槛，先修复、验证，再考虑新的候选。
停止原样重发 306。首次 Q、语言像素、其他地图／线路、三职业完整 1–30、阿洛和 Native 同档等总目标继续。
