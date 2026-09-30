# Offline fixtures only: no game execution, real event-log queries or policy IO.
[CmdletBinding()]
param()
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$diagnostic = Join-Path $PSScriptRoot 'diagnose-launch-block.ps1'
. $diagnostic -GameExePath 'C:\fixture\mir2-platform-windows.exe'

$script:Assertions = 0
function Assert-Fixture {
    param([bool]$Condition, [string]$Because)
    if (-not $Condition) { throw "Fixture failed: $Because" }
    $script:Assertions++
}
function New-FixtureEvent {
    param([int]$Id = 3077, [string]$Path = 'C:\fixture\mir2-platform-windows.exe', [string]$Activity = '{A25C1E57-23CB-4229-92F8-911B43A10F40}', [string]$Time = '2026-09-30T10:00:00.0000000Z', [hashtable]$Additional = @{})
    $fields = [ordered]@{}
    if ($Path) { $fields['FileName'] = $Path }
    foreach ($key in $Additional.Keys) { $fields[$key] = $Additional[$key] }
    $data = @($fields.Keys | ForEach-Object { '<Data Name="' + [Security.SecurityElement]::Escape($_) + '">' + [Security.SecurityElement]::Escape([string]$fields[$_]) + '</Data>' }) -join ''
    $xml = '<Event xmlns="http://schemas.microsoft.com/win/2004/08/events/event"><System><EventID>' + $Id + '</EventID><TimeCreated SystemTime="' + $Time + '"/><Correlation ActivityID="' + $Activity + '"/><Computer>PRIVATE-HOST</Computer><Security UserID="PRIVATE-SID"/></System><EventData>' + $data + '</EventData></Event>'
    return ConvertFrom-Mir2CodeIntegrityXml -EventXml $xml
}

$gamePath = 'C:\fixture\mir2-platform-windows.exe'
$gameHash = 'A' * 64
$since = [datetime]::Parse('2026-09-28T12:00:00Z').ToUniversalTime()
$until = [datetime]::Parse('2026-09-30T12:00:00Z').ToUniversalTime()
$selected = New-FixtureEvent -Additional @{ PolicyName = 'VerifiedAndReputableDesktop'; PolicyId = '{597473FC-05A9-481B-ADC9-A97B52E0ADFC}'; Status = '0xC0000428'; RequestedSigningLevel = '2'; ValidatedSigningLevel = '1'; ProcessName = 'C:\Users\PrivatePerson\private-installer.exe'; UserName = 'PRIVATE-ACCOUNT'; SHA256FlatHash = $gameHash }
$sameNameOtherFile = New-FixtureEvent -Path 'D:\unrelated\mir2-platform-windows.exe' -Activity '{90DCC4B6-8B72-4EF3-9A2F-5EAD942944E2}'
$parentOnly = New-FixtureEvent -Path 'C:\other\unrelated.dll' -Additional @{ ProcessName = $gamePath }
$signature = New-FixtureEvent -Id 3089 -Path '' -Additional @{ TotalSignatureCount = '0'; VerificationError = '21'; Hash = ('B' * 64); PublisherName = 'Not Exported' }
$wrongActivity = New-FixtureEvent -Id 3089 -Path '' -Activity '{90DCC4B6-8B72-4EF3-9A2F-5EAD942944E2}' -Additional @{ VerificationError = '99' }
$conflictingSignaturePath = New-FixtureEvent -Id 3089 -Path 'C:\other\private.exe' -Additional @{ VerificationError = '88' }
$results = @(Select-Mir2CodeIntegrityEvents -Events @($sameNameOtherFile, $signature, $wrongActivity, $conflictingSignaturePath, $parentOnly, $selected) -TargetPath $gamePath -TargetSha256 $gameHash -SinceUtc $since -UntilUtc $until)
Assert-Fixture ($results.Count -eq 2) 'only the exact EXE block and its filename-less correlated signature should remain'
Assert-Fixture ($results[0].match -eq 'exact-file-path') 'block proof should state exact path'
Assert-Fixture ($results[1].match -eq 'correlated-selected-file-activity') 'signature order in the input must not matter'
Assert-Fixture ($results[0].Status -eq '0xC0000428' -and $results[1].VerificationError -eq '21') 'preserve policy/signature status'
$publicJson = $results | ConvertTo-Json -Depth 6
foreach ($private in @('PRIVATE-HOST', 'PRIVATE-SID', 'PRIVATE-ACCOUNT', 'PrivatePerson', 'private-installer', 'unrelated.dll', 'private.exe', 'PublisherName', 'EventData')) {
    Assert-Fixture (-not $publicJson.Contains($private)) "public report must not export $private"
}
foreach ($privatePath in @('C:\Users\Private Person\policy.xml', 'C:/Users/Private Person/policy.xml', '\Device\HarddiskVolume7\Users\Private Person\policy.xml')) {
    $sanitized = Protect-Mir2DiagnosticText $privatePath
    Assert-Fixture (-not $sanitized.Contains('Private') -and -not $sanitized.Contains('Person') -and $sanitized.Contains('<redacted>')) 'profile names containing spaces are completely redacted'
}
$certificate = [pscustomobject]@{ Subject = 'CN=Fixture Publisher'; Issuer = 'CN=Fixture CA'; Thumbprint = ('B' * 40); NotBefore = $since; NotAfter = $until; PrivateKey = 'PRIVATE-KEY-MUST-NOT-BE-READ' }
$certificateJson = Get-Mir2CertificateSummary $certificate | ConvertTo-Json
Assert-Fixture ($certificateJson.Contains('Fixture Publisher') -and -not $certificateJson.Contains('PrivateKey') -and -not $certificateJson.Contains('PRIVATE-KEY')) 'certificate summary exports only the allowlisted public metadata'

foreach ($path in @('c:\FIXTURE\mir2-platform-windows.exe', '\??\C:\fixture\mir2-platform-windows.exe', '\\?\C:\fixture\mir2-platform-windows.exe')) {
    $event = New-FixtureEvent -Path $path
    Assert-Fixture ((Get-Mir2CodeIntegrityMatch -Event $event -TargetPath $gamePath -TargetSha256 $gameHash) -eq 'exact-file-path') 'case and known Win32 prefixes preserve exact identity'
}
$authHashOnly = New-FixtureEvent -Path '\Device\HarddiskVolume7\fixture\mir2-platform-windows.exe' -Additional @{ SHA256Hash = $gameHash }
Assert-Fixture ((Get-Mir2CodeIntegrityMatch -Event $authHashOnly -TargetPath $gamePath -TargetSha256 $gameHash) -eq '') 'Authenticode hash is not a flat-file hash'
$flatHashEvent = New-FixtureEvent -Path '\Device\HarddiskVolume7\fixture\mir2-platform-windows.exe' -Additional @{ SHA256FlatHash = $gameHash }
Assert-Fixture ((Get-Mir2CodeIntegrityMatch -Event $flatHashEvent -TargetPath $gamePath -TargetSha256 $gameHash) -eq 'exact-flat-sha256') 'flat hash can identify a device-path event'
$device = New-FixtureEvent -Id 3033 -Path '\Device\HarddiskVolume7\fixture\mir2-platform-windows.exe'
$fixtureCalls = [Collections.Generic.List[string]]::new()
$reader = { param($p); [void]$fixtureCalls.Add($p); return ('A' * 64) }.GetNewClosure()
Assert-Fixture ((Get-Mir2CodeIntegrityMatch -Event $device -TargetPath $gamePath -TargetSha256 $gameHash -DevicePathHashReader $reader) -eq 'device-file-current-flat-sha256') 'device path requires same-content proof'
Assert-Fixture ($fixtureCalls.Count -eq 1) 'the eligible device path should be read exactly once by the match function'
$differentTail = New-FixtureEvent -Path '\Device\HarddiskVolume7\other\mir2-platform-windows.exe'
Assert-Fixture ((Get-Mir2CodeIntegrityMatch -Event $differentTail -TargetPath $gamePath -TargetSha256 $gameHash -DevicePathHashReader $reader) -eq '') 'same basename on another path must not cause a device read'
Assert-Fixture ($fixtureCalls.Count -eq 1) 'unselected file was not read'
$wrongHashReader = { param($p); return ('B' * 64) }
Assert-Fixture ((Get-Mir2CodeIntegrityMatch -Event $device -TargetPath $gamePath -TargetSha256 $gameHash -DevicePathHashReader $wrongHashReader) -eq '') 'same suffix on another volume with different bytes is not the selected file'

$old = New-FixtureEvent -Time '2026-09-28T11:59:59Z'
$future = New-FixtureEvent -Time '2026-09-30T12:00:01Z'
$windowResult = @(Select-Mir2CodeIntegrityEvents -Events @($old, $future, $selected) -TargetPath $gamePath -TargetSha256 $gameHash -SinceUtc $since -UntilUtc $until)
Assert-Fixture ($windowResult.Count -eq 1) 'respect the exact 48-hour window including future records'
$emptyActivityBlock = New-FixtureEvent -Activity '{00000000-0000-0000-0000-000000000000}'
$emptyActivitySig = New-FixtureEvent -Id 3089 -Path '' -Activity '{00000000-0000-0000-0000-000000000000}'
$emptyActivityResults = @(Select-Mir2CodeIntegrityEvents -Events @($emptyActivityBlock, $emptyActivitySig) -TargetPath $gamePath -TargetSha256 $gameHash -SinceUtc $since -UntilUtc $until)
Assert-Fixture ($emptyActivityResults.Count -eq 1) 'zero ActivityID cannot correlate unrelated signatures'
$parseRejected = $false
try { ConvertFrom-Mir2CodeIntegrityXml -EventXml '<!DOCTYPE x [<!ENTITY a SYSTEM "file:///C:/private.txt">]><Event>&a;</Event>' | Out-Null } catch { $parseRejected = $true }
Assert-Fixture $parseRejected 'DTD/external entities are prohibited'
$duplicateRejected = $false
try { New-FixtureEvent -Additional @{ 'File Name' = 'C:\other.exe' } | Out-Null } catch { $duplicateRejected = $true }
Assert-Fixture $duplicateRejected 'ambiguous duplicate normalized fields fail closed'

# Reader failures are tested using controlled command providers. None of these
# invokes the registry, Windows event service, trust store or real EXE.
function Get-Item {
    [CmdletBinding()]param([string]$LiteralPath, [switch]$Force)
    return [pscustomobject]@{ PSIsContainer = $false; Extension = '.exe'; FullName = $gamePath; Name = 'mir2-platform-windows.exe'; Length = 123; VersionInfo = [pscustomobject]@{ FileVersion = '1.2.3.4'; ProductVersion = 'fixture' } }
}
function Get-FileHash {
    [CmdletBinding()]param([string]$LiteralPath, [string]$Algorithm)
    return [pscustomobject]@{ Hash = $gameHash }
}
function Get-AuthenticodeSignature {
    [CmdletBinding()]param([string]$LiteralPath)
    return [pscustomobject]@{ Status = 'NotSigned'; SignatureType = 'None'; SignerCertificate = $null; TimeStamperCertificate = $null }
}
function Get-ItemProperty {
    [CmdletBinding()]param([string]$LiteralPath, [string]$Name)
    if ($Name -eq 'VerifiedAndReputablePolicyState') { throw [UnauthorizedAccessException]::new('PRIVATE-ACCOUNT C:\Users\PrivatePerson access denied') }
    return [pscustomobject]@{ CurrentBuildNumber = '26100'; UBR = 1; DisplayVersion = '24H2'; EditionID = 'Professional'; RegisteredOwner = 'PRIVATE-ACCOUNT'; ProductId = 'PRIVATE-LICENSE' }
}
function Get-WinEvent {
    [CmdletBinding()]param([hashtable]$FilterHashtable, [int]$MaxEvents)
    throw [UnauthorizedAccessException]::new('PRIVATE-ACCOUNT C:\Users\PrivatePerson event access denied')
}
$report = Get-Mir2LaunchBlockReport -Path $gamePath
Assert-Fixture ($report.authenticode.status -eq 'NotSigned') 'an unsigned EXE remains explicitly unsigned'
Assert-Fixture ($report.codeIntegrity.state -eq 'unavailable' -and $report.smartAppControl -eq $null) 'read failure is not Off/no-block'
Assert-Fixture ($report.readFailures.Count -eq 2) 'retain both SAC and event-log read failures'
$failureJson = $report | ConvertTo-Json -Depth 10
foreach ($private in @('PRIVATE-ACCOUNT', 'PrivatePerson', 'PRIVATE-LICENSE', 'RegisteredOwner', 'InvocationInfo', 'ScriptStackTrace')) {
    Assert-Fixture (-not $failureJson.Contains($private)) "reader failures/public OS fields must not leak $private"
}

foreach ($invalidPath in @('mir2-platform-windows.exe', 'C:mir2-platform-windows.exe', '\mir2-platform-windows.exe')) {
    $rejected = $false
    try { Get-Mir2LaunchBlockReport -Path $invalidPath | Out-Null } catch { $rejected = $true }
    Assert-Fixture $rejected 'relative paths must not silently inspect a different executable'
}

# A readable but empty log must remain distinct from the access-denied case.
# Windows Get-WinEvent uses NoMatchingEventsFound rather than an empty array.
function Get-ItemProperty {
    [CmdletBinding()]param([string]$LiteralPath, [string]$Name)
    if ($Name -eq 'VerifiedAndReputablePolicyState') { return [pscustomobject]@{ VerifiedAndReputablePolicyState = 2 } }
    return [pscustomobject]@{ CurrentBuildNumber = '26100'; UBR = 1 }
}
function Get-WinEvent {
    [CmdletBinding()]param([hashtable]$FilterHashtable, [int]$MaxEvents)
    Assert-Fixture ($MaxEvents -eq 4096) 'event reading must have a finite count bound'
    Assert-Fixture ($FilterHashtable.LogName -eq 'Microsoft-Windows-CodeIntegrity/Operational') 'only the CodeIntegrity channel is queried'
    Assert-Fixture (($FilterHashtable.Id -join ',') -eq '3077,3033,3089') 'only the three selected event families are queried'
    Assert-Fixture (($FilterHashtable.EndTime - $FilterHashtable.StartTime).TotalHours -eq 48) 'the event query is exactly the preceding 48 hours'
    $failure = [Management.Automation.ErrorRecord]::new([Exception]::new('fixture no records'), 'NoMatchingEventsFound', [Management.Automation.ErrorCategory]::ObjectNotFound, $null)
    $PSCmdlet.ThrowTerminatingError($failure)
}
$emptyReport = Get-Mir2LaunchBlockReport -Path $gamePath
Assert-Fixture ($emptyReport.codeIntegrity.state -eq 'read' -and $emptyReport.codeIntegrity.events.Count -eq 0) 'no records is a successful empty read'
Assert-Fixture ($emptyReport.readFailures.Count -eq 0) 'no records must not be reported as an access failure'
Assert-Fixture ($emptyReport.smartAppControl.label -eq 'Evaluation' -and $emptyReport.smartAppControl.VerifiedAndReputablePolicyState -eq 2) 'preserve the SAC evaluation state without calling it an enforced block'

$tokens = $null; $parseErrors = $null
$ast = [Management.Automation.Language.Parser]::ParseFile($diagnostic, [ref]$tokens, [ref]$parseErrors)
Assert-Fixture ($parseErrors.Count -eq 0) 'diagnostic syntax parses'
$commands = @($ast.FindAll({ param($node) $node -is [Management.Automation.Language.CommandAst] }, $true) | ForEach-Object { $_.GetCommandName() })
foreach ($forbidden in @('Start-Process', 'Set-ExecutionPolicy', 'Set-ItemProperty', 'New-ItemProperty', 'Remove-ItemProperty', 'Import-Certificate', 'Import-PfxCertificate', 'Set-MpPreference', 'Add-MpPreference', 'Invoke-WebRequest', 'Invoke-RestMethod', 'Add-Type')) {
    Assert-Fixture ($commands -notcontains $forbidden) "diagnostic must not invoke $forbidden"
}
Write-Output "LAUNCH_BLOCK_DIAGNOSTIC_SELFTEST=passed ($script:Assertions offline assertions; no affected-machine acceptance implied)"
