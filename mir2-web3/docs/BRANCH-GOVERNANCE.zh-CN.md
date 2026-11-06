# Mir2 分支治理与恢复手册

更新时间：2026-09-30

## 主线规则

- 默认分支：`main`
- GitHub Ruleset：`Protect main production line`（ID `20223113`）
- `main` 禁止删除、禁止非快进推送，所有变更必须通过 Pull Request。
- 仅允许 Squash Merge；所有 Review Thread 必须解决。
- 仓库已启用 `delete_branch_on_merge=true`，PR 合并后自动删除源分支。

## 2026-09-30 仓库整理

本轮仅整理 Git 引用、维护工具与文档，不合并 PR、不改游戏代码、不部署生产。原有脏工作区和所有既有本地 worktree 保留原样；维护变更在独立的 `codex/repository-hygiene` worktree 中完成。

清理前远端有 14 个分支、70 个标签、5 个开放 PR。本批移除 2 个旧分支名，新增 2 个归档标签；12 个业务/历史分支和原有 70 个标签保留。维护分支与本 PR 另计。精确提交与检查结果见 [本轮维护记录](REPOSITORY-MAINTENANCE-20260930.json)。

### 已归档的旧基线

这两条分支关联已关闭、未合并的 [PR #228](https://github.com/Zombieliu/mir2/pull/228)；后续跨平台主线集成由已合并的 [PR #229](https://github.com/Zombieliu/mir2/pull/229) 承接。它们与当前 `main` 没有共同祖先，**不能宣称旧分支的每个改动均已被主线包含**。

删除前完成：核验远端完整 HEAD、确认不是开放 PR 的 Head/Base、检查本地 worktree 引用、发布 Annotated Tag 并核验其提交。普通原子删除还使用命令级 pre-push 校验，若远端 HEAD 改变或出现计划外引用则拒绝推送；没有使用强推、reset、clean、stash 或覆盖标签。

| 已删除分支 | 可恢复 Tag | 原 HEAD |
|---|---|---|
| `codex/cross-platform-win-android` | `archive/legacy-branches/2026-09-30/codex-cross-platform-win-android` | `f6e0625e98071388041ce6181b3fe82e631c3cc5` |
| `fix/local-parity-and-i18n` | `archive/legacy-branches/2026-09-30/fix-local-parity-and-i18n` | `e65fca946a5ec208078497979f8946fc977d034e` |

恢复时先获取精确标签、核对解引用提交，再在**新的目录**中审计，避免切换当前脏工作区。例如恢复第一条：

```bash
git fetch origin refs/tags/archive/legacy-branches/2026-09-30/codex-cross-platform-win-android:refs/tags/archive/legacy-branches/2026-09-30/codex-cross-platform-win-android
git rev-parse 'refs/tags/archive/legacy-branches/2026-09-30/codex-cross-platform-win-android^{commit}'
# 必须得到上表 f6e0625e... 的完整 SHA；若本地同名标签冲突，停止，不覆盖。
git worktree add --detach <new-audit-path> 'refs/tags/archive/legacy-branches/2026-09-30/codex-cross-platform-win-android^{commit}'
```

归档用于恢复/审计，不是新开发基线。实际移植应从最新 `main` 新建 `codex/` 分支，选择必要补丁，重新验证。

### 保留分支的用途

| 分支 | 用途与本轮处理 |
|---|---|
| `main` | 受保护的默认主线；保持原提交，不直接推送。 |
| `codex/windows-visual-parity` | [#249](https://github.com/Zombieliu/mir2/pull/249)，Windows 角色/效果/UI/怪物音效；同时是 #250 的 Base，保留。 |
| `codex/windows-player-journey` | [#250](https://github.com/Zombieliu/mir2/pull/250)，Windows 玩家流程与共享状态/UI；同时是 #251 的 Base，保留。 |
| `codex/android-player-journey` | [#251](https://github.com/Zombieliu/mir2/pull/251)，Android 共享 UI、原生宿主及阶段证据；保留，不等同真机验收完成。 |
| `codex/wn-candidate-recovery` | [#248](https://github.com/Zombieliu/mir2/pull/248)，较早 Windows 候选恢复；相对当前 Windows 分支仍有 5 个独有历史提交，先查补丁等价性。 |
| `codex/steam-main` | [#230](https://github.com/Zombieliu/mir2/pull/230)，Steam 服务端校验/SDK/打包，独立渠道；保留。 |
| `codex/playtest-registration` | 报名失败提示、角色预览、DPI、多语言和 Windows 安装包等后续工作；包含当前 #250 HEAD，并多 12 个提交，无开放 PR，必须纳入 Windows 收口审阅。盘点期间该分支从 `a953243e...` 快进到 `f8800b9a...`，本轮未改动它。 |
| `codex/game-website` | Numeron 官网与 explorer 实现；相对 `main` 有 1 个独有提交，无开放 PR，保留待独立审阅。 |
| `codex/autonomous-quest-agent` | 自主任务代理实现/实验；有独有历史及本地 worktree，保留待等价审计。 |
| `codex/autonomous-quest-agent-main` | 任务代理主线集成/后续工作；不能因历史关联 PR 已合并就删除当前 HEAD，保留待审计。 |
| `codex/cross-platform-bevy-m0-m1-contract` | 早期 Bevy 跨平台契约/骨架；保留待与后续共享客户端实现比对。 |
| `codex/pwa-mobile-fullscreen` | 历史 PWA/移动端及后续改动；当前 HEAD 不等于 8 月归档 HEAD，保留待审计。 |

提交数量只表示历史差异，不等于功能仍缺失或已经迁移。上表是日期快照，当前提交以只读盘点结果为准。

### 下一轮整合边界

1. 先核对 #248 的 5 个独有提交，避免丢弃恢复工作。
2. 审阅 #249、#250 和 `playtest-registration`，形成明确的 Windows 集成候选、差异范围和实测/CI 基线，不直接逐个点击合并。
3. 当前 PR 依赖是 `main ← #249 ← #250 ← #251`，但这是 Head/Base 关系，**不是下游已经同步最新提交的证明**。`main` 仅允许 Squash Merge：收口后应重新核对下游 Base、差异及测试，必要时从新主线建立后续分支、按需移植独有补丁；不对现有工作分支自动强推。
4. Android 独立推进共享 UI、真实网络与玩家流程。构建、模拟器画面、真实登录和真机验收分别记录，Windows gate 不替代 Android 验收。
5. Steam、官网和四条实验线分别审阅；部署/回滚用途未确认前，不扩大删除范围。维护工具不替代游戏验收或生产变更授权。

### 标签保留与告警

原有标签全部保留：48 个 `developer-environment-starter-*`、1 个 `developer-image-*`、1 个 `developer-assets-*`、1 个 `repository-evidence-*`、19 个 `archive/*`；本轮新增 2 个 `archive/legacy-branches/*`。

- `developer-assets-f71b89aa3850` 仍由当前主线开发资源锁引用；资源包及 QA 证据 Release 不动。体积管理继续遵守 [大文件策略](REPOSITORY-LARGE-FILE-POLICY.md)，不进行历史重写。
- 49 个 Starter/镜像见证标签中，25 个名称内 SHA 与实际目标提交不一致。这里只告警；不能按名称认定是当前有效见证，也不能自动删除或重建它们。历史变化原因需要另行审计。
- 当前主线对应的 `developer-environment-starter-119553ff6aabbe05e7bcb4ee977a5470b477a250` 名称/目标一致；这不表示全部资源、当前镜像、游戏或手机验收通过。
- 下方 8 月归档表是当时记录。恢复历史归档时必须核对**当前远端标签解引用提交**；不要仅相信旧记录或标签名称内 SHA，也不要覆盖历史标签。

### 可重复的只读盘点

需要 Node.js 22+ 和已登录的 GitHub CLI。工具只调用 GET API，完整分页、按固定 SHA 比较；不会修改远端引用、Git 配置或工作文件，也不会输出凭据。

```bash
node mir2-web3/scripts/audit-repository.mjs --repo Zombieliu/mir2
node --test mir2-web3/scripts/test-audit-repository.mjs
# 可重放保存的 JSON 输出，不需要联网：
node mir2-web3/scripts/audit-repository.mjs --snapshot <report.json>
```

报告区分受保护分支、开放 PR Head/Base、未整合提交、无共同祖先和未知状态。404/403/限流/网络失败或过期比较不会被当成“可删除”；即使已经被主线包含，也只标记“待审阅”，不自动清理。该工具不覆盖本地 worktree、部署和回滚引用，执行清理前仍须单独检查。

本轮 16 项测试已在本地通过，但**没有新增自动 CI 工作流**：当前推送凭据缺少 GitHub `workflow` scope，包含新工作流的首次推送被拒绝。本轮因此仅发布工具、测试与文档，不改变凭据或权限。自动测试接入留待获得工作流变更授权后单独推进；本轮没有定时自动删除、远端维护或部署任务。

## 2026-08-02 第一批安全清理

下列 26 个远程分支在删除前均满足：

1. 分支 HEAD 是 `main` 的严格祖先；
2. 不是开放 PR 的 Head 或 Base；
3. 对应提交仍由 `main` 永久引用；
4. 删除分支名不会删除任何提交或代码。

| 分支 | 删除前 HEAD |
|---|---|
| `claude/fe2-packets` | `2b91a048289a7b84e7baa65a47606de9bb13df1f` |
| `claude/fe2-vfx` | `4d524387835d6522e50bf07d70b72bf076707f5d` |
| `claude/fe2-windows` | `fc0921bf70291154b472824fac58542213a2c177` |
| `claude/fe3-outbound` | `3fc3f9c4bbd512b6b4059c5346a5235ae11fe6ab` |
| `claude/fe3-tests` | `800e5a9cd7314ef5d99d7d49df0a596e7d9677ff` |
| `claude/fe3-windows` | `90ed45f3cd5d1101497e01b18fc011857026b6b6` |
| `claude/fe4-outbound` | `9a280185a78da1b1806562bb1bf3517ca272823a` |
| `claude/fe4-scene-hud` | `9a0cc79f36237e6616893f93a47858880b09c2bf` |
| `claude/fe4-sim-parity` | `1ac041967e5e4b35f1156dbe35a2bcfa0f82c42f` |
| `claude/fe4-ui-polish` | `48c04eaec23e45e35c160e782e8d487f5851d32b` |
| `claude/fe5-backend` | `85d9cc3ecbc6237d664323c268da4ef1fd37aaec` |
| `claude/fe5-fe-core` | `e5f18b7b15967f57844eb59131bdb8d43de8d981` |
| `claude/fe5-fe-play` | `b081b33bd1a9f3795e2b0b96299b310f4f622472` |
| `claude/fe5-fe-social` | `88ee6d47b16573a89bffcbbb1cad2fc688889c9b` |
| `claude/fe6-adapters` | `05c0aa82fb1df9f2f2c275b682e3c5544da3b577` |
| `claude/fe6-backend-data` | `797e4748b9989cf2e0e5dc2ceb66a341ef61726d` |
| `claude/fe7-adapters` | `3a49eac4676db728fab3a1f76132e72653ad55aa` |
| `claude/fe7-backend-data` | `37fdfe63dd25acd5197e950ffb7d7f07f1b293f4` |
| `claude/fe-audit-doc` | `21a5180a11d9b0141c001e03e490a4784b5b1584` |
| `claude/fe-packets` | `df36f3dc683517eb6870aeb096af07f1182b9013` |
| `claude/fe-sw-input` | `b64c073f3401da7a90e8b28441d9f63fe2c6d735` |
| `claude/fe-ui-windows` | `cbee958fcc490140214c9f6b7d185cb66d0f8bfa` |
| `claude/fe-vfx` | `50ee63947301f01565fb2fdc1e21a3c56a919663` |
| `claude/sim-parity` | `32d4a40bba2ab7cf7731013d03e9edafc5338302` |
| `codex/bevy-019-low-end` | `20f75496c68cb4487b5f95a5f85eacdc4a27a1fe` |
| `codex/weather-lighting-parity` | `ea9e98275abfeba3a216afea124b06befe1ef21c` |

### 恢复方法

如确实需要恢复某个名称，可从上表 SHA 重建：

```bash
git branch <branch-name> <full-sha>
git push origin <branch-name>
```

## 2026-08-02 独有提交归档

以下 12 个旧分支不满足“已被 `main` 完整包含”的条件，因此没有直接丢弃。清理前已为每个 HEAD 创建并推送远程 Annotated Tag，验证 Tag 解引用后的提交与原分支 HEAD 完全一致，然后才删除分支。

Tag 统一位于：

```text
archive/ai-branches/2026-08-02/*
```

| 已删除分支 | 可恢复 Tag | 原 HEAD |
|---|---|---|
| `backup/main-pre-codex-29852402` | `archive/ai-branches/2026-08-02/main-pre-codex-29852402` | `29852402e5c3aa5fd344dbb7de09aa910eef0b1e` |
| `claude/amazing-clarke-OR5tg` | `archive/ai-branches/2026-08-02/claude-amazing-clarke-OR5tg` | `da0257a91059aba42f6e31bf24c8f7d07fcf202e` |
| `claude/bevy-map-stage1` | `archive/ai-branches/2026-08-02/claude-bevy-map-stage1` | `cdc0af4b4613afc8b9adb94262d5c4901eccc195` |
| `claude/chinese-developer-docs-pcyypr` | `archive/ai-branches/2026-08-02/claude-chinese-developer-docs-pcyypr` | `2025db31d201142300ab7fe3498869e204f0b939` |
| `claude/eloquent-bardeen-azFSn` | `archive/ai-branches/2026-08-02/claude-eloquent-bardeen-azFSn` | `91708efd9ff2922ab2b1bc509d486cae08e0c786` |
| `claude/fervent-shannon-e78d51` | `archive/ai-branches/2026-08-02/claude-fervent-shannon-e78d51` | `dc7e6ad62319fd481d5eee00f283caa642eefa25` |
| `claude/great-keller-16RP5` | `archive/ai-branches/2026-08-02/claude-great-keller-16RP5` | `3fe543b6d4a111557c066c491e994ac80219083e` |
| `claude/hopeful-goodall-goCgp` | `archive/ai-branches/2026-08-02/claude-hopeful-goodall-goCgp` | `5d1da1e9f5d0ae752bd03eaaf2fedfaee88fb69d` |
| `claude/level-0-30-tasks-playable-m7HFX` | `archive/ai-branches/2026-08-02/claude-level-0-30-tasks-playable-m7HFX` | `a9603e8e3a5841fc9d005c0315cdffa339b005fa` |
| `claude/peaceful-gates-ZTEPJ` | `archive/ai-branches/2026-08-02/claude-peaceful-gates-ZTEPJ` | `091353d76b88927a017a924013957a5de7cd9c14` |
| `claude/quirky-mccarthy-ubgnio` | `archive/ai-branches/2026-08-02/claude-quirky-mccarthy-ubgnio` | `d3a8b9b9fec20721a37293a7bd68f9ad58714b7e` |
| `codex/fix-vercel-scene-bundle-20260530` | `archive/ai-branches/2026-08-02/codex-fix-vercel-scene-bundle-20260530` | `07a00d63878cf9d2fc879083ccc44d1219140e0a` |

恢复归档实现时，应从最新 `main` 新建功能分支，再按需 Cherry-pick 或手工移植；不要直接把旧 Tag 当作新开发基线。

## 2026-08-02 主线收口

PR #203 将 PR #200、#201、#202 以及 PWA、World Director、移动端、观战、商业身份和薄客户端等生产功能统一整合到 `main`。合并前已通过：

- Windows、macOS Intel、macOS Apple Silicon 和 Linux 桌面构建；
- Windows、macOS 与 Ubuntu 干净检出开发环境验收；
- Gateway、Home Agent Gate 22–25、Rust 工作区和 Web 资源门；
- 真实开发镜像及 Docker Compose 健康检查。

PR #203 以 Squash 方式合并为 `2aead73e6e9cb69ed0e6d915e731e75b06f37988`。PR #200、#201、#202 随后以“已被 #203 替代”关闭。

下列剩余旧分支均先创建并验证远程 Annotated Tag，再删除远程 Head：

| 已删除分支 | 可恢复 Tag | 原 HEAD |
|---|---|---|
| `codex/pwa-mobile-fullscreen` | `archive/superseded-prs/2026-08-02/codex-pwa-mobile-fullscreen` | `5e0408430ac9c7b9208c85172927861378bd709c` |
| `codex/world-director-approval-beta` | `archive/superseded-prs/2026-08-02/codex-world-director-approval-beta` | `b6e0b21ea1d18446b7b2567a6d4156ac2217a854` |
| `feat/commercial-identity-gate` | `archive/superseded-prs/2026-08-02/feat-commercial-identity-gate` | `e7bc8899f5ac338c654ca02c09616a535afed7c5` |
| `feat/mobile-ux-final` | `archive/superseded-prs/2026-08-02/feat-mobile-ux-final` | `cd3b4f356ca1d13cc2a6c883c6e3ca3b4bbdbd91` |
| `feat/responsive-ui-gamepad` | `archive/superseded-prs/2026-08-02/feat-responsive-ui-gamepad` | `57fe4518c09d22b6d62f8b0b87d2ae4ae0a8eb3a` |
| `feat/ucloud-commercial-identity` | `archive/superseded-prs/2026-08-02/feat-ucloud-commercial-identity` | `60d0cf6aa331c944f4d0e05c6df729395d9cef4d` |
| `hotfix/ucloud-gate15-new-player-20260802` | `archive/superseded-prs/2026-08-02/hotfix-ucloud-gate15-new-player-20260802` | `8681d9451c2a1934e68696c05a66afde5a86e1dc` |

收口后远程 Head 仅保留 `main`。如需恢复旧实现：

```bash
git fetch origin --tags
git worktree add --detach <new-audit-path> '<archive-tag>^{commit}'
```

恢复分支仅用于审计或按需移植；新开发仍应从最新 `main` 创建分支。

## 后续分支处置标准

- 经完整远端历史确认 `branchOnly=0`、无开放 PR 且无保护/部署/回滚/活跃工作依赖：经审阅后可删除，不自动清理。
- 默认/受保护分支、有开放 PR、被开放 PR 用作 Base、被生产部署、回滚或活跃 worktree 引用：保留。
- 存在独有提交但无 PR：先生成补丁等价性报告；选择移植、建立归档 Tag 或明确废弃后再删除。
- 无共同祖先、浅克隆或查询失败：不得推断已经合并。必要清理必须先建立并验证远端可恢复归档，再核对精确 HEAD；并发变化则停止。
- 不自动覆盖、移动或删除验证/资源/归档标签；不以标签名代替目标提交核验。
- 禁止仅依据 `claude/`、`codex/`、`agent/` 前缀批量删除。
