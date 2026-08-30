[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ProjectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$RepositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot ".."))
$TestRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("mir2-wrapper-test-" + [Guid]::NewGuid().ToString("N"))
$FakeBin = Join-Path $TestRoot "bin"
$FakeDocker = Join-Path $FakeBin "docker.cmd"
$FakeGh = Join-Path $FakeBin "gh.cmd"
$ReleaseLock = Join-Path $ProjectRoot "config\developer-release.json"
$ReleaseBackup = Join-Path $TestRoot "developer-release.json"
$DockerLog = Join-Path $TestRoot "docker.log"
$RuntimeSentinel = Join-Path $TestRoot "runtime-fetch-failed"
$EnvironmentKeys = @(
    "PATH",
    "DOCKER_CONFIG",
    "GH_TOKEN",
    "MIR2_FAKE_GH_FORBIDDEN",
    "MIR2_FAKE_PULL_FAIL",
    "MIR2_FAKE_RUNTIME_FETCH_FAIL",
    "MIR2_FAKE_DOCKER_LOG",
    "MIR2_FAKE_RUNTIME_SENTINEL",
    "MIR2_DEV_IMAGE",
    "MIR2_DEVELOPER_IMAGE_REVISION",
    "MIR2_WEB_PORT",
    "MIR2_GATEWAY_WEB_PORT",
    "MIR2_GATEWAY_TCP_PORT",
    "MIR2_BIND_ADDRESS",
    "MIR2_GATEWAY_WS_URL",
    "MIR2_ASSET_BASE_URL"
)
$OriginalEnvironment = @{}
foreach ($Key in $EnvironmentKeys) {
    $OriginalEnvironment[$Key] = [Environment]::GetEnvironmentVariable($Key, "Process")
}

New-Item -ItemType Directory -Path $FakeBin | Out-Null
[System.IO.File]::WriteAllText(
    $FakeDocker,
@"
@echo off
if not "%MIR2_FAKE_DOCKER_LOG%"=="" echo %*>>"%MIR2_FAKE_DOCKER_LOG%"
if "%MIR2_FAKE_RUNTIME_FETCH_FAIL%"=="1" if exist "%MIR2_FAKE_RUNTIME_SENTINEL%" exit /b 0
if "%MIR2_FAKE_RUNTIME_FETCH_FAIL%"=="1" (
  echo %* | findstr /C:"fetch-prebuilt-bevy-runtime.mjs" >nul
  if not errorlevel 1 (
    type nul >"%MIR2_FAKE_RUNTIME_SENTINEL%"
    exit /b 1
  )
)
if "%MIR2_FAKE_PULL_FAIL%"=="1" if "%~1"=="pull" goto :forced_pull_failure
if "%MIR2_FAKE_PULL_FAIL%"=="1" if "%~1"=="image" if "%~2"=="inspect" goto :forced_pull_failure
if "%~1"=="info" (
  echo 26.1.0
  exit /b 0
)
if "%~1"=="login" (
  more >nul
  if "%~2"=="ghcr.io" exit /b 0
  exit /b 2
)
if "%~1"=="pull" (
  if "%~2"=="ghcr.io/zombieliu/mir2-developer@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" exit /b 0
  exit /b 2
)
if "%~1"=="image" (
  if "%~2"=="inspect" (
    echo %MIR2_DEVELOPER_IMAGE_REVISION%
    exit /b 0
  )
  exit /b 2
)
if "%~1"=="compose" (
  if "%~2"=="version" (
    echo Docker Compose version v2.24.4
    exit /b 0
  )
  if "%~2"=="-f" (
    if "%~4"=="config" if "%~5"=="--quiet" exit /b 0
    if "%~4"=="build" if "%~5"=="workspace" exit /b 0
    if "%~4"=="run" (
      if "%~7"=="workspace" exit /b 0
      if "%~7"=="-T" if "%~8"=="asset-fetch" (
        more >nul
        exit /b 0
      )
    )
  )
)
echo Unexpected fake Docker command: %* 1>&2
exit /b 2
:forced_pull_failure
exit /b 1
"@,
    [System.Text.Encoding]::ASCII
)
[System.IO.File]::WriteAllText(
    $FakeGh,
@"
@echo off
if "%MIR2_FAKE_GH_FORBIDDEN%"=="1" goto :forbidden
if "%~1"=="api" if "%~2"=="repos/Zombieliu/mir2/git/ref/tags/developer-image-%MIR2_DEVELOPER_IMAGE_REVISION%" (
  echo tag
  echo cccccccccccccccccccccccccccccccccccccccc
  exit /b 0
)
if "%~1"=="api" if "%~2"=="repos/Zombieliu/mir2/git/tags/cccccccccccccccccccccccccccccccccccccccc" (
  echo %MIR2_DEVELOPER_IMAGE_REVISION%
  echo ghcr.io/zombieliu/mir2-developer@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
  exit /b 0
)
if "%~1"=="auth" if "%~2"=="status" exit /b 0
if "%~1"=="auth" if "%~2"=="token" (
  echo fixture-token
  exit /b 0
)
if "%~1"=="api" if "%~2"=="user" (
  echo fixture-user
  exit /b 0
)
echo Unexpected fake GitHub CLI command: %* 1>&2
exit /b 2
:forbidden
echo GitHub CLI must not be called during Starter fallback. 1>&2
exit /b 99
"@,
    [System.Text.Encoding]::ASCII
)

try {
    $env:PATH = "$FakeBin;$($OriginalEnvironment.PATH)"
    $env:MIR2_DEV_IMAGE = "mir2-web3-developer:wrapper-test"

    $CmdOutput = (& (Join-Path $ProjectRoot "scripts\dev.cmd") doctor 2>&1 | Out-String)
    if ($LASTEXITCODE -ne 0 -or
        $CmdOutput -notmatch "\[ok\] Developer environment definition is valid\.") {
        throw "Developer Windows .cmd wrapper fixture failed:`n$CmdOutput"
    }
    Write-Host ($CmdOutput.Trim())

    $Sentinels = @{
        MIR2_DEV_IMAGE = "mir2-web3-developer:environment-sentinel"
        MIR2_DEVELOPER_IMAGE_REVISION = "revision-sentinel"
        MIR2_WEB_PORT = "45555"
        MIR2_GATEWAY_WEB_PORT = "45556"
        MIR2_GATEWAY_TCP_PORT = "45557"
        MIR2_BIND_ADDRESS = "127.0.0.2"
        MIR2_GATEWAY_WS_URL = "ws://127.0.0.2:45556/ws"
        MIR2_ASSET_BASE_URL = "https://assets.example.test/revision"
    }
    foreach ($Entry in $Sentinels.GetEnumerator()) {
        Set-Item -Path "Env:$($Entry.Key)" -Value $Entry.Value
    }
    & (Join-Path $ProjectRoot "scripts\dev.ps1") -Command doctor
    foreach ($Entry in $Sentinels.GetEnumerator()) {
        $Actual = [Environment]::GetEnvironmentVariable($Entry.Key, "Process")
        if ($Actual -ne $Entry.Value) {
            throw "Developer Windows wrapper did not restore $($Entry.Key)."
        }
    }

    Copy-Item -LiteralPath $ReleaseLock -Destination $ReleaseBackup
    $Release = Get-Content -LiteralPath $ReleaseLock -Raw | ConvertFrom-Json
    $Revision = (& git -C $RepositoryRoot rev-parse HEAD).Trim()
    $Release.container.publishedDigest = "sha256:" + ("a" * 64)
    $Release.container.publishedRevision = $Revision
    $Release | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $ReleaseLock -Encoding utf8

    Remove-Item Env:MIR2_DEV_IMAGE -ErrorAction SilentlyContinue
    $AssetOutput = (
        & (Join-Path $ProjectRoot "scripts\dev.ps1") -Command assets *>&1 |
            Out-String
    )
    if ($AssetOutput -notmatch "\[ok\] Assets") {
        throw "Developer Windows immutable asset image fixture failed:`n$AssetOutput"
    }
    if ($null -ne [Environment]::GetEnvironmentVariable("MIR2_DEV_IMAGE", "Process")) {
        throw "Developer Windows asset command did not restore MIR2_DEV_IMAGE."
    }
    Write-Host ($AssetOutput.Trim())

    $env:MIR2_FAKE_PULL_FAIL = "1"
    $env:MIR2_FAKE_GH_FORBIDDEN = "1"
    $FallbackOutput = (
        & (Join-Path $ProjectRoot "scripts\dev.ps1") -Command shell *>&1 |
            Out-String
    )
    Remove-Item Env:MIR2_FAKE_PULL_FAIL -ErrorAction SilentlyContinue
    Remove-Item Env:MIR2_FAKE_GH_FORBIDDEN -ErrorAction SilentlyContinue
    if ($FallbackOutput -notmatch "falling back to the locked local build" -or
        $FallbackOutput -notmatch "Build the locked local developer image") {
        throw "Developer Windows private-image fallback fixture failed:`n$FallbackOutput"
    }
    Write-Host ($FallbackOutput.Trim())

    $env:MIR2_DEV_IMAGE = "mir2-web3-developer:wrapper-test"
    $env:MIR2_FAKE_RUNTIME_FETCH_FAIL = "1"
    $env:MIR2_FAKE_DOCKER_LOG = $DockerLog
    $env:MIR2_FAKE_RUNTIME_SENTINEL = $RuntimeSentinel
    $RuntimeFallbackOutput = (
        & (Join-Path $ProjectRoot "scripts\dev.ps1") -Command verify *>&1 |
            Out-String
    )
    Remove-Item Env:MIR2_FAKE_RUNTIME_FETCH_FAIL -ErrorAction SilentlyContinue
    Remove-Item Env:MIR2_FAKE_DOCKER_LOG -ErrorAction SilentlyContinue
    Remove-Item Env:MIR2_FAKE_RUNTIME_SENTINEL -ErrorAction SilentlyContinue
    $DockerCalls = Get-Content -LiteralPath $DockerLog -Raw
    if ($RuntimeFallbackOutput -notmatch
        "Pinned Bevy runtime is unavailable; rebuilding it from current source\.") {
        throw "Developer Windows runtime fallback fixture failed:`n$RuntimeFallbackOutput`n$DockerCalls"
    }
    if ($DockerCalls -notmatch
        "MIR2_USE_PREBUILT_BEVY_RUNTIME=0 node apps/web/scripts/build-bevy-runtime.mjs release") {
        throw "Developer Windows runtime fallback did not invoke the source build."
    }
    Write-Host ($RuntimeFallbackOutput.Trim())

    $env:MIR2_DEV_IMAGE =
        "ghcr.io/example/evil@sha256:" + ("b" * 64)
    $Rejected = $false
    try {
        & (Join-Path $ProjectRoot "scripts\dev.ps1") -Command assets *> $null
    }
    catch {
        $Rejected = $_.Exception.Message -match
            "Full asset authorization refuses a custom developer image"
    }
    if (-not $Rejected) {
        throw "Developer Windows wrapper did not reject a custom digest image."
    }

    Write-Host "Developer Windows wrapper fixture passed."
}
finally {
    if (Test-Path -LiteralPath $ReleaseBackup) {
        Copy-Item -LiteralPath $ReleaseBackup -Destination $ReleaseLock -Force
    }
    foreach ($Key in $EnvironmentKeys) {
        $Value = $OriginalEnvironment[$Key]
        if ($null -eq $Value) {
            Remove-Item -Path "Env:$Key" -ErrorAction SilentlyContinue
        }
        else {
            Set-Item -Path "Env:$Key" -Value $Value
        }
    }
    foreach ($Path in @(
        $FakeDocker,
        $FakeGh,
        $ReleaseBackup,
        $DockerLog,
        $RuntimeSentinel
    )) {
        if (Test-Path -LiteralPath $Path) {
            Remove-Item -LiteralPath $Path -Force
        }
    }
    if (Test-Path -LiteralPath $FakeBin) {
        Remove-Item -LiteralPath $FakeBin -Force
    }
    if (Test-Path -LiteralPath $TestRoot) {
        Remove-Item -LiteralPath $TestRoot -Force
    }
}
