plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

fun loadEnvFile(file: java.io.File): Map<String, String> {
    if (!file.exists()) return emptyMap()
    return file.readLines()
        .filter { it.isNotBlank() && !it.startsWith("#") && it.contains("=") }
        .associate { line ->
            val (key, value) = line.split("=", limit = 2)
            key.trim() to value.trim()
        }
}

val envLocal = loadEnvFile(rootProject.file(".env.local"))
val apiBaseUrl =
    envLocal["VITE_API_URL"]
        ?: envLocal["API_BASE_URL"]
        ?: "http://10.0.2.2:3000"
val gatewayUrl =
    envLocal["VITE_GATEWAY_URL"]
        ?: envLocal["GATEWAY_URL"]
        ?: run {
            val api = apiBaseUrl.trimEnd('/')
            val wsProtocol = if (api.startsWith("https")) "wss" else "ws"
            val host = api.removePrefix("https://").removePrefix("http://")
            "$wsProtocol://$host/realtime"
        }

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
