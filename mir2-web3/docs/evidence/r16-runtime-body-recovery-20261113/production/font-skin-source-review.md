# 固定源码的字体与皮肤恢复边界

核对来源固定为 `432c58a07e`。只读检查没有使用或修改未提交的 Native 工作副本，也没有为本报告新增浏览器、资源读取或签名场景。

实证是两条独立边界：资源尚未完整取得；项目还缺少同一 Bevy 实例在资源失败后的恢复。TC 上游曾 200 后传输终止，随后当前镜像／源站多次超时，旧版本副本 404；现有字体网络记录缺少该次逐块间隔，不能断言它一定由 15 秒 idle 触发。TC 最终完整缓存仍缺失。827 是 HUD 商城 hover 资源，不在 Quest required skins，不能将其当作 Quest skin 失败的具体根因。

固定源码的调用链：

- [PortableUiFonts.request:56](https://github.com/Zombieliu/mir2/blob/432c58a07e44db021679aab9a6562a02c9124b10/mir2-web3/apps/game-client/runtime/src/portable_ui_fonts.rs#L56) 使用 `HashMap::or_insert_with(server.load)`；已有 Failed 句柄仍占用条目，不再次请求。字体 [readiness:79](https://github.com/Zombieliu/mir2/blob/432c58a07e44db021679aab9a6562a02c9124b10/mir2-web3/apps/game-client/runtime/src/portable_ui_fonts.rs#L79) 检查完整字体集合，另需 [FontCx fallback:172](https://github.com/Zombieliu/mir2/blob/432c58a07e44db021679aab9a6562a02c9124b10/mir2-web3/apps/game-client/runtime/src/portable_ui_fonts.rs#L172) 真正完成。
- [HUD:353](https://github.com/Zombieliu/mir2/blob/432c58a07e44db021679aab9a6562a02c9124b10/mir2-web3/apps/game-client/runtime/src/hud_ui_host.rs#L353) 只在 `!applied.requested` 时加载全部图片；[HUD:373](https://github.com/Zombieliu/mir2/blob/432c58a07e44db021679aab9a6562a02c9124b10/mir2-web3/apps/game-client/runtime/src/hud_ui_host.rs#L373) 要求字体及所有图片 Loaded。
- [Quest:889](https://github.com/Zombieliu/mir2/blob/432c58a07e44db021679aab9a6562a02c9124b10/mir2-web3/apps/game-client/runtime/src/quest_ui_host.rs#L889) 请求字体和首次 skin；[required skins:144](https://github.com/Zombieliu/mir2/blob/432c58a07e44db021679aab9a6562a02c9124b10/mir2-web3/apps/game-client/client-bevy/src/quest_ui.rs#L144) 定义具体皮肤列表。
- [Quest:1427](https://github.com/Zombieliu/mir2/blob/432c58a07e44db021679aab9a6562a02c9124b10/mir2-web3/apps/game-client/runtime/src/quest_ui_host.rs#L1427) 将 font/skin 失败写入 `status.error`；随后只清 viewport 错误，没有在资源恢复时清这条 asset error。ready 又要求 error=None。

固定依赖为 Bevy Asset 0.19.0。本机同版 `server/info.rs:233–240` 表明 `AssetServer::load` 对已有 Failed／NotLoaded 资源能沿同一 ID 重新进入 Loading。阻断点是项目不再调用它，不能泛化成 Bevy 自身不支持重试，也不应默认 force reload 全部资源。

修复应同时覆盖真实字节取得、失败句柄有界再请求、恢复后的错误清理。只重试 Failed，不重载 Loading／Loaded；按路径去重并保留原句柄、generation 和 FontCx 来源。只有全部必需字体、fallback 与 skins 真正就绪，才清对应 asset error；保留身份、输入和 viewport 等其他错误。

验证应在同一实例中先制造 TC 504，再恢复真实字体；分别让 HUD 图片及一张 Quest skin 经 Failed→Loading→Loaded，观察真实 Native 界面接管和 Q 开关，确认无重新登录、无新角色、无第二次 App boot。保留永久失败、损坏字体、重试次数／冷却、无关错误不被清除的反例；冷启及首步原门槛不变。恢复观察不替代原失败。
