package com.gamingcafe.consoletv.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.gamingcafe.consoletv.AppPhase
import com.gamingcafe.consoletv.ConsoleTvUiState
import com.gamingcafe.consoletv.ConsoleTvViewModel
import com.gamingcafe.consoletv.RegistrationStep
import com.gamingcafe.consoletv.ui.screens.KioskHomeScreen
import com.gamingcafe.consoletv.ui.screens.RegistrationScreen
import com.gamingcafe.consoletv.ui.screens.SessionScreen

@Composable
fun ConsoleTvRoot(
    state: ConsoleTvUiState,
    viewModel: ConsoleTvViewModel,
) {
    when (state.phase) {
        AppPhase.LOADING -> {
            Arena360Background {
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    BrandHeader(logoHeight = 64.dp)
                    Text(
                        text = state.statusMessage,
                        color = Arena360Colors.Muted,
                        textAlign = TextAlign.Center,
                    )
                }
            }
        }
        AppPhase.REGISTER -> {
            val (headerTitle, headerSubtitle) =
                when (state.registrationStep) {
                    RegistrationStep.CREDENTIALS ->
                        "Station registration" to
                            "Sign in with an administrator account to register this PlayStation station."
                    RegistrationStep.TOTP ->
                        "Station registration" to
                            "Enter the authenticator code for ${state.registerForm.username}."
                    RegistrationStep.DEVICE ->
                        "Station registration" to
                            if (state.adminAuthenticated) {
                                "Administrator verified. Name this PlayStation station."
                            } else {
                                "Name this PlayStation station."
                            }
                }
            Arena360Background {
                Arena360GlassCard(modifier = Modifier.fillMaxWidth(0.75f)) {
                    BrandHeader(
                        title = headerTitle,
                        subtitle = headerSubtitle,
                    )
                    RegistrationScreen(
                        step = state.registrationStep,
                        form = state.registerForm,
                        statusMessage = state.statusMessage,
                        isBusy = state.isBusy,
                        onUsernameChange = { viewModel.updateRegisterCredentials(username = it) },
                        onPasswordChange = { viewModel.updateRegisterCredentials(password = it) },
                        onTotpChange = { viewModel.updateRegisterCredentials(totp = it) },
                        onRegisterFieldChange = viewModel::updateRegisterField,
                        onSubmitCredentials = viewModel::submitAdminLogin,
                        onSubmitTotp = viewModel::submitAdminLogin,
                        onProvision = viewModel::provisionDevice,
                        onBackToCredentials = viewModel::backToCredentials,
                    )
                }
            }
        }
        AppPhase.KIOSK_HOME -> {
            KioskHomeScreen(
                title = state.deviceName.ifBlank { "Arena360 Station" },
                subtitle = state.statusMessage,
                deviceType = state.deviceType,
                wsConnected = state.wsConnected,
            )
        }
        AppPhase.SESSION -> {
            SessionScreen(
                playerLabel = state.activeSession?.playerLabel.orEmpty(),
                remainingMinutes = state.activeSession?.remainingMinutes ?: 0.0,
                wsConnected = state.wsConnected,
                statusMessage = state.statusMessage,
                cecDegraded = state.cecDegraded,
            )
        }
    }
}
