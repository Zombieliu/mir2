$ErrorActionPreference='Stop'
$stoneRepo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$stoneParent='07c7a55b2d834f097ead83e6f8f600ba82e4d683'
$stoneSnapshot=Get-Content (Join-Path $PSScriptRoot 'stone-session-tests-03/source-snapshot.json') -Raw|ConvertFrom-Json
$stoneAuthored=Get-Content (Join-Path $PSScriptRoot 'stone-runtime-authored-source-07.json') -Raw|ConvertFrom-Json
$stoneProtected=@($stoneSnapshot.files.path|Where-Object {$_ -notin $stoneAuthored.files.path})
$stoneTree=@(& git -c gc.auto=0 -C $stoneRepo ls-tree -r --full-tree $stoneParent)
if($LASTEXITCODE -ne 0){throw 'Cannot inspect parent'}
$stoneBlobs=@{}
foreach($stoneRow in $stoneTree){if($stoneRow -match '^\d+ blob ([0-9a-f]+)\t(.+)$'){$stoneBlobs[$Matches[2]]=$Matches[1]}}
$stoneCurrent=@($stoneProtected|& git -c gc.auto=0 -C $stoneRepo hash-object --stdin-paths)
if($LASTEXITCODE -ne 0 -or $stoneCurrent.Count -ne $stoneProtected.Count){throw 'Cannot qualify protected inputs'}
$stoneMismatch=@()
for($stoneI=0;$stoneI -lt $stoneProtected.Count;$stoneI++){
    $stonePath=$stoneProtected[$stoneI]
    if(-not $stoneBlobs.ContainsKey($stonePath) -or $stoneBlobs[$stonePath] -ne $stoneCurrent[$stoneI]){
        $stoneMismatch+=@{path=$stonePath;parent=$stoneBlobs[$stonePath];current=$stoneCurrent[$stoneI]}
    }
}
$stoneOut=[ordered]@{schema='mir2.stone-parent-qualification.v1';parent=$stoneParent;
    declared=$stoneSnapshot.files.Count;authored=$stoneAuthored.files.Count;protected=$stoneProtected.Count;
    mismatchCount=$stoneMismatch.Count;mismatches=$stoneMismatch;method='Canonical Git clean blobs, batched stdin-paths';createdUtc=[DateTime]::UtcNow.ToString('o')}
$stonePath=Join-Path $PSScriptRoot 'backend-protected-parent-qualification-01.json'
if(Test-Path $stonePath){throw 'Preserve qualification history'}
[IO.File]::WriteAllText($stonePath,($stoneOut|ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
$stoneOut|ConvertTo-Json -Depth 6
if($stoneMismatch.Count -gt 0){exit 1}
