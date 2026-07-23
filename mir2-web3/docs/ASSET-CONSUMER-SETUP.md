# Asset Consumer Setup

本项目把代码和多 GB Crystal 派生素材分开分发。新开发者不应该通过 Git LFS 或普通 Git 获取全量图集，也不应在首次运行时一次性把所有素材加载进内存。

## 三种消费模式

| 模式 | 本地全量目录 | 网络 | 推荐用途 |
| --- | --- | --- | --- |
| Starter | 不存在 | 可离线 | Gateway/Simulation、普通 UI、新手流程 |
| GitHub 私有开发素材包 | 存在 | 仅安装时需要，可离线开发 | 全图集开发、调试和离线视觉验收 |
| R2 CDN | 不需要 | 游戏时按需请求 | 维护者模板；当前尚未发布可用 URL |

运行时全量图集路径固定为：

```text
apps/web/public/generated/crystal-packs/full
```

该目录被 Git 忽略。不要强制添加它。

## Starter 模式

Starter 随仓库提供：

- 登录、选角和 HUD 素材。
- 新手地图所需的原始地图图块。
- 关键角色、NPC、Monster 和 Starter Entity Atlas。
- 预编译 Bevy WebGPU/WebGL2 Runtime。

启动：

```powershell
.\scripts\start-developer.ps1 -OpenBrowser
```

限制：

- 并非每个职业、装备、怪物和特效都能使用完整打包图集。
- `/generated/crystal-packs/full/index.json` 可能返回 404，运行时会回退到 Starter/prebuilt/live atlas 路径。
- 严格的全素材验收不能在 Starter 模式签字。

纯 Starter 调试 URL：

```text
http://127.0.0.1:3002/?crystalFullPack=0
```

## GitHub 私有开发素材包

### 权限与 manifest

私有包使用仓库的 GitHub Release，分卷默认约 1.5GB。安装器首先需要 `developer-assets.json`，其中包含：

- 私有仓库名和 Release tag。
- Full pack `contentHash`。
- 总归档大小和 SHA-256。
- 每个分卷的名称、大小和 SHA-256。
- 安装目标和库/页数量摘要。

`install-developer-assets.ps1` 默认读取仓库跟踪的 `config/developer-assets.json`。该文件是当前认可素材版本的唯一入口；升级素材时先发布新的私有 Release，再在同一提交中替换清单。

当前认可版本是 `developer-assets-f71b89aa3850`，内容哈希为 `f71b89aa38504c6c127b937043d4af6ecd26d9dd1a2b9ed3b91100e6a1f0052e`。它包含 1,440 个 library shards 和 4,446 张唯一 PNG pages；总归档是确定性 USTAR，并拆成 7 个 Release 分卷。

该 Release 只安装转换后的完整视觉图集，不包含原始 `.Lib`、原生客户端、可执行文件或完整声音库。仓库只跟踪 4 个 Starter WAV；其余 316 个声音文件必须来自项目所有者单独提供且允许使用的本地数据源，或未来经过验收的受控音频发布。不要因为 `crystal-present-sounds.generated.json` 列出了 320 个发布态文件，就推定干净克隆已经拥有这些音频字节。

维护者在具有合法本地声音源时可执行严格闭包检查：

```powershell
$env:CRYSTAL_CLIENT_ROOT = "<authorized-client-root>"
npm --prefix apps/web run export:crystal-sounds
npm --prefix apps/web run generate:present-sounds
$env:MIR2_REQUIRE_LOCAL_SOUND_CLOSURE = "1"
npm --prefix apps/web run preflight:asset-release
```

### 在线安装

先确认仓库所有者已授予私有仓库和 Release 的读取权限：

```powershell
gh auth login
gh auth status
gh auth setup-git
gh release view developer-assets-f71b89aa3850 --repo Zombieliu/mir2
.\scripts\install-developer-assets.ps1 -Download
```

安装器会从 manifest 指定的 Release 下载缺失分卷、逐一校验大小和 SHA-256、重组总归档、再次校验总哈希，然后解压。下载中断后直接重新运行；脚本会保留有效分片，并删除后重下不完整或损坏的缓存分片。默认会删除重组出来的临时 tar；使用 `-KeepArchive` 可保留。

磁盘预算：

- 7 个缓存分片合计约 9.08 GiB。
- 安装后的完整图集约 9.08 GiB。
- 安装时重组 tar 和 staging 各可能再占约 9.08 GiB。
- 新装至少准备 40 GiB 空闲；连同 Git、Node、Rust 和构建缓存，建议为完整开发环境预留 50 GiB 以上。

缓存可改到其他磁盘：

```powershell
.\scripts\install-developer-assets.ps1 `
  -Download `
  -CacheDirectory F:\mir2-asset-cache\developer-assets-f71b89aa3850
```

默认缓存位于 `.mir2-data/developer-assets/developer-assets-f71b89aa3850`。确认安装和 `verify-developer-setup.ps1` 均通过后，可以删除这一精确 tag 的缓存目录来释放约 9.08 GiB；以后重装需要重新下载。不要对不确定路径执行递归删除。

### 离线安装

把 `developer-assets.json` 和所有 `.partNNN` 放在同一目录：

```powershell
.\scripts\install-developer-assets.ps1 `
  -ManifestPath D:\mir2-assets\developer-assets.json `
  -PartsDirectory D:\mir2-assets
```

已经安装不同 `contentHash` 时，脚本会拒绝覆盖。确认确实要替换后再传 `-Force`。

### 验证安装

```powershell
$Index = Get-Content `
  .\apps\web\public\generated\crystal-packs\full\index.json `
  -Raw | ConvertFrom-Json
$Index.contentHash

.\scripts\verify-developer-setup.ps1 -SkipBuild
```

安装器会逐页校验 SHA-256；后续 `verify-developer-setup.ps1` 会执行较快的本地闭包检查，确认固定 `contentHash`、所有 shard/page 引用和零孤儿文件，而不会在每次日常验证时重复读取整套 9.08 GiB 页面内容。

## R2 CDN

**状态：当前 R2 完整素材尚未发布。以下为维护者模板，不是新开发者可直接使用的地址。**

未来 R2 适合不需要把约 9.08 GiB 全量包永久放在本机的开发者。素材必须放在不可变版本目录，例如：

```text
https://assets.example.com/mir2/v/<version>
```

消费：

```powershell
$AssetBaseUrl = "https://assets.example.com/mir2/v/<version>"
.\scripts\verify-developer-setup.ps1 -AssetBaseUrl $AssetBaseUrl -SkipBuild
.\scripts\start-developer.ps1 -AssetBaseUrl $AssetBaseUrl -OpenBrowser
```

单独检查完整索引只适合快速诊断：

```powershell
Invoke-WebRequest -UseBasicParsing -Method Head `
  "$AssetBaseUrl/generated/crystal-packs/full/index.json"
```

正式验收必须运行上面的 `verify-developer-setup.ps1 -AssetBaseUrl ...`。该命令读取远程 `remote-asset-release.json`，校验精确对象集合，并发探测全部 5,887 个 full-pack 对象；只看到 `index.json` 返回 200 不能代表全量素材已经发布完整。

浏览器只按当前场景和当前实体请求需要的 shards/pages；不要增加“启动时下载整个 full pack”的逻辑。Service Worker 使用同源请求作为缓存键，并从配置的 R2 URL 回源。

### `.env.local` 方式

脚本参数是首选。需要固定配置时，可在 `apps/web/.env.local` 写：

```dotenv
NEXT_PUBLIC_MIR2_GATEWAY_WS_URL=ws://127.0.0.1:7110/ws
NEXT_PUBLIC_MIR2_ASSET_BASE_URL=https://assets.example.com/mir2/v/<version>
```

`.env.local` 被 Git 忽略，不要提交。开发环境设置素材 URL 后会自动启用 Asset Service Worker；可用 `?assetCache=0` 临时禁用。

## 维护者：生成 GitHub 私有包

前置条件：本机已经具有经过合法授权的 Crystal Client 数据，并已构建 full pack。

```powershell
cd apps\web
npm run assets:full-pack:build
npm run assets:full-pack:verify
cd ..\..

.\scripts\package-developer-assets.ps1
```

默认输出：

```text
dist/developer-assets/developer-assets-<content-hash-prefix>/
```

读取 tag 并创建私有 Release：

```powershell
$BundleDir = Get-ChildItem .\dist\developer-assets -Directory | `
  Sort-Object LastWriteTime -Descending | Select-Object -First 1
$Manifest = Get-Content (Join-Path $BundleDir.FullName "developer-assets.json") `
  -Raw | ConvertFrom-Json
$ReleaseAssets = @(
  Join-Path $BundleDir.FullName "developer-assets.json"
)
$ReleaseAssets += Get-ChildItem -LiteralPath $BundleDir.FullName `
  -Filter "*.part*" -File | Sort-Object Name |
  Select-Object -ExpandProperty FullName

gh release create $Manifest.releaseTag `
  --repo $Manifest.repository `
  --title $Manifest.releaseTag `
  --notes "Verified Crystal developer asset bundle: $($Manifest.contentHash)" `
  @ReleaseAssets
```

发布后，从另一缓存目录实际执行一次 `install-developer-assets.ps1 -Download`，避免只验证上传端。

## 维护者：发布 R2 完整版本

先验证 full pack，再生成引用本地文件的紧凑发布清单：

```powershell
cd apps\web
npm run assets:full-pack:verify

$env:MIR2_ASSET_VERSION = "<immutable-version>"
$env:NEXT_PUBLIC_MIR2_ASSET_BASE_URL = `
  "https://assets.example.com/mir2/v/$env:MIR2_ASSET_VERSION"

npm run assets:full-release:build
npm run assets:r2:dry-run
```

确认 dry-run 的文件数、总字节数、object prefix 和完整图集统计后，设置 R2 凭据：

```powershell
$env:MIR2_R2_BUCKET = "<bucket>"
$env:MIR2_R2_UPLOAD_DRIVER = "r2-s3"
$env:MIR2_R2_ACCESS_KEY_ID = "<access-key>"
$env:MIR2_R2_SECRET_ACCESS_KEY = "<secret-key>"
npm run assets:r2:upload
```

凭据只能进入本机安全存储或 CI Secret，不能写入 `.env.example`、README、发布清单或 Git。

发布完成后：

```powershell
cd ..\..
.\scripts\verify-developer-setup.ps1 `
  -AssetBaseUrl $env:NEXT_PUBLIC_MIR2_ASSET_BASE_URL
```

## 重建 full pack 的原始数据

仓库和 Crystal 子模块都不包含 `Crystal/Build/Client/Debug/Data` 的完整二进制素材。只有素材维护者需要原始客户端数据。默认 Windows 来源是：

```text
<repo>\Crystal\Build\Client\Debug\Data
```

部分导入工具也支持 `CRYSTAL_CLIENT_ROOT` 或 `CRYSTAL_CLIENT_DATA_DIR`。普通消费者不应为了运行游戏而自行寻找 `.Lib` 文件；优先使用私有开发包或 R2。

## 缓存与低端设备

- Full pack 的“全量”指发布覆盖率，不代表一次性下载或解码全部内容。
- 运行时按场景请求 Atlas shard/page，并受浏览器缓存预算控制。
- R2 文件使用内容版本目录和 immutable cache；新版本发布到新目录，不覆盖旧对象。
- 低端设备验收应同时测试 WebGL2、冷缓存、热缓存和存储配额不足。
- 可用 `?cacheDebug=1` 查看缓存状态，用 `window.__mir2AssetCacheReset()` 清理 Mir2 CacheStorage。

## 法律与访问控制

完整权利清单见 `docs/LEGAL-AND-ASSET-RIGHTS.md`。仓库或 Release 的技术访问权不等于公开、商业或再分发许可。

Crystal/Wemade 素材只能在获得相应权利或授权的范围内分发。授权未明确时：

- GitHub Release 保持私有，只授予项目开发者访问权。
- R2 使用受控域名或鉴权层，不公开桶列表和写入凭据。
- 不将原始客户端安装包、`.Lib` 或完整素材页提交到公开 Git。
