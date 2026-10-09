param([int]$Port = 3921, [string]$DataDirectory)
$ErrorActionPreference = 'Stop'
if ($Port -lt 1024 -or $Port -gt 65535) { throw 'Choose a port between 1024 and 65535.' }
$promoRuntime = Get-Command python -ErrorAction SilentlyContinue
if (-not $promoRuntime) { throw 'Python 3.11+ is required. Install Python, then run this script again.' }
if (-not $DataDirectory) { $DataDirectory = Join-Path $PSScriptRoot 'artifacts/promo-studio' }
$promoDataPath = [IO.Path]::GetFullPath($DataDirectory)
New-Item -ItemType Directory -Force -Path $promoDataPath | Out-Null
$promoUrl = 'http://127.0.0.1:' + $Port
try {
    $promoRunning = Invoke-RestMethod -Uri ($promoUrl + '/api/status') -TimeoutSec 2
    if ($promoRunning.campaigns -and $promoRunning.provider -and $promoRunning.tooling) {
        Write-Output ('Studio already running: ' + $promoUrl)
        Start-Process $promoUrl
        return
    }
} catch { }
$promoStamp = Get-Date -Format 'yyyyMMdd-HHmmss-fff'
$promoArguments = @('-u', ('"' + (Join-Path $PSScriptRoot 'server.py') + '"'), '--port', [string]$Port, '--data-dir', ('"' + $promoDataPath.TrimEnd('\') + '"'))
$promoProcess = Start-Process -FilePath $promoRuntime.Source -ArgumentList $promoArguments -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $promoDataPath ('startup-' + $promoStamp + '.log')) -RedirectStandardError (Join-Path $promoDataPath ('startup-' + $promoStamp + '-error.log'))
$promoDeadline = (Get-Date).AddSeconds(25)
while ((Get-Date) -lt $promoDeadline) {
    if ($promoProcess.HasExited) { throw ('Studio did not start. Inspect the startup logs in ' + $promoDataPath) }
    try {
        $promoReady = Invoke-RestMethod -Uri ($promoUrl + '/api/status') -TimeoutSec 2
        if ($promoReady.campaigns -and $promoReady.provider -and $promoReady.tooling) {
            Write-Output ('Studio: ' + $promoUrl + ' | PID: ' + $promoProcess.Id)
            Start-Process $promoUrl
            return
        }
    } catch { }
    Start-Sleep -Milliseconds 300
}
throw ('Studio is still starting. Inspect the startup logs in ' + $promoDataPath)
