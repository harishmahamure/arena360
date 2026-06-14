package com.gamingcafe.consoletv.media

import android.content.Context
import android.media.MediaPlayer
import com.gamingcafe.consoletv.R

class SessionSounds(context: Context) {
    private val players =
        mapOf(
            10 to MediaPlayer.create(context, R.raw.ten_minutes),
            5 to MediaPlayer.create(context, R.raw.five_minutes),
            2 to MediaPlayer.create(context, R.raw.two_minutes),
        )

    fun prepare() {
        players.values.forEach { player ->
            player.setVolume(0f, 0f)
            player.start()
            player.pause()
            player.seekTo(0)
            player.setVolume(1f, 1f)
        }
    }

    fun play(minutes: Int) {
        players[minutes]?.let { player ->
            player.seekTo(0)
            player.start()
        }
    }

    fun release() {
        players.values.forEach { it.release() }
    }
}
