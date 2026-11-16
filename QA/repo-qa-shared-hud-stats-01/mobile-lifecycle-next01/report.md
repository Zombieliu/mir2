# M9生命周期只读准备

`original-client-mobile-controls.tsx:233-247,272-291` 已监听blur/pagehide/visibilitychange并释放摇杆；`page.tsx:8786-8788` stop只清待发方向。page无连接生命周期/online/offline监听。WS close `page.tsx:6387-6415` 已接受控重连 `:6266-6307`；关闭清Storage待办且禁止重放 `:4943-4945`。

`apps/mir2-mobile/package.json:16-22` 仅Core/Android/iOS/CLI，无App/Network插件；无node_modules/生成清单，native API未证实。先复用DOM事件并设备验证；若后台不派发DOM生命周期事件，再加App插件。Network插件是否必要待真机网络事件证据，勿凭API名判断。

最小范围：`apps/web/app/page.tsx` 增前后台/在线门控，复用现有单次受控恢复；移动控件沿用现有退场。回归：后台释放摇杆、恢复需新手势；断网只重连一次；Storage不重放。Android/iOS真机测后台/恢复/断网；桌面触屏不替代。

只读；未构建、测试或操作GUI。
