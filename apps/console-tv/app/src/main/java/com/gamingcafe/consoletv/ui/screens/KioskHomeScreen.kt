package com.gamingcafe.consoletv.ui.screens

import android.view.ViewGroup
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.PlayerView
import com.gamingcafe.consoletv.BuildConfig
import com.gamingcafe.consoletv.ui.Arena360Colors
import com.gamingcafe.consoletv.ui.BrandHeader

@Composable
fun KioskHomeScreen(
    title: String,
    subtitle: String,
    deviceType: String,
    wsConnected: Boolean,
) {
    val context = LocalContext.current
    val player =
        remember {
            ExoPlayer.Builder(context).build().apply {
                repeatMode = Player.REPEAT_MODE_ALL
                setMediaItem(MediaItem.fromUri(BuildConfig.BACKGROUND_VIDEO_URL))
                prepare()
                playWhenReady = true
            }
        }

    DisposableEffect(Unit) {
        onDispose { player.release() }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        AndroidView(
            factory = { ctx ->
                PlayerView(ctx).apply {
                    layoutParams =
                        ViewGroup.LayoutParams(
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            ViewGroup.LayoutParams.MATCH_PARENT,
                        )
                    useController = false
                    this.player = player
                }
            },
            modifier = Modifier.fillMaxSize(),
        )

        Box(
            modifier =
                Modifier
                    .fillMaxSize()
                    .background(
                        Brush.radialGradient(
                            colors = listOf(Color.Transparent, Color(0xCC0B0F1A)),
                        ),
                    ),
        )

        Column(
            modifier =
                Modifier
                    .align(Alignment.Center)
                    .padding(32.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            BrandHeader(logoHeight = 72.dp)
            Text(
                text = title,
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.Bold,
                color = Arena360Colors.OnSurface,
                textAlign = TextAlign.Center,
            )
            if (deviceType.isNotBlank()) {
                Text(
                    text = deviceType,
                    style = MaterialTheme.typography.bodyLarge,
                    color = Arena360Colors.OnSurfaceVariant,
                    textAlign = TextAlign.Center,
                )
            }
            if (subtitle.isNotBlank()) {
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodyMedium,
                    color = Arena360Colors.Muted,
                    textAlign = TextAlign.Center,
                )
            }
            Text(
                text = if (wsConnected) "Connected" else "Reconnecting…",
                style = MaterialTheme.typography.labelLarge,
                color = if (wsConnected) Arena360Colors.Primary else Arena360Colors.Muted,
                textAlign = TextAlign.Center,
            )
        }
    }
}
