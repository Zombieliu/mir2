# Build the Windows native host from a clean checkout and emit the exact v2
# attestation consumed by package-windows-candidate.ps1.
[CmdletBinding()]
param(
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-Sha256Hex {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][byte[]]$Bytes)
    return ([BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash($Bytes))).Replace('-', '')
}

function Get-TextSha256 {
    param([Parameter(Mandatory = $true)][string]$Text)
    return Get-Sha256Hex -Bytes ([Text.Encoding]::UTF8.GetBytes($Text))
}

function Assert-NoReparseAncestors {
    param([Parameter(Mandatory = $true)][string]$Path)
    $current = [IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    while (-not [string]::IsNullOrWhiteSpace($current)) {
        if (Test-Path -LiteralPath $current) {
            $item = Get-Item -LiteralPath $current -Force
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "attested build rejects reparse/junction paths: $current"
            }
        }
        $parent = Split-Path -Parent $current
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -ceq $current) { break }
        $current = $parent
    }
}

function Assert-NoReparseTree {
    param([Parameter(Mandatory = $true)][string]$Path)
    Assert-NoReparseAncestors -Path $Path
    if (-not (Test-Path -LiteralPath $Path)) { return }
    $items = @(Get-Item -LiteralPath $Path -Force) + @(Get-ChildItem -LiteralPath $Path -Force -Recurse)
    foreach ($item in $items) {
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "attested build rejects a reparse/junction target tree: $($item.FullName)"
        }
    }
}

function Reset-AttestedTargetDirectory {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$ExpectedLeaf
    )
    $full = [IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    $boundary = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    if (-not $full.StartsWith($boundary + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'refusing to reset an attested target outside the repository'
    }
    if ((Split-Path -Leaf $full) -cne $ExpectedLeaf) {
        throw "refusing to reset an unexpected attested target: $full"
    }
    Assert-NoReparseAncestors -Path $full
    if (Test-Path -LiteralPath $full) {
        Assert-NoReparseTree -Path $full
        Remove-Item -LiteralPath $full -Recurse -Force
    }
    New-Item -ItemType Directory -Path $full | Out-Null
    Assert-NoReparseTree -Path $full
}

function Get-CleanWorktreeState {
    param([Parameter(Mandatory = $true)][string]$Root)
    Push-Location $Root
    try {
        $revision = (& git rev-parse HEAD 2>$null).Trim().ToLowerInvariant()
        if ($LASTEXITCODE -ne 0 -or $revision -notmatch '^[0-9a-f]{40}$') {
            throw 'git HEAD is unavailable'
        }
        $status = & git -c core.quotepath=false -c core.autocrlf=false -c core.safecrlf=false status --porcelain=v1 --untracked-files=all
        if ($LASTEXITCODE -ne 0) { throw 'git status failed' }
        if (@($status).Count -ne 0) {
            throw "attested build requires a clean worktree; first change: $($status | Select-Object -First 1)"
        }
        & git diff --quiet --no-ext-diff --
        if ($LASTEXITCODE -ne 0) { throw 'attested build rejected a worktree diff' }
        & git diff --cached --quiet --no-ext-diff --
        if ($LASTEXITCODE -ne 0) { throw 'attested build rejected an index diff' }

        $scope = 'git-status-z+diff+all-untracked-content-v2'
        $emptyHash = Get-Sha256Hex -Bytes ([byte[]]::new(0))
        $canonical = "SCOPE`n$scope`nREVISION`n$revision`nSTATUS-Z`n0`n$emptyHash`nINDEX-DIFF`n0`n$emptyHash`nWORKTREE-DIFF`n0`n$emptyHash`nUNTRACKED`n0`n`n"
        return [ordered]@{
            revision = $revision
            dirty = $false
            statusLineCount = 0
            statusScope = $scope
            statusSha256 = Get-TextSha256 -Text $canonical
        }
    } finally {
        Pop-Location
    }
}

$ScriptDir = Split-Path -Parent $PSCommandPath
$RepoRoot = [IO.Path]::GetFullPath((Join-Path $ScriptDir '..\..\..\..')).TrimEnd('\', '/')
$TargetDirName = 'target-attested-windows-candidate'
$TargetDir = [IO.Path]::GetFullPath((Join-Path $RepoRoot $TargetDirName)).TrimEnd('\', '/')
if (-not $TargetDir.StartsWith($RepoRoot + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'attested target directory escaped the repository'
}

if ($SelfTest) {
    $emptyHash = Get-Sha256Hex -Bytes ([byte[]]::new(0))
    if ($emptyHash -ne 'E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855') {
        throw 'SHA256 self-test failed'
    }
    if ((Split-Path -Leaf $TargetDir) -cne $TargetDirName) {
        throw 'target directory self-test failed'
    }
    $selfRoot = Join-Path ([IO.Path]::GetTempPath()) ('mir2-attested-build-selftest-' + [guid]::NewGuid().ToString('N'))
    $selfTarget = Join-Path $selfRoot $TargetDirName
    try {
        New-Item -ItemType Directory -Path $selfRoot | Out-Null
        Reset-AttestedTargetDirectory -Path $selfTarget -Root $selfRoot -ExpectedLeaf $TargetDirName
        Set-Content -LiteralPath (Join-Path $selfTarget 'stale.txt') -Value 'stale'
        Reset-AttestedTargetDirectory -Path $selfTarget -Root $selfRoot -ExpectedLeaf $TargetDirName
        if (Test-Path -LiteralPath (Join-Path $selfTarget 'stale.txt')) {
            throw 'target reset self-test retained stale build output'
        }
    } finally {
        if (Test-Path -LiteralPath $selfRoot) {
            Assert-NoReparseTree -Path $selfRoot
            if ((Split-Path -Leaf $selfRoot) -notlike 'mir2-attested-build-selftest-*') {
                throw 'refusing unsafe attested build self-test cleanup'
            }
            Remove-Item -LiteralPath $selfRoot -Recurse -Force
        }
    }
    Write-Host 'build-attested-windows-candidate self-test passed'
    exit 0
}

$before = Get-CleanWorktreeState -Root $RepoRoot
Reset-AttestedTargetDirectory -Path $TargetDir -Root $RepoRoot -ExpectedLeaf $TargetDirName
$cargoHome = if ($env:CARGO_HOME) {
    [IO.Path]::GetFullPath($env:CARGO_HOME).TrimEnd('\', '/')
} else {
    [IO.Path]::GetFullPath((Join-Path $env:USERPROFILE '.cargo')).TrimEnd('\', '/')
}
$remapFlags = @(
    "--remap-path-prefix=$RepoRoot=."
    "--remap-path-prefix=$cargoHome=cargo-home"
)
$controlledBuildVariables = @(
    'RUSTFLAGS',
    'CARGO_ENCODED_RUSTFLAGS',
    'CARGO_BUILD_RUSTFLAGS',
    'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS',
    'RUSTC',
    'RUSTC_WRAPPER',
    'RUSTC_WORKSPACE_WRAPPER',
    'CARGO_BUILD_RUSTC',
    'CARGO_BUILD_RUSTC_WRAPPER',
    'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER'
)
$contaminatedVariables = @(
    foreach ($name in $controlledBuildVariables) {
        $value = [Environment]::GetEnvironmentVariable($name, 'Process')
        if (-not [string]::IsNullOrWhiteSpace($value)) { $name }
    }
    Get-ChildItem Env: | Where-Object { $_.Name -like 'CARGO_PROFILE_RELEASE_*' -and -not [string]::IsNullOrWhiteSpace($_.Value) } | ForEach-Object { $_.Name }
)
if ($contaminatedVariables.Count -gt 0) {
    throw ('attested build rejects compiler-affecting environment variables: ' + (($contaminatedVariables | Sort-Object -Unique) -join ', '))
}
$previousRustFlags = [Environment]::GetEnvironmentVariable('RUSTFLAGS', 'Process')
try {
    $env:RUSTFLAGS = ($remapFlags -join ' ')
    Push-Location $RepoRoot
    try {
        & cargo +1.95.0 build --locked --release --manifest-path apps/game-client/platform-windows/Cargo.toml --bin mir2-platform-windows --target x86_64-pc-windows-msvc --target-dir $TargetDirName
        if ($LASTEXITCODE -ne 0) { throw "attested cargo build failed with exit code $LASTEXITCODE" }
    } finally {
        Pop-Location
    }
} finally {
    [Environment]::SetEnvironmentVariable('RUSTFLAGS', $previousRustFlags, 'Process')
}

$after = Get-CleanWorktreeState -Root $RepoRoot
if (($before | ConvertTo-Json -Compress) -cne ($after | ConvertTo-Json -Compress)) {
    throw 'worktree state changed during attested build'
}

$exePath = Join-Path $TargetDir 'x86_64-pc-windows-msvc\release\mir2-platform-windows.exe'
if (-not (Test-Path -LiteralPath $exePath -PathType Leaf)) {
    throw "attested Release EXE missing: $exePath"
}
$exe = Get-Item -LiteralPath $exePath
$attestationPath = Join-Path $TargetDir 'BUILD-ATTESTATION.json'
$attestation = [ordered]@{
    schema = 'mir2.windows.build-attestation.v2'
    exeSha256 = (Get-FileHash -LiteralPath $exePath -Algorithm SHA256).Hash.ToUpperInvariant()
    exeSizeBytes = [int64]$exe.Length
    gitRevision = $after.revision
    worktreeDirty = $false
    worktreeStatusScope = $after.statusScope
    worktreeStatusSha256 = $after.statusSha256
    worktreeStatusLineCount = 0
    cargoVersion = (& cargo +1.95.0 --version).Trim()
    rustcVersion = (& rustc +1.95.0 --version).Trim()
    buildCommand = [ordered]@{
        executable = 'cargo'
        toolchain = '+1.95.0'
        subcommand = 'build'
        manifestPath = 'apps/game-client/platform-windows/Cargo.toml'
        bin = 'mir2-platform-windows'
        release = $true
        locked = $true
        target = 'x86_64-pc-windows-msvc'
        profile = 'release'
        targetDir = $TargetDirName
        extraArgs = @()
    }
    pathRemapping = [ordered]@{
        enabled = $true
        environmentVariable = 'RUSTFLAGS'
        flags = @(
            [ordered]@{ sourceToken = '<REPO_ROOT>'; destination = '.' }
            [ordered]@{ sourceToken = '<CARGO_HOME>'; destination = 'cargo-home' }
        )
    }
    buildCompletedUtc = [DateTimeOffset]::UtcNow.ToString('o')
}
$json = ($attestation | ConvertTo-Json -Depth 8) + "`n"
[IO.Directory]::CreateDirectory($TargetDir) | Out-Null
[IO.File]::WriteAllText($attestationPath, $json, [Text.UTF8Encoding]::new($false))

Write-Host "releaseExe=$exePath"
Write-Host "buildAttestation=$attestationPath"
Write-Host "gitRevision=$($after.revision)"
Write-Host "exeSha256=$($attestation.exeSha256)"
