plugins { id("com.android.application") }

android {
    namespace = "dev.paimon.touchreceiver"
    compileSdk = 35
    defaultConfig {
        applicationId = "dev.paimon.touchreceiver"
        minSdk = 34
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
