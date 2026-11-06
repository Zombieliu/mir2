# Progress64 · Native 普通 NPC 商店共享控件

本批完成普通 Gold / unlimited / panel0 商店的共同 Rust controller 和 Bevy painter，并接入 Native 实际键盘、按钮和绘制入口。当前仅接受最终 Source03 的源码与有限检查；portable/runtime/Web host 尚未接通，没有操作界面、启动客户端、renderer Instance、HTTP/server、浏览器或移动设备。整体 goal 保持 active。

## 改动与边界

- 共同 controller 复用既有购买 planner，仅保存 selection、quantity、page 与显示版本；完整 Shop/Inventory 和 Core 观察版本变化会退役旧控件。控件须同时证明当前 Gate 实例和 source/presentation stamp，缺失或旧证明不能取消当前请求。
- Native 数量快捷键与共同按钮先撤销尚未进入传输的购买，再修改本地数量。Bound readiness waiter 同步唤醒；已进入传输的 Entered / Flushed / Unknown 屏障保留。Native producer 每帧观察同一实际来源，支持取消后的下一次手动购买；不会自动重试或声称购买成功。
- painter 保留原 244×334 框、八行 205×32、单行滚动、原资源键、完整 tooltip / hideAddedStats / 新物品标记。共同控件的 disabled 状态不携带可执行 action；Gold 与总价来自同一当前 Inventory/plan。
- 仅完整非空目录全部满足普通范围才使用共同树。离页的 Pearl、有限库存、resale/nonpanel0 或 legacy 条目使整目录保留旧入口，不过滤；标记普通但缺 raw 来源的条目仍严格禁买，不降级绕过 planner。

## 实际有限检查

全部七组在最终 Source03 实际执行；没有承接旧 Source02 的成功结果。

| 配置与筛选 | 实际通过 |
| --- | ---: |
| Native npc_shop_ui | 20 |
| Native shop_paint | 10 |
| Native gold_buy | 5 |
| Native npc_dialog | 7 |
| Native chat | 29 |
| portable npc_shop_ui | 7 |
| portable shop_paint | 10 |
| 跨配置执行合计 | 88 |

合计是测试执行次数，不能当作 88 个独立场景。新增30项 authored：共同 controller7、Native系统13、实际 painter10。布局验证使用真实 UiPlugin / ComputedNode / UiGlobalTransform / CalculatedClip 和实际打包字体，在内存中运行，无 Window 或 renderer。

每组均 locked/offline/jobs1/inc0/64MiB/testThreads1，单 Cargo 写者；独立 .NET Guard 新鲜实际 Get-Volume C ≥ 50GiB，Policy B completed/exited/disposed 齐全，child 与外层真实 exit/close0。声明 Rust808 与字体/runner 前后匹配。九次已关闭检查的27个已知 Guard/callee/probe PID 经实际单次查询均 absent；这是查询时点证据，未枚举或操作旧 Native/launcher。

## 保留的失败与修复

Source02 Native20通过、painter9通过1失败均保留且不计入上述最终选择。唯一失败是新增1.5倍缩放测试误期待 ComputedNode307.5px；Bevy0.19默认将物理像素布局取整为308px。Source03仅在这一新测试块增加未取整设计宽度断言，并将实际宽度期待改为 round；原 near0.1、scale1几何、框尺寸、字体、高度及 tooltip 断言不变。全文件 before 备份与精确逆替相等，无生产绘制尺寸修改。

## 输入与证据

最终 Source03 为543源码/配置输入加1个实际公开字体＝544，Rust807源码/配置输入加该字体＝808，两图 union970。相对前批63仅4既有授权路径变更、4新增Rust源码；535组合与799Rust原成员受保护，完整4份before匹配。源码冻结记录不等于整个操作系统或依赖缓存的完整镜像。

固定QA目录：C:/mir2-cross-platform-storage-20261002/repo-qa-shared-npc-shop-ui-01。

- root-source-snapshot03.json：167648B / 80f1a6d4dbf79e23a416302926a11f4adb4879b0e2b780f777848650ed2356e9。
- rust-snapshot03.json：188681B / 34173bb41fb3f71737f18483f0b993962ee11862d8907adcd6bcb84b17906cd2。
- root-source-audit01.json：7422B / dd8b086a578ab8db9880d7ed0f96512f808e60af059657a1883eb262ffd60278。
- root-code-verification01.json：24753B / 1f9183e92acd12b5fee7ec318c6dfb5bdfe0910d3086a94b2982d1df3ef9a653。
- owned-process-closure01.json：934B / a1436c5ecc43cc7f3bbb591e07de7114b8ad0e9fc0651aa0e93fad32295f3780。
- 原始结果/日志、每次Guard before/after/probe、修复完整before和issued configs均留在该目录。独立M14源码与准入审查blocker0；最终实际回执/文档审查另记录在source acceptance。

## 后续仍开放

下一轮先新增 portable/runtime 普通商店 host，复用本批 controller/painter，再接 Web prepare→arming→active 唯一控件树和现有持久 Core dispatcher。Core authorityRevision 必须保持规范十进制字符串，Rust严格转u64，不经JS Number，也不新增购买slot或producer。

Sell/Repair/BUYBACK/USED、无专用购买ACK的结果恢复、服务端容量预检与元数据插入一致性仍开放。本批未做生产 EXE/renderer/Core/Next/独立包重建或发布；前批产物不能代替本批源码的组合验收。实际登录/战斗/保存重登、原生资产显示与指针命中、浏览器/触屏、Android/iOS真机生命周期、最终人类 frontend/Candidate 与整体 goal 尚未验收。持续遵守用户“继续代码，暂不操作界面”。
