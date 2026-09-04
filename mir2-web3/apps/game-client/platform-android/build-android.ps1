param(
    [ValidateSet('check', 'package')]
    [string]$Mode = $(if ($env:MIR2_ANDROID_MODE) { $env:MIR2_ANDROID_MODE } else { 'check' }),
    [ValidateSet('debug', 'release')]
    [string]$Variant = $(if ($env:MIR2_ANDROID_VARIANT) { $env:MIR2_ANDROID_VARIANT } else { 'debug' })
)

$ErrorActionPreference = 'Stop'
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$manifest = Join-Path $scriptDir 'Cargo.toml'
$androidProject = Join-Path $scriptDir 'android'
$jniRoot = Join-Path $androidProject 'app\src\main\jniLibs'
$ndkOutput = Join-Path $scriptDir 'target\android-jni'
$toolchain = if ($env:MIR2_CLIENT_TOOLCHAIN) { $env:MIR2_CLIENT_TOOLCHAIN } else { '1.95.0' }
$target = if ($env:MIR2_ANDROID_TARGET) { $env:MIR2_ANDROID_TARGET } else { 'aarch64-linux-android' }
$abi = if ($env:MIR2_ANDROID_ABI) { $env:MIR2_ANDROID_ABI } else { 'arm64-v8a' }
$apiLevel = if ($env:MIR2_ANDROID_API_LEVEL) { [int]$env:MIR2_ANDROID_API_LEVEL } else { 31 }
$rustProfile = if ($env:MIR2_ANDROID_RUST_PROFILE) { $env:MIR2_ANDROID_RUST_PROFILE } else { 'release' }

function Fail([string]$Message) {
    throw "[platform-android] error: $Message"
}

if ($target -ne 'aarch64-linux-android' -or $abi -ne 'arm64-v8a') {
    Fail 'M0 supports only aarch64-linux-android / arm64-v8a'
}
if ($apiLevel -lt 31) { Fail 'GameActivity M0 requires MIR2_ANDROID_API_LEVEL >= 31' }
if ($rustProfile -notin @('debug', 'release')) { Fail "MIR2_ANDROID_RUST_PROFILE must be 'debug' or 'release'" }

foreach ($command in @('cargo', 'cargo-ndk', 'rustup')) {
    if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
        Fail "required command '$command' is not on PATH"
    }
}

& rustup run $toolchain rustc --version *> $null
if ($LASTEXITCODE -ne 0) {
    Fail "Rust toolchain '$toolchain' is unavailable; install it locally or set MIR2_CLIENT_TOOLCHAIN"
}

$installedTargets = @(& rustup target list --installed --toolchain $toolchain)
if ($installedTargets -notcontains $target) {
    Fail "Rust target '$target' is not installed for '$toolchain'; run 'rustup target add --toolchain $toolchain $target'"
}

$sdkRoot = if ($env:ANDROID_SDK_ROOT) { $env:ANDROID_SDK_ROOT } elseif ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { Join-Path $env:LOCALAPPDATA 'Android\Sdk' }
if (-not (Test-Path -LiteralPath $sdkRoot)) { Fail 'Android SDK not found; set ANDROID_SDK_ROOT or ANDROID_HOME' }

$ndkHome = if ($env:ANDROID_NDK_HOME) { $env:ANDROID_NDK_HOME } elseif ($env:ANDROID_NDK_ROOT) { $env:ANDROID_NDK_ROOT } else { $null }
if (-not $ndkHome) {
    $ndkRoot = Join-Path $sdkRoot 'ndk'
    if (Test-Path -LiteralPath $ndkRoot) {
        $ndkHome = Get-ChildItem -LiteralPath $ndkRoot -Directory |
            Sort-Object { [version]$_.Name } -Descending |
            Select-Object -First 1 -ExpandProperty FullName
    }
}
if (-not $ndkHome -or -not (Test-Path -LiteralPath $ndkHome)) {
    Fail 'Android NDK not found; set ANDROID_NDK_HOME or ANDROID_NDK_ROOT'
}

$env:ANDROID_SDK_ROOT = $sdkRoot
$env:ANDROID_NDK_HOME = $ndkHome

Write-Host "[platform-android] $Mode $target with Rust $toolchain, NDK $ndkHome, API $apiLevel"
$ndkArgs = @("+$toolchain", 'ndk', '--manifest-path', $manifest, '--target', $abi, '--platform', "$apiLevel")

if ($Mode -eq 'check') {
    & cargo @ndkArgs check --lib --locked --offline
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    Write-Host '[platform-android] target check passed'
    exit 0
}

if (-not (Get-Command java -ErrorAction SilentlyContinue)) { Fail "required command 'java' is not on PATH" }
$gradleWrapper = Join-Path $androidProject 'gradlew.bat'
if (-not (Test-Path -LiteralPath $gradleWrapper)) { Fail 'checked-in Gradle wrapper is missing' }

$buildArgs = @('build', '--lib', '--locked', '--offline')
$gradleTask = 'assembleDebug'
$apkPath = Join-Path $androidProject 'app\build\outputs\apk\debug\app-debug.apk'
if ($rustProfile -eq 'release') {
    $buildArgs += '--release'
}
if ($Variant -eq 'release') {
    $gradleTask = 'assembleRelease'
    $apkPath = Join-Path $androidProject 'app\build\outputs\apk\release\app-release-unsigned.apk'
}

$nativeLib = Join-Path $jniRoot "$abi\libmir2_platform_android.so"
$stagedLib = Join-Path $ndkOutput "$abi\libmir2_platform_android.so"
Remove-Item -LiteralPath $stagedLib -Force -ErrorAction SilentlyContinue
& cargo @ndkArgs --output-dir $ndkOutput @buildArgs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
if (-not (Test-Path -LiteralPath $stagedLib)) { Fail "cargo-ndk did not produce '$stagedLib'" }
$nativeDir = Split-Path -Parent $nativeLib
New-Item -ItemType Directory -Path $nativeDir -Force | Out-Null
Remove-Item -LiteralPath $nativeLib -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath (Join-Path $nativeDir 'libmir2_bevy_runtime.so') -Force -ErrorAction SilentlyContinue
Copy-Item -LiteralPath $stagedLib -Destination $nativeLib

Push-Location $androidProject
try {
    & $gradleWrapper --no-daemon $gradleTask
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $apkPath)) { Fail "Gradle did not produce '$apkPath'" }
$apkSha = (Get-FileHash -Algorithm SHA256 -LiteralPath $apkPath).Hash.ToLowerInvariant()
Write-Host "[platform-android] APK: $apkPath"
Write-Host "[platform-android] SHA-256: $apkSha"
Write-Host '[platform-android] package gate passed'
