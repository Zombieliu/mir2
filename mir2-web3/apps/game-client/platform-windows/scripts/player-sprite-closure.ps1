# Validate the individual Crystal player libraries used when a frame is not in
# the prebuilt entity atlas. The runtime consumes each library's meta.json and
# matching PNG, so Candidate packaging must keep those pairs atomic.

function Get-PlayerSpritePngDimensions {
    param([Parameter(Mandatory = $true)][string]$Path)

    $bytes = New-Object byte[] 24
    $stream = [IO.File]::OpenRead($Path)
    try {
        if ($stream.Read($bytes, 0, $bytes.Length) -ne $bytes.Length) {
            throw "PNG header is truncated: $Path"
        }
    } finally {
        $stream.Dispose()
    }
    $signature = @(137, 80, 78, 71, 13, 10, 26, 10)
    for ($index = 0; $index -lt $signature.Count; $index++) {
        if ([int]$bytes[$index] -ne $signature[$index]) {
            throw "PNG signature mismatch: $Path"
        }
    }
    if ([Text.Encoding]::ASCII.GetString($bytes, 12, 4) -cne 'IHDR') {
        throw "PNG is missing an IHDR header: $Path"
    }
    $width = ([uint64]$bytes[16] * 16777216) + ([uint64]$bytes[17] * 65536) + ([uint64]$bytes[18] * 256) + [uint64]$bytes[19]
    $height = ([uint64]$bytes[20] * 16777216) + ([uint64]$bytes[21] * 65536) + ([uint64]$bytes[22] * 256) + [uint64]$bytes[23]
    if ($width -le 0 -or $height -le 0 -or $width -gt [uint32]::MaxValue -or $height -gt [uint32]::MaxValue) {
        throw "PNG dimensions are invalid: $Path"
    }
    return [pscustomobject]@{ width = [uint32]$width; height = [uint32]$height }
}

function Assert-PlayerSpriteClosure {
    param(
        [Parameter(Mandatory = $true)][string]$OriginalUiRoot,
        [Parameter(Mandatory = $true)][string[]]$FamilyNames
    )

    if (-not (Test-Path -LiteralPath $OriginalUiRoot -PathType Container)) {
        throw "original-ui root missing: $OriginalUiRoot"
    }
    $libraryCount = 0
    $frameCount = 0
    foreach ($familyName in $FamilyNames) {
        if ($familyName -notmatch '^[A-Za-z][A-Za-z0-9_-]*$') {
            throw "invalid player sprite family: $familyName"
        }
        $familyRoot = Join-Path $OriginalUiRoot $familyName
        if (-not (Test-Path -LiteralPath $familyRoot -PathType Container)) {
            throw "player sprite family missing: $familyRoot"
        }
        $libraries = @(Get-ChildItem -LiteralPath $familyRoot -Directory -Force)
        if ($libraries.Count -eq 0) {
            throw "player sprite family has no exported libraries: $familyRoot"
        }
        foreach ($library in $libraries) {
            $libraryCount++
            $metaPath = Join-Path $library.FullName 'meta.json'
            if (-not (Test-Path -LiteralPath $metaPath -PathType Leaf)) {
                throw "player sprite metadata missing: $metaPath"
            }
            try {
                $payload = Get-Content -LiteralPath $metaPath -Raw | ConvertFrom-Json
            } catch {
                throw "player sprite metadata is invalid JSON: $metaPath"
            }
            $frames = @($payload.frames)
            if ($frames.Count -eq 0 -or [int64]$payload.count -lt $frames.Count) {
                throw "player sprite metadata count is invalid: $metaPath"
            }
            $seen = [Collections.Generic.HashSet[int64]]::new()
            foreach ($frame in $frames) {
                foreach ($field in @('index', 'width', 'height', 'x', 'y', 'path')) {
                    if ($null -eq $frame.PSObject.Properties[$field]) {
                        throw "player sprite frame is missing $field in $metaPath"
                    }
                }
                $index = [int64]$frame.index
                $width = [uint64]$frame.width
                $height = [uint64]$frame.height
                if ($index -lt 0 -or $width -le 0 -or $height -le 0 -or -not $seen.Add($index)) {
                    throw "player sprite frame identity is invalid in $metaPath"
                }
                $expectedPath = "/original-ui/$familyName/$($library.Name)/$index.png"
                if ([string]$frame.path -cne $expectedPath) {
                    throw "player sprite frame path mismatch in ${metaPath}: $($frame.path)"
                }
                $pngPath = Join-Path $library.FullName "$index.png"
                if (-not (Test-Path -LiteralPath $pngPath -PathType Leaf)) {
                    throw "player sprite frame PNG missing: $pngPath"
                }
                $dimensions = Get-PlayerSpritePngDimensions -Path $pngPath
                if ([uint64]$dimensions.width -ne $width -or [uint64]$dimensions.height -ne $height) {
                    throw "player sprite frame dimensions mismatch: $pngPath"
                }
                $frameCount++
            }
            foreach ($png in Get-ChildItem -LiteralPath $library.FullName -File -Filter '*.png' -Force) {
                if ($png.BaseName -notmatch '^\d+$') {
                    throw "non-numeric player sprite PNG rejected: $($png.FullName)"
                }
            }
        }
    }
    return [pscustomobject]@{ familyCount = $FamilyNames.Count; libraryCount = $libraryCount; frameCount = $frameCount }
}

function Get-CandidateFishingEffectFrames {
    return [ordered]@{
        'original-effects/Effect' = @(640..645) + @(650..655) + @(665..676)
        'original-effects/Prguse' = @(1350..1351)
    }
}

function Get-CandidateFishingSoundIdentities {
    return @(
        [pscustomobject]@{ name='fishing_throw.wav'; size=43742; sha256='a011c7a62ab99d51a46999980eaa107cbba062d6e60a532049a722858a34fc8f' }
        [pscustomobject]@{ name='fishing_pull.wav'; size=46132; sha256='44f4c0a27e733f8ae150f1c9e3f399c78b0da2e9f4b4afc6a003708be405aff8' }
        [pscustomobject]@{ name='fishing.wav'; size=20696; sha256='630bd3c2d02b5f6b2572117810dff33c7701b82b7ab6e72c525616d3e4eb79e3' }
    )
}

function Get-CandidateFishingRequiredFiles {
    foreach ($rod in @('00', '01')) {
        "original-ui/Fishing/$rod/meta.json"
        foreach ($index in 0..351) { "original-ui/Fishing/$rod/$index.png" }
    }
    foreach ($effect in (Get-CandidateFishingEffectFrames).GetEnumerator()) {
        "$($effect.Key)/meta.json"
        foreach ($index in $effect.Value) { "$($effect.Key)/$index.png" }
    }
    foreach ($sound in Get-CandidateFishingSoundIdentities) { "original-ui/Sound/$($sound.name)" }
}

function Assert-CandidateFishingAssetClosure {
    param([Parameter(Mandatory = $true)][string]$AssetRoot)

    $rods = Assert-PlayerSpriteClosure -OriginalUiRoot (Join-Path $AssetRoot 'original-ui') -FamilyNames @('Fishing')
    if ($rods.libraryCount -ne 2 -or $rods.frameCount -ne 704) { throw 'Fishing rod metadata must cover both complete libraries' }

    foreach ($effect in (Get-CandidateFishingEffectFrames).GetEnumerator()) {
        $meta = Get-Content -LiteralPath (Join-Path $AssetRoot "$($effect.Key)/meta.json") -Raw | ConvertFrom-Json
        foreach ($index in $effect.Value) {
            $relative = "$($effect.Key)/$index.png"
            $property = $meta.frames.PSObject.Properties[[string]$index]
            if ($null -eq $property -or $property.Value.path -cne "/$relative") { throw "Fishing effect metadata missing: $relative" }
            $dimensions = Get-PlayerSpritePngDimensions -Path (Join-Path $AssetRoot $relative)
            if ($dimensions.width -ne $property.Value.width -or $dimensions.height -ne $property.Value.height) { throw "Fishing effect dimensions mismatch: $relative" }
        }
    }

    foreach ($sound in Get-CandidateFishingSoundIdentities) {
        $soundPath = Join-Path $AssetRoot "original-ui/Sound/$($sound.name)"
        if ((Get-Item -LiteralPath $soundPath).Length -ne $sound.size -or
            (Get-FileHash -LiteralPath $soundPath -Algorithm SHA256).Hash -ine $sound.sha256) { throw "Fishing sound identity mismatch: $soundPath" }
    }
}
