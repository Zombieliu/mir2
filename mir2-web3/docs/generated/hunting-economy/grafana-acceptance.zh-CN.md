# Grafana原生配置与只读查询验收

2026-10-19，在单独创建的loopback Grafana OSS 13.2.4实例上完成原生provisioning和实际PostgreSQL查询验收。没有更改已有Grafana或远程服务，没有发送真实通知。验收数据库只包含本地独立测试数据，不能代表真实玩家、全服资产覆盖或经济参数已校准。

| 检查 | 实际结果 | 证据 |
| --- | --- | --- |
| 官方Windows包 | 466,144,621字节；官方SHA256匹配；TLS证书验证未关闭 | grafana-download.json |
| 原生配置导入 | 14个panel、5条alert规则、标准webhook contact point；SQL原文保留 | grafana-schema-validation.json |
| PostgreSQL数据源 | 专用只读角色；verify-full；file-path公开CA；真实health为OK | grafana-panel-query-validation.json |
| 图表SQL | 13个数据panel查询全部实际执行 | grafana-native-query-validation.json |
| 原生告警查询及表达式 | 29个合成realm，57项SQL值、57项原生reduce/threshold值符合期望 | grafana-native-query-validation.json |
| 缺失语义 | 其中17项明确NULL期望，原生表达式保持NULL，没有转成正常0 | grafana-native-query-validation.json |
| 独立离线回归 | 33个case、72项模型断言、221项artifact检查通过 | infra/economy-observability/validation-result.json |

实际执行发现并修复了一个离线形状验证未暴露的问题：原生导入接受 `settings.mode=strict`，表达式执行却返回HTTP400。五个reduce模型改为省略settings，使用原生strict默认行为。没有改SQL、业务阈值、完整日数或NoData策略。重新导入后，从Grafana API取出实际持久的模型链执行57+57断言通过；没有用仅在内存中修好的请求代替持久配置验收。原始失败与修复记录在 `grafana-compatibility-history.json`。

合成数据由脚本生成可审查SQL，再由集成人在本地隔离库提交。realm前缀为 `hunting-observer-test-grafana-20261019-native1-`，只append自己的独立realm，未修改既有不可变历史。实际提交SQL的SHA256为 `bc5cc0e00770aa21a0775b1fd3f72687927728a211fb0ce0d627686954b32f1a`，与 `grafana-fixture-applied.json` 和查询证据一致。fixture完整覆盖标记只用于该合成namespace的阈值试验，不能回流成真实服覆盖资格或游戏许可额度。

所有日期由当前数据库now和北京时间04:00推导。案例名称包含leap day并不代表本次十月窗口实际跨2月29日，本次仅证明当前完整经济日及缺日/NULL语义。月卡价格、材料价格和收益缺来源时仍显示NULL。本轮未验证钉钉/企业微信真实消息接收、恢复通知或生产发布。

## 原生浏览器目标面板抽查

主线程在正常原生IAB的窄视口中，选择合成经济服 `hunting-observer-test-grafana-20261019-native1-steady_complete`，完成下列实际渲染检查。该视口宽446像素，四张局部截图为446×636；顶部入口截图请求过 `fullPage=true`，保存图像为446×4735。该选项及嵌套滚动区域的截图不构成全部14个面板视觉、布局或全页覆盖验收，本轮只声明表中列出的可见内容。

| 截图 | 已看到的内容 |
| --- | --- |
| grafana-dashboard-steady-complete.jpg | 合成服选择入口、经济口径、日常保底/轻PVP/沙巴克三层销毁堆叠 |
| grafana-dashboard-permit.jpg | 许可新增/消耗/衰减图例，以及全局许可日结库存与计划每日需求图 |
| grafana-dashboard-players.jpg | 合成休闲生产6、轻度PVP3、硬核沙巴克1的60%/30%/10%饼图与图例 |
| grafana-dashboard-health.jpg | 该合成服经济数据完整性绿色值0 |
| grafana-dashboard-income-null.jpg | 收益来源未接入时保留空曲线；结合SQL中的NULL，不解释为实测收益为0 |

五张JPG的实际字节大小、SHA256、尺寸及有限验收范围记录在 `grafana-browser-validation.json`。这里不声明真实月卡成交数据、所有14个面板的视觉/布局、其他分辨率或真实玩家经济已验收。

浏览器检查后已用原生helper关闭自有实例，`grafana-shutdown.json`确认只停止13个自有backend子进程及server，`stopped=true`，私有运行证据保留。未发送外部通知。用户明确禁止Docker；此次浏览器验收使用原生Windows Grafana和PostgreSQL，后续不启动Docker、WSL或虚拟化环境。

可复跑入口为 `infra/economy-observability/tests/grafana_native_acceptance.py`。它只接受自有目录标记、loopback服务与 `mir2_hunting_acceptance_` 数据库，私有凭据不进入仓库或标准输出。`fixture-sql`仅生成SQL；`query`只读数据库。`verify`验证原生导入，`query`验证实际SQL和原生表达式，二者不能混称。

官方依据：[Windows下载与校验](https://grafana.com/grafana/download?edition=oss&platform=windows)、[PostgreSQL TLS原生配置](https://grafana.com/docs/grafana/latest/datasources/postgres/configure/#provisioning-with-tls)、[原生告警文件provisioning](https://grafana.com/docs/grafana/latest/alerting/set-up/provision-alerting-resources/file-provisioning/)、[13.2.4默认配置](https://github.com/grafana/grafana/blob/v13.2.4/conf/defaults.ini)。
