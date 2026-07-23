[CmdletBinding()]
param(
    [switch]$SkipRustCheck,
    [switch]$SkipWebInstall
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ProjectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$RepositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot ".."))
$WebRoot = Join-Path $ProjectRoot "apps\web"
$AdminWebRoot = Join-Path $ProjectRoot "apps\admin-web"

function Require-Command {
    param([string]$Name, [string]$InstallHint)

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing required command '$Name'. $InstallHint"
    }
}

function Invoke-Checked {
    param([string]$Label, [scriptblock]$Command)

    Write-Host "[setup] $Label"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE"
    }
}

Require-Command "git" "Install Git for Windows and clone with --recurse-submodules."
Require-Command "node" "Install Node.js 22 or newer."
Require-Command "npm.cmd" "npm is included with Node.js."
if (-not $SkipRustCheck) {
    Require-Command "rustup" "Install Rust from https://rustup.rs/."
    Require-Command "cargo" "Install Rust from https://rustup.rs/."
}

$NodeMajor = [int]((& node --version).TrimStart("v").Split(".")[0])
if ($NodeMajor -lt 22) {
    throw "Node.js 22 or newer is required; found $(& node --version)."
}

$SubmoduleState = (& git -C $RepositoryRoot submodule status --recursive 2>&1 | Out-String).Trim()
if (-not $SubmoduleState -or $SubmoduleState.StartsWith("-") -or $SubmoduleState.StartsWith("+")) {
    Invoke-Checked "initialize Crystal submodule" {
        git -C $RepositoryRoot submodule update --init --recursive
    }
}

$ExpectedCrystalCommit = (& git -C $RepositoryRoot ls-tree HEAD Crystal).Split()[2]
$ActualCrystalCommit = (& git -C (Join-Path $RepositoryRoot "Crystal") rev-parse HEAD).Trim()
if ($ExpectedCrystalCommit -ne $ActualCrystalCommit) {
    throw "Crystal submodule mismatch: expected $ExpectedCrystalCommit, found $ActualCrystalCommit. Run git submodule update --init --recursive."
}

if (-not $SkipRustCheck) {
    $Toolchains = (& rustup toolchain list | Out-String)
    if ($Toolchains -notmatch "(?m)^1\.89\.0") {
        Invoke-Checked "install Rust 1.89.0" {
            rustup toolchain install 1.89.0 --profile minimal
        }
    }
}

if (-not $SkipWebInstall) {
    Invoke-Checked "install Player Web dependencies" {
        & npm.cmd ci --prefix $WebRoot
    }
    Invoke-Checked "install Admin Web dependencies" {
        & npm.cmd ci --prefix $AdminWebRoot
    }
}

if (-not $SkipRustCheck) {
    Push-Location $ProjectRoot
    try {
        Invoke-Checked "check Rust Gateway" {
            cargo +1.89.0 check --locked -p mir2-gateway
        }
    }
    finally {
        Pop-Location
    }
}

$PrebuiltRuntime = Join-Path $WebRoot "public\bevy-runtime\pkg-webgpu\mir2_bevy_runtime_bg.wasm"
if (-not (Test-Path -LiteralPath $PrebuiltRuntime -PathType Leaf)) {
    throw "Tracked prebuilt Bevy runtime is missing: $PrebuiltRuntime"
}

Write-Host ""
Write-Host "Developer bootstrap passed."
Write-Host "Start the game with:"
Write-Host "  powershell -ExecutionPolicy Bypass -File .\scripts\start-developer.ps1 -OpenBrowser"
