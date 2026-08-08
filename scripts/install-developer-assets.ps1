[CmdletBinding()]
param(
    [string]$ManifestPath = "",
    [string]$PartsDirectory = "",
    [string]$CacheDirectory = "",
    [switch]$Download,
    [switch]$Force,
    [switch]$KeepArchive
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ProjectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$WebRoot = Join-Path $ProjectRoot "apps\web"
$PublicRoot = Join-Path $WebRoot "public"
$ArchiveInspector = Join-Path $WebRoot "scripts\asset-pipeline\inspect-ustar-archive.mjs"
$ClosureVerifier = Join-Path $WebRoot "scripts\asset-pipeline\verify-full-pack-closure.mjs"
$Destination = [System.IO.Path]::GetFullPath((Join-Path $PublicRoot "generated\crystal-packs\full"))
$ExpectedDestination = [System.IO.Path]::GetFullPath((Join-Path $PublicRoot "generated\crystal-packs\full"))

function Assert-SafeChildPath {
    param([string]$Path, [string]$Parent, [string]$Label)

    $ResolvedPath = [System.IO.Path]::GetFullPath($Path)
    $ResolvedParent = [System.IO.Path]::GetFullPath($Parent).TrimEnd("\", "/")
    $Prefix = "$ResolvedParent$([System.IO.Path]::DirectorySeparatorChar)"
    if (-not $ResolvedPath.StartsWith($Prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "$Label is outside the expected parent directory: $ResolvedPath"
    }
    return $ResolvedPath
}

function Assert-SafeFileName {
    param([string]$Name, [string]$Label)

    if (-not $Name -or $Name -ne [System.IO.Path]::GetFileName($Name) -or $Name -match "[\\/]") {
        throw "$Label must be a plain file name: $Name"
    }
}

function Test-AssetPart {
    param([string]$Path, [object]$Part)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $false
    }
    if ((Get-Item -LiteralPath $Path).Length -ne [long]$Part.size) {
        return $false
    }
    $Hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
    return $Hash -eq [string]$Part.sha256
}

if (-not $ManifestPath) {
    $ManifestPath = Join-Path $ProjectRoot "config\developer-assets.json"
}
$ManifestPath = [System.IO.Path]::GetFullPath($ManifestPath)
if (-not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
    throw "Developer asset manifest is missing: $ManifestPath"
}

$Manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
if ($Manifest.kind -ne "mir2-developer-asset-bundle" -or $Manifest.schemaVersion -ne 1) {
    throw "Unsupported developer asset manifest: $ManifestPath"
}
if ([string]$Manifest.destination -ne "mir2-web3/apps/web/public/generated/crystal-packs/full") {
    throw "Unexpected developer asset destination in manifest: $($Manifest.destination)"
}
if ([string]$Manifest.contentHash -notmatch "^[a-f0-9]{64}$") {
    throw "Developer asset manifest has an invalid contentHash."
}
if ([string]$Manifest.repository -notmatch "^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$") {
    throw "Developer asset manifest has an invalid GitHub repository."
}
if ([string]$Manifest.releaseTag -notmatch "^[A-Za-z0-9][A-Za-z0-9._-]*$") {
    throw "Developer asset manifest has an invalid release tag."
}
if (
    [string]$Manifest.archive.format -ne "ustar" -or
    [long]$Manifest.archive.size -le 0 -or
    [string]$Manifest.archive.sha256 -notmatch "^[a-f0-9]{64}$"
) {
    throw "Developer asset manifest has invalid archive metadata."
}
if (@($Manifest.parts).Count -eq 0) {
    throw "Developer asset manifest contains no archive parts."
}
Assert-SafeFileName -Name ([string]$Manifest.archive.name) -Label "Archive name"
$SeenPartNames = @{}
foreach ($Part in $Manifest.parts) {
    $PartName = [string]$Part.name
    Assert-SafeFileName -Name $PartName -Label "Archive part name"
    if ($SeenPartNames.ContainsKey($PartName)) {
        throw "Developer asset manifest contains a duplicate part: $PartName"
    }
    if ([long]$Part.size -le 0 -or [string]$Part.sha256 -notmatch "^[a-f0-9]{64}$") {
        throw "Developer asset manifest contains invalid metadata for part: $PartName"
    }
    $SeenPartNames[$PartName] = $true
}

$ExistingIndex = Join-Path $Destination "index.json"
if (Test-Path -LiteralPath $ExistingIndex -PathType Leaf) {
    $Existing = Get-Content -LiteralPath $ExistingIndex -Raw | ConvertFrom-Json
    if ([string]$Existing.contentHash -eq [string]$Manifest.contentHash) {
        $PreviousErrorActionPreference = $ErrorActionPreference
        try {
            $ErrorActionPreference = "Continue"
            & node.exe $ClosureVerifier `
                --root $Destination `
                --expectedContentHash ([string]$Manifest.contentHash) `
                --verifyPages true 2>$null | Out-Null
            $ExistingVerificationExitCode = $LASTEXITCODE
        }
        finally {
            $ErrorActionPreference = $PreviousErrorActionPreference
        }
        if ($ExistingVerificationExitCode -eq 0) {
            Write-Host "Full Crystal pack is already installed and verified: $($Manifest.contentHash)"
            exit 0
        }
        Write-Warning "The installed full Crystal pack is incomplete or corrupt; reinstalling the pinned bundle."
    }
    elseif (-not $Force) {
        throw "A different full Crystal pack is installed. Re-run with -Force to replace it."
    }
}

if (-not $CacheDirectory) {
    $CacheDirectory = Join-Path $ProjectRoot ".mir2-data\developer-assets\$($Manifest.releaseTag)"
}
$CacheDirectory = [System.IO.Path]::GetFullPath($CacheDirectory)
New-Item -ItemType Directory -Path $CacheDirectory -Force | Out-Null

if ($Download) {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
        throw "GitHub CLI is required for -Download. Install it, run 'gh auth login', and retry."
    }
    foreach ($Part in $Manifest.parts) {
        $Target = Join-Path $CacheDirectory ([string]$Part.name)
        if (Test-AssetPart -Path $Target -Part $Part) {
            Write-Host "[assets] verified cached part: $($Part.name)"
            continue
        }
        if (Test-Path -LiteralPath $Target -PathType Leaf) {
            $SafeTarget = Assert-SafeChildPath `
                -Path $Target `
                -Parent $CacheDirectory `
                -Label "Corrupt cached part"
            Write-Warning "Removing incomplete or corrupt cached part: $($Part.name)"
            Remove-Item -LiteralPath $SafeTarget -Force
        }
        Write-Host "[assets] downloading: $($Part.name)"
        & gh release download ([string]$Manifest.releaseTag) `
            --repo ([string]$Manifest.repository) `
            --pattern ([string]$Part.name) `
            --dir $CacheDirectory
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to download $($Part.name) from GitHub Release."
        }
        if (-not (Test-AssetPart -Path $Target -Part $Part)) {
            throw "Downloaded asset bundle part failed size or SHA-256 verification: $Target"
        }
    }
    $PartsDirectory = $CacheDirectory
}
elseif (-not $PartsDirectory) {
    $PartsDirectory = Split-Path -Parent $ManifestPath
}
$PartsDirectory = [System.IO.Path]::GetFullPath($PartsDirectory)

foreach ($Part in $Manifest.parts) {
    $PartPath = Join-Path $PartsDirectory ([string]$Part.name)
    if (-not (Test-Path -LiteralPath $PartPath -PathType Leaf)) {
        throw "Asset bundle part is missing: $PartPath"
    }
    $Info = Get-Item -LiteralPath $PartPath
    if ($Info.Length -ne [long]$Part.size) {
        throw "Asset bundle part size mismatch: $PartPath"
    }
    $Hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $PartPath).Hash.ToLowerInvariant()
    if ($Hash -ne [string]$Part.sha256) {
        throw "Asset bundle part hash mismatch: $PartPath"
    }
}

$ArchivePath = Join-Path $CacheDirectory ([string]$Manifest.archive.name)
$OutputStream = [System.IO.File]::Create($ArchivePath)
try {
    $Buffer = New-Object byte[] (8MB)
    foreach ($Part in $Manifest.parts) {
        $PartPath = Join-Path $PartsDirectory ([string]$Part.name)
        $InputStream = [System.IO.File]::OpenRead($PartPath)
        try {
            while (($Read = $InputStream.Read($Buffer, 0, $Buffer.Length)) -gt 0) {
                $OutputStream.Write($Buffer, 0, $Read)
            }
        }
        finally {
            $InputStream.Dispose()
        }
    }
}
finally {
    $OutputStream.Dispose()
}

$ArchiveInfo = Get-Item -LiteralPath $ArchivePath
if ($ArchiveInfo.Length -ne [long]$Manifest.archive.size) {
    throw "Reassembled archive size mismatch: $ArchivePath"
}
$ArchiveHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $ArchivePath).Hash.ToLowerInvariant()
if ($ArchiveHash -ne [string]$Manifest.archive.sha256) {
    throw "Reassembled archive hash mismatch: $ArchivePath"
}

& node.exe $ArchiveInspector `
    --archive $ArchivePath `
    --prefix "generated/crystal-packs/full"
if ($LASTEXITCODE -ne 0) {
    throw "Asset archive safety inspection failed with exit code $LASTEXITCODE"
}

$InstallId = [Guid]::NewGuid().ToString("N")
$StagingRoot = Assert-SafeChildPath `
    -Path (Join-Path $PublicRoot ".mir2-asset-install-$InstallId") `
    -Parent $PublicRoot `
    -Label "Asset staging directory"
$StagedDestination = Join-Path $StagingRoot "generated\crystal-packs\full"
$BackupDestination = Assert-SafeChildPath `
    -Path (Join-Path (Split-Path -Parent $ExpectedDestination) ".full-backup-$InstallId") `
    -Parent $PublicRoot `
    -Label "Asset backup directory"

New-Item -ItemType Directory -Path $StagingRoot -Force | Out-Null
try {
    & tar.exe -xf $ArchivePath -C $StagingRoot
    if ($LASTEXITCODE -ne 0) {
        throw "Asset extraction failed with exit code $LASTEXITCODE"
    }

    $StagedIndexPath = Join-Path $StagedDestination "index.json"
    if (-not (Test-Path -LiteralPath $StagedIndexPath -PathType Leaf)) {
        throw "Extracted asset bundle does not contain its full-pack index."
    }
    $StagedIndex = Get-Content -LiteralPath $StagedIndexPath -Raw | ConvertFrom-Json
    if ([string]$StagedIndex.contentHash -ne [string]$Manifest.contentHash) {
        throw "Extracted full Crystal pack content hash does not match the bundle manifest."
    }
    & node.exe $ClosureVerifier `
        --root $StagedDestination `
        --expectedContentHash ([string]$Manifest.contentHash) `
        --verifyPages true
    if ($LASTEXITCODE -ne 0) {
        throw "Extracted full Crystal pack closure verification failed with exit code $LASTEXITCODE"
    }

    $HadExistingDestination = Test-Path -LiteralPath $ExpectedDestination
    try {
        if ($HadExistingDestination) {
            Move-Item -LiteralPath $ExpectedDestination -Destination $BackupDestination
        }
        Move-Item -LiteralPath $StagedDestination -Destination $ExpectedDestination
    }
    catch {
        if ($HadExistingDestination -and -not (Test-Path -LiteralPath $ExpectedDestination) -and (Test-Path -LiteralPath $BackupDestination)) {
            Move-Item -LiteralPath $BackupDestination -Destination $ExpectedDestination
        }
        throw
    }

    if (Test-Path -LiteralPath $BackupDestination) {
        Remove-Item -LiteralPath $BackupDestination -Recurse -Force
    }
}
finally {
    if (Test-Path -LiteralPath $StagingRoot) {
        Remove-Item -LiteralPath $StagingRoot -Recurse -Force
    }
}

if (-not $KeepArchive) {
    $ResolvedArchive = [System.IO.Path]::GetFullPath($ArchivePath)
    if (-not $ResolvedArchive.StartsWith($CacheDirectory, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove archive outside cache directory: $ResolvedArchive"
    }
    Remove-Item -LiteralPath $ResolvedArchive -Force
}

Write-Host "Full Crystal pack installed: $Destination"
Write-Host "Content hash: $($Manifest.contentHash)"
