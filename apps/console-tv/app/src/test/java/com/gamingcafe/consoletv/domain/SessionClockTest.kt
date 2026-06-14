package com.gamingcafe.consoletv.domain

import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.Instant

class SessionClockTest {
    private val peakProfile =
        DeductionProfile(
            peakWindowStart = "00:00",
            peakWindowEnd = "24:00",
            peakRatio = 2.0,
            lowWindowStart = "12:00",
            lowWindowEnd = "12:00",
            lowRatio = 0.5,
        )

    @Test
    fun effectiveRemaining_withoutProfile_decreasesOnePerWallMinute() {
        val start = Instant.parse("2026-06-07T13:00:00Z").toEpochMilli()
        val now = start + 5 * 60_000
        val result =
            effectiveRemainingMinutes(
                "2026-06-07T13:00:00Z",
                30.0,
                0.0,
                null,
                "UTC",
                now,
            )
        assertEquals(25.0, result, 0.001)
    }

    @Test
    fun effectiveRemaining_neverGoesNegative() {
        val start = Instant.parse("2026-06-07T13:00:00Z").toEpochMilli()
        val now = start + 120 * 60_000
        val result =
            effectiveRemainingMinutes(
                "2026-06-07T13:00:00Z",
                1.0,
                0.0,
                null,
                "UTC",
                now,
            )
        assertEquals(0.0, result, 0.001)
    }

    @Test
    fun ticker_reanchor_updatesRemaining() {
        val ticker =
            SessionClockTicker(
                "2026-06-07T13:00:00Z",
                20.0,
                0.0,
                null,
                "UTC",
            )
        val start = Instant.parse("2026-06-07T13:00:00Z").toEpochMilli()
        assertEquals(18.0, ticker.remainingNow(start + 2 * 60_000), 0.001)
        ticker.reanchor(20.0, start + 2 * 60_000)
        assertEquals(19.0, ticker.remainingNow(start + 3 * 60_000), 0.001)
    }

    @Test
    fun peakRatio_doublesConsumption() {
        val start = Instant.parse("2026-06-07T13:00:00Z").toEpochMilli()
        val now = start + 10 * 60_000
        val result =
            effectiveRemainingMinutes(
                "2026-06-07T13:00:00Z",
                60.0,
                0.0,
                peakProfile,
                "UTC",
                now,
            )
        assertEquals(40.0, result, 0.001)
    }

    @Test
    fun weightedMinutesBetween_matchesLowWindowBurn() {
        val profile =
            DeductionProfile(
                peakWindowStart = "18:00:00",
                peakWindowEnd = "23:00:00",
                peakRatio = 1.5,
                lowWindowStart = "07:00:00",
                lowWindowEnd = "11:00:00",
                lowRatio = 0.8,
            )
        val start = Instant.parse("2026-06-07T02:30:00.000Z").toEpochMilli()
        val end = start + 60 * 60_000
        val weighted = weightedMinutesBetween(start, end, profile, "Asia/Kolkata")
        assertEquals(48.0, weighted, 1.0)
    }
}
