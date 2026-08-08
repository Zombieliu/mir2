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

先设置两个秘密，不能使用 Compose 中的开发默认值：

```bash
export MIR2_EARLY_POSTGRES_PASSWORD='<random-password>'
export MIR2_EARLY_ZONE_TOKEN='<at-least-32-random-bytes>'
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
