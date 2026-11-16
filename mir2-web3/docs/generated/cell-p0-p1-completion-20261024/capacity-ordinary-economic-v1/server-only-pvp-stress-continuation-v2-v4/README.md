# Server-only PvP continuation v2–v4：各次失败原件

本目录按[显式清单](copied-evidence-manifest.json)保存27份公开原件，源前后及副本SHA256均核对。三次均为独立的离线预置、分散PvP诊断尝试，原false保留，不作为ordinary/P0或容量通过。

| 尝试 | 实际结果 | 清理边界 |
|---|---|---|
| [v2报告](attempt-v2/reports/1793128541388-d6b9ab90.pvp-stress-report.json) | 10人完整300.092秒false；继续向25时实际21入场后V2 timeout硬停，没有完整25人档 | driver cleanup=true；[外层cleanup](attempt-v2/cleanup-v1.json)网关exit0、端口释放 |
| [v3报告](attempt-v3/reports/1793129235637-60e146d6.pvp-stress-report.json) | actor13普通出生点由预定105,80变为104,79；准备失败，无测量 | 原driver cleanup=false及18条未完整normal drain保留；[外层cleanup](attempt-v3/cleanup-v1.json)网关exit0、端口释放，不能覆盖driver失败 |
| [v4报告](attempt-v4/reports/1793130492719-0687c913.pvp-stress-report.json) | 25人登录及12组placement完整，准入71.199秒；随后9秒内Underlying异常，无完整window/stage | driver cleanup=true；[外层cleanup](attempt-v4/cleanup-v1.json)网关exit0、端口释放 |

21入场时的timeout及v3出生点细节为Root已定位事实；原报告中的通用Underlying/准备错误仍原样保留。v4确实有25人入场，不能说只入场14人；monitor在drain阶段的active8/14也不能当peak容量。当前未测50/100/250/500，未完成两小时。真实错误源尚待独立firstError证据，找根因前不据本档案声称server瓶颈或修复通过。

每次归档原report/samples、health前后、cleanup、monitor-current、resourceprofile、runintent及公开fixture manifest。私有Store/pool/账本/keys、Gateway trace日志、录制与EXE均未复制。已有[首轮v1对照](../server-only-pvp-stress-v1/README.md)独立保留，旧10人延迟失败不会因continuation被追认通过。
