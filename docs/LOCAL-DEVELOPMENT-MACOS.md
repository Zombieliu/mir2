# macOS 本地开发

macOS 开发者运行 Starter 只需要 Git、Docker Desktop 和至少 50 GiB 可用磁盘；完整私有素材模式还需要宿主机 GitHub CLI。Node、npm、Rust、C/C++ 构建工具与 WASM target 都由锁定的 Linux 开发镜像提供，不需要在每台 Mac 上分别安装。

支持：

- Apple Silicon：`linux/arm64`
- Intel Mac：`linux/amd64`
- Player Web、Gateway、Simulation、WebGPU/WebGL2 浏览器运行时

不支持在 macOS 本机运行：

- Crystal 原生 `Client.exe`
- Windows GDI/DirectX 原版客户端采集

原版对照应使用 Windows 验收机、共享的原生截图/录像/trace，或远程 Windows 桌面。Web 产品开发本身不依赖 Windows 客户端。

## 从空目录启动

先启动 Docker Desktop，然后执行：

```bash
git clone --filter=blob:none --recurse-submodules \
  https://github.com/Zombieliu/mir2.git
cd mir2/mir2-web3
./scripts/dev.sh up --build --open
```

首次构建会下载锁定镜像和依赖。后续运行：

```bash
./scripts/dev.sh up --open
./scripts/dev.sh status
./scripts/dev.sh logs
./scripts/dev.sh down
```

Player Web 为 `http://127.0.0.1:3002/`，Gateway 健康检查为 `http://127.0.0.1:7110/health`。

## 安装完整素材

完整素材不是 Git 仓库的一部分。第一次使用时，下面这一条命令会通过宿主机官方 `gh` 完成 GitHub 设备授权，再下载、校验、安装和启动；token 不会写入仓库：

```bash
./scripts/dev.sh up --full-assets --open
```

启动器先核验远端 witness、release lock 中的精确 digest 和镜像 revision，使用临时 Docker 配置拉取，再经标准输入把下载凭据交给隔离 fetcher；凭据不进入容器配置，本地构建镜像、默认 Dev Container、Gateway 与 Web 都无法读取。安装器读取 `config/developer-assets.json`，逐卷校验大小与 SHA-256，并以 staging + 原子替换方式安装。完整视觉包约 9.08 GiB，安装过程建议预留至少 40 GiB，完整开发环境建议预留 50 GiB。

## VS Code Dev Container

仓库包含 `.devcontainer/devcontainer.json`。安装 VS Code 的 Dev Containers 扩展后打开 `mir2-web3`，选择 **Reopen in Container** 即可使用相同锁定环境。

## 空目录验收

```bash
./scripts/accept-clean-room.sh
```

该脚本在系统临时目录重新 clone、构建、启动 Gateway 与 Player Web、检查 HTTP 健康状态，再清理临时目录。需要连同完整素材验收时使用：

```bash
./scripts/accept-clean-room.sh --full-assets
```

Full 模式需要 GitHub Release 权限和足够磁盘，不应在每个 PR 上重复运行。
