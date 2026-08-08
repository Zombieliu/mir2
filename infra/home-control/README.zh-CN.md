# Dubhe Home Control UCloud Beta 部署

本目录用于把 Enrollment Authority、QUIC Relay 和签名遥测 Collector 部署到
UCloud 轻量应用云主机，并把官方 Mir2 Gateway 的 Zone RPC 转发到家庭节点。

当前 Beta 验证拓扑：

```text
玩家 -> Mir2 Gateway
              |
              | Zone RPC + 内部令牌
              v
       Relay 127.0.0.1:9444
              |
              | QUIC + 双向 mTLS，公网 UDP/9443
              v
       家庭 Home Agent -> Zone Host 127.0.0.1:7020

Dubhe Node -> HTTPS 签名遥测 -> Collector
Dubhe Node -> HTTPS Enrollment -> Enrollment Authority
```

家庭路由器不需要端口映射。家庭节点只发起出站 QUIC 连接，公网看不到家庭
Zone Host 端口；Relay 只接受 Authority 签发的客户端证书和已登记节点。

## 前置条件

- Ubuntu 24.04；
- `mir2` 系统用户和组；
- Rust 1.89.0；
- 已最终确认的 Sui testnet 节点注册 JSON；
- Release 构建产物：
  `home_relay`、`home_enrollment_service`、`home_telemetry_collector`、
  `node_identity`；
- 云防火墙和主机 UFW 均放行 `UDP/9443`；
- `TCP/443` 已由 Caddy 或同类反向代理提供 HTTPS。

只放行主机 UFW 不够。UCloud 轻量主机的“外网防火墙”还必须显式增加：

```text
接受 | UDP: 9443 | 0.0.0.0/0
```

Relay 应用层仍强制双向 mTLS、节点登记、容量证书和 placement 校验。

## 安装

```bash
cd /path/to/mir2-web3
cargo +1.89.0 build --release --locked -p mir2-gateway \
  --bin home_relay \
  --bin home_enrollment_service \
  --bin home_telemetry_collector \
  --bin node_identity

bash infra/home-control/install-ucloud-beta.sh \
  "$PWD" \
  docs/generated/gate13/testnet/dubhe-desktop-registration.json
```

安装器会：

- 生成生产随机的 Enrollment、Control、Relay Ed25519 身份；
- 生成独立 Relay CA 和服务端证书；
- 生成内部 Gateway Relay 令牌和遥测运维令牌；
- 以受限权限安装三个 systemd 服务；
- 把 placement、admission 和密钥保存在
  `/var/lib/mir2/home-control`；
- 使用 `/opt/mir2/home-control/current` 原子切换 Release。

私钥、令牌和证书材料不得提交到 Git。

## HTTPS 路由

Caddy 需要把以下路径转发到仅监听回环地址的服务：

```caddyfile
handle /v1/challenges* {
    reverse_proxy 127.0.0.1:18080
}
handle /v1/enrollments* {
    reverse_proxy 127.0.0.1:18080
}
handle /v1/capacity/* {
    reverse_proxy 127.0.0.1:18080
}
handle_path /home/enrollment/* {
    reverse_proxy 127.0.0.1:18080
}
handle_path /home/telemetry/* {
    reverse_proxy 127.0.0.1:18081
}
```

当前香港 Beta 使用 Cloudflare 托管的固定域名
`relay-hk.obelisk.build`，DNS 为灰云 `A` 记录并直指 Relay 公网 IP。这里不能
开启普通橙云代理：Home Agent 使用公网 `UDP/9443` QUIC，而普通 Cloudflare
HTTP 代理不会转发该 UDP 端口。

公网 HTTPS 入口：

```text
https://relay-hk.obelisk.build/v1/challenges
https://relay-hk.obelisk.build/v1/enrollments
https://relay-hk.obelisk.build/v1/capacity/*
https://relay-hk.obelisk.build/home/enrollment/*
https://relay-hk.obelisk.build/home/telemetry/*
```

Relay 服务端证书必须包含 `DNS:relay-hk.obelisk.build` SAN。安装器同时保留
旧 `sslip.io` SAN 作为迁移回退；新的 Enrollment Bundle 只下发正式域名。

远程管理台使用 `GET /home/telemetry/v1/operator` 读取全部已准入家庭节点、
分配 Zone、认证容量与最近一次签名遥测。该接口必须携带
`Authorization: Bearer <telemetry operator token>`，令牌只保存在 UCloud
密钥文件与 Vercel 加密环境变量中；浏览器不会收到该令牌。单节点路径
`/home/telemetry/v1/operator/{node_id}` 保留用于定点巡检与撤销后的排障。

## 接入官方 Gateway

Gateway 二进制必须包含 `MIR2_ZONE_HOST_ADDR` 远程 Zone RPC 支持。旧二进制
即使加入环境变量也会继续在 Gateway 进程内运行世界模拟，因此验收时必须同时
检查 Relay 私有入口流量和家庭 Zone Host RPC 计数。

在 `/etc/mir2/gateway.env` 增加：

```dotenv
MIR2_ZONE_HOST_ADDR=127.0.0.1:9444
MIR2_ZONE_HOST_TOKEN=<读取 /var/lib/mir2/home-control/secrets/gateway-relay.token>
```

变更前备份配置和 `/opt/mir2/gateway/current` 指向的 Release。升级 Gateway
后重启：

```bash
sudo systemctl restart mir2-gateway
sudo systemctl is-active mir2-gateway
```

## 验收

服务端：

```bash
systemctl is-active \
  mir2-gateway \
  dubhe-home-enrollment \
  dubhe-home-relay \
  dubhe-home-telemetry

curl -fsS http://127.0.0.1:18080/healthz
curl -fsS http://127.0.0.1:18081/healthz
sudo ss -lunp | grep ':9443'
sudo ss -lntp | grep ':9444'
```

真实玩家探针：

```bash
MIR2_HOME_PLAYER_GATEWAY_ADDR=127.0.0.1:7000 \
MIR2_HOME_PLAYER_PROBE_OUT=/tmp/home-player-production.json \
cargo +1.89.0 run --release -p mir2-gateway --bin home_player_probe
```

通过条件：

- 输出 `HOME_PLAYER_PROBE_PASS`；
- `connect`、`login`、`startGame`、`keepAlive` 四步均成功；
- `tcpdump -ni lo tcp port 9444` 能看到 Gateway 到 Relay 的流量；
- 家庭 Zone Host 的 `rpcRequestsTotal` 增长且 `rpcErrorsTotal` 为零；
- Dubhe Node 显示 `QUIC + mTLS 隧道已连接`；
- Collector 接受连续递增的签名遥测序号。

只看到进程在线、HTTPS 健康或玩家探针通过，不能单独证明玩家世界逻辑确实在
家庭节点执行。

## 本次公网 Beta 验证

2026-07-27 在 UCloud 香港轻量主机和 macOS 家庭节点完成：

- Cloudflare `relay-hk.obelisk.build` 灰云 DNS 和 Let's Encrypt HTTPS；
- 容量认证：128 Sessions、8 Zones、p95 1ms、成功率 100%；
- QUIC 双向 mTLS 隧道建立；
- 签名遥测被 Collector 接受；
- 正式 Gateway 经 Relay 执行 Mir2 玩家生命周期；
- Relay 重启后自动恢复，并再次通过玩家探针；
- 家庭 Zone Host 最终累计 82 次 RPC、0 RPC 错误。

这些数字是当前机器和当前 Beta 工作负载的验收结果，不是商业服容量承诺。
