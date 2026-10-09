param(
    [Parameter(Mandatory = $true)][string]$ClientExe,
    [Parameter(Mandatory = $true)][string]$LogDirectory
)
$ErrorActionPreference = 'Stop'
$taskClient = (Resolve-Path -LiteralPath $ClientExe).Path
$taskLogs = [IO.Path]::GetFullPath($LogDirectory)
New-Item -ItemType Directory -Path $taskLogs -Force | Out-Null
$taskPrefix = Join-Path $taskLogs (Get-Date -Format 'yyyyMMdd-HHmmss-fff')
$env:RUST_BACKTRACE = '1'
$env:MIR2_NATIVE_SOAK_METRICS = '1'
$taskProcess = Start-Process -FilePath $taskClient -WorkingDirectory (Split-Path -Parent $taskClient) -PassThru -RedirectStandardOutput "$taskPrefix.stdout.log" -RedirectStandardError "$taskPrefix.stderr.log"
@{ event='start'; at=(Get-Date).ToUniversalTime().ToString('o'); processId=$taskProcess.Id; executable=$taskClient } | ConvertTo-Json -Compress | Set-Content "$taskPrefix.process.jsonl"
while (-not $taskProcess.WaitForExit(10000)) {
    $taskProcess.Refresh()
    if (-not $taskProcess.HasExited) {
        @{ event='sample'; at=(Get-Date).ToUniversalTime().ToString('o'); privateBytes=$taskProcess.PrivateMemorySize64; workingSet=$taskProcess.WorkingSet64; cpuSeconds=$taskProcess.TotalProcessorTime.TotalSeconds } | ConvertTo-Json -Compress | Add-Content "$taskPrefix.process.jsonl"
    }
}
$taskProcess.WaitForExit()
@{ event='exit'; at=(Get-Date).ToUniversalTime().ToString('o'); exitCode=$taskProcess.ExitCode } | ConvertTo-Json -Compress | Add-Content "$taskPrefix.process.jsonl"
