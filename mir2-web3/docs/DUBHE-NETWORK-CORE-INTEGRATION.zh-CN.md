# Dubhe Network Core 与 Mir2 适配边界

## 结论

Dubhe 的通用分布式节点能力以
[`0xobelisk/dubhe-chain-poc`](https://github.com/0xobelisk/dubhe-chain-poc)
为唯一上游。Mir2 是第一个接入它的游戏，不是这些通用协议的所有者。

本仓库暂时在 `vendor/dubhe-network-core` 固定一份上游源码快照。这样两个私有
GitHub 仓库之间的 CI 不需要共享个人令牌，也能复现构建。快照来源、版本和许可
记录在 `vendor/dubhe-network-core/UPSTREAM.toml`；它不是允许双向修改的分叉。

## 代码边界

| 所属 | 负责内容 |
| --- | --- |
| Dubhe Network Core | Ed25519 节点身份、Sui 注册与容量证书、Enrollment、签名反向隧道协议、沙箱声明、Agent 更新策略、隐私遥测、工作回执和奖励结算 |
| Mir2 适配层 | Crystal 协议、玩家登录、地图与 Zone 模拟、Gateway 路由、QUIC 进程与 Mir2 Zone RPC 编解码、Mir2 桌面端 sidecar |
| 基础设施 | 官方 Relay、Commonware 控制面、遥测 API 与持久化；使用 Core 协议，通过 Mir2 adapter 承载具体游戏流量 |

因此，一次玩家请求的路径是：

```text
Mir2 客户端
  -> Mir2 Gateway（认证、Session）
  -> Dubhe/Commonware placement（选择 Region、Relay、Node）
  -> Dubhe 签名隧道（身份、授权、重放保护）
  -> Mir2 transport adapter（Zone RPC）
  -> Mir2 Zone Host（地图权威模拟）
```

Core 不认识地图编号、角色背包或 Crystal 数据包；Mir2 不自行定义第二套节点身份、
证书、遥测和奖励协议。

## 当前接入方式

- `apps/gateway` 直接依赖 `vendor/dubhe-network-core`。
- Gateway 原有的通用模块名继续作为兼容门面，并从 Core 重新导出类型，避免现有
  Mir2 二进制和运维脚本一次性改名。
- `apps/dubhe-node-desktop` 直接依赖 Core；Mir2 专用 sidecar 仍由产品仓库打包。
- Wire schema 暂时保留已经上线的 `obelisk.*.v1` 名称，避免现有节点失去兼容性；
  多游戏隔离使用 `game_id`，Mir2 的值为 `mir2`。

## 上游同步规则

1. 通用能力只能先在 `dubhe-chain-poc/crates/network-core` 修改、测试和合并。
2. 将合并后的 `crates/network-core` 完整同步到本仓库
   `vendor/dubhe-network-core`。
3. 更新 `UPSTREAM.toml` 的 `revision`，保持源码与该提交完全一致。
4. 运行：

   ```bash
   cargo +1.89.0 test -p dubhe-network-core
   cargo +1.89.0 check -p mir2-gateway --all-targets
   cargo +1.89.0 test -p mir2-gateway --lib --tests -- --test-threads=1
   npm --prefix apps/dubhe-node-desktop run build
   TAURI_CONFIG='{"bundle":{"externalBin":[]}}' \
     cargo +1.89.0 test \
       --manifest-path apps/dubhe-node-desktop/src-tauri/Cargo.toml
   ```

5. 如果修改需要读取 Crystal 数据包、地图状态或 Mir2 Zone RPC，它应留在 Mir2
   adapter，不应反向加入 Core。

## 为什么暂时 vendoring

这是私有仓库阶段的可复现方案，不是最终发布形式。待
`0xobelisk/dubhe-chain-poc` 的公开发布、版本标签和供应链签名稳定后，可切换到
固定 Git tag 或 crates.io 版本。切换前仍必须固定精确 revision，不能跟随分支
HEAD。

## 许可

`dubhe-network-core` 使用 Apache-2.0，vendored 快照保留独立的 `LICENSE` 和
上游元数据。Mir2/Crystal 专用代码、资产和数据不因 Core 的许可自动变成
Apache-2.0。
