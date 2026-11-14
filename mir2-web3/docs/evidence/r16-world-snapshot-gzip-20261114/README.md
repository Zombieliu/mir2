# R16 完整场景快照压缩候选（2026-11-14）

已实现服务器与 Web 的显式协商压缩，完成本地协议/真实 TLS 回放、真实 WASM、相关回归及独立生产构建，**尚未部署**。这是减少同连接大消息的一项候选；原公网 61.929 秒 ACK 失败与根因未明的结论保持，不宣称延迟已经修好或整体 1:1 完成。[原失败和采样](../r16-startup-delivery-20261114/README.md)

## 行为与兼容

Web 只在加载到匹配的实际解码器后声明 `serverWorldSnapshotGzipV1`。声明完整发送成功后，解码器绑定当前物理连接；后续能力更新保留这个已绑定的能力，换连接重新声明。旧 WASM 仍只声明目录能力，旧服务器及未声明能力的客户端继续原 Text 路径。

服务器仅压缩原 `worldSnapshot` 整条文本，保留所有字段、UTF-8、数字拼写、字段顺序和消息位置。身份、普通协议包、经济回执及 `npcPurchaseOwner` 保持原路径。未删快照、改奖励、碰撞、组队、战斗、存档或授权规则；大于限制、过小或压缩无收益的快照继续原 Text 表示。

| 二进制域 | 规则 |
| --- | --- |
| Magic | `M2WSPGZ1`，与目录的 `M2CATGZ1` 分离 |
| 载体 | 复用原有有界 gzip framing，16 字节头、一个 UTF-8 文本条目 |
| 声明与正文 | 仅一个 `worldSnapshot`，payload 必须为对象；重复/未知外层字段及其他消息种类拒绝 |
| 边界 | 解码区含长度前缀最多 128 KiB，文本最多 128 KiB−4，wire 132 KiB；原上限不扩大 |
| 失败 | CRC、尾随成员/数据、截断、长度或类型异常在任何交付前拒绝；沿用当前连接关闭与换连接隔离 |
| 顺序 | 同步 Rust WASM 解码后交回原 Hero/raw 和普通消息处理，无异步 Blob 队列 |

目录 ABI 保持 1，新增独立快照 ABI 1 导出；目录验证器仍拒绝快照域。四份 Core policy 包保持原各自小于 262,144 B 的预算，目录 policy 为 195,564 B。渲染器未重编译，独立 Next 构建使用已发布 `bevy-0c553a754308e5b8` 的固定清单，未修改 canonical 的渲染器清单。

## 已通过的范围

- 协议 **100 项**：50 单元、16 目录 envelope、34 codec。涵盖精确文本与 u64 拼写、边界、CRC/截断、独立 magic 和拒绝身份/回执等。
- Gateway **35 项**：新连接级开关/实际 writer、原有目录、写入期限/失败围栏，以及本地真实 TLS WebSocket 的普通注册→登录→建角→进城→转向 ACK→正常注销。旧目录 opt-in、旧客户端和原 resume 回归同时通过。
- Web/Core **59 项**：实际优化后的 WASM、独立 ABI/manifest/预算、旧模块兼容、真实 Page 能力替换和原消息入口、Text/快照/目录同步顺序及坏包/旧连接隔离。
- 独立 Next 生产构建与 TypeScript 通过，BUILD_ID `wBO16WGldRpPirWZxFGa1`。用于后续本地 UI 的同源 Gateway debug EXE 也构建通过，未当作生产工件或 Windows 玩家安装包。

194 项是本轮三个相关集合之和，不重复累计早先的 4/4/14 子集，也不是全工作区或完整游戏验收。[资格范围](qualification.json) · [检查投影](checks.txt)

真实 TLS 用例的四条已协商快照：原文本 **167,824 B**，压缩后的 WebSocket **消息载荷 38,070 B，减少 77.316%**。这个数不包含 WS/TLS 头，也不代表整个连接流量或公网延迟下降同等比例。登录前尚未协商的首条快照仍是 Text。

本地默认数据用例包含 **502 条完整目录 envelope**，新旧客户端逐条原字符串相等；公网的 529 条验收门槛保持不变。第一次新测试错误地套用了公网计数，实际 34/35，通过修正为与同一完整本地旧客户端逐条对照后 35/35；原失败保留。[失败及工具恢复](retained-failures.json)

## 来源与保全

构建从已提交 `40d1662f83a52511d2e845ca237180fc6201dec8` 提取，加本轮 11 份明确文件；缺失的 56 个编译期 include 文件全部从同一提交补齐，未复制其他 Native/Shell WIP，也未执行这些 SQL 文件。初次源子集缺 include 的构建失败保留。

首次 Next 构建的跨盘依赖链接生成了无效 `./E:/...` 路径；旧链接保留，改为新物理源码及物理依赖目录后完整通过。新 Core 清单的 source fingerprint 与 canonical 本轮源相等，编译后的 Next 清单与匹配的四 policy 清单相等；八个新不可变文件逐项 SHA/大小校验，旧叶子未覆盖。[清单](core-manifest.json) · [发布到本地源码的叶子](core-publication.json) · [隔离布局](build-layout.json)

原始输出留在 `C:/r16-world-snapshot-gzip-20261114`，提交仅含脱敏投影及[原始输入摘要](inputs-manifest.json)，未导出实际游戏快照、认证凭据或模型密钥。既有 Shell 未编辑，仍 20+/9−；其他未提交改动继续保留。

## 复测与下一项

原命令在独立源码使用已安装 MSVC、Rust 1.89.0、WASM Rust 1.95.0、绑定工具及已验证 Binaryen 131；相关包未新增依赖或扩大预算。

```powershell
cargo +1.89.0 test --locked -p mir2-protocol --features catalog-gzip
cargo +1.89.0 test --locked -p mir2-gateway --lib -- web::world_snapshot_transport:: web::catalog_transport:: web::transport:: --test-threads=1
node --test scripts/test-gateway-catalog-transport.mjs scripts/test-client-core-runtime.mjs scripts/test-client-core-release-files.mjs scripts/test-client-core-build.mjs
```

下一步用匹配的新 Gateway 和已构建的 Web 完整跑实际 R16 游戏浏览器流程，确认压缩快照确实进入原界面、走路/寻路及旧服务器兼容；再准备同源 Linux 工件与受保护的发布和公网扫码、首步、完整字库/HUD 冷启验收。当前仅有 TLS 测试客户端和 Node WASM 证据，**尚无完整游戏浏览器消费压缩快照的通过记录**，也没有新线上发布。原 8 秒移动、120 秒冷启动、字库、公共目录与完整 1:1 门槛继续有效。
