# TEST release controller V5

V5 为下一项 Close 回复修补提供独立的制品目录、状态与配置命名空间。
它沿用已审查 V4 的传输、CAS、九项归零、正常 SIGINT、保护配置核对和禁止旧二进制
回退规则。V4 源码及其历史状态、回执、制品和四组既有配置均保留。

本工具只控制 `mir2-playtest.service`。旧 Gateway、旧 Zone、Caddy、验证者、VPN、
路由、防火墙和 sysctl 不在操作范围内。不读写账号数据，不重置私有认证账本。

## 命名与准备条件

- 新制品后缀：`-equip-logout-20261112-v5`。
- 永久 drop-in 为 50 个 `z` 加 `-equip-logout-20261112-v5.conf`，
  环境文件为 `/etc/mir2-playtest-equip-logout-20261112-v5.env`。
- 临时 drop-in 为 52 个 `z` 加 `-equip-logout-capacity-20261112-v5.conf`，
  环境文件为 `/run/mir2-playtest-equip-logout-capacity-20261112-v5.env`。
- 准备快照须有 17 个唯一 EnvironmentFiles，末四项依次是 v1/v2/v3/v4 的永久环境文件；
  四个永久 drop-in 必须按同样顺序保留。它们都不是 V5 所有的路径。
- 预期环境文件数为准备 17、安装 18、提额 19、恢复 18。每个既有文件的身份及哈希
  仍受完整 baseline 保护；等长替换或交换顺序也必须拒绝。

所有命令使用新建的、仓库外 `release-v5` 状态目录；每一步都传入当时实际完整 PID。
当前已核实的旧版本是 `62931e1c5111ca978808a3b0702c12183974d53d`，二进制 SHA-256 为
`245985b46951ce31356471bae5a1eff16e6bcb01b16c4c4cb7bbf5e0c76f97d3`。
新的候选版本、二进制、CI 和 PID 必须来自实际制品与回执，不能由这个说明推定。

## 执行顺序

先完成新候选的原生 Linux 制品、启动、安全、地图与精确源码兼容性验证，再运行
`prepare → install`。`prepare` 需要固定的旧源码/二进制、实际候选完整源码、制品目录、
完整候选验证封装及其 SHA-256，并显式提供 `--confirm-maps-match` 和
`--deny-old-binary-rollback`。候选封装本身必须引用已逐项核对的地图证明。

SSH 必须在本地 PTY 中通过 getpass 临时输入，先核对固定 ED25519 主机指纹；
密码不能放入 argv、环境、文件或 sudo stdin。仅使用已审查的 `sudo -n` 路径。

安装后先用原私有池和原账本完成低人数装备、正常退出及保存重登检查。只在九项归零
且原安装身份一致时 `apply-caps`，临时上限为 120/150/120；认证仍为 2/1/2，内存、
保护阈值和客户端时限均不改变。百人测量及其监控实际结束后 `restore-caps`，恢复
51/66/51 并保留本次新二进制。恢复只读回执必须再次证实完整九项归零及全部保护项。

V4 失败案的公开归档依赖其实际 V4 状态哈希。在运行任何新 V4 操作之前必须封存它；
V5 使用独立状态目录，不重写 V4 状态或旧结果。绝不把一次准备、安装或恢复成功当作
百人可玩成功。

## 离线验证

运行 `python -B -m unittest discover -s mir2-web3/scripts/capacity/cloud-reference -p test_cloud_test_release_v5.py -v`。
测试包含四组旧配置的哈希/inode 替换、顺序变化、路径冲突、九项计数、正常退出、
并发新会话、错误版本/二进制、禁止旧回退，以及 SSH 认证/输入闭合边界。
AST 回归固定 V4 哈希，并要求除 `transaction` 中的准备 guard 外，其他函数和类保持一致；
人工 diff 还须确认 `transaction` 的变化只扩展准备 guard。
