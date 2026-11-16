# R16 走跑后的施法姿势

对照基点为分支 `codex/windows-player-journey` 的 `8fa80d83ce`。Windows 在新走跑段仍有效时用最新动作替换过时的走跑与受击帧，已有施法、攻击或死亡保持队列顺序。Web 复用共享动画规则。远端角色需要匹配的移动字段，本机也可使用当前显示位置对应的有效位移段。原服务器 `UserLocation` 回执仍按位置和朝向处理。 [Windows 动作桥](../../../apps/game-client/platform-windows/src/entity_presentation.rs) · [Web 动作桥](../../../apps/game-client/platform-web/src/entity_animation_state.rs)

## 本机生产构建复测

使用普通登录、File 存档副本、比奇路面走跑、F11 绑键和 Ctrl+F1。道士在七格外对天然野怪施放一次灵魂火符。浏览器记录实际送入轻量 WASM 的动作和队列、身体图、服务端包与重登。探针只读取一次正常解析的输入与输出。

| 生产浏览器对照 | 首次收到施法时的身体动作 | 该次动作后的队列深度 | 首次采到已加载施法身体图 |
| --- | --- | ---: | --- |
| 旧版连续走跑 | 走路 | 4 | 施法窗口内未出现 |
| 旧版等到队列归零 | 施法 | 0 | `ObjectMagic` 后 296 毫秒 |
| 新渲染包与旧轻量呈现包 | 跑步 | 7 | 施法窗口内未出现 |
| 同源新包 WebGL2-shared | 施法 | 0 | `ObjectMagic` 后 246 毫秒 |
| 同源新包默认 WebGPU | 施法 | 0 | `ObjectMagic` 后 241 毫秒 |

两场新版各收到 18 段走跑确认。移动期间的队列峰值为 0，施法动作各开始一次，身体首帧为 `CArmour/05/326.png`。服务端各发一次 5 点伤害回执，页面浮字随 `DamageIndicator` 显示。正不透明度的 DOM 首次采样分别在回执后 255 和 395 毫秒，WebGPU 命中画面实拍可读到 5。两场符纸 43→42，魔法值 260→257，熟练度和快捷键均经正常注销与新会话读回。 [浏览器收据](browser-verification.json) · [施法首帧](webgl2-shared-caster-spell.jpg) · [命中浮字](webgpu-impact-number.jpg)

## 构建和开放项

三套 Bevy 包 `bevy-7c0a3b5dc8a548d9` 的直接与版本化文件完成 12 次散列核验。页面实际请求独立的轻量呈现 JS 和 WASM，两个浏览器均收到匹配构建的文件。轻量呈现首次构建为 263,179 字节，超过 262,144 字节的严格门槛。固定 Binaryen 131 增加函数重排后为 261,300 字节，ABI、体积、版本化文件及源码指纹通过。Rust 平台 94 项、共享动画 28 项、Web 动作和输入 70 项、轻量包回归 216 项、优化器 41 项、Stage5 适配器 508 组及 TypeScript 检查通过。 [渲染包](runtime-verification.json) · [轻量包及回归](client-core-verification.json)

另一次对照中，施法结束后普通走到相邻格，服务器确认位移，页面仍复用旧施法标识并显示站姿。按位置永久清理标识的试验会影响钓鱼动作证明，后续需分别校验动画与权威回执。正式发行需同步三套渲染包，Windows 窗口实机、完整 Hosted 链路和最终真人验收另行推进。
