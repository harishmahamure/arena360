package com.gamingcafe.consoletv.hdmi

import android.content.Context
import android.util.Log

/**
 * HDMI-CEC control via reflection so the app compiles against public SDK stubs.
 * Real TV hardware exposes [android.hardware.hdmi.HdmiControlManager] at runtime.
 */
class CecController(context: Context) {
    private val manager: Any? =
        try {
            context.getSystemService(HDMI_CONTROL_SERVICE)
        } catch (_: Exception) {
            null
        }

    private var discoveredAddress: Int? = null

    fun discoverPlayStation(): Int? {
        val client = invoke(manager, "getPlaybackClient") ?: return null
        val devices = invoke(client, "getConnectedDevices") as? List<*> ?: emptyList<Any>()
        val playStation =
            devices.firstOrNull { info ->
                val deviceType = invoke(info, "getDeviceType") as? Int
                val displayName = invoke(info, "getDisplayName") as? String ?: ""
                deviceType == DEVICE_PLAYBACK &&
                    (
                        displayName.contains("playstation", ignoreCase = true) ||
                            displayName.contains("PS5", ignoreCase = true) ||
                            displayName.contains("PS4", ignoreCase = true)
                    )
            } ?: devices.firstOrNull { info ->
                invoke(info, "getDeviceType") as? Int == DEVICE_PLAYBACK
            }

        discoveredAddress = invoke(playStation, "getLogicalAddress") as? Int
        return discoveredAddress
    }

    fun switchToConsole(): Boolean {
        val address = discoveredAddress ?: discoverPlayStation() ?: return false
        return try {
            val client = invoke(manager, "getPlaybackClient") ?: return false
            invoke(client, "oneTouchPlay", address) as? Boolean ?: false
        } catch (error: Exception) {
            Log.w(TAG, "CEC switchToConsole failed", error)
            false
        }
    }

    fun switchToTv(): Boolean {
        return try {
            val client = invoke(manager, "getPlaybackClient") ?: return false
            invoke(client, "activeSource") as? Boolean ?: false
        } catch (error: Exception) {
            Log.w(TAG, "CEC switchToTv failed", error)
            false
        }
    }

    private fun invoke(
        target: Any?,
        method: String,
        vararg args: Any?,
    ): Any? {
        if (target == null) return null
        return try {
            val parameterTypes =
                args.map { arg ->
                    when (arg) {
                        is Int -> Int::class.javaPrimitiveType!!
                        is Boolean -> Boolean::class.javaPrimitiveType!!
                        null -> Any::class.java
                        else -> arg.javaClass
                    }
                }.toTypedArray()
            target.javaClass.getMethod(method, *parameterTypes).invoke(target, *args)
        } catch (error: Exception) {
            Log.d(TAG, "CEC reflection call failed: $method", error)
            null
        }
    }

    companion object {
        private const val TAG = "CecController"
        private const val HDMI_CONTROL_SERVICE = "hdmi_control"
        private const val DEVICE_PLAYBACK = 4
    }
}
