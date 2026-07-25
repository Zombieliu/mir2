[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ProjectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$TestRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("mir2-wrapper-test-" + [Guid]::NewGuid().ToString("N"))
$FakeBin = Join-Path $TestRoot "bin"
$FakeDocker = Join-Path $FakeBin "docker.cmd"
$PreviousPath = $env:PATH
$PreviousImage = $env:MIR2_DEV_IMAGE

New-Item -ItemType Directory -Path $FakeBin | Out-Null
[System.IO.File]::WriteAllText(
    $FakeDocker,
    @"
@echo off
if "%~1"=="info" (
  echo 26.1.0
  exit /b 0
)
if "%~1"=="volume" (
  echo mir2-developer-gh-config
  exit /b 0
)
if "%~1"=="compose" (
  if "%~2"=="version" echo Docker Compose version v2.24.4
  exit /b 0
)
echo Unexpected fake Docker command: %* 1>&2
exit /b 2
"@,
    [System.Text.Encoding]::ASCII
)

try {
    $env:PATH = "$FakeBin;$PreviousPath"
    $env:MIR2_DEV_IMAGE = "mir2-web3-developer:wrapper-test"
    $Output = (& (Join-Path $ProjectRoot "scripts\dev.cmd") doctor 2>&1 | Out-String)
    if ($LASTEXITCODE -ne 0) {
        throw "Developer Windows wrapper fixture failed:`n$Output"
    }
    if ($Output -notmatch "\[ok\] Developer environment definition is valid\.") {
        throw "Developer Windows wrapper did not complete doctor:`n$Output"
    }
    Write-Host ($Output.Trim())
    Write-Host "Developer Windows wrapper fixture passed."
}
finally {
    $env:PATH = $PreviousPath
    if ($null -eq $PreviousImage) {
        Remove-Item Env:MIR2_DEV_IMAGE -ErrorAction SilentlyContinue
    }
    else {
        $env:MIR2_DEV_IMAGE = $PreviousImage
    }
    if (Test-Path -LiteralPath $FakeDocker) {
        Remove-Item -LiteralPath $FakeDocker -Force
    }
    if (Test-Path -LiteralPath $FakeBin) {
        Remove-Item -LiteralPath $FakeBin -Force
    }
    if (Test-Path -LiteralPath $TestRoot) {
        Remove-Item -LiteralPath $TestRoot -Force
    }
}
