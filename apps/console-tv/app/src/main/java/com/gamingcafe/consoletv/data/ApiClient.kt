package com.gamingcafe.consoletv.data

import com.gamingcafe.consoletv.BuildConfig
import com.gamingcafe.consoletv.domain.DeductionProfile
import com.google.gson.Gson
import com.google.gson.JsonObject
import com.google.gson.annotations.SerializedName
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.util.concurrent.TimeUnit

class ApiException(message: String) : Exception(message)

data class AuthResponse(
    @SerializedName("accessToken") val accessToken: String,
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

data class DeviceListItem(
    val id: String,
    val name: String,
)

data class TvSessionResponse(
    @SerializedName("sessionId") val sessionId: String,
    @SerializedName("remainingMinutes") val remainingMinutes: Double,
    @SerializedName("startTime") val startTime: String,
    @SerializedName("deductionProfile") val deductionProfile: DeductionProfile?,
    @SerializedName("cafeTimezone") val cafeTimezone: String,
    @SerializedName("timeCreditsConsumed") val timeCreditsConsumed: Double = 0.0,
    @SerializedName("playerUsername") val playerUsername: String?,
    @SerializedName("expiryDate") val expiryDate: String? = null,
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

    private fun parseErrorMessage(body: String): String? {
        if (body.isBlank()) return null
        return runCatching {
            val root = gson.fromJson(body, JsonObject::class.java)
            val messageEl = root.get("message")
            if (messageEl != null && messageEl.isJsonPrimitive) {
                messageEl.asString
            } else {
                null
            }
        }.getOrNull()
    }

    private fun requireDataObject(body: String): JsonObject {
        if (body.isBlank()) throw ApiException("Empty response from server")
        val root = gson.fromJson(body, JsonObject::class.java)
        val data = root.get("data")
        if (data == null || data.isJsonNull || !data.isJsonObject) {
            throw ApiException(parseErrorMessage(body) ?: "Invalid response from server")
        }
        return data.asJsonObject
    }

    private fun optionalDataObject(body: String): JsonObject? {
        if (body.isBlank()) return null
        val root = gson.fromJson(body, JsonObject::class.java)
        val data = root.get("data") ?: return null
        if (data.isJsonNull || !data.isJsonObject) return null
        return data.asJsonObject
    }

    private inline fun <reified T> parseEnvelope(body: String): T {
        val data = requireDataObject(body)
        return gson.fromJson(data, T::class.java)
    }

    private fun parseProvisionResponse(body: String): ProvisionResponse {
        val data = requireDataObject(body)
        val accessToken =
            data.get("accessToken")?.takeIf { it.isJsonPrimitive }?.asString
                ?: throw ApiException("Invalid provision response (missing accessToken)")
        val deviceJson = data.get("device")?.takeIf { it.isJsonObject }?.asJsonObject
            ?: throw ApiException("Invalid provision response (missing device)")
        val deviceId =
            deviceJson.get("id")?.takeIf { it.isJsonPrimitive }?.asString
                ?: throw ApiException("Invalid provision response (missing device id)")
        val deviceName =
            deviceJson.get("name")?.takeIf { it.isJsonPrimitive }?.asString
                ?: throw ApiException("Invalid provision response (missing device name)")
        val deviceType =
            deviceJson.get("deviceType")?.takeIf { it.isJsonPrimitive }?.asString
                ?: ""
        return ProvisionResponse(
            accessToken = accessToken,
            device =
                RegisteredDevice(
                    id = deviceId,
                    name = deviceName,
                    deviceType = deviceType,
                ),
        )
    }

    private fun parseDeviceList(body: String): List<DeviceListItem> {
        val data = requireDataObject(body)
        val page = data.get("data")?.takeIf { it.isJsonArray }?.asJsonArray ?: return emptyList()
        return page.mapNotNull { element ->
            if (!element.isJsonObject) return@mapNotNull null
            val item = element.asJsonObject
            val id = item.get("id")?.takeIf { it.isJsonPrimitive }?.asString ?: return@mapNotNull null
            val name = item.get("name")?.takeIf { it.isJsonPrimitive }?.asString ?: return@mapNotNull null
            DeviceListItem(id = id, name = name)
        }
    }

    private fun ensureSuccess(
        response: Response,
        body: String,
    ) {
        if (response.isSuccessful) return
        throw ApiException(parseErrorMessage(body) ?: "Request failed (${response.code})")
    }

    private fun authorizedGet(
        adminToken: String,
        path: String,
    ): String {
        val request =
            Request.Builder()
                .url("${baseUrl()}$path")
                .addHeader("Authorization", "Bearer $adminToken")
                .get()
                .build()
        return client.newCall(request).execute().use { response ->
            val text = response.body?.string().orEmpty()
            ensureSuccess(response, text)
            text
        }
    }

    private fun authorizedDelete(
        adminToken: String,
        path: String,
    ) {
        val request =
            Request.Builder()
                .url("${baseUrl()}$path")
                .addHeader("Authorization", "Bearer $adminToken")
                .delete()
                .build()
        client.newCall(request).execute().use { response ->
            val text = response.body?.string().orEmpty()
            ensureSuccess(response, text)
        }
    }

    fun loginAdmin(
        username: String,
        password: String,
        totp: String? = null,
    ): AuthResponse {
        val payload =
            gson.toJson(
                buildMap {
                    put("username", username)
                    put("password", password)
                    if (!totp.isNullOrBlank()) put("totp", totp.trim())
                },
            )
        val request =
            Request.Builder()
                .url("${baseUrl()}/auth/login/admin")
                .post(payload.toRequestBody(json))
                .build()
        val body = client.newCall(request).execute().use { response ->
            val text = response.body?.string().orEmpty()
            ensureSuccess(response, text)
            text
        }
        return parseEnvelope(body)
    }

    fun provision(
        adminToken: String,
        requestBody: ProvisionRequest,
    ): ProvisionResponse {
        val payload = gson.toJson(requestBody)
        val request =
            Request.Builder()
                .url("${baseUrl()}/devices/provision")
                .addHeader("Authorization", "Bearer $adminToken")
                .post(payload.toRequestBody(json))
                .build()
        val body = client.newCall(request).execute().use { response ->
            val text = response.body?.string().orEmpty()
            ensureSuccess(response, text)
            text
        }
        return parseProvisionResponse(body)
    }

    fun findDeviceByName(
        adminToken: String,
        name: String,
    ): List<DeviceListItem> {
        val encodedName = URLEncoder.encode(name.trim(), StandardCharsets.UTF_8.toString())
        val body = authorizedGet(adminToken, "/devices?name=$encodedName&limit=5")
        return parseDeviceList(body)
    }

    fun deleteDevice(
        adminToken: String,
        deviceId: String,
    ) {
        authorizedDelete(adminToken, "/devices/$deviceId")
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
        val data = optionalDataObject(body) ?: return null
        return gson.fromJson(data, TvSessionResponse::class.java)
    }

    fun endTvSession(
        deviceToken: String,
        sessionId: String,
        reason: String = "auto",
    ) {
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
