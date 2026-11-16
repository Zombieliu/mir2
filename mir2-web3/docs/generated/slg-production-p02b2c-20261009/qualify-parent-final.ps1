$ErrorActionPreference = 'Stop'
$taskRepo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$taskParent = '78d3abecb887d0e2c447d06e8224e7670486758e'
$taskSnapshot = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'creature-reward-tests-02/source-snapshot.json') -Raw | ConvertFrom-Json
$taskAuthored = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'authored-source-02.json') -Raw | ConvertFrom-Json
$taskProtected = @($taskSnapshot.files.path | Where-Object {$_ -notin $taskAuthored.files.path})
if($taskSnapshot.files.Count -ne 462 -or $taskProtected.Count -ne 458) {throw 'Unexpected bounded input declaration'}
$taskTree = @(& git -c "safe.directory=$taskRepo" -c gc.auto=0 -C $taskRepo ls-tree -r --full-tree $taskParent)
if($LASTEXITCODE -ne 0) {throw 'Cannot inspect protected production parent'}
$taskBlobs = @{}
foreach($taskRow in $taskTree) {
    if($taskRow -match '^\d+ blob ([0-9a-f]+)\t(.+)$') {$taskBlobs[$Matches[2]] = $Matches[1]}
}
$taskCurrent = @($taskProtected | & git -c "safe.directory=$taskRepo" -c gc.auto=0 -C $taskRepo hash-object --stdin-paths)
if($LASTEXITCODE -ne 0 -or $taskCurrent.Count -ne $taskProtected.Count) {throw 'Cannot compute canonical protected input blobs'}
$taskMismatches = @()
for($taskIndex=0;$taskIndex -lt $taskProtected.Count;$taskIndex++) {
    $taskPath = $taskProtected[$taskIndex]
    if(-not $taskBlobs.ContainsKey($taskPath) -or $taskBlobs[$taskPath] -ne $taskCurrent[$taskIndex]) {
        $taskMismatches += [ordered]@{path=$taskPath;parent=$taskBlobs[$taskPath];current=$taskCurrent[$taskIndex]}
    }
}
$taskOut = [ordered]@{schema='mir2.production-protected-parent.v1';createdUtc=[DateTime]::UtcNow.ToString('o');parent=$taskParent;declaredInputs=462;authoredInputs=4;declaredProtectedInputs=458;mismatchCount=$taskMismatches.Count;mismatches=$taskMismatches;method='Current canonical Git-clean file blobs compared to parent ls-tree; batched stdin-paths'}
$taskOutPath = Join-Path $PSScriptRoot 'final-protected-parent-qualification.json'
if(Test-Path -LiteralPath $taskOutPath) {throw 'Preserve existing final qualification'}
[IO.File]::WriteAllText($taskOutPath,($taskOut | ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
if($taskMismatches.Count -ne 0) {throw 'Protected inputs differ from production parent'}
$taskOut | ConvertTo-Json -Depth 6
