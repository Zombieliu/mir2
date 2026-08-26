#requires -Version 5.1

[CmdletBinding()]
param(
    [string]$EvidenceDirectory,
    [string]$RustToolchain = "1.95.0",
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $scriptDirectory "..\..\..\..")
)

function New-Control {
    param(
        [Parameter(Mandatory = $true)][string]$Id,
        [Parameter(Mandatory = $true)][string]$Description,
        [Parameter(Mandatory = $true)][string]$Executable,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    [pscustomobject][ordered]@{
        id = $Id
        description = $Description
        executable = $Executable
        arguments = @($Arguments)
    }
}

function Invoke-NativeControl {
    param(
        [Parameter(Mandatory = $true)][string]$Executable,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [switch]$Quiet
    )

    # Windows PowerShell 5.1 promotes native stderr to NativeCommandError when
    # ErrorActionPreference is Stop. Cargo writes ordinary progress such as an
    # index update to stderr, so capture it without treating the stream itself
    # as failure. The native process exit code remains the fail-closed signal.
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        $capturedLines = @(& $Executable @Arguments 2>&1 | ForEach-Object {
            $line = $_.ToString()
            if (-not $Quiet) {
                Write-Host $line
            }
            $line
        })
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }

    [pscustomobject][ordered]@{
        lines = $capturedLines
        exitCode = $exitCode
    }
}

$cargo = "cargo"
$cargoToolchainArgument = "+$RustToolchain"
$controls = @(
    New-Control `
        -Id "native-host-contract" `
        -Description "Windows native host and client contract tests" `
        -Executable $cargo `
        -Arguments @(
            $cargoToolchainArgument,
            "test",
            "--locked",
            "--manifest-path",
            "apps/game-client/platform-windows/Cargo.toml",
            "--",
            "--test-threads=1"
        )
    New-Control `
        -Id "five-class-bichon-functional-slice" `
        -Description "Five-class, Bichon quest, combat, reward, and shared-player vertical slice" `
        -Executable $cargo `
        -Arguments @(
            $cargoToolchainArgument,
            "test",
            "--locked",
            "-p",
            "mir2-simulation",
            "--test",
            "vertical_slice",
            "--",
            "--test-threads=1"
        )
    New-Control `
        -Id "ordinary-unprivileged-candidate-loop" `
        -Description "Ordinary player NPC, combat, pickup, reward, logout, and reload loop" `
        -Executable $cargo `
        -Arguments @(
            $cargoToolchainArgument,
            "test",
            "--locked",
            "-p",
            "mir2-simulation",
            "--test",
            "ordinary_candidate_loop",
            "--",
            "--test-threads=1"
        )
    New-Control `
        -Id "security-lifecycle" `
        -Description "Authentication, production-command rejection, character lifecycle, and transform recovery" `
        -Executable $cargo `
        -Arguments @(
            $cargoToolchainArgument,
            "test",
            "--locked",
            "-p",
            "mir2-simulation",
            "--test",
            "security_lifecycle",
            "--",
            "--test-threads=1"
        )
    New-Control `
        -Id "shared-zone-authority" `
        -Description "Shared Zone movement, AOI, monster, drop-claim, settlement, and recovery authority" `
        -Executable $cargo `
        -Arguments @(
            $cargoToolchainArgument,
            "test",
            "--locked",
            "-p",
            "mir2-simulation",
            "--test",
            "shared_zone",
            "--",
            "--test-threads=1"
        )
    New-Control `
        -Id "gateway-logout-reload" `
        -Description "Gateway fresh-account Bichon logout and authoritative new-session reload" `
        -Executable $cargo `
        -Arguments @(
            $cargoToolchainArgument,
            "test",
            "--locked",
            "-p",
            "mir2-gateway",
            "--test",
            "vertical_slice_gateway_persistence",
            "--",
            "--test-threads=1"
        )
    New-Control `
        -Id "web-shared-code-regression" `
        -Description "Player Web type safety for the shared Gateway and read-model surface" `
        -Executable "npm.cmd" `
        -Arguments @("--prefix", "apps/web", "run", "typecheck")
)

$expectedControlIds = @(
    "native-host-contract",
    "five-class-bichon-functional-slice",
    "ordinary-unprivileged-candidate-loop",
    "security-lifecycle",
    "shared-zone-authority",
    "gateway-logout-reload",
    "web-shared-code-regression"
)

function Assert-Contract {
    if ($RustToolchain -cnotmatch "^[0-9]+\.[0-9]+\.[0-9]+$") {
        throw "RustToolchain must be an exact stable version such as 1.95.0"
    }

    if ($controls.Count -ne $expectedControlIds.Count) {
        throw "vertical-slice control count changed without updating the fixed contract"
    }

    $actualIds = @($controls | ForEach-Object { $_.id })
    if ([string]::Join("`n", $actualIds) -cne [string]::Join("`n", $expectedControlIds)) {
        throw "vertical-slice control IDs or order differ from the fixed contract"
    }

    if (($actualIds | Select-Object -Unique).Count -ne $actualIds.Count) {
        throw "vertical-slice control IDs must be unique"
    }

    foreach ($control in $controls) {
        if ([string]::IsNullOrWhiteSpace($control.description)) {
            throw "control '$($control.id)' has no description"
        }
        if ($control.arguments.Count -eq 0) {
            throw "control '$($control.id)' has no arguments"
        }
        if ($control.executable -cnotin @("cargo", "npm.cmd")) {
            throw "control '$($control.id)' uses an unapproved executable"
        }
    }

    $vertical = $controls | Where-Object { $_.id -ceq "five-class-bichon-functional-slice" }
    if ($vertical.arguments -cnotcontains "vertical_slice") {
        throw "the functional slice control must execute the complete vertical_slice test target"
    }

    $gateway = $controls | Where-Object { $_.id -ceq "gateway-logout-reload" }
    if ($gateway.arguments -cnotcontains "vertical_slice_gateway_persistence") {
        throw "the Gateway control must execute its exact persistence integration target"
    }
}

Assert-Contract

if ($SelfTest) {
    $stderrProbe = Invoke-NativeControl `
        -Executable "cmd.exe" `
        -Arguments @("/d", "/s", "/c", "echo native-stderr-probe 1>&2") `
        -Quiet
    $stderrProbeMatched = @($stderrProbe.lines | Where-Object {
        $_.Trim() -ceq "native-stderr-probe"
    }).Count -eq 1
    if ($stderrProbe.exitCode -ne 0 -or -not $stderrProbeMatched) {
        throw "native stderr capture compatibility probe failed"
    }

    [pscustomobject][ordered]@{
        schema = "mir2.windows.vertical-slice-gate-self-test.v1"
        status = "PASS"
        controlCount = $controls.Count
        controlIds = $expectedControlIds
        nativeStderrCapture = "PASS"
        globalParityPercent = $null
    } | ConvertTo-Json -Depth 4
    exit 0
}

if ([string]::IsNullOrWhiteSpace($EvidenceDirectory)) {
    throw "EvidenceDirectory is required outside -SelfTest mode"
}

$resolvedEvidenceParent = Split-Path -Parent ([System.IO.Path]::GetFullPath(
    (Join-Path $projectRoot $EvidenceDirectory)
))
$resolvedEvidenceDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path $projectRoot $EvidenceDirectory)
)

if (Test-Path -LiteralPath $resolvedEvidenceDirectory) {
    throw "EvidenceDirectory already exists; vertical-slice evidence is no-overwrite"
}

$projectRootWithSeparator = $projectRoot.TrimEnd(
    [System.IO.Path]::DirectorySeparatorChar,
    [System.IO.Path]::AltDirectorySeparatorChar
) + [System.IO.Path]::DirectorySeparatorChar
if (-not $resolvedEvidenceDirectory.StartsWith(
    $projectRootWithSeparator,
    [System.StringComparison]::OrdinalIgnoreCase
)) {
    throw "EvidenceDirectory must stay inside the mir2-web3 project root"
}

foreach ($executable in @("git", "cargo", "npm.cmd")) {
    if ($null -eq (Get-Command $executable -ErrorAction SilentlyContinue)) {
        throw "required executable is unavailable: $executable"
    }
}

Push-Location $projectRoot
try {
    $gitRevision = (& git rev-parse HEAD).Trim().ToLowerInvariant()
    if ($LASTEXITCODE -ne 0 -or $gitRevision -cnotmatch "^[0-9a-f]{40}$") {
        throw "unable to resolve the implementation Git revision"
    }

    $gitStatus = @(& git status --porcelain=v1 --untracked-files=all)
    if ($LASTEXITCODE -ne 0) {
        throw "unable to inspect the implementation worktree"
    }
    if ($gitStatus.Count -ne 0) {
        throw "formal vertical-slice evidence requires a clean implementation worktree"
    }

    if (-not (Test-Path -LiteralPath $resolvedEvidenceParent -PathType Container)) {
        [System.IO.Directory]::CreateDirectory($resolvedEvidenceParent) | Out-Null
    }
    [System.IO.Directory]::CreateDirectory($resolvedEvidenceDirectory) | Out-Null

    $startedAt = [DateTimeOffset]::UtcNow
    $results = [System.Collections.Generic.List[object]]::new()
    $failed = $false
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)

    foreach ($control in $controls) {
        $logPath = Join-Path $resolvedEvidenceDirectory ("{0}.log" -f $control.id)
        $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
        Write-Host "`n==> [$($control.id)] $($control.description)"
        Write-Host ("    {0} {1}" -f $control.executable, [string]::Join(" ", $control.arguments))

        $nativeResult = Invoke-NativeControl `
            -Executable $control.executable `
            -Arguments @($control.arguments)
        $lines = @($nativeResult.lines)
        $exitCode = $nativeResult.exitCode
        $stopwatch.Stop()

        [System.IO.File]::WriteAllText(
            $logPath,
            ([string]::Join([Environment]::NewLine, $lines) + [Environment]::NewLine),
            $utf8NoBom
        )
        $logSha256 = (Get-FileHash -LiteralPath $logPath -Algorithm SHA256).Hash.ToUpperInvariant()
        $status = if ($exitCode -eq 0) { "PASS" } else { "FAIL" }
        $results.Add([pscustomobject][ordered]@{
            id = $control.id
            description = $control.description
            status = $status
            exitCode = $exitCode
            durationMilliseconds = [int64]$stopwatch.ElapsedMilliseconds
            executable = $control.executable
            arguments = @($control.arguments)
            logFile = [System.IO.Path]::GetFileName($logPath)
            logSha256 = $logSha256
        })

        if ($exitCode -ne 0) {
            $failed = $true
            break
        }
    }

    $finishedAt = [DateTimeOffset]::UtcNow
    $passedControlCount = @($results | Where-Object { $_.status -ceq "PASS" }).Count
    $coveragePercent = if (-not $failed -and $results.Count -eq $controls.Count) {
        100.0
    } else {
        [Math]::Round(($passedControlCount * 100.0) / $controls.Count, 2)
    }

    $summary = [pscustomobject][ordered]@{
        schema = "mir2.windows.vertical-slice-functional-gate.v1"
        gateId = "windows-verifiable-vertical-slice-functional"
        status = if ($failed) { "FAIL" } else { "PASS" }
        implementationRevision = $gitRevision
        worktreeClean = $true
        startedAtUtc = $startedAt.ToString("O")
        finishedAtUtc = $finishedAt.ToString("O")
        rustToolchain = $RustToolchain
        declaredControlCount = $controls.Count
        executedControlCount = $results.Count
        passedControlCount = $passedControlCount
        automatedFunctionalCoveragePercent = $coveragePercent
        globalParityPercent = $null
        accepted = $false
        visualAccepted = $false
        excludedHumanOrReleaseGates = @(
            "same-exe-human-ui-account-to-quest",
            "authenticated-live-websocket-same-exe",
            "real-windows-125-and-150-percent-dpi",
            "real-30-minute-native-soak",
            "original-client-human-visual-and-gameplay-feel",
            "formal-publisher-certificate-and-authenticode"
        )
        controls = @($results)
    }

    $summaryPath = Join-Path $resolvedEvidenceDirectory "SUMMARY.json"
    [System.IO.File]::WriteAllText(
        $summaryPath,
        ($summary | ConvertTo-Json -Depth 8) + [Environment]::NewLine,
        $utf8NoBom
    )
    Write-Host "`nVertical-slice evidence: $summaryPath"

    if ($failed -or $results.Count -ne $controls.Count) {
        throw "Windows verifiable vertical-slice functional gate failed"
    }
}
finally {
    Pop-Location
}
