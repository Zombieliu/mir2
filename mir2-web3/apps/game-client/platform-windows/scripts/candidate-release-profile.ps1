# Shared release configuration and resource closure for the active classic profile.
# This file does not sign, publish, mutate saves, or relax the Candidate gates.
$script:CandidateContentProfilePath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../../packages/game-data/data/content_profiles/platinum_176.json'))

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
    $names = @($result)
    [Array]::Sort($names, [StringComparer]::Ordinal)
    return $names
}

function Get-CandidatePreviouslyShippedMapNames {
    $merchantMaps = @('0101', '0102', '0103', '0104', '0105', '0106', '0107', '0125', '0132', '0140')
    $siegeMaps = @('0122', '0150', '0151', '0152', '0153', '0154', '0155')
    return @(Resolve-CandidateMapNames -MapNames (@(Get-CandidateRequiredMapNames) + $merchantMaps + $siegeMaps))
}

function Get-CandidateMapListSha256 {
    param([string[]]$MapNames)
    $bytes = [Text.Encoding]::UTF8.GetBytes(((Resolve-CandidateMapNames -MapNames $MapNames) -join "`n") + "`n")
    $hasher = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($hasher.ComputeHash($bytes)).Replace('-', '').ToLowerInvariant() }
    finally { $hasher.Dispose() }
}

function Get-CandidateResourceOnlyMapListSha256 {
    param([string[]]$MapNames = @())
    if ($MapNames.Count -gt 1 -or @($MapNames | Where-Object { $_ -cne 'd71653' }).Count -gt 0) { throw 'Resource-only hash requires the named original room or an empty selection' }
    $bytes = [Text.Encoding]::UTF8.GetBytes(($MapNames -join "`n") + "`n")
    $hasher = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($hasher.ComputeHash($bytes)).Replace('-', '').ToLowerInvariant() }
    finally { $hasher.Dispose() }
}

function Get-CandidateResourceOnlyMapNames {
    param([object]$Profile)
    $names = @()
    if ($null -ne $Profile.PSObject.Properties['nativeResourceOnlyMaps']) {
        if ($Profile.nativeResourceOnlyMaps -isnot [array]) { throw 'Only the canonical named original resource room D71653 may be declared' }
        $raw = @($Profile.nativeResourceOnlyMaps)
        if ($raw.Count -gt 1 -or @($raw | Where-Object { $_ -isnot [string] -or $_ -cne 'D71653' }).Count -gt 0) { throw 'Only the canonical named original resource room D71653 may be declared' }
        $names = @($raw | ForEach-Object { $_.ToLowerInvariant() })
    }
    if ($names.Count -gt 0 -and $Profile.profileId -cne 'platinum_176') { throw 'Resource-only room requires platinum_176' }
    $runtime = @($Profile.mapWhitelist | ForEach-Object { ([string]$_.fileName).ToLowerInvariant() })
    foreach ($name in $names) { if ($runtime -ccontains $name) { throw 'Resource-only room must not be runtime-admitted' } }
    return $names
}

function Assert-CandidateResourceOnlyRoomProof {
    param([object]$Profile, [object]$RespawnManifest, [object]$NpcScriptManifest, [object]$MapEventManifest)
    $names = @(Get-CandidateResourceOnlyMapNames -Profile $Profile)
    if ($names.Count -eq 0) { return @() }
    $npcText = ($NpcScriptManifest | ConvertTo-Json -Depth 100 -Compress).ToLowerInvariant()
    $eventText = ($MapEventManifest | ConvertTo-Json -Depth 100 -Compress).ToLowerInvariant()
    foreach ($name in $names) {
        $rooms = @($RespawnManifest.maps | Where-Object { $_.map_file_name -ceq 'D71653' })
        if ($rooms.Count -ne 1) { throw 'D71653 no longer matches its original no-entry source proof' }
        $room = $rooms[0]
        if ($room.map_index -ne 235 -or $room.map_title -cne 'TacticalMaze' -or $room.mini_map -ne 0 -or $room.big_map -ne 0 -or
            $room.respawn_count -ne 0 -or @($room.respawns).Count -ne 0 -or @($room.safe_zones).Count -ne 0 -or
            $room.movement_count -ne 1 -or @($room.movements).Count -ne 1) { throw 'D71653 no longer matches its original no-entry source proof' }
        $move = $room.movements[0]
        $inbound = @($RespawnManifest.maps | ForEach-Object { $_.movements } | Where-Object { $_.map_index -eq 235 })
        if ($move.map_index -ne 209 -or $move.source.x -ne 17 -or $move.source.y -ne 12 -or $move.destination.x -ne 36 -or $move.destination.y -ne 34 -or
            $move.need_hole -isnot [bool] -or $move.need_hole -ne $false -or $move.need_move -isnot [bool] -or $move.need_move -ne $false -or
            $move.conquest_index -ne 0 -or $move.show_on_big_map -isnot [bool] -or $move.show_on_big_map -ne $false -or $move.icon -ne 0 -or
            $inbound.Count -gt 0 -or $npcText.Contains($name) -or $eventText.Contains($name)) { throw 'D71653 no longer matches its original no-entry source proof' }
        [pscustomobject]@{ map = $name; mapIndex = 235; reason = 'source-has-no-imported-entry'; runtimeAdmitted = $false
            outgoingMapIndices = @(209); source = [pscustomobject]@{ x = 17; y = 12 }; destination = [pscustomobject]@{ x = 36; y = 34 } }
    }
}

function Get-CandidateTopologyEvidence {
    param([object]$Profile)
    $paths = @('packages/game-data/data/generated/crystal_respawn_manifest.json', 'packages/game-data/data/generated/crystal_npc_manifest.json', 'packages/game-data/data/generated/crystal_map_event_manifest.json')
    $project = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
    $values = @(); $files = @()
    foreach ($relative in $paths) {
        $file = Join-Path $project $relative
        $values += (Get-Content -LiteralPath $file -Raw | ConvertFrom-Json)
        $files += [pscustomobject]@{ path = $relative; sha256 = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() }
    }
    $rooms = @(Assert-CandidateResourceOnlyRoomProof -Profile $Profile -RespawnManifest $values[0] -NpcScriptManifest $values[1] -MapEventManifest $values[2])
    $inputText = ($files | ForEach-Object { $_.path + "`0" + $_.sha256 + "`n" }) -join ''
    $hasher = [Security.Cryptography.SHA256]::Create()
    try { $sha = [BitConverter]::ToString($hasher.ComputeHash([Text.Encoding]::UTF8.GetBytes($inputText))).Replace('-', '').ToLowerInvariant() }
    finally { $hasher.Dispose() }
    return [pscustomobject]@{ files = $files; sha256 = $sha; rooms = $rooms }
}

function Resolve-CandidateMapScope {
    param([string[]]$MapNames = @(), [string]$ProfilePath = $script:CandidateContentProfilePath)
    $profile = Get-Content -LiteralPath $ProfilePath -Raw | ConvertFrom-Json
    if ($null -eq $profile.PSObject.Properties['profileId'] -or [string]$profile.profileId -cne 'platinum_176' -or
        $null -eq $profile.PSObject.Properties['version'] -or $profile.version -is [string] -or
        [decimal]$profile.version -ne [int64]$profile.version -or [int64]$profile.version -lt 1 -or
        $null -eq $profile.PSObject.Properties['mapWhitelist']) { throw 'Candidate map scope requires the identified platinum_176 content profile' }
    $rawNames = @($profile.mapWhitelist | ForEach-Object { [string]$_.fileName })
    $profileNames = @(Resolve-CandidateMapNames -MapNames $rawNames)
    if ($profileNames.Count -ne $rawNames.Count) { throw 'Candidate content profile contains duplicate map identities' }
    $resourceNames = @(Get-CandidateResourceOnlyMapNames -Profile $profile)
    $evidence = Get-CandidateTopologyEvidence -Profile $profile
    $nativeNames = @(Resolve-CandidateMapNames -MapNames ($profileNames + $resourceNames))
    foreach ($previous in Get-CandidatePreviouslyShippedMapNames) {
        if ($profileNames -cnotcontains $previous) { throw "Candidate content profile omitted a previously shipped map: $previous" }
    }
    $mode = 'profile'
    $expected = $nativeNames
    if ($MapNames.Count -gt 0) { $mode = 'explicit'; $expected = @(Resolve-CandidateMapNames -MapNames $MapNames) }
    $selectedResourceNames = @($resourceNames | Where-Object { $expected -ccontains $_ })
    return [pscustomobject]@{
        schema = 'mir2.windows.candidate-map-scope.v1'; mode = $mode
        profileId = [string]$profile.profileId; profileVersion = [int64]$profile.version
        profileSha256 = (Get-FileHash -LiteralPath $ProfilePath -Algorithm SHA256).Hash.ToLowerInvariant()
        profileMapCount = $profileNames.Count; expectedMaps = $expected
        expectedMapsSha256 = Get-CandidateMapListSha256 -MapNames $expected
        resourceOnlyMaps = $selectedResourceNames; resourceOnlyMapsSha256 = Get-CandidateResourceOnlyMapListSha256 -MapNames $selectedResourceNames
        profileNativeMapCount = $nativeNames.Count; sourceTopology = $evidence.files; sourceTopologySha256 = $evidence.sha256
        completeClassicWorld = $false
    }
}

function Get-CandidateDefaultMapNames {
    # Derive admitted maps plus proved resource rooms, retaining the 32-map floor.
    # Explicit selections still use Resolve-CandidateMapNames independently.
    return @((Resolve-CandidateMapScope).expectedMaps)
}

function Assert-CandidateMapCoverageScope {
    param([object]$Report, [string[]]$ExpectedMapNames = @(), [string]$ScopeMode = '')
    if ($null -eq $Report.PSObject.Properties['scope'] -or $null -eq $Report.scope -or
        $Report.scope.schema -cne 'mir2.windows.candidate-map-scope.v1' -or $Report.scope.mode -cnotin @('profile', 'explicit')) {
        throw 'Candidate map coverage requires an explicit content-profile scope receipt'
    }
    $scope = $Report.scope
    if ($ScopeMode -and $ScopeMode -cne $scope.mode) { throw 'Candidate map coverage scope mode differs from the requested selection' }
    if ($scope.mode -ceq 'profile') {
        $expected = Resolve-CandidateMapScope
        if ($ExpectedMapNames.Count -gt 0 -and (Get-CandidateMapListSha256 -MapNames $ExpectedMapNames) -cne $expected.expectedMapsSha256) {
            throw 'Candidate full-profile map scope differs from the requested map selection'
        }
    } else {
        $selection = $ExpectedMapNames
        if ($selection.Count -eq 0) { $selection = @($scope.expectedMaps) }
        $expected = Resolve-CandidateMapScope -MapNames $selection
    }
    foreach ($field in @('schema', 'mode', 'profileId', 'profileVersion', 'profileSha256', 'profileMapCount', 'expectedMapsSha256', 'resourceOnlyMapsSha256', 'profileNativeMapCount', 'sourceTopologySha256')) {
        if ($null -eq $scope.PSObject.Properties[$field] -or [string]$scope.$field -cne [string]$expected.$field) { throw "Candidate map coverage scope identity mismatch: $field" }
    }
    if ($null -eq $scope.PSObject.Properties['completeClassicWorld'] -or $scope.completeClassicWorld -isnot [bool] -or $scope.completeClassicWorld -ne $false) { throw 'Active-profile map coverage cannot claim complete classic-world acceptance' }
    if ($null -eq $scope.PSObject.Properties['resourceOnlyMaps'] -or ($scope.resourceOnlyMaps -join "`n") -cne ($expected.resourceOnlyMaps -join "`n") -or
        $null -eq $scope.PSObject.Properties['sourceTopology'] -or ($scope.sourceTopology | ConvertTo-Json -Depth 10 -Compress) -cne ($expected.sourceTopology | ConvertTo-Json -Depth 10 -Compress)) { throw 'Candidate resource-only declaration or original topology hash binding mismatch' }
    $declared = @(Resolve-CandidateMapNames -MapNames @($scope.expectedMaps))
    $actual = @(Resolve-CandidateMapNames -MapNames @($Report.mapNames))
    if (($declared -join "`n") -cne ($expected.expectedMaps -join "`n") -or ($actual -join "`n") -cne ($expected.expectedMaps -join "`n")) {
        throw 'Candidate map coverage omitted or added a map outside the expected scope'
    }
    if ($null -eq $Report.PSObject.Properties['unexpectedMaps'] -or @($Report.unexpectedMaps).Count -ne 0) { throw 'Candidate map coverage contains an unexpected map' }
    if ($null -eq $Report.PSObject.Properties['scopeRoutes'] -or $Report.scopeRoutes.schema -cne 'mir2.windows.candidate-map-topology.v1' -or
        [int64]$Report.scopeRoutes.expectedMapCount -ne $expected.expectedMaps.Count -or $Report.scopeRoutes.runtimeGameplayAccepted -isnot [bool] -or $Report.scopeRoutes.runtimeGameplayAccepted -ne $false -or
        $null -eq $Report.scopeRoutes.PSObject.Properties['unreachableRuntimeMaps'] -or $null -eq $Report.scopeRoutes.PSObject.Properties['resourceOnlyRooms'] -or
        ($scope.mode -ceq 'profile' -and (@($Report.scopeRoutes.unreachableRuntimeMaps).Count -ne 0 -or ($Report.scopeRoutes.unreachableMaps -join "`n") -cne ($expected.resourceOnlyMaps -join "`n")))) { throw 'Candidate map topology receipt is missing, unreachable or claims gameplay acceptance' }
    $sourceProfile = Get-Content -LiteralPath $script:CandidateContentProfilePath -Raw | ConvertFrom-Json
    $evidence = Get-CandidateTopologyEvidence -Profile $sourceProfile
    $rooms = @($evidence.rooms | Where-Object { $expected.resourceOnlyMaps -ccontains $_.map })
    if (@($Report.scopeRoutes.resourceOnlyRooms).Count -ne $rooms.Count -or
        (ConvertTo-Json -InputObject @($Report.scopeRoutes.resourceOnlyRooms) -Depth 20 -Compress) -cne (ConvertTo-Json -InputObject $rooms -Depth 20 -Compress)) { throw 'Candidate topology forged a resource-only room or its original source proof' }
    return $expected
}

function Get-CandidateActorLibraryNames {
    # The shipped classic libraries plus v27 EvilSnake's source Monster/049.
    # Requiring every metadata file detects a silently
    # omitted entire library, which per-existing-directory checks cannot do.
    $monsters = '000,003,004,005,006,007,008,009,010,011,012,013,014,015,016,017,018,019,020,021,022,023,024,025,026,027,029,030,031,032,033,034,035,036,037,038,039,040,041,042,043,044,045,046,047,048,049,051,052,053,054,055,056,057,058,059,060,061,062,063,064,065,066,067,068,069,070,071,072,073,074,078,081,082,083,084,085,087,088,090,091,092,094,095,097,098,099,100,103,104,105,106,107,108,109,110,111,112,114,139,152,153,154,155,163,378'
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
    param([string]$NativeMapRoot, [string]$MapAtlasRoot, [string[]]$ExpectedMapNames = @(), [string]$ScopeMode = '')
    $report = Get-Content -LiteralPath (Join-Path $NativeMapRoot 'coverage-audit.json') -Raw | ConvertFrom-Json
    if ($report.schema -cne 'mir2.windows.candidate-map-coverage.v1' -or $report.passed -ne $true -or
        [int64]$report.blockingReferenceCount -ne 0 -or @($report.omittedMaps).Count -ne 0) { throw 'Candidate map coverage has an omitted map or missing drawable source frame' }
    $scope = Assert-CandidateMapCoverageScope -Report $report -ExpectedMapNames $ExpectedMapNames -ScopeMode $ScopeMode
    $nativeHash = (Get-FileHash -LiteralPath (Join-Path $NativeMapRoot 'manifest.json') -Algorithm SHA256).Hash.ToLowerInvariant()
    $atlasHash = (Get-FileHash -LiteralPath (Join-Path $MapAtlasRoot 'manifest.json') -Algorithm SHA256).Hash.ToLowerInvariant()
    if ([string]$report.manifestSha256 -cne $nativeHash -or [string]$report.mapAtlasManifestSha256 -cne $atlasHash) { throw 'Candidate map coverage report does not match the shipped manifests' }
    $native = Get-Content -LiteralPath (Join-Path $NativeMapRoot 'manifest.json') -Raw | ConvertFrom-Json
    if ((Get-CandidateMapListSha256 -MapNames @($native.mapFileNames)) -cne $scope.expectedMapsSha256 -or @($native.mapFileNames).Count -ne $scope.expectedMaps.Count) {
        throw 'Candidate native manifest maps differ from the scoped coverage report'
    }
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
    $profileScope = Resolve-CandidateMapScope
    if (($defaultMaps -join "`n") -cne ($profileScope.expectedMaps -join "`n") -or $defaultMaps.Count -lt 32) {
        throw 'Candidate default map list differs from the active content-profile scope'
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
    Write-Host "Candidate release configuration tests passed (endpoints, profiles, daylight, $($defaultMaps.Count)-map profile coverage, explicit legacy/custom coverage, actor allowlist)"
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
