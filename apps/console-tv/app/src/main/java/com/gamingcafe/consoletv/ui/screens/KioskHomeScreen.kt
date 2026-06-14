package com.gamingcafe.consoletv.ui.screens

import android.view.ViewGroup
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.PlayerView
import com.gamingcafe.consoletv.BuildConfig
import com.gamingcafe.consoletv.RegisterForm

@Composable
fun KioskHomeScreen(
    title: String,
    subtitle: String,
    deviceType: String,
    wsConnected: Boolean,
    showRegisterOverlay: Boolean,
    registerStep: Int,
    registerForm: RegisterForm,
    onDeviceIdChange: (String) -> Unit,
    onRegisterFieldChange: (name: String?, deviceType: String?, deviceSubType: String?, location: String?) -> Unit,
    onBeginPairing: () -> Unit,
    onProvision: () -> Unit,
    isBusy: Boolean = false,
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
                            colors = listOf(Color.Transparent, Color(0xCC000000)),
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
            Text(text = title)
            if (deviceType.isNotBlank()) {
                Text(text = deviceType)
            }
            Text(text = subtitle)
            Text(text = if (wsConnected) "Connected" else "Reconnecting…")

            if (showRegisterOverlay) {
                when (registerStep) {
                    0 -> {
                        OutlinedTextField(
                            value = registerForm.deviceId,
                            onValueChange = onDeviceIdChange,
                            label = { Text("Device ID") },
                            modifier = Modifier.fillMaxWidth(0.6f),
                            enabled = !isBusy,
                        )
                        Button(onClick = onBeginPairing, enabled = !isBusy) {
                            Text(if (isBusy) "Connecting…" else "Connect & wait for SSO")
                        }
                    }
                    1 -> {
                        Text("Waiting for admin SSO via WebSocket…")
                    }
                    else -> {
                        OutlinedTextField(
                            value = registerForm.name,
                            onValueChange = { onRegisterFieldChange(it, null, null, null) },
                            label = { Text("Station name") },
                            modifier = Modifier.fillMaxWidth(0.6f),
                            enabled = !isBusy,
                        )
                        OutlinedTextField(
                            value = registerForm.location,
                            onValueChange = { onRegisterFieldChange(null, null, null, it) },
                            label = { Text("Location") },
                            modifier = Modifier.fillMaxWidth(0.6f),
                            enabled = !isBusy,
                        )
                        Button(onClick = onProvision, enabled = !isBusy) {
                            Text(if (isBusy) "Provisioning…" else "Provision station")
                        }
                    }
                }
            }
        }
    }
}
