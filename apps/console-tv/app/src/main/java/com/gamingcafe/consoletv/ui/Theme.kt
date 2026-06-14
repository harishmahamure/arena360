package com.gamingcafe.consoletv.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val DarkColors =
    darkColorScheme(
        primary = Color(0xFF7C4DFF),
        background = Color(0xFF0B0B12),
        surface = Color(0xFF151522),
        onBackground = Color.White,
        onSurface = Color.White,
    )

@Composable
fun ConsoleTvTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = DarkColors, content = content)
}
