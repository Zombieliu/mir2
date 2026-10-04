[CmdletBinding()]
param([string]$EvidenceRoot = '')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'candidate-release-profile.ps1')
Test-CandidateReleaseConfiguration
$scope = Resolve-CandidateMapScope
if ($scope.mode -cne 'profile' -or $scope.expectedMaps.Count -ne 164) { throw 'The admitted-world fixture must currently contain 164 maps' }
function Copy-Report([object]$Value) { return ($Value | ConvertTo-Json -Depth 30 | ConvertFrom-Json) }
function Assert-Rejected([scriptblock]$Action, [string]$Label) {
    $rejected = $false
    try { & $Action | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw "Candidate accepted the invalid $Label fixture" }
}
$report = [pscustomobject]@{
    scope = $scope; mapNames = @($scope.expectedMaps); unexpectedMaps = @()
    scopeRoutes = [pscustomobject]@{ schema = 'mir2.windows.candidate-map-topology.v1'; expectedMapCount = 164; runtimeGameplayAccepted = $false; unreachableMaps = @() }
}
Assert-CandidateMapCoverageScope -Report $report -ScopeMode profile -ExpectedMapNames $scope.expectedMaps | Out-Null
$missing = Copy-Report $report
$missing.mapNames = @($missing.mapNames | Where-Object { $_ -cne 'd10054' })
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $missing -ScopeMode profile } 'omitted classic side branch'
foreach ($field in @('profileId', 'profileVersion', 'profileSha256', 'profileMapCount', 'expectedMapsSha256')) {
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
$explicitScope = Resolve-CandidateMapScope -MapNames (Get-CandidateRequiredMapNames)
$explicit = Copy-Report $report
$explicit.scope = $explicitScope
$explicit.mapNames = @($explicitScope.expectedMaps)
$explicit.scopeRoutes.expectedMapCount = $explicitScope.expectedMaps.Count
Assert-CandidateMapCoverageScope -Report $explicit -ScopeMode explicit -ExpectedMapNames (Get-CandidateRequiredMapNames) | Out-Null
if ($explicitScope.expectedMaps.Count -ne 15) { throw 'Explicit legacy selection was expanded' }
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $explicit -ScopeMode profile } 'explicit list labeled as profile coverage'
$outside = Copy-Report $report
$outside.mapNames += 'd71653'
$outside.unexpectedMaps = @('d71653')
Assert-Rejected { Assert-CandidateMapCoverageScope -Report $outside } 'unexpected orphan map'
if ($EvidenceRoot) {
    $evidenceFull = [IO.Path]::GetFullPath($EvidenceRoot)
    New-Item -ItemType Directory -Path $evidenceFull -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $evidenceFull 'powershell-map-scope.json'), ($scope | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    # Bind the receipt to the actual native manifest, even if an attacker
    # updates its hash while retaining a claimed 164-name report list.
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
Write-Host "Candidate map scope fixtures passed (164 full-profile, exact 15 explicit, omitted branch, identities, legacy receipt and acceptance boundaries); expectedSha=$($scope.expectedMapsSha256)"
