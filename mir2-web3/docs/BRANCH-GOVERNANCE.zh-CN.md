# Mir2 分支治理与恢复手册

更新时间：2026-08-02

## 主线规则

- 默认分支：`main`
- GitHub Ruleset：`Protect main production line`（ID `20223113`）
- `main` 禁止删除、禁止非快进推送，所有变更必须通过 Pull Request。
- 仅允许 Squash Merge；所有 Review Thread 必须解决。
- 仓库已启用 `delete_branch_on_merge=true`，PR 合并后自动删除源分支。

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
git switch main
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

## 必须保留到主线收口完成的分支

- `feat/mobile-ux-final`：PR #200。
- `feat/commercial-identity-gate`：PR #201，早期主线身份实现。
- `feat/ucloud-commercial-identity`：PR #202，当前生产身份实现与验收证据。
- `codex/world-director-approval-beta`：PR #202 的当前 Base，也是生产功能线。
- `hotfix/ucloud-gate15-new-player-20260802`：UCloud/Gate15 回滚依据。
- `feat/responsive-ui-gamepad`：合并 #193 后仍包含独有提交，需先审计。
- `codex/pwa-mobile-fullscreen`：尚无 PR 的最新独有提交，需先收口。

## 后续分支处置标准

- `branchOnly=0` 且无开放 PR：可删除。
- 有开放 PR、被开放 PR 用作 Base、被生产部署或回滚引用：保留。
- 存在独有提交但无 PR：先生成补丁等价性报告；选择移植、建立归档 Tag 或明确废弃后再删除。
- 禁止仅依据 `claude/`、`codex/`、`agent/` 前缀批量删除。
