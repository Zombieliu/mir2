[CmdletBinding()]
param([string]$EvidenceRoot = '')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'candidate-release-profile.ps1')
Test-CandidateReleaseConfiguration
$scope = Resolve-CandidateMapScope
if ($scope.mode -cne 'profile' -or $scope.expectedMaps.Count -ne 209 -or $scope.profileMapCount -ne 208 -or $scope.profileNativeMapCount -ne 209 -or ($scope.resourceOnlyMaps -join ',') -cne 'd71653') { throw 'The scope fixture requires 208 runtime maps and 209 native resources' }
function Copy-Report([object]$Value) { return ($Value | ConvertTo-Json -Depth 30 | ConvertFrom-Json) }
function Assert-Rejected([scriptblock]$Action, [string]$Label) {
    $rejected = $false
    try { & $Action | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw "Candidate accepted the invalid $Label fixture" }
}
$profile = Get-Content -LiteralPath $script:CandidateContentProfilePath -Raw | ConvertFrom-Json
$proof = Get-CandidateTopologyEvidence -Profile $profile
$report = [pscustomobject]@{
    scope = $scope; mapNames = @($scope.expectedMaps); unexpectedMaps = @()
    scopeRoutes = [pscustomobject]@{ schema = 'mir2.windows.candidate-map-topology.v1'; expectedMapCount = 209; runtimeGameplayAccepted = $false
        unreachableMaps = @('d71653'); unreachableRuntimeMaps = @(); resourceOnlyRooms = @($proof.rooms) }
}
Assert-CandidateMapCoverageScope -Report $report -ScopeMode profile -ExpectedMapNames $scope.expectedMaps | Out-Null
$missing = Copy-Report $report
$missing.mapNames = @($missing.mapNames | Where-Object { $_ -cne 'd10054' })
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $missing -ScopeMode profile } 'omitted classic side branch'
foreach ($field in @('profileId', 'profileVersion', 'profileSha256', 'profileMapCount', 'expectedMapsSha256', 'resourceOnlyMapsSha256', 'profileNativeMapCount', 'sourceTopologySha256')) {
    $wrong = Copy-Report $report
    $wrong.scope.$field = 'wrong'
    Assert-Rejected { Assert-CandidateMapCoverageScope -Report $wrong -ScopeMode profile } $field
}
$legacyReport = Copy-Report $report
$legacyReport.PSObject.Properties.Remove('scope')
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $legacyReport } 'legacy journey-only report'
$falseWorld = Copy-Report $report
$falseWorld.scope.completeClassicWorld = $true
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $falseWorld } 'complete-world claim'
$stringFlag = Copy-Report $report
$stringFlag.scope.completeClassicWorld = 'false'
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $stringFlag } 'string acceptance flag'
$falseAcceptance = Copy-Report $report
$falseAcceptance.scopeRoutes.runtimeGameplayAccepted = $true
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $falseAcceptance } 'static-topology gameplay acceptance'
$sourceHash = Copy-Report $report
$sourceHash.scope.sourceTopology[0].sha256 = ('0' * 64)
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $sourceHash } 'forged source manifest hash'
$otherRoom = Copy-Report $report
$otherRoom.scope.resourceOnlyMaps = @('d71652')
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $otherRoom } 'forged resource-only name'
$forgedRoom = Copy-Report $report
$forgedRoom.scopeRoutes.resourceOnlyRooms[0].destination.x = 35
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $forgedRoom } 'forged original exit coordinates'
$roomAdmitted = Copy-Report $report
$roomAdmitted.scopeRoutes.resourceOnlyRooms[0].runtimeAdmitted = $true
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $roomAdmitted } 'resource-only room claimed as runtime admitted'
$roomAdmittedString = Copy-Report $report
$roomAdmittedString.scopeRoutes.resourceOnlyRooms[0].runtimeAdmitted = 'false'
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $roomAdmittedString } 'string runtime admission flag'
$otherUnreachable = Copy-Report $report
$otherUnreachable.scopeRoutes.unreachableMaps += '5'
$otherUnreachable.scopeRoutes.unreachableRuntimeMaps = @('5')
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $otherUnreachable } 'another unreachable runtime map'
$hiddenUnreachable = Copy-Report $otherUnreachable
$hiddenUnreachable.scopeRoutes.unreachableRuntimeMaps = @()
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $hiddenUnreachable } 'runtime unreachable hidden behind resource exception'
$missingOrphan = Copy-Report $report
$missingOrphan.mapNames = @($scope.expectedMaps | Where-Object { $_ -cne 'd71653' })
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $missingOrphan } 'omitted original resource room'
$explicitScope = Resolve-CandidateMapScope -MapNames (Get-CandidateRequiredMapNames)
$explicit = Copy-Report $report
$explicit.scope = $explicitScope
$explicit.mapNames = @($explicitScope.expectedMaps)
$explicit.scopeRoutes.expectedMapCount = $explicitScope.expectedMaps.Count
$explicit.scopeRoutes.unreachableMaps = @()
$explicit.scopeRoutes.resourceOnlyRooms = @()
Assert-CandidateMapCoverageScope -Report $explicit -ScopeMode explicit -ExpectedMapNames (Get-CandidateRequiredMapNames) | Out-Null
if ($explicitScope.expectedMaps.Count -ne 15) { throw 'Explicit legacy selection was expanded' }
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $explicit -ScopeMode profile } 'explicit list labeled as profile coverage'
$outside = Copy-Report $report
$outside.mapNames += 'd71654'
$outside.unexpectedMaps = @('d71654')
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $outside } 'unexpected map'
$project = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
$respawns = Get-Content -LiteralPath (Join-Path $project 'packages/game-data/data/generated/crystal_respawn_manifest.json') -Raw | ConvertFrom-Json
$npcs = Get-Content -LiteralPath (Join-Path $project 'packages/game-data/data/generated/crystal_npc_manifest.json') -Raw | ConvertFrom-Json
$events = Get-Content -LiteralPath (Join-Path $project 'packages/game-data/data/generated/crystal_map_event_manifest.json') -Raw | ConvertFrom-Json
$sourceArgs = @{ Profile = $profile; RespawnManifest = $respawns; NpcScriptManifest = $npcs; MapEventManifest = $events }
Assert-CandidateResourceOnlyRoomProof @sourceArgs | Out-Null
$inbound = Copy-Report $respawns
$edge = Copy-Report (($respawns.maps | Where-Object { $_.map_file_name -ceq 'D71653' }).movements[0])
$edge.map_index = 235
$inbound.maps[0].movements += $edge
Assert-Rejected { Assert-CandidateResourceOnlyRoomProof -Profile $profile -RespawnManifest $inbound -NpcScriptManifest $npcs -MapEventManifest $events } 'invented inbound movement'
$scriptEntry = Copy-Report $npcs
$scriptEntry | Add-Member -NotePropertyName fixture -NotePropertyValue 'MAPMOVE D71653 17 12'
Assert-Rejected { Assert-CandidateResourceOnlyRoomProof -Profile $profile -RespawnManifest $respawns -NpcScriptManifest $scriptEntry -MapEventManifest $events } 'new NPC entry invalidating original orphan proof'
$eventEntry = Copy-Report $events
$eventEntry | Add-Member -NotePropertyName fixture -NotePropertyValue 'MAPMOVE D71653 17 12'
Assert-Rejected { Assert-CandidateResourceOnlyRoomProof -Profile $profile -RespawnManifest $respawns -NpcScriptManifest $npcs -MapEventManifest $eventEntry } 'new event entry invalidating original orphan proof'
$movedExit = Copy-Report $respawns
($movedExit.maps | Where-Object { $_.map_file_name -ceq 'D71653' }).movements[0].source.x = 16
Assert-Rejected { Assert-CandidateResourceOnlyRoomProof -Profile $profile -RespawnManifest $movedExit -NpcScriptManifest $npcs -MapEventManifest $events } 'changed source exit'
if (@(Get-CandidateActorLibraryNames | Where-Object { $_ -ceq 'Monster/049' }).Count -ne 1) { throw 'Candidate must require exactly one Monster/049 library' }
$actorProbeParent = if ($EvidenceRoot) { [IO.Path]::GetFullPath($EvidenceRoot) } else { [IO.Path]::GetTempPath() }
$actorProbe = Join-Path $actorProbeParent ('actor-049-missing-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $actorProbe -Force | Out-Null
Assert-Rejected { Assert-CandidateSpriteLibraryClosure -OriginalUiRoot $actorProbe -Library 'Monster/049' } 'omitted entire 049 library'
if ($EvidenceRoot) {
    $evidenceFull = [IO.Path]::GetFullPath($EvidenceRoot)
    New-Item -ItemType Directory -Path $evidenceFull -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $evidenceFull 'powershell-map-scope.json'), ($scope | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    # Bind the receipt to the actual native manifest, even if an attacker
    # updates its hash while retaining a claimed 209-name report list.
    $fixture = Join-Path $evidenceFull ('native-scope-mismatch-' + [guid]::NewGuid().ToString('N'))
    $nativeRoot = Join-Path $fixture 'native'
    $atlasRoot = Join-Path $fixture 'atlas'
    New-Item -ItemType Directory -Path $nativeRoot, $atlasRoot -Force | Out-Null
    $nativePath = Join-Path $nativeRoot 'manifest.json'
    $atlasPath = Join-Path $atlasRoot 'manifest.json'
    [IO.File]::WriteAllText($nativePath, (@{ mapFileNames = $missing.mapNames } | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($atlasPath, '{"schemaVersion":2,"edgeExtrusion":1,"pages":[]}', [Text.UTF8Encoding]::new($false))
    $forged = Copy-Report $report
    $forged | Add-Member -NotePropertyName schema -NotePropertyValue 'mir2.windows.candidate-map-coverage.v1'
    $forged | Add-Member -NotePropertyName passed -NotePropertyValue $true
    $forged | Add-Member -NotePropertyName blockingReferenceCount -NotePropertyValue 0
    $forged | Add-Member -NotePropertyName omittedMaps -NotePropertyValue @()
    $forged | Add-Member -NotePropertyName manifestSha256 -NotePropertyValue (Get-FileHash -LiteralPath $nativePath -Algorithm SHA256).Hash.ToLowerInvariant()
    $forged | Add-Member -NotePropertyName mapAtlasManifestSha256 -NotePropertyValue (Get-FileHash -LiteralPath $atlasPath -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $nativeRoot 'coverage-audit.json'), ($forged | ConvertTo-Json -Depth 30), [Text.UTF8Encoding]::new($false))
    Assert-Rejected { Assert-CandidateMapCoverageReport -NativeMapRoot $nativeRoot -MapAtlasRoot $atlasRoot -ScopeMode profile } 'receipt/native map-list mismatch'
}
Write-Host "Candidate map scope fixtures passed (208 runtime/209 native, exact 15 explicit, source-only named proof, 049 requirement, omitted maps, identities and acceptance boundaries); expectedSha=$($scope.expectedMapsSha256)"
