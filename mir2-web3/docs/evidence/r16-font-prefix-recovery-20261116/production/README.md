# e0 字体断点恢复成对发布与公网验收

2026-11-16（Asia/Shanghai）。[正式游戏](https://numeron.obelisk.build/game) 的 Web 与 Gateway 均为应用源码 `e0ceea22964c03041d45e49e506b3a8badd6984e`，Vercel 为 `dpl_FoNUgjSRpw56hwUFDNrSs7LZZQFN`。发布控制器为 `2dd2d1ef3c3ca899e1a488caa7d7873be97fddb8`，保留的回退基线为 `6ba38557dd`／`dpl_G8iq4UBFibrWSEaSbwahtboeAom3`。

本次交付是整源字体读取中断后保留已校验分片，并完成配套发行；不是整体 Windows/Web 1:1 验收。开发和本地故障复现见[上级记录](../README.md)。

## 构建、来源与隔离旧库

- [配套后端构建 43766785046](https://github.com/Zombieliu/mir2/actions/runs/43766785046) 成功；下载 artifact `14183931894` 后重新核对 ZIP SHA、1,841 个包内文件、310 项共享世界及网关测试、216 项原生 PostgreSQL 测试，零跳过。[发行包回执](backend-artifact.json)
- Linux ELF 为 93,576,560 字节，SHA `22bb54130b2084a34c31d7b0b7bd604cc99a260a08e8d924a5513b329ee4ec96`，与前一版程序字节相同。新构建、测试与 SOURCE-SHA 仍按 e0 独立核对；远端仅在新包核验后复用相同 SHA 的传输缓存。
- 来源闭包为 4 提交、49 文件，35／18 文件两个检查点均与 GitHub 比较匹配。[Web stage](web-stage.json)、[保护检查](web-check.json)及[独立 READY 核对](independent-candidate.json)完成后，才执行发布。
- 以当时正式 6ba 的实际 PID、23 份配置和同一导出快照建立私有备份、隔离 PostgreSQL 克隆及独立 Redis。原有 314／226／226 行完整恢复后，用普通注册、登录、新建战士、移动、两次正常注销及同角色重登验收；协议发包至移动确认 109.997 ms，观察到 1 条中立怪物行走，未发 QA／传送命令。[隔离回执](server-canary-qualification.json)
- 隔离服务停止后再检查零在线连接、存档和配置；实际切换仅改变 `MIR2_DEPLOY_REVISION`，保留数据库和全部原配置。[切换回执](backend-rollout.json)

Web [stage 43767689429](https://github.com/Zombieliu/mir2/actions/runs/43767689429)、[check 43768607552](https://github.com/Zombieliu/mir2/actions/runs/43768607552)、[promote 43770557755](https://github.com/Zombieliu/mir2/actions/runs/43770557755) 均成功。[发布回执](web-promote.json)与[最终独立域名映射](alias-final.json)指向同一 Fo 候选。

## 原两场公网门槛

沿用原 23 个冻结脚本、完整行走载荷／PNG 校验、六套完整字体 SHA、113 项 HUD、移动 8 秒、行走 65 秒、Native／运行包 120 秒及原准备时限。只更新应用、回退基线、候选、场次名和实读钱包版本。原分类器 SHA `17086d553a3eebddb9a656d09c94024417081656e1e3c939a6bc3916f61ee402` 不变；未用补充观察时限覆盖原门槛。

| 独立公网场次 | 物理输入至权威移动 ACK | 运行包正文，首帧后 | 共享 HUD，首帧后 | 原分类器 |
| --- | ---: | ---: | ---: | --- |
| R1 | 199 ms | 16,322 ms | 47,939 ms | 通过 |
| R2 | 141 ms | 7,601 ms | 42,927 ms | 通过 |

[R1 原判定](public-r1-acceptance.json) · [R2 原判定](public-r2-acceptance.json) · [分项摘要](public-summaries.json)

两场均使用普通公网 `/game`、官方 Testnet 钱包和真实签名登录；每场只执行一次授权确认，禁用的钱包操作计数为零。正常进比奇、真实键盘移动、完整资源、Q 开关、Worker 停止后重启、持久缓存及退出至选角均通过，所有浏览器关闭。它们实际使用 WebGPU，不能据此认定公网 WebGL2 已验；本地共享 WebGL2 证据另见上级记录。这些时序也不能单独证明本次分片修复带来的因果提速。

简体、繁体额外各一次真实矩阵出码、待确认与取消通过：各创建 1 次，取消后新增轮询 0，浏览器关闭。[简繁回执](public-qr.json)。官方手机尺寸浏览器完成了签名流程；**没有使用实体手机相机验收**。

## Q 的实际阶段与画面边界

初次 Q 两场都是 `ordinary-react-fallback`。共享 HUD 门槛后的 Q 两场均为 `ordinary-shared-native`，实际 owner 为 `bevy`，开关与零世界指令检查通过。[分阶段记录](public-q-phases.json)。原分类器的 `sharedNativeQuest` 字段本身允许备用窗口，不能只看字段名就认定 Native。

- R1 [早期 Q 抓图](r1-first-q-black.png)是 1024×768 全黑 PNG，SHA 与上一正式 6ba 的 R1 **完全相同**。它不构成早期画面验收，仍需区分抓图问题和实际短暂黑帧；没有修改原脚本或重写该结果。
- R2 [早期备用窗](r2-first-q-fallback.png)实际可见，但处于 0／113 加载阶段，底部 HUD 与小地图纹理未齐。
- [R1 Native Q](r1-native-q.png)、[R2 Native Q](r2-native-q.png)及[Worker 重启后完整画面](r2-after-worker-restart.png)已逐图查看。繁体环境仍有简体控件标题、英文 NPC 名及系统消息，语言像素对齐保持开放。

[原 PNG 的像素及 SHA 观察](public-q-pixels.json)保留了上述区别。此次不能关闭“冷启动第一次 Q 已经与 Windows 一致”或全部视觉验收。

## 线上数据与收尾

最终 `/game`、`/version`、独立 Gateway `/health`、公开 Worker 均 HTTP 200；两端 revision 为 e0，公开 Worker 68,357 字节／SHA `9c301d24217f498408d77eb0b38f88f9c6825020d7d380ed2c383022db615f97` 与候选完全一致。[最终读回](final-public-readback.json)

原 314 个账号、226 个角色和 226 份存档逐行保持；测试新增的 2 组账号／角色／存档保留，总数为 316／228／228。记录的原 23 份配置 SHA 保持；未恢复或覆盖生产数据库。[数据保全](preservation.json)

29 份原始 WIP 内容 SHA 与 33 条原 tracked 状态保持，12 份私有构建覆盖已恢复。所有本轮浏览器、本地服务、隔离网关／Redis 和 SSH 控制器均已关闭。一次错误文件指纹在本机校验处被拒绝，未发送远端命令；更正后按固定主机指纹重新连接。未将凭证写入本记录。

下一项先定位早期 Q 黑色抓图／画面与 Native 接管，再继续语言、地图、长路线、三职业完整 1–30、战斗经济、阿洛及 Native 同档等适用差距。既有失败记录、人类 Windows 窗口及实体手机验收保留；整体 Candidate／Accepted 均未宣告完成，Goal active。

[证据文件 SHA 清单](evidence-index.json)
