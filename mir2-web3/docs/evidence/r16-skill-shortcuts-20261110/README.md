# R16 Web 三职业技能快捷键

旧 Web 战士用普通 F11、F1 和 Ctrl+F1 实测，`SpellToggle` 确认开启后，紧随其后的 `worldSnapshot` 省略开关字段。Web 随后又发送开启，半月弯刀无法关闭。旧版也接受约 243 毫秒内的跨键位切换。 [旧版回执](warrior-baseline-r1/receipt.json)

Web 只在同一角色会话、同一学会技能且新快照确实省略开关时保留服务端回包状态。已发送或结果未知的持久武技切换占用 R16 共用的 1 秒间隔。实际使用原有 Bevy WASM，战士成功发出半月开、刺杀开、半月关、半月再开，均有本人回包。两个过快输入不发包，新登录的同角色可正常开启。 [战士回执](warrior-candidate-r4/receipt.json) · [实际画面](warrior-candidate-r4/two-bound-bars.png)

三职业 30 级毕业档各经游戏 F11 把技能绑定到第 1 和第 9 位。共六次普通注销，重登和 File 落盘读回准确，浏览器零错误，原夹具哈希一致。法师的火墙、道士的召唤骷髅和对自己治疗均有权威施放包。法师大火球已发出带目标请求，该次缺施放回包。道士首轮灵魂火符未发出，第二栏使用治疗术复验。 [法师](wizard-candidate-r3/receipt.json) · [道士](taoist-candidate-r2/receipt.json)

107 项 Web 回归与 TypeScript 通过。此前失败的两个测试夹具已对照当前生产路径修正，分别覆盖交易确认的最终送出顺序和公会键盘判定。首轮提前关闭技能页和首次重登立即按键的失败均保留。原生窗口实战、武器准备技和追击冷却继续验收。 [三职业汇总、源指纹和保留失败](qualified-summary.json)
