# 共享技能窗口与战斗输入：本机构建检查点

2026-10-03，M10 与 M11 合并验证，版本 `bevy-49c4a8559bee071c`。Windows/Web 的技能窗口、快捷键和普通技能/近战输入已进入同一套共享规则；当前结论限于代码、构建、独立 Web 包和本机启动检查，完整玩家流程尚未验收。

- 共享代码：444 个 Source 文件，本轮改动 17 个；独立审查无剩余 P0/P1/P2。9 项实际 Cargo 检查/测试完成，Rust 通过 388 次执行（跨配置，不能当作唯一测试数）；Node 39 项通过，TypeScript 通过。
- Windows：新的 dev EXE 已构建并复制，尚未启动或验证实际游戏界面。
- Web：三种 WASM、完整 Next 页面构建通过。独立包 375,633,540 B（约 358.2 MiB），7,301 文件、776 目录、0 链接，在原定 360 MiB 上限内。
- 独立副本：所有文件与依赖保持一致，实际 Sharp 图像解码通过。32 项声明的 loopback 页面/资源请求与 19 项资源元数据检查通过；2 张图片来自真实包、3 张来自明确记录的本地测试资源，因此公共资源交付仍待验证。
- 地图生成数据只移除了 JSON 空白，完整解析数据和当前编译加载器的 0/D011 回退结果一致。其余 7,300 文件保持不变。原包超限的 exit/close 1 和第一版 QA 读取次数假设失败均保留，未改写为成功。
- 启动后 7,301 个原文件仍精确一致，只新增 721 B 图像优化缓存和 3 个缓存目录；测试进程与端口已关闭。

[完整本机构建回执](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-combat-input-01/root-aggregate-local-build-acceptance01.json)；[当前 Windows EXE](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-combat-input-01/native-build01/mir2-platform-windows.exe)；[独立 Web 副本入口](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-combat-input-01/isolated-copy01/mir2-web3/apps/web/server.js)。

后续需验证真实登录、技能/近战、任务/装备、保存重登、服务端实际回执、公共资源以及 Android/iOS 真机。Native 队列到 socket 的归属保护已有最小修复设计，尚未实现。编译与本机 HTTP 检查不构成这些项目或整个 Candidate 的验收。此前用户按 Escape 停止了界面操作，本轮未恢复浏览器或原生窗口控制；恢复实际界面验证需要新的明确授权。

不再以缺少验收分母的百分比报告进度；后续按完整玩家流程记录完成项、未完成项和实际产物。
