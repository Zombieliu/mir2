[CmdletBinding()]
param(
    [string]$OutputDirectory = "",
    [ValidateRange(104857600, 1932735283)]
    [long]$PartSizeBytes = 1610612736,
    [switch]$KeepArchive
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ProjectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$WebRoot = Join-Path $ProjectRoot "apps\web"
$PublicRoot = Join-Path $WebRoot "public"
$FullPackRoot = Join-Path $PublicRoot "generated\crystal-packs\full"
$IndexPath = Join-Path $FullPackRoot "index.json"

if (-not (Test-Path -LiteralPath $IndexPath -PathType Leaf)) {
    throw "Full Crystal pack is missing. Build it with npm run assets:full-pack:build first."
}

Push-Location $WebRoot
try {
    & npm.cmd run assets:full-pack:verify
    if ($LASTEXITCODE -ne 0) {
        throw "Full Crystal pack verification failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

$Index = Get-Content -LiteralPath $IndexPath -Raw | ConvertFrom-Json
$ContentHash = [string]$Index.contentHash
if ($ContentHash -notmatch "^[a-f0-9]{64}$") {
    throw "Full Crystal pack index has an invalid contentHash."
}

$ReleaseTag = "developer-assets-$($ContentHash.Substring(0, 12))"
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $ProjectRoot "dist\developer-assets\$ReleaseTag"
}
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
if (Get-ChildItem -LiteralPath $OutputDirectory -Force | Select-Object -First 1) {
    throw "Output directory must be empty: $OutputDirectory"
}

$ArchiveName = "mir2-crystal-full-pack-$($ContentHash.Substring(0, 12)).tar"
$ArchivePath = Join-Path $OutputDirectory $ArchiveName
Write-Host "[assets] creating archive $ArchivePath"
& node.exe `
    (Join-Path $WebRoot "scripts\asset-pipeline\package-full-pack-archive.mjs") `
    --root $FullPackRoot `
    --output $ArchivePath `
    --expectedContentHash $ContentHash
if ($LASTEXITCODE -ne 0) {
    throw "Deterministic full-pack archive creation failed with exit code $LASTEXITCODE"
}

$ArchiveHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $ArchivePath).Hash.ToLowerInvariant()
$Parts = @()
$InputStream = [System.IO.File]::OpenRead($ArchivePath)
try {
    $Buffer = New-Object byte[] (8MB)
    $PartNumber = 1
    while ($InputStream.Position -lt $InputStream.Length) {
        $PartName = "$ArchiveName.part$($PartNumber.ToString('000'))"
        $PartPath = Join-Path $OutputDirectory $PartName
        $OutputStream = [System.IO.File]::Create($PartPath)
        try {
            [long]$Written = 0
            while ($Written -lt $PartSizeBytes -and $InputStream.Position -lt $InputStream.Length) {
                $Remaining = [Math]::Min([long]$Buffer.Length, $PartSizeBytes - $Written)
                $Read = $InputStream.Read($Buffer, 0, [int]$Remaining)
                if ($Read -le 0) {
                    break
                }
                $OutputStream.Write($Buffer, 0, $Read)
                $Written += $Read
            }
        }
        finally {
            $OutputStream.Dispose()
        }

        $PartInfo = Get-Item -LiteralPath $PartPath
        $Parts += [ordered]@{
            name = $PartName
            size = $PartInfo.Length
            sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $PartPath).Hash.ToLowerInvariant()
        }
        Write-Host "[assets] wrote $PartName ($($PartInfo.Length) bytes)"
        $PartNumber += 1
    }
}
finally {
    $InputStream.Dispose()
}

$LibraryCount = @(Get-ChildItem -LiteralPath (Join-Path $FullPackRoot "libraries") -File -Recurse -Filter "*.json").Count
$PageCount = @(Get-ChildItem -LiteralPath (Join-Path $FullPackRoot "pages") -File -Recurse -Filter "*.png").Count
$Manifest = [ordered]@{
    schemaVersion = 1
    kind = "mir2-developer-asset-bundle"
    repository = "Zombieliu/mir2"
    releaseTag = $ReleaseTag
    contentHash = $ContentHash
    sourceContentHash = [string]$Index.sourceContentHash
    destination = "mir2-web3/apps/web/public/generated/crystal-packs/full"
    archive = [ordered]@{
        name = $ArchiveName
        size = (Get-Item -LiteralPath $ArchivePath).Length
        sha256 = $ArchiveHash
        format = "ustar"
    }
    summary = [ordered]@{
        libraryCount = $LibraryCount
        uniquePageCount = $PageCount
        partCount = $Parts.Count
    }
    parts = $Parts
}

$ManifestPath = Join-Path $OutputDirectory "developer-assets.json"
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($ManifestPath, ($Manifest | ConvertTo-Json -Depth 8), $Utf8NoBom)

if (-not $KeepArchive) {
    $ResolvedArchive = [System.IO.Path]::GetFullPath($ArchivePath)
    if (-not $ResolvedArchive.StartsWith($OutputDirectory, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove archive outside output directory: $ResolvedArchive"
    }
    Remove-Item -LiteralPath $ResolvedArchive -Force
}

Write-Host "[assets] bundle manifest: $ManifestPath"
Write-Host "[assets] release tag: $ReleaseTag"
