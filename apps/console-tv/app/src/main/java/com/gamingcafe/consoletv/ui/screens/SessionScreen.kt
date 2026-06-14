package com.gamingcafe.consoletv.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
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
    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .padding(48.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(text = "Session active")
        Text(text = playerLabel)
        Text(text = "$displayMinutes min remaining")
        Text(text = if (wsConnected) "Live" else "Offline — local clock")
        if (statusMessage.isNotBlank()) {
            Text(text = statusMessage)
        }
        if (cecDegraded) {
            Text(text = "CEC unavailable — switch HDMI manually")
        }
    }
}
