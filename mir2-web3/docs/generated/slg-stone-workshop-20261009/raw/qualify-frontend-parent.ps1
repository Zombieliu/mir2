$ErrorActionPreference='Stop'
$stoneRepo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$stoneProduction='07c7a55b2d834f097ead83e6f8f600ba82e4d683'
$stoneFrontend='12880592d68a498921c25b8bc8d4ebdebc909ed1'
$stoneSnapshot=Get-Content (Join-Path $PSScriptRoot 'stone-native-tests-01/source-snapshot.json') -Raw|ConvertFrom-Json
$stoneAuthored=Get-Content (Join-Path $PSScriptRoot 'frontend-authored-source-01.json') -Raw|ConvertFrom-Json
$stoneProtected=@($stoneSnapshot.files.path|Where-Object {$_ -notin $stoneAuthored.files.path})
function Read-StoneTree([string]$Ref){
    $stoneRows=@(& git -c gc.auto=0 -C $stoneRepo ls-tree -r --full-tree $Ref)
    if($LASTEXITCODE -ne 0){throw 'Cannot inspect dependency parent'}
    $stoneMap=@{}
    foreach($stoneRow in $stoneRows){if($stoneRow -match '^\d+ blob ([0-9a-f]+)\t(.+)$'){$stoneMap[$Matches[2]]=$Matches[1]}}
    return $stoneMap
}
$stoneProdTree=Read-StoneTree $stoneProduction
$stoneFrontTree=Read-StoneTree $stoneFrontend
$stoneCurrent=@($stoneProtected|& git -c gc.auto=0 -C $stoneRepo hash-object --stdin-paths)
if($LASTEXITCODE -ne 0 -or $stoneCurrent.Count -ne $stoneProtected.Count){throw 'Cannot qualify protected files'}
$stoneIncoming=@();$stoneMissing=@()
for($stoneI=0;$stoneI -lt $stoneProtected.Count;$stoneI++){
    $stonePath=$stoneProtected[$stoneI];$stoneBlob=$stoneCurrent[$stoneI]
    if($stoneProdTree[$stonePath] -eq $stoneBlob){continue}
    if($stoneFrontTree[$stonePath] -eq $stoneBlob){$stoneIncoming+=@{path=$stonePath;blob=$stoneBlob;source=$stoneFrontend};continue}
    $stoneMissing+=@{path=$stonePath;current=$stoneBlob;production=$stoneProdTree[$stonePath];frontend=$stoneFrontTree[$stonePath]}
}
$stoneOut=[ordered]@{schema='mir2.stone-frontend-protected.v1';productionParent=$stoneProduction;frontendParent=$stoneFrontend;
    declared=$stoneSnapshot.files.Count;authored=$stoneAuthored.files.Count;protected=$stoneProtected.Count;
    requiredCommittedFrontendDependencies=$stoneIncoming;unmatchedCount=$stoneMissing.Count;unmatched=$stoneMissing;
    method='Canonical Git clean blobs; unchanged production parent plus explicit committed frontend dependency carry';createdUtc=[DateTime]::UtcNow.ToString('o')}
$stonePath=Join-Path $PSScriptRoot 'frontend-protected-parent-qualification-01.json'
if(Test-Path $stonePath){throw 'Preserve qualification history'}
[IO.File]::WriteAllText($stonePath,($stoneOut|ConvertTo-Json -Depth 7),[Text.UTF8Encoding]::new($false))
$stoneOut|ConvertTo-Json -Depth 7
if($stoneMissing.Count -gt 0){exit 1}
