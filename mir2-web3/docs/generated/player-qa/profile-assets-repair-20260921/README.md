# 当前内容资源补全 — 2026-09-21

## 已完成

- 当前platinum_176的174种怪物、新手BoneFamiliar，以及54件武器/衣服实际shape：
  139库、60,556个可绘制源帧，完整男女帧；原版RGBA、mask、尺寸、锚点、哈希验证通过。
- Windows在starter atlas之外可加载规范Monster/NNN和Gate/00..03路径，沿用人物
  帧的metadata和文件校验，保留逐帧按需加载，未把全部纹理装入常驻大图集。
- 服务端950..953改为原版Gate/00..03映射，动作表由各库提供，帧基址仍为0。
- 全量复审中当前profile174及新手11种的有效源帧public/native缺失为0、unknown为0。
- 原生atlas回归32/32、Gate映射测试2/2、导出器空帧回归及原版库测试通过。
  客户端与Gateway release编译通过。

## 来源例外及未通过项

- Gate/00原版29个零尺寸槽不导出PNG，不伪造图；可绘制帧数分母不含这些槽。
- RootSpider原版只使用0..2方向，方向3..7的描述越界不是可达动作；当前Rust
  初始化也只取前三方向。Sheep采集后Skeleton方向1..7在原版越界并跳过绘制，
  不夹到224帧来冒充原版正常绘制。
- 原始全目录555怪物仍有230定义未完成有效帧闭包，这些不属于本轮当前profile。
  Dragon等未用特殊映射、动态装备职业/等级变体、实际画面和帧率待各自验证。
- 所有Items/StateItem图标此前已通过；34个原版空地面物品帧策略不在本轮伪造。
- 本轮没有角色登录、实机动作/穿戴/遮挡截图，不声称视觉或整个游戏100%。

## 包与切换

本地测试包 `C:/numeron-legend-of-rebirth-20260921-profile-assets`。
客户端SHA256：`F78E7934F3AF3B9B0B56BBD9E6AA63E427C3B0DC97D0A4179A62D366A9AFD82B`。
Gateway SHA256：`A64DAFF28751031BB038B39BF4659F31EBFFB6B6DB71958110762B2C72E7422C`。
mir2-assets仍用仓库public junction；配置保留19910/newcomer-v2。
旧客户端仍在运行，已请求正常退出以保存a1后再切换；没有强杀或改存档。

## 复查

- [源像素和全库报告](../profile-actor-assets-20260921/README.md)
- [有效源帧逐动作方向复审](monster-audit.md)
- [复审JSON](monster-audit.json)
- 旧全量缺口基线保留在all-assets-20260921和all-monster-assets-20260921。
  新审计脚本使用新输出目录，不覆盖旧基线。
