package com.gamingcafe.consoletv.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.gamingcafe.consoletv.R

object Arena360Colors {
    val Background = Color(0xFF0B0F1A)
    val Surface = Color(0xFF0F131E)
    val SurfaceContainer = Color(0xFF1B1F2B)
    val OnSurface = Color(0xFFDFE2F2)
    val OnSurfaceVariant = Color(0xFFE2BFB0)
    val Muted = Color(0xFF94A3B8)
    val Primary = Color(0xFFFF6900)
    val PrimaryHover = Color(0xFFFF8A3D)
    val GlassBackground = Color(0x9E0F131E)
    val GlassBorder = Color(0x14FFFFFF)
}

@Composable
fun Arena360Background(
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    Box(
        modifier =
            modifier
                .fillMaxSize()
                .background(
                    Brush.radialGradient(
                        colors = listOf(Arena360Colors.Surface, Arena360Colors.Background),
                    ),
                ),
        contentAlignment = Alignment.Center,
    ) {
        content()
    }
}

@Composable
fun Arena360GlassCard(
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    Column(
        modifier =
            modifier
                .background(Arena360Colors.GlassBackground, RoundedCornerShape(16.dp))
                .border(1.dp, Arena360Colors.GlassBorder, RoundedCornerShape(16.dp))
                .padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        content()
    }
}

@Composable
fun BrandHeader(
    modifier: Modifier = Modifier,
    logoHeight: Dp = 72.dp,
    title: String? = null,
    subtitle: String? = null,
) {
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Image(
            painter = painterResource(R.drawable.logo_dark),
            contentDescription = "Arena360",
            modifier = Modifier.height(logoHeight),
            contentScale = ContentScale.Fit,
        )
        if (title != null) {
            Text(
                text = title,
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.Bold,
                color = Arena360Colors.OnSurface,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 12.dp),
            )
        }
        if (subtitle != null) {
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodyMedium,
                color = Arena360Colors.OnSurfaceVariant.copy(alpha = 0.8f),
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 4.dp),
            )
        }
    }
}
