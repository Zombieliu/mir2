# Read-only application-control diagnostics for one explicitly selected EXE.
# Does not launch the EXE, change policy/trust, or transmit the report.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$GameExePath,
    [string]$OutputPath = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Protect-Mir2DiagnosticText {
    param([AllowNull()][object]$Value, [int]$Limit = 256)
    if ($null -eq $Value) { return $null }
    $text = [string]$Value
    $text = [regex]::Replace($text, '(?i)([A-Z]:[\\/]|[\\/]Device[\\/]HarddiskVolume\d+[\\/])Users[\\/][^\\/]+', '$1Users\<redacted>')
    foreach ($privateValue in @($env:USERPROFILE, $env:USERNAME)) {
        if (-not [string]::IsNullOrWhiteSpace($privateValue)) {
            $text = [regex]::Replace($text, [regex]::Escape($privateValue), '<redacted>', [Text.RegularExpressions.RegexOptions]::IgnoreCase)
        }
    }
    $text = [regex]::Replace($text, '[\p{C}\p{Zl}\p{Zp}]', ' ')
    if ($text.Length -gt $Limit) { $text = $text.Substring(0, $Limit) + '...' }
    return $text
}

function Get-Mir2ReadFailure {
    param([string]$Stage, [System.Management.Automation.ErrorRecord]$Failure)
    # Exception.Message, TargetObject, stack and InvocationInfo can contain
    # personal paths or account names; retain a useful error class/code only.
    return [ordered]@{
        stage = $Stage
        state = 'unavailable'
        errorType = $Failure.Exception.GetType().Name
        hresult = '0x' + $Failure.Exception.HResult.ToString('X8')
        note = 'This read failed; absence of data is not evidence that no policy or block exists.'
    }
}

function Get-Mir2HexHash {
    param([AllowNull()][object]$Value, [int]$Length = 64)
    if ($null -eq $Value) { return $null }
    $text = ([string]$Value).Replace(' ', '').Trim()
    if ($text -match ('\A[0-9A-Fa-f]{' + $Length + '}\z')) { return $text.ToUpperInvariant() }
    return $null
}

function Get-Mir2NormalizedEventPath {
    param([AllowNull()][object]$Value)
    if ($null -eq $Value) { return '' }
    $text = ([string]$Value).Trim().Trim('"').Replace('/', '\')
    if ($text.StartsWith('\??\', [StringComparison]::Ordinal)) { $text = $text.Substring(4) }
    if ($text.StartsWith('\\?\', [StringComparison]::Ordinal)) { $text = $text.Substring(4) }
    if ($text.StartsWith('UNC\', [StringComparison]::OrdinalIgnoreCase)) { $text = '\\' + $text.Substring(4) }
    return $text.TrimEnd('\')
}

function Get-Mir2EventField {
    param([Collections.IDictionary]$Data, [string[]]$Names)
    foreach ($name in $Names) {
        $key = ($name -replace '[\s_-]', '').ToLowerInvariant()
        if ($Data.Contains($key)) { return [string]$Data[$key] }
    }
    return $null
}

function ConvertFrom-Mir2CodeIntegrityXml {
    param([Parameter(Mandatory = $true)][string]$EventXml)
    if ($EventXml.Length -gt 262144) { throw 'CodeIntegrity event exceeds diagnostic XML bound' }
    $settings = New-Object Xml.XmlReaderSettings
    $settings.DtdProcessing = [Xml.DtdProcessing]::Prohibit
    $settings.XmlResolver = $null
    $source = New-Object IO.StringReader($EventXml)
    $reader = $null
    try {
        $reader = [Xml.XmlReader]::Create($source, $settings)
        $document = New-Object Xml.XmlDocument
        $document.XmlResolver = $null
        $document.Load($reader)
        $ns = New-Object Xml.XmlNamespaceManager($document.NameTable)
        $ns.AddNamespace('e', 'http://schemas.microsoft.com/win/2004/08/events/event')
        $idNode = $document.SelectSingleNode('/e:Event/e:System/e:EventID', $ns)
        $timeNode = $document.SelectSingleNode('/e:Event/e:System/e:TimeCreated', $ns)
        if ($null -eq $idNode -or $null -eq $timeNode) { throw 'Incomplete CodeIntegrity event header' }
        $id = [int]$idNode.InnerText
        if ($id -notin @(3077, 3033, 3089)) { return $null }
        $activity = ''
        $correlation = $document.SelectSingleNode('/e:Event/e:System/e:Correlation', $ns)
        if ($null -ne $correlation) {
            $candidate = [guid]::Empty
            if ([guid]::TryParse($correlation.GetAttribute('ActivityID'), [ref]$candidate) -and $candidate -ne [guid]::Empty) {
                $activity = $candidate.ToString('D')
            }
        }
        $data = [ordered]@{}
        foreach ($node in $document.SelectNodes('/e:Event/e:EventData/e:Data', $ns)) {
            $key = ($node.GetAttribute('Name') -replace '[\s_-]', '').ToLowerInvariant()
            if ($data.Contains($key)) { throw 'Ambiguous duplicate CodeIntegrity field' }
            $data[$key] = $node.InnerText
        }
        return [pscustomobject]@{
            id = $id
            utc = [DateTimeOffset]::Parse($timeNode.GetAttribute('SystemTime'), [Globalization.CultureInfo]::InvariantCulture).UtcDateTime
            activity = $activity
            data = $data
        }
    } finally {
        if ($null -ne $reader) { $reader.Dispose() }
        $source.Dispose()
    }
}

function Get-Mir2CodeIntegrityMatch {
    param([object]$Event, [string]$TargetPath, [string]$TargetSha256, [scriptblock]$DevicePathHashReader = $null)
    $eventPath = Get-Mir2NormalizedEventPath (Get-Mir2EventField -Data $Event.data -Names @('FileName', 'FilePath', 'ImageName'))
    $target = Get-Mir2NormalizedEventPath $TargetPath
    if ($eventPath.Length -gt 0 -and $eventPath.Equals($target, [StringComparison]::OrdinalIgnoreCase)) { return 'exact-file-path' }
    # SHA256Hash is the Authenticode hash; only SHA256FlatHash is comparable to
    # Get-FileHash. Do not confuse them or match the parent ProcessName.
    $flat = Get-Mir2HexHash (Get-Mir2EventField -Data $Event.data -Names @('SHA256FlatHash'))
    if ($flat -and $TargetSha256 -and $flat -ceq $TargetSha256) { return 'exact-flat-sha256' }
    if ($null -ne $DevicePathHashReader -and $eventPath -match '\A\\Device\\HarddiskVolume\d+\\' -and $target -match '\A[A-Za-z]:\\') {
        $tail = $eventPath -replace '\A\\Device\\HarddiskVolume\d+', ''
        if ($tail.Equals($target.Substring(2), [StringComparison]::OrdinalIgnoreCase)) {
            $deviceHash = & $DevicePathHashReader $eventPath
            if ($deviceHash -and $TargetSha256 -and $deviceHash -ceq $TargetSha256) { return 'device-file-current-flat-sha256' }
        }
    }
    return ''
}

function ConvertTo-Mir2PublicCodeIntegrityEvent {
    param([object]$Event, [string]$Basis)
    $result = [ordered]@{
        id = $Event.id
        timeUtc = $Event.utc.ToString('o')
        match = $Basis
        file = '<selected-exe>'
    }
    # No message, XML, ProcessName, user SID/name, machine name or unselected
    # file path is exported. Policy/signing status remains actionable evidence.
    foreach ($name in @('PolicyName')) {
        $value = Get-Mir2EventField -Data $Event.data -Names @($name)
        if ($null -ne $value) { $result[$name] = Protect-Mir2DiagnosticText $value }
    }
    foreach ($name in @('PolicyId', 'PolicyGuid')) {
        $value = Get-Mir2EventField -Data $Event.data -Names @($name)
        $guid = [guid]::Empty
        if ($value -and [guid]::TryParse($value, [ref]$guid)) { $result[$name] = $guid.ToString('D') }
    }
    foreach ($name in @('Status', 'RequestedSigningLevel', 'ValidatedSigningLevel', 'VerificationError', 'TotalSignatureCount', 'Signature', 'SignatureType')) {
        $value = Get-Mir2EventField -Data $Event.data -Names @($name)
        if ($null -ne $value) {
            $result[$name] = if ($value -match '\A(?:0x[0-9a-fA-F]+|[0-9]+)\z') { $value } else { '<unrecognized-status>' }
        }
    }
    foreach ($name in @('SHA256FlatHash', 'SHA256Hash', 'PolicyHash')) {
        $hash = Get-Mir2HexHash (Get-Mir2EventField -Data $Event.data -Names @($name))
        if ($hash) { $result[$name] = $hash }
    }
    return [pscustomobject]$result
}

function Select-Mir2CodeIntegrityEvents {
    param([object[]]$Events, [string]$TargetPath, [string]$TargetSha256, [datetime]$SinceUtc, [datetime]$UntilUtc, [scriptblock]$DevicePathHashReader = $null)
    $window = @($Events | Where-Object { $null -ne $_ -and $_.utc -ge $SinceUtc -and $_.utc -le $UntilUtc })
    $selected = New-Object Collections.Generic.List[object]
    $activities = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($event in $window) {
        if ($event.id -eq 3089) { continue }
        $basis = Get-Mir2CodeIntegrityMatch -Event $event -TargetPath $TargetPath -TargetSha256 $TargetSha256 -DevicePathHashReader $DevicePathHashReader
        if (-not $basis) { continue }
        if ($event.activity) { [void]$activities.Add($event.activity) }
        [void]$selected.Add((ConvertTo-Mir2PublicCodeIntegrityEvent -Event $event -Basis $basis))
    }
    foreach ($event in $window) {
        if ($event.id -ne 3089) { continue }
        $basis = Get-Mir2CodeIntegrityMatch -Event $event -TargetPath $TargetPath -TargetSha256 $TargetSha256 -DevicePathHashReader $DevicePathHashReader
        if (-not $basis -and $event.activity -and $activities.Contains($event.activity)) {
            # Signature events normally have no filename. A conflicting explicit
            # filename must not be included merely because an ActivityID repeats.
            $explicitPath = Get-Mir2EventField -Data $event.data -Names @('FileName', 'FilePath', 'ImageName')
            if ([string]::IsNullOrWhiteSpace($explicitPath)) { $basis = 'correlated-selected-file-activity' }
        }
        if ($basis) { [void]$selected.Add((ConvertTo-Mir2PublicCodeIntegrityEvent -Event $event -Basis $basis)) }
    }
    return @($selected | Sort-Object timeUtc, id)
}

function Get-Mir2CertificateSummary {
    param([AllowNull()][object]$Certificate)
    if ($null -eq $Certificate) { return $null }
    return [ordered]@{
        subject = Protect-Mir2DiagnosticText $Certificate.Subject
        issuer = Protect-Mir2DiagnosticText $Certificate.Issuer
        thumbprint = $Certificate.Thumbprint
        notBeforeUtc = $Certificate.NotBefore.ToUniversalTime().ToString('o')
        notAfterUtc = $Certificate.NotAfter.ToUniversalTime().ToString('o')
    }
}

function Get-Mir2LaunchBlockReport {
    param([Parameter(Mandatory = $true)][string]$Path)
    # IsPathRooted also accepts drive-relative C:game.exe and root-relative
    # \game.exe on Windows. Require a fully qualified drive or UNC path.
    if ($Path -notmatch '\A(?:[A-Za-z]:[\\/]|\\\\[^\\/]+[\\/][^\\/]+[\\/])') { throw 'GameExePath must be an explicit absolute EXE path.' }
    $file = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    if ($file.PSIsContainer -or $file.Extension -ine '.exe') { throw 'GameExePath must identify an existing EXE file.' }
    $now = [datetime]::UtcNow
    $since = $now.AddHours(-48)
    $failures = New-Object Collections.Generic.List[object]
    $report = [ordered]@{
        schema = 'mir2.launch-block-diagnostic.v1'
        collectedUtc = $now.ToString('o')
        file = [ordered]@{ name = Protect-Mir2DiagnosticText $file.Name; bytes = $file.Length; sha256 = $null; version = $null }
        authenticode = $null
        os = $null
        smartAppControl = $null
        codeIntegrity = [ordered]@{ fromUtc = $since.ToString('o'); untilUtc = $now.ToString('o'); state = 'unavailable'; events = @() }
        readFailures = @()
        interpretation = 'A launch error or SAC state alone does not identify the blocking policy. No matching event is not proof of no block; policy, log access, clock and retention can limit evidence.'
    }
    try { $report.file.sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256 -ErrorAction Stop).Hash.ToUpperInvariant() }
    catch { [void]$failures.Add((Get-Mir2ReadFailure -Stage 'file-sha256' -Failure $_)) }
    try {
        $version = $file.VersionInfo
        $report.file.version = [ordered]@{ fileVersion = Protect-Mir2DiagnosticText $version.FileVersion; productVersion = Protect-Mir2DiagnosticText $version.ProductVersion }
    } catch { [void]$failures.Add((Get-Mir2ReadFailure -Stage 'file-version' -Failure $_)) }
    try {
        $signature = Get-AuthenticodeSignature -LiteralPath $file.FullName -ErrorAction Stop
        $report.authenticode = [ordered]@{
            status = [string]$signature.Status
            signatureType = [string]$signature.SignatureType
            signer = Get-Mir2CertificateSummary $signature.SignerCertificate
            timestampSigner = Get-Mir2CertificateSummary $signature.TimeStamperCertificate
            note = 'Authenticode describes this EXE. A separate package CMS signature does not Authenticode-sign the EXE; a valid signature alone does not prove this machine policy allows it.'
        }
    } catch { [void]$failures.Add((Get-Mir2ReadFailure -Stage 'authenticode' -Failure $_)) }
    try {
        $os = Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -ErrorAction Stop
        $report.os = [ordered]@{}
        foreach ($name in @('CurrentBuildNumber', 'UBR', 'DisplayVersion', 'EditionID')) {
            $property = $os.PSObject.Properties[$name]
            if ($null -ne $property) { $report.os[$name] = Protect-Mir2DiagnosticText $property.Value }
        }
    } catch { [void]$failures.Add((Get-Mir2ReadFailure -Stage 'os-version' -Failure $_)) }
    try {
        $policy = Get-ItemProperty -LiteralPath 'HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy' -Name VerifiedAndReputablePolicyState -ErrorAction Stop
        $value = [int]$policy.VerifiedAndReputablePolicyState
        $label = switch ($value) { 0 { 'Off' } 1 { 'Enforcement' } 2 { 'Evaluation' } default { 'Unknown value' } }
        $report.smartAppControl = [ordered]@{ state = 'read'; VerifiedAndReputablePolicyState = $value; label = $label; note = 'This registry value does not enumerate other App Control policies.' }
    } catch { [void]$failures.Add((Get-Mir2ReadFailure -Stage 'smart-app-control-state' -Failure $_)) }

    try {
        $limit = 4096
        $records = @()
        try {
            $records = @(Get-WinEvent -FilterHashtable @{ LogName = 'Microsoft-Windows-CodeIntegrity/Operational'; Id = @(3077, 3033, 3089); StartTime = $since; EndTime = $now } -MaxEvents $limit -ErrorAction Stop)
        } catch {
            if ($_.FullyQualifiedErrorId -notlike 'NoMatchingEventsFound*') { throw }
        }
        $parsed = New-Object Collections.Generic.List[object]
        $parseFailures = 0
        foreach ($record in $records) {
            try { [void]$parsed.Add((ConvertFrom-Mir2CodeIntegrityXml -EventXml $record.ToXml())) }
            catch { $parseFailures++ }
        }
        $deviceState = @{ cache = @{}; failures = 0; capped = $false }
        $deviceReader = {
            param([string]$DevicePath)
            if ($deviceState.cache.ContainsKey($DevicePath)) { return $deviceState.cache[$DevicePath] }
            if ($deviceState.cache.Count -ge 16) { $deviceState.capped = $true; return $null }
            $hash = $null
            try {
                $hash = (Get-FileHash -LiteralPath ('\\?\GLOBALROOT' + $DevicePath) -Algorithm SHA256 -ErrorAction Stop).Hash.ToUpperInvariant()
            } catch { $deviceState.failures++ }
            $deviceState.cache[$DevicePath] = $hash
            return $hash
        }.GetNewClosure()
        $events = @(Select-Mir2CodeIntegrityEvents -Events $parsed.ToArray() -TargetPath $file.FullName -TargetSha256 $report.file.sha256 -SinceUtc $since -UntilUtc $now -DevicePathHashReader $deviceReader)
        $report.codeIntegrity.state = 'read'
        $report.codeIntegrity['scanLimitReached'] = ($records.Count -ge $limit)
        $report.codeIntegrity['parseFailures'] = $parseFailures
        $report.codeIntegrity['devicePathReadFailures'] = $deviceState.failures
        $report.codeIntegrity['devicePathReadLimitReached'] = $deviceState.capped
        $report.codeIntegrity['matchingEventCount'] = $events.Count
        $report.codeIntegrity['reportLimitReached'] = ($events.Count -gt 128)
        $report.codeIntegrity.events = @($events | Select-Object -Last 128)
    } catch { [void]$failures.Add((Get-Mir2ReadFailure -Stage 'code-integrity-log' -Failure $_)) }
    $report.readFailures = $failures.ToArray()
    return [pscustomobject]$report
}

# Dot-sourcing only defines functions for the controlled offline fixture tests.
if ($MyInvocation.InvocationName -ne '.') {
    try {
        $report = Get-Mir2LaunchBlockReport -Path $GameExePath
        $json = $report | ConvertTo-Json -Depth 12
        if ([string]::IsNullOrWhiteSpace($OutputPath)) { $json }
        else {
            # Report writing is explicit, refuses overwrite, and never creates
            # directories or modifies the inspected game/policy/certificate.
            $destination = [IO.Path]::GetFullPath($OutputPath)
            $stream = [IO.File]::Open($destination, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
            try {
                $bytes = [Text.UTF8Encoding]::new($false).GetBytes($json + [Environment]::NewLine)
                $stream.Write($bytes, 0, $bytes.Length)
            } finally { $stream.Dispose() }
            Write-Output 'Diagnostic report saved to the explicitly requested new file. Nothing was uploaded.'
        }
    } catch {
        $failure = Get-Mir2ReadFailure -Stage 'diagnostic-input-or-output' -Failure $_
        $failure | ConvertTo-Json -Depth 4
        exit 1
    }
}
