# Shared release configuration and resource closure for the invited 0-30 and Sabuk build.
# This file does not sign, publish, mutate saves, or relax the Candidate gates.

function Resolve-CandidateGatewayWsUrl {
    param([string]$Value, [switch]$AllowFixtureHost)

    # Keep the accepted input narrower than URI/TOML syntax. In particular no
    # userinfo, query tokens, fragments, escapes, whitespace or TOML injection.
    if ($Value -cnotmatch '\Awss://(?:[A-Za-z0-9.-]+|\[[0-9A-Fa-f:]+\])(?::[0-9]{1,5})?/[A-Za-z0-9/_.-]*\z') {
        throw 'GatewayWsUrl must be an explicit wss:// address with a plain path and no credentials, query, fragment or escapes'
    }
    $uri = $null
    if (-not [Uri]::TryCreate($Value, [UriKind]::Absolute, [ref]$uri) -or
        $uri.Scheme -cne 'wss' -or $uri.Port -le 0 -or
        $uri.UserInfo.Length -ne 0 -or $uri.Query.Length -ne 0 -or $uri.Fragment.Length -ne 0 -or
        $Value -match '/\.{1,2}(?:/|$)') {
        throw 'GatewayWsUrl is not a valid remote WSS endpoint'
    }
    $hostName = $uri.DnsSafeHost.TrimEnd('.').ToLowerInvariant()
    $ip = $null
    if ([Net.IPAddress]::TryParse($hostName.Trim('[', ']'), [ref]$ip)) {
        if ([Net.IPAddress]::IsLoopback($ip) -or $ip.Equals([Net.IPAddress]::Any) -or $ip.Equals([Net.IPAddress]::IPv6Any)) {
            throw 'GatewayWsUrl must not point testers at their own computer'
        }
    } elseif ([Uri]::CheckHostName($hostName) -ne [UriHostNameType]::Dns -or -not $hostName.Contains('.')) {
        throw 'GatewayWsUrl must use a valid remote DNS name or IP address'
    }
    $reserved = $hostName -match '(^|\.)(example|invalid|test|localhost)$|(^|\.)example\.(com|org|net)$|^(192\.0\.2\.|198\.51\.100\.|203\.0\.113\.)|^2001:db8:'
    if ($reserved -and -not $AllowFixtureHost) {
        throw 'GatewayWsUrl is a reserved placeholder; a real test-server endpoint is required'
    }
    return $uri.AbsoluteUri
}

function New-CandidateClientConfiguration {
    param([string]$GatewayWsUrl, [string]$QuestGuidance = 'newcomer-v2', [bool]$ForceDaylight = $true)
    $endpoint = Resolve-CandidateGatewayWsUrl -Value $GatewayWsUrl
    if (@('crystal', 'newcomer-v1', 'newcomer-v2') -cnotcontains $QuestGuidance) { throw 'unsupported Candidate quest guidance profile' }
    $daylight = if ($ForceDaylight) { 'true' } else { 'false' }
    return "# Candidate client configuration; credentials are forbidden.`n[server]`ngateway_ws_url = `"$endpoint`"`n[display]`nwidth = 1024`nheight = 768`nforce_daylight = $daylight`n[gameplay]`nquest_guidance = `"$QuestGuidance`"`n"
}

function Assert-CandidateClientConfiguration {
    param([string]$Text)
    $values = @{}
    $section = ''
    foreach ($rawLine in ($Text -split '\r?\n')) {
        $line = $rawLine.Trim()
        if ($line.Length -eq 0 -or $line.StartsWith('#')) { continue }
        if ($line -cmatch '^\[(server|display|gameplay)\]$') { $section = $Matches[1]; continue }
        if ($line -cnotmatch '^([a-z_]+)\s*=\s*(.+)$' -or $section.Length -eq 0) { throw 'invalid Candidate client configuration syntax' }
        $key = $section + '.' + $Matches[1]
        if ($values.ContainsKey($key)) { throw "duplicate Candidate client configuration key: $key" }
        $values[$key] = $Matches[2]
    }
    $expected = @('server.gateway_ws_url', 'display.width', 'display.height', 'display.force_daylight', 'gameplay.quest_guidance')
    if ($values.Count -ne $expected.Count) { throw 'Candidate client configuration contains missing or unexpected keys' }
    foreach ($key in $expected) { if (-not $values.ContainsKey($key)) { throw "Candidate client configuration missing $key" } }
    if ($values['server.gateway_ws_url'] -cnotmatch '^"([^"]+)"$') { throw 'Candidate gateway must be a quoted WSS address' }
    $endpoint = Resolve-CandidateGatewayWsUrl -Value $Matches[1]
    if ($values['gameplay.quest_guidance'] -cnotmatch '^"(crystal|newcomer-v1|newcomer-v2)"$') { throw 'unsupported Candidate gameplay profile' }
    $profile = $Matches[1]
    if ($values['display.width'] -cne '1024' -or $values['display.height'] -cne '768' -or $values['display.force_daylight'] -cnotmatch '^(true|false)$') {
        throw 'Candidate display configuration must use 1024x768 and an explicit daylight boolean'
    }
    return [pscustomobject]@{ gatewayWsUrl = $endpoint; questGuidance = $profile; forceDaylight = ($values['display.force_daylight'] -ceq 'true') }
}

function Get-CandidateRequiredMapNames {
    # V2 main quests; the ordinary Travis supply route (0108 -> 0109); the
    # D605 graduation route (0 -> 2 -> 3 -> D601 -> D607 -> D602 -> D605).
    # Keep the previously shipped 0141 resource closure too.
    return @('0', '1', '2', '3', '0108', '0109', '0141', 'd001', 'd021', 'd022', 'd401', 'd601', 'd602', 'd605', 'd607')
}

function Resolve-CandidateMapNames {
    param([string[]]$MapNames)
    $result = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($name in $MapNames) {
        if ($name -cnotmatch '^[A-Za-z0-9_-]+$') { throw 'Candidate map names must be safe map identifiers' }
        [void]$result.Add($name.ToLowerInvariant())
    }
    foreach ($required in Get-CandidateRequiredMapNames) {
        if (-not $result.Contains($required)) { throw "Candidate map coverage missing required journey/supply/entrance map: $required" }
    }
    return @($result | Sort-Object)
}

function Get-CandidateDefaultMapNames {
    # Default native terrain coverage includes Bichon town and BorderVillage
    # merchants, the Bichon siege registrar (0122), Sabuk palace (0150), and
    # the five adjacent Sabuk interiors (0151-0155). The public approach is map
    # 3, already in the journey minimum. Keep the minimum separate so explicit
    # custom lists retain their existing validation and are not expanded silently.
    $merchantMaps = @('0101', '0102', '0103', '0104', '0105', '0106', '0107', '0125', '0132', '0140')
    $siegeMaps = @('0122', '0150', '0151', '0152', '0153', '0154', '0155')
    return @(Resolve-CandidateMapNames -MapNames (@(Get-CandidateRequiredMapNames) + $merchantMaps + $siegeMaps))
}

function Get-CandidateActorLibraryNames {
    # Exact exported classic-profile libraries as of the September 2026
    # newcomer candidate. Requiring every metadata file detects a silently
    # omitted entire library, which per-existing-directory checks cannot do.
    $monsters = '000,003,004,005,006,007,008,009,010,011,012,013,014,015,016,017,018,019,020,021,022,023,024,025,026,027,029,030,031,032,033,034,035,036,037,038,039,040,041,042,043,044,045,046,047,048,051,052,053,054,055,056,057,058,059,060,061,062,063,064,065,066,067,068,069,070,071,072,073,074,078,081,082,083,084,085,087,088,090,091,092,094,095,097,098,099,100,103,104,105,106,107,108,109,110,111,112,114,139,152,153,154,155,163,378'
    foreach ($name in $monsters.Split(',')) { "Monster/$name" }
    foreach ($name in '00,01,03,04,05,06,07,08,09,11,15,16,25,27,45,52,83'.Split(',')) { "NPC/$name" }
    foreach ($name in @('00','01','02','03')) { "Gate/$name" }
}

function Test-CandidateActorFileAllowed {
    param([string]$RelativePath)
    return $RelativePath -cmatch '^mir2-assets/original-ui/(?:(?:Monster/[0-9]{3}|NPC/[0-9]{2}|Gate/0[0-3])/|MapLinkIcon/)(?:meta\.json|(?:0|[1-9][0-9]*)(?:\.mask)?\.png)$'
}

function Test-CandidateActorDirectoryAllowed {
    param([string]$RelativePath)
    return $RelativePath -cmatch '^mir2-assets/original-ui/(?:Monster(?:/[0-9]{3})?|NPC(?:/[0-9]{2})?|Gate(?:/0[0-3])?|MapLinkIcon)$'
}

function Assert-CandidateSpriteLibraryClosure {
    param([string]$OriginalUiRoot, [string]$Library)
    $libraryRoot = Join-Path $OriginalUiRoot $Library
    $metaPath = Join-Path $libraryRoot 'meta.json'
    if (-not (Test-Path -LiteralPath $metaPath -PathType Leaf)) { throw "Candidate sprite metadata missing: $Library" }
    $meta = Get-Content -LiteralPath $metaPath -Raw | ConvertFrom-Json
    $frames = @($meta.frames)
    if ($frames.Count -eq 0 -or [int64]$meta.count -lt $frames.Count) { throw "Candidate sprite metadata count invalid: $Library" }
    $expectedFiles = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    [void]$expectedFiles.Add('meta.json')
    foreach ($frame in $frames) {
        $index = [int64]$frame.index
        if ($index -lt 0 -or $index -ge [int64]$meta.count -or [string]$frame.path -cne "/original-ui/$Library/$index.png") {
            throw "Candidate sprite frame identity/path invalid: $Library"
        }
        if (-not $expectedFiles.Add("$index.png")) { throw "Candidate sprite has a duplicate frame: $Library/$index" }
        $pngPath = Join-Path $libraryRoot "$index.png"
        if (-not (Test-Path -LiteralPath $pngPath -PathType Leaf)) { throw "Candidate sprite frame missing: $Library/$index.png" }
        $dimensions = Get-PlayerSpritePngDimensions -Path $pngPath
        if ([uint64]$dimensions.width -ne [uint64]$frame.width -or [uint64]$dimensions.height -ne [uint64]$frame.height) { throw "Candidate sprite geometry mismatch: $Library/$index.png" }
        if ($null -ne $frame.PSObject.Properties['pngSha256']) {
            if ([string]$frame.pngSha256 -cnotmatch '^[0-9a-f]{64}$' -or (Get-FileHash -LiteralPath $pngPath -Algorithm SHA256).Hash.ToLowerInvariant() -cne [string]$frame.pngSha256) {
                throw "Candidate sprite source hash mismatch: $Library/$index.png"
            }
        }
        if ($null -ne $frame.PSObject.Properties['maskPath'] -and $null -ne $frame.maskPath) {
            $maskName = "$index.mask.png"
            if ([string]$frame.maskPath -cne "/original-ui/$Library/$maskName" -or -not $expectedFiles.Add($maskName)) { throw "Candidate sprite mask path invalid: $Library/$index" }
            $maskDimensions = Get-PlayerSpritePngDimensions -Path (Join-Path $libraryRoot $maskName)
            if ([uint64]$maskDimensions.width -ne [uint64]$frame.maskWidth -or [uint64]$maskDimensions.height -ne [uint64]$frame.maskHeight) { throw "Candidate sprite mask geometry mismatch: $Library/$index" }
        }
    }
    foreach ($child in Get-ChildItem -LiteralPath $libraryRoot -Force) {
        if ($child.PSIsContainer -or -not $expectedFiles.Contains($child.Name)) { throw "Candidate sprite has an unreferenced file/directory: $Library/$($child.Name)" }
    }
    return [pscustomobject]@{ library = $Library; frames = $frames.Count; files = $expectedFiles.Count }
}

function Assert-CandidateActorClosure {
    param([string]$OriginalUiRoot)
    $count = 0; $frames = 0
    foreach ($library in @((Get-CandidateActorLibraryNames)) + @('MapLinkIcon')) {
        $result = Assert-CandidateSpriteLibraryClosure -OriginalUiRoot $OriginalUiRoot -Library $library
        $count++; $frames += $result.frames
    }
    return [pscustomobject]@{ libraryCount = $count; frameCount = $frames }
}

function Assert-CandidateNativeMapClosure {
    param([string]$AssetRoot, [string]$NativeMapRoot = '', [string[]]$ExpectedMapNames = (Get-CandidateRequiredMapNames))
    if ([string]::IsNullOrWhiteSpace($NativeMapRoot)) { $NativeMapRoot = Join-Path $AssetRoot 'generated/native-map-keyed' }
    $path = Join-Path $NativeMapRoot 'manifest.json'
    $manifest = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    if ([string]$manifest.kind -cne 'mir2-native-map-keyed-manifest' -or [int]$manifest.schemaVersion -ne 1) { throw 'Candidate native map manifest schema mismatch' }
    $names = @(Resolve-CandidateMapNames -MapNames @($manifest.mapFileNames))
    foreach ($expected in (Resolve-CandidateMapNames -MapNames $ExpectedMapNames)) {
        if ($names -cnotcontains $expected) { throw "Candidate native map coverage missing: $expected" }
    }
    if ([int64]$manifest.stats.missingSourceCount -lt 0 -or [int64]$manifest.stats.missingSourceCount -gt 2969) { throw 'Candidate native map source-missing budget exceeded' }
    $entries = @($manifest.entries)
    if ($entries.Count -eq 0 -or $entries.Count -ne [int64]$manifest.stats.emittedEntryCount -or $names.Count -ne [int64]$manifest.stats.mapCount) { throw 'Candidate native map count mismatch' }
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($entry in $entries) {
        if (-not $seen.Add([string]$entry.key) -or [string]$entry.imageUrl -cnotmatch '^/generated/native-map-keyed/pages/[0-9a-f]{64}\.png$') { throw 'Candidate native map has a duplicate/unsafe entry' }
        $pngPath = Join-Path $NativeMapRoot ([string]$entry.imageUrl).Substring('/generated/native-map-keyed/'.Length)
        $dimensions = Get-PlayerSpritePngDimensions -Path $pngPath
        if ([uint64]$dimensions.width -ne [uint64]$entry.width -or [uint64]$dimensions.height -ne [uint64]$entry.height) { throw "Candidate native map frame geometry mismatch: $($entry.key)" }
    }
    return [pscustomobject]@{ maps = $names; frameCount = $entries.Count; missingSourceCount = [int64]$manifest.stats.missingSourceCount; noDrawReferenceCount = [int64]$manifest.stats.noDrawReferenceCount }
}

function Assert-CandidateMapCoverageReport {
    param([string]$NativeMapRoot, [string]$MapAtlasRoot)
    $report = Get-Content -LiteralPath (Join-Path $NativeMapRoot 'coverage-audit.json') -Raw | ConvertFrom-Json
    if ($report.schema -cne 'mir2.windows.candidate-map-coverage.v1' -or $report.passed -ne $true -or
        [int64]$report.blockingReferenceCount -ne 0 -or @($report.omittedMaps).Count -ne 0) { throw 'Candidate map coverage has an omitted map or missing drawable source frame' }
    Resolve-CandidateMapNames -MapNames @($report.mapNames) | Out-Null
    $nativeHash = (Get-FileHash -LiteralPath (Join-Path $NativeMapRoot 'manifest.json') -Algorithm SHA256).Hash.ToLowerInvariant()
    $atlasHash = (Get-FileHash -LiteralPath (Join-Path $MapAtlasRoot 'manifest.json') -Algorithm SHA256).Hash.ToLowerInvariant()
    if ([string]$report.manifestSha256 -cne $nativeHash -or [string]$report.mapAtlasManifestSha256 -cne $atlasHash) { throw 'Candidate map coverage report does not match the shipped manifests' }
    $atlas = Get-Content -LiteralPath (Join-Path $MapAtlasRoot 'manifest.json') -Raw | ConvertFrom-Json
    if ([int]$atlas.schemaVersion -ne 2 -or [int]$atlas.edgeExtrusion -ne 1) { throw 'Candidate map atlas must retain owned one-pixel borders' }
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($page in @($atlas.pages)) {
        if ([string]$page.u -cnotmatch '^/generated/map-atlas/[A-Za-z0-9_-]+/p[0-9]+\.([0-9a-f]{16})\.png$') { throw 'Candidate map atlas contains an unsafe page URL' }
        $expectedPrefix = $Matches[1]
        $file = Join-Path $MapAtlasRoot ([string]$page.u).Substring('/generated/map-atlas/'.Length)
        $dimensions = Get-PlayerSpritePngDimensions -Path $file
        if ([uint64]$dimensions.width -ne [uint64]$page.w -or [uint64]$dimensions.height -ne [uint64]$page.h -or
            [uint64]$page.w * [uint64]$page.h -gt 262144 -or (Get-Item -LiteralPath $file).Length -ne [int64]$page.b -or
            (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant().Substring(0, 16) -cne $expectedPrefix) { throw 'Candidate map atlas page geometry/hash/budget mismatch' }
        foreach ($rect in @($page.r)) {
            if (@($rect).Count -ne 5 -or [int64]$rect[0] -lt 0 -or [int64]$rect[1] -lt 1 -or [int64]$rect[2] -lt 1 -or
                [int64]$rect[3] -le 0 -or [int64]$rect[4] -le 0 -or
                [int64]$rect[1] + [int64]$rect[3] -ge [int64]$page.w -or [int64]$rect[2] + [int64]$rect[4] -ge [int64]$page.h -or
                -not $seen.Add(([string]$page.l + '#' + [string]$rect[0]))) { throw 'Candidate map atlas frame is invalid, duplicated or has no owned gutter' }
        }
    }
    return [pscustomobject]@{ mapCount = @($report.mapNames).Count; drawableMissing = 0; atlasFrameCount = $seen.Count; sourceOmissions = $report.missingByReason }
}

function Test-CandidateReleaseConfiguration {
    $endpoint = 'wss://playtest.mir2.obelisk.build/playtest/ws'
    foreach ($bad in @('', 'ws://playtest.mir2.obelisk.build/ws', 'wss://candidate-gateway.example/ws', 'wss://example.com/ws', 'wss://127.0.0.1/ws', 'wss://localhost/ws', 'wss://[::1]/ws', 'wss://user:secret@playtest.mir2.obelisk.build/ws', ($endpoint + '?token=secret'), ($endpoint + '#secret'), ($endpoint + "`n[credentials]"), ($endpoint + '"'), 'wss://playtest.mir2.obelisk.build/a/../ws', 'wss://playtest.mir2.obelisk.build/%0aws')) {
        $rejected = $false
        try { Resolve-CandidateGatewayWsUrl -Value $bad | Out-Null } catch { $rejected = $true }
        if (-not $rejected) { throw 'Candidate accepted a forbidden endpoint test vector' }
    }
    foreach ($profile in @('crystal', 'newcomer-v1', 'newcomer-v2')) {
        foreach ($daylight in @($true, $false)) {
            $text = New-CandidateClientConfiguration -GatewayWsUrl $endpoint -QuestGuidance $profile -ForceDaylight $daylight
            $parsed = Assert-CandidateClientConfiguration -Text $text
            if ($parsed.gatewayWsUrl -cne $endpoint -or $parsed.questGuidance -cne $profile -or $parsed.forceDaylight -ne $daylight) { throw 'Candidate configuration roundtrip failed' }
        }
    }
    $good = New-CandidateClientConfiguration -GatewayWsUrl $endpoint
    foreach ($badText in @(($good + "password = `"bad`"`n"), ($good + "quest_guidance = `"crystal`"`n"), $good.Replace('newcomer-v2', 'typo'), $good.Replace($endpoint, 'wss://candidate-gateway.example/ws'), $good.Replace('force_daylight = true', 'force_daylight = "true"'))) {
        $rejected = $false
        try { Assert-CandidateClientConfiguration -Text $badText | Out-Null } catch { $rejected = $true }
        if (-not $rejected) { throw 'Candidate accepted invalid/duplicate/secret configuration' }
    }
    $missingSupply = @((Get-CandidateRequiredMapNames) | Where-Object { $_ -cne '0109' })
    $rejected = $false
    try { Resolve-CandidateMapNames -MapNames $missingSupply | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Candidate accepted omitted Taoist supply map' }
    $defaultMaps = @(Get-CandidateDefaultMapNames)
    $canonicalMapBytes = [Text.Encoding]::UTF8.GetBytes(($defaultMaps -join "`n") + "`n")
    $mapHasher = [Security.Cryptography.SHA256]::Create()
    try { $defaultMapHash = [BitConverter]::ToString($mapHasher.ComputeHash($canonicalMapBytes)).Replace('-', '').ToLowerInvariant() }
    finally { $mapHasher.Dispose() }
    if ($defaultMaps.Count -ne 32 -or $canonicalMapBytes.Length -ne 148 -or $defaultMapHash -cne 'f4fa9561a574f2b1d221599430bec7203ebbf57d268d84cab61982a3790642af') {
        throw 'Candidate default map list differs from the reviewed Bichon merchant and Sabuk closure'
    }
    $explicitMaps = @(Resolve-CandidateMapNames -MapNames (Get-CandidateRequiredMapNames))
    if ($explicitMaps.Count -ne 15 -or $explicitMaps -contains '0103' -or $explicitMaps -contains '0122' -or $explicitMaps -contains '0150') { throw 'Candidate expanded an explicit legacy map list' }
    $customMaps = @(Resolve-CandidateMapNames -MapNames (@(Get-CandidateRequiredMapNames) + @('0100', 'D001')))
    if ($customMaps.Count -ne 16 -or $customMaps -cnotcontains '0100' -or $customMaps -cnotcontains 'd001') {
        throw 'Candidate lost or duplicated an explicitly selected custom map'
    }
    $customSiegeMaps = @(Resolve-CandidateMapNames -MapNames (@(Get-CandidateRequiredMapNames) + @('0150', '0155', '0150', 'D001')))
    if ($customSiegeMaps.Count -ne 17 -or $customSiegeMaps -cnotcontains '0150' -or $customSiegeMaps -cnotcontains '0155' -or $customSiegeMaps -contains '0122') {
        throw 'Candidate changed an explicit partial siege map selection or failed to deduplicate it'
    }
    foreach ($badMap in @('../0103', '0103.map.gz', '0:debug', '')) {
        $rejected = $false
        try { Resolve-CandidateMapNames -MapNames (@(Get-CandidateRequiredMapNames) + @($badMap)) | Out-Null } catch { $rejected = $true }
        if (-not $rejected) { throw 'Candidate accepted an unsafe custom map identifier' }
    }
    foreach ($valid in @('mir2-assets/original-ui/Monster/009/80.png', 'mir2-assets/original-ui/NPC/45/meta.json', 'mir2-assets/original-ui/Gate/03/0.png', 'mir2-assets/original-ui/MapLinkIcon/100.png')) {
        if (-not (Test-CandidateActorFileAllowed -RelativePath $valid)) { throw 'Candidate actor allowlist rejected a valid path' }
    }
    foreach ($invalid in @('mir2-assets/original-ui/Monster/9/80.png', 'mir2-assets/original-ui/Monster/009/payload.exe.png', 'mir2-assets/original-ui/NPC/45/../0.png', 'mir2-assets/original-ui/Gate/04/0.png', 'mir2-assets/original-ui/MapLinkIcon/01.png')) {
        if (Test-CandidateActorFileAllowed -RelativePath $invalid) { throw 'Candidate actor allowlist accepted an invalid path' }
    }
    Write-Host 'Candidate release configuration tests passed (endpoints, profiles, daylight, 32-map default merchant/Sabuk coverage, explicit legacy/custom coverage, actor allowlist)'
}

function Test-CandidateSpriteClosureGuards {
    param([string]$OriginalUiRoot, [string]$TemporaryRoot)
    $fixture = Join-Path $TemporaryRoot 'actor-closure-probe'
    $libraryRoot = Join-Path $fixture 'NPC/45'
    New-Item -ItemType Directory -Path $libraryRoot -Force | Out-Null
    $source = Get-Content -LiteralPath (Join-Path $OriginalUiRoot 'NPC/45/meta.json') -Raw | ConvertFrom-Json
    $frame = @($source.frames)[0]
    Copy-Item -LiteralPath (Join-Path $OriginalUiRoot "NPC/45/$($frame.index).png") -Destination $libraryRoot
    $payload = [ordered]@{ version = 3; count = [int64]$source.count; frames = @($frame) }
    $metaPath = Join-Path $libraryRoot 'meta.json'
    [IO.File]::WriteAllText($metaPath, ($payload | ConvertTo-Json -Depth 12), [Text.UTF8Encoding]::new($false))
    Assert-CandidateSpriteLibraryClosure -OriginalUiRoot $fixture -Library 'NPC/45' | Out-Null
    $payload.frames = @($frame, $frame)
    [IO.File]::WriteAllText($metaPath, ($payload | ConvertTo-Json -Depth 12), [Text.UTF8Encoding]::new($false))
    $rejected = $false
    try { Assert-CandidateSpriteLibraryClosure -OriginalUiRoot $fixture -Library 'NPC/45' | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Candidate accepted duplicate actor metadata' }
    $payload.frames = @($frame)
    $frame.path = '/original-ui/NPC/45/../0.png'
    [IO.File]::WriteAllText($metaPath, ($payload | ConvertTo-Json -Depth 12), [Text.UTF8Encoding]::new($false))
    $rejected = $false
    try { Assert-CandidateSpriteLibraryClosure -OriginalUiRoot $fixture -Library 'NPC/45' | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Candidate accepted an escaping actor metadata path' }
    $frame.path = '/original-ui/NPC/45/999999.png'; $frame.index = 999999; $payload.count = 1000000
    [IO.File]::WriteAllText($metaPath, ($payload | ConvertTo-Json -Depth 12), [Text.UTF8Encoding]::new($false))
    $rejected = $false
    try { Assert-CandidateSpriteLibraryClosure -OriginalUiRoot $fixture -Library 'NPC/45' | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Candidate accepted missing actor image data' }
    $rejected = $false
    try { Assert-CandidateActorClosure -OriginalUiRoot $fixture | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Candidate accepted omitted complete actor libraries' }
    Write-Host 'Candidate actor closure guards passed (valid frame, duplicate, escaping path, missing image, missing library)'
}
