#Requires -Version 7.2
<#
Build only the native executable for a signed packed generation. This does not
expand resources, prepare a v4 Candidate, sign, launch, or publish anything.
A clean committed source inventory is evidence of input bytes, not proof of a
hermetic or reproducible build. Existing Cargo caches are deliberately reusable.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-fA-F]{40}$')][string]$SourceRevision,
    [Parameter(Mandatory)][string]$OutputRoot,
    [Parameter(Mandatory)][string]$TargetDir,
    [Parameter(Mandatory)][string]$PythonPath
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-PackedAbsolutePath([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value) -or $Value.IndexOfAny([char[]]"`0`r`n") -ge 0 -or
        ![IO.Path]::IsPathFullyQualified($Value)) { throw 'An explicit absolute local path is required' }
    $full = [IO.Path]::GetFullPath($Value)
    if ($full -notmatch '^[A-Za-z]:[\\/]') { throw 'UNC and device namespace paths are unsupported' }
    return $full.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
}
function Assert-PackedPlainPath([string]$Path) {
    $current = [IO.Path]::GetFullPath($Path)
    while ($current) {
        if (Test-Path -LiteralPath $current) {
            $item = Get-Item -LiteralPath $current -Force
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "Linked input/output path rejected: $current"
            }
        }
        $parent = Split-Path -Parent $current
        if (!$parent -or $parent -eq $current) { break }
        $current = $parent
    }
}
function Test-PackedWithin([string]$Path, [string]$Root) {
    return $Path.Equals($Root, [StringComparison]::OrdinalIgnoreCase) -or
        $Path.StartsWith($Root.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar,
            [StringComparison]::OrdinalIgnoreCase)
}
function Assert-PackedBuildPaths([string]$SourceRoot, [string]$Output, [string]$Target) {
    foreach ($path in @($SourceRoot, $Output, $Target)) { Assert-PackedPlainPath $path }
    if (([IO.Path]::GetPathRoot($Target)) -ine 'C:\') { throw 'TargetDir must be on the local C volume' }
    if ((Test-PackedWithin $Output $SourceRoot) -or (Test-PackedWithin $Target $SourceRoot) -or
        (Test-PackedWithin $SourceRoot $Output) -or (Test-PackedWithin $SourceRoot $Target)) {
        throw 'Build outputs and source tree must not overlap'
    }
    if ((Test-PackedWithin $Output $Target) -or (Test-PackedWithin $Target $Output)) {
        throw 'OutputRoot and TargetDir must not overlap'
    }
    if (Test-Path -LiteralPath $Output) { throw 'OutputRoot must be a fresh create-only directory' }
    if (!(Test-Path -LiteralPath (Split-Path -Parent $Output) -PathType Container)) {
        throw 'OutputRoot parent directory must already exist'
    }
    if ((Test-Path -LiteralPath $Target) -and !(Test-Path -LiteralPath $Target -PathType Container)) {
        throw 'TargetDir must be a directory'
    }
}
function Assert-PackedCompilerEnvironment {
    # Reject both Rust's variables and Cargo's environment forms of those build
    # settings. No flags (including path remapping) are added by this script.
    $names = @('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER',
        'CARGO_BUILD_RUSTFLAGS', 'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS',
        'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER', 'RUSTC', 'CARGO_BUILD_RUSTC')
    $record = [ordered]@{}
    foreach ($name in $names) {
        if (![string]::IsNullOrEmpty([Environment]::GetEnvironmentVariable($name))) {
            throw "Uncontrolled compiler environment rejected: $name"
        }
        $record[$name] = ''
    }
    return $record
}
function Invoke-PackedPython([string]$Python, [string]$Code, [string[]]$Arguments) {
    $lines = @(& $Python '-B' '-c' $Code @Arguments 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "Packed build input gate failed: $($lines -join [Environment]::NewLine)" }
    try { return ($lines -join [Environment]::NewLine) | ConvertFrom-Json -AsHashtable }
    catch { throw 'Packed build input gate did not return valid JSON' }
}
function Resolve-PackedPlainPath([string]$Python, [string]$Value) {
    $absolute = Get-PackedAbsolutePath $Value
    Assert-PackedPlainPath $absolute
    # Resolve existing ancestors, including DOS short-name aliases, before
    # comparing source/output boundaries. Reparse paths remain forbidden.
    $code = 'import json,os,sys; print(json.dumps({"path":os.path.realpath(sys.argv[1])}))'
    $resolved = Get-PackedAbsolutePath (Invoke-PackedPython $Python $code @($absolute)).path
    Assert-PackedPlainPath $resolved
    return $resolved
}
function Get-PackedSourceSnapshot([string]$Python, [string]$PrepareScript, [string]$Root, [string]$Revision) {
    $code = @'
import importlib.util,json,sys
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location("packed_native_preparation",sys.argv[1])
module=importlib.util.module_from_spec(spec)
sys.modules[spec.name]=module
spec.loader.exec_module(module)
try:
    value=module.source_snapshot(sys.argv[2],sys.argv[3])
    print(json.dumps(value,separators=(",",":")))
except Exception as error:
    sys.exit(str(error))
'@
    return Invoke-PackedPython $Python $code @($PrepareScript, $Root, $Revision)
}
function Assert-PackedSameSource($Before, $After) {
    foreach ($key in @('root', 'revision', 'clean', 'statusSha256', 'sourceFilesChecked', 'sourceInventorySha256')) {
        if ($Before[$key] -cne $After[$key]) { throw "Source changed during build: $key" }
    }
    if ($Before.clean -ne $true -or $Before.sourceFilesChecked -lt 1) { throw 'Committed compiler source inventory required' }
}
function Copy-PackedExecutable([string]$Python, [string]$PrepareScript, [string]$InputPath, [string]$OutputPath) {
    $code = @'
import importlib.util,json,os,sys
from pathlib import Path
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location("packed_native_preparation",sys.argv[1])
module=importlib.util.module_from_spec(spec)
sys.modules[spec.name]=module
spec.loader.exec_module(module)
try:
    target=Path(sys.argv[3])
    module.require(not os.path.lexists(target),"executable output must be create-only")
    entry=module.file_entry(Path(sys.argv[2]),target.name,512*1024**2)
    module.require(module.copy_new(entry,target),"executable output must be new")
    print(json.dumps(module.entry_dict(entry),separators=(",",":")))
except Exception as error:
    sys.exit(str(error))
'@
    return Invoke-PackedPython $Python $code @($PrepareScript, $InputPath, $OutputPath)
}
function Write-PackedNewJson([string]$Path, $Value) {
    Assert-PackedPlainPath $Path
    $bytes = [Text.UTF8Encoding]::new($false).GetBytes(($Value | ConvertTo-Json -Depth 20 -Compress))
    $file = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try { $file.Write($bytes); $file.Flush($true) } finally { $file.Dispose() }
}

if (!$IsWindows) { throw 'The packed native release build requires Windows' }
$SourceRevision = $SourceRevision.ToLowerInvariant()
$OutputRoot = Get-PackedAbsolutePath $OutputRoot
$TargetDir = Get-PackedAbsolutePath $TargetDir
$PythonPath = Get-PackedAbsolutePath $PythonPath
Assert-PackedPlainPath $PythonPath
if (!(Test-Path -LiteralPath $PythonPath -PathType Leaf) -or [IO.Path]::GetExtension($PythonPath) -ine '.exe') {
    throw 'PythonPath must name an existing Python executable'
}
$compilerEnvironment = Assert-PackedCompilerEnvironment
$project = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
$git = (Get-Command git -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
$cargo = (Get-Command cargo -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
# source_snapshot uses root-relative ls-tree/hash-object paths. Never pass the
# nested mir2-web3 project directory when Git's actual top-level is its parent.
$rootLines = @(& $git '-C' $project 'rev-parse' '--show-toplevel' 2>&1)
if ($LASTEXITCODE -ne 0 -or $rootLines.Count -ne 1) { throw 'Actual Git top-level source directory required' }
$sourceRoot = Resolve-PackedPlainPath $PythonPath ([string]$rootLines[0])
$project = Resolve-PackedPlainPath $PythonPath $project
$OutputRoot = Resolve-PackedPlainPath $PythonPath $OutputRoot
$TargetDir = Resolve-PackedPlainPath $PythonPath $TargetDir
if (!(Test-PackedWithin $project $sourceRoot)) { throw 'Native project must belong to the source repository' }
Assert-PackedBuildPaths $sourceRoot $OutputRoot $TargetDir
$prepareScript = Join-Path $PSScriptRoot 'prepare-packed-generation.py'
$archiveScript = Join-Path $PSScriptRoot 'archive-update-release.py'
$manifest = Join-Path $project 'apps/game-client/platform-windows/Cargo.toml'
foreach ($path in @($prepareScript, $archiveScript, $PSCommandPath, $manifest)) {
    Assert-PackedPlainPath $path
    if (!(Test-Path -LiteralPath $path -PathType Leaf)) { throw "Required build input missing: $path" }
}
$sourceBefore = Get-PackedSourceSnapshot $PythonPath $prepareScript $sourceRoot $SourceRevision
Assert-PackedSameSource $sourceBefore $sourceBefore
# New-Item without -Force refuses an existing directory. Failures leave this
# evidence directory for inspection; subsequent runs need a different output.
New-Item -ItemType Directory -Path $OutputRoot -ErrorAction Stop | Out-Null
$started = [DateTimeOffset]::UtcNow
$buildLog = Join-Path $OutputRoot 'cargo-build.log'
$command = @('+1.95.0', 'build', '--locked', '--release', '--manifest-path', $manifest,
    '--bin', 'mir2-platform-windows', '--target', 'x86_64-pc-windows-msvc', '--target-dir', $TargetDir)
$receipt = [ordered]@{
    schema = 'mir2.windows.packed-native-build.v1'; passed = $false
    sourceRevision = $SourceRevision; sourceBefore = $sourceBefore; sourceAfter = $null
    startedUtc = $started.ToString('o'); completedUtc = $null
    outputRoot = $OutputRoot; targetDir = $TargetDir; target = 'x86_64-pc-windows-msvc'; toolchain = '1.95.0'
    command = [ordered]@{ executable = $cargo; arguments = $command; workingDirectory = $project }
    compilerEnvironment = $compilerEnvironment; packedRelease = '1'; remapPathPrefixAdded = $false
    python = $PythonPath; cargoVersion = $null; rustcVersion = $null; buildExitCode = $null
    tools = @(@($PSCommandPath, $prepareScript, $archiveScript) | ForEach-Object {
        [ordered]@{path = $_; sha256 = (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash}
    })
    executable = $null; buildLog = $null
    scope = 'native executable only; no resources expanded, v4 Candidate packaging, CMS, launch or publication'
    reproducibleBuild = $false
    provenanceLimit = 'Clean committed source bytes do not prove a hermetic or reproducible build; Cargo caches, SDK and host environment remain build inputs.'
}
$oldPackedRelease = [Environment]::GetEnvironmentVariable('MIR2_PACKED_RELEASE', 'Process')
$hadPackedRelease = Test-Path Env:MIR2_PACKED_RELEASE
try {
    $env:MIR2_PACKED_RELEASE = '1'
    try {
        Push-Location -LiteralPath $project
        try {
            $version = @(& $cargo '+1.95.0' '--version' 2>&1)
            if ($LASTEXITCODE -ne 0) { throw 'Pinned Cargo toolchain is unavailable' }
            $receipt.cargoVersion = $version -join [Environment]::NewLine
            $rustc = (Get-Command rustc -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
            $version = @(& $rustc '+1.95.0' '--version' '--verbose' 2>&1)
            if ($LASTEXITCODE -ne 0) { throw 'Pinned rustc toolchain is unavailable' }
            $receipt.rustcVersion = $version -join [Environment]::NewLine
            & $cargo @command 2>&1 | Tee-Object -FilePath $buildLog
            $receipt.buildExitCode = $LASTEXITCODE
        } finally { Pop-Location }
    } finally {
        $receipt.sourceAfter = Get-PackedSourceSnapshot $PythonPath $prepareScript $sourceRoot $SourceRevision
        Assert-PackedSameSource $sourceBefore $receipt.sourceAfter
    }
    if ($receipt.buildExitCode -ne 0) { throw "Native release build failed (exit $($receipt.buildExitCode))" }
    $built = Join-Path $TargetDir 'x86_64-pc-windows-msvc/release/mir2-platform-windows.exe'
    $receipt.executable = Copy-PackedExecutable $PythonPath $prepareScript $built (Join-Path $OutputRoot 'mir2-platform-windows.exe')
    $receipt.passed = $true
} catch {
    $receipt['error'] = $_.Exception.Message
    throw
} finally {
    if ($hadPackedRelease) {
        [Environment]::SetEnvironmentVariable('MIR2_PACKED_RELEASE', $oldPackedRelease, 'Process')
    } else { Remove-Item Env:MIR2_PACKED_RELEASE -ErrorAction SilentlyContinue }
    $receipt.completedUtc = [DateTimeOffset]::UtcNow.ToString('o')
    if (Test-Path -LiteralPath $buildLog -PathType Leaf) {
        $receipt.buildLog = [ordered]@{path = 'cargo-build.log'; size = (Get-Item -LiteralPath $buildLog).Length
            sha256 = (Get-FileHash -LiteralPath $buildLog -Algorithm SHA256).Hash}
    }
    Write-PackedNewJson (Join-Path $OutputRoot 'BUILD-PACKED-NATIVE.json') $receipt
}
$receipt | ConvertTo-Json -Depth 20
