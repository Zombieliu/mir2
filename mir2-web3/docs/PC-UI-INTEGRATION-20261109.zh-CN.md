# Windows PC UI 本地集成记录（2026-11-09）

## 范围与状态

本轮仅处理 Windows 显式 opt-in 的黑金 HUD 与钱包优先的实际原生登录页。HUD 使用历史本地参考实现和两张编译内置资源；保留既有账户、钱包取消/刷新、安全键及服务端权威数据路径。未开发 Web/Android 对应界面。

最新平面渲染稿的官方导入仍失败，因此本轮不代表与该渲染稿一致，也不是全套 PC UI 完成。没有修改认证、安全签名、资源信任或正式发布准入规则。

原始隔离分支基线为 `6d30e0f9`。规范线现为 `8a8fd7a8`，新增差异仅涉及 Web/website/docs，与本轮代码不重叠；本记录生成时仍待本地整合，不能据此声称已交付规范线。

## 已取得的证据

- 离屏钱包 GPU 测试通过；属于 UI 测试，不是钱包认证成功。
- HUD 独立离屏 UI fixture 通过。使用合成 HP/MP、空世界，不是实际角色进入游戏或玩法验收。
- 正常开发 EXE 编译成功，并实际启动到简体中文空账户登录页。窗口正常 Close，返回 `AppExit::Success`。
- Windows suite：1254 通过、2 失败、9 忽略；不能称全部通过。
- 新增 shared client 的 3 项 `pc_login` 测试全部通过；shared native-ui 全套 1821 通过、2 失败、20 忽略，其中 crystal_ui 977 项、native_shell 87 项通过。
- 未登录、未执行钱包签名、未进入世界；未正式打包或发布。

证据根目录（下称 taskbase）：
`C:\Users\Administrator\Documents\Codex\2026-11-08\task\pc-ui-integration`

开发程序：`taskbase\client\Numeron-PC-UI-Dev.exe`
SHA256：`CBC9374B6C832708D59E51336534DA6894F5B99344A860E92330016F87507F06`

实际窗口截图：
`taskbase\native-login-20261108T212223Z\captures\pc-ui-native-login-login-1794172945740-1.png`
SHA256：`18b2110186f930c46cc35ebedfb5f6beda9e860cb70d2d81122b29d0df8cd821`
截图 metadata：`eligible=false`，原因 `trusted-build-provenance-unavailable`。继续保留公开限制；该开发截图不构成可信发布或公开验收证据。

仓库内可移植截图与回执见 [本轮证据](evidence/windows-pc-ui-20261109/README.md)。最终正常客户端增量构建再次通过，EXE 哈希未变。`default-run` 保持指向真实客户端，离线证据二进制不会改变日常 `cargo run` 入口。

主要日志：`build-pc-client.log`、`test-wallet-gpu.log`、`test-hud-preview-v2.log`、`test-windows-full-with-assets.log`、`test-client-bevy-full-native-ui.log`、`test-pc-login-model-native-ui.log`、`build-pc-client-final.log`，均位于 taskbase。失败尝试保留为独立日志，不以历史成功覆盖。

## 两项尚未通过的 Windows 测试

1. `effects::tests::crystal_foreground_scene_and_actor_effects_render_after_selected_redraw`：`effects.rs:8032` 附近断言 persistent ObjectSpell 应留在 world pass 失败。测试直接构造 NativeEffects 与假动画后调用 tick，不执行本轮 HUD/login/input 插件。Persistent 的 effect_z 使用绝对坐标，而 atlas 深度边界经 world_depth::object_depth 使用相机相对坐标，存在既有深度坐标体系不一致。
2. `shell_bridge::tests::billing_scope_stays_local_and_closing_cancels_the_exact_request`：`shell_bridge.rs:684` 附近期望 billingStatus JSON 无 includePaymentProviders，但现有 forward_billing_request 明确设置 true。测试直接创建 shell/billing 请求并序列化，不经过 PC 登录展示或按钮处理。

上述为源码调用路径分析，说明与本轮 PC UI 改动无因果关联；没有独立执行基线重现，不能称已取得基线复现证据。两项保留，不在本轮擅自修复。


## shared native-ui 未通过项

1. `quest_hunt_regions::tests::wooma_practice_uses_three_actual_footholds_even_after_kill_quota`：`quest_hunt_regions.rs:239` 的三个据点数量已通过；`:242` 期望半径 3，而配置 spread=0 经 `:146–149` 下限规则得到 2。测试直接构造 QuestTracker 并筛选编译内置配置，不实例化本轮 UI。
2. `quest_ui::periodic_quest_ui_tests::periodic_nine_language_catalog_covers_actual_server_copy_without_losing_reset_rules`：`periodic_quest_ui_tests.rs:602` 读取 `entry[locale.code()].as_str()` 得到 None。当前 Locale::ALL 包含 zh-CN，但 `native-i18n/periodic.json` 的 26 项 periodic.kill 条目均缺此键；测试直接遍历目录及文案函数，不执行本轮 HUD/login 插件。

这两项同样只有源码因果诊断，未独立复跑原始基线，也未在本轮顺手修改。不能将共享库全套标记为通过。

## 本地复验

以下仅用于开发测试。先读取并遵守仓库 AGENTS，使用现有 Rust/Windows 工具链；不得修改生产签名规则或公开限制。

在隔离 checkout 下执行：

```powershell
$env:MIR2_NATIVE_ASSET_ROOT = 'E:\mir2-player-journey\mir2-web3\apps\web\public'
$manifest = 'mir2-web3/apps/game-client/platform-windows/Cargo.toml'
rustup run 1.95.0-x86_64-pc-windows-msvc cargo test --manifest-path $manifest --bin mir2-platform-windows --offline --locked -- --test-threads=1
$env:MIR2_PC_HUD_VISUAL_OUT = Join-Path $env:TEMP 'mir2-pc-hud-fixture'
rustup run 1.95.0-x86_64-pc-windows-msvc cargo test --manifest-path $manifest --bin mir2-platform-windows --offline --locked pc_hud_visual_tests::render_pc_hud_offscreen_simulation_fixture -- --ignored --exact --test-threads=1
```

本轮实际 GPU HUD 截图通过独立 `numeron-pc-hud-preview` 二进制生成，复验可使用 `rustup run 1.95.0-x86_64-pc-windows-msvc cargo run --manifest-path $manifest --bin numeron-pc-hud-preview --offline --locked`。shared client 测试使用其自身 manifest、`--features native-ui --lib` 与 `profile.dev.opt-level=1`、`profile.dev.package."*".opt-level=3`，全套则运行对应已编译测试二进制。

HUD fixture 模块/名称应以当前源码为准；离屏需要可用 GPU。运行全量测试须使用上述完整开发资源根，而非隔离 worktree 不完整的 public。已验证该根的必要 manifest、Items、StateItem、Prguse2、Cursors 文件存在；Bichon 地图来自受支持的相邻 `apps/web/lib/generated/crystal-map-pack/0.map.gz`。文件存在性检查不等于所有可选帧完整性认证。

最终仍需真实登录、钱包批准、角色入世界与交互观察，以及独立正式打包/签名/发布验证；不得以离屏合成图替代。
