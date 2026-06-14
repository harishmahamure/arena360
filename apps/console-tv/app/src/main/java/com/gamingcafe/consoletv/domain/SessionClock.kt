package com.gamingcafe.consoletv.domain

import kotlin.math.max

fun interpolateRemainingMinutes(
    authoritativeMinutes: Double,
    syncedAtMs: Long,
    nowMs: Long,
    deductionProfile: DeductionProfile? = null,
    cafeTimezone: String? = null,
): Double {
    val elapsedWallMinutes = (nowMs - syncedAtMs) / 60_000.0
    val ratio =
        if (deductionProfile != null && cafeTimezone != null) {
            currentDeductionRatio(deductionProfile, cafeTimezone)
        } else {
            1.0
        }
    return max(0.0, authoritativeMinutes - elapsedWallMinutes * ratio)
}

class SessionClockTicker(
    private var anchorMinutes: Double,
    private var syncedAtMs: Long,
    private var deductionProfile: DeductionProfile?,
    private var cafeTimezone: String,
) {
    fun reanchor(minutes: Double, atMs: Long = System.currentTimeMillis()) {
        anchorMinutes = minutes
        syncedAtMs = atMs
    }

    fun updateProfile(profile: DeductionProfile?, timezone: String) {
        deductionProfile = profile
        cafeTimezone = timezone
    }

    fun remainingNow(nowMs: Long = System.currentTimeMillis()): Double =
        interpolateRemainingMinutes(
            anchorMinutes,
            syncedAtMs,
            nowMs,
            deductionProfile,
            cafeTimezone,
        )
}
