package com.gamingcafe.consoletv.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import com.gamingcafe.consoletv.AppPhase
import com.gamingcafe.consoletv.ConsoleTvUiState
import com.gamingcafe.consoletv.ConsoleTvViewModel
import com.gamingcafe.consoletv.ui.screens.KioskHomeScreen
import com.gamingcafe.consoletv.ui.screens.SessionScreen

@Composable
fun ConsoleTvRoot(
    state: ConsoleTvUiState,
    viewModel: ConsoleTvViewModel,
) {
    Box(modifier = Modifier.fillMaxSize()) {
        when (state.phase) {
            AppPhase.LOADING,
            AppPhase.KIOSK_HOME,
            AppPhase.REGISTER,
            -> {
                KioskHomeScreen(
                    title =
                        when (state.phase) {
                            AppPhase.REGISTER -> "Station registration"
                            else -> state.deviceName.ifBlank { "Arena360 Station" }
                        },
                    subtitle = state.statusMessage,
                    deviceType = state.deviceType,
                    wsConnected = state.wsConnected,
                    showRegisterOverlay = state.phase == AppPhase.REGISTER,
                    registerStep = state.registerStep,
                    registerForm = state.registerForm,
                    onDeviceIdChange = viewModel::updateRegisterDeviceId,
                    onRegisterFieldChange = viewModel::updateRegisterField,
                    onBeginPairing = viewModel::beginRegisterPairing,
                    onProvision = viewModel::provisionDevice,
                    isBusy = state.isBusy,
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
