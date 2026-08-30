param(
    [ValidateSet('check', 'package')]
    [string]$Mode = $(if ($env:MIR2_ANDROID_MODE) { $env:MIR2_ANDROID_MODE } else { 'check' })
)

$ErrorActionPreference = 'Stop'
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$manifest = Join-Path $scriptDir 'Cargo.toml'
$toolchain = if ($env:MIR2_CLIENT_TOOLCHAIN) { $env:MIR2_CLIENT_TOOLCHAIN } else { '1.95.0' }
$target = if ($env:MIR2_ANDROID_TARGET) { $env:MIR2_ANDROID_TARGET } else { 'aarch64-linux-android' }
$apiLevel = if ($env:MIR2_ANDROID_API_LEVEL) { $env:MIR2_ANDROID_API_LEVEL } else { '26' }

function Fail([string]$Message) {
    throw "[platform-android] error: $Message"
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { Fail "required command 'cargo' is not on PATH" }
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) { Fail "required command 'rustup' is not on PATH" }

& rustup run $toolchain rustc --version *> $null
if ($LASTEXITCODE -ne 0) { Fail "Rust toolchain '$toolchain' is unavailable; install it locally or set MIR2_CLIENT_TOOLCHAIN" }

$installedTargets = @(& rustup target list --installed --toolchain $toolchain)
if ($installedTargets -notcontains $target) {
    Fail "Rust target '$target' is not installed for '$toolchain'; run 'rustup target add --toolchain $toolchain $target'"
}

$ndkHome = if ($env:ANDROID_NDK_HOME) { $env:ANDROID_NDK_HOME } elseif ($env:ANDROID_NDK_ROOT) { $env:ANDROID_NDK_ROOT } else { $null }
$sdkRoots = @($env:ANDROID_SDK_ROOT, $env:ANDROID_HOME, (Join-Path $env:LOCALAPPDATA 'Android\Sdk')) | Where-Object { $_ }
if (-not $ndkHome) {
    foreach ($sdkRoot in $sdkRoots) {
        $ndkRoot = Join-Path $sdkRoot 'ndk'
        if (Test-Path -LiteralPath $ndkRoot) {
            $ndkHome = Get-ChildItem -LiteralPath $ndkRoot -Directory | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
            if ($ndkHome) { break }
        }
    }
}
if (-not $ndkHome -or -not (Test-Path -LiteralPath $ndkHome)) { Fail 'Android NDK not found; set ANDROID_NDK_HOME or ANDROID_NDK_ROOT' }

$prebuiltRoot = Join-Path $ndkHome 'toolchains\llvm\prebuilt'
$prebuilt = Get-ChildItem -LiteralPath $prebuiltRoot -Directory -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $prebuilt) { Fail "NDK LLVM prebuilt directory not found below '$ndkHome'" }

$targetEnv = $target.ToUpperInvariant().Replace('-', '_')
$linkerPattern = "$target$apiLevel-clang*"
$linker = Get-ChildItem -LiteralPath (Join-Path $prebuilt.FullName 'bin') -Filter $linkerPattern -File -ErrorAction SilentlyContinue | Select-Object -First 1
$ar = Get-ChildItem -LiteralPath (Join-Path $prebuilt.FullName 'bin') -Filter 'llvm-ar*' -File -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $linker) { Fail "NDK linker '$target$apiLevel-clang' is missing" }
if (-not $ar) { Fail "NDK llvm-ar is missing" }

Set-Item -Path "Env:CARGO_TARGET_${targetEnv}_LINKER" -Value $linker.FullName
Set-Item -Path "Env:CARGO_TARGET_${targetEnv}_AR" -Value $ar.FullName

Write-Host "[platform-android] $Mode $target with Rust $toolchain, NDK $ndkHome, API $apiLevel"
if ($Mode -eq 'package') {
    if (-not (Get-Command cargo-apk -ErrorAction SilentlyContinue)) { Fail "required command 'cargo-apk' is not on PATH" }
    & cargo "+$toolchain" apk --manifest-path $manifest --target $target --release --locked --offline
} else {
    & cargo "+$toolchain" check --manifest-path $manifest --target $target --locked --offline
}
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "[platform-android] $Mode gate passed"
