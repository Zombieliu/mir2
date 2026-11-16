$ErrorActionPreference='Stop'
$taskRepo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$taskUtf8=[Text.UTF8Encoding]::new($false)
$taskQaRoot=Join-Path $taskRepo 'mir2-web3/docs/generated/slg-production-p02b-20261008'
$taskReview=Get-Content -LiteralPath (Join-Path $taskQaRoot 'independent-result-review.json') -Raw | ConvertFrom-Json
if($taskReview.accepted -ne $true -or $taskReview.testCounts.total -ne 177){throw 'Do not update completed progress before independently accepted actual results.'}
$taskHeader=@'
## 制作加工 P02b-1：购买、拆分和制作共用物品编号，限定回归通过（2026-10-08）

新增可信服务器配置绑定，把暂存 NPC 购买、背包/仓库拆分和个人制作接到同一持久 File UID 分配器；克隆只共享服务句柄，不复制发行游标。发行前抬高完整账号历史、当前实例及数字保管预留的下限，包含已经完全合并到旧堆、只剩购买回执的 incomingUniqueId；货架选择编号不当历史实例。拆分先成功分配再扣原数量，分配器缺失、损坏或耗尽时拒绝操作。存档不保存服务绑定，隔离重放不能经本批路径发行，错误替换或移除已绑定服务永久冻结新发行；原请求仍可查询已保存回执。

原串行 CargoGuard 实际 UID11＋制作18＋NPC35＋存档59＋背包37＋File分配器17＝177项通过，0失败/ignored；19项新唯一测试，来源05的455个声明输入在每次调用前后及最终重核匹配。早期编译失败和文件占用链接失败各一次（皆0测试）、拆分测试9通过/1失败、制作测试16通过/2失败、背包测试35通过/2失败的原记录完整保留；最后一组发现并修正 Legacy 裸World无配置时的实际崩溃，并补持久绑定配置缺失/不匹配的拒绝测试。之前通过记录也按最终来源重新运行，不叠加通过数；文件占用后仅迁移编译缓存目录，原Guard和测试条件保持，未关闭用户游戏。

这只完成 P02b-1 受控接线，不代表全服 UID 迁移或作坊可玩：默认配置和正常网关未启用新绑定，公共目录仍禁用。任务/脚本奖励、掉落、邮件、采矿/钓鱼、初始物品等发行者仍须迁移；外部世界与历史编号全量盘点、旧重复编号处理、Postgres单一权威和未知提交显式协调仍是上线条件。正常采集、种植、比奇/盟重作坊与侧栏界面未落地；未启游戏、重启网关、部署服务或更新安装包，不提升 parity 验收比例。详见[代码进度](SLG-PRODUCTION-IMPLEMENTATION.zh-CN.md)和[完整验证记录](generated/slg-production-p02b-20261008/README.md)。

'@
$taskPaths=@('mir2-web3/docs/AGENT-TASK-QUEUE.md','mir2-web3/docs/CRYSTAL-1TO1-ROADMAP.md','mir2-web3/docs/BACKEND-1TO1-PROGRESS.md','mir2-web3/docs/CRYSTAL-SERVER-PARITY.md')
$taskOldQueue='- [ ] P02b：全发行者统一 UID 权威、Postgres故障路径和未知提交的显式协调；正常联网入口尚未开放。'
$taskNewQueue=@'
- [x] P02b-1：可信 File UID 绑定与暂存 NPC 购买、背包/仓库拆分、制作的同一服务接线；19项新测试，最终177项回归通过，默认玩家入口仍未启用。
- [ ] P02b-2：迁移其余新物品发行者（通用 add/normalize、任务与脚本、GM、掉落、采矿/钓鱼、邮件/商城/宠物兑换、初始种子及 Hero）；完整物品保管返回保留原 UID 并拒绝冲突。
- [ ] P02b-3：独占维护窗口下盘点账号、世界、外部保管与全部历史回执，处理旧重复 UID；正常网关统一绑定服务，补 Postgres单一权威、故障路径和未知提交显式协调后才开放玩家入口。
'@
$taskOverrides=Join-Path $PSScriptRoot 'scoped-docs'
$null=New-Item -ItemType Directory -Path $taskOverrides
$taskRows=@()
foreach($taskPath in $taskPaths){
    $taskFull=Join-Path $taskRepo $taskPath
    $taskCurrent=[IO.File]::ReadAllText($taskFull).Replace("`r`n","`n")
    if($taskCurrent.Contains('## 制作加工 P02b-1：')){throw 'Do not duplicate progress entry.'}
    $taskAt=$taskCurrent.IndexOf("`n")
    $taskUpdated=$taskCurrent.Substring(0,$taskAt+1)+"`n"+$taskHeader+"`n"+$taskCurrent.Substring($taskAt+1).TrimStart("`n")
    $taskUpdated=$taskUpdated.Replace($taskOldQueue,$taskNewQueue)
    [IO.File]::WriteAllText($taskFull,$taskUpdated,$taskUtf8)
    # Publish only our additive progress on the actual production parent;
    # retain concurrent frontend work in the shared checkout.
    $taskBase=@(& git -c "safe.directory=$taskRepo" -c gc.auto=0 -C $taskRepo show "fde81a0de5084d02b3daaa30bac4d4f1fde411f4:$taskPath") -join "`n"
    if($LASTEXITCODE -ne 0){throw 'Cannot read production parent document.'}
    $taskBaseAt=$taskBase.IndexOf("`n")
    $taskScoped=$taskBase.Substring(0,$taskBaseAt+1)+"`n"+$taskHeader+"`n"+$taskBase.Substring($taskBaseAt+1).TrimStart("`n")+"`n"
    $taskScoped=$taskScoped.Replace($taskOldQueue,$taskNewQueue)
    $taskScopedPath=Join-Path $taskOverrides ([IO.Path]::GetFileName($taskPath))
    [IO.File]::WriteAllText($taskScopedPath,$taskScoped,$taskUtf8)
    $taskRows+=[ordered]@{path=$taskPath;scopedPath=$taskScopedPath;sha256=(Get-FileHash -LiteralPath $taskScopedPath -Algorithm SHA256).Hash.ToLowerInvariant();preservedForeignCheckoutContent=$true}
}
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'scoped-docs.json'),($taskRows | ConvertTo-Json -Depth 5),$taskUtf8)
