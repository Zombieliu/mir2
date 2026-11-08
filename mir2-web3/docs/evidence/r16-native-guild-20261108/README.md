# Native 公会数据与请求生命周期

本轮仅修改 Windows R16 代码基线的客户端消费、队列和发送桥；服务器规则、协议与存档 schema 均未变。

- 70 项社交、143 项公会限定、29 项 Windows 与6项实际入包队列测试通过；248 次选定执行去重为198项（69新增、129相邻）。
- 两项真实素材 GPU/GDI 图像测试明确未执行，具体名字和原因见 [verification.json](verification.json)。
- 完整名册仅255；其它状态按成员／职级增量处理。权限只由真实status7的actor、嵌套index、目标bit及新鲜cursor确认，元数据不会抹掉已到达回执。
- 普通真实UserInformation经typed分支进入SocialModel，使用realId与name绑定；不拿AOI对象或选角槽替代身份。
- 读请求确定未发即精确释放；真实8槽bounded通道满载后可再次发一次，已接收请求保持等待。24槽普通队列不再淘汰已占8槽额度的受保护公会请求。
- 原失败及首个未资格候选保留；最后候选通过原manifest/lock的Rust1.95 release/offline/locked构建，13直接源码及6个manifest/lock前后稳定，不声称全工作区输入闭包。
- [候选](candidate-final.json)是单EXE，需要既有R16资源；没有启动、安装或替换原游戏。原三服和Windows EXE保持，未调用模型或部署。
- 成员上限、招募／建职阶的精确失败回执、索引重编号并发、Web共用预算、像素／多语言及最终真人验收仍开放。

tools 是实际本机历史脚本，含当时工作树与构建路径；不含密钥或私人测试账号。
