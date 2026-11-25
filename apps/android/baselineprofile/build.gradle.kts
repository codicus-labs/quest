plugins {
    id("com.android.test")
    id("androidx.baselineprofile")
}

/**
 * Generates the app's Baseline Profile (`BaselineProfileGenerator`) and measures cold start with and
 * without it (`StartupBenchmark`). Runs on a device or emulator with the app signed in; see README.
 */
android {
    namespace = "app.quest.android.baselineprofile"
    compileSdk = 37
    compileSdkMinor = 2

    defaultConfig {
        minSdk = 31
        targetSdk = 37
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    targetProjectPath = ":app"
}

baselineProfile {
    // The emulator `quest-api36` (or a phone) that adb sees; no Gradle-managed device.
    useConnectedDevices = true
}

dependencies {
    implementation("androidx.test.ext:junit:1.3.0")
    implementation("androidx.test.uiautomator:uiautomator:2.4.1")
    implementation("androidx.benchmark:benchmark-macro-junit4:1.5.0")
}
