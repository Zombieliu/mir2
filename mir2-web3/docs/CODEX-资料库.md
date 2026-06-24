# 资料库 / Codex — 游戏数据库（玩家 + 开发者通用）

类似魔兽 Wowhead 的站内游戏资料库。数据 1:1 提取自 Crystal，**玩家**可查物品属性 /
职业需求 / 掉落来源，**开发者**可查 Crystal 原始字段与枚举解码出处。

> 第一期只做 **物品（1628 件）**。怪物 / 技能 / NPC 为规划中（见下文「扩展」）。

## 访问路径

| 路由 | 内容 |
|---|---|
| `/codex` | 资料库首页，列出各数据域 |
| `/codex/items` | 物品资料库：搜索 + 类型/品质筛选 + 详情（含开发者视图开关）|

页面在 `apps/web/app/codex/`，纯客户端从 `/codex/items.json` 拉取静态数据，不进主包、不改 `page.tsx`。

## 数据管线

```
packages/game-data/data/generated/
  crystal_item_manifest.json     (1628 物品)
  crystal_drop_manifest.json     (70k 掉落条目, 按掉落表)
  crystal_monster_manifest.json  (555 怪物, 含 drop_path)
        │
        ▼  scripts/codex/build-item-codex.mjs   (交叉链接 + 枚举解码)
        ▼
apps/web/public/codex/items.json  ← 页面 fetch 这个
```

**重新生成**（manifest 更新后）：

```bash
cd mir2-web3/apps/web && npm run build:item-codex
# 或： node mir2-web3/scripts/codex/build-item-codex.mjs
```

## 交叉链接（item ↔ drop ↔ monster）

- 怪物 `drop_path`（如 `Provinces\Sheep`）→ 掉落表 `relative_path` 按 basename 匹配（449/555 命中）。
- 掉落条目 `item_name` == 物品 `name` → 反查「哪个怪掉这件物品」（518/1628 物品有掉落来源）。
- 每条掉落来源带怪物等级、是否 Boss、概率（`chance_raw` 原文 + 数值，按概率降序）。

## 枚举解码出处（权威 = Rust 镜像）

Crystal 子模块在本仓库通常未 checkout，故所有枚举解码表均**转录自 Rust crate**（Crystal 的 1:1 镜像），
并在生成脚本头部与页面「开发者视图」中标注出处：

| 字段 | 出处 |
|---|---|
| Stat id → 标签 | `packages/protocol/src/types.rs:1745` `crystal_stat_label` |
| ItemType | `apps/simulation/src/runtime/crystal_compat.rs:178-198` (+ `fishing.rs:40-43`) |
| ItemGrade | `apps/simulation/src/config.rs:29` `ItemGrade` |
| MirClass（`required_class` 位掩码）| `packages/protocol/src/types.rs:29` `MirClass` |

**未在 Rust 源中确认的数值不臆测**：未知 `item_type` 直接显示为 `Type38` 等原始编号，符合项目
「code is authoritative，cite file:line」准则。开发者视图同时展示整条 Crystal 原始记录（`raw`）。

## 已知缺口

- **物品无中文名**：`localization_bundle.json` 主要是 UI 文案，没有覆盖 1628 件物品的中译表。
  目前标题显示 Crystal 内部名（如 `SpiritBlade`）。补中文名需要单独的翻译表。
- 部分 `item_type` 编号在 Rust 源中尚无常量（21–26、33–37、42 等），暂以原始编号显示。

## 扩展（下一期）

数据底料已全部就绪（怪物 555 / 技能 110 / NPC 375 / 配方 79 / 商店 105），复制 `build-item-codex.mjs`
的模式即可加新域：
1. 写 `scripts/codex/build-<domain>-codex.mjs` 生成 `public/codex/<domain>.json`；
2. 加路由 `apps/web/app/codex/<domain>/`；
3. 在 `app/codex/page.tsx` 的 `DOMAINS` 把对应卡片 `ready` 置为 `true`。
