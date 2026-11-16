# Progress60 · 普通仓库组合构建

用户要求继续代码、暂不操作界面。本批把 Progress59 共享仓库源码纳入新的 Windows EXE 与三种生产 renderer，并新增产物级 Storage 导出校验。以下是有限源码/构建证据，实际玩家界面与设备验证保持 open。

## 已核实产物

- Windows development profile EXE：104,290,304 B，SHA256 `672766fd30439295b1218b8c5035517657160dbeea03016bd5931d926f76d4d2`。实际 Cargo exit0，Policy B completed/exited/disposed；未启动 EXE。
- Core 复用已验收生产版本 `b17ef64191a90ec1d9d50fce20fc6816a6c567177b2bae6c1806cab36cf40c8c`：19 输入、完整14个 Rust 成员、指针和精确两叶产物均匹配；WASM 259,714 B，严格低于262,144 B。没有声称重新编译 Core。
- Renderer 新版本 `bevy-5d8af086efcc7155`：三次真实 release+QUEST 构建与固定 Binaryen131/O1 优化均 exit0，metadataMatched=true。

| renderer | WASM B | gzip B | JS B |
|---|---:|---:|---:|
| webgpu |30,288,723|5,832,040|125,238|
| webgl2 |21,877,611|5,353,137|123,212|
| webgl2-shared |31,251,283|6,172,584|123,212|

各包原预算保持 WASM ≤32,505,856 B、gzip ≤7,340,032 B、JS ≤204,800 B；没有提升预算。三对当前 JS/WASM 静态验证7个唯一 Storage 函数导出，生成 JS未执行，WASM只读取 Module 元数据，没有 Instance/renderer启动。旧 lean GL2 仍保留其能力降级，导出存在不表示 startupReady 或 compiled=true。

## 输入与验证

原 Source490 保持；加入37个显式编译非 Rust 输入和2个校验脚本，当前 Source529。Native 输入闭包369；三种 renderer 构建前后529源码及备份一致，已有 Core6/renderer152叶保留，新不可变 renderer恰好7叶。

新静态校验脚本实际16/16通过；旧 renderer 的负例实际exit1，因缺少 Storage 导出被拒绝。Progress59 的 Web150/150与 Rust Native113/portable56/runtime20 pass executions按原配置保留，本批没有重新执行或把跨配置执行相加为独立测试总数。

## 完整 Web 与独立包

实际 Next01 Webpack、严格 TypeScript 与13/13静态页面生成成功，spawn/exit/close均0。编译嵌入的 Core/Bevy manifests 精确绑定当前产物；源码529、refs90、runtime105、metadata4、aliases22，构建前后772项保护核对通过。仅 `tsconfig.json` 与 `next-env.d.ts` 发生受控变更；`tsbuildinfo`、`next.config.ts`保持。

有限 inventory CLI实际对 Next19,006 文件与依赖24,891文件做43,897个摘要核对，文件/目录/stamps/唯一受控node_modules alias前后稳定，未读私有env内容。

独立包 `[standalone-shared-storage-combined-01](E:/mir2-player-journey/mir2-web3/apps/web/standalone-shared-storage-combined-01)`：**376,489,742 B（359.05 MiB）**，7,299文件、775目录、零链接。原360 MiB预算为377,487,360 B，余997,618 B。实际包装工具和外层均退出0；复制后的 Core恰好2叶、当前 renderer closure恰好8叶（6产物+2manifests），三对复制 JS/WASM均通过7个Storage导出检查。没有执行JS renderer或创建WASM Instance。

所有旧独立包、Core与renderer release及失败记录保留；旧Core外层预测版本失败仍为失败，当前仅复用另行验收的真实b17生产Core。既有不可变 original-ui 远端资源代理要求仍在：本批未配置/访问该远端或做HTTP资源覆盖验收。

## 实际记录与审查

证据根 `C:/mir2-cross-platform-storage-20261002/repo-qa-shared-storage-combined-build-01`。Root负责架构/集成/构建，Sol high按单文件单写者实现有界校验器和有限工具；C2/M13/M14各做声明范围的只读审查。canonical、Next、Thin独立实际审查均0个确认blocker，Thin所有7,299文件pins已独立核对。

| 记录 | SHA256 |
|---|---|
| root-source-snapshot01.json（529） |cbf1dba815678952647dbe09ba78994e50a14735b072e3f8d8b10c8a1ca0c5d8|
| canonical-build01/canonical-build-result01.json |f2a7cf43c6578e31fa68ddd195a18ef35863364476e665f9d08192c80fe0f502|
| next-production-build01/next-build-result01.json |4f88ede77a43106220583ffabe0782d2a40593e59b103be4b38b9e28db5a7e02|
| thin-production-package01/thin-package-result01.json |75ab57b273dcf3aa252ce0d3e0cdb38ec39e2aa8a29652553f6d3499aa7cccfb|
| thin-production-package01/thin-client-size01.json |58624f0f8d8a93d5c6298eadcbbcae1f11342e09e79952f444716d7851e73d89|
| root-verification01.json |f7201c66b151b93f219811c0669280ec3a28f378181eca2678e4bbeb0a0cffaa|

## 下一项与未验收范围

继续普通NPC金币买入共享planner/painter/host。源码盘点已确认：BuyItem绑定catalog UserItem.unique_id；多件价格应按原始basePrice×qty再乘rate的服务端顺序；普通商品count不是有限库存，背包/腰带可堆叠容量不能简化为空格计数。现有协议没有购买专用ACK，静默失败和未知结果须与效果观察/服务退役分开，不发明requestId、不自动重发，也不继承旧无限库存pending为完成证明。保留其他商店与交易入口。

本批只验收有限源码、编译与打包。实际仓库存取ACK、密码/租赁、登录/战斗/保存重登、IME/剪贴板、DPI/触屏、Android/iOS真机、远端媒体覆盖及最终frontend/Candidate仍待独立验收；整体goal保持active。用户暂不操作界面的限制持续有效。

文档发布后的边界交接：refs90内有6个文档引用随本次已声明文档更新改变，529产品源码不变。旧边界仍保留为构建当时证据，当前文档指纹另存最终交接；旧before-pin核对差异保留，不冒充新的构建失败或产品测试通过。初始差异说明只写了首个队列引用，最终交接已修正为6个已声明文档引用。
