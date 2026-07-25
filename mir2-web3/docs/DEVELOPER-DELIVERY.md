# 开发环境与素材交付

## 目标

任何获授权的开发者都应从空目录得到同一个：

- Git 提交与 Crystal 子模块提交
- Node、npm、Rust、系统构建依赖和 WASM target
- Starter 素材，或指定内容哈希的完整素材
- Gateway、Player Web 端口与运行参数
- 可自动判定成功或失败的验收结果

唯一版本入口是 `config/developer-release.json`。它同时锁定 Crystal commit、工具链、容器基础镜像和 `config/developer-assets.json` 的素材 tag/content hash。

## 分层职责

| 系统 | 职责 | 是否保存大素材 |
| --- | --- | --- |
| GitHub | 代码、PR、Issue、CI、Crystal gitlink、Release manifest | 否 |
| GitHub Release | 当前受控的开发者完整素材分卷 | 是，过渡方案 |
| R2/对象存储 | 不可变版本素材、按需下载、CDN 缓存 | 是，推荐长期方案 |
| GHCR | amd64/arm64 锁定开发镜像 | 仅工具链 |
| 第二台服务器 | 共享 Gateway、Web 验收站、日志与远程联调 | 不应成为唯一素材源 |
| 本机 Docker/Dev Container | Windows、Intel Mac、Apple Silicon 一致开发环境 | 仅本地缓存 |

Perforce 只在未来多人频繁修改不可合并的原始美术源文件时有价值。它不能替代 GitHub 的代码协作、容器化工具链、对象存储和共享服务，因此当前不迁移整个仓库。

## 新开发者命令

Windows：

```powershell
git clone --filter=blob:none --recurse-submodules https://github.com/Zombieliu/mir2.git
cd mir2\mir2-web3
.\scripts\dev.cmd up -Build -OpenBrowser
```

macOS/Linux：

```bash
git clone --filter=blob:none --recurse-submodules https://github.com/Zombieliu/mir2.git
cd mir2/mir2-web3
./scripts/dev.sh up --build --open
```

完整素材一条命令启动；首次运行会在容器内要求 GitHub 设备授权，凭据保存在本机 Docker 命名卷：

```text
Windows: .\scripts\dev.cmd up -FullAssets -OpenBrowser
macOS:   ./scripts/dev.sh up --full-assets --open
```

## 两级验收

Starter clean-room gate：

- 从空目录 clone
- 校验版本锁与 Crystal gitlink
- 构建锁定开发镜像
- 启动 Gateway 和 Player Web
- 等待两个健康检查
- Windows、macOS、Ubuntu CI 检查脚本和版本锁

Windows、macOS、Ubuntu 的 hosted CI 都会实际执行各自的一键入口
`doctor` 合约，而不只做语法解析；Ubuntu 额外运行真实 Docker
Gateway/Web。发布工作流构建 `linux/amd64` 与 `linux/arm64` 镜像。这个组合
验证 Mac 所需的宿主路径和 Apple Silicon 容器架构，但不冒充 Crystal
Windows 原生客户端或 Mac 物理机的人工画面/手感验收。

Full asset gate：

- 校验私有 Release 或受控对象存储权限
- 下载全部 7 个分卷
- 校验分卷与 9.08 GiB USTAR 总归档
- 安全解包并校验 full-pack closure/content hash
- 运行 Player Web 并进行素材网络请求与场景验收

Starter 通过不代表完整视觉素材通过。Full gate 不应因下载成本在每个普通 PR 上执行，而应在素材版本发布、交接和 Candidate 验收时执行。

## 第二台服务器

推荐准备一台 Linux 主机，安装 Git、`jq`、Docker Engine 和 Docker Compose `2.24.4+`，配置 DNS A/AAAA 记录并开放 80/443。先用固定 Caddy 镜像生成 Argon2id 或 bcrypt hash，并把用户名、hash 和只用于部署后探活的明文密码放入服务器秘密环境变量，不要写进仓库或命令行。服务器 clone 到固定目录后：

```bash
export MIR2_ACCEPTANCE_BASIC_AUTH_USER='mir2-qa'
export MIR2_ACCEPTANCE_BASIC_AUTH_HASH='$argon2id$...'
export MIR2_ACCEPTANCE_BASIC_AUTH_PASSWORD='<deployment-health-check-password>'

cd /srv/mir2/mir2-web3
git fetch --all --prune
git checkout <accepted-commit>
git submodule update --init --recursive
./scripts/deploy-acceptance.sh \
  --domain play-dev.example.com \
  --asset-base-url https://assets.example.com/mir2/v/<content-version> \
  --build
```

部署使用 Caddy 自动签发 HTTPS 证书并对全部 HTTP/WSS 路由启用 Basic Auth；`/ws`、`/health` 转发到 Gateway，其余请求转发到 production Player Web。Gateway/Web 外露端口被 acceptance overlay 强制绑定 loopback，每次部署都会强制重建容器，并同时核对容器标签、Gateway `/health` 和 Player Web `/version` 的真实运行 revision。探活密码只写入临时 `0600` curl 配置，既不传给容器，也不出现在 curl argv。

服务器部署前必须保证：

- DNS 已指向服务器
- 80/443 可从公网访问
- Basic Auth 密码和强 hash 已放入服务器秘密环境
- 素材域名和分发权利已经确认
- 仓库是明确提交且 tracked/untracked worktree 都干净
- 不把 GitHub token、R2 Secret、账户数据提交进 Git

第二台服务器用于共享验收，不取代每台开发机的 clean-room gate，也不应成为没有备份和版本号的唯一素材存储。

## 当前限制

- Crystal 原生客户端只能在 Windows 上运行。
- 当前完整素材不包含尚未授权/发布的完整声音库。
- `config/developer-release.json` 的 `assets.remoteBaseUrl` 仍为空，表示正式 R2 URL 尚未落地。
- 发布 GHCR 镜像后，应把 `container.publishedImage` 固定到镜像 digest；在此之前脚本会从 digest-pinned 基础镜像本地构建。
