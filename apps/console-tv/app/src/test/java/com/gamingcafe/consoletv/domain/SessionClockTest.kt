package com.gamingcafe.consoletv.domain

import org.junit.Assert.assertEquals
import org.junit.Test

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
    fun interpolate_withoutProfile_decreasesOnePerWallMinute() {
        val synced = 1_000_000L
        val result = interpolateRemainingMinutes(30.0, synced, synced + 5 * 60_000)
        assertEquals(25.0, result, 0.001)
    }

    @Test
    fun interpolate_neverGoesNegative() {
        val synced = 0L
        val result = interpolateRemainingMinutes(1.0, synced, synced + 120 * 60_000)
        assertEquals(0.0, result, 0.001)
    }

    @Test
    fun ticker_reanchor_updatesRemaining() {
        val ticker = SessionClockTicker(20.0, 0L, null, "UTC")
        assertEquals(18.0, ticker.remainingNow(2 * 60_000), 0.001)
        ticker.reanchor(40.0, 2 * 60_000)
        assertEquals(39.0, ticker.remainingNow(3 * 60_000), 0.001)
    }

    @Test
    fun peakRatio_doublesConsumption() {
        val synced = 0L
        val result =
            interpolateRemainingMinutes(
                60.0,
                synced,
                synced + 10 * 60_000,
                peakProfile,
                "UTC",
            )
        assertEquals(40.0, result, 0.001)
    }
}
