package com.gamingcafe.consoletv.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

private val DarkColors =
    darkColorScheme(
        primary = Arena360Colors.Primary,
        onPrimary = Arena360Colors.OnSurface,
        primaryContainer = Arena360Colors.PrimaryHover,
        background = Arena360Colors.Background,
        surface = Arena360Colors.Surface,
        surfaceVariant = Arena360Colors.SurfaceContainer,
        onBackground = Arena360Colors.OnSurface,
        onSurface = Arena360Colors.OnSurface,
        onSurfaceVariant = Arena360Colors.OnSurfaceVariant,
        outline = Arena360Colors.GlassBorder,
    )

private val ArenaTypography =
    Typography(
        headlineSmall =
            TextStyle(
                fontFamily = FontFamily.SansSerif,
                fontWeight = FontWeight.Bold,
                fontSize = 24.sp,
                lineHeight = 28.sp,
            ),
        titleLarge =
            TextStyle(
                fontFamily = FontFamily.SansSerif,
                fontWeight = FontWeight.Bold,
                fontSize = 28.sp,
                lineHeight = 32.sp,
            ),
        bodyLarge =
            TextStyle(
                fontFamily = FontFamily.SansSerif,
                fontWeight = FontWeight.Normal,
                fontSize = 16.sp,
                lineHeight = 22.sp,
            ),
        bodyMedium =
            TextStyle(
                fontFamily = FontFamily.SansSerif,
                fontWeight = FontWeight.Normal,
                fontSize = 14.sp,
                lineHeight = 20.sp,
            ),
        labelLarge =
            TextStyle(
                fontFamily = FontFamily.SansSerif,
                fontWeight = FontWeight.SemiBold,
                fontSize = 14.sp,
                lineHeight = 18.sp,
            ),
    )

@Composable
fun ConsoleTvTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = DarkColors,
        typography = ArenaTypography,
        content = content,
    )
}
