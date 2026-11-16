# Edge 右键手势与原生菜单反馈

2026-11-13 玩家提供两张桌面浏览器照片：游戏中右键拖动出现蓝线及“无效手势”；另一张在地图内出现浏览器图片右键菜单。没有把这些照片记为本站自动验收通过，也没有复制玩家桌面照片或修改其浏览器设置。

第一张符合 Edge 自带鼠标手势：按住右键绘制轨迹执行浏览器操作。[Microsoft 官方策略说明](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-policies/MouseGestureEnabled) 第二张为浏览器原生菜单。两者会干扰经典右键移动；仍需确认用户用的是内置手势还是扩展，并在对应 Edge 环境复现。

只读检查提交 `d99212e3cdde834b6eb751dd36e9a6361bc2d100`：

- Shell 1475–1494 已在 window capture 拦截 frame 内 `contextmenu` 和右键 `mousedown`；viewport 还有 React 捕获/冒泡拦截。不能描述为完全没写 `preventDefault`。
- 两个 portal 均指向连接的 `.client-stage-frame`；常驻图片替换在 frame 内 span；两个 Bevy canvas 也在 frame，overflow 被裁切。尚未找到已证实的地图图片或 canvas 逃出拦截范围。
- 已证实的范围边界是 `target instanceof Element && target.closest('.client-stage-frame')`：body、黑边、非 Element 或真正外部浮层不会进入该分支；没有按下来源锁存和坐标回退。但这些是待复现场景，不能直接归因为照片里的地图菜单。
- 浏览器级手势不等同于 DOM 默认菜单。网页不能承诺用普通事件处理强制关闭所有浏览器/扩展的手势设置。

下一轮记录同款 Edge 一次真实右键单击、按住拖动、移出 frame 释放和资源交接时的 trusted/cancelable/defaultPrevented、target/composedPath、坐标与 frameRect，先确认事件是否派发及取消，再决定最小修复。Chrome 自动流程不能代替 Edge 原生手势验收。

不要盲目在 `pointerdown` 全部 `preventDefault`：经典鼠标世界动作仍从 `mousedown` 发起，pointerdown 的世界分支主要服务触摸；阻止兼容鼠标事件可能反而切断右键移动。此次只有源码调查和队列更新，没有产品补丁、Edge 实测或已解决声明。
