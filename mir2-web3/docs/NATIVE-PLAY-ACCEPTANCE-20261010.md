# Windows 原生实机验收记录（2026-10-10）

已通过启动器实际更新并打开 R23。**尚未登入角色，22 项游戏内验收全部未运行；不代表游戏玩法或 Candidate100 已通过。** 用户已授权操作游戏，登入需由用户手动完成，之后继续逐项操作和截图。

本机原为 R19，首次启动检查 154579 个文件耗时 457535ms，随后 R23 的资源下载返回404，更新失败后启动旧版。现场核实四个文件确实未上传。按照已有发布清单的完整长度和SHA补齐772163B，未覆盖已有文件，未修改签名、清单或feed，未重启网关。公网九个资源/版本/签名入口完整读取全部匹配；正常启动器重试已实际激活sequence18和R23。安装EXE为111343104B，SHA256为`cf7bdb16ad307a744a910f3e17b7d21c78482e556676788bab7b0682913171e1`。

重试的本地检查仍耗时414474ms（6分54秒），暂存8524ms、安装284ms。新增下载四个资源772163B；包含清单等的wireBytes为42880824。**逐文件检查的等待仍需优化，本轮只修复缺失资源，不宣称启动性能问题已解决。** 这四个补发文件只覆盖本机R19→R23实际缺口，没有验收所有更旧版本的升级路径。

| 实机项目 | 本轮实际结果 | 验收范围 |
| --- | --- | --- |
| 正常更新、启动 | 修复后通过 | 已核安装R23版本、EXE及四个资源完整哈希，原CMS更新器接受sequence18 |
| 分辨率设置 | 有界通过 | 1024×768自动→1280×960实际放大，截图含边框由1026×800变1282×992；恢复自动；其他设备DPI和游戏内布局未验 |
| 九种语言基础显示 | 有界通过 | 繁中、英文、巴葡、俄语、印地语、印尼语、越南语、泰语、阿拉伯语的登入前标签实际切换和观察；俄语/越南语长按钮换行；不是整游戏翻译或专家字形验收 |
| 新手、买药、滚轮、战斗/移动、挖矿、钓鱼、加工、任务、行会、师徒、沙巴克 | 未运行 | 需手动登入测试角色；双账号、职业/等级和活动时段另需满足条件 |
| 新制作加工与赌石 | 未发布，未运行 | 当前服务器c704、客户端R23不包含本制作加工/赌石分支，须配套隔离部署及持久存储验证 |

客户端已恢复繁体中文和自动1024×768，停在空登入页。本轮没有输入账号、密码或提交登入，没有改在线账号存档，没有把历史协议QA或单元测试算作实玩。[computer-use技能](C:/Users/Administrator/.codex/plugins/cache/openai-bundled/computer-use/26.1002.52244/skills/computer-use/SKILL.md)引用的[guidance](C:/Users/Administrator/.codex/plugins/cache/openai-bundled/computer-use/26.1002.52244/docs/guidance.md)明确要求：`Do not automate user authentication dialogs.` 登入后按新手任务→买药/滚轮/背包→移动/战斗/快捷键→矿镐/矿区→钓鱼/加工→日周常与多人系统顺序继续。

本轮有效证据含16张已观察到的窗口截图；遮挡到其他应用的截图不接受、不保存。原始失败记录保留：最初只读检查用了不存在的unit名称，后续确认实际`mir2-playtest.service`健康；首次补发dry-run因暂存权限664而拒绝，四个独占暂存文件收紧到600后通过，公开写入仅执行一次。正式补发前后签名/清单/Caddy等七项pin及网关/Caddy进程均保持。

证据：[实际结果与待验收清单](generated/native-play-acceptance-20261010/acceptance-result.json)、[逐份证据索引](generated/native-play-acceptance-20261010/evidence-index.json)、[公网完整字节核验](generated/native-play-acceptance-20261010/distribution-public-verification.json)、[实际安装版本](generated/native-play-acceptance-20261010/installed-r23-verified.json)、[更新事件投影](generated/native-play-acceptance-20261010/updater-events-projection.json)、[R23待登入截图](generated/native-play-acceptance-20261010/screenshots/016-r23-final-waiting-login.jpg)。当前整体进度没有可信百分比，游戏内/人类前端验收仍未完成。
