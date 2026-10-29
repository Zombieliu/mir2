# Progress58 · 普通个人仓库存取规则与发送边界

2026-10-04。本批完成源码和有限回归检查。按用户“继续代码，暂不操作界面”的要求，未操作浏览器或原生窗口。整体 goal 保持 active。

## 已完成的代码

- 从 Native 拖拽处理提取公共 Rust `storage_interaction`：三个纯 planner 处理背包→仓库、仓库→背包、仓库内移动，使用当前权威模型复核 slot/UID、密码、容量、重复单元及 UID；完整且当前一致的模板/数量才允许合并。保留原有满目标空格回退、同槽 no-op、队列和 pending 回执语义。
- Native 快捷取回按权威 `bag_slot_capacity()` 内的实际空格判断，修复固定 46 格计数导致基础背包判满和扩展容量判断错误的问题。现有装备、密码、租赁和 Gateway 处理未改。
- Web 存入来源和取回目标明确携带 Bag1/Bag2/Storage 单元。Bag2 页内编号转换为协议 `40 + slot`；使用 `authoritativeUniqueId`，拒绝 display fallback、已替换来源或重复身份。
- 发送前消耗请求编号并登记既有 receipt Map；同步 `mir2:action` 后再复核当前 owner/socket、来源、目标、容量、保护锁和冻结 DTO。确定未发送只取消该 reservation，已进入 `socket.send` 后抛错保持未知结果，普通刷新不释放，精确 V2 ACK/NACK 才结算。
- reservation 同时保护来源和空目标格，阻止冲突的物品操作、邮件附件/邮票、报价/发送及无法证明入包目标的收件操作。允许 Mail unlock 清理。角色退役、新连接和成功 StartGame 清理旧 reservation，编号不重置。
- React 两个仓库提交回调返回 boolean；拒绝时显示失败并保留选择，取回目标使用该次点击捕获的页签。按 React skill 复核：没有新增全局事件监听、effect、网络请求或渲染状态同步；发送所需临时所有权保存在 ref 中。

## 实际验证

| 检查 | 结果 |
| --- | --- |
| Storage 纯 TS adapter 与实际 Page/回执/UI callback AST 测试 | 24/24 |
| 邻接身份、装备 adapter、Spells raw ingress、Combat 测试 | 72/72 |
| `mir2-client-bevy --features native-ui --lib storage` | 105/105 |
| `mir2-client-bevy --features portable-quest-ui --lib storage` | 34/34 |
| 严格 TypeScript `--noEmit --incremental false` | exit 0 |
| 本批文件 `git diff --check` | exit 0 |

Rust 共 139 次通过执行，包含跨配置重复测试，不是 139 个独立场景。Cargo 顺序执行、locked/offline/jobs1/inc0，复用逐次 50 GiB 磁盘守卫；两次实际 C 盘余量分别为 304,380,370,944 B 和 279,836,839,936 B，均通过。`.NET` 子进程 completed/exited/disposed 与 Policy B 后置证据均通过，321 个 Rust 输入前后一致。

本批最终 Source480：14 个既有变更、462 个既有保护、4 个新增。新增文件是 Rust planner、Web identity adapter 和两个专项测试。邻接初次 57/72、第二次 71/72 的失败记录保留；问题为旧夹具缺依赖及注释中的 `await` 被文本断言误识别，补齐隔离夹具并改为真实 AST 后通过，原行为断言未删。

原始证据：[Root 汇总](C:/mir2-cross-platform-storage-20261002/repo-qa-storage-transfer-01/root-verification01.json)、[Source480](C:/mir2-cross-platform-storage-20261002/repo-qa-storage-transfer-01/source-snapshot-final01.json)、[Storage Node](C:/mir2-cross-platform-storage-20261002/repo-qa-storage-transfer-01/storage-node01.tap)、[邻接 Node](C:/mir2-cross-platform-storage-20261002/repo-qa-storage-transfer-01/adjacent-node03.tap)、[Native Rust](C:/mir2-cross-platform-storage-20261002/repo-qa-storage-transfer-01/native-storage01.result.json)、[portable Rust](C:/mir2-cross-platform-storage-20261002/repo-qa-storage-transfer-01/portable-storage01.result.json)。

## 尚未完成

Web 当前仍由 React 绘制仓库，并保留既有 receipt Map；公共 Rust planner 已用于 Native，并在 portable 配置编译/测试，但浏览器共享 Storage host、意图出口与 common painter 尚未接入。密码、租赁、公会仓库属于独立流程，不能据此标记完成。下一批贯通普通仓库共同控件树、模型和意图，随后重建组合产物。

本批没有生产 EXE/Core/renderer/Next/独立包重建；Progress57 的产物仍是其原 Source 的历史证据。没有启动 renderer Instance、HTTP、实际账号、客户端界面或真机。登录、存取实际 ACK、保存/重登、IME、触屏与 Android/iOS 证据、最终 frontend/Candidate 均未据此验收。
