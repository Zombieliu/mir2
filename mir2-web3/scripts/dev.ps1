[CmdletBinding()]
param(
    [ValidateSet("doctor", "auth", "build", "up", "down", "logs", "shell", "verify", "assets", "status")]
    [string]$Command = "up",

    [ValidateRange(1024, 65535)]
    [int]$WebPort = 3002,

    [ValidateRange(1024, 65535)]
    [int]$GatewayWebPort = 7110,

    [ValidateRange(1024, 65535)]
    [int]$GatewayTcpPort = 7000,

    [string]$BindAddress = "127.0.0.1",
    [string]$GatewayWsUrl = "",
    [string]$AssetBaseUrl = "",
    [switch]$OpenBrowser,
    [switch]$Build,
    [switch]$FullAssets,
    [switch]$RemoveVolumes
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ProjectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$RepositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot ".."))
$ComposeFile = Join-Path $ProjectRoot "infra\compose.developer.yml"
$ReleaseLockPath = Join-Path $ProjectRoot "config\developer-release.json"
$AssetManifestPath = Join-Path $ProjectRoot "config\developer-assets.json"
$WebUrl = "http://127.0.0.1:$WebPort/"
$GatewayHealthUrl = "http://127.0.0.1:$GatewayWebPort/health"

function Require-Command {
    param([string]$Name, [string]$InstallHint)

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing required command '$Name'. $InstallHint"
    }
}

function Invoke-External {
    param(
        [string]$Label,
        [string]$FilePath,
        [string[]]$Arguments
    )

    Write-Host "[dev] $Label"
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE"
    }
}

function Invoke-Compose {
    param([string[]]$Arguments)

    Invoke-External -Label "docker compose $($Arguments -join ' ')" `
        -FilePath "docker" `
        -Arguments (@("compose", "-f", $ComposeFile) + $Arguments)
}

function Test-HttpOk {
    param([string]$Url)

    try {
        $Response = Invoke-WebRequest -UseBasicParsing -Uri $Url -TimeoutSec 3
        return $Response.StatusCode -ge 200 -and $Response.StatusCode -lt 400
    }
    catch {
        return $false
    }
}

function Wait-ForHttp {
    param(
        [string]$Name,
        [string]$Url,
        [int]$TimeoutSeconds = 600
    )

    $Deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $Deadline) {
        if (Test-HttpOk -Url $Url) {
            Write-Host "[ready] $Name $Url"
            return
        }
        Start-Sleep -Seconds 2
    }

    Write-Host "[error] $Name did not become ready. Recent logs:"
    & docker compose -f $ComposeFile logs --tail 120 gateway web
    throw "$Name did not become ready within $TimeoutSeconds seconds: $Url"
}

function Test-DockerEngine {
    $PreviousPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $Output = (& docker info --format "{{.ServerVersion}}" 2>&1 | Out-String).Trim()
        $ExitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $PreviousPreference
    }
    if ($ExitCode -ne 0 -or -not $Output -or $Output -match "(?i)(error|failed|cannot connect)") {
        throw "Docker engine is not ready. Start Docker Desktop and wait for the Linux engine.`n$Output"
    }
    Write-Host "[ok] Docker engine $Output"
}

function Test-ReleaseLock {
    $ReleaseLock = Get-Content -LiteralPath $ReleaseLockPath -Raw | ConvertFrom-Json
    $AssetManifest = Get-Content -LiteralPath $AssetManifestPath -Raw | ConvertFrom-Json
    $ExpectedCrystal = (& git -C $RepositoryRoot ls-tree HEAD Crystal).Split()[2]

    $SubmoduleState = (& git -C $RepositoryRoot submodule status --recursive 2>&1 | Out-String).Trim()
    if (-not $SubmoduleState -or $SubmoduleState.StartsWith("-")) {
        Invoke-External -Label "initialize Crystal submodule" `
            -FilePath "git" `
            -Arguments @("-C", $RepositoryRoot, "submodule", "update", "--init", "--recursive")
    }

    $ActualCrystal = (& git -C (Join-Path $RepositoryRoot "Crystal") rev-parse HEAD).Trim()

    if ($ExpectedCrystal -ne $ReleaseLock.crystal.commit) {
        throw "developer-release.json Crystal commit does not match the repository gitlink."
    }
    if ($ActualCrystal -ne $ExpectedCrystal) {
        throw "Crystal submodule mismatch. Run: git submodule update --init --recursive"
    }
    if ($ReleaseLock.assets.releaseTag -ne $AssetManifest.releaseTag -or
        $ReleaseLock.assets.contentHash -ne $AssetManifest.contentHash) {
        throw "Developer release lock and asset manifest are out of sync."
    }

    Write-Host "[ok] Crystal $ActualCrystal"
    Write-Host "[ok] Assets $($AssetManifest.releaseTag) / $($AssetManifest.contentHash)"
    Write-Host "[ok] Toolchain Node $($ReleaseLock.toolchains.node), npm $($ReleaseLock.toolchains.npm), Rust $($ReleaseLock.toolchains.rust)"
}

function Install-FullAssets {
    if (-not $env:MIR2_DEV_IMAGE -or $env:MIR2_DEV_IMAGE -notmatch "@sha256:") {
        throw "Full assets require the published digest-pinned developer image."
    }

    Invoke-External -Label "ensure persistent GitHub authorization volume" `
        -FilePath "docker" `
        -Arguments @("volume", "create", "mir2-developer-gh-config")

    $RunArguments = @("run", "--rm", "--no-deps")
    if ($env:GH_TOKEN) {
        $RunArguments += @("-e", "GH_TOKEN")
    }

    $AuthReady = $false
    $PreviousPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & docker compose -f $ComposeFile @RunArguments asset-auth gh auth status *> $null
        $AuthReady = $LASTEXITCODE -eq 0
    }
    finally {
        $ErrorActionPreference = $PreviousPreference
    }
    if (-not $AuthReady) {
        Write-Host "[assets] Authorize access to the pinned private release."
        Invoke-Compose -Arguments @(
            "run", "--rm", "--no-deps", "asset-auth",
            "gh", "auth", "login", "--web", "--git-protocol", "https"
        )
    }

    Invoke-Compose -Arguments ($RunArguments + @("asset-fetch"))

    $AssetManifest = Get-Content -LiteralPath $AssetManifestPath -Raw | ConvertFrom-Json
    $AssetCache = ".mir2-data/developer-assets/$($AssetManifest.releaseTag)"
    Invoke-Compose -Arguments @(
        "run", "--rm", "--no-deps", "workspace",
        "bash", "scripts/install-developer-assets.sh",
        "--parts-directory", $AssetCache,
        "--cache-directory", $AssetCache
    )
}

Require-Command "docker" "Install Docker Desktop and enable Docker Compose."
Require-Command "git" "Install Git for Windows."
Test-DockerEngine
Invoke-External -Label "check Docker Compose" -FilePath "docker" -Arguments @("compose", "version")
Invoke-External -Label "ensure persistent GitHub authorization volume" `
    -FilePath "docker" `
    -Arguments @("volume", "create", "mir2-developer-gh-config")

$PreviousEnvironment = @{
    MIR2_DEV_IMAGE = $env:MIR2_DEV_IMAGE
    MIR2_WEB_PORT = $env:MIR2_WEB_PORT
    MIR2_GATEWAY_WEB_PORT = $env:MIR2_GATEWAY_WEB_PORT
    MIR2_GATEWAY_TCP_PORT = $env:MIR2_GATEWAY_TCP_PORT
    MIR2_BIND_ADDRESS = $env:MIR2_BIND_ADDRESS
    MIR2_GATEWAY_WS_URL = $env:MIR2_GATEWAY_WS_URL
    MIR2_ASSET_BASE_URL = $env:MIR2_ASSET_BASE_URL
}

try {
    if (-not $env:MIR2_DEV_IMAGE) {
        $DeveloperRelease = Get-Content -LiteralPath $ReleaseLockPath -Raw | ConvertFrom-Json
        if ($DeveloperRelease.container.publishedDigest) {
            $env:MIR2_DEV_IMAGE = "$($DeveloperRelease.container.publishedImage)@$($DeveloperRelease.container.publishedDigest)"
        }
    }
    $env:MIR2_WEB_PORT = "$WebPort"
    $env:MIR2_GATEWAY_WEB_PORT = "$GatewayWebPort"
    $env:MIR2_GATEWAY_TCP_PORT = "$GatewayTcpPort"
    $env:MIR2_BIND_ADDRESS = $BindAddress
    $env:MIR2_GATEWAY_WS_URL = if ($GatewayWsUrl) {
        $GatewayWsUrl
    }
    else {
        "ws://127.0.0.1:$GatewayWebPort/ws"
    }
    $env:MIR2_ASSET_BASE_URL = $AssetBaseUrl.TrimEnd("/")

    switch ($Command) {
        "doctor" {
            Require-Command "git" "Install Git for Windows."
            Test-ReleaseLock
            Invoke-Compose -Arguments @("config", "--quiet")
            Write-Host "[ok] Developer environment definition is valid."
        }
        "auth" {
            Test-ReleaseLock
            if (-not $env:MIR2_DEV_IMAGE -or $env:MIR2_DEV_IMAGE -notmatch "@sha256:") {
                throw "Asset authorization requires the published digest-pinned developer image."
            }
            Invoke-External -Label "ensure persistent GitHub authorization volume" `
                -FilePath "docker" `
                -Arguments @("volume", "create", "mir2-developer-gh-config")
            Invoke-Compose -Arguments @(
                "run", "--rm", "--no-deps", "asset-auth",
                "gh", "auth", "login", "--web", "--git-protocol", "https"
            )
        }
        "build" {
            $env:MIR2_DEV_IMAGE = "mir2-web3-developer:local"
            Test-ReleaseLock
            Invoke-Compose -Arguments @("build", "workspace")
        }
        "up" {
            Test-ReleaseLock
            if ($FullAssets) {
                Install-FullAssets
            }
            if ($Build) {
                $env:MIR2_DEV_IMAGE = "mir2-web3-developer:local"
            }
            $UpArguments = @("up", "-d")
            if ($Build) {
                $UpArguments += "--build"
            }
            $UpArguments += @("gateway", "web")
            Invoke-Compose -Arguments $UpArguments
            Wait-ForHttp -Name "Gateway" -Url $GatewayHealthUrl
            Wait-ForHttp -Name "Player Web" -Url $WebUrl
            Write-Host ""
            Write-Host "Mir2 is ready: $WebUrl"
            Write-Host "Stop it with: .\scripts\dev.ps1 down"
            if ($OpenBrowser) {
                Start-Process $WebUrl
            }
        }
        "down" {
            $DownArguments = @("down", "--remove-orphans")
            if ($RemoveVolumes) {
                $DownArguments += "--volumes"
            }
            Invoke-Compose -Arguments $DownArguments
        }
        "logs" {
            & docker compose -f $ComposeFile logs -f --tail 200 gateway web
            if ($LASTEXITCODE -ne 0) {
                throw "docker compose logs failed with exit code $LASTEXITCODE"
            }
        }
        "shell" {
            Invoke-Compose -Arguments @("run", "--rm", "--no-deps", "workspace", "bash")
        }
        "verify" {
            Test-ReleaseLock
            Invoke-Compose -Arguments @(
                "run", "--rm", "--no-deps", "workspace",
                "bash", "-lc",
                "npm ci --prefix apps/web && npm --prefix apps/web run typecheck && cargo +1.89.0 check --locked -p mir2-gateway"
            )
        }
        "assets" {
            Test-ReleaseLock
            Install-FullAssets
        }
        "status" {
            Invoke-Compose -Arguments @("ps")
            Write-Host "Gateway health: $(if (Test-HttpOk $GatewayHealthUrl) { 'ready' } else { 'not ready' })"
            Write-Host "Player Web:     $(if (Test-HttpOk $WebUrl) { 'ready' } else { 'not ready' })"
        }
    }
}
finally {
    foreach ($Entry in $PreviousEnvironment.GetEnumerator()) {
        Set-Item -Path "Env:$($Entry.Key)" -Value $Entry.Value
    }
}
