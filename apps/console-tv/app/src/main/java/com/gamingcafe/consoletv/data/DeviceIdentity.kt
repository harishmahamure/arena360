package com.gamingcafe.consoletv.data

import android.content.Context
import android.os.Build
import android.provider.Settings
import java.time.Instant
import java.util.UUID

object DeviceIdentity {
    fun stableSerial(context: Context): String {
        val buildSerial = Build.SERIAL?.trim().orEmpty()
        if (buildSerial.isNotBlank() && !buildSerial.equals("unknown", ignoreCase = true)) {
            return buildSerial
        }
        val androidId =
            Settings.Secure.getString(context.contentResolver, Settings.Secure.ANDROID_ID)
                ?.trim()
                .orEmpty()
        if (androidId.isNotBlank()) {
            return androidId
        }
        return UUID.randomUUID().toString()
    }

    fun stableBiosUuid(tokenStore: TokenStore): String {
        tokenStore.biosUuid?.let { existing ->
            if (existing.isNotBlank()) return existing
        }
        val created = UUID.randomUUID().toString()
        tokenStore.biosUuid = created
        return created
    }

    fun buildFingerprint(
        context: Context,
        tokenStore: TokenStore,
    ): DeviceFingerprint {
        return DeviceFingerprint(
            mac = "android-tv",
            serial = stableSerial(context),
            biosUuid = stableBiosUuid(tokenStore),
            platform = "AndroidTV",
            collectedAt = Instant.now().toString(),
        )
    }
}
