# Crystal library exports are pinned to independently checked source libraries and
# export metadata. Never follow metadata paths or require a Crystal installation.

function Get-PetGuildAssetIdentities {
    return @(
        [pscustomobject]@{ version=3; library='Pet/00'; count=368; sourceBytes=348008; sourceSha256='31fb134a57463368ebf9e74012fa6e2fdbc7aa58d02f3ce5250e4f62afc9343a'; metaSha256='e1d99cb93a298c03c2f98463cc6a0c2748c923e3faad9d37062af9e261543f99' },
        [pscustomobject]@{ version=3; library='Pet/01'; count=432; sourceBytes=252086; sourceSha256='3e151cacbe3c4209b9bb778f622ee442c9de45b086b7003bb1769e065e28537a'; metaSha256='303b6f02cdd90c16c7934571f9d1d68c8a6d360e3bc491bab77ee20764356a42' },
        [pscustomobject]@{ version=3; library='Pet/02'; count=360; sourceBytes=277561; sourceSha256='77c75e884c6b7843fbb372a699c369cd13a3588ca468ee7eb38f8b98a2d09c21'; metaSha256='d7cf92ab3ce71ffc940618b0e65ca7f27eafc01f6ececc59d814bd3f8f51a0a1' },
        [pscustomobject]@{ version=3; library='Pet/03'; count=440; sourceBytes=360851; sourceSha256='6d900de4590614b9ed97297950ac50e3f9f5d672127e5f9e4292bf294f9afb1f'; metaSha256='b499aad937c833a0f199577195f96108709a2ee0d38377c2aee1768415ade9f8' },
        [pscustomobject]@{ version=3; library='Pet/04'; count=320; sourceBytes=323775; sourceSha256='98edb8553d8ec5a7f48517f0f2bd71c62fad359847f077ebffd9f0938523c698'; metaSha256='8587cefc44c0066e60f21f8768ba2afc1f24f94bd4e5b271bd5ab155982a7796' },
        [pscustomobject]@{ version=3; library='Pet/05'; count=312; sourceBytes=119860; sourceSha256='56b6e6d96e3eba32fda0c7fca2f99058b61ef30dc6b610e887268a573e8d01f7'; metaSha256='a234137b165af60ba65ed78fa77a40ce737c4172b2d2aaa6f0f1c0d290628fd3' },
        [pscustomobject]@{ version=3; library='Pet/06'; count=360; sourceBytes=309951; sourceSha256='a41f6e74396423ab39926e8cf93f703f6b7ce5398852cdf53cbe21e04063113f'; metaSha256='b6a886cc119685adeba35232e9044d62bc57dea6e144e7a04af7e14089dfd6f2' },
        [pscustomobject]@{ version=3; library='Pet/07'; count=280; sourceBytes=576267; sourceSha256='5c506dc778a68c407d24372b417be75d0637792486eadc22b7b293fe1dec496a'; metaSha256='ed9649d68feb72543b30c0df88a0843abe4480c71f6a8ca4c2015b0e91c7ff0b' },
        [pscustomobject]@{ version=3; library='Pet/08'; count=312; sourceBytes=581462; sourceSha256='b5c1af7d32a0cf228373f8d48b35bdb81bf8d4fffb9badde40c334d5b592bd1f'; metaSha256='1438ebdbfbda73d6a51b26c5bc39a1dee76a789b3e2a0c16d8785cb49aa0610a' },
        [pscustomobject]@{ version=3; library='Pet/09'; count=219; sourceBytes=541375; sourceSha256='a0183677860dfc0cbd80d14d29aa9a267ad2ca2f5b29f9d495a01dab8e2ea41f'; metaSha256='1d25ece405ac9e096d424cafe5c276584911fe9b52ef06597ea11d1b9b159ea8' },
        [pscustomobject]@{ version=3; library='Pet/10'; count=336; sourceBytes=482933; sourceSha256='3500c3c2db54576e57643ef2a52f82ab98e1c997dd3c8a2006649589bd5406b0'; metaSha256='6b7a668742d752d1e28a3d154317c6ba4e74f3844b239651ed63c2dae7d34adf' },
        [pscustomobject]@{ version=3; library='Pet/11'; count=368; sourceBytes=572210; sourceSha256='90d742082ed003b42e200c152aa6065a77105fad1215c859636dc9ea91a58dc9'; metaSha256='eaa76a86d2f48667b116c971f37ec146f9b708c3a325ab4c4b96768dc7ebc7ac' },
        [pscustomobject]@{ version=3; library='Pet/12'; count=376; sourceBytes=685473; sourceSha256='86044cb3389741f19142d3c180995fc195467e017c049fbefb5bb5f113ac7402'; metaSha256='6075d25930fbe1745081465eccffb6f1da95641f10e83ceca47a0e8fe9cdeb38' },
        [pscustomobject]@{ version=3; library='Pet/13'; count=416; sourceBytes=852042; sourceSha256='0ab6fe9e97dd4ea1d1e1f8084525d33f57401c92cf3861158600480fa98deb8b'; metaSha256='35449b042dc24c3d86fb07f7f36899d5b74d8984b33d6f7a8d4b8f971a061614' },
        [pscustomobject]@{ version=3; library='Pet/14'; count=416; sourceBytes=1213166; sourceSha256='08d55cb112c6d50c63382ca233a0a74fe9f9af523fbdb58ab0ef89ecc9b4d0b9'; metaSha256='2b801998a7ea65ce78974140db77cf872b3e93744503c664e4525295d05e33ed' },
        [pscustomobject]@{ version=2; library='GuildSkill'; count=48; sourceBytes=65609; sourceSha256='31f1897c00258270ea2b3c9404e93650d952c35c0278f57acf6de8c6636cbf5b'; metaSha256='10a295247d74501b736a0fabdcfcf7b2083bdeeee94d5dc0433cc95ad944d38e' },
        [pscustomobject]@{ version=3; library='MagIcon2'; count=224; sourceBytes=345567; sourceSha256='0875fc38fa58a101f09346374479de712661f672ef39e6c0ea9679d59aad2a4e'; metaSha256='9f588c506b6711111cf477725105d1ec0bde1968a562d4648a10b6831b749372' },
        [pscustomobject]@{ version=3; library='MagIcon'; count=224; sourceBytes=199927; sourceSha256='5f49bf35158f680852a66833ec3c706ad045448ef1526d1f7fdd116807de3030'; metaSha256='40c8a1d8d1674b031538754ff0b63abdf9f3d16dbe8bff34aec50e0749a66952' },
        [pscustomobject]@{ version=2; library='BuffIcon'; count=265; sourceBytes=148746; sourceSha256='326be33f0ebe2e25777f47959cae84ad191ba19cefe7d8e685006f8761f8dfb8'; metaSha256='c9b1b239cfb5161a078a473e542ba996b4ec4162d096231a18ceb00b9c507537' }
    )
}

function Get-PetSoundIdentities {
    return @(
        [pscustomobject]@{ name='pet_blackdragon.wav'; bytes=118060; sha256='8253e843bc25c3f66e545dc994a83ace223f539ee0f473b73e3aaba5cf88594f' },
        [pscustomobject]@{ name='pet_chick.wav'; bytes=51954; sha256='72b1998c1796a42ee5cd41dd8c315630cec98a36f5c636b6671242d32202c603' },
        [pscustomobject]@{ name='pet_frog.wav'; bytes=1187928; sha256='887ce5b02a615550926ff1c331f9908e0c05752f3e2a4e7f07ee67a63fc47beb' },
        [pscustomobject]@{ name='pet_kitty.wav'; bytes=79788; sha256='c73bbf3e3b4fdf88904d4eeaef57b32ee9c14941c85c8695a673549719f15865' },
        [pscustomobject]@{ name='pet_monkey.wav'; bytes=113616; sha256='7f7c65e56f1cd5d570e8dbf176a6de76c1de4e1aff4256fe3c94cc88e2c73fc4' },
        [pscustomobject]@{ name='pet_olympicmascot.wav'; bytes=44156; sha256='6440636cb32bb15ab68b3761009e2358bcd17732f418c96d249c0967679d1866' },
        [pscustomobject]@{ name='pet_pickup.wav'; bytes=14030; sha256='769f1e02c231fd1b31350dd79d83d6b9d7b103ffdd7b89ae941489959d49a777' },
        [pscustomobject]@{ name='pet_pig.wav'; bytes=62344; sha256='34d22859920789e0ff33ea7df025d8985e79f294514caf40f9e452e2713b22c8' },
        [pscustomobject]@{ name='pet_pigman.wav'; bytes=46480; sha256='66a188857ec3d8754a8537a6a6dabe706568d0c4ffa8a7ecefecb424d2308cde' },
        [pscustomobject]@{ name='pet_skeleton.wav'; bytes=57572; sha256='824d270bbd3db0781cde41176620da7b99c6814f9cd3b0c5225f1dc105023aed' },
        [pscustomobject]@{ name='pet_weman.wav'; bytes=37346; sha256='e0cb7ec6e3ea6b91d4c744d78a93423f81d49b77b1534fac22b82f77a1fbab22' }
    )
}

function Get-PetGuildRequiredFiles {
    foreach ($identity in Get-PetGuildAssetIdentities) {
        "original-ui/$($identity.library)/meta.json"
        for ($index = 0; $index -lt $identity.count; $index++) {
            "original-ui/$($identity.library)/$index.png"
        }
    }
    foreach ($identity in Get-PetSoundIdentities) { "original-ui/Sound/$($identity.name)" }
    foreach ($identity in Get-SkillBarFrameIdentities) {
        "original-ui/$($identity.library)/meta.json"
        foreach ($index in $identity.indices) { "original-ui/$($identity.library)/$index.png" }
    }
}

function Assert-PetGuildLibrary {
    param([string]$OriginalUiRoot, [object]$Identity, [object]$FrameCatalog)
    # Identity comes only from the checked-in table, never from package JSON.
    $root = Join-Path $OriginalUiRoot $Identity.library
    $metaPath = Join-Path $root 'meta.json'
    if (-not (Test-Path -LiteralPath $metaPath -PathType Leaf)) { throw "pet/guild metadata missing: $metaPath" }
    if ((Get-FileHash -LiteralPath $metaPath -Algorithm SHA256).Hash -ine $Identity.metaSha256) {
        throw "pet/guild metadata identity mismatch: $metaPath"
    }
    $meta = Get-Content -LiteralPath $metaPath -Raw | ConvertFrom-Json
    if ($meta.version -ne $Identity.version -or $meta.count -ne $Identity.count -or @($meta.frames).Count -ne $Identity.count -or
        $meta.sourceLibrary.path -cne ($Identity.library + '.Lib') -or
        $meta.sourceLibrary.sha256 -ine $Identity.sourceSha256 -or $meta.sourceLibrary.bytes -ne $Identity.sourceBytes) {
        throw "pet/guild source identity mismatch: $metaPath"
    }
    if ($Identity.library.StartsWith('Pet/', [StringComparison]::Ordinal)) {
        $entry = $FrameCatalog.libraries.PSObject.Properties[$Identity.library]
        if ($null -eq $entry -or $entry.Value.version -ne 3 -or $entry.Value.sourceSha256 -ine $Identity.sourceSha256 -or
            $entry.Value.actionCount -ne $meta.frameSet.count -or
            (ConvertTo-Json -InputObject $entry.Value.actions -Depth 12 -Compress) -cne
            (ConvertTo-Json -InputObject $meta.frameSet.actions -Depth 12 -Compress)) {
            throw "pet frame-set identity mismatch: $($Identity.library)"
        }
    }
    $seen = [Collections.Generic.HashSet[int]]::new()
    foreach ($frame in $meta.frames) {
        $index = [int]$frame.index
        if ($index -lt 0 -or $index -ge $Identity.count -or -not $seen.Add($index) -or
            $frame.path -cne "/original-ui/$($Identity.library)/$index.png") {
            throw "pet/guild frame index or path mismatch: $metaPath"
        }
        $pngPath = Join-Path $root "$index.png"
        if (-not (Test-Path -LiteralPath $pngPath -PathType Leaf)) { throw "pet/guild PNG missing: $pngPath" }
        if ((Get-FileHash -LiteralPath $pngPath -Algorithm SHA256).Hash -ine $frame.pngSha256) {
            throw "pet/guild PNG identity mismatch: $pngPath"
        }
    }
    # No extra nested files or image variants can escape the fixed inventory.
    $files = @(Get-ChildItem -LiteralPath $root -File -Recurse -Force)
    if ($files.Count -ne $Identity.count + 1) { throw "pet/guild unexpected file count: $root" }
}

function Assert-PetSoundClosure {
    param([string]$OriginalUiRoot)
    foreach ($identity in Get-PetSoundIdentities) {
        $path = Join-Path $OriginalUiRoot ('Sound/' + $identity.name)
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "pet sound missing: $path" }
        if ((Get-Item -LiteralPath $path).Length -ne $identity.bytes -or
            (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ine $identity.sha256) {
            throw "pet sound identity mismatch: $path"
        }
    }
}

function Assert-PetGuildAssetClosure {
    param([Parameter(Mandatory = $true)][string]$OriginalUiRoot)
    foreach ($relative in @('Pet', 'GuildSkill', 'MagIcon2', 'MagIcon', 'BuffIcon', 'frame-sets.generated.json') + @((Get-PetSoundIdentities) | ForEach-Object { 'Sound/' + $_.name })) {
        $path = Join-Path $OriginalUiRoot $relative
        Assert-NoReparseTree -Path $path
        Assert-NoAlternateDataStreams -Path $path
    }
    $catalog = Get-Content -LiteralPath (Join-Path $OriginalUiRoot 'frame-sets.generated.json') -Raw | ConvertFrom-Json
    foreach ($identity in Get-PetGuildAssetIdentities) {
        Assert-PetGuildLibrary -OriginalUiRoot $OriginalUiRoot -Identity $identity -FrameCatalog $catalog
    }
    $petDirectories = @(Get-ChildItem -LiteralPath (Join-Path $OriginalUiRoot 'Pet') -Directory -Force)
    if ($petDirectories.Count -ne 15) { throw 'pet library count mismatch' }
    Assert-PetSoundClosure -OriginalUiRoot $OriginalUiRoot
    Assert-SkillBarFrames -OriginalUiRoot $OriginalUiRoot
    $identities = @(Get-PetGuildAssetIdentities)
    $frames = @(Get-SkillBarFrameIdentities)
    $pngCount = ($identities | Measure-Object -Property count -Sum).Sum
    foreach ($frame in $frames) { $pngCount += @($frame.indices | Select-Object -Unique).Count }
    return [pscustomobject]@{ libraries = $identities.Count + $frames.Count; png = [int]$pngCount; sounds = @(Get-PetSoundIdentities).Count }
}

function Test-PetGuildAssetClosure {
    param([string]$OriginalUiRoot, [string]$TemporaryRoot)
    $closure = Assert-PetGuildAssetClosure -OriginalUiRoot $OriginalUiRoot
    $probe = Join-Path $TemporaryRoot 'pet-guild-closure-probe'
    New-Item -ItemType Directory -Path (Join-Path $probe 'Pet') -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $OriginalUiRoot 'Pet/00') -Destination (Join-Path $probe 'Pet/00') -Recurse
    Copy-Item -LiteralPath (Join-Path $OriginalUiRoot 'GuildSkill') -Destination (Join-Path $probe 'GuildSkill') -Recurse
    Copy-Item -LiteralPath (Join-Path $OriginalUiRoot 'MagIcon2') -Destination (Join-Path $probe 'MagIcon2') -Recurse
    Copy-Item -LiteralPath (Join-Path $OriginalUiRoot 'MagIcon') -Destination (Join-Path $probe 'MagIcon') -Recurse
    Copy-Item -LiteralPath (Join-Path $OriginalUiRoot 'BuffIcon') -Destination (Join-Path $probe 'BuffIcon') -Recurse
    New-Item -ItemType Directory -Path (Join-Path $probe 'Sound') -Force | Out-Null
    foreach ($sound in Get-PetSoundIdentities) {
        Copy-Item -LiteralPath (Join-Path $OriginalUiRoot ('Sound/' + $sound.name)) -Destination (Join-Path $probe ('Sound/' + $sound.name))
    }
    $catalog = Get-Content -LiteralPath (Join-Path $OriginalUiRoot 'frame-sets.generated.json') -Raw | ConvertFrom-Json
    $identities = @(Get-PetGuildAssetIdentities)
    foreach ($identity in @($identities[0], $identities[15], $identities[16], $identities[17], $identities[18])) {
        Assert-PetGuildLibrary -OriginalUiRoot $probe -Identity $identity -FrameCatalog $catalog
        $png = Join-Path $probe ($identity.library + '/0.png')
        $bytes = [IO.File]::ReadAllBytes($png)
        # The test temporary root is supplied by the caller's existing safe cleanup.
        [IO.File]::Move($png, $png + '.missing')
        $rejected = $false
        try { Assert-PetGuildLibrary -OriginalUiRoot $probe -Identity $identity -FrameCatalog $catalog } catch { if ($_.Exception.Message -like '*PNG missing:*') { $rejected = $true } else { throw } }
        [IO.File]::Move($png + '.missing', $png)
        if (-not $rejected) { throw 'pet/guild self-test accepted missing PNG' }
        [IO.File]::WriteAllBytes($png, [byte[]](1,2,3))
        $rejected = $false
        try { Assert-PetGuildLibrary -OriginalUiRoot $probe -Identity $identity -FrameCatalog $catalog } catch { if ($_.Exception.Message -like '*PNG identity mismatch:*') { $rejected = $true } else { throw } }
        [IO.File]::WriteAllBytes($png, $bytes)
        if (-not $rejected) { throw 'pet/guild self-test accepted replaced PNG' }
        $metaPath = Join-Path $probe ($identity.library + '/meta.json')
        $metaBytes = [IO.File]::ReadAllBytes($metaPath)
        $tampered = Get-Content -LiteralPath $metaPath -Raw | ConvertFrom-Json
        $tampered.sourceLibrary.sha256 = '0' * 64
        [IO.File]::WriteAllText($metaPath, ($tampered | ConvertTo-Json -Depth 20))
        $rejected = $false
        try { Assert-PetGuildLibrary -OriginalUiRoot $probe -Identity $identity -FrameCatalog $catalog } catch { if ($_.Exception.Message -like '*metadata identity mismatch:*') { $rejected = $true } else { throw } }
        [IO.File]::WriteAllBytes($metaPath, $metaBytes)
        if (-not $rejected) { throw 'pet/guild self-test accepted self-certified metadata' }
    }
    $catalog.libraries.'Pet/00'.actions[0].interval++
    $rejected = $false
    try { Assert-PetGuildLibrary -OriginalUiRoot $probe -Identity $identities[0] -FrameCatalog $catalog } catch { if ($_.Exception.Message -like '*frame-set identity mismatch:*') { $rejected = $true } else { throw } }
    if (-not $rejected) { throw 'pet self-test accepted frame-set drift' }
    foreach ($sound in Get-PetSoundIdentities) {
        $path = Join-Path $probe ('Sound/' + $sound.name)
        $bytes = [IO.File]::ReadAllBytes($path)
        $tampered = [byte[]]$bytes.Clone(); $tampered[$tampered.Length - 1] = $tampered[$tampered.Length - 1] -bxor 1
        [IO.File]::WriteAllBytes($path, $tampered)
        $rejected = $false
        try { Assert-PetSoundClosure -OriginalUiRoot $probe } catch { if ($_.Exception.Message -like '*pet sound identity mismatch:*') { $rejected = $true } else { throw } }
        [IO.File]::WriteAllBytes($path, $bytes)
        if (-not $rejected) { throw "pet self-test accepted replaced sound: $($sound.name)" }
    }
    Test-SkillBarFrames -OriginalUiRoot $OriginalUiRoot -ProbeRoot $probe
    Write-Host "PET_GUILD_CLOSURE_SELFTEST=passed ($($closure.png) PNG, $($closure.libraries) metadata, 15 Pet frame sets, $($closure.sounds) sounds; mutation rejection checks passed)"
}

# Sparse original UI libraries: pin the full trusted metadata before consulting
# selected frame hashes. Indices and paths come from this source, never metadata.
function Get-SkillBarFrameIdentities {
    @(
        [pscustomobject]@{ library='Title'; indices=@(156,157,158,287,288,289,516,517,560,561,562,563,564,565); sourceBytes=5617784; sourceSha256='bd3e9485548e9b3cb5d4cb261c9a01a07752fb41fcea7f5d8569e602feaed648'; metaSha256='197d9346fde28aecc0d65d1326a8816ae8173b6d1d58e8ff8d0f4f033b1154dc' },
        [pscustomobject]@{ library='Prguse'; indices=@(710,1656,1657,1658,1428,1429,1921,1934,1943,1946,2190,2193,2247,2105) + @(2110..2113) + @(2122..2127) + @(2131..2137) + @(2140..2162); sourceBytes=12414386; sourceSha256='584b67694a420a29c98f51b04cdf681acf1ed071c15c57014fdfd59c762a3c96'; metaSha256='b01dfd909aecab822b791a9d9ef6329c24871898a063184a9042a45b83a12a2f' },
        [pscustomobject]@{ library='Prguse2'; indices=@(1260..1282) + @(1290..1324) + @(7,8,9) + @(20..33); sourceBytes=3557648; sourceSha256='510ebc77315089075fc472c9ef745be2e16c6dece970c307d6859ce074cce861'; metaSha256='4e3f749fd52abc2f4f36527d375611ec60b02417cca24deb24f566b9a3213e71' }
    )
}
function Assert-SkillBarFrames {
    param([string]$OriginalUiRoot)
    foreach ($identity in Get-SkillBarFrameIdentities) {
        $root = Join-Path $OriginalUiRoot $identity.library
        Assert-NoReparseTree -Path $root
        Assert-NoAlternateDataStreams -Path $root
        $metaPath = Join-Path $root 'meta.json'
        if (-not (Test-Path -LiteralPath $metaPath -PathType Leaf)) { throw "skillbar metadata missing: $metaPath" }
        if ((Get-FileHash -LiteralPath $metaPath -Algorithm SHA256).Hash -ine $identity.metaSha256) { throw "skillbar metadata identity mismatch: $metaPath" }
        $meta = Get-Content -LiteralPath $metaPath -Raw | ConvertFrom-Json
        if ($meta.version -ne 3 -or $meta.sourceLibrary.path -cne ($identity.library + '.Lib') -or
            $meta.sourceLibrary.sha256 -ine $identity.sourceSha256 -or $meta.sourceLibrary.bytes -ne $identity.sourceBytes) { throw "skillbar source identity mismatch: $metaPath" }
        foreach ($index in $identity.indices) {
            $frame = @($meta.frames | Where-Object { $_.index -eq $index })
            if ($frame.Count -ne 1 -or $frame[0].path -cne "/original-ui/$($identity.library)/$index.png") { throw "skillbar frame mismatch: $metaPath" }
            $png = Join-Path $root "$index.png"
            if (-not (Test-Path -LiteralPath $png -PathType Leaf)) { throw "skillbar PNG missing: $png" }
            if ((Get-FileHash -LiteralPath $png -Algorithm SHA256).Hash -ine $frame[0].pngSha256) { throw "skillbar PNG identity mismatch: $png" }
        }
    }
}
function Test-SkillBarFrames {
    param([string]$OriginalUiRoot, [string]$ProbeRoot)
    foreach ($identity in Get-SkillBarFrameIdentities) {
        $destination = Join-Path $ProbeRoot $identity.library
        New-Item -ItemType Directory -Path $destination -Force | Out-Null
        foreach ($name in @('meta.json') + @($identity.indices | ForEach-Object { "$_.png" })) {
            Copy-Item -LiteralPath (Join-Path (Join-Path $OriginalUiRoot $identity.library) $name) -Destination (Join-Path $destination $name)
        }
    }
    Assert-SkillBarFrames -OriginalUiRoot $ProbeRoot
    foreach ($identity in Get-SkillBarFrameIdentities) {
        $png = Join-Path (Join-Path $ProbeRoot $identity.library) "$($identity.indices[0]).png"
        $bytes = [IO.File]::ReadAllBytes($png)
        [IO.File]::Move($png, $png + '.missing')
        $rejected = $false
        try { Assert-SkillBarFrames -OriginalUiRoot $ProbeRoot } catch { if ($_.Exception.Message -like '*skillbar PNG missing:*') { $rejected = $true } else { throw } }
        [IO.File]::Move($png + '.missing', $png)
        if (-not $rejected) { throw 'skillbar self-test accepted missing PNG' }
        [IO.File]::WriteAllBytes($png, [byte[]](1,2,3))
        $rejected = $false
        try { Assert-SkillBarFrames -OriginalUiRoot $ProbeRoot } catch { if ($_.Exception.Message -like '*skillbar PNG identity mismatch:*') { $rejected = $true } else { throw } }
        [IO.File]::WriteAllBytes($png, $bytes)
        if (-not $rejected) { throw 'skillbar self-test accepted changed PNG' }
        $meta = Join-Path (Join-Path $ProbeRoot $identity.library) 'meta.json'
        $bytes = [IO.File]::ReadAllBytes($meta)
        [IO.File]::WriteAllBytes($meta, [byte[]](123,125))
        $rejected = $false
        try { Assert-SkillBarFrames -OriginalUiRoot $ProbeRoot } catch { if ($_.Exception.Message -like '*skillbar metadata identity mismatch:*') { $rejected = $true } else { throw } }
        [IO.File]::WriteAllBytes($meta, $bytes)
        if (-not $rejected) { throw 'skillbar self-test accepted changed metadata' }
    }
}
