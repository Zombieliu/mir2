# 资产 CDN Range 已修复上线，完整字库仍未通过

2026-11-15，沿 `codex/windows-player-journey`，产品提交 `f28dc9453856955fce19e005647d9612a2930f7c`。本轮只发布 `mir2-r2-asset-cache`；游戏前端仍为 `20de5ea4fd`／`AWss`，Gateway 仍为 `fd3fcedc8b`。没有重启游戏服、修改存档或切换游戏前端。整体 1:1、两场完整本地／公网及最终人类验收继续开放。

## 修复和发布

原 CDN 及 hotlink 字体请求返回正确的 64KiB 原字节，却携带 `Content-Range: bytes 0-NaN/16437364`。原因是代码读取了不存在的 `object.range.end`。[R2 API](https://developers.cloudflare.com/r2/api/workers/workers-api-reference/#ranged-reads) 的范围使用 `offset` 和 `length`。

现在根据实际返回的起点和长度计算包含结束字节的区间，并对照请求及完整对象大小检查。合法的闭区间、开放末尾和超出末尾后的裁剪均返回正确的 206、`Content-Range` 和 `Content-Length`；缺失或不一致的元数据返回不缓存的 502。HEAD 忽略 Range；存储 gzip 的请求继续回退完整解压后的 200，不携带压缩前长度。未使用的 R2 正文会取消。未知 Range 语法仍回退完整响应，本轮没有实现完整的 suffix／multipart／不可满足区间语义。

发布前后均只读核对实际账号、脚本、路由、桶、版本和四个运行变量。资源前缀、fallback、兼容日期、绑定和路由保持；按 Worker 技能要求启用 10% trace 采样，原日志保持。Cloudflare 连接器需重新认证，但本机 Wrangler 的既有登录已可用，没有再次索要凭据。部署使用项目现有 CI 同版 Wrangler **4.112.0**，未升级依赖或更改部署工作流。

- 旧 Worker 部署：`c2ca3842-0b1a-40db-8d7f-d2e4af36bfdf`。
- 回退版本保留：`4317a349-f738-4234-8613-8ad33bee9402`。
- 新部署：`ab5d6857-21c5-4379-bec5-8af6e20bf4c5`。
- 新版本：`9e9b30bd-b567-4f5d-8bf6-f8f43eb4fd69`，100% 流量，已由独立 API 读回。
- 实际上传代码 11,139 字节，SHA256 `cf9b674c5f8461cf647f15f3c197a5c1cac14d030c9d093a30de956942672d89`；通过独立 API 下载已部署模块再验摘要，与资格预演包一致。

## 验证

`npm run test:asset-cdn-ranges`：**22 项通过**，无跳过；已接入已有 `test:asset-delivery`。相邻的资产发布契约及域名代理检查 **9 项通过**。最终同一份 22 项夹具运行于未修改的旧源码，2 项通过、20 项失败，保留原输出。首轮夹具自身关闭全局缓存的错误单列保留，更正后才做上述旧源码对照，没有倒填首次结果。

使用 Wrangler 生成的实际运行时类型做严格 TypeScript 检查通过。两次 `deploy --dry-run` 通过，最终配置的生成代码 SHA 与之前真实运行时复验的代码相同。独立 Miniflare／workerd／本地 R2 复验 **9 项通过**，实际 R2 返回 `{offset:2,length:4}`，验证区间、HEAD、hotlink、overlay fallback 和 gzip；未连接或写入远程 R2，运行时已释放。最初私有脚本的 Windows ESM 路径和跨盘根目录错误均发生于正式用例之前，原失败日志保留；修正私有夹具路径后才得到这 9 项通过。

## 公网实际结果：响应头通过，完整字库失败

发布后，canonical 和 hotlink 两个入口各验证四项，共 **8 项协议检查通过**：

| 请求 | 实际状态和范围 |
| --- | --- |
| 前 65,536 字节 | `206 bytes 0-65535/16437364` |
| 最后 1,024 字节，开放末尾 | `206 bytes 16436340-16437363/16437364` |
| 最后 10 字节，请求超出末尾 | `206 bytes 16437354-16437363/16437364` |
| HEAD 携带 Range | 200、完整长度 16,437,364、无正文和 Content-Range |

六个 GET 的长度、CORS、无编码、BYPASS 和原字节摘要均验证，不依赖截图判断。

随后以两个并发读取完整 SC 字体的 16 个 1MiB 分段，**本次整体失败**。编号 0、2、3、4、5 共五段、5,242,880 字节完整且逐段 SHA 与原字体匹配；其余没有形成完整字体。本次私有探针的每请求 30 秒期限出现 `TimeoutError`，完整字体 SHA 未取得。这只是 Node 协议／字节测试；不是游戏内 Native、普通浏览器速度或完整体验通过。原失败原样保留，未增加期限、重新拼补缺段或以五段通过替代完整体。

独立于这次失败，再次读取游戏 `/version` 和 Gateway `/playtest/health`，两者均 200，分别为原 `20de5ea4fd` 和 `fd3fcedc8b`，服务健康。CDN 响应修复保留正式；尚不能据此认定普通整文件字体停滞的根因或恢复游戏前端发布。

## 下一边界与保全

浏览器的字体 Range 加载器、分段保留及失败恢复**尚未实现**，第一方入口忽略 Range 也未在本轮修改。下一步针对已验证的 CDN 区间设计有界的分段获取与校验，保留原完整字体摘要、全部六套字体和原 8／65／120 秒体验门槛，再重演两场完整本地及公网流程。[上轮繁体 Native 超时](../r16-walking-parts-20261115/README.md)和原运行包组合故障不能由本轮关闭；其它地图／目标、三职业、阿洛、语言像素、Windows 窗口与实体手机继续。

原 30 份内容 WIP 中 29 份原始 SHA 保持；容量文档由独立提交 `213fc1737f`／`b80759fe3b` 更新，后续独立合并 `362ea11c24` 也已保留。动画 WIP、Native 素材及其它未提交文件没有纳入本轮产品提交。统一代码线检查通过，Goal 保持 active，整体 Candidate100／Accepted100 均为 false。

详细范围、五份源码绑定、实际发布及失败值见 [qualification.json](qualification.json)。[原始私有回执摘要](private-receipt-pins.json)保留每份记录的字节和 SHA；认证配置、令牌和密码未复制到本证据目录。
