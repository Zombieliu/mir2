param(
    [string]$Profile = "release"
)

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$RuntimeDir = Join-Path $Root "..\\game-client\\runtime"
$PkgDir = Join-Path $Root "public\\bevy-runtime\\pkg"
$Cargo = "C:\\Users\\Administrator\\.cargo\\bin\\cargo.exe"
$WasmBindgen = "C:\\Users\\Administrator\\.cargo\\bin\\wasm-bindgen.exe"

if ($Profile -eq "dev") {
    $CargoProfileDir = "debug"
    $CargoFlags = @()
} elseif ($Profile -eq "release") {
    $CargoProfileDir = "release"
    $CargoFlags = @("--release")
} else {
    throw "Unsupported profile: $Profile"
}

if (Test-Path $PkgDir) {
    Remove-Item -Recurse -Force $PkgDir
}

New-Item -ItemType Directory -Force -Path $PkgDir | Out-Null

Push-Location $RuntimeDir
try {
    & $Cargo build --target wasm32-unknown-unknown @CargoFlags

    $WasmFile = Join-Path $RuntimeDir "target\\wasm32-unknown-unknown\\$CargoProfileDir\\mir2_bevy_runtime.wasm"
    & $WasmBindgen `
        --target web `
        --out-dir $PkgDir `
        --out-name mir2_bevy_runtime `
        $WasmFile
}
finally {
    Pop-Location
}
