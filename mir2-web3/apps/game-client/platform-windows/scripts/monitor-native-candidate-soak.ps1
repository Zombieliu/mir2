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
    125% of the warmup baseline. Health/log parameters remain syntactically
    optional for exploratory runs, but omitting either keeps the gate failed.

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

function Get-ClientLogIndicators {
    param([string]$Path)
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
    }
    $result = [ordered]@{
        enabled = (-not [string]::IsNullOrWhiteSpace($Path))
        available = $false
        complete = $false
        reason = $null
        indicators = $indicators
        counts = $counts
    }
    if (-not $result.enabled) {
        $result.reason = 'client-log-not-supplied'
        return $result
    }
    try {
        if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
            $result.reason = 'client-log-not-found'
            return $result
        }
        $text = [IO.File]::ReadAllText((Get-FullPathSafe -Path $Path))
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
        # Every scan reads the complete log. Keep the largest observed total;
        # summing scans would multiply the same historical lines.
        $Aggregate.counts[$key] = [math]::Max([int]$Aggregate.counts[$key], [int]$Scan.counts[$key])
    }
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
    $logAggregate = [ordered]@{
        enabled = $false
        available = $false
        complete = $true
        scans = 0
        lastReason = $null
        indicators = [ordered]@{ crash = $false; panic = $false; deviceLost = $false; B0001 = $false; unhandledProtocol = $false }
        counts = [ordered]@{ crash = 0; panic = 0; deviceLost = 0; B0001 = 0; unhandledProtocol = 0 }
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

                $scan = Get-ClientLogIndicators -Path $LogPath
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
                    if (-not $currentIdentity.verified) {
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
    if ($indicatorPresent) { [void]$reasons.Add('client-log-failure-indicator') }
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
