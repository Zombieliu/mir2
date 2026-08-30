# Gate 12 Gateway save-recovery

Gate 12 的 gateway 固定使用实例身份 gate12-gateway-1、绝对目录
/var/lib/obelisk/save-recovery/gateway、逻辑卷 gateway-save-recovery 和物理卷名
mir2-gate12-gateway-save-recovery-v1。普通重启及修改 COMPOSE_PROJECT_NAME/-p
都必须保留这一映射。

实际运行 Gateway 的 service 必须显式声明
`com.obelisk.mir2.role=gateway`。该标签是验证器的权威 Gateway 清单；service
改名、换用 generic image 或使用默认监听地址都不影响发现。build/image/TCP+Web
线索只用于拒绝疑似 Gateway 缺标或错标。

启动前必须从密钥管理系统注入 MIR2_GATEWAY_SAVE_RECOVERY_MAC_KEY。Compose
只拒绝缺失或空值，不校验非空值的格式或强度。Gateway Rust 强度门为：

    cargo +1.95.0 test --manifest-path apps/gateway/Cargo.toml --bin mir2-gateway       --jobs 1 tests::empty_malformed_and_weak_recovery_keys_are_rejected       -- --exact --test-threads=1

recovery key、固定实例身份与 physical volume 是不可拆分的恢复单元，必须在同一
恢复点联合备份。固定 physical name 意味着同一 Docker daemon 只能运行一套
Gate 12；用不同 -p 启动第二套会复用同一 sidecar。独立集群必须使用不同
主机/daemon，或经过重新审计后显式改名。

仓库静态 wiring 验收命令：

    python3 infra/gate21/verify-save-recovery-compose.py

该命令不启动服务，不输出渲染 key，并明确把 Compose wiring 与未执行的 Rust
强度门分开报告。
