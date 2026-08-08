# Gate 23：跨平台 Dubhe Home Agent

Gate 23 提供家庭节点的跨平台后台管理核心、系统密钥库、签名更新、资源降载和
本地管理页。它建立在 Gate 22 的出站 QUIC/mTLS 数据面上。

## 当前交付

- `home_agent`：出站 QUIC/mTLS 隧道；Ed25519 节点种子和 mTLS PKCS#8 私钥均可
  从操作系统密钥库读取；
- `home_agent_supervisor`：资源策略、休眠检测、手动/自动 drain、退出前 drain、
  loopback-only 管理页与 Bearer 保护的变更 API；正式安装后由它启动并监控
  `zone_host` 与 `home_agent`，任一子进程异常退出都会先 fail closed，再让系统
  服务管理器重启整组进程；
- `home_agent_release`：离线 Ed25519 签名的版本清单；
- `home_agent_launcher`：固定的小型启动器；只启动当前已验签版本，启动健康窗口
  失败时原子回滚上一版本并隔离失败版本；
- 自动更新：HTTPS-only、禁止 redirect/嵌入凭据、target/channel/有效期/最低版本、
  下载上限、长度与 SHA-256 校验；压缩包只能包含三个平铺的受信二进制，拒绝
  路径穿越、符号链接、额外文件和解压炸弹；更新前 drain，退出码 `75` 交给
  Launcher 切换版本；
- macOS LaunchAgent、Linux systemd user unit、Windows per-user Scheduled Task 安装器；
- 三平台 release 构建矩阵和可重复本机验收脚本。

管理页默认只监听 `http://127.0.0.1:17990/`。GET 状态不包含 IP、用户名、路径或
密钥；所有 drain/resume 变更必须携带管理 Bearer token。

## 本机验收

macOS/Linux 运行：

```bash
./infra/gate23/verify-gate23.sh
```

脚本会真实写入一个随机测试身份到系统密钥库、重新读取公钥、验证并暂存签名更新、
启动真实 Zone Host 与 Supervisor、证明未认证 drain 返回 401、认证 drain/resume
能改变 Zone Host readiness，最后删除测试密钥和进程。

成功标志：

```text
GATE23_LOCAL_ACCEPTED
```

证据写入 `docs/generated/home-node/gate23-local-acceptance.json`。

## 安装包

```bash
./infra/gate23/package-home-agent.sh \
  aarch64-apple-darwin \
  target/home-agent-packages
```

支持目标：

- `aarch64-apple-darwin` / `x86_64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

安装后先完成平台签名的 enrollment，取得 mTLS 证书、容量证书、签名配置和
placement，再启动后台服务。安装器不会把节点种子写入 env、配置文件、日志或
容器镜像。

## 资源策略

| 变量 | 默认 | 行为 |
| --- | --- | --- |
| `MIR2_HOME_MAX_CPU_PERCENT` | `75` | 连续超限后停止接新 Session |
| `MIR2_HOME_MIN_AVAILABLE_MEMORY_MIB` | `2048` | 低于阈值进入 drain |
| `MIR2_HOME_OVERLOAD_SAMPLES` | `3` | 过滤瞬时尖峰 |
| `MIR2_HOME_RECOVERY_SAMPLES` | `12` | 持续恢复后才 resume |
| `MIR2_HOME_SAMPLE_INTERVAL_MS` | `5000` | 资源采样周期 |

采样间隔出现超过三倍的单调时钟跳跃时，Supervisor 把它视为休眠/唤醒，立即
fail closed。退出或升级前先设置 Zone Host draining，等待现有 Session 归零；
超过维护窗口时由 Regional standby 接管。

## 更新信任链

1. CI 构建每个平台的不可变制品和 SHA-256；
2. 离线/HSM release issuer 对 canonical JSON manifest 签名；
3. Agent 固定 issuer 公钥，只接受 HTTPS、正确 target/channel、有效期内且更高版本；
4. 下载后同时校验长度和 SHA-256，在临时目录安全解包，逐文件 fsync 后原子 rename；
5. Supervisor drain 后退出，由固定 Launcher 激活 staged version；
6. 启动健康检查失败则回退 previous version，并永久隔离 failed version，直到新的
   rollout/version；
7. OS 代码签名和公证是额外的外部发布门，不能被应用层签名替代。

自动检查需要 enrollment 配置：

```text
MIR2_HOME_UPDATE_MANIFEST_URL=https://updates.example/home-agent/stable.json
MIR2_HOME_UPDATE_ISSUER_PUBLIC_KEY=<offline issuer public key>
MIR2_HOME_UPDATE_TARGET=<Rust target triple>
MIR2_HOME_UPDATE_ROOT=<install root>/update
MIR2_HOME_UPDATE_CHECK_INTERVAL_SECONDS=3600
```

Manifest 和 artifact URL 都必须为 HTTPS，且不允许凭据或重定向。Launcher 本身只随
经过平台签名的安装包升级，普通应用层 release 只包含 `home_agent`、
`home_agent_supervisor` 和 `zone_host`。

## 尚需外部证据

仓库可以验证三平台源码可构建和应用层签名链，但 Apple Developer ID/notarization、
Microsoft Authenticode、正式 Linux 包仓签名需要真实组织证书和外部账号。没有这些
证据时，不得宣称“OS 商店级签名已完成”。Gate 25 还需要三家真实运营商家庭网络。
