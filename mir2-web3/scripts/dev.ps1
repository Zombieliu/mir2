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
$script:DeveloperRevision = ""
$script:LocalDeveloperImage = ""
$script:RuntimeImagePrepared = $false
$script:BevyRuntimePrepared = $false
$script:AssetImagePrepared = $false
$script:PublishedImage = ""
$script:PublishedDigest = ""
$script:PublishedRevision = ""
$script:PublishedReference = ""
$script:RequestedDeveloperImage = $env:MIR2_DEV_IMAGE

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

function Invoke-ComposeWithInput {
    param(
        [string]$InputText,
        [string[]]$Arguments
    )

    Write-Host "[dev] docker compose $($Arguments -join ' ')"
    $InputText | & docker compose -f $ComposeFile @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
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

function Select-LocalDeveloperImage {
    $env:MIR2_DEV_IMAGE = $script:LocalDeveloperImage
}

function Test-DockerImageAvailable {
    param([string]$Image)

    $PreviousPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & docker image inspect $Image *> $null
        if ($LASTEXITCODE -eq 0) {
            return $true
        }
        & docker pull $Image *> $null
        return $LASTEXITCODE -eq 0
    }
    finally {
        $ErrorActionPreference = $PreviousPreference
    }
}

function Test-PublishedImageWitness {
    $WitnessTag = "developer-image-$($script:PublishedRevision)"
    $ReferenceRecord = (
        & gh api `
            "repos/Zombieliu/mir2/git/ref/tags/$WitnessTag" `
            --jq '.object.type, .object.sha' |
            Out-String
    ).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to read published developer image witness: $WitnessTag"
    }
    $ReferenceParts = $ReferenceRecord -split "\r?\n"
    if ($ReferenceParts.Count -ne 2 -or
        $ReferenceParts[0] -ne "tag" -or
        $ReferenceParts[1] -notmatch "^[a-f0-9]{40}$") {
        throw "Published developer image witness is missing or is not annotated: $WitnessTag"
    }

    $WitnessRecord = (
        & gh api `
            "repos/Zombieliu/mir2/git/tags/$($ReferenceParts[1])" `
            --jq '.object.sha, .message' |
            Out-String
    ).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to read published developer image witness payload: $WitnessTag"
    }
    $WitnessParts = $WitnessRecord -split "\r?\n"
    if ($WitnessParts.Count -ne 2 -or
        $WitnessParts[0] -ne $script:PublishedRevision -or
        $WitnessParts[1] -ne $script:PublishedReference) {
        throw "Published developer image witness does not match the release lock."
    }
}

function Prepare-RuntimeImage {
    if ($script:RuntimeImagePrepared) {
        return
    }

    if ($script:PublishedReference -and $env:MIR2_DEV_IMAGE -eq $script:PublishedReference) {
        if (Test-DockerImageAvailable -Image $env:MIR2_DEV_IMAGE) {
            $script:RuntimeImagePrepared = $true
            return
        }
        Write-Host "[dev] Published image is unavailable; falling back to the locked local build."
        Select-LocalDeveloperImage
    }

    if ($env:MIR2_DEV_IMAGE -eq $script:LocalDeveloperImage) {
        $ActualRevision = ""
        if (-not $Build) {
            $PreviousPreference = $ErrorActionPreference
            $ErrorActionPreference = "Continue"
            try {
                $ActualRevision = (
                    & docker image inspect `
                        --format '{{ index .Config.Labels "org.opencontainers.image.revision" }}' `
                        $env:MIR2_DEV_IMAGE 2>$null |
                        Out-String
                ).Trim()
            }
            finally {
                $ErrorActionPreference = $PreviousPreference
            }
        }
        if ($ActualRevision -ne $script:DeveloperRevision) {
            Write-Host "[dev] Build the locked local developer image for $($script:DeveloperRevision)."
            Invoke-Compose -Arguments @("build", "workspace")
        }
    }
    elseif (-not (Test-DockerImageAvailable -Image $env:MIR2_DEV_IMAGE)) {
        if ($env:MIR2_DEV_IMAGE -match "@sha256:") {
            throw "Unable to pull the explicitly selected developer image: $($env:MIR2_DEV_IMAGE)"
        }
        Invoke-Compose -Arguments @("build", "workspace")
    }

    $script:RuntimeImagePrepared = $true
}

function Prepare-BevyRuntime {
    if ($script:BevyRuntimePrepared) {
        return
    }

    Prepare-RuntimeImage
    Invoke-Compose -Arguments @(
        "run", "--rm", "--no-deps",
        "--user", "node",
        "--entrypoint", "node",
        "workspace",
        "apps/web/scripts/fetch-prebuilt-bevy-runtime.mjs"
    )
    $script:BevyRuntimePrepared = $true
}

function Prepare-AssetImage {
    if ($script:AssetImagePrepared) {
        return
    }

    if (-not $script:PublishedReference -or -not $script:PublishedRevision) {
        throw "Full assets require a published image digest and revision in config/developer-release.json."
    }
    if ($script:PublishedImage -ne "ghcr.io/zombieliu/mir2-developer" -or
        $script:PublishedDigest -notmatch "^sha256:[a-f0-9]{64}$" -or
        $script:PublishedRevision -notmatch "^[a-f0-9]{40}$") {
        throw "Full assets require the trusted published image, digest, and revision lock."
    }

    if ($script:RequestedDeveloperImage -and
        $script:RequestedDeveloperImage -ne $script:PublishedReference) {
        throw @"
Full asset authorization refuses a custom developer image.
Expected exactly: $($script:PublishedReference)
"@
    }

    Require-Command "gh" "Install GitHub CLI, then run 'gh auth login'."
    $AuthReady = $false
    $PreviousPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & gh auth status --hostname github.com *> $null
        $AuthReady = $LASTEXITCODE -eq 0
    }
    finally {
        $ErrorActionPreference = $PreviousPreference
    }
    if (-not $AuthReady) {
        Write-Host "[assets] Authorize the private repository and package."
        Invoke-External -Label "GitHub device authorization" `
            -FilePath "gh" `
            -Arguments @(
                "auth", "login",
                "--hostname", "github.com",
                "--web",
                "--git-protocol", "https",
                "--scopes", "repo,read:packages"
            )
    }
    Test-PublishedImageWitness

    $GitHubLogin = $env:MIR2_GITHUB_LOGIN
    if (-not $GitHubLogin) {
        $GitHubLogin = (& gh api user --jq .login | Out-String).Trim()
    }
    if ($LASTEXITCODE -ne 0 -or -not $GitHubLogin) {
        throw "GitHub CLI did not return the authenticated login."
    }
    $GitHubToken = (& gh auth token --hostname github.com | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or -not $GitHubToken) {
        throw "GitHub CLI did not return an authentication token."
    }
    $DockerConfigRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    $DockerConfig = Join-Path $DockerConfigRoot ("mir2-docker-auth-" + [Guid]::NewGuid().ToString("N"))
    $PreviousDockerConfig = [Environment]::GetEnvironmentVariable("DOCKER_CONFIG", "Process")
    New-Item -ItemType Directory -Path $DockerConfig | Out-Null
    try {
        $env:DOCKER_CONFIG = $DockerConfig
        $GitHubToken | & docker login ghcr.io --username $GitHubLogin --password-stdin
        if ($LASTEXITCODE -ne 0) {
            throw "GHCR login failed. Run: gh auth refresh --hostname github.com --scopes read:packages"
        }
        Invoke-External -Label "pull immutable developer image" `
            -FilePath "docker" `
            -Arguments @("pull", $script:PublishedReference)
    }
    finally {
        $GitHubToken = $null
        if ($null -eq $PreviousDockerConfig) {
            Remove-Item Env:DOCKER_CONFIG -ErrorAction SilentlyContinue
        }
        else {
            $env:DOCKER_CONFIG = $PreviousDockerConfig
        }
        if (Test-Path -LiteralPath $DockerConfig) {
            $ResolvedDockerConfig = [System.IO.Path]::GetFullPath($DockerConfig)
            if (-not $ResolvedDockerConfig.StartsWith(
                $DockerConfigRoot,
                [System.StringComparison]::OrdinalIgnoreCase
            )) {
                throw "Refusing to remove Docker auth directory outside the temporary root."
            }
            Remove-Item -LiteralPath $ResolvedDockerConfig -Recurse -Force
        }
    }

    $env:MIR2_DEV_IMAGE = $script:PublishedReference
    $PreviousPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $ActualRevision = (
            & docker image inspect `
                --format '{{ index .Config.Labels "org.opencontainers.image.revision" }}' `
                $env:MIR2_DEV_IMAGE 2>&1 |
                Out-String
        ).Trim()
        $InspectExitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $PreviousPreference
    }
    if ($InspectExitCode -ne 0) {
        throw "Unable to inspect the published developer image."
    }
    if ($ActualRevision -ne $script:PublishedRevision) {
        throw "Published developer image revision mismatch: expected $($script:PublishedRevision), got $ActualRevision."
    }

    $script:AssetImagePrepared = $true
}

function Install-FullAssets {
    Prepare-AssetImage

    $PreviousToken = $env:GH_TOKEN
    $AssetToken = (& gh auth token --hostname github.com | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or -not $AssetToken) {
        throw "GitHub CLI did not return a token for the asset fetcher."
    }
    try {
        Invoke-ComposeWithInput -InputText $AssetToken -Arguments @(
            "run", "--rm", "--no-deps", "-T", "asset-fetch"
        )
    }
    finally {
        $AssetToken = $null
        if ($null -eq $PreviousToken) {
            Remove-Item Env:GH_TOKEN -ErrorAction SilentlyContinue
        }
        else {
            $env:GH_TOKEN = $PreviousToken
        }
    }

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

$PreviousEnvironment = @{
    MIR2_DEV_IMAGE = $env:MIR2_DEV_IMAGE
    MIR2_DEVELOPER_IMAGE_REVISION = $env:MIR2_DEVELOPER_IMAGE_REVISION
    MIR2_WEB_PORT = $env:MIR2_WEB_PORT
    MIR2_GATEWAY_WEB_PORT = $env:MIR2_GATEWAY_WEB_PORT
    MIR2_GATEWAY_TCP_PORT = $env:MIR2_GATEWAY_TCP_PORT
    MIR2_BIND_ADDRESS = $env:MIR2_BIND_ADDRESS
    MIR2_GATEWAY_WS_URL = $env:MIR2_GATEWAY_WS_URL
    MIR2_ASSET_BASE_URL = $env:MIR2_ASSET_BASE_URL
}

try {
    $script:DeveloperRevision = (& git -C $RepositoryRoot rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $script:DeveloperRevision -notmatch "^[a-f0-9]{40}$") {
        throw "Unable to resolve a full Git revision for the developer image."
    }
    $env:MIR2_DEVELOPER_IMAGE_REVISION = $script:DeveloperRevision
    $script:LocalDeveloperImage = "mir2-web3-developer:local-$($script:DeveloperRevision.Substring(0, 12))"
    $DeveloperRelease = Get-Content -LiteralPath $ReleaseLockPath -Raw | ConvertFrom-Json
    $script:PublishedImage = [string]$DeveloperRelease.container.publishedImage
    $script:PublishedDigest = [string]$DeveloperRelease.container.publishedDigest
    $script:PublishedRevision = [string]$DeveloperRelease.container.publishedRevision
    if ($script:PublishedDigest) {
        $script:PublishedReference = "$($script:PublishedImage)@$($script:PublishedDigest)"
    }

    if (-not $env:MIR2_DEV_IMAGE) {
        if ($script:PublishedReference) {
            $env:MIR2_DEV_IMAGE = $script:PublishedReference
        }
        else {
            Select-LocalDeveloperImage
        }
    }
    if ($Build) {
        Select-LocalDeveloperImage
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
            Prepare-AssetImage
            Write-Host "[ok] GitHub and GHCR authorization are ready for $($script:PublishedReference)."
        }
        "build" {
            Select-LocalDeveloperImage
            Test-ReleaseLock
            Invoke-Compose -Arguments @("build", "workspace")
        }
        "up" {
            $RecoveryHelper = Join-Path $PSScriptRoot "Initialize-LocalSaveRecovery.ps1"
            if (-not (Test-Path -LiteralPath $RecoveryHelper -PathType Leaf)) { throw "Missing local save-recovery bootstrap helper: $RecoveryHelper" }
            . $RecoveryHelper
            $RecoveryBootstrap = Initialize-Mir2LocalSaveRecovery -ProjectRoot $ProjectRoot
            $RecoveryBootstrap.MacKey = $null
            Test-ReleaseLock
            if ($FullAssets) {
                Install-FullAssets
                if ($Build) {
                    Select-LocalDeveloperImage
                    $script:RuntimeImagePrepared = $false
                }
            }
            Prepare-BevyRuntime
            $UpArguments = @("up", "-d")
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
            Prepare-RuntimeImage
            Invoke-Compose -Arguments @("run", "--rm", "--no-deps", "workspace", "bash")
        }
        "verify" {
            Test-ReleaseLock
            Prepare-BevyRuntime
            Invoke-Compose -Arguments @(
                "run", "--rm", "--no-deps", "workspace",
                "bash", "-lc",
                "node apps/web/scripts/fetch-prebuilt-bevy-runtime.mjs && node scripts/check-developer-release.mjs && cargo +1.89.0 fmt --all -- --check && npm ci --prefix apps/web && npm ci --prefix apps/admin-web && npm --prefix apps/web run typecheck && npm --prefix apps/admin-web run typecheck && cargo +1.89.0 check --locked -p mir2-gateway -p mir2-admin-api"
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
        if ($null -eq $Entry.Value) {
            Remove-Item -Path "Env:$($Entry.Key)" -ErrorAction SilentlyContinue
        }
        else {
            Set-Item -Path "Env:$($Entry.Key)" -Value $Entry.Value
        }
    }
}
