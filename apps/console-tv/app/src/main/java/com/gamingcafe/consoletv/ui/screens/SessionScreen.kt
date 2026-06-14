package com.gamingcafe.consoletv.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.gamingcafe.consoletv.ui.Arena360Background
import com.gamingcafe.consoletv.ui.Arena360Colors
import com.gamingcafe.consoletv.ui.BrandHeader
import kotlin.math.ceil

@Composable
fun SessionScreen(
    playerLabel: String,
    remainingMinutes: Double,
    wsConnected: Boolean,
    statusMessage: String,
    cecDegraded: Boolean,
) {
    val displayMinutes = ceil(remainingMinutes).toInt().coerceAtLeast(0)
    Arena360Background {
        Column(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(48.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            BrandHeader(logoHeight = 56.dp)
            Text(
                text = "Session active",
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.Bold,
                color = Arena360Colors.OnSurface,
                modifier = Modifier.padding(top = 24.dp),
            )
            Text(
                text = playerLabel,
                style = MaterialTheme.typography.titleLarge,
                color = Arena360Colors.OnSurface,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 8.dp),
            )
            Text(
                text = "$displayMinutes min remaining",
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.Bold,
                color = Arena360Colors.Primary,
                modifier = Modifier.padding(top = 16.dp),
            )
            Text(
                text = if (wsConnected) "Live" else "Offline — local clock",
                style = MaterialTheme.typography.bodyMedium,
                color = if (wsConnected) Arena360Colors.Muted else Arena360Colors.OnSurfaceVariant,
                modifier = Modifier.padding(top = 8.dp),
            )
            if (statusMessage.isNotBlank()) {
                Text(
                    text = statusMessage,
                    style = MaterialTheme.typography.bodyMedium,
                    color = Arena360Colors.Muted,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 8.dp),
                )
            }
            if (cecDegraded) {
                Text(
                    text = "CEC unavailable — switch HDMI manually",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Arena360Colors.OnSurfaceVariant,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 8.dp),
                )
            }
        }
    }
}
