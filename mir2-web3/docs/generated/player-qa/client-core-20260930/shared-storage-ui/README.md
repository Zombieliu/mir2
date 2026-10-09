# Progress59 · 普通仓库共同 Rust painter / portable host / Web 接入

## Progress60 后续组合构建

本文件的Progress59仓库源码已纳入新Windows开发EXE、三种renderer、Next01和359.05 MiB独立包；新增产物静态校验通过，限定复用未变Core。实际UI/存取/密码/租赁/保存重登与移动真机仍保持未验收，用户暂不操作界面。详见 [Progress60组合构建QA](../shared-storage-combined-build/README.md)。


2026-10-04。本批完成源码与有限测试/编译检查，遵守用户“继续代码，暂不操作界面”的边界；整体 goal 保持 active。

## 已完成的代码

- Native 仓库绘制提取到共同 Rust `crystal_ui/storage_paint.rs`，保留原 388×346 布局、页签、80 单元、空操作格、密码/租赁遮罩、当前 UID 选择和到期日期格式。原 overlays 包装调用同一 painter，其他 overlay 边界保留。
- portable Storage 复用共同仓库/背包 painter 和三个 Storage planner，内部背包移动/合并亦复核当前 Rust 元数据。桌面并列、触屏切页/裁剪/滚动布局分开；最小 320×240 CSS 像素布局通过受控测试，未据此接受真机效果。
- Runtime 提供独立 Storage ABI1 的能力、snapshot、status、intent sink、pointer edge 和 exact-identity withdraw。运行/连接/会话/owner、模型/布局/服务版本匹配才启用；旧 terminal 在推进序号前拒绝，world 来源边缘不产生仓库意图。
- Web 薄适配投影当前权威 UID/容量/密码/元数据，先提交禁用 snapshot 并确认控件树就绪，再启用共同面板。独立 ABI 不支持或输入不合格时承接既有 React 流程。轻量 Core 未新增 Storage API，既有精确 capability 对象未扩展。
- Page 接通普通存取、仓库内部移动/合并和背包内部操作。沿用既有 receipt Map、请求编号、来源/空目标 reservation 与精确 V2 ACK；同步 action listener 后，最终 socket 入口再验冻结 DTO、当前 owner/model/服务、物品/目标与 Mail/装备冲突。自身 reservation 不会误拒自己的发送，已进入 socket 后抛错保留未知结果。
- 关闭共同仓库恢复背包；密码一次性交接既有 set/change/unlock 窗口，租赁沿用 `@ADDSTORAGE`。这只完成既有流程接入，没有新建共享密码或租赁 UI。
- Shell 用 downSequence 区分同 pointerId 的新按下，并固定 down 来源直到 terminal。旧 cancel/up、同步重入及旧 hook cleanup 不清理替代手势/owner/sink；arming/active 均阻止世界键盘移动和战斗/物品快捷键。blur/resize/layout/owner 改变撤回租约。Bag hook 首次 owner 回调修复构造期 TDZ。
- 新增有限 `npm run test:storage-ui` 入口。实际测试由冻结 helper 直接执行，未声称实际调用 npm 别名。按已读 React skill 复核：ref 保持当前读数，effect 清理隔离 host 实例，监听/计时器对应撤销，没有新增网络请求或第二套传输 receipt ledger。

## 实际验证

| 检查 | 结果 |
| --- | --- |
| Storage host/model、既有 adapter 与实际 Page 入口纯源码测试 | 48/48 |
| 邻接 Bag/身份/装备/Spells/Combat | 82/82 |
| 实际 Page/Shell/Inventory/hook AST 接入 | 20/20 |
| `mir2-client-bevy --features native-ui --lib storage` | 113/113 |
| `mir2-client-bevy --features portable-quest-ui --lib storage` | 56/56 |
| `mir2-bevy-runtime --features web-quest-ui --lib storage` | 20/20 |
| 严格 TypeScript `--noEmit --incremental false` | exit 0 |
| wasm32/no-default-features/`webgl2-shared-ui` locked offline check | exit 0 |
| 本批产品/文档 `git diff --check` | exit 0 |

Web 合计 150 次通过；Rust 合计 189 次通过执行，包含跨配置重复用例，不是 189 个独立场景。Cargo 顺序执行、locked/offline/jobs1/inc0、64 MiB stack、单测试线程；四次实际 C 盘余量为 275,049,463,808 / 274,840,805,376 / 274,693,021,696 / 273,430,929,408 B，均高于原 53,687,091,200 B 门槛。每次 .NET 子进程 completed/exited/disposed 与 Policy B 后置证据通过，326 个 Rust 输入前后一致。受控 Bevy UiPlugin/ComputedNode 测试执行 CPU 控件树与指针处理，没有客户端窗口或 renderer Instance。

Source490：16 个既有变更、464 个已声明输入保护、10 个新增。Web 检查后唯一既有输入变化是 package.json 新增有限别名；被测 TS/React 模块和编译器字节相同，新增接入测试在 Web04 冻结输入上单独实际执行。绑定已记录，未冒充一次新的 TSC 执行。

独立只读审查和 Root 复核没有剩余确认 P0/P1/P2。初次 TSC 导入/类型问题、Storage 47/48 和邻接 70/82 的旧夹具依赖问题已修复，原断言保留；最初守卫因 review JSON 字段错误返回 90，未转发 Cargo，原记录保留。冻结 Web 输入的 preflight 拒绝也保留。

原始证据：[Root 汇总](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-storage-ui-01/root-verification01.json)、[Source490](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-storage-ui-01/source-snapshot-final01.json)、[输入绑定](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-storage-ui-01/final-source-binding01.json)、[Storage Node](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-storage-ui-01/storage03.result.json)、[邻接 Node](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-storage-ui-01/adjacent02.result.json)、[接入 Node](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-storage-ui-01/integration01.result.json)、[Native Rust](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-storage-ui-01/native02.result.json)、[portable Rust](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-storage-ui-01/portable02.result.json)、[runtime Rust](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-storage-ui-01/runtime02.result.json)、[shared WASM check](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-storage-ui-01/shared_wasm02.result.json)。

## 尚未完成

当前生产 Windows EXE、三种 renderer、Next 与独立包尚未包含本批 Storage 源码；Progress57 产物只代表其当时输入。下一轮组合构建须核对当前生成 JS 与 WASM Module 元数据的独立 Storage 七项导出并保持既有预算，不实例化或扩展旧 manifest keys。legacy WebGL2 为 fallback，不能要求所有 backend 的 compiled/startup 都为 true。未变小 Core 可在完整当前输入图匹配后沿用。

密码/租赁仅通过纯代码交接测试，公会仓库独立。当前 Web 已转成 Number 的 expiry ticks 精度不能恢复，Rust 格式化不等于原始 tick 无损传输。真实 UID/ACK、像素/字体/拖拽、登录战斗/保存重登、IME/剪贴板、移动真机、最终 frontend/Candidate/Accepted 均保持 open。本批未操作 UI、HTTP、账号/存档或 WASM Instance。
