import groovy.json.JsonSlurper
import java.util.Properties
import javax.inject.Inject

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
    id("io.github.takahirom.roborazzi")
    id("androidx.baselineprofile")
}

/** `versionName` from `version.properties` (major.minor.patch) and the `versionCode` derived from it. */
val appVersionName: String = Properties().run {
    rootProject.file("version.properties").inputStream().use { load(it) }
    getProperty("versionName") ?: error("version.properties: versionName is missing.")
}
val appVersionCode: Int = run {
    val parts = Regex("""(\d+)\.(\d+)\.(\d+)""").matchEntire(appVersionName)?.destructured?.toList()?.map(String::toInt)
        ?: error("version.properties: versionName must be major.minor.patch, got \"$appVersionName\".")
    val (major, minor, patch) = parts
    require(minor < 1000 && patch < 1000) { "version.properties: minor and patch must stay below 1000." }
    major * 1_000_000 + minor * 1_000 + patch
}

/** A release signing value from `~/.gradle/gradle.properties` or the environment, never from the repo. */
fun releaseSigning(name: String): String? {
    check(rootProject.file("gradle.properties").readLines().none { it.substringBefore('=').substringBefore(':').trim() == name }) {
        "$name is set in apps/android/gradle.properties, which is in the repo. Move it to ~/.gradle/gradle.properties."
    }
    return providers.gradleProperty(name).orElse(providers.environmentVariable(name)).orNull?.takeIf { it.isNotBlank() }
}

val releaseStoreFile = releaseSigning("QUEST_RELEASE_STORE_FILE")

/**
 * ABIs built into the APK (Gradle property `quest.abis`, space-separated). Default: phones and both emulator
 * kinds. Local builds on Apple silicon may set `quest.abis=arm64-v8a` in `~/.gradle/gradle.properties`, which
 * halves the Rust build time; release builds keep the default.
 */
val androidAbis: List<String> = providers.gradleProperty("quest.abis").orElse("arm64-v8a x86_64").get()
    .split(' ').filter { it.isNotBlank() }

android {
    namespace = "app.quest.android"
    compileSdk = 37
    compileSdkMinor = 2
    ndkVersion = "30.0.16248370"

    defaultConfig {
        applicationId = "app.quest.android"
        minSdk = 31
        targetSdk = 37
        versionCode = appVersionCode
        versionName = appVersionName
        ndk { abiFilters += androidAbis }
    }

    signingConfigs {
        // Without QUEST_RELEASE_STORE_FILE the release APK stays unsigned (app-release-unsigned.apk), so a local
        // assembleRelease works without the key; the release workflow fails when its secrets are missing.
        if (releaseStoreFile != null) {
            create("release") {
                storeFile = file(releaseStoreFile)
                storePassword = releaseSigning("QUEST_RELEASE_STORE_PASSWORD")
                    ?: error("QUEST_RELEASE_STORE_PASSWORD is not set (gradle.properties or environment).")
                keyAlias = releaseSigning("QUEST_RELEASE_KEY_ALIAS")
                    ?: error("QUEST_RELEASE_KEY_ALIAS is not set (gradle.properties or environment).")
                keyPassword = releaseSigning("QUEST_RELEASE_KEY_PASSWORD")
                    ?: error("QUEST_RELEASE_KEY_PASSWORD is not set (gradle.properties or environment).")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release")
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    testOptions {
        unitTests.isIncludeAndroidResources = true
        // Screenshots render through Robolectric's native graphics; hardware pixel copy draws every frame reliably.
        unitTests.all { it.systemProperty("robolectric.pixelCopyRenderMode", "hardware") }
    }
}

roborazzi {
    outputDir.set(file("src/test/screenshots"))
}

/** Builds the Rust core (`crates/quest-mobile`) for Android and generates its Kotlin bindings. */
abstract class CargoBuild : DefaultTask() {
    @get:InputFile
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val script: RegularFileProperty

    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val rustSources: ConfigurableFileCollection

    /** Passed to the script as QUEST_ANDROID_ABIS. */
    @get:Input
    abstract val abis: ListProperty<String>

    @get:OutputDirectory
    abstract val jniLibs: DirectoryProperty

    @get:OutputDirectory
    abstract val kotlin: DirectoryProperty

    @get:Inject
    abstract val exec: ExecOperations

    @get:Inject
    abstract val files: FileSystemOperations

    @TaskAction
    fun build() {
        // A dropped ABI must not linger in the APK from an earlier run.
        files.delete { delete(jniLibs) }
        exec.exec {
            // Rust build scripts and proc macros run here; they get no release signing secrets.
            environment.keys.removeAll { it.startsWith("QUEST_RELEASE_") }
            environment("QUEST_ANDROID_ABIS", abis.get().joinToString(" "))
            commandLine(
                script.get().asFile.absolutePath,
                jniLibs.get().asFile.absolutePath,
                kotlin.get().asFile.absolutePath,
            )
        }
    }
}

/**
 * Generates `QuestColors.kt` from the desktop's colour tokens (`crates/quest-app/assets/theme.json`),
 * so Android and desktop share one source. Token names become camelCase properties; `accent` may
 * name a palette entry.
 */
abstract class GenerateQuestColors : DefaultTask() {
    @get:InputFile
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val themeJson: RegularFileProperty

    @get:OutputDirectory
    abstract val kotlin: DirectoryProperty

    @TaskAction
    @Suppress("UNCHECKED_CAST")
    fun generate() {
        val theme = JsonSlurper().parse(themeJson.get().asFile) as Map<String, Any>
        val palette = theme["palette"] as Map<String, String>
        val light = theme["light"] as Map<String, String>
        val dark = theme["dark"] as Map<String, String>
        require(light.keys == dark.keys) {
            "theme.json: light and dark must define the same tokens (light only: ${light.keys - dark.keys}, dark only: ${dark.keys - light.keys})"
        }
        val names = light.keys.toList()
        fun camel(token: String) = token.split("-").mapIndexed { i, part -> if (i == 0) part else part.replaceFirstChar(Char::uppercase) }.joinToString("")
        fun argb(value: String): String {
            val hex = (palette[value] ?: value).removePrefix("#")
            val (rgb, alpha) = when (hex.length) {
                6 -> hex to "ff"
                8 -> hex.substring(0, 6) to hex.substring(6)
                else -> throw GradleException("theme.json: \"$value\" is not a #rrggbb or #rrggbbaa colour or a palette name")
            }
            return "0x${alpha}$rgb".uppercase().replace("0X", "0x")
        }
        fun instance(name: String, tokens: Map<String, String>) =
            "val $name = QuestColors(\n" + names.joinToString("") { "    ${camel(it)} = Color(${argb(tokens.getValue(it))}),\n" } + ")\n"

        val source = buildString {
            append("// Generated by :app:generateQuestColors from crates/quest-app/assets/theme.json. Do not edit.\n")
            append("package app.quest.android.ui.theme\n\n")
            append("import androidx.compose.runtime.Immutable\n")
            append("import androidx.compose.ui.graphics.Color\n\n")
            append("/** Quest's colour tokens for one appearance. */\n")
            append("@Immutable\n")
            append("data class QuestColors(\n")
            names.forEach { append("    val ${camel(it)}: Color,\n") }
            append(")\n\n")
            append(instance("LightQuestColors", light))
            append("\n")
            append(instance("DarkQuestColors", dark))
        }
        val out = kotlin.get().asFile.resolve("app/quest/android/ui/theme/QuestColors.kt")
        out.parentFile.mkdirs()
        out.writeText(source)
    }
}

val workspace = rootProject.layout.projectDirectory.dir("../..")
val cargoBuild = tasks.register<CargoBuild>("cargoBuild") {
    script.set(rootProject.layout.projectDirectory.file("build-rust.sh"))
    rustSources.from(
        workspace.file("Cargo.toml"),
        workspace.file("Cargo.lock"),
        fileTree(workspace.dir("crates")) {
            include("quest-domain/**", "quest-protocol/**", "quest-capture/**", "quest-store/**", "quest-view/**", "quest-mobile/**")
            exclude("**/target/**")
        },
    )
    abis.set(androidAbis)
    jniLibs.set(layout.buildDirectory.dir("rust/jniLibs"))
    kotlin.set(layout.buildDirectory.dir("rust/kotlin"))
}

val generateQuestColors = tasks.register<GenerateQuestColors>("generateQuestColors") {
    themeJson.set(workspace.file("crates/quest-app/assets/theme.json"))
    kotlin.set(layout.buildDirectory.dir("generated/questTheme/kotlin"))
}

androidComponents {
    onVariants { variant ->
        variant.sources.jniLibs?.addGeneratedSourceDirectory(cargoBuild, CargoBuild::jniLibs)
        variant.sources.kotlin?.addGeneratedSourceDirectory(cargoBuild, CargoBuild::kotlin)
        variant.sources.kotlin?.addGeneratedSourceDirectory(generateQuestColors, GenerateQuestColors::kotlin)
    }
}

/**
 * Version of the Kotlin half of rustls-platform-verifier. It must equal the crate
 * rustls-platform-verifier-android in Cargo.lock, else HTTPS verification fails at runtime.
 */
val rustlsVerifierVersion = providers.fileContents(workspace.file("Cargo.lock")).asText.map { lock ->
    Regex("name = \"rustls-platform-verifier-android\"\\nversion = \"([^\"]+)\"").find(lock)?.groupValues?.get(1)
        ?: error("rustls-platform-verifier-android is missing from Cargo.lock. Run `cargo build -p quest-mobile` once.")
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2026.11.01")
    implementation(composeBom)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material3:material3-adaptive-navigation-suite")
    implementation("androidx.compose.material:material-icons-core:1.7.8")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.11.1")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.11.1")
    implementation("androidx.lifecycle:lifecycle-process:2.11.1")
    implementation("androidx.datastore:datastore-preferences:1.3.0")
    implementation("androidx.glance:glance-appwidget:1.2.0")
    implementation("androidx.work:work-runtime:2.12.0")
    // Sign-in in a Custom Tab (RFC 8252: the user's browser, not a WebView).
    implementation("androidx.browser:browser:1.10.0")
    // Installs the Baseline Profile on sideloaded installs (adb, Obtainium), where no store compiles it.
    implementation("androidx.profileinstaller:profileinstaller:1.4.1")
    baselineProfile(project(":baselineprofile"))
    implementation("net.java.dev.jna:jna:5.19.1@aar")
    implementation(rustlsVerifierVersion.map { "org.rustls:rustls-platform-verifier:$it" })

    testImplementation(composeBom)
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.11.0")
    testImplementation("org.robolectric:robolectric:4.18.1")
    testImplementation("androidx.compose.ui:ui-test-junit4")
    testImplementation("io.github.takahirom.roborazzi:roborazzi:1.79.0")
    testImplementation("io.github.takahirom.roborazzi:roborazzi-compose:1.79.0")
    testImplementation("androidx.work:work-testing:2.12.0")
    testImplementation("androidx.glance:glance-appwidget-testing:1.2.0")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}
