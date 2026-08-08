# Mir2 瘦客户端与 R2 素材边界

## 结论

`apps/web/public` 是可重复生成游戏内容的源码素材库，不等于要交付给玩家的客户端。
生产交付应使用 `npm run build:thin` 生成 `.mir2-thin-client`，再让浏览器从版本化
R2/CDN 按需获取登录、HUD、角色、怪物和声音素材。当前生产构建还会通过
`/api/asset-manifest` 声明经过验证的完整图集与内容寻址地图 Atlas 能力。

当前仓库的本地素材多，主要有三个原因：

1. 原始 Crystal 素材用于离线开发、素材转换、Atlas 重建和回归比较；
2. WebGPU 与 WebGL2 是两个不同的 Bevy WASM 后端，源码库需要同时保存，浏览器运行时只会选择一个；
3. `.next/cache` 和 `.next/dev` 是编译缓存，不是玩家安装包。

2026-07-30 的实测边界如下：

| 内容 | 逻辑体积 | 是否进入玩家瘦包 |
| --- | ---: | --- |
| `apps/web/public` 完整源码素材 | 429,070,362 B（409.2 MiB） | 否，只选择性复制 |
| `original-ui` | 155,487,185 B（148.3 MiB） | 大型目录走 R2 |
| `original-map` | 60,407,160 B（57.6 MiB） | 原始帧不进入瘦包 |
| `.next/cache` | 677,712,037 B（646.3 MiB） | 否 |
| `.next/dev` | 423,204,526 B（403.6 MiB） | 否 |
| `.next/server` | 7,505,924 B（7.2 MiB） | 是 |
| `.next/static` | 7,954,938 B（7.6 MiB） | 是 |

因此在工作目录看到约 700 MB 或 1 GB 以上，并不代表玩家会下载这么多；把源码素材、
开发缓存和实际发行物混在一起统计，才会得到这个数字。

本次验收生成的 `.mir2-thin-client` 为 348,608,686 B（332.46 MiB）；tgz 归档为
214,519,212 B（204.58 MiB），SHA-256：
`841d27b4178c3cc1a2a48f3df1b1532b58142eab4dad656d933a323604bc25a2`。

## 为什么不能直接删除全部本地素材

历史 R2 前缀 `mir2/v/37596e16d64fde7c/` 曾有大量对象：

- 86,447 个对象；
- 443,736,598 B；
- 其中 `original-map` 69,885 个对象，`original-ui` 16,551 个对象。

但对象数量多不等于“发布完整”。该历史前缀的
`remote-asset-release.json` 只登记 188 个文件，并且生成于 2026-05-22；当前新手地图
引用的部分原始帧（例如 `WemadeMir2/Objects/200.png`）在这个前缀下返回 404。
它已被新的不可变生产发布替代：

- 当前版本为 `20260730-fullcrystal-f71b89aa-gzip1`；
- full pack 已验证 1,440 个 library shards 与 4,446 张唯一页面；
- 紧凑地图 Atlas 已验证 57 张内容寻址页面，运行时拒绝未验证或可变 manifest；
- 同源产物存在时优先命中，同源缺失时 Service Worker 按浏览器安全的 R2 fallback 顺序回源；
- DOM 原始图片路径仍保留为低配/兼容回退，不会阻塞默认 GPU 地图渲染。

## 瘦包包含什么

`.mir2-thin-client` 包含：

- Next.js standalone 服务端与浏览器静态代码；
- `generated/map-atlas`：地图贴图页和索引；
- `bevy-entity-atlases`：新手区实体 Atlas；
- `bevy-runtime/pkg-webgpu` 与 `pkg-webgl2`；
- 原始特效、必要生成物、Service Worker 和本地兼容资源。

瘦包排除：

- `.next` 编译器缓存和 dev 缓存；
- `public/debug`；
- 已废弃的 `bevy-runtime/pkg` WebGL2 重复镜像；
- `original-map` 原始帧；
- 已由 R2 验证的 `original-ui` 大型 UI、角色、怪物、声音目录。

旧 R2 版本缺失的 3 个 HUD 小图标（`Prguse/2092.png`、`2094.png`、`2095.png`）
以及地图合成回退使用的 2 个小贴图，作为显式本地兼容资源保留在瘦包中；其余 QA
截图和调试样本仍然排除。

WebGPU 与 WebGL2 都保留是为了浏览器兼容，不是重复下载。页面只加载选中的后端，
约 35–37 MiB WASM；另一个后端不会进入该玩家的网络请求。

## 构建

```bash
cd apps/web
NEXT_PUBLIC_MIR2_ASSET_BASE_URL=https://assets.mir2.obelisk.build/mir2/v/20260730-fullcrystal-f71b89aa-gzip1 \
MIR2_ASSET_VERSION=20260730-fullcrystal-f71b89aa-gzip1 \
npm run build:thin
```

构建脚本执行正式 Next.js 构建，复制 standalone 运行物，按白名单裁剪公开资源，生成体积报告，
并用 360 MiB 逻辑体积作为硬上限。构建时还会强制用本地文件系统生成完整素材清单，避免
旧 R2 发布清单把 39,409 条源码记录错误缩成 184 条。源码目录不会被删除。

仅重新统计：

```bash
npm run report:client-size
```

报告写入：

```text
docs/generated/remote-assets/latest-thin-client-size.json
```

## R2 与包边界验收

```bash
npm run smoke:thin-client-assets -- \
  --assetBaseUrl https://assets.mir2.obelisk.build/mir2/v/20260730-fullcrystal-f71b89aa-gzip1
```

该命令同时检查：

- 两套 Bevy 运行时、地图 Atlas、实体 Atlas 和 Service Worker 是否存在于瘦包；
- 登录、选角、HUD、小地图和登录声音的代表性 R2 对象是否返回 200；
- CDN 是否提供一年缓存；
- 在线发布清单是否与当前不可变版本匹配。

Asset Delivery v2 还应运行 `npm run test:asset-delivery` 与
`npm run verify:map-atlas-release`。前者覆盖生命周期调度、release capability 与 Worker fallback；
后者逐文件核对内容寻址地图 Atlas。升级素材时必须创建新的不可变版本，不能覆盖当前前缀。

## 运行

```bash
cd apps/web/.mir2-thin-client
PORT=3002 \
HOSTNAME=127.0.0.1 \
MIR2_ASSET_BASE_URL=https://assets.mir2.obelisk.build/mir2/v/20260730-fullcrystal-f71b89aa-gzip1 \
MIR2_R2_PROXY_BASE=https://assets.mir2.obelisk.build/mir2/v/20260730-fullcrystal-f71b89aa-gzip1 \
node server.js
```

同源资源请求优先命中瘦包；瘦包里不存在的受管素材才由 `/api/r2-proxy` 回源 R2。浏览器的
`mir2-asset-worker.js` 按版本缓存资源，升级版本后使用新命名空间，不需要每次重新下载全部内容。

## 发布前验收清单

1. `npm run typecheck`；
2. `npm run test:asset-delivery`；
3. `npm run build:thin`；
4. `npm run smoke:thin-client-assets -- --assetBaseUrl <不可变 R2 根路径>`；
5. 使用真实 Gateway 登录、选角、进入比奇地图并移动；
6. 确认浏览器控制台没有关键资源 404，地图 Atlas 与选定 Bevy 后端处于 ready；
7. 压缩 `.mir2-thin-client`，记录归档体积与 SHA-256 后再发布。
