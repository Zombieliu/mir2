#requires -Version 5.1
<#
.SYNOPSIS
    Read-only observation of a running Windows native Candidate process.

.DESCRIPTION
    Samples one already-running mir2-platform-windows.exe process, optionally
    probes a gateway health endpoint, records relevant Windows event IDs, and
    emits a small evidence packet. This script does not start, stop, attach to,
    or configure the client, gateway, Windows, or any debugger.

    A normal run is a 30-minute gate by default. A report is PASS only when the
    requested run is complete, the hard 30-minute requirement is complete, the
    native PID identity is verified, event observation was available, Gateway
    health and client-log evidence were supplied and clean, no relevant Windows
    crash event was observed, and the post-warmup RSS final value is within
    125% of the warmup baseline. The client log must also contain current-window
    runtime and effect-producer soak metrics spanning at least 29 minutes, with
    active effects inside their cap and additive cache entries backed by live
    assets; retained scene totals must not grow monotonically after the 10-minute
    warmup. Health/log parameters remain syntactically optional for exploratory
    runs, but omitting either keeps the gate failed.

    This script does not launch the client. Start the candidate separately with
    MIR2_NATIVE_SOAK_METRICS=1 and redirect its stderr to ClientLogPath before
    invoking the observer. verify-windows-candidate.ps1 deliberately clears the
    metrics variable because it validates a normal packaged launch.

.EXAMPLE
    .\monitor-native-candidate-soak.ps1 -ProcessId 12345 -OutputDirectory .\soak

.EXAMPLE
    .\monitor-native-candidate-soak.ps1 -ProcessId 12345 -GatewayHealthUrl 'http://127.0.0.1:7110/health' -ClientLogPath .\client.log -OutputDirectory .\soak

.EXAMPLE
    .\monitor-native-candidate-soak.ps1 -SelfTest
#>
[CmdletBinding()]
param(
    [int]$ProcessId = 0,
    [ValidateRange(0.001, 1440)]
    [double]$DurationMinutes = 30,
    # 10 seconds is the default. Values below 10 are intentionally allowed so
    # SelfTest can complete in about two seconds without relaxing normal gates.
    [ValidateRange(1, 3600)]
    [int]$SampleIntervalSeconds = 10,
    [string]$OutputDirectory = '',
    [string]$GatewayHealthUrl = '',
    [string]$ClientLogPath = '',
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:Schema = 'mir2.windows.native-candidate-soak.v1'
$script:ExpectedProcessName = 'mir2-platform-windows.exe'
$script:MinimumRequiredMinutes = 30
$script:MinimumTelemetrySpanSeconds = 1740
$script:MaximumTelemetryGapMs = 30000
$script:ExpectedActiveEffectsCap = 96
$script:OutputWriteErrors = New-Object System.Collections.ArrayList
$script:Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Get-UtcTimestamp {
    return [DateTimeOffset]::UtcNow.ToString('o', [Globalization.CultureInfo]::InvariantCulture)
}

function Get-FullPathSafe {
    param([Parameter(Mandatory = $true)][string]$Path)
    return [IO.Path]::GetFullPath($Path)
}

function Add-OutputWriteError {
    param([Parameter(Mandatory = $true)][string]$Kind)
    [void]$script:OutputWriteErrors.Add($Kind)
}

function Write-ObserverLog {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Message
    )
    $line = ('{0} {1}{2}' -f (Get-UtcTimestamp), $Message, [Environment]::NewLine)
    try {
        [IO.File]::AppendAllText($Path, $line, $script:Utf8NoBom)
    } catch {
        Add-OutputWriteError -Kind 'soak-client.log-write-failed'
    }
}

function Write-AtomicJson {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )
    $parent = Split-Path -Parent $Path
    $temporary = Join-Path $parent ('.' + (Split-Path -Leaf $Path) + '.' + [guid]::NewGuid().ToString('N') + '.tmp')
    try {
        $json = $Value | ConvertTo-Json -Depth 30
        [IO.File]::WriteAllText($temporary, $json, $script:Utf8NoBom)
        if (Test-Path -LiteralPath $Path) {
            try {
                [IO.File]::Replace($temporary, $Path, $null)
            } catch {
                # Move-Item uses a same-directory rename on the normal path;
                # this is retained as a compatibility fallback for PS 5.1.
                Move-Item -LiteralPath $temporary -Destination $Path -Force
            }
        } else {
            Move-Item -LiteralPath $temporary -Destination $Path -Force
        }
    } catch {
        Add-OutputWriteError -Kind 'soak-30m.json-write-failed'
        throw
    } finally {
        if (Test-Path -LiteralPath $temporary) {
            try { Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue } catch { }
        }
    }
}

function Write-MemoryCsv {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Samples
    )
    try {
        if (@($Samples).Count -eq 0) {
            $header = 'timestamp,elapsed,pid,alive,workingSet,privateBytes,cpuSeconds,cpuPercentApprox,threadCount,handleCount,responding'
            [IO.File]::WriteAllText($Path, $header + [Environment]::NewLine, $script:Utf8NoBom)
            return
        }
        $csvRows = foreach ($sample in @($Samples)) {
            [pscustomobject]([ordered]@{
                timestamp = $sample.timestamp
                elapsed = $sample.elapsed
                pid = $sample.pid
                alive = $sample.alive
                workingSet = $sample.workingSet
                privateBytes = $sample.privateBytes
                cpuSeconds = $sample.cpuSeconds
                cpuPercentApprox = $sample.cpuPercentApprox
                threadCount = $sample.threadCount
                handleCount = $sample.handleCount
                responding = $sample.responding
            })
        }
        $csvLines = @($csvRows | ConvertTo-Csv -NoTypeInformation)
        [IO.File]::WriteAllLines($Path, $csvLines, $script:Utf8NoBom)
    } catch {
        Add-OutputWriteError -Kind 'memory-samples.csv-write-failed'
    }
}

function Get-Origin {
    param([Parameter(Mandatory = $true)][string]$Url)
    try {
        $uri = New-Object System.Uri($Url)
        if ($uri.Scheme -notin @('http', 'https')) { return $null }
        $hostPart = $uri.Host
        if ($hostPart -like '*:*' -and $hostPart -notlike '[*]') {
            $hostPart = '[' + $hostPart + ']'
        }
        $portPart = ''
        if (-not (($uri.Scheme -eq 'http' -and $uri.Port -eq 80) -or ($uri.Scheme -eq 'https' -and $uri.Port -eq 443))) {
            $portPart = ':' + $uri.Port.ToString([Globalization.CultureInfo]::InvariantCulture)
        }
        # Deliberately construct from scheme/host/port; user-info, path,
        # query, and fragment are never written to evidence.
        return ($uri.Scheme.ToLowerInvariant() + '://' + $hostPart + $portPart)
    } catch {
        return $null
    }
}

function Invoke-HealthCheck {
    param(
        [Parameter(Mandatory = $true)][string]$Url,
        [Parameter(Mandatory = $true)][string]$Phase,
        [double]$ElapsedSeconds = 0
    )
    $origin = Get-Origin -Url $Url
    $result = [ordered]@{
        timestamp = Get-UtcTimestamp
        phase = $Phase
        elapsed = [math]::Round($ElapsedSeconds, 3)
        origin = $origin
        status = $null
        ok = $false
        latencyMs = $null
        error = $null
    }
    if ($null -eq $origin) {
        $result.error = 'invalid-or-unsupported-health-url'
        return $result
    }

    $stopwatch = [Diagnostics.Stopwatch]::StartNew()
    $response = $null
    try {
        $request = [Net.WebRequest]::Create($Url)
        $request.Method = 'GET'
        $request.Timeout = 5000
        if ($request -is [Net.HttpWebRequest]) {
            $request.ReadWriteTimeout = 5000
            $request.AllowAutoRedirect = $true
        }
        $response = $request.GetResponse()
        $result.status = [int]$response.StatusCode
        $result.ok = ($result.status -ge 200 -and $result.status -lt 400)
        if (-not $result.ok) { $result.error = 'http-non-success' }
    } catch [Net.WebException] {
        if ($null -ne $_.Exception.Response) {
            try {
                $result.status = [int]$_.Exception.Response.StatusCode
                $result.error = 'http-non-success'
            } catch {
                $result.error = 'request-failed'
            }
        } elseif ($_.Exception.Status -eq [Net.WebExceptionStatus]::Timeout) {
            $result.error = 'timeout'
        } else {
            $result.error = 'request-failed'
        }
    } catch {
        $result.error = 'request-failed'
    } finally {
        $stopwatch.Stop()
        $result.latencyMs = [math]::Round($stopwatch.Elapsed.TotalMilliseconds, 1)
        if ($null -ne $response) {
            try { $response.Dispose() } catch { }
        }
    }
    return $result
}

function Get-ProcessIdentity {
    param([Parameter(Mandatory = $true)][int]$TargetPid)
    $identity = [ordered]@{
        pid = $TargetPid
        expectedProcessName = $script:ExpectedProcessName
        processName = $null
        executablePath = $null
        startTimeUtc = $null
        nameMatch = $false
        pathMatch = $null
        verified = $false
        reason = $null
    }
    $process = $null
    try {
        $process = Get-Process -Id $TargetPid -ErrorAction Stop
    } catch {
        $identity.reason = 'process-not-found'
        return $identity
    }

    $identity.processName = ($process.ProcessName + '.exe')
    $identity.nameMatch = [String]::Equals($identity.processName, $script:ExpectedProcessName, [StringComparison]::OrdinalIgnoreCase)
    try { $identity.startTimeUtc = $process.StartTime.ToUniversalTime().ToString('o', [Globalization.CultureInfo]::InvariantCulture) } catch { }

    $path = $null
    try { $path = $process.Path } catch { }
    if ([string]::IsNullOrWhiteSpace($path)) {
        try { $path = $process.MainModule.FileName } catch { }
    }
    if ([string]::IsNullOrWhiteSpace($path)) {
        try {
            $cim = Get-CimInstance -ClassName Win32_Process -Filter ('ProcessId={0}' -f $TargetPid) -ErrorAction Stop
            $path = $cim.ExecutablePath
        } catch {
            try {
                $wmi = Get-WmiObject -Class Win32_Process -Filter ('ProcessId={0}' -f $TargetPid) -ErrorAction Stop
                $path = $wmi.ExecutablePath
            } catch { }
        }
    }
    if (-not [string]::IsNullOrWhiteSpace($path)) {
        $identity.executablePath = $path
        $identity.pathMatch = [String]::Equals((Split-Path -Leaf $path), $script:ExpectedProcessName, [StringComparison]::OrdinalIgnoreCase)
    }

    if (-not $identity.nameMatch) {
        $identity.reason = 'process-name-mismatch'
    } elseif ($false -eq $identity.pathMatch) {
        $identity.reason = 'executable-path-mismatch'
    } elseif ($null -eq $identity.startTimeUtc) {
        $identity.reason = 'process-start-time-unavailable'
    } else {
        # A protected process may hide its path. The OS process name is still
        # checked, but the evidence keeps pathMatch=null so this limitation is
        # visible to the reviewer.
        $identity.verified = $true
        if ($null -eq $identity.pathMatch) { $identity.reason = 'verified-by-process-name-path-unavailable' }
        else { $identity.reason = 'verified' }
    }
    return $identity
}

function Get-ProcessSample {
    param(
        [Parameter(Mandatory = $true)][int]$TargetPid,
        [Parameter(Mandatory = $true)][double]$ElapsedSeconds,
        $PreviousSample,
        [Parameter(Mandatory = $true)][double]$PreviousWallSeconds
    )
    $sample = [ordered]@{
        timestamp = Get-UtcTimestamp
        elapsed = [math]::Round($ElapsedSeconds, 3)
        pid = $TargetPid
        alive = $false
        workingSet = $null
        privateBytes = $null
        cpuSeconds = $null
        cpuPercentApprox = $null
        threadCount = $null
        handleCount = $null
        responding = $null
    }
    $process = $null
    try { $process = Get-Process -Id $TargetPid -ErrorAction Stop } catch { return $sample }
    $sample.alive = $true
    try { $sample.workingSet = [int64]$process.WorkingSet64 } catch { }
    try { $sample.privateBytes = [int64]$process.PrivateMemorySize64 } catch { }
    try { $sample.cpuSeconds = [math]::Round($process.TotalProcessorTime.TotalSeconds, 3) } catch { }
    try { $sample.threadCount = [int]$process.Threads.Count } catch { }
    try { $sample.handleCount = [int]$process.HandleCount } catch { }
    try { $sample.responding = [bool]$process.Responding } catch { }

    if ($null -ne $PreviousSample -and $null -ne $sample.cpuSeconds -and $null -ne $PreviousSample.cpuSeconds) {
        $wallDelta = $ElapsedSeconds - $PreviousWallSeconds
        $cpuDelta = [double]$sample.cpuSeconds - [double]$PreviousSample.cpuSeconds
        if ($wallDelta -gt 0) {
            $percent = ($cpuDelta / $wallDelta) / [Environment]::ProcessorCount * 100
            $sample.cpuPercentApprox = [math]::Round([math]::Max(0, [math]::Min(100, $percent)), 2)
        }
    }
    return $sample
}

function Get-ClientLogIdentity {
    param([string]$Path)
    $identity = [ordered]@{
        enabled = (-not [string]::IsNullOrWhiteSpace($Path))
        available = $false
        fullPath = $null
        length = [int64]0
        creationTimeUtc = $null
        fileId = $null
        reason = $null
    }
    if (-not $identity.enabled) {
        $identity.reason = 'client-log-not-supplied'
        return $identity
    }
    try {
        if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
            $identity.reason = 'client-log-not-found'
            return $identity
        }
        $item = Get-Item -LiteralPath $Path -ErrorAction Stop
        $identity.fullPath = $item.FullName
        $identity.length = [int64]$item.Length
        $identity.creationTimeUtc = $item.CreationTimeUtc.ToString('o', [Globalization.CultureInfo]::InvariantCulture)

        # File ID remains stable when a writer appends to or truncates the same
        # file, but changes when log rotation replaces the path. Creation time
        # and canonical path are retained as independently reviewable evidence.
        $query = @(& fsutil.exe file queryfileid $item.FullName 2>$null)
        if ($LASTEXITCODE -ne 0) {
            $identity.reason = 'client-log-file-id-unavailable'
            return $identity
        }
        $match = [Text.RegularExpressions.Regex]::Match(($query -join "`n"), '(?i)0x[0-9a-f]+')
        if (-not $match.Success) {
            $identity.reason = 'client-log-file-id-unavailable'
            return $identity
        }
        $identity.fileId = $match.Value.ToLowerInvariant()
        $identity.available = $true
        $identity.reason = 'verified'
        return $identity
    } catch {
        $identity.reason = 'client-log-identity-read-failed'
        return $identity
    }
}

function Test-ClientLogIdentityMatch {
    param(
        [Parameter(Mandatory = $true)]$Expected,
        [Parameter(Mandatory = $true)]$Actual
    )
    if (-not $Expected.available -or -not $Actual.available) { return $false }
    return (
        [String]::Equals([string]$Expected.fullPath, [string]$Actual.fullPath, [StringComparison]::OrdinalIgnoreCase) -and
        [String]::Equals([string]$Expected.creationTimeUtc, [string]$Actual.creationTimeUtc, [StringComparison]::Ordinal) -and
        [String]::Equals([string]$Expected.fileId, [string]$Actual.fileId, [StringComparison]::OrdinalIgnoreCase)
    )
}

function Read-ClientLogTail {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][int64]$StartOffset
    )
    $result = [ordered]@{
        available = $false
        complete = $false
        reason = $null
        text = ''
        startOffset = $StartOffset
        endOffset = $null
    }
    $stream = $null
    $reader = $null
    try {
        if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
            $result.reason = 'client-log-not-found'
            return $result
        }
        $fullPath = Get-FullPathSafe -Path $Path
        $stream = New-Object IO.FileStream(
            $fullPath,
            [IO.FileMode]::Open,
            [IO.FileAccess]::Read,
            ([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete)
        )
        if ($stream.Length -lt $StartOffset) {
            $result.reason = 'client-log-truncated-during-observation'
            return $result
        }
        [void]$stream.Seek($StartOffset, [IO.SeekOrigin]::Begin)
        $reader = New-Object IO.StreamReader($stream, [Text.Encoding]::UTF8, $true, 4096, $true)
        $result.text = $reader.ReadToEnd()
        $result.endOffset = [int64]$stream.Position
        $result.available = $true
        $result.complete = $true
        return $result
    } catch {
        $result.reason = 'client-log-read-failed'
        return $result
    } finally {
        if ($null -ne $reader) { try { $reader.Dispose() } catch { } }
        if ($null -ne $stream) { try { $stream.Dispose() } catch { } }
    }
}

function Convert-NativeSoakTelemetry {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)

    $runtimeFields = @(
        'processId', 'timestampMs', 'snapshotEffects', 'retainedEffectPrimary',
        'retainedEffectMasks', 'retainedEffectShadows', 'retainedEffectImages',
        'retainedEntityLayers', 'legacySceneEntities', 'entityAtlases',
        'mapRenderTiles', 'mapSpawnedEntities', 'mineNodes', 'lightingLayers',
        'lightingImages', 'additiveCacheEntries', 'additiveCacheLiveEntries',
        'additiveAssetCount'
    )
    $fxFields = @('processId', 'timestampMs', 'activeEffects', 'activeEffectsCap')
    $runtimeSamples = New-Object System.Collections.ArrayList
    $fxSamples = New-Object System.Collections.ArrayList
    $parseErrorCount = 0
    $pattern = '(?m)^\s*\[(native-soak|native-soak-fx)\]\s+(\{[^\r\n]*\})\s*$'
    $matches = [Text.RegularExpressions.Regex]::Matches($Text, $pattern)
    $taggedLineCount = [Text.RegularExpressions.Regex]::Matches($Text, '(?m)^.*\[native-soak(?:-fx)?\].*$').Count
    $malformedTaggedLineCount = [math]::Max(0, $taggedLineCount - $matches.Count)

    foreach ($match in $matches) {
        try {
            $kind = $match.Groups[1].Value
            $value = $match.Groups[2].Value | ConvertFrom-Json -ErrorAction Stop
            $fields = if ($kind -eq 'native-soak') { $runtimeFields } else { $fxFields }
            $sample = [ordered]@{}
            foreach ($field in $fields) {
                $property = $value.PSObject.Properties[$field]
                if ($null -eq $property -or $null -eq $property.Value) {
                    throw "missing telemetry field: $field"
                }
                $number = [double]$property.Value
                if ([double]::IsNaN($number) -or [double]::IsInfinity($number) -or $number -lt 0 -or [math]::Floor($number) -ne $number -or $number -gt [int64]::MaxValue) {
                    throw "invalid telemetry field: $field"
                }
                $sample[$field] = [int64]$number
            }
            if ($kind -eq 'native-soak') {
                $sample['retainedTotal'] = [int64](
                    $sample.retainedEffectPrimary + $sample.retainedEffectMasks +
                    $sample.retainedEffectShadows + $sample.retainedEntityLayers +
                    $sample.legacySceneEntities + $sample.mapRenderTiles +
                    $sample.mapSpawnedEntities + $sample.mineNodes +
                    $sample.lightingLayers
                )
                [void]$runtimeSamples.Add($sample)
            }
            else { [void]$fxSamples.Add($sample) }
        } catch {
            $parseErrorCount += 1
        }
    }

    $runtimeStrict = $true
    $fxStrict = $true
    [int64]$runtimeMaxGapMs = 0
    [int64]$fxMaxGapMs = 0
    for ($index = 1; $index -lt $runtimeSamples.Count; $index++) {
        $gap = [int64]$runtimeSamples[$index].timestampMs - [int64]$runtimeSamples[$index - 1].timestampMs
        if ($gap -le 0) { $runtimeStrict = $false }
        else { $runtimeMaxGapMs = [math]::Max($runtimeMaxGapMs, $gap) }
    }
    for ($index = 1; $index -lt $fxSamples.Count; $index++) {
        $gap = [int64]$fxSamples[$index].timestampMs - [int64]$fxSamples[$index - 1].timestampMs
        if ($gap -le 0) { $fxStrict = $false }
        else { $fxMaxGapMs = [math]::Max($fxMaxGapMs, $gap) }
    }

    $runtimeSpanSeconds = if ($runtimeSamples.Count -ge 2) {
        [math]::Round(([double]$runtimeSamples[$runtimeSamples.Count - 1].timestampMs - [double]$runtimeSamples[0].timestampMs) / 1000, 3)
    } else { 0 }
    $fxSpanSeconds = if ($fxSamples.Count -ge 2) {
        [math]::Round(([double]$fxSamples[$fxSamples.Count - 1].timestampMs - [double]$fxSamples[0].timestampMs) / 1000, 3)
    } else { 0 }
    $activeWithinCap = @($fxSamples | Where-Object { [int64]$_.activeEffects -gt [int64]$_.activeEffectsCap }).Count -eq 0
    $activeCapValid = @($fxSamples | Where-Object { [int64]$_.activeEffectsCap -ne $script:ExpectedActiveEffectsCap }).Count -eq 0
    $cacheWithinAssets = @($runtimeSamples | Where-Object { [int64]$_.additiveCacheEntries -gt [int64]$_.additiveAssetCount }).Count -eq 0
    $cacheHandlesLive = @($runtimeSamples | Where-Object { [int64]$_.additiveCacheEntries -ne [int64]$_.additiveCacheLiveEntries }).Count -eq 0
    $runtimeSamplingDense = $runtimeSamples.Count -ge 2 -and $runtimeMaxGapMs -le $script:MaximumTelemetryGapMs
    $fxSamplingDense = $fxSamples.Count -ge 2 -and $fxMaxGapMs -le $script:MaximumTelemetryGapMs
    $streamsAligned = $runtimeSamples.Count -gt 0 -and $fxSamples.Count -gt 0
    if ($runtimeSamples.Count -gt 0 -and $fxSamples.Count -gt 0) {
        foreach ($sample in $runtimeSamples) {
            $matched = @($fxSamples | Where-Object {
                [math]::Abs([int64]$_.timestampMs - [int64]$sample.timestampMs) -le $script:MaximumTelemetryGapMs
            }).Count -gt 0
            if (-not $matched) { $streamsAligned = $false; break }
        }
        if ($streamsAligned) {
            foreach ($sample in $fxSamples) {
                $matched = @($runtimeSamples | Where-Object {
                    [math]::Abs([int64]$_.timestampMs - [int64]$sample.timestampMs) -le $script:MaximumTelemetryGapMs
                }).Count -gt 0
                if (-not $matched) { $streamsAligned = $false; break }
            }
        }
    }
    $runtimeProcessIds = @($runtimeSamples | ForEach-Object { [int64]$_.processId } | Sort-Object -Unique)
    $fxProcessIds = @($fxSamples | ForEach-Object { [int64]$_.processId } | Sort-Object -Unique)
    [int64]$maxActiveEffects = 0
    foreach ($sample in $fxSamples) { $maxActiveEffects = [math]::Max($maxActiveEffects, [int64]$sample.activeEffects) }
    [int64]$maxRetainedTotal = 0
    [int64]$maxAdditiveCacheEntries = 0
    foreach ($sample in $runtimeSamples) {
        $maxRetainedTotal = [math]::Max($maxRetainedTotal, [int64]$sample.retainedTotal)
        $maxAdditiveCacheEntries = [math]::Max($maxAdditiveCacheEntries, [int64]$sample.additiveCacheEntries)
    }
    $retainedObservedDecrease = $false
    $additiveCacheObservedDecrease = $false
    for ($index = 1; $index -lt $runtimeSamples.Count; $index++) {
        if ([int64]$runtimeSamples[$index].retainedTotal -lt [int64]$runtimeSamples[$index - 1].retainedTotal) { $retainedObservedDecrease = $true }
        if ([int64]$runtimeSamples[$index].additiveCacheEntries -lt [int64]$runtimeSamples[$index - 1].additiveCacheEntries) { $additiveCacheObservedDecrease = $true }
    }
    $retainedMonotonicGrowthAfterWarmup = $false
    if ($runtimeSamples.Count -ge 3) {
        $warmupBoundary = [int64]$runtimeSamples[0].timestampMs + 600000
        $postWarmup = @($runtimeSamples | Where-Object { [int64]$_.timestampMs -ge $warmupBoundary })
        if ($postWarmup.Count -ge 3) {
            $nonDecreasing = $true
            $strictIncreaseObserved = $false
            for ($index = 1; $index -lt $postWarmup.Count; $index++) {
                if ([int64]$postWarmup[$index].retainedTotal -lt [int64]$postWarmup[$index - 1].retainedTotal) { $nonDecreasing = $false }
                if ([int64]$postWarmup[$index].retainedTotal -gt [int64]$postWarmup[$index - 1].retainedTotal) { $strictIncreaseObserved = $true }
            }
            $retainedMonotonicGrowthAfterWarmup = $nonDecreasing -and $strictIncreaseObserved
        }
    }

    return [ordered]@{
        runtimeSamples = @($runtimeSamples)
        fxSamples = @($fxSamples)
        runtimeSampleCount = $runtimeSamples.Count
        fxSampleCount = $fxSamples.Count
        parseErrorCount = $parseErrorCount
        malformedTaggedLineCount = $malformedTaggedLineCount
        runtimeTimestampsStrictlyIncreasing = [bool]$runtimeStrict
        fxTimestampsStrictlyIncreasing = [bool]$fxStrict
        runtimeSpanSeconds = $runtimeSpanSeconds
        fxSpanSeconds = $fxSpanSeconds
        minimumRequiredSpanSeconds = $script:MinimumTelemetrySpanSeconds
        activeEffectsWithinCap = [bool]$activeWithinCap
        activeEffectsCapIsExpected = [bool]$activeCapValid
        expectedActiveEffectsCap = $script:ExpectedActiveEffectsCap
        additiveCacheWithinAssetCount = [bool]$cacheWithinAssets
        additiveCacheHandlesLive = [bool]$cacheHandlesLive
        runtimeMaximumGapMs = $runtimeMaxGapMs
        fxMaximumGapMs = $fxMaxGapMs
        maximumAllowedGapMs = $script:MaximumTelemetryGapMs
        runtimeSamplingDense = [bool]$runtimeSamplingDense
        fxSamplingDense = [bool]$fxSamplingDense
        streamsAligned = [bool]$streamsAligned
        runtimeProcessIds = $runtimeProcessIds
        fxProcessIds = $fxProcessIds
        maxActiveEffects = $maxActiveEffects
        maxRetainedTotal = $maxRetainedTotal
        maxAdditiveCacheEntries = $maxAdditiveCacheEntries
        retainedObservedDecrease = [bool]$retainedObservedDecrease
        additiveCacheObservedDecrease = [bool]$additiveCacheObservedDecrease
        retainedMonotonicGrowthAfterWarmup = [bool]$retainedMonotonicGrowthAfterWarmup
    }
}

function Test-NativeSoakProcessBinding {
    param(
        [Parameter(Mandatory = $true)]$Telemetry,
        [Parameter(Mandatory = $true)][int64]$ExpectedProcessId
    )
    $runtimeIds = @($Telemetry.runtimeProcessIds)
    $fxIds = @($Telemetry.fxProcessIds)
    return [ordered]@{
        runtimeMatches = ($runtimeIds.Count -eq 1 -and [int64]$runtimeIds[0] -eq $ExpectedProcessId)
        fxMatches = ($fxIds.Count -eq 1 -and [int64]$fxIds[0] -eq $ExpectedProcessId)
    }
}

function Get-ClientLogIndicators {
    param(
        [string]$Path,
        [int64]$StartOffset = 0,
        $ExpectedIdentity = $null
    )
    $indicators = [ordered]@{
        crash = $false
        panic = $false
        deviceLost = $false
        B0001 = $false
        unhandledProtocol = $false
    }
    $counts = [ordered]@{
        crash = 0
        panic = 0
        deviceLost = 0
        B0001 = 0
        unhandledProtocol = 0
        successfulReconnects = 0
    }
    $result = [ordered]@{
        enabled = (-not [string]::IsNullOrWhiteSpace($Path))
        available = $false
        complete = $false
        reason = $null
        indicators = $indicators
        counts = $counts
        startOffset = $StartOffset
        endOffset = $null
        identity = $null
        telemetry = Convert-NativeSoakTelemetry -Text ''
    }
    if (-not $result.enabled) {
        $result.reason = 'client-log-not-supplied'
        return $result
    }
    try {
        if ($null -ne $ExpectedIdentity) {
            $currentIdentity = Get-ClientLogIdentity -Path $Path
            $result.identity = $currentIdentity
            if (-not $ExpectedIdentity.available) {
                $result.reason = 'client-log-identity-not-verified-at-start'
                return $result
            }
            if (-not $currentIdentity.available) {
                $result.reason = $currentIdentity.reason
                return $result
            }
            if (-not (Test-ClientLogIdentityMatch -Expected $ExpectedIdentity -Actual $currentIdentity)) {
                $result.reason = 'client-log-identity-changed'
                return $result
            }
        }
        $tail = Read-ClientLogTail -Path $Path -StartOffset $StartOffset
        if (-not $tail.available -or -not $tail.complete) {
            $result.reason = $tail.reason
            return $result
        }
        $text = $tail.text
        $result.endOffset = $tail.endOffset
        $patterns = [ordered]@{
            crash = '(?i)\b(crash(?:ed|ing)?|appcrash|fatal\s+error)\b'
            panic = '(?i)\bpanic\b'
            deviceLost = '(?i)(device[-\s]?lost|DXGI_ERROR_DEVICE_REMOVED|D3D12[^\r\n]*(?:removed|reset)|wgpu[^\r\n]*lost)'
            B0001 = '(?i)\bB0001\b'
            unhandledProtocol = '(?i)(unhandled\s+protocol|protocol\s+unhandled|unknown\s+packet|unsupported\s+packet|malformed\s+(?:packet|protocol))'
        }
        foreach ($key in $patterns.Keys) {
            $count = [Text.RegularExpressions.Regex]::Matches($text, [string]$patterns[$key]).Count
            $result.counts[$key] = $count
            $result.indicators[$key] = ($count -gt 0)
        }
        $result.counts.successfulReconnects = [Text.RegularExpressions.Regex]::Matches(
            $text,
            '(?im)^\s*\[gateway-client\]\s+connected\b[^\r\n]*\bresume=true\b'
        ).Count
        $result.telemetry = Convert-NativeSoakTelemetry -Text $text
        $result.available = $true
        $result.complete = $true
        return $result
    } catch {
        $result.reason = 'client-log-read-failed'
        return $result
    }
}

function Merge-ClientLogIndicators {
    param(
        [Parameter(Mandatory = $true)]$Aggregate,
        [Parameter(Mandatory = $true)]$Scan
    )
    $Aggregate.scans = [int]$Aggregate.scans + 1
    if ($Scan.available -eq $true) { $Aggregate.available = $true }
    if ($Scan.complete -ne $true) { $Aggregate.complete = $false }
    foreach ($key in @('crash', 'panic', 'deviceLost', 'B0001', 'unhandledProtocol')) {
        if ($Scan.indicators[$key]) { $Aggregate.indicators[$key] = $true }
        # Every scan reads the complete observation-window tail. Keep the
        # largest observed total; summing scans would multiply the same lines.
        $Aggregate.counts[$key] = [math]::Max([int]$Aggregate.counts[$key], [int]$Scan.counts[$key])
    }
    $Aggregate.counts.successfulReconnects = [math]::Max(
        [int]$Aggregate.counts.successfulReconnects,
        [int]$Scan.counts.successfulReconnects
    )
    if ($null -ne $Scan.telemetry) { $Aggregate.telemetry = $Scan.telemetry }
    if ($null -ne $Scan.endOffset) { $Aggregate.endOffset = $Scan.endOffset }
    if ($null -ne $Scan.reason) { $Aggregate.lastReason = $Scan.reason }
}

function Get-WindowsEventEvidence {
    param(
        [Parameter(Mandatory = $true)][DateTimeOffset]$StartUtc,
        [Parameter(Mandatory = $true)][DateTimeOffset]$EndUtc
    )
    $evidence = [ordered]@{
        available = $false
        windowStart = $StartUtc.ToString('o', [Globalization.CultureInfo]::InvariantCulture)
        windowEnd = $EndUtc.ToString('o', [Globalization.CultureInfo]::InvariantCulture)
        queries = @()
        events = @()
        count = 0
        relevantCount = 0
        unavailableReason = $null
    }
    $queryDefinitions = @(
        [ordered]@{ logName = 'System'; ids = @(41, 6008, 1001) },
        [ordered]@{ logName = 'Application'; ids = @(1000, 1001) }
    )
    $allEvents = New-Object System.Collections.ArrayList
    $queryResults = New-Object System.Collections.ArrayList
    try {
        foreach ($definition in $queryDefinitions) {
            $queryResult = [ordered]@{
                logName = $definition.logName
                ids = @($definition.ids)
                status = 'unknown'
                count = 0
            }
            $filter = @{
                LogName = $definition.logName
                Id = [int[]]$definition.ids
                StartTime = $StartUtc.LocalDateTime
                EndTime = $EndUtc.LocalDateTime
            }
            try {
                $records = @(Get-WinEvent -FilterHashtable $filter -ErrorAction Stop)
                foreach ($record in $records) {
                    $provider = [string]$record.ProviderName
                    $message = try { [string]$record.Message } catch { '' }
                    $relevant = if ($definition.logName -eq 'System') {
                        ($record.Id -eq 41 -and $provider -match '(?i)Kernel-Power') -or
                        ($record.Id -eq 6008 -and $provider -match '(?i)EventLog') -or
                        ($record.Id -eq 1001 -and ($provider -match '(?i)WER-SystemErrorReporting' -or $message -match '(?i)bugcheck'))
                    } else {
                        $message.IndexOf('mir2-platform-windows', [StringComparison]::OrdinalIgnoreCase) -ge 0
                    }
                    [void]$allEvents.Add([ordered]@{
                        logName = $definition.logName
                        id = [int]$record.Id
                        timeCreated = if ($null -ne $record.TimeCreated) { $record.TimeCreated.ToUniversalTime().ToString('o', [Globalization.CultureInfo]::InvariantCulture) } else { $null }
                        providerName = $provider
                        recordId = $record.RecordId
                        relevant = [bool]$relevant
                        category = if ($relevant -and $definition.logName -eq 'System') { 'host-crash' } elseif ($relevant) { 'native-client-crash' } else { 'unrelated' }
                    })
                }
                $queryResult.status = 'ok'
                $queryResult.count = $records.Count
            } catch {
                $message = [string]$_.Exception.Message
                if ($_.Exception -is [UnauthorizedAccessException] -or $message -match '(?i)(access\s+is\s+denied|permission|拒绝访问)') {
                    $queryResult.status = 'unavailable-permission'
                    $evidence.unavailableReason = 'event-log-permission-denied'
                } elseif ($message -match '(?i)(no\s+events?\s+were\s+found|no\s+events?\s+found|specified\s+selection\s+criteria)') {
                    $queryResult.status = 'ok-no-events'
                    $queryResult.count = 0
                } else {
                    $queryResult.status = 'unavailable-query-failed'
                    $evidence.unavailableReason = 'event-log-query-failed'
                }
            }
            [void]$queryResults.Add($queryResult)
        }
        $evidence.queries = @($queryResults)
        $evidence.events = @($allEvents)
        $evidence.count = $allEvents.Count
        $evidence.relevantCount = @($allEvents | Where-Object { $_.relevant }).Count
        $evidence.available = (@($queryResults | Where-Object { $_.status -like 'unavailable-*' }).Count -eq 0)
        if (-not $evidence.available -and $null -eq $evidence.unavailableReason) {
            $evidence.unavailableReason = 'event-log-unavailable'
        }
    } catch {
        $evidence.unavailableReason = 'event-log-provider-unavailable'
        $evidence.queries = @($queryResults)
    }
    return $evidence
}

function Get-RssEvidence {
    param([Parameter(Mandatory = $true)]$Samples)
    $result = [ordered]@{
        warmupMinutes = 10
        warmupCompleted = $false
        warmupSampleElapsedSeconds = $null
        warmupWorkingSetBytes = $null
        finalSampleElapsedSeconds = $null
        finalWorkingSetBytes = $null
        finalLimitBytes = $null
        finalWithinWarmup125Percent = $false
        postWarmupSampleCount = 0
        trend = 'unavailable'
        deltaBytes = $null
        slopeBytesPerMinute = $null
    }
    $warm = $null
    foreach ($sample in @($Samples)) {
        if ($sample.alive -and $null -ne $sample.workingSet -and [double]$sample.elapsed -ge 600) {
            $warm = $sample
            break
        }
    }
    $liveWithMemory = @($Samples | Where-Object { $_.alive -and $null -ne $_.workingSet })
    if ($liveWithMemory.Count -gt 0) {
        $final = $liveWithMemory[$liveWithMemory.Count - 1]
        $result.finalSampleElapsedSeconds = $final.elapsed
        $result.finalWorkingSetBytes = [int64]$final.workingSet
    }
    if ($null -eq $warm) { return $result }
    $result.warmupCompleted = $true
    $result.warmupSampleElapsedSeconds = $warm.elapsed
    $result.warmupWorkingSetBytes = [int64]$warm.workingSet
    $result.finalLimitBytes = [int64][math]::Ceiling([double]$warm.workingSet * 1.25)
    $post = @($Samples | Where-Object { $_.alive -and $null -ne $_.workingSet -and [double]$_.elapsed -ge [double]$warm.elapsed })
    $result.postWarmupSampleCount = $post.Count
    if ($null -ne $result.finalWorkingSetBytes) {
        $result.finalWithinWarmup125Percent = ([double]$result.finalWorkingSetBytes -le [double]$result.finalLimitBytes)
    }
    if ($post.Count -ge 2) {
        $first = $post[0]
        $last = $post[$post.Count - 1]
        $delta = [double]$last.workingSet - [double]$first.workingSet
        $minutes = ([double]$last.elapsed - [double]$first.elapsed) / 60
        $result.deltaBytes = [int64]$delta
        if ($minutes -gt 0) { $result.slopeBytesPerMinute = [math]::Round($delta / $minutes, 2) }
        if ($delta -gt 0) { $result.trend = 'increasing' }
        elseif ($delta -lt 0) { $result.trend = 'decreasing' }
        else { $result.trend = 'stable' }
    } else {
        $result.trend = 'insufficient-post-warmup-samples'
    }
    return $result
}

function Invoke-SoakObservation {
    param(
        [Parameter(Mandatory = $true)][int]$TargetPid,
        [Parameter(Mandatory = $true)][double]$RequestedDurationMinutes,
        [Parameter(Mandatory = $true)][int]$IntervalSeconds,
        [Parameter(Mandatory = $true)][string]$OutDir,
        [string]$HealthUrl = '',
        [string]$LogPath = '',
        [switch]$IsSelfTest
    )
    $out = Get-FullPathSafe -Path $OutDir
    New-Item -ItemType Directory -Path $out -Force | Out-Null
    $csvPath = Join-Path $out 'memory-samples.csv'
    $jsonPath = Join-Path $out 'soak-30m.json'
    $observerLogPath = Join-Path $out 'soak-client.log'
    [IO.File]::WriteAllText($observerLogPath, '', $script:Utf8NoBom)

    $startUtc = [DateTimeOffset]::UtcNow
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $identity = if ($IsSelfTest) {
        [ordered]@{
            pid = $TargetPid
            expectedProcessName = $script:ExpectedProcessName
            processName = $null
            executablePath = $null
            startTimeUtc = $null
            nameMatch = $false
            pathMatch = $null
            verified = $true
            reason = 'self-test-exception'
        }
    } else {
        Get-ProcessIdentity -TargetPid $TargetPid
    }
    $realNativePid = ((-not $IsSelfTest) -and $identity.verified -and $identity.nameMatch)
    Write-ObserverLog -Path $observerLogPath -Message ('observer-start pid={0} expected={1} realNativePid={2} selfTest={3}' -f $TargetPid, $script:ExpectedProcessName, $realNativePid, [bool]$IsSelfTest)
    if ($identity.reason -ne $null) { Write-ObserverLog -Path $observerLogPath -Message ('identity reason={0}' -f $identity.reason) }

    $healthChecks = New-Object System.Collections.ArrayList
    $healthEnabled = -not [string]::IsNullOrWhiteSpace($HealthUrl)
    if ($healthEnabled) {
        $check = Invoke-HealthCheck -Url $HealthUrl -Phase 'before' -ElapsedSeconds 0
        [void]$healthChecks.Add($check)
        Write-ObserverLog -Path $observerLogPath -Message ('health phase=before origin={0} status={1} ok={2}' -f $check.origin, $check.status, $check.ok)
    }

    $samples = New-Object System.Collections.ArrayList
    $logIdentity = Get-ClientLogIdentity -Path $LogPath
    $logStartOffset = if ($logIdentity.available) { [int64]$logIdentity.length } else { [int64]0 }
    $logAggregate = [ordered]@{
        enabled = $false
        available = $false
        complete = $true
        scans = 0
        lastReason = $null
        startOffset = $logStartOffset
        endOffset = $null
        identity = $logIdentity
        indicators = [ordered]@{ crash = $false; panic = $false; deviceLost = $false; B0001 = $false; unhandledProtocol = $false }
        counts = [ordered]@{ crash = 0; panic = 0; deviceLost = 0; B0001 = 0; unhandledProtocol = 0; successfulReconnects = 0 }
        telemetry = Convert-NativeSoakTelemetry -Text ''
    }
    $logAggregate.enabled = -not [string]::IsNullOrWhiteSpace($LogPath)
    $previousSample = $null
    $previousWallSeconds = 0.0
    $terminationReason = $null
    $initialIdentityValid = $IsSelfTest -or $identity.verified
    $targetDurationSeconds = $RequestedDurationMinutes * 60

    try {
        if (-not $initialIdentityValid) {
            $terminationReason = 'process-identity-not-verified'
            Write-ObserverLog -Path $observerLogPath -Message 'observation-stopped reason=process-identity-not-verified'
        } else {
            do {
                $elapsed = $watch.Elapsed.TotalSeconds
                $sample = Get-ProcessSample -TargetPid $TargetPid -ElapsedSeconds $elapsed -PreviousSample $previousSample -PreviousWallSeconds $previousWallSeconds
                [void]$samples.Add($sample)
                $previousSample = $sample
                $previousWallSeconds = $elapsed

                $scan = Get-ClientLogIndicators -Path $LogPath -StartOffset $logStartOffset -ExpectedIdentity $logIdentity
                Merge-ClientLogIndicators -Aggregate $logAggregate -Scan $scan
                if (@($scan.indicators.Keys | Where-Object { $scan.indicators[$_] }).Count -gt 0) {
                    Write-ObserverLog -Path $observerLogPath -Message ('log-indicator elapsed={0} crash={1} panic={2} deviceLost={3} B0001={4} unhandledProtocol={5}' -f $sample.elapsed, $scan.indicators.crash, $scan.indicators.panic, $scan.indicators.deviceLost, $scan.indicators.B0001, $scan.indicators.unhandledProtocol)
                }
                if ($healthEnabled) {
                    $check = Invoke-HealthCheck -Url $HealthUrl -Phase 'sample' -ElapsedSeconds $elapsed
                    [void]$healthChecks.Add($check)
                    Write-ObserverLog -Path $observerLogPath -Message ('health phase=sample elapsed={0} origin={1} status={2} ok={3}' -f $sample.elapsed, $check.origin, $check.status, $check.ok)
                }
                Write-ObserverLog -Path $observerLogPath -Message ('sample elapsed={0} pid={1} alive={2} workingSet={3} privateBytes={4} cpuSeconds={5} cpuPercentApprox={6} threads={7} handles={8} responding={9}' -f $sample.elapsed, $sample.pid, $sample.alive, $sample.workingSet, $sample.privateBytes, $sample.cpuSeconds, $sample.cpuPercentApprox, $sample.threadCount, $sample.handleCount, $sample.responding)

                if (-not $sample.alive) {
                    $terminationReason = 'process-exited-early'
                    break
                }
                if (-not $IsSelfTest) {
                    $currentIdentity = Get-ProcessIdentity -TargetPid $TargetPid
                    if (-not $currentIdentity.verified -or $currentIdentity.startTimeUtc -ne $identity.startTimeUtc) {
                        $terminationReason = 'process-identity-changed'
                        Write-ObserverLog -Path $observerLogPath -Message 'observation-stopped reason=process-identity-changed'
                        break
                    }
                }
                $remaining = $targetDurationSeconds - $watch.Elapsed.TotalSeconds
                if ($remaining -gt 0) {
                    $sleepSeconds = [math]::Min([double]$IntervalSeconds, $remaining)
                    Start-Sleep -Milliseconds ([int][math]::Max(1, [math]::Round($sleepSeconds * 1000)))
                }
            } while ($watch.Elapsed.TotalSeconds -lt $targetDurationSeconds)
        }
    } catch {
        $terminationReason = 'observer-error'
        Write-ObserverLog -Path $observerLogPath -Message 'observation-stopped reason=observer-error'
    } finally {
        $finalLogScan = Get-ClientLogIndicators -Path $LogPath -StartOffset $logStartOffset -ExpectedIdentity $logIdentity
        Merge-ClientLogIndicators -Aggregate $logAggregate -Scan $finalLogScan
        if ($healthEnabled) {
            $afterElapsed = $watch.Elapsed.TotalSeconds
            $check = Invoke-HealthCheck -Url $HealthUrl -Phase 'after' -ElapsedSeconds $afterElapsed
            [void]$healthChecks.Add($check)
            Write-ObserverLog -Path $observerLogPath -Message ('health phase=after elapsed={0} origin={1} status={2} ok={3}' -f ([math]::Round($afterElapsed, 3)), $check.origin, $check.status, $check.ok)
        }
        $watch.Stop()
    }

    $endUtc = [DateTimeOffset]::UtcNow
    $actualSeconds = $watch.Elapsed.TotalSeconds
    $actualMinutes = $actualSeconds / 60
    $requestedComplete = ($actualSeconds -ge ($targetDurationSeconds - 0.25)) -and ($terminationReason -notin @('process-exited-early', 'process-identity-changed', 'process-identity-not-verified'))
    $thirtyMinuteComplete = ($actualSeconds -ge (($script:MinimumRequiredMinutes * 60) - 0.25)) -and $requestedComplete
    $rss = Get-RssEvidence -Samples @($samples)
    $events = Get-WindowsEventEvidence -StartUtc $startUtc -EndUtc $endUtc
    Write-MemoryCsv -Path $csvPath -Samples @($samples)

    $healthFailures = @($healthChecks | Where-Object { -not $_.ok }).Count
    $indicatorPresent = @($logAggregate.indicators.Keys | Where-Object { $logAggregate.indicators[$_] }).Count -gt 0
    $telemetry = $logAggregate.telemetry
    $telemetryProcessBinding = Test-NativeSoakProcessBinding -Telemetry $telemetry -ExpectedProcessId $TargetPid
    $reasons = New-Object System.Collections.ArrayList
    if (-not $realNativePid) { [void]$reasons.Add('not-real-native-pid') }
    if (-not $requestedComplete) { [void]$reasons.Add('FAIL-short-duration') }
    if (-not $thirtyMinuteComplete) { [void]$reasons.Add('FAIL-short-duration-30m-requirement') }
    if ($null -ne $terminationReason) { [void]$reasons.Add($terminationReason) }
    if (-not $events.available) { [void]$reasons.Add('event-observation-unavailable') }
    elseif ([int]$events.relevantCount -gt 0) { [void]$reasons.Add('windows-crash-event-observed') }
    if (-not $healthEnabled) { [void]$reasons.Add('gateway-health-not-supplied') }
    elseif ($healthFailures -gt 0) { [void]$reasons.Add('gateway-health-failed') }
    if (-not $logAggregate.enabled) { [void]$reasons.Add('client-log-not-supplied') }
    elseif (-not $logAggregate.available -or -not $logAggregate.complete) { [void]$reasons.Add('client-log-unavailable') }
    if (-not $logIdentity.available -and $logAggregate.enabled) { [void]$reasons.Add('client-log-identity-not-verified') }
    if ($logAggregate.lastReason -eq 'client-log-identity-changed') { [void]$reasons.Add('client-log-identity-changed') }
    if ($indicatorPresent) { [void]$reasons.Add('client-log-failure-indicator') }
    if ([int]$logAggregate.counts.successfulReconnects -lt 1) { [void]$reasons.Add('successful-reconnect-not-observed') }
    if ([int]$telemetry.parseErrorCount -gt 0) { [void]$reasons.Add('native-soak-metrics-parse-error') }
    if ([int]$telemetry.malformedTaggedLineCount -gt 0) { [void]$reasons.Add('native-soak-metrics-malformed-line') }
    if ([int]$telemetry.runtimeSampleCount -eq 0) { [void]$reasons.Add('runtime-soak-metrics-missing') }
    elseif ([double]$telemetry.runtimeSpanSeconds -lt $script:MinimumTelemetrySpanSeconds) { [void]$reasons.Add('runtime-soak-metrics-span-too-short') }
    if ([int]$telemetry.fxSampleCount -eq 0) { [void]$reasons.Add('fx-soak-metrics-missing') }
    elseif ([double]$telemetry.fxSpanSeconds -lt $script:MinimumTelemetrySpanSeconds) { [void]$reasons.Add('fx-soak-metrics-span-too-short') }
    if (-not $telemetry.runtimeTimestampsStrictlyIncreasing) { [void]$reasons.Add('runtime-soak-metrics-timestamps-invalid') }
    if (-not $telemetry.fxTimestampsStrictlyIncreasing) { [void]$reasons.Add('fx-soak-metrics-timestamps-invalid') }
    if (-not $telemetry.runtimeSamplingDense) { [void]$reasons.Add('runtime-soak-metrics-sampling-gap') }
    if (-not $telemetry.fxSamplingDense) { [void]$reasons.Add('fx-soak-metrics-sampling-gap') }
    if (-not $telemetry.streamsAligned) { [void]$reasons.Add('native-soak-metrics-streams-not-aligned') }
    if (-not $telemetryProcessBinding.runtimeMatches) { [void]$reasons.Add('runtime-soak-metrics-pid-mismatch') }
    if (-not $telemetryProcessBinding.fxMatches) { [void]$reasons.Add('fx-soak-metrics-pid-mismatch') }
    if (-not $telemetry.activeEffectsCapIsExpected) { [void]$reasons.Add('active-effects-cap-contract-mismatch') }
    if (-not $telemetry.activeEffectsWithinCap) { [void]$reasons.Add('active-effects-over-cap') }
    if (-not $telemetry.additiveCacheHandlesLive) { [void]$reasons.Add('additive-cache-stale-handle') }
    if (-not $telemetry.additiveCacheWithinAssetCount) { [void]$reasons.Add('additive-cache-exceeds-assets') }
    if ($telemetry.retainedMonotonicGrowthAfterWarmup) { [void]$reasons.Add('retained-entities-monotonic-growth-after-warmup') }
    if (-not $rss.warmupCompleted) { [void]$reasons.Add('warmup-10m-incomplete') }
    elseif ($null -eq $rss.finalWorkingSetBytes) { [void]$reasons.Add('final-rss-unavailable') }
    elseif (-not $rss.finalWithinWarmup125Percent) { [void]$reasons.Add('rss-over-warmup-125-percent') }
    Write-ObserverLog -Path $observerLogPath -Message ('observation-finished actualMinutes={0}' -f ([math]::Round($actualMinutes, 3)))
    if (@($script:OutputWriteErrors).Count -gt 0) { [void]$reasons.Add('evidence-write-failed') }

    $pass = (@($reasons).Count -eq 0)
    $status = if ($pass) { 'PASS' } elseif (@($reasons | Where-Object { $_ -like 'FAIL-short-duration*' }).Count -gt 0) { 'FAIL-short-duration' } elseif ($null -ne $terminationReason) { 'FAIL-' + $terminationReason } else { 'FAIL' }
    $report = [ordered]@{
        schema = $script:Schema
        generatedAt = Get-UtcTimestamp
        selfTest = [bool]$IsSelfTest
        target = [ordered]@{
            pid = $TargetPid
            expectedProcessName = $script:ExpectedProcessName
            realNativePid = [bool]$realNativePid
            identity = $identity
        }
        request = [ordered]@{
            durationMinutes = $RequestedDurationMinutes
            sampleIntervalSeconds = $IntervalSeconds
            recommendedSampleIntervalSeconds = '10..30'
            minimumRequiredDurationMinutes = $script:MinimumRequiredMinutes
            gatewayHealthEnabled = [bool]$healthEnabled
            clientLogEnabled = (-not [string]::IsNullOrWhiteSpace($LogPath))
        }
        run = [ordered]@{
            startedAt = $startUtc.ToString('o', [Globalization.CultureInfo]::InvariantCulture)
            endedAt = $endUtc.ToString('o', [Globalization.CultureInfo]::InvariantCulture)
            actualDurationSeconds = [math]::Round($actualSeconds, 3)
            actualDurationMinutes = [math]::Round($actualMinutes, 3)
            meetsRequestedDuration = [bool]$requestedComplete
            meetsThirtyMinuteRequirement = [bool]$thirtyMinuteComplete
            terminationReason = $terminationReason
        }
        samples = @($samples)
        health = [ordered]@{
            enabled = [bool]$healthEnabled
            origin = if (@($healthChecks).Count -gt 0) { $healthChecks[0].origin } else { $null }
            checks = @($healthChecks)
            failureCount = $healthFailures
        }
        clientLog = [ordered]@{
            enabled = [bool]$logAggregate.enabled
            available = [bool]$logAggregate.available
            complete = [bool]$logAggregate.complete
            indicators = $logAggregate.indicators
            counts = $logAggregate.counts
            scans = $logAggregate.scans
            lastReason = $logAggregate.lastReason
            observationStartOffset = $logAggregate.startOffset
            observationEndOffset = $logAggregate.endOffset
            observationIdentity = $logAggregate.identity
            telemetry = $telemetry
        }
        windowsEvents = $events
        rss = $rss
        outputs = [ordered]@{
            memorySamplesCsv = $csvPath
            soakClientLog = $observerLogPath
            reportJson = $jsonPath
        }
        gate = [ordered]@{
            pass = [bool]$pass
            status = $status
            reasons = @($reasons | Select-Object -Unique)
            note = 'A missing requirement or unavailable evidence is never converted to PASS.'
        }
        observer = [ordered]@{
            outputWriteErrors = @($script:OutputWriteErrors | Select-Object -Unique)
            powershellVersion = $PSVersionTable.PSVersion.ToString()
            readOnly = $true
        }
    }
    Write-AtomicJson -Path $jsonPath -Value $report
    return $report
}

function Invoke-SelfTest {
    $selfRoot = Join-Path ([IO.Path]::GetTempPath()) ('mir2-native-candidate-soak-selftest-' + [guid]::NewGuid().ToString('N'))
    try {
        New-Item -ItemType Directory -Path $selfRoot -Force | Out-Null
        $syntheticTelemetry = @'
[native-soak] {"processId":4242,"timestampMs":0,"snapshotEffects":1,"retainedEffectPrimary":1,"retainedEffectMasks":0,"retainedEffectShadows":1,"retainedEffectImages":2,"retainedEntityLayers":12,"legacySceneEntities":0,"entityAtlases":2,"mapRenderTiles":470,"mapSpawnedEntities":0,"mineNodes":0,"lightingLayers":2,"lightingImages":3,"additiveCacheEntries":2,"additiveCacheLiveEntries":2,"additiveAssetCount":2}
[native-soak-fx] {"processId":4242,"timestampMs":0,"activeEffects":1,"activeEffectsCap":96}
[native-soak] {"processId":4242,"timestampMs":1740000,"snapshotEffects":0,"retainedEffectPrimary":0,"retainedEffectMasks":0,"retainedEffectShadows":0,"retainedEffectImages":0,"retainedEntityLayers":12,"legacySceneEntities":0,"entityAtlases":2,"mapRenderTiles":470,"mapSpawnedEntities":0,"mineNodes":0,"lightingLayers":2,"lightingImages":3,"additiveCacheEntries":0,"additiveCacheLiveEntries":0,"additiveAssetCount":0}
[native-soak-fx] {"processId":4242,"timestampMs":1740000,"activeEffects":0,"activeEffectsCap":96}
[gateway-client] connected generation=2 resume=true
'@
        $telemetry = Convert-NativeSoakTelemetry -Text $syntheticTelemetry
        if ($telemetry.runtimeSampleCount -ne 2 -or $telemetry.fxSampleCount -ne 2) { throw 'SelfTest telemetry sample count mismatch' }
        if ($telemetry.runtimeSpanSeconds -ne 1740 -or $telemetry.fxSpanSeconds -ne 1740) { throw 'SelfTest telemetry span mismatch' }
        if (-not $telemetry.activeEffectsWithinCap -or -not $telemetry.additiveCacheWithinAssetCount -or -not $telemetry.additiveCacheHandlesLive) { throw 'SelfTest telemetry bounds mismatch' }
        if (-not $telemetry.activeEffectsCapIsExpected -or -not $telemetry.streamsAligned) { throw 'SelfTest telemetry contract mismatch' }
        if (@($telemetry.runtimeProcessIds).Count -ne 1 -or $telemetry.runtimeProcessIds[0] -ne 4242) { throw 'SelfTest runtime process identity mismatch' }
        if (@($telemetry.fxProcessIds).Count -ne 1 -or $telemetry.fxProcessIds[0] -ne 4242) { throw 'SelfTest FX process identity mismatch' }
        $matchingBinding = Test-NativeSoakProcessBinding -Telemetry $telemetry -ExpectedProcessId 4242
        $wrongBinding = Test-NativeSoakProcessBinding -Telemetry $telemetry -ExpectedProcessId 4243
        if (-not $matchingBinding.runtimeMatches -or -not $matchingBinding.fxMatches) { throw 'SelfTest matching process binding was rejected' }
        if ($wrongBinding.runtimeMatches -or $wrongBinding.fxMatches) { throw 'SelfTest mismatched process binding was accepted' }
        if ($telemetry.runtimeSamplingDense -or $telemetry.fxSamplingDense) { throw 'SelfTest sparse telemetry was accepted' }
        if ($telemetry.parseErrorCount -ne 0) { throw 'SelfTest telemetry parse mismatch' }
        $malformed = Convert-NativeSoakTelemetry -Text '[native-soak] invalid-json'
        if ($malformed.malformedTaggedLineCount -ne 1) { throw 'SelfTest malformed telemetry was ignored' }
        $tailPrefix = "historical-line`n"
        $tailFixture = Join-Path $selfRoot 'tail-fixture.log'
        [IO.File]::WriteAllText($tailFixture, $tailPrefix + $syntheticTelemetry, $script:Utf8NoBom)
        $tailOffset = [Text.Encoding]::UTF8.GetByteCount($tailPrefix)
        $tail = Read-ClientLogTail -Path $tailFixture -StartOffset $tailOffset
        if (-not $tail.available -or -not $tail.complete -or $tail.text -ne $syntheticTelemetry) { throw 'SelfTest client-log tail mismatch' }
        $tailIdentity = Get-ClientLogIdentity -Path $tailFixture
        if (-not $tailIdentity.available -or [string]::IsNullOrWhiteSpace($tailIdentity.fileId)) { throw 'SelfTest client-log identity unavailable' }
        $tailScan = Get-ClientLogIndicators -Path $tailFixture -StartOffset $tailOffset -ExpectedIdentity $tailIdentity
        if (-not $tailScan.available -or -not $tailScan.complete) { throw 'SelfTest verified client-log scan failed' }
        if ($tailScan.counts.successfulReconnects -ne 1) { throw 'SelfTest successful reconnect was not counted' }
        $replacement = Join-Path $selfRoot 'tail-replacement.log'
        [IO.File]::WriteAllText($replacement, $tailPrefix + $syntheticTelemetry, $script:Utf8NoBom)
        Move-Item -LiteralPath $replacement -Destination $tailFixture -Force
        $replacedScan = Get-ClientLogIndicators -Path $tailFixture -StartOffset $tailOffset -ExpectedIdentity $tailIdentity
        if ($replacedScan.reason -ne 'client-log-identity-changed') { throw 'SelfTest client-log replacement was accepted' }

        $report = Invoke-SoakObservation -TargetPid $PID -RequestedDurationMinutes 0.03 -IntervalSeconds 1 -OutDir $selfRoot -IsSelfTest
        $jsonPath = Join-Path $selfRoot 'soak-30m.json'
        $csvPath = Join-Path $selfRoot 'memory-samples.csv'
        $logPath = Join-Path $selfRoot 'soak-client.log'
        foreach ($path in @($jsonPath, $csvPath, $logPath)) {
            if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw 'SelfTest output file missing' }
        }
        $parsed = Get-Content -LiteralPath $jsonPath -Raw | ConvertFrom-Json
        if ($parsed.schema -ne $script:Schema) { throw 'SelfTest schema mismatch' }
        if ($parsed.selfTest -ne $true) { throw 'SelfTest marker missing' }
        if ($parsed.target.realNativePid -ne $false) { throw 'SelfTest native PID exception mismatch' }
        if ($parsed.gate.pass -ne $false) { throw 'SelfTest incorrectly passed a short run' }
        if ($parsed.gate.status -ne 'FAIL-short-duration') { throw 'SelfTest short-duration gate mismatch' }
        $header = Get-Content -LiteralPath $csvPath -TotalCount 1
        $headerFields = @($header.Replace('"', '').Split(','))
        foreach ($field in @('timestamp', 'elapsed', 'pid', 'alive', 'workingSet', 'privateBytes', 'cpuSeconds', 'cpuPercentApprox', 'threadCount', 'handleCount', 'responding')) {
            if ($headerFields -notcontains $field) { throw 'SelfTest CSV schema mismatch' }
        }
        Write-Host ('monitor-native-candidate-soak SelfTest passed: FAIL-short-duration recorded at {0}' -f $selfRoot)
    } finally {
        # Keep the evidence directory for inspection; no system setting or
        # process state is changed by SelfTest.
    }
}

if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}

if ($ProcessId -le 0) { throw 'ProcessId is required unless -SelfTest is used.' }
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path (Get-Location) 'native-candidate-soak'
}
$finalReport = Invoke-SoakObservation -TargetPid $ProcessId -RequestedDurationMinutes $DurationMinutes -IntervalSeconds $SampleIntervalSeconds -OutDir $OutputDirectory -HealthUrl $GatewayHealthUrl -LogPath $ClientLogPath
if ($finalReport.gate.pass) { exit 0 }
exit 2
