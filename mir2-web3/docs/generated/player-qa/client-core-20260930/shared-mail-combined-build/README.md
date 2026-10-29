# Shared Mail：Windows / Web 组合构建（Progress57）

2026-10-04。本阶段完成代码与有限构建检查。Windows、Web 沿用共同 Rust Mail 编辑器、状态和 painter；本轮补齐发布构建链，不改变玩法协议。用户要求继续代码、暂不操作界面。

代码和有限组合构建已接受；当前客户端/UI、移动设备、100% Candidate 与整体 goal 保持未验收。

## 代码与检查

本轮修改三处 Web 构建脚本：将固定 Binaryen 131 的 `-O1 --strip-debug` 优化扩展至三个 release + QUEST renderer，并保留 dev 与无 QUEST 的既有路径。另在 `tsconfig.json` 增加 `standalone-*/**/*` 排除，防止保留的独立包被误当作源码；所有严格编译选项和源码 include 保留。

实际检查：优化器行为测试 25/25、既有 policy 测试 5/5；既有 builder self-check 的 23 处断言通过，语法检查通过。每项检查的 1,317 对输入保护一致。未新增重复测试。后续真实三包生产构建各有 pinned optimizer status 0；元数据一致性不替代运行语义验收。

## 构建结果

Windows EXE 已由此前本批 Native 编译生成并封存：104,249,344 B，开发构建（优化并含调试信息），未启动。本轮四个 Web 构建/配置改动未影响 Native Rust 与 Core 19 个输入，因此沿用已核实的 Native 结果；此前 Node 86/86 和 Rust 92 次通过执行仍是历史检查，92 不是唯一用例数，本轮未重跑。

生产 Core 当前为 `b17ef64191a90ec1d9d50fce20fc6816a6c567177b2bae6c1806cab36cf40c8c`，仅 JS 17,316 B 和 WASM 259,714 B 两叶；WASM 严格低于 262,144 B。原 wrapper 的预估版本与真实版本不符，原 outer exit 1 保留；Root 根据真实 producer/Cargo 成功、19 输入 fingerprint 和两份独立复核另行接受真实生产版本。生产 JS 与 QA JS 的差异仅为 49 B 非执行注释，WASM 字节相同；QA 产物未作为生产输入。

三个真实 renderer 已发布为 `bevy-d59dd56cdd0c6b24`，各自优化器 status 0，实际 producer exit/close 0，四份 Cargo 回执正常完成。预算未改：WASM ≤32,505,856 B，gzip ≤7,340,032 B，JS ≤204,800 B。

| Renderer | WASM B | gzip B | JS B |
| --- | ---: | ---: | ---: |
| WebGPU | 30,057,627 | 5,771,583 | 123,073 |
| WebGL2 | 21,837,318 | 5,339,482 | 121,284 |
| WebGL2 shared | 31,020,155 | 6,109,978 | 121,284 |

Next03 真实生产编译 exit/close 0：Webpack、严格 TypeScript、13 个静态页面生成通过，约 84 秒。编译时 Core/Bevy manifest 与当前生产文件一致。仅追加两条03生成类型 include、将 next-env 路由 token02 改为03；排除规则保留，next.config 和 tsbuildinfo 字节未变。

Thin03 使用 `--skipBuild`，实际打包 exit/close 0：**375,942,789 B（358.53 MiB）**，7,299 文件、775 目录、零链接，低于原 **377,487,360 B（360 MiB）** 预算 1,544,571 B（1.47 MiB）。包内 Core 仅当前两叶，Bevy 仅当前六个程序文件与两份 manifest；复制后的 required-server-files 与 Next03 相同。新包目录为 `apps/web/standalone-shared-mail-combined-03`。本次服务端运行依赖按 Windows x64 生成；这不等于移动端客户端或设备验收。

Next/Thin 每次均保护 476 Source、90 references、98 runtime、4 Next metadata 与22 exact aliases，共712项。Next的709个非受控输入保持字节一致，其余3项只允许明确的生成元数据变更；Thin前后全部712项字节一致。Core六叶与Bevy152个 immutable历史叶及备份保留。完整Next及依赖库存共43,897个文件由有限CLI逐文件hash并确认前后成员/metadata稳定；独立复核另核完整成员/尺寸/alias和关键文件，未宣称重复逐文件hash。源码、实际Canonical02、Next03、Thin03有限复核均无剩余确认问题。

## 保留的失败

Thin01 为379,432,951 B，超原预算1,945,591 B，实际exit/close1；旧输出与失败回执保留。Next02 Webpack通过但严格TSC误扫描保留的standalone目录，actualexit/close1；本轮目录排除修复后Next03完整TSC通过。Core原outer版本预测失败仍保留，真实producer和另行接受记录分别记录。预算未放宽，失败产物未删除、覆盖或作为本次成功证据。

## 验收范围与后续

本轮没有启动浏览器、原生窗口、renderer Instance或HTTP服务。真实登录、战斗、邮件操作、保存重登、IME/剪贴板、触屏与Android/iOS真机、最终frontend及整体Candidate仍未验收。既有44项依赖trace warning与远端原始媒体要求保留；远端不可变资源覆盖、release:doctor及浏览器smoke未在本轮执行。继续推进可独立完成的共享代码任务，实际界面验证遵从用户当前暂不操作界面的限制；整体goal保持active。

## 主要证据

证据根目录为 `C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-combined-build-01`。

| 阶段 | 实际结果 / Root有限接受 |
| --- | --- |
| Native | [EXE编译记录](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-combined-build-01/native-build01/root-native-artifact01.json) |
| Core | [真实生产接受](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-combined-build-01/root-production-core-acceptance01.json) |
| Canonical02 | [实际结果](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-combined-build-01/canonical-optimized-build02/canonical-build-result01.json) · [有限接受](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-combined-build-01/root-canonical-production-acceptance02.json) |
| Next03 | [实际结果](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-combined-build-01/next-production-build03/next-build-result01.json) · [独立复核](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-combined-build-01/next-production-build03/independent-actual-review01.json) · [有限接受](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-combined-build-01/root-next-production-acceptance03.json) |
| Thin03 | [实际结果](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-combined-build-01/thin-production-package03/thin-package-result01.json) · [实际尺寸](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-combined-build-01/thin-production-package03/thin-client-size01.json) · [独立复核](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-combined-build-01/thin-production-package03/independent-actual-review01.json) · [有限接受](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-combined-build-01/root-thin-production-acceptance03.json) |
| TS修复 | [精确源码变更](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-combined-build-01/root-tsconfig-generated-output-fix01/root-source-application01.json) |
| 30项Node检查 | [实际检查汇总](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-combined-build-01/renderer-all-backends-validation01/root-actual-focused-checks01.json) |
