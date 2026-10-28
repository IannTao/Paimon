param([string]$Ndk = $env:ANDROID_NDK_HOME, [switch]$CheckOnly)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
$previousNdk = $env:PAIMON_NDK
$previousLinker = $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER
$previousTargetDir = $env:CARGO_TARGET_DIR
try {
    & cargo check --workspace --locked --offline
    if ($LASTEXITCODE -ne 0) { throw 'Rust typecheck failed' }
    & cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw 'Rust formatting check failed' }
    if ($CheckOnly) { return }
    if (-not $Ndk -and $env:ANDROID_HOME) { $Ndk = Join-Path $env:ANDROID_HOME 'ndk/27.0.12077973' }
    if (-not $Ndk) { throw 'Pass -Ndk with the Android NDK 27.0.12077973 directory.' }
    $Ndk = (Resolve-Path -LiteralPath $Ndk).Path
    $linker = Join-Path $Ndk 'toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android34-clang.cmd'
    if (-not (Test-Path -LiteralPath $linker -PathType Leaf)) { throw "NDK linker missing: $linker" }
    $env:PAIMON_NDK = $Ndk
    $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = $linker
    $env:CARGO_TARGET_DIR = Join-Path $projectRoot 'target'
    & cargo build -p paimon-touch-helper --release --target aarch64-linux-android --locked --offline
    if ($LASTEXITCODE -ne 0) { throw 'Android helper build failed' }
    Write-Output 'Helper: target/aarch64-linux-android/release/paimon-touch-helper'
} finally {
    $env:PAIMON_NDK = $previousNdk
    $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = $previousLinker
    $env:CARGO_TARGET_DIR = $previousTargetDir
    Pop-Location
}
