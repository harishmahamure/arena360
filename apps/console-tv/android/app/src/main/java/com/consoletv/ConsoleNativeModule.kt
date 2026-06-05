package com.consoletv

import android.media.AudioManager
import android.media.ToneGenerator
import android.os.Build
import android.provider.Settings
import android.view.WindowManager
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.bridge.WritableNativeMap
import java.time.Instant

class ConsoleNativeModule(private val reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

  override fun getName(): String = "ConsoleNative"

  @ReactMethod
  fun getFingerprint(promise: Promise) {
    try {
      val androidId =
          Settings.Secure.getString(reactContext.contentResolver, Settings.Secure.ANDROID_ID)
              ?: ""
      val map = WritableNativeMap()
      map.putString("mac", "N/A")
      map.putString("serial", "N/A")
      map.putString("biosUuid", "N/A")
      map.putString("platform", "android_tv")
      map.putString("androidId", androidId)
      map.putString("manufacturer", Build.MANUFACTURER)
      map.putString("model", Build.MODEL)
      map.putString("collectedAt", Instant.now().toString())
      promise.resolve(map)
    } catch (e: Exception) {
      promise.reject("FINGERPRINT_ERROR", e)
    }
  }

  @ReactMethod
  fun playReminder(which: String) {
    val durationMs =
        when (which) {
          "ten" -> 800
          "five" -> 600
          else -> 400
        }
    try {
      ToneGenerator(AudioManager.STREAM_NOTIFICATION, 80).use { tone ->
        tone.startTone(ToneGenerator.TONE_PROP_BEEP, durationMs)
      }
    } catch (_: Exception) {
      // Best-effort audio on TV hardware
    }
  }

  @ReactMethod
  fun setKeepScreenOn(enabled: Boolean) {
    val activity = reactContext.currentActivity ?: return
    activity.runOnUiThread {
      if (enabled) {
        activity.window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
      } else {
        activity.window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
      }
    }
  }
}
