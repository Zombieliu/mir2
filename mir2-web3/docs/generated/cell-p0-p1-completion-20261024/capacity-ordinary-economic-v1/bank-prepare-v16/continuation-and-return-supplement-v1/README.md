# W16 结果独审、两组续跑预检与返药商离线诊断

[独立公共结果审查](public-continuation-review-v1.json)核实本轮16人正常保存，与历史42人组合的58个位置互异。逐人售卖、购买证明与report一致：共购176药、花7040金币，34笔鹿肉售卖共8199金币。failed59仅售2肉得521金币，未买药；不据此推断其当前私库最终余额或库存。

[Root实际纯预检](actual-two-group-history-receipt-v1.json)调用当前production validator，以W15原42人和W16新增16人两组独立四pin输入，得到history58、remaining42、原59～100号。漏掉第一组、倒序、重复、以combined58索引代替current16索引，四个负例均被拒绝。未执行auth、角色库复制或普通重登；所有58人均为下轮historical/notFresh。

[离线重放](return-geometry-result-v1.json)使用实际map0、69个transfer、终局全部11个记忆位置（6closed、5live）、16个occupancy，重建1577个explicit禁点，前128个顺序与原诊断一致，1662个动态禁点复现原NoPath。仅投影closed记录按原TTL自然到期，保持5live、occupancy及transfer不变：7.325秒和7.918秒仍无路，8.539秒后出现28步路线，所有closed原期13.697秒结束后仍为28步。

这是离线因果证据，不保证后续真实AOI不变。未来只在售后前往Ruben的精确导航范围复用原自然到期等待；不清空缓存、续期、降低安全距离、忽略占格或重置命令/时间预算。原W16失败不追认为通过。

六个匿名公共原件由[清单](copied-evidence-manifest.json)逐字节绑定，原W16 46件归档未修改；这里没有私有Store、pool、账本、密钥或完整录制。
