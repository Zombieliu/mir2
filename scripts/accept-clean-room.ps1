[CmdletBinding()]
param(
    [string]$Repository = "",
    [string]$Revision = "HEAD",
    [string]$Destination = "",
    [switch]$FullAssets,
    [switch]$Keep,
    [ValidateRange(1024, 65535)]
    [int]$WebPort = 13002,
    [ValidateRange(1024, 65535)]
    [int]$GatewayWebPort = 17110,
    [ValidateRange(1024, 65535)]
    [int]$GatewayTcpPort = 17000
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ProjectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$RepositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot ".."))
$GeneratedTemporaryDestination = -not $Destination
if (-not $Repository) {
    $Repository = $RepositoryRoot
}
if (-not $Destination) {
    $Destination = Join-Path ([System.IO.Path]::GetTempPath()) ("mir2-clean-room-" + [Guid]::NewGuid().ToString("N"))
}
$Destination = [System.IO.Path]::GetFullPath($Destination)
$CloneRoot = Join-Path $Destination "mir2"
$CloneProject = Join-Path $CloneRoot "mir2-web3"
$PreviousComposeProjectName = $env:MIR2_COMPOSE_PROJECT_NAME
$env:MIR2_COMPOSE_PROJECT_NAME = "mir2-clean-room-$([Guid]::NewGuid().ToString('N').Substring(0, 12))"

function Invoke-Checked {
    param([string]$Label, [scriptblock]$Command)

    Write-Host "[clean-room] $Label"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE"
    }
}

if (Test-Path -LiteralPath $Destination) {
    throw "Clean-room destination already exists: $Destination"
}

New-Item -ItemType Directory -Path $Destination | Out-Null
try {
    Invoke-Checked "clone repository into an empty directory" {
        git clone --no-local --recurse-submodules $Repository $CloneRoot
    }
    if ($Revision -ne "HEAD") {
        Invoke-Checked "checkout $Revision" {
            git -C $CloneRoot checkout --detach $Revision
            git -C $CloneRoot submodule update --init --recursive
        }
    }

    $Arguments = @(
        "up",
        "-Build",
        "-WebPort", $WebPort,
        "-GatewayWebPort", $GatewayWebPort,
        "-GatewayTcpPort", $GatewayTcpPort
    )
    if ($FullAssets) {
        $Arguments += "-FullAssets"
    }

    & (Join-Path $CloneProject "scripts\dev.ps1") @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Clean-room startup failed with exit code $LASTEXITCODE"
    }

    $Gateway = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:$GatewayWebPort/health" -TimeoutSec 5
    $Web = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:$WebPort/" -TimeoutSec 10
    if ($Gateway.StatusCode -ne 200 -or $Web.StatusCode -lt 200 -or $Web.StatusCode -ge 400) {
        throw "Clean-room services did not return successful HTTP status codes."
    }

    Write-Host ""
    Write-Host "Clean-room acceptance passed."
    Write-Host "Repository: $Repository"
    Write-Host "Revision:   $(& git -C $CloneRoot rev-parse HEAD)"
    Write-Host "Player Web: http://127.0.0.1:$WebPort/"
}
finally {
    if (Test-Path -LiteralPath $CloneProject) {
        try {
            & (Join-Path $CloneProject "scripts\dev.ps1") down `
                -RemoveVolumes `
                -WebPort $WebPort `
                -GatewayWebPort $GatewayWebPort `
                -GatewayTcpPort $GatewayTcpPort
        }
        catch {
            Write-Warning "Clean-room container cleanup failed: $($_.Exception.Message)"
        }
    }
    if ($GeneratedTemporaryDestination -and -not $Keep -and (Test-Path -LiteralPath $Destination)) {
        $ResolvedDestination = (Resolve-Path -LiteralPath $Destination).Path
        $ResolvedTemp = (Resolve-Path -LiteralPath ([System.IO.Path]::GetTempPath())).Path
        $ResolvedTempPrefix = $ResolvedTemp.TrimEnd("\") + "\"
        if (-not $ResolvedDestination.StartsWith($ResolvedTempPrefix, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove a clean-room directory outside the system temp root: $ResolvedDestination"
        }
        Remove-Item -LiteralPath $ResolvedDestination -Recurse -Force
    }
    if ($null -eq $PreviousComposeProjectName) {
        Remove-Item Env:MIR2_COMPOSE_PROJECT_NAME -ErrorAction SilentlyContinue
    }
    else {
        $env:MIR2_COMPOSE_PROJECT_NAME = $PreviousComposeProjectName
    }
}
