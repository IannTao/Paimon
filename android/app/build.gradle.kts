plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "dev.paimon.probe"
    compileSdk = 35
    ndkVersion = "27.0.12077973"
    defaultConfig {
        applicationId = "dev.paimon.probe"
        minSdk = 34
        targetSdk = 34
        versionCode = 7
        versionName = "0.4.0-localization"
        testInstrumentationRunner = providers.gradleProperty("deviceCheckRunner")
            .orElse("dev.paimon.probe.MovementDeviceChecks").get()
        ndk { abiFilters += "arm64-v8a" }
    }
    buildFeatures { aidl = true; buildConfig = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    sourceSets["main"].jniLibs.srcDir(layout.buildDirectory.dir("rustJniLibs"))
    sourceSets["androidTest"].assets.srcDir("../../probe-core/tests/fixtures")
}

dependencies {
    implementation("dev.rikka.shizuku:api:13.1.5")
    implementation("dev.rikka.shizuku:provider:13.1.5")
}

val buildRust by tasks.registering(Exec::class) {
    val ndk = android.sdkDirectory.resolve("ndk/${android.ndkVersion}")
    val windows = System.getProperty("os.name").startsWith("Windows")
    val host = if (windows) "windows-x86_64" else if (System.getProperty("os.name").contains("Mac")) "darwin-x86_64" else "linux-x86_64"
    val linker = ndk.resolve("toolchains/llvm/prebuilt/$host/bin/aarch64-linux-android34-clang${if (windows) ".cmd" else ""}")
    workingDir(rootProject.projectDir.parentFile)
    environment("CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER", linker.absolutePath)
    // Keep output location independent of any developer-global Cargo configuration.
    environment("CARGO_TARGET_DIR", rootProject.projectDir.parentFile.resolve("target").absolutePath)
    commandLine("cargo", "build", "--locked", "--offline", "--release", "--target", "aarch64-linux-android", "-p", "paimon-probe")
    inputs.dir(rootProject.projectDir.parentFile.resolve("probe-core/src"))
    inputs.dir(rootProject.projectDir.parentFile.resolve("probe-core/assets"))
    inputs.file(rootProject.projectDir.parentFile.resolve("probe-core/Cargo.toml"))
    inputs.file(rootProject.projectDir.parentFile.resolve("Cargo.lock"))
    outputs.file(rootProject.projectDir.parentFile.resolve("target/aarch64-linux-android/release/libpaimon_probe.so"))
    doFirst { check(linker.isFile) { "Install Android NDK ${android.ndkVersion}: $linker" } }
}
val packageRust by tasks.registering(Copy::class) {
    dependsOn(buildRust)
    from(rootProject.projectDir.parentFile.resolve("target/aarch64-linux-android/release/libpaimon_probe.so"))
    into(layout.buildDirectory.dir("rustJniLibs/arm64-v8a"))
}
tasks.named("preBuild") { dependsOn(packageRust) }
