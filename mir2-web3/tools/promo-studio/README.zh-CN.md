# Numeron 宣传短片工作室

本机创作工具：选题 → 分镜 → 原创情境图 / 游戏录影 → 后台编码 → MP4、封面、字幕、投稿文案。独立于游戏网关，不占用玩家命令队列。

## 启动

需要 Python 3.11+、FFmpeg / ffprobe（支持 libx264、AAC、libass）和中文字幕字体。Windows 会查找 PATH、WinGet 链接及 Microsoft JhengHei；其他系统可设置 `PROMO_STUDIO_FONT`。系统旁白仅在装有对应语音的 Windows 可用。

在 `mir2-web3` 中运行：

```powershell
python tools/promo-studio/server.py --port 3921
```

打开 <http://127.0.0.1:3921>。默认数据目录为 `tools/promo-studio/artifacts/promo-studio`，可以用 `--data-dir` 或 `PROMO_STUDIO_DATA` 改到自己的素材盘。支持 Windows、macOS、Linux 的路径；不假定项目盘符。

开发机本轮实际输出位于 `C:/mir2-promo-studio-20261010`，这只是验收目录，不是程序硬编码依赖。

## 制作一支片

1. 选“今晚，回比奇。”或另外两个玩法模板；比奇模板有繁体中文、英文。
2. 加入 `assets/bichon-reunion-ai.png`，来源选“AI 原创情境”；加入自己录的游戏 MP4，来源选“真实游戏录影”。截图应选“游戏截图”。
3. 核对来源说明和玩法标签，再勾选素材。挖矿、三职业模板需要相应标签的真实素材；不能用比奇街景替代挖矿录像。标签是运营者声明，仍需人工核对内容。
4. 选择画幅及旁白。字幕版无旁白；系统语音使用 Windows 已安装声音，**不是神经网络 AI 配音**。中文自动选中文声音；英文选英文声音。
5. 检查分镜，点击“制作这支短片”。编码在后台进行，可以继续编辑；刷新页面仍可查看队列。
6. 预览 MP4，下载封面、SRT 和投稿文案 JSON。文件保存在数据目录的 `jobs/<job-id>/`。

直式为 720×1280，横式为 1280×720，H.264 / yuv420p / 30fps / AAC。游戏素材保留完整画幅；AI 情境标注“AI 创意视觉 · 非实机画面”；截图标注“截图演示”。原素材声音默认不混入，使用系统旁白或静音轨。缓入缓出、原创 AI 情境图缓慢推近、视频分镜按连续偏移取段，循环使用在元数据中记录。

程序会保存来源、哈希、分镜和导出参数。制作失败显示错误；重启把未完成的编码标为中断，排队任务继续。独占数据目录可防止两个工作进程重复写同一队列。

## AI 脚本

本轮预设脚本由 Codex 编写，情境图由内置 imagegen 实际生成。当前工作室没有配置付费模型凭据，点击生成会明确加载预设，不伪装成即时模型调用。

若已有自有模型服务，**仅在服务进程的环境变量中**配置：

```text
PROMO_LLM_BASE_URL=https://your-provider.example/v1
PROMO_LLM_API_KEY=<本机环境变量中的密钥>
PROMO_LLM_MODEL=<服务商可用的模型名>
```

接口使用 OpenAI compatible `POST /chat/completions`。密钥不进入浏览器或投稿包。生成内容受 `campaigns.json` 中已验证文案和创意词组约束；不允许自动新增掉率、收益、在线容量、完整玩法等说法。模型失败或无效内容明确报错，不用不明来源文案冒充成功。本轮只验收模拟供应商响应 / 错误路径，**未验收真实付费模型服务**。

## 游戏素材录制

复用现有只读观战页，需要网页服务与对应地图上的真实玩家在线：

```powershell
node tools/promo-studio/record-spectator.mjs --url "http://127.0.0.1:3211/spectate?spectateMap=0&bevyRuntime=0&capture=1" --output ./tools/promo-studio/artifacts/captures --seconds 22 --fps 10 --require-live
```

需要 Chrome 和 Node 22+。可以用 `--chrome` / `--ffmpeg` / `--ffprobe` 指定实际程序位置。录像使用新建的专用无头浏览器，不截桌面，不发送玩家操作，不登录账号。采样为 8–12fps，按实际截图时间保持帧，再编码为 30fps；**不等于原生客户端原始 30fps 录像**。需要更流畅战斗素材时应导入 Windows 原生客户端录影。

录制保存 `evidence.json`、原始帧与录影。过期来源保留并标记过期，`--require-live` 会返回非零；不把静态旧帧当实时精彩片段。

## 验证

```powershell
python -m unittest discover -s tools/promo-studio -p test_engine.py -v
node tools/promo-studio/smoke-studio.mjs
```

浏览器验收需要工作室已启动且素材库至少有一张 AI 图和一段比奇录像；会上传自有图、创建英文旁白视频、验证播放和下载。默认截图 / 日志在忽略的 `artifacts/browser-qa` 下。可设置 `PROMO_STUDIO_UI_OUTPUT` 保存到自己的验收目录。

## 发布边界

目前交付本机成片与人工投稿素材包。**生成成功不等于已上传 / 已投稿**。B站应用稿件权限 ARC_BASE 仍待审核，未接平台发布；YouTube 也未获取上传 OAuth。第三方平台需要单独授权之后才能验收上传回执。

已有网关 `clipExport` 仅投递 JSON，没有完整视频及时间索引。本工具尚未接收该投递；未来接高光切片时需录像存档、事件时间索引、幂等接收和独立发布状态。不是从快照凭空生成真实战斗画面。

工作室只监听回环地址，变更请求要求同源 Origin；不要通过公网反向代理暴露。素材只复制进自己的目录，不能导入网络路径、符号链接或重解析点。
