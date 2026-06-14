package com.gamingcafe.consoletv.domain

import java.time.Instant
import kotlin.math.max

const val AUTO_END_REMAINING_SECONDS = 10

/** Local session countdown tick interval (matches kiosk/admin). */
const val SESSION_CLOCK_TICK_MS = 1_000L

fun weightedMinutesBetween(
    startMs: Long,
    endMs: Long,
    profile: DeductionProfile,
    cafeTimezone: String,
): Double {
    if (endMs <= startMs) return 0.0
    var total = 0.0
    var cursor = startMs
    while (cursor < endMs) {
        val ratio = currentDeductionRatio(profile, cafeTimezone, Instant.ofEpochMilli(cursor))
        val nextMinute = cursor + 60_000
        val segmentEnd = minOf(nextMinute, endMs)
        val secs = (segmentEnd - cursor) / 1000.0
        total += (secs / 60.0) * ratio
        cursor = segmentEnd
    }
    return total
}

fun effectiveRemainingMinutes(
    sessionStartTime: String,
    walletBalanceMinutes: Double,
    timeCreditsConsumed: Double,
    deductionProfile: DeductionProfile?,
    cafeTimezone: String,
    nowMs: Long = System.currentTimeMillis(),
): Double {
    val startMs = Instant.parse(sessionStartTime).toEpochMilli()
    val consumed =
        if (deductionProfile != null) {
            weightedMinutesBetween(startMs, nowMs, deductionProfile, cafeTimezone)
        } else {
            (nowMs - startMs) / 60_000.0
        }
    val owed = max(0.0, consumed - timeCreditsConsumed)
    return max(0.0, walletBalanceMinutes - owed)
}

/** Floor minutes from now until plan expiry (0 when already expired). */
fun minutesUntilExpiry(expiryDateIso: String?, nowMs: Long = System.currentTimeMillis()): Double {
    if (expiryDateIso.isNullOrBlank()) return Double.MAX_VALUE
    val expiryMs =
        try {
            Instant.parse(expiryDateIso).toEpochMilli()
        } catch (_: Exception) {
            return 0.0
        }
    return max(0.0, kotlin.math.floor((expiryMs - nowMs) / 60_000.0))
}

/** Cap wallet-based remaining by minutes until plan expiry. */
fun capRemainingByExpiry(
    walletRemainingMinutes: Double,
    expiryDateIso: String?,
    nowMs: Long = System.currentTimeMillis(),
): Double {
    if (expiryDateIso.isNullOrBlank()) return walletRemainingMinutes
    return minOf(walletRemainingMinutes, minutesUntilExpiry(expiryDateIso, nowMs))
}

fun walletBalanceFromEffectiveRemaining(
    sessionStartTime: String,
    effectiveRemainingMinutes: Double,
    timeCreditsConsumed: Double,
    deductionProfile: DeductionProfile?,
    cafeTimezone: String,
    syncedAtMs: Long = System.currentTimeMillis(),
): Double {
    val startMs = Instant.parse(sessionStartTime).toEpochMilli()
    val consumed =
        if (deductionProfile != null) {
            weightedMinutesBetween(startMs, syncedAtMs, deductionProfile, cafeTimezone)
        } else {
            (syncedAtMs - startMs) / 60_000.0
        }
    val owed = max(0.0, consumed - timeCreditsConsumed)
    return effectiveRemainingMinutes + owed
}

class SessionClockTicker(
    private val sessionStartTime: String,
    private var walletBalanceMinutes: Double,
    private var timeCreditsConsumed: Double,
    private var deductionProfile: DeductionProfile?,
    private var cafeTimezone: String,
    private var expiryDateIso: String? = null,
) {
    fun reanchor(effectiveRemainingMinutes: Double, atMs: Long = System.currentTimeMillis()) {
        walletBalanceMinutes =
            walletBalanceFromEffectiveRemaining(
                sessionStartTime,
                effectiveRemainingMinutes,
                timeCreditsConsumed,
                deductionProfile,
                cafeTimezone,
                atMs,
            )
    }

    fun updateProfile(profile: DeductionProfile?, timezone: String) {
        deductionProfile = profile
        cafeTimezone = timezone
    }

    fun remainingNow(nowMs: Long = System.currentTimeMillis()): Double =
        capRemainingByExpiry(
            effectiveRemainingMinutes(
                sessionStartTime,
                walletBalanceMinutes,
                timeCreditsConsumed,
                deductionProfile,
                cafeTimezone,
                nowMs,
            ),
            expiryDateIso,
            nowMs,
        )
}
