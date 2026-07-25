param(
  [Parameter(Mandatory = $true)]
  [string]$SourceDir
)

$ErrorActionPreference = "Stop"
$installRoot = Join-Path $env:LOCALAPPDATA "Obelisk\DubheHomeAgent"
$binDir = Join-Path $installRoot "bin"
New-Item -ItemType Directory -Force -Path $binDir | Out-Null

Copy-Item (Join-Path $SourceDir "home_agent.exe") $binDir -Force
Copy-Item (Join-Path $SourceDir "home_agent_launcher.exe") $binDir -Force
Copy-Item (Join-Path $SourceDir "home_agent_supervisor.exe") $binDir -Force
Copy-Item (Join-Path $SourceDir "zone_host.exe") $binDir -Force

& (Join-Path $binDir "home_agent_supervisor.exe") key-init
if ($LASTEXITCODE -ne 0) {
  throw "Home Agent identity initialization failed"
}

[Environment]::SetEnvironmentVariable(
  "MIR2_HOME_MANAGE_CHILDREN",
  "true",
  [EnvironmentVariableTarget]::User
)
[Environment]::SetEnvironmentVariable(
  "MIR2_HOME_BIN_DIR",
  $binDir,
  [EnvironmentVariableTarget]::User
)

$action = New-ScheduledTaskAction `
  -Execute (Join-Path $binDir "home_agent_launcher.exe")
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
$settings = New-ScheduledTaskSettingsSet `
  -RestartCount 10 `
  -RestartInterval (New-TimeSpan -Minutes 1) `
  -ExecutionTimeLimit (New-TimeSpan -Days 3650)
Register-ScheduledTask `
  -TaskName "DubheHomeAgent" `
  -Action $action `
  -Trigger $trigger `
  -Settings $settings `
  -Description "Dubhe Home Agent per-user supervisor" `
  -Force | Out-Null

Write-Host "Binaries and the Windows Credential Manager identity are installed."
Write-Host "Complete signed enrollment/configuration before starting the scheduled task."
