# 新源码的发行准备

固定应用源码为 f05fb757ef4dcac75dd5bfbdd2b23ddb67ce810b。独立 Vercel alias／deployment 与公网 version／Gateway health 均读回正式基线 e0ceea22964c03041d45e49e506b3a8badd6984e／dpl_FoNUgjSRpw56hwUFDNrSs7LZZQFN，本记录没有切换生产流量。

本机 Git 与 GitHub 独立比较三段完整来源：14 个提交、合并后 237 个文件，分段文件数 90／59／105；每项路径、状态和 blob 均固定。控制器仅修改允许的来源字段及对应测试期望，111 项发布保护全部通过、无跳过。原超时、资源 SHA、回退和并发变更保护保持。

同源 Linux 构建 [43842463384](https://github.com/Zombieliu/mir2/actions/runs/43842463384) 在记录时仍进行中。尚需构建完成、制品校验、隔离旧档 canary、新候选检查及公网原流程，不能把这份准备记录当作已发布或通过整体 1:1 的回执。原 8／65／120 秒、完整六字体与 113 项 UI 资源、正常退出和同角重进要求保持；历史失败保留。

本地 73 项回归、两后端与 48 次快速 Q 开关的范围见 [本轮说明](../README.md)。
