[CmdletBinding()]
param(
    [ValidateRange(1024, 65535)]
    [int]$WebPort = 3002,

    [ValidateRange(1024, 65535)]
    [int]$GatewayWebPort = 7110,

    [ValidateRange(1024, 65535)]
    [int]$GatewayTcpPort = 7000,

    [string]$AssetBaseUrl = "",

    [switch]$OpenBrowser,
    [switch]$SkipGatewayBuild,
    [switch]$ReuseGateway
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ProjectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$WebRoot = Join-Path $ProjectRoot "apps\web"
$LocalFullPackIndex = Join-Path $WebRoot "public\generated\crystal-packs\full\index.json"
$LogRoot = Join-Path $ProjectRoot ".mir2-data\developer-logs"
$GatewayExe = Join-Path $ProjectRoot "target\debug\mir2-gateway.exe"
$GatewayHealthUrl = "http://127.0.0.1:$GatewayWebPort/health"
$WebUrl = "http://127.0.0.1:$WebPort/"
$GatewayProcess = $null
$StartedGateway = $false

function Test-HttpOk {
    param([string]$Url)
    try {
        $Response = Invoke-WebRequest -UseBasicParsing -Uri $Url -TimeoutSec 2
        return $Response.StatusCode -ge 200 -and $Response.StatusCode -lt 300
    }
    catch {
        return $false
    }
}

function Test-ListeningPort {
    param([int]$Port)
    return $null -ne (Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue | Select-Object -First 1)
}

if (-not (Get-Command npm.cmd -ErrorAction SilentlyContinue)) {
    throw "npm.cmd is missing. Run scripts/bootstrap-developer.ps1 first."
}

if (Test-ListeningPort $WebPort) {
    throw "Web port $WebPort is already in use. Stop that process or pass -WebPort with another port."
}

$GatewayHealthy = Test-HttpOk $GatewayHealthUrl
if ($GatewayHealthy -and -not $ReuseGateway) {
    throw "A healthy service is already using Gateway port $GatewayWebPort. Stop it, choose other ports, or explicitly pass -ReuseGateway."
}

if (-not $GatewayHealthy) {
    if (Test-ListeningPort $GatewayWebPort) {
        throw "Gateway port $GatewayWebPort is occupied by a service that does not answer $GatewayHealthUrl."
    }
    if (Test-ListeningPort $GatewayTcpPort) {
        throw "Gateway TCP port $GatewayTcpPort is already in use."
    }

    if (-not $SkipGatewayBuild -or -not (Test-Path -LiteralPath $GatewayExe -PathType Leaf)) {
        Push-Location $ProjectRoot
        try {
            Write-Host "[start] building Gateway (incremental)"
            cargo +1.89.0 build --locked -p mir2-gateway --bin mir2-gateway
            if ($LASTEXITCODE -ne 0) {
                throw "Gateway build failed with exit code $LASTEXITCODE"
            }
        }
        finally {
            Pop-Location
        }
    }

    New-Item -ItemType Directory -Path $LogRoot -Force | Out-Null
    $GatewayOut = Join-Path $LogRoot "gateway.out.log"
    $GatewayErr = Join-Path $LogRoot "gateway.err.log"
    $PreviousWebAddress = $env:MIR2_GATEWAY_WEB_ADDR
    $PreviousTcpAddress = $env:MIR2_GATEWAY_TCP_ADDR
    try {
        $env:MIR2_GATEWAY_WEB_ADDR = "127.0.0.1:$GatewayWebPort"
        $env:MIR2_GATEWAY_TCP_ADDR = "127.0.0.1:$GatewayTcpPort"
        $GatewayProcess = Start-Process `
            -FilePath $GatewayExe `
            -WorkingDirectory $ProjectRoot `
            -WindowStyle Hidden `
            -RedirectStandardOutput $GatewayOut `
            -RedirectStandardError $GatewayErr `
            -PassThru
        $StartedGateway = $true
    }
    finally {
        $env:MIR2_GATEWAY_WEB_ADDR = $PreviousWebAddress
        $env:MIR2_GATEWAY_TCP_ADDR = $PreviousTcpAddress
    }

    $Ready = $false
    for ($Attempt = 0; $Attempt -lt 120; $Attempt += 1) {
        if ($GatewayProcess.HasExited) {
            $ErrorTail = if (Test-Path -LiteralPath $GatewayErr) {
                (Get-Content -LiteralPath $GatewayErr -Tail 40 | Out-String)
            }
            else {
                "(no gateway error log)"
            }
            throw "Gateway exited during startup.`n$ErrorTail"
        }
        if (Test-HttpOk $GatewayHealthUrl) {
            $Ready = $true
            break
        }
        Start-Sleep -Milliseconds 500
    }
    if (-not $Ready) {
        throw "Gateway did not become healthy within 60 seconds. Logs: $LogRoot"
    }
}
else {
    Write-Host "[start] explicitly reusing healthy Gateway at $GatewayHealthUrl"
}

$PreviousPrebuilt = $env:MIR2_USE_PREBUILT_BEVY_RUNTIME
$PreviousGatewayWs = $env:NEXT_PUBLIC_MIR2_GATEWAY_WS_URL
$PreviousAssetBase = $env:NEXT_PUBLIC_MIR2_ASSET_BASE_URL
try {
    $env:MIR2_USE_PREBUILT_BEVY_RUNTIME = "1"
    $env:NEXT_PUBLIC_MIR2_GATEWAY_WS_URL = "ws://127.0.0.1:$GatewayWebPort/ws"
    if ($AssetBaseUrl) {
        $env:NEXT_PUBLIC_MIR2_ASSET_BASE_URL = $AssetBaseUrl.TrimEnd("/")
    }
    else {
        # An empty process value wins over stale machine or .env values and keeps startup deterministic.
        $env:NEXT_PUBLIC_MIR2_ASSET_BASE_URL = ""
    }

    Write-Host "[start] Gateway: ws://127.0.0.1:$GatewayWebPort/ws"
    Write-Host "[start] Web:     $WebUrl"
    if ($AssetBaseUrl) {
        Write-Host "[start] Assets: $($env:NEXT_PUBLIC_MIR2_ASSET_BASE_URL)"
    }
    elseif (Test-Path -LiteralPath $LocalFullPackIndex -PathType Leaf) {
        Write-Host "[start] Assets: repository-local full Crystal pack"
    }
    else {
        Write-Host "[start] Assets: repository-local Starter mode"
    }
    Write-Host "[start] Press Ctrl+C to stop Web and the Gateway started by this script."

    if ($OpenBrowser) {
        Start-Process $WebUrl
    }

    Push-Location $WebRoot
    try {
        & npm.cmd run dev -- --hostname 127.0.0.1 --port $WebPort
    }
    finally {
        Pop-Location
    }
}
finally {
    $env:MIR2_USE_PREBUILT_BEVY_RUNTIME = $PreviousPrebuilt
    $env:NEXT_PUBLIC_MIR2_GATEWAY_WS_URL = $PreviousGatewayWs
    $env:NEXT_PUBLIC_MIR2_ASSET_BASE_URL = $PreviousAssetBase

    if ($StartedGateway -and $null -ne $GatewayProcess -and -not $GatewayProcess.HasExited) {
        Stop-Process -Id $GatewayProcess.Id -Force
        $GatewayProcess.WaitForExit()
        Write-Host "[stop] Gateway PID $($GatewayProcess.Id) stopped."
    }
}
