# Windows 弓手资源缺失修复

完整 Windows host 回归已通过：**1140 passed / 0 failed / 7 原 ignored**，0 filtered，19.54s。原失败用例独立复跑为 1 passed / 0 failed，1146 filtered，0.02s。两次实际执行均绑定 `53700985cf98f1a4aa479e763f3b27ba3c68053b`；后续文档提交不改写执行版本。

[脱敏结果与资源哈希](generated/player-qa/archer-assets-20261017/outcomes.json)保留两次原始日志、结果、Guard 审计及快照的 SHA256。旧 `fd21fbdc38` 的 1139/1/7 失败记录继续保留在[历史缺口](generated/player-qa/fashion-20261017/host-asset-gap.json)。7 项忽略逐项与旧完整调用相同，没有新增 ignore，也没有修改 Rust 测试或降低资源断言。

## 原因与修复

本机已安装资源包较旧，缺少 `ArcherNormalArrow` 的 16 方向 × 5 帧、`ArcherFocusProc` 的 10 帧、对应完整元数据和两个生产特效配置。配套发射、命中及凝神音效也缺失。仓库中的原版导出资源完整。

[准备脚本](../scripts/prepare-native-archer-overlay.mjs)将已安装资源作为只读基底，生成新的本地测试资源目录：

| 范围 | 修复与核对 |
| --- | --- |
| 普通箭 | `1030 + 10*d + [0..4]`，`d=0..15`，80 张 PNG |
| 凝神 | `2730..2739`，10 张 PNG |
| Magic3 元数据 | 179 → 269 条；原有记录不变，库 `count=7898` 不变 |
| 生产特效配置 | 原样补入两个 Source `client_effects` 条目，10 → 12；其他条目及区段不变 |
| 音效 | `M121-0.wav`、`M121-2.wav`、`M121-5.wav`；Sound 文件数 62 → 65 |

90 张 PNG 共 135969 字节，逐张完整解码、尺寸、PNG SHA256、RGBA SHA256 均与 Source 元数据一致。3 个 WAV 共 319900 字节，完整读取并核对 PCM 单声道、16 bit、44100 Hz。两组生产配置与 Source 相同，Magic3 可用库没有重复，后续 object/map 区段没有覆盖同名特效。

本次输出为 `.mir2-player-qa/archer-assets-20261017/assets`。可写分支为实体目录，其余资源通过目录别名引用安装基底；每次写入都检查父路径，拒绝写穿别名。244 个原基底文件前后哈希不变，38 个目录别名已核对。该目录是依赖本机安装基底的本地 QA 资源根，不是独立发行包，也不表示全局安装资源清单已与 Source 对齐。

## 复用方式

在仓库根目录运行，使用一个尚不存在的输出目录；输出的父目录必须已存在：

```powershell
node ./mir2-web3/scripts/prepare-native-archer-overlay.mjs --base-root <已安装资源根> --output <新的测试资源目录>
```

脚本从自身位置查找仓库资源，不假设盘符。输出目录已存在时会拒绝覆盖，旧结果继续保留。启动本地客户端或资源测试时，将 `MIR2_NATIVE_ASSET_ROOT` 与 `MIR2_NATIVE_AUDIO_ROOT` 指向新目录。

本机后续时装验收的入口更新为 `fashion-20261017/start-local-client07.ps1`，它在复制和启动客户端前额外核对资源收据及 95 个补齐资源/配置文件的哈希。原 `start-local-client06.ps1` 和历史执行记录保留。入口准备不代表启动或 GUI 验收。

本轮 Cargo 仍经原单通道 Guard 执行，50 GiB / 2000 ms / 15000 ms / Policy B 不变。冻结输入从原有 1453 项扩展到 1549 项，原有产品输入逐项不变；新增脚本、90 PNG、3 WAV、2 JSON 被纳入。95 个输出资源/配置在两次调用前后均未变化，源码漂移为 0。

本轮没有启动新的客户端或服务，没有执行 GUI、真 PostgreSQL 或部署；`Candidate100=false`。
