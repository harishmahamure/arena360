package com.gamingcafe.consoletv.domain

import java.time.Instant
import java.time.ZoneId
import java.time.ZonedDateTime

data class DeductionProfile(
    val peakWindowStart: String,
    val peakWindowEnd: String,
    val peakRatio: Double,
    val lowWindowStart: String,
    val lowWindowEnd: String,
    val lowRatio: Double,
)

fun parseTimeToMinutes(value: String): Int {
    val parts = value.split(":").map { it.toIntOrNull() ?: 0 }
    val hour = parts.getOrElse(0) { 0 }
    val minute = parts.getOrElse(1) { 0 }
    return hour * 60 + minute
}

fun minuteInWindow(minuteOfDay: Int, start: String, end: String): Boolean {
    val s = parseTimeToMinutes(start)
    val e = parseTimeToMinutes(end)
    if (s == e) return false
    return if (s < e) {
        minuteOfDay in s until e
    } else {
        minuteOfDay >= s || minuteOfDay < e
    }
}

fun ratioAtMinute(minuteOfDay: Int, profile: DeductionProfile): Double {
    if (minuteInWindow(minuteOfDay, profile.peakWindowStart, profile.peakWindowEnd)) {
        return profile.peakRatio
    }
    if (minuteInWindow(minuteOfDay, profile.lowWindowStart, profile.lowWindowEnd)) {
        return profile.lowRatio
    }
    return 1.0
}

fun localMinuteOfDay(instant: Instant, cafeTimezone: String): Int {
    val zoned = ZonedDateTime.ofInstant(instant, ZoneId.of(cafeTimezone))
    return zoned.hour * 60 + zoned.minute
}

fun currentDeductionRatio(
    profile: DeductionProfile,
    cafeTimezone: String,
    now: Instant = Instant.now(),
): Double = ratioAtMinute(localMinuteOfDay(now, cafeTimezone), profile)
