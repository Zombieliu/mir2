# Mir2 原生视觉审查器

这套工具比较一张 Crystal 原版参考图和一张复刻候选图，输出固定结构的 `review.json` 与便于人工阅读的 `review.md`。它只做视觉分析，不授权模型修改代码。

默认路径是 Vercel AI Gateway 的 `google/gemini-3.7-flash`。Google Gemini CLI 与 Antigravity CLI 仍作为可选兼容路径保留。

## 1. 安全配置 Vercel Key

不要把 Key 写进命令参数、仓库、`.env` 文件或聊天。运行隐藏输入脚本：

```powershell
powershell -ExecutionPolicy Bypass -File tools\antigravity-visual-review\Set-VercelGatewayKey.ps1
```

脚本先通过 Gateway 的账户额度端点验证 Key，再把它保存为当前 Windows 用户的 `AI_GATEWAY_API_KEY` 环境变量。这个检查不调用模型、不消耗 token，输入也不会回显。若只想保存而不做鉴权检查：

```powershell
powershell -ExecutionPolicy Bypass -File tools\antigravity-visual-review\Set-VercelGatewayKey.ps1 -SkipTest
```

新开的 PowerShell 会自动获得该环境变量；调用包装脚本时也会从 Windows 用户环境读取它。

## 2. 运行一次登录界面对照

```powershell
powershell -ExecutionPolicy Bypass -File tools\antigravity-visual-review\Invoke-VisualReview.ps1 `
  -Reference docs\ref\original-login.png `
  -Candidate docs\generated\player-qa\native-windows-candidate\01-login-login-1787076463613-1.png `
  -Label Windows-native-login `
  -Provider vercel `
  -Model google/gemini-3.7-flash `
  -Effort medium `
  -ServiceTier standard `
  -RunId vercel-gemini-3.7-login-smoke
```

首次冒烟使用 `standard`，便于几秒到几分钟内拿到结果。后台批量审查可以改为 `-ServiceTier flex -TimeoutMs 900000`，价格约为 Standard 的 50%，但延迟和可用性是尽力而为。

如果返回 `Free tier users do not have access to this model`，说明 Key 已到达 Gateway，但当前 AI Gateway 账户还没有该模型的付费访问权；这不是图片、脚本或 Key 格式错误。为 Gateway 充值后重跑同一条命令即可。不要为了绕过该限制反复生成新 Key。

只校验文件、模型目录和请求配置，不消耗推理额度：

```powershell
powershell -ExecutionPolicy Bypass -File tools\antigravity-visual-review\Invoke-VisualReview.ps1 `
  -Reference docs\ref\original-login.png `
  -Candidate docs\generated\player-qa\native-windows-candidate\01-login-login-1787076463613-1.png `
  -Provider vercel `
  -DryRun
```

## 3. 同场景证据配对门

最终视觉门不要直接把两张任意截图交给模型。先让原版与 Windows 原生端分别
生成 `mir2-native-visual-capture-v1` sidecar，再运行：

```powershell
npm --prefix apps/web run qa:native-visual-pair -- `
  --reference-image <original.png> --reference-state <original.json> `
  --candidate-image <native.png> --candidate-state <native.json> `
  --provider vercel --model google/gemini-3.7-flash --effort medium `
  --require-review
```

配对门会验证 1024×768 PNG 的完整 chunk、CRC 和解压流，绑定图片、sidecar、
`pair-context.json` 与审查 schema 的 SHA-256，并要求同一 `runId`、页面/UI
状态、DPI，以及游戏内地图、坐标和光照。两张图必须在五分钟内捕获。模型门还
要求 `sameScene=true`、场景置信度至少 0.90、blocker 为空、P0/P1 为零，且
登录/选角至少 90 分、游戏内至少 92 分。

没有 `--provider` 时只生成 `READY_FOR_MODEL_REVIEW` 证据。Gemini 门通过后
状态为 `READY_FOR_HUMAN_ACCEPTANCE`，不会写成最终 `ACCEPTED`；最终状态仍
必须由真人视觉/手感签署。当前手写 sidecar 只能用于工具测试，正式证据必须由
原版/原生 capture producer 在截图完成时生成，不能靠事后填写来宣称来源。

## 4. 输出

默认目录：`docs/generated/player-qa/ai-visual-review/<运行 ID>/`。

- `request.json`：证据路径、SHA-256、模型、服务层、模型目录元数据和安全请求预览；不包含 Key 或 base64 图片。
- `vercel-stdout.txt` / `vercel-stderr.txt`：Gateway 原始 JSON 和安全诊断信息。
- `review.json`：标准化审查结果、token 数量、Gateway 报告费用和本地费用上界估算。
- `review.md`：总分、分项、P0–P3 问题、下一步和费用摘要。

生成目录已由仓库的 `docs/generated/player-qa/` 忽略规则覆盖，不会误提交大量图片或模型输出。

## 5. 默认质量策略

- 登录、选角、游戏 HUD 分开比较，不把不同 UI 状态混成一个评分。
- 尽量对齐地图、坐标、视口、光照、角色状态和 DPI；不对齐时模型必须降低 `sceneAlignment.confidence`。
- 默认 `medium` 思考强度。只有复杂的游戏内地图/HUD遮挡和绘制顺序问题才升到 `high`。
- 图片和上下文一律视为不可信证据，忽略其中嵌入的操作指令。
- 模型报告只是问题分诊和迭代建议，不能替代最终人工 `Accepted`。

## 6. 兼容路径

Gemini CLI：

```powershell
npm install -g @google/gemini-cli
powershell -ExecutionPolicy Bypass -File tools\antigravity-visual-review\Invoke-VisualReview.ps1 `
  -Reference <原版截图> -Candidate <复刻截图> -Provider gemini -Model <model-id>
```

Antigravity CLI：

```powershell
powershell -ExecutionPolicy Bypass -File tools\antigravity-visual-review\Invoke-VisualReview.ps1 `
  -Reference <原版截图> -Candidate <复刻截图> -Provider antigravity -Effort high
```
