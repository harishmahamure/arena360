import java.io.File
import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

fun loadEnvFile(file: File): Map<String, String> {
    if (!file.exists()) return emptyMap()
    return file.readLines()
        .filter { it.isNotBlank() && !it.startsWith("#") && it.contains("=") }
        .associate { line ->
            val (key, value) = line.split("=", limit = 2)
            key.trim() to value.trim().substringBefore(" #").trim()
        }
}

/** Process env overrides files; within each tier, keys are tried in list order. */
fun resolveFirst(keys: List<String>, vararg files: Map<String, String>): String? {
    for (key in keys) {
        System.getenv(key)?.trim()?.takeIf { it.isNotEmpty() }?.let { return it }
    }
    for (file in files) {
        for (key in keys) {
            file[key]?.trim()?.takeIf { it.isNotEmpty() }?.let { return it }
        }
    }
    return null
}

val envLocal = loadEnvFile(rootProject.file(".env.local"))
val envFile = loadEnvFile(rootProject.file(".env"))
val envMaps = arrayOf(envLocal, envFile)

val apiBaseUrl =
    resolveFirst(listOf("VITE_API_URL", "API_BASE_URL"), *envMaps)
        ?: "http://10.0.2.2:3000"

val gatewayUrl =
    resolveFirst(listOf("VITE_GATEWAY_URL", "VITE_API_URL_WS", "GATEWAY_URL"), *envMaps)
        ?: run {
            val api = apiBaseUrl.trimEnd('/')
            val wsProtocol = if (api.startsWith("https")) "wss" else "ws"
            val host = api.removePrefix("https://").removePrefix("http://")
            "$wsProtocol://$host/realtime"
        }

data class ReleaseSigning(
    val storeFile: File,
    val storePassword: String,
    val keyAlias: String,
    val keyPassword: String,
)

fun loadReleaseSigning(rootDir: File): ReleaseSigning? {
    val envPath = System.getenv("ANDROID_KEYSTORE_PATH")
    val envStorePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
    val envKeyAlias = System.getenv("ANDROID_KEY_ALIAS")
    val envKeyPassword = System.getenv("ANDROID_KEY_PASSWORD")

    if (
        !envPath.isNullOrBlank() &&
        !envStorePassword.isNullOrBlank() &&
        !envKeyAlias.isNullOrBlank() &&
        !envKeyPassword.isNullOrBlank()
    ) {
        val storeFile = File(envPath)
        if (!storeFile.exists()) {
            throw GradleException("ANDROID_KEYSTORE_PATH does not exist: $envPath")
        }
        return ReleaseSigning(storeFile, envStorePassword, envKeyAlias, envKeyPassword)
    }

    val propertiesFile = rootDir.resolve("keystore.properties")
    if (!propertiesFile.exists()) return null

    val props = Properties()
    propertiesFile.inputStream().use { props.load(it) }

    val storeFilePath = props.getProperty("storeFile")?.trim().orEmpty()
    val storePassword = props.getProperty("storePassword")?.trim().orEmpty()
    val keyAlias = props.getProperty("keyAlias")?.trim().orEmpty()
    val keyPassword = props.getProperty("keyPassword")?.trim().orEmpty()

    if (storeFilePath.isBlank() || storePassword.isBlank() || keyAlias.isBlank() || keyPassword.isBlank()) {
        return null
    }

    val storeFile = rootDir.resolve(storeFilePath)
    if (!storeFile.exists()) {
        throw GradleException("Release keystore not found at ${storeFile.absolutePath}")
    }

    return ReleaseSigning(storeFile, storePassword, keyAlias, keyPassword)
}

val releaseSigning = loadReleaseSigning(rootProject.projectDir)

android {
    namespace = "com.gamingcafe.consoletv"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.gamingcafe.consoletv"
        minSdk = 24
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        buildConfigField("String", "API_BASE_URL", "\"$apiBaseUrl\"")
        buildConfigField("String", "GATEWAY_URL", "\"$gatewayUrl\"")
        buildConfigField(
            "String",
            "BACKGROUND_VIDEO_URL",
            "\"https://cdn.arena360.cloud/launch.webm\"",
        )
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    testOptions {
        unitTests.isReturnDefaultValues = true
    }

    lint {
        // AGP/lifecycle lint crash on Kotlin 2.0 + StateFlow (NonNullableMutableLiveDataDetector)
        disable += "NullSafeMutableLiveData"
    }

    signingConfigs {
        val signing = releaseSigning
        if (signing != null) {
            create("release") {
                storeFile = signing.storeFile
                storePassword = signing.storePassword
                keyAlias = signing.keyAlias
                keyPassword = signing.keyPassword
            }
        }
    }

    buildTypes {
        release {
            isDebuggable = false
            isMinifyEnabled = false
            if (releaseSigning != null) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }
}

gradle.taskGraph.whenReady {
    val releaseRequested = allTasks.any { task ->
        val name = task.name.lowercase()
        name.contains("release") &&
            (name.startsWith("bundle") || name.startsWith("assemble") || name.startsWith("package"))
    }
    if (releaseRequested && releaseSigning == null) {
        throw GradleException(
            "Release signing is required. Provide keystore.properties (see keystore.properties.example) " +
                "or set ANDROID_KEYSTORE_PATH, ANDROID_KEYSTORE_PASSWORD, ANDROID_KEY_ALIAS, ANDROID_KEY_PASSWORD.",
        )
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2024.10.01")
    implementation(composeBom)
    androidTestImplementation(composeBom)

    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.tv:tv-foundation:1.0.0")
    implementation("androidx.tv:tv-material:1.0.1")
    implementation("androidx.media3:media3-exoplayer:1.4.1")
    implementation("androidx.media3:media3-ui:1.4.1")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("com.google.code.gson:gson:2.11.0")
    implementation("androidx.security:security-crypto:1.1.0-alpha06")

    testImplementation("junit:junit:4.13.2")
    debugImplementation("androidx.compose.ui:ui-tooling")
}
