param(
    [Parameter(Mandatory=$true)][string]$CheckId,
    [Parameter(Mandatory=$true)][string]$Kind,
    [ValidateRange(1,45)][int]$WaitSeconds=45
)
$ErrorActionPreference='Stop'
$taskDeadline=[DateTime]::UtcNow.AddSeconds($WaitSeconds)
while(@(Get-Process cargo,rustc -ErrorAction SilentlyContinue).Count -gt 0) {
    if([DateTime]::UtcNow -ge $taskDeadline) {
        [ordered]@{decision='DEFER_CARGO';cargoStarted=$false;testsExecuted=0;waitSeconds=$WaitSeconds}|ConvertTo-Json
        exit 0
    }
    Start-Sleep -Milliseconds 250
}
& (Join-Path $PSScriptRoot 'run-check.ps1') -CheckId $CheckId -Kind $Kind *> (Join-Path $PSScriptRoot "$CheckId.console.log")
$taskExit=$LASTEXITCODE
$taskResult=Join-Path $PSScriptRoot "$CheckId/result.json"
if(Test-Path -LiteralPath $taskResult){Get-Content -LiteralPath $taskResult}
exit $taskExit
