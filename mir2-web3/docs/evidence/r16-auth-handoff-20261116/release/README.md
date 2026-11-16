# 新源发行控制器准备

固定应用源 fea64484b6a25ccd59a22d8f52a5718aa6e57df6，正式基线保持 e0ceea22964c03041d45e49e506b3a8badd6984e／dpl_FoNUgjSRpw56hwUFDNrSs7LZZQFN。独立 Vercel alias／deployment 及 version／Gateway health 已匹配；本记录未切流。

独立本机 Git 与 GitHub 比较了 9 提交、139 文件，两段分别 90／59 文件，固定每项 blob。只改允许的控制器来源／闭包字段和对应测试期望，四套共111项发布保护通过；原截止、完整资源、回退和防并发变更检查保持。

同源后端构建 [43809422151](https://github.com/Zombieliu/mir2/actions/runs/43809422151) 在记录时仍进行中，不能宣称取得发行资格。本地认证候选与原失败见 [本轮说明](../README.md)。后续仍须构建完成／制品 SHA／隔离旧档普通协议／新候选检查及公网原流程；这不是已发布回执。
