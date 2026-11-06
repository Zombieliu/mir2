function Test-EntityAtlasPathWithin {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Root
    )
    $rootFull = [IO.Path]::GetFullPath($Root).TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    $pathFull = [IO.Path]::GetFullPath($Path)
    return $pathFull.StartsWith($rootFull + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)
}

function Assert-EntityAtlasClosure {
    param(
        [Parameter(Mandatory = $true)][string]$ManifestPath,
        [Parameter(Mandatory = $true)][string]$AssetRoot
    )

    $manifestFull = [IO.Path]::GetFullPath($ManifestPath)
    $assetRootFull = [IO.Path]::GetFullPath($AssetRoot)
    if (-not (Test-Path -LiteralPath $manifestFull -PathType Leaf)) {
        throw "entity atlas manifest missing: $manifestFull"
    }
    if (-not (Test-Path -LiteralPath $assetRootFull -PathType Container)) {
        throw "entity atlas asset root missing: $assetRootFull"
    }

    try {
        $manifest = Get-Content -LiteralPath $manifestFull -Raw | ConvertFrom-Json
    }
    catch {
        throw "entity atlas manifest is invalid JSON: $manifestFull"
    }
    if ([int]$manifest.schemaVersion -ne 2 -or [string]$manifest.kind -cne 'mir2-bevy-entity-atlas-manifest') {
        throw 'entity atlas manifest schema/kind mismatch'
    }
    $atlases = @($manifest.atlases)
    if ($atlases.Count -eq 0) { throw 'entity atlas manifest contains no atlases' }

    Add-Type -AssemblyName System.Drawing
    $seenUrls = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $pageCount = 0
    foreach ($atlas in $atlases) {
        $atlasKey = [string]$atlas.key
        if ([string]::IsNullOrWhiteSpace($atlasKey)) { throw 'entity atlas key is empty' }
        $pages = @($atlas.pages)
        if ($pages.Count -eq 0) { throw "entity atlas '$atlasKey' contains no integrity-bound pages" }

        foreach ($rect in @($atlas.rects)) {
            $pageIndex = if ($null -eq $rect.PSObject.Properties['pageIndex']) { 0 } else { [int]$rect.pageIndex }
            if ($pageIndex -lt 0 -or $pageIndex -ge $pages.Count) {
                throw "entity atlas '$atlasKey' rect '$($rect.key)' references invalid pageIndex $pageIndex"
            }
        }

        foreach ($page in $pages) {
            $imageUrl = [string]$page.imageUrl
            $expectedWidth = [int]$page.width
            $expectedHeight = [int]$page.height
            $expectedBytes = [int64]$page.imageBytes
            $expectedHash = ([string]$page.sha256).ToLowerInvariant()
            if ($imageUrl -notmatch '^/bevy-entity-atlases/[A-Za-z0-9._-]+\.png$') {
                throw "entity atlas '$atlasKey' page URL is unsafe: $imageUrl"
            }
            if (-not $seenUrls.Add($imageUrl)) { throw "duplicate entity atlas page URL: $imageUrl" }
            if ($expectedWidth -le 0 -or $expectedHeight -le 0 -or $expectedBytes -le 0) {
                throw "entity atlas '$atlasKey' page metadata is not positive: $imageUrl"
            }
            if ($expectedHash -notmatch '^[0-9a-f]{64}$') {
                throw "entity atlas '$atlasKey' page SHA-256 is invalid: $imageUrl"
            }

            $relative = $imageUrl.TrimStart('/').Replace('/', [IO.Path]::DirectorySeparatorChar)
            $pagePath = [IO.Path]::GetFullPath((Join-Path $assetRootFull $relative))
            if (-not (Test-EntityAtlasPathWithin -Path $pagePath -Root $assetRootFull)) {
                throw "entity atlas page escapes asset root: $imageUrl"
            }
            if (-not (Test-Path -LiteralPath $pagePath -PathType Leaf)) {
                throw "entity atlas page missing: $imageUrl"
            }
            $item = Get-Item -LiteralPath $pagePath
            if ([int64]$item.Length -ne $expectedBytes) {
                throw "entity atlas page byte count mismatch: $imageUrl"
            }
            $actualHash = (Get-FileHash -LiteralPath $pagePath -Algorithm SHA256).Hash.ToLowerInvariant()
            if ($actualHash -cne $expectedHash) {
                throw "entity atlas page SHA-256 mismatch: $imageUrl"
            }

            $image = $null
            try {
                $image = [Drawing.Image]::FromFile($pagePath)
                if ([int]$image.Width -ne $expectedWidth -or [int]$image.Height -ne $expectedHeight) {
                    throw "entity atlas page dimensions mismatch: $imageUrl"
                }
                if ([string]$image.RawFormat.Guid -cne [string][Drawing.Imaging.ImageFormat]::Png.Guid) {
                    throw "entity atlas page is not a decodable PNG: $imageUrl"
                }
            }
            catch {
                throw "entity atlas page decode failed for ${imageUrl}: $($_.Exception.Message)"
            }
            finally {
                if ($null -ne $image) { $image.Dispose() }
            }
            $pageCount += 1
        }
    }

    return [pscustomobject]@{
        atlasCount = $atlases.Count
        pageCount = $pageCount
    }
}
