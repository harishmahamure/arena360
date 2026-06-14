package com.gamingcafe.consoletv.data

import com.gamingcafe.consoletv.BuildConfig
import com.gamingcafe.consoletv.domain.DeductionProfile
import com.google.gson.Gson
import com.google.gson.annotations.SerializedName
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.util.concurrent.TimeUnit

data class AuthResponse(
    @SerializedName("accessToken") val accessToken: String,
)

data class DevicePairingResponse(
    @SerializedName("accessToken") val accessToken: String,
    @SerializedName("deviceId") val deviceId: String,
)

data class ProvisionRequest(
    val fingerprint: DeviceFingerprint,
    val name: String,
    val deviceType: String,
    val deviceSubType: String,
    val location: String?,
    val serialNumber: String?,
    val provisionClient: String = "console-tv",
)

data class DeviceFingerprint(
    val mac: String,
    val serial: String,
    val biosUuid: String,
    val platform: String,
    val collectedAt: String,
)

data class ProvisionResponse(
    @SerializedName("accessToken") val accessToken: String,
    val device: RegisteredDevice,
)

data class RegisteredDevice(
    val id: String,
    val name: String,
    val deviceType: String,
)

data class TvSessionResponse(
    @SerializedName("sessionId") val sessionId: String,
    @SerializedName("remainingMinutes") val remainingMinutes: Double,
    @SerializedName("startTime") val startTime: String,
    @SerializedName("deductionProfile") val deductionProfile: DeductionProfile?,
    @SerializedName("cafeTimezone") val cafeTimezone: String,
    @SerializedName("playerUsername") val playerUsername: String?,
)

class ApiClient {
    private val gson = Gson()
    private val json = "application/json".toMediaType()
    private val client =
        OkHttpClient.Builder()
            .connectTimeout(15, TimeUnit.SECONDS)
            .readTimeout(30, TimeUnit.SECONDS)
            .build()

    private fun baseUrl(): String = BuildConfig.API_BASE_URL.trimEnd('/')

    private inline fun <reified T> parseEnvelope(body: String): T {
        val root = gson.fromJson(body, com.google.gson.JsonObject::class.java)
        val data = root.get("data") ?: throw IllegalStateException("Missing data envelope")
        return gson.fromJson(data, T::class.java)
    }

    fun devicePairing(deviceId: String): DevicePairingResponse {
        val payload = gson.toJson(mapOf("deviceId" to deviceId))
        val request =
            Request.Builder()
                .url("${baseUrl()}/auth/device-pairing")
                .post(payload.toRequestBody(json))
                .build()
        val body = client.newCall(request).execute().use { it.body?.string().orEmpty() }
        return parseEnvelope(body)
    }

    fun redeemSsoToken(token: String): AuthResponse {
        val payload = gson.toJson(mapOf("token" to token))
        val request =
            Request.Builder()
                .url("${baseUrl()}/auth/sso/redeem")
                .post(payload.toRequestBody(json))
                .build()
        val body = client.newCall(request).execute().use { it.body?.string().orEmpty() }
        return parseEnvelope(body)
    }

    fun provision(staffToken: String, requestBody: ProvisionRequest): ProvisionResponse {
        val payload = gson.toJson(requestBody)
        val request =
            Request.Builder()
                .url("${baseUrl()}/devices/provision")
                .addHeader("Authorization", "Bearer $staffToken")
                .post(payload.toRequestBody(json))
                .build()
        val body = client.newCall(request).execute().use { it.body?.string().orEmpty() }
        return parseEnvelope(body)
    }

    fun currentTvSession(deviceToken: String): TvSessionResponse? {
        val request =
            Request.Builder()
                .url("${baseUrl()}/tv/sessions/current")
                .addHeader("Authorization", "Bearer $deviceToken")
                .get()
                .build()
        val response = client.newCall(request).execute()
        val body = response.body?.string().orEmpty()
        if (!response.isSuccessful) return null
        return parseEnvelope<TvSessionResponse?>(body)
    }

    fun endTvSession(deviceToken: String, sessionId: String, reason: String = "auto") {
        val payload = gson.toJson(mapOf("reason" to reason))
        val request =
            Request.Builder()
                .url("${baseUrl()}/tv/sessions/$sessionId/end")
                .addHeader("Authorization", "Bearer $deviceToken")
                .patch(payload.toRequestBody(json))
                .build()
        client.newCall(request).execute().close()
    }
}
