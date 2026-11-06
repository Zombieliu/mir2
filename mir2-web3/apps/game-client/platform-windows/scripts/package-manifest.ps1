# Hash every regular file under a Candidate package root.
#
# The defaults are derived from this checkout and may be overridden with
# MIR2_CANDIDATE_PACKAGE_ROOT / MIR2_PACKAGE_MANIFEST_OUTPUT. No machine-local
# drive or checkout path is embedded in the manifest tool.
param(
    [string]$PackageRoot = $env:MIR2_CANDIDATE_PACKAGE_ROOT,
    [string]$OutputPath = $env:MIR2_PACKAGE_MANIFEST_OUTPUT,
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-RelativeUnixPath {
    param(
        [string]$Root,
        [string]$FullName
    )
    $rootFull = (Resolve-Path -LiteralPath $Root).Path.TrimEnd("\", "/")
    $full = (Resolve-Path -LiteralPath $FullName).Path
    if (-not $full.StartsWith($rootFull, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "path $full is outside $rootFull"
    }
    $relative = $full.Substring($rootFull.Length).TrimStart("\", "/")
    return ($relative -replace "\\", "/")
}

function Test-PathWithin {
    param(
        [string]$Child,
        [string]$Parent
    )
    $childFull = [IO.Path]::GetFullPath($Child).TrimEnd('\', '/')
    $parentFull = [IO.Path]::GetFullPath($Parent).TrimEnd('\', '/')
    return $childFull.Equals($parentFull, [StringComparison]::OrdinalIgnoreCase) -or
        $childFull.StartsWith($parentFull + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
        $childFull.StartsWith($parentFull + [IO.Path]::AltDirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)
}

function Find-RepoRoot {
    param([string]$StartPath)
    $cursor = [IO.Path]::GetFullPath($StartPath)
    if (Test-Path -LiteralPath $cursor -PathType Leaf) { $cursor = Split-Path -Parent $cursor }
    while ($true) {
        if ((Test-Path -LiteralPath (Join-Path $cursor '.git')) -or
            ((Test-Path -LiteralPath (Join-Path $cursor 'apps') -PathType Container) -and
             (Test-Path -LiteralPath (Join-Path $cursor 'docs') -PathType Container))) { return $cursor }
        $parent = Split-Path -Parent $cursor
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $cursor) { throw "repository root not found from $StartPath" }
        $cursor = $parent
    }
}

$scriptRoot = Split-Path -Parent $PSCommandPath
$repoRoot = Find-RepoRoot -StartPath $scriptRoot
if ([string]::IsNullOrWhiteSpace($PackageRoot)) { $PackageRoot = Join-Path $repoRoot 'dist/mir2-windows-candidate' }
if ([string]::IsNullOrWhiteSpace($OutputPath)) { $OutputPath = Join-Path $repoRoot 'docs/generated/player-qa/windows-package-preflight/package-manifest.json' }

if ($SelfTest) {
    $defaultPackage = [IO.Path]::GetFullPath((Join-Path $repoRoot 'dist/mir2-windows-candidate'))
    $defaultOutput = [IO.Path]::GetFullPath((Join-Path $repoRoot 'docs/generated/player-qa/windows-package-preflight/package-manifest.json'))
    [ordered]@{
        ok = $true
        status = 'HANDOFF'
        repoRoot = $repoRoot
        defaultPackageRoot = $defaultPackage
        defaultOutputPath = $defaultOutput
        desktopTouched = $false
        packageMutated = $false
        note = 'Self-test validates path derivation only; no package was hashed or written.'
    } | ConvertTo-Json -Depth 5
    exit 0
}

$root = (Resolve-Path -LiteralPath $PackageRoot).Path
$resolvedOutput = [IO.Path]::GetFullPath($OutputPath)
if (Test-PathWithin -Child $resolvedOutput -Parent $root) {
    throw "OutputPath must be outside PackageRoot so the manifest cannot hash itself: $resolvedOutput"
}
$files = Get-ChildItem -LiteralPath $root -Recurse -File |
    Where-Object {
        $rel = Get-RelativeUnixPath -Root $root -FullName $_.FullName
        -not ($rel -like "logs/*" -and $rel -ne "logs/.keep")
    } |
    Sort-Object FullName

$entries = @()
$aggregateLines = New-Object System.Collections.Generic.List[string]
$totalBytes = [int64]0

foreach ($file in $files) {
    $relative = Get-RelativeUnixPath -Root $root -FullName $file.FullName
    $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToUpperInvariant()
    $size = [int64]$file.Length
    $totalBytes += $size
    $entries += [ordered]@{
        path   = $relative
        size   = $size
        sha256 = $hash
    }
    [void]$aggregateLines.Add("$relative`t$size`t$hash")
}

$aggregateText = (($aggregateLines | Sort-Object) -join "`n") + "`n"
$aggregateBytes = [System.Text.Encoding]::UTF8.GetBytes($aggregateText)
$aggregateSha = ([BitConverter]::ToString([System.Security.Cryptography.SHA256]::Create().ComputeHash($aggregateBytes))).Replace("-", "")

$manifest = [ordered]@{
    schema               = 'mir2.windows.package-manifest.tool.v1'
    packageRoot          = $root
    packageRootRelative  = if (Test-PathWithin -Child $root -Parent $repoRoot) { Get-RelativeUnixPath -Root $repoRoot -FullName $root } else { $null }
    generatedAtUtc       = [DateTime]::UtcNow.ToString("o")
    fileCount            = $entries.Count
    totalBytes           = $totalBytes
    aggregateSha256      = $aggregateSha
    files                = $entries
}

$outDir = Split-Path -Parent $resolvedOutput
if ($outDir -and -not (Test-Path -LiteralPath $outDir)) {
    New-Item -ItemType Directory -Path $outDir | Out-Null
}
$json = $manifest | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText($resolvedOutput, $json)

Write-Host ("package-manifest status=HANDOFF files={0} bytes={1} aggregate={2} output={3}" -f $entries.Count, $totalBytes, $aggregateSha, $resolvedOutput)
