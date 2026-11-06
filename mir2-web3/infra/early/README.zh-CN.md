# Early：单机 4C / 8GB、100 CCU

这是 Mir2 / Dubhe Node 的低成本首发形态。它保留 Gateway、权威 Zone、
PostgreSQL 经济账本、Redis Session 缓存和热点地图分线，但把它们放在同一台
机器上。它适用于 `0–100 CCU`，不冒充 Regional 高可用集群。

## 容量边界

- 机器：至少 `4 CPU / 7.5GiB` Docker 可用内存、`100GiB` NVMe；
- 账号同时在线上限：`100`；
- 700 多张地图仍可进入，但只按玩家访问按需激活，建议最多同时活跃 32 张；
- 地图 `0` 以每线 50 人、硬上限 64 人自动扩成最多两线；
- CPU 持续 `>65%`、内存 `>70%`、命令 p95 `>150ms` 或 CCU 持续 `>=80`
  时触发扩容，不等到 100 人满载才处理。

## 启动

先设置三个秘密；recovery key 必须来自密钥管理系统，不能使用开发默认值：

```bash
export MIR2_EARLY_POSTGRES_PASSWORD='<random-password>'
export MIR2_EARLY_ZONE_TOKEN='<at-least-32-random-bytes>'
export MIR2_GATEWAY_SAVE_RECOVERY_MAC_KEY='<64-hex-secret-from-secret-manager>'
./infra/early/preflight.sh
docker compose -f infra/early/docker-compose.yml up -d --build
docker compose -f infra/early/docker-compose.yml ps
```

Gateway 对外端口默认为 TCP `7000`、HTTP/WebSocket `7010`。公网只开放 Gateway；
PostgreSQL、Redis 和 Zone RPC 不映射宿主端口。

## 备份与恢复演练

```bash
MIR2_EARLY_BACKUP_DIR=/mnt/off-host-backups ./infra/early/backup.sh
./infra/early/restore.sh --confirm /absolute/path/to/mir2-YYYYMMDDTHHMMSSZ.dump
```

备份必须复制到另一台机器或对象存储。恢复脚本先恢复到 `mir2_restore` 验证库，
不会直接覆盖在线 `mir2` 数据库；完成应用检查后再走人工切换。

## 扩容路径

第一次扩容增加一台 `4C / 8GB` Zone Host，把地图执行移出首服；第二次扩容把
PostgreSQL 迁到独立节点并增加 Gateway。达到 300–500 CCU 后切换到 Gate 19/20
HA 形态，最终 Regional 使用 4 active + 4 standby Zone Host。

机房必须靠近玩家。香港节点适合东亚和东南亚；如果首发用户主要在巴西，应把
同一套 Compose 部署到圣保罗区域，不能用增加 CPU 掩盖跨洲网络延迟。

## Gateway save-recovery

实际运行 Gateway 的 service 必须显式声明
`com.obelisk.mir2.role=gateway`。该标签是验证器的权威 Gateway 清单，不依赖 service
名称、镜像名称或监听地址。build target、Gateway image token 和 TCP/Web 环境变量
只作为兼容守卫，用来拒绝疑似 Gateway 缺少或错写角色标签的配置。

Compose 使用 MIR2_GATEWAY_SAVE_RECOVERY_MAC_KEY 的必填插值，只证明变量存在且
非空；它不会判断 malformed、placeholder 或重复弱值。Gateway 进程负责校验恰好
64 个十六进制字符和最低多样性。本静态检查不执行 Rust 强度门，独立命令是：

    cargo +1.95.0 test --manifest-path apps/gateway/Cargo.toml --bin mir2-gateway       --jobs 1 tests::empty_malformed_and_weak_recovery_keys_are_rejected       -- --exact --test-threads=1

逻辑卷 gateway-save-recovery 的固定物理名是
mir2-early-gateway-save-recovery-v1，挂载到实例 early-gateway-1 的绝对 recovery
目录。普通重启及修改 COMPOSE_PROJECT_NAME/-p 都会复用同一 sidecar。现有
backup.sh 只备份 PostgreSQL；发布备份必须在同一恢复点联合保存 recovery key 与
该 physical volume。

固定物理名也意味着同一 Docker daemon 只能运行一套 Early 拓扑；不同 -p 启动
第二套会有意连接同一 sidecar。独立集群必须使用不同主机/daemon，或经过重新审计
后显式改名。不要把展开后的 docker compose config 输出到日志；静态检查器捕获
渲染结果且只报告 wiring 结论。
