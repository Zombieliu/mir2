# 比奇商人遮挡复测

固定同一 R16 比奇位置 `(288,615)`、普通角色和资源，在旧版与修复版分别进入 `/game`。旧默认 WebGPU 把商人与摊位画到不同的画布层，两个对象各自的行深度虽正确，却无法交错。修复后，同一画布尚未完成地图和角色时，角色与可遮挡物共用 DOM 行顺序。独立图集仍可画地面。Bevy 完整呈现当前视野后，由同一世界画布接管。

- 默认 WebGPU、共享 WebGL2 和独立 WebGL2 均完成冷启动复测。斯科特与摊位同格 `(291,610)`，旧默认路径的 130 个可区分像素全部来自摊位。修复版至少 46 个精确匹配斯科特，部分像素因待机帧不同无法归类。鲁本前方建筑的 42 个对照像素仍全部来自建筑。 [像素比较](pixel-ownership.json) · [旧画面](run-baseline-r1/market-default.png) · [修复画面](run-candidate-r1/market-default.png)
- 真实方向键从 `(288,615)` 走到 `(288,614)` 再走回，分别收到一次移动与权威位置回执。固定 79 页图集时，全世界 Bevy 路径接管 104 个 DOM 遮挡物。禁用 Bevy 地图后，独立图集负责地面，104 个遮挡物与斯科特仍共用 DOM。 [位移](run-candidate-walk-r1/receipt.json) · [图集接管](run-candidate-pinned-r2/receipt.json)
- 四项 Web 地图测试与 TypeScript 检查通过。修复版运行收据中的源码 SHA-256 以 LF 行尾计算，与提交内容一致。验证范围为本机浏览器和固定比奇画面。Windows 窗口、其他地图的逐像素校验和公开发行另验。 [汇总与范围](market-depth-summary.json) · [旧运行](run-baseline-r1/receipt.json) · [修复运行](run-candidate-r1/receipt.json)
