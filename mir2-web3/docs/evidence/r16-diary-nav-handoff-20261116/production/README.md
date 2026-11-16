# f05 同源试发：公网登录未通过，已回退

应用源固定为 f05fb757ef4dcac75dd5bfbdd2b23ddb67ce810b，控制器为 b5e31bffc82ead96b0f624a29ccec7eaf4e02b08。三段来源覆盖 14 提交、237 文件，111 项发行保护通过。Linux 原制品完整 ZIP 摘要及 1,841 个文件通过，Linux 共享相关 310 项、Native PostgreSQL 216 项和 7 个后台执行必需用例均通过；普通注册／口令 canary 的 106ms 移动、中立怪漫游、两次正常注销、同角保存及原快照保全通过。[制品](linux-package-verification.json) · [canary](server-canary.json) · [发行准备](../release/README.md)

已实际成对试发到 dpl_EsA9a7nDuU7ggAC2xBUqtdmCCasj；网站和网关独立读回同为 f05。原公网 R1 在官方手机尺寸 Testnet PWA 内用虚拟 Passkey 完成一次同应用／同会话的登录批准，禁止操作数为 0。游戏随后未观察到批准，原失败是 APPROVED_WALLET_LOGIN_NOT_OBSERVED；游戏登录交换 0 次、游戏 WS 0 条、未建角、未到进城或移动。原判定 false，R2 未执行。[原判定](public-r1-original-acceptance.json) · [游戏阶段](public-r1-auth-summary.json) · [批准范围](phone-approval.json)

累计记录 16 个 200 轮询响应与 pending 状态，缺少逐次轮询时间，不能由累计值断言批准后仍返回 pending，也不能确定缓存、正文停滞、过期或唯一根因。此次没有测到公网原 8／65／120 秒、完整六字体／113、Q 或正常退出门槛；本地通过仍限其已记录范围。[被动观察与下一项](passive-failure-analysis.json) · [本地原门槛](../README.md)

已通过认证的 Vercel 连接器按当前自有 Es 域名检查回退到 Fo，后端按固定脚本只移除本轮自有覆盖，恢复 e0ceea22964c03041d45e49e506b3a8badd6984e。正式 PID 为 1832898，全部 322 账号／234 角色／234 存档和原 24 配置保持，没有恢复数据库；本次未创建正式 QA 角色。私有快照、隔离库和全部原失败保留。[Web 回退](web-rollback.json) · [后端回退](backend-rollback.json) · [保全](postrollback-preservation.json) · [HTTP 读回](postrollback-independent-http.json)

恢复后的简繁均各一次真实出码、待确认、取消、停止轮询及浏览器关闭通过。恢复版签名到游戏登录与实体手机相机仍未重测。所有本轮浏览器和 SSH 已关闭，隔离服务停止，27 私有覆盖已恢复，33 份原追踪 WIP 保持。[简中](restored-qr-zh-cn.json) · [繁中](restored-qr-zh-tw.json) · [句柄](owned-processes-closed.json)

下一项先围绕单次真实批准增加被动请求／响应／正文时序，关联批准与游戏轮询交接，保留本次原流程及门槛。完整 Windows R16 → Web 100% Candidate Goal 继续 active，UI／语言和其余玩法／地图／同档及最终人类 Accepted 均保持开放。
