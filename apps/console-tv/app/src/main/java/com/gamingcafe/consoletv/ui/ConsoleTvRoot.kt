package com.gamingcafe.consoletv.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.gamingcafe.consoletv.AppPhase
import com.gamingcafe.consoletv.ConsoleTvUiState
import com.gamingcafe.consoletv.ConsoleTvViewModel
import com.gamingcafe.consoletv.ui.screens.KioskHomeScreen
import com.gamingcafe.consoletv.ui.screens.RegistrationScreen
import com.gamingcafe.consoletv.ui.screens.SessionScreen

@Composable
fun ConsoleTvRoot(
    state: ConsoleTvUiState,
    viewModel: ConsoleTvViewModel,
) {
    Box(modifier = Modifier.fillMaxSize()) {
        when (state.phase) {
            AppPhase.LOADING -> {
                Box(
                    modifier =
                        Modifier
                            .fillMaxSize()
                            .background(
                                Brush.radialGradient(
                                    colors = listOf(Color(0xFF1A1A2E), Color(0xFF0F0F1A)),
                                ),
                            ),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(text = state.statusMessage)
                }
            }
            AppPhase.REGISTER -> {
                Box(
                    modifier =
                        Modifier
                            .fillMaxSize()
                            .background(
                                Brush.radialGradient(
                                    colors = listOf(Color(0xFF1A1A2E), Color(0xFF0F0F1A)),
                                ),
                            ),
                    contentAlignment = Alignment.Center,
                ) {
                    Column(
                        modifier = Modifier.padding(32.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Text(text = "Station registration")
                        RegistrationScreen(
                            step = state.registrationStep,
                            form = state.registerForm,
                            statusMessage = state.statusMessage,
                            adminAuthenticated = state.adminAuthenticated,
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
}
