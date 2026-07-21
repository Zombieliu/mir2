[CmdletBinding()]
param(
    [string]$AssetBaseUrl = "",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ProjectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$RepositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot ".."))
$WebRoot = Join-Path $ProjectRoot "apps\web"
$CrystalRoot = Join-Path $RepositoryRoot "Crystal"
$RemoteReleaseManifestPath = ""

function Invoke-Checked {
    param([string]$Label, [scriptblock]$Command)
    Write-Host "[verify] $Label"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE"
    }
}

$ExpectedCrystalCommit = (& git -C $RepositoryRoot ls-tree HEAD Crystal).Split()[2]
$ActualCrystalCommit = (& git -C $CrystalRoot rev-parse HEAD).Trim()
if ($ExpectedCrystalCommit -ne $ActualCrystalCommit) {
    throw "Crystal submodule mismatch: expected $ExpectedCrystalCommit, found $ActualCrystalCommit."
}

$RemoteCrystalCommit = (& git -C $CrystalRoot ls-remote origin "refs/heads/codex/handoff-parity-tools" | Out-String)
if ($RemoteCrystalCommit -notmatch [regex]::Escape($ExpectedCrystalCommit)) {
    throw "Crystal commit $ExpectedCrystalCommit is not reachable from the handoff branch on the configured origin."
}

foreach ($RelativePath in @(
    "public\bevy-runtime\pkg-webgpu\mir2_bevy_runtime_bg.wasm",
    "public\bevy-runtime\pkg-webgl2\mir2_bevy_runtime_bg.wasm",
    "public\generated\crystal-packs\entities-starter\manifest.json",
    "public\original-ui\NPC\03\0.png",
    "public\original-map\WemadeMir2\Objects\2136.png"
)) {
    $FullPath = Join-Path $WebRoot $RelativePath
    if (-not (Test-Path -LiteralPath $FullPath -PathType Leaf)) {
        throw "Required tracked runtime asset is missing: $FullPath"
    }
}

if ($AssetBaseUrl) {
    $ReleaseUrl = "$($AssetBaseUrl.TrimEnd('/'))/remote-asset-release.json"
    $RemoteReleaseDirectory = Join-Path $ProjectRoot ".mir2-data\remote-release-verification"
    New-Item -ItemType Directory -Path $RemoteReleaseDirectory -Force | Out-Null
    $RemoteReleaseManifestPath = Join-Path $RemoteReleaseDirectory "remote-asset-release.json"
    Write-Host "[verify] remote release: $ReleaseUrl"
    Invoke-WebRequest `
        -UseBasicParsing `
        -Uri $ReleaseUrl `
        -OutFile $RemoteReleaseManifestPath `
        -TimeoutSec 30
}

Push-Location $ProjectRoot
try {
    Invoke-Checked "Gateway check" {
        cargo +1.89.0 check --locked -p mir2-gateway
    }
}
finally {
    Pop-Location
}

Invoke-Checked "developer asset installer fixture" {
    & (Join-Path $PSScriptRoot "test-developer-asset-installer.ps1")
}

Push-Location $WebRoot
try {
    Invoke-Checked "asset release safety tests" {
        & npm.cmd run test:asset-release-safety
    }
    Invoke-Checked "domain proxy asset routing" {
        & npm.cmd run test:domain-proxy-routing
    }
    if ($AssetBaseUrl) {
        Invoke-Checked "remote full-pack closure" {
            & node.exe .\scripts\release-doctor.mjs `
                --manifest $RemoteReleaseManifestPath `
                --checkManifest true `
                --checkR2 true `
                --checkWorker false `
                --checkBevyRuntime false `
                --requireFullCrystalPack true `
                --probeConcurrency 32 `
                --assetBaseUrl $AssetBaseUrl
        }
    }
    Invoke-Checked "TypeScript" {
        .\node_modules\.bin\tsc.cmd --noEmit --pretty false
    }
    if (-not $SkipBuild) {
        $PreviousPrebuilt = $env:MIR2_USE_PREBUILT_BEVY_RUNTIME
        try {
            $env:MIR2_USE_PREBUILT_BEVY_RUNTIME = "1"
            Invoke-Checked "production Web build" {
                & npm.cmd run build
            }
        }
        finally {
            $env:MIR2_USE_PREBUILT_BEVY_RUNTIME = $PreviousPrebuilt
        }
    }
}
finally {
    Pop-Location
}

Write-Host "Developer setup verification passed."
