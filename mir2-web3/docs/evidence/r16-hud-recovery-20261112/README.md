# R16 HUD失败补读候选

基于7033a012bf，候选7033a012bf39-hud-recovery-b26c3e87。期间合入的Gateway提交未修改Web/客户端，未部署Gateway。

仅对hud-core初次实际失败的URL做一次补读，最多8项、并发2，继续使用原完整正文/超时策略。只等待已存在Worker配置的750ms ACK，不等待注册。以局部真实读取结果重算同一个run的113项成功/失败，保留初次失败和补读日志；取消时不发后续请求。HUD补读结束后，六份原字体与阶段限时的运行包并行，避免两段大下载串行。

177项检查与生产构建通过，覆盖完整正文前不报成功、失败项专属重试、8项上限、并发、配置失败和取消日志。原65文件保全，另外的WIP不纳入。本次未变更WASM、规则、授权或存档。[回执](receipt.json) · [构建](build-receipt.json) · [177项](tests.log) · [修复前失败](before-recovery-red.log) · [构建输出](production-build.log)。

当前正式站仍c6bc815bae，画面/共享HUD/Q/签名到退出通过，预取108/113失败[保留](../r16-runtime-delivery-20261112/production/README.md)。本候选须再次部署并验实际113/113、缓存标记、完整画面/Native与退出，未预先记为通过。原120秒Native冷启动、实体手机和整体1:1继续开放。
