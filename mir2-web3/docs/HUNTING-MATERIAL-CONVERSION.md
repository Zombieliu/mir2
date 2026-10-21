# 材料绑定转可交易：服务端合同

固定估值采用用户已确认的规则。总估值 E = 策划单价 × 数量；扣 E 许可原子（1,000,000 原子 = 1 单位许可），另扣 ceil(2% × E) 现有非绑定金币。材料数量不因手续费减少，不生成金币、不扣绑定金币，只有手续费进入日常销毁池。

## 配置和物品范围

`config/hunting-material-valuations.json` 是严格版本化策划文件模板，正式 entries 为空，未知 item_index 拒绝。权威报价读取 PG 中已经注册并激活的不可变估值版本；仅修改 JSON 文件不会自动生效。可信管理路径使用 `register_material_valuation`、`activate_material_valuation`，激活 revision 防止版本切换后切回的 ABA。未新增客户端估值写接口，正式价格和管理发布流程仍待落地。

首个支持范围是当前角色普通 Bag 中有完整、同角色代际 Bound 来源的根材料；原始 Crystal 交易/租借/灵魂/封印限制仍执行。不接受装备、嵌套占用载体、冲突 UID、非 canonical 或别名重叠 Bag 格、旧无来源物品和已 Converted 物品。整栈保留 UID/格；部分转换保留原绑定余数，输出到独立空 Bag 格并分配正数新 UID。输出完整实例持有 Converted 来源，同 realm 的跨角色完整 custody 保留来源；不支持的有损变形仍拒绝。

## 只读报价伪代码

```text
Quote(当前物理 owner, 客户端 UID, 数量):
  主服核已认证账号、当前角色/代际、真正本地权威和完整 StartGame scope
  捕获真实 World 的完整库存前像、不可伪造变更见证和当前保存版本
  PG 开事务，取得该 realm 经济锁，锁后读取数据库时钟
  再核当前账号/角色授权，读取活动 policy 与活动估值 revision
  从可信前像校验目标 Bound 根、数量、canonical Bag 格、所有 UID
  校验价格白名单；checked 计算 E、ceil 手续费和许可原子
  以服务端随机命名空间保存不可变报价，TTL = 30 秒
  提交并返回服务端价格、费用、数量、版本、原 quote_id
  // 不扣金币、许可或材料；客户端不传价格/owner/checkpoint
```

## 实时确认与恢复伪代码

```text
Confirm(当前物理 owner, 原 quote_id):
  主服核原 actor、完整来源前像及变更见证；留存原请求私有 Pending
  PG 开事务，取得 realm 经济锁，锁后读数据库时钟
  先核当前账号/角色授权
  若原 quote 已有不可变提交结果：直接返回原结果，不再次执行来源
  核 quote 所有者、TTL、活动政策/估值版本及 revision、日结已追赶
  核现有非绑定钱包 >= fee、全局许可库存 >= E
  在同一 PG 事务比较并更新完整角色 checkpoint/CAS 与必要 Guild 源
  非绑定钱包 -= fee；许可库存 -= E；绑定钱包及旧 gold 不变
  插入 fee 日常销毁、材料许可消耗账本、不可变完整结果及 outbox
  一次提交全部写入
  仅原 World 前像/见证/代际仍匹配时发布转换库存；否则保留凭证并要求重登

Query(当前物理 owner, 原 quote_id):
  重新核当前账号/角色授权，只读取原结果；不发起转换
  已提交：返回不可变原回执；主服仅在安全原前像上发布一次
  已证实未提交：结束原 Pending；必须显式重新 Quote，不能自动重 Confirm
  结果仍未知：保留原 Pending，只允许继续原 Query

普通 LogOut/Disconnect/下一次 Login:
  先核本角色未决订单与私有冻结凭证，再刷新当前 canonical 状态
  退休 World 不得继续保存；不得用旧库存覆盖已提交完整 checkpoint
```

## 日结、导出与观测

材料 E 累加许可消耗，fee 计入日常销毁；E 不计金币销毁或可交易金币流入。金币转换原有 gross/net 保持金币语义。日结回执新增两个可省略零字段，历史摘要仍按原始 payload 校验；导出 consumer v2 保留已存在 v1 ACK。只读视图保留原 41 列顺序并追加材料估值、材料费、金币转换费，只有可信完整证据满足时才按旧历史缺省零，否则保持 NULL。

业务控制均在主服/PG，Grafana 只读。新钱包、市场、画像和收益覆盖资格不因材料接线而提升。默认关闭、K=0.1、基准每日0.72/上限1.08许可保持。此合同不宣称 Windows EXE/GUI、默认异步保存竞争、全服所有生产来源或发布资格已验收。
