package com.gamingcafe.consoletv.data

import android.content.Context
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

class TokenStore(context: Context) {
    private val prefs =
        EncryptedSharedPreferences.create(
            context,
            "console_tv_secure",
            MasterKey.Builder(context).setKeyScheme(MasterKey.KeyScheme.AES256_GCM).build(),
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
        )

    var deviceToken: String?
        get() = prefs.getString(KEY_DEVICE, null)
        set(value) {
            prefs.edit().putString(KEY_DEVICE, value).apply()
        }

    var deviceId: String?
        get() = prefs.getString(KEY_DEVICE_ID, null)
        set(value) {
            prefs.edit().putString(KEY_DEVICE_ID, value).apply()
        }

    var deviceName: String?
        get() = prefs.getString(KEY_DEVICE_NAME, null)
        set(value) {
            prefs.edit().putString(KEY_DEVICE_NAME, value).apply()
        }

    var deviceType: String?
        get() = prefs.getString(KEY_DEVICE_TYPE, null)
        set(value) {
            prefs.edit().putString(KEY_DEVICE_TYPE, value).apply()
        }

    fun clearDevice() {
        prefs.edit()
            .remove(KEY_DEVICE)
            .remove(KEY_DEVICE_ID)
            .remove(KEY_DEVICE_NAME)
            .remove(KEY_DEVICE_TYPE)
            .apply()
    }

    companion object {
        private const val KEY_DEVICE = "device_jwt"
        private const val KEY_DEVICE_ID = "device_id"
        private const val KEY_DEVICE_NAME = "device_name"
        private const val KEY_DEVICE_TYPE = "device_type"
    }
}
