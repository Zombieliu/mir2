[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ProjectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$DataRoot = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot ".mir2-data"))
$TestRoot = [System.IO.Path]::GetFullPath((Join-Path $DataRoot "installer-test-$([Guid]::NewGuid().ToString('N'))"))
$SafePrefix = "$($DataRoot.TrimEnd('\', '/'))$([System.IO.Path]::DirectorySeparatorChar)"

if (-not $TestRoot.StartsWith($SafePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to create fixture outside .mir2-data: $TestRoot"
}

function Write-JsonFile {
    param([string]$Path, [object]$Value)

    $Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 8), $Utf8NoBom)
}

try {
    $FixtureProject = Join-Path $TestRoot "project"
    $FixtureScripts = Join-Path $FixtureProject "scripts"
    $FixturePublic = Join-Path $FixtureProject "apps\web\public"
    $FixtureAssetScripts = Join-Path $FixtureProject "apps\web\scripts\asset-pipeline"
    $FixtureParts = Join-Path $TestRoot "parts"
    $FixtureCache = Join-Path $TestRoot "cache"
    $FixturePack = Join-Path $TestRoot "full-pack"
    $ExistingPack = Join-Path $FixturePublic "generated\crystal-packs\full"

    New-Item -ItemType Directory -Path @(
        $FixtureScripts,
        $FixtureAssetScripts,
        $FixturePublic,
        $FixtureParts,
        $FixtureCache,
        (Join-Path $FixturePack "libraries\fixture"),
        $ExistingPack
    ) -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot "install-developer-assets.ps1") -Destination $FixtureScripts
    foreach ($ScriptName in @(
        "full-pack-closure.mjs",
        "inspect-ustar-archive.mjs",
        "verify-full-pack-closure.mjs"
    )) {
        Copy-Item `
            -LiteralPath (Join-Path $ProjectRoot "apps\web\scripts\asset-pipeline\$ScriptName") `
            -Destination $FixtureAssetScripts
    }

    $ContentHash = "a" * 64
    $PageBytes = [System.Text.Encoding]::UTF8.GetBytes("fixture-page")
    $Sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $PageHash = ([System.BitConverter]::ToString($Sha256.ComputeHash($PageBytes))).Replace("-", "").ToLowerInvariant()
    }
    finally {
        $Sha256.Dispose()
    }
    $PageRelativePath = "pages\$($PageHash.Substring(0, 2))\$PageHash.png"
    $PagePath = Join-Path $FixturePack $PageRelativePath
    New-Item -ItemType Directory -Path (Split-Path -Parent $PagePath) -Force | Out-Null
    [System.IO.File]::WriteAllBytes($PagePath, $PageBytes)

    $LibraryRelativePath = "libraries\fixture\fixture.json"
    $LibraryPath = Join-Path $FixturePack $LibraryRelativePath
    Write-JsonFile -Path $LibraryPath -Value ([ordered]@{
        libraryKey = "Fixture/00"
        pages = @([ordered]@{
            id = "p0"
            key = "sha256:$PageHash"
            sha256 = $PageHash
            imageUrl = "/generated/crystal-packs/full/$($PageRelativePath.Replace('\', '/'))"
            networkBytes = $PageBytes.Length
        })
    })
    $LibraryHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $LibraryPath).Hash.ToLowerInvariant()
    Write-JsonFile -Path (Join-Path $FixturePack "index.json") -Value ([ordered]@{
        schemaVersion = 1
        kind = "mir2-crystal-full-pack-index"
        contentHash = $ContentHash
        sourceContentHash = "c" * 64
        summary = [ordered]@{ libraryCount = 1 }
        libraries = @([ordered]@{
            key = "Fixture/00"
            pageCount = 1
            manifestUrl = "/generated/crystal-packs/full/$($LibraryRelativePath.Replace('\', '/'))"
            shardUrl = "/generated/crystal-packs/full/$($LibraryRelativePath.Replace('\', '/'))"
            manifestSha256 = $LibraryHash
        })
    })
    Write-JsonFile -Path (Join-Path $ExistingPack "index.json") -Value ([ordered]@{
        # A matching index with missing shards/pages must be repaired, not trusted.
        contentHash = $ContentHash
    })

    $ArchivePath = Join-Path $TestRoot "fixture.tar"
    & node.exe `
        (Join-Path $ProjectRoot "apps\web\scripts\asset-pipeline\package-full-pack-archive.mjs") `
        --root $FixturePack `
        --output $ArchivePath `
        --expectedContentHash $ContentHash
    if ($LASTEXITCODE -ne 0) {
        throw "Fixture archive creation failed with exit code $LASTEXITCODE"
    }
    $RepeatArchivePath = Join-Path $TestRoot "fixture-repeat.tar"
    & node.exe `
        (Join-Path $ProjectRoot "apps\web\scripts\asset-pipeline\package-full-pack-archive.mjs") `
        --root $FixturePack `
        --output $RepeatArchivePath `
        --expectedContentHash $ContentHash | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Repeated fixture archive creation failed with exit code $LASTEXITCODE"
    }
    $ArchiveHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $ArchivePath).Hash
    $RepeatArchiveHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $RepeatArchivePath).Hash
    if ($ArchiveHash -ne $RepeatArchiveHash) {
        throw "Deterministic archive hashes differ: $ArchiveHash != $RepeatArchiveHash"
    }

    $UnsafeArchivePath = Join-Path $TestRoot "fixture-symlink.tar"
    $UnsafeBytes = [System.IO.File]::ReadAllBytes($ArchivePath)
    $UnsafeBytes[156] = [byte][char]"2"
    for ($Index = 148; $Index -lt 156; $Index += 1) {
        $UnsafeBytes[$Index] = 0x20
    }
    $Checksum = 0
    for ($Index = 0; $Index -lt 512; $Index += 1) {
        $Checksum += $UnsafeBytes[$Index]
    }
    $ChecksumBytes = [System.Text.Encoding]::ASCII.GetBytes([Convert]::ToString($Checksum, 8).PadLeft(6, "0"))
    [System.Array]::Copy($ChecksumBytes, 0, $UnsafeBytes, 148, 6)
    $UnsafeBytes[154] = 0
    $UnsafeBytes[155] = 0x20
    [System.IO.File]::WriteAllBytes($UnsafeArchivePath, $UnsafeBytes)
    $UnsafeStdout = Join-Path $TestRoot "unsafe-inspector.out.log"
    $UnsafeStderr = Join-Path $TestRoot "unsafe-inspector.err.log"
    $UnsafeProcess = Start-Process `
        -FilePath "node.exe" `
        -ArgumentList @(
            (Join-Path $ProjectRoot "apps\web\scripts\asset-pipeline\inspect-ustar-archive.mjs"),
            "--archive",
            $UnsafeArchivePath,
            "--prefix",
            "generated/crystal-packs/full"
        ) `
        -WindowStyle Hidden `
        -RedirectStandardOutput $UnsafeStdout `
        -RedirectStandardError $UnsafeStderr `
        -Wait `
        -PassThru
    if ($UnsafeProcess.ExitCode -eq 0) {
        throw "Archive inspector accepted a synthetic symlink entry."
    }

    $PartName = "fixture.tar.part001"
    $PartPath = Join-Path $FixtureParts $PartName
    Copy-Item -LiteralPath $ArchivePath -Destination $PartPath
    $ArchiveInfo = Get-Item -LiteralPath $ArchivePath
    $PartInfo = Get-Item -LiteralPath $PartPath
    $ManifestPath = Join-Path $TestRoot "developer-assets.json"
    Write-JsonFile -Path $ManifestPath -Value ([ordered]@{
        schemaVersion = 1
        kind = "mir2-developer-asset-bundle"
        repository = "Zombieliu/mir2"
        releaseTag = "fixture"
        contentHash = $ContentHash
        destination = "mir2-web3/apps/web/public/generated/crystal-packs/full"
        archive = [ordered]@{
            name = "fixture.tar"
            size = $ArchiveInfo.Length
            sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $ArchivePath).Hash.ToLowerInvariant()
            format = "ustar"
        }
        parts = @([ordered]@{
            name = $PartName
            size = $PartInfo.Length
            sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $PartPath).Hash.ToLowerInvariant()
        })
    })

    & (Join-Path $FixtureScripts "install-developer-assets.ps1") `
        -ManifestPath $ManifestPath `
        -PartsDirectory $FixtureParts `
        -CacheDirectory $FixtureCache

    $InstalledIndex = Get-Content -LiteralPath (Join-Path $ExistingPack "index.json") -Raw | ConvertFrom-Json
    if ([string]$InstalledIndex.contentHash -ne $ContentHash) {
        throw "Fixture install content hash mismatch."
    }
    if (-not (Test-Path -LiteralPath (Join-Path $ExistingPack $PageRelativePath) -PathType Leaf)) {
        throw "Fixture install page is missing."
    }
    if (Get-ChildItem -LiteralPath (Split-Path -Parent $ExistingPack) -Directory -Filter ".full-backup-*" | Select-Object -First 1) {
        throw "Fixture install left a backup directory behind."
    }

    Write-Host "Developer asset installer fixture passed."
}
finally {
    if (Test-Path -LiteralPath $TestRoot) {
        $ResolvedTestRoot = [System.IO.Path]::GetFullPath($TestRoot)
        if (-not $ResolvedTestRoot.StartsWith($SafePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove fixture outside .mir2-data: $ResolvedTestRoot"
        }
        Remove-Item -LiteralPath $ResolvedTestRoot -Recurse -Force
    }
}
