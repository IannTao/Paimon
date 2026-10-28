param([string]$Gradle = 'gradle', [switch]$CheckOnly)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    & cargo check --workspace --locked --offline
    if ($LASTEXITCODE -ne 0) { throw 'Rust typecheck failed' }
    & cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw 'Rust formatting check failed' }
    if ($CheckOnly) { return }
    if (-not (Get-Command java -ErrorAction SilentlyContinue) -and -not $env:JAVA_HOME) {
        throw 'Install JDK 17 and set JAVA_HOME before building Android.'
    }
    if (-not (Get-Command $Gradle -ErrorAction SilentlyContinue)) {
        throw 'Install Gradle 8.9, or pass -Gradle with the absolute path to gradle.bat.'
    }
    $installedTargets = & rustup target list --installed
    if ('aarch64-linux-android' -notin $installedTargets) {
        throw 'Install the Rust Android target: rustup target add aarch64-linux-android'
    }
    & $Gradle --version
    if ($LASTEXITCODE -ne 0) { throw 'Gradle is unavailable' }
    & $Gradle -p android :app:assembleDebug :app:lintDebug
    if ($LASTEXITCODE -ne 0) { throw 'Android build or lint failed' }
    Write-Output 'APK: android/app/build/outputs/apk/debug/app-debug.apk'
} finally { Pop-Location }
