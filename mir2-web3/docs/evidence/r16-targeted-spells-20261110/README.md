# R16 大火球与灵魂火符实战

使用各自隔离的 30 级毕业档和普通 Web UI，在 F11 绑定技能，绕过城镇传送点后以真实野怪对象施法。固定基线的大火球已命中普通鸡。旧灵魂火符对同一只活着的中立野怪提交八次普通请求，均无施放回包，43 张护身符未动。 [旧大火球](wizard-field-r2/receipt.json) · [旧灵魂火符](taoist-field-r2/receipt.json)

修复版让扣符前检查与共享 Zone 施法共用野怪目标规则。活的中立野怪可被攻击。召唤物仍按攻击模式和安全区检查，过期坐标与越距请求仍会拒绝。 [源码构建范围](gateway-soulfire-build.json) · [原失败](zone-soulfire-red-r4.log) · [回归](zone-soulfire-green.log)

| 技能 | 七格命中 | 资源变化 | 普通重登及 File 读回 |
| --- | --- | --- | --- |
| 法师大火球 | 5 伤害并击杀 | MP 541 → 536 | F11 绑定保留，熟练度 501 → 503 |
| 道士灵魂火符 | 5 伤害并击杀 | MP 260 → 257，护身符 43 → 42，仅一次扣符包 | F11 绑定保留，熟练度 470 → 471，护身符 42 |

道士先以和平模式点击自己召唤的骷髅，收到目标请求但无施放或扣符，43 张护身符保持。两职业共四次正常注销，原档哈希保持。五组 Rust 回归共 88 项。 [法师](wizard-zone-candidate-r1/receipt.json) · [道士](taoist-zone-candidate-r1/receipt.json) · [总表](qualified-targeted-summary.json)

本次七格浏览器施放到命中为 8／11 毫秒，固定 Crystal `92b4ce4` 按 `500 + 50 × 距离` 为 850 毫秒。之后的回合已验证共享服务器延时和实际命中。Windows 窗口与公开部署另验。 [延时复验](../r16-delayed-projectiles-20261110/README.md)
