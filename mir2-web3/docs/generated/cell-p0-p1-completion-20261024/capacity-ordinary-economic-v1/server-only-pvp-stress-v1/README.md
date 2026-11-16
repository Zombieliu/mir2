# Server-only PvP stress v1：10人档命中证据延迟失败

本目录只保存[显式白名单及SHA256](copied-evidence-manifest.json)所列的16份公开原件，源前后与副本逐字节SHA核对。该profile为离线预置、静止分散、全员战斗PvP，不计ordinary经济/P0验收。

[实际报告](reports/1793124648375-f15d2de5.pvp-stress-report.json)：2 fighters完整300.096秒通过，429 launched、254 positive；10 fighters完整300.102秒失败，2216 launched、1404 positive。唯一失败为第2个60秒窗口actorIndex5的positiveHitLatency p95=1683ms，超过该profile的1000ms门槛；同窗ACK p95=422ms、44 launched、29 positive，无攻击频率、timeout或witness缺失失败。[原samples](reports/1793124648375-f15d2de5.pvp-stress-samples.jsonl)保留，不改原false。

延迟来自server还是collector尚待独立审查；不能据本轮说机器只能支撑2人。25/50/100/250/500、两小时、移动PvP及热点未测。

[原资源profile](resource-profile-v1.json)、[monitor历史](monitor-history.jsonl)与[terminal monitor](monitor-current.json)保留实际资源指标，本轮resourceStop=false。[health-after](health-after-v1.json)六项capacity当前计数为0；[cleanup](cleanup-v1.json)记录gateway/monitor正常exit0、driver exit1、closed=true及监听端口为空。

[run intent](run-intent-v1.json)绑定[公开fixture manifest](fixture/fixture-manifest-v1.json)。无单独公开preparation receipt，未伪造。后端[原validation及release构建收据](backend-v2/validation-v1.json)内含7项load-test-login-policy、12项adjacent-auth测试和gateway-release构建结果；[测试stdout1](backend-v2/load-test-login-policy.stdout.log)、[测试stdout2](backend-v2/adjacent-auth.stdout.log)、[release stdout](backend-v2/gateway-release.stdout.log)与[release stderr](backend-v2/gateway-release.stderr.log)原样保存。

未归档私有Store/pool/账本/keys、完整录制、私有Gateway日志或EXE；不改normal stats/cooldown/healing，也不宣称容量或P0通过。
