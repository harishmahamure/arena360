package com.gamingcafe.consoletv

import android.app.Application
import android.os.Build
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.gamingcafe.consoletv.data.ApiClient
import com.gamingcafe.consoletv.data.DeviceFingerprint
import com.gamingcafe.consoletv.data.ProvisionRequest
import com.gamingcafe.consoletv.data.RealtimeClient
import com.gamingcafe.consoletv.data.RealtimeEvent
import com.gamingcafe.consoletv.data.TokenStore
import com.gamingcafe.consoletv.data.TvSessionResponse
import com.gamingcafe.consoletv.domain.DeductionProfile
import com.gamingcafe.consoletv.domain.SessionClockTicker
import com.gamingcafe.consoletv.hdmi.CecController
import com.gamingcafe.consoletv.media.SessionSounds
import com.google.gson.Gson
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.time.Instant
import java.util.UUID

enum class AppPhase {
    LOADING,
    REGISTER,
    KIOSK_HOME,
    SESSION,
}

data class RegisterForm(
    val deviceId: String = "",
    val name: String = "",
    val deviceType: String = "PS5",
    val deviceSubType: String = "PREMIUM_TV_CONSOLES",
    val location: String = "",
)

data class ActiveSessionUi(
    val sessionId: String,
    val remainingMinutes: Double,
    val playerLabel: String,
)

data class ConsoleTvUiState(
    val phase: AppPhase = AppPhase.LOADING,
    val registerStep: Int = 0,
    val registerForm: RegisterForm = RegisterForm(),
    val wsConnected: Boolean = false,
    val statusMessage: String = "Starting…",
    val deviceName: String = "",
    val deviceType: String = "",
    val activeSession: ActiveSessionUi? = null,
    val cecDegraded: Boolean = false,
    val staffToken: String? = null,
)

class ConsoleTvViewModel(application: Application) : AndroidViewModel(application) {
    private val tokenStore = TokenStore(application)
    private val api = ApiClient()
    private val gson = Gson()
    private val cec = CecController(application)
    private val sounds = SessionSounds(application)

    private val _uiState = MutableStateFlow(ConsoleTvUiState())
    val uiState: StateFlow<ConsoleTvUiState> = _uiState.asStateFlow()

    private var realtime: RealtimeClient? = null
    private var clock: SessionClockTicker? = null
    private var tickJob: Job? = null
    private val playedReminders = mutableSetOf<Int>()

    init {
        bootstrap()
    }

    private fun bootstrap() {
        viewModelScope.launch {
            val deviceToken = tokenStore.deviceToken
            if (deviceToken.isNullOrBlank()) {
                _uiState.value =
                    ConsoleTvUiState(
                        phase = AppPhase.REGISTER,
                        registerStep = 0,
                        statusMessage = "Enter device ID from admin",
                    )
                return@launch
            }
            enterKioskOrSession(deviceToken)
        }
    }

    private suspend fun enterKioskOrSession(deviceToken: String) {
        cec.discoverPlayStation()
        connectDeviceWs(deviceToken)
        val current = api.currentTvSession(deviceToken)
        if (current != null) {
            startSessionUi(current)
            cec.switchToConsole()
        } else {
            _uiState.value =
                _uiState.value.copy(
                    phase = AppPhase.KIOSK_HOME,
                    deviceName = tokenStore.deviceName.orEmpty(),
                    deviceType = tokenStore.deviceType.orEmpty(),
                    statusMessage = "Waiting for session",
                    activeSession = null,
                )
        }
        sounds.prepare()
    }

    fun updateRegisterDeviceId(deviceId: String) {
        _uiState.value =
            _uiState.value.copy(
                registerForm = _uiState.value.registerForm.copy(deviceId = deviceId.trim()),
            )
    }

    fun updateRegisterField(
        name: String? = null,
        deviceType: String? = null,
        deviceSubType: String? = null,
        location: String? = null,
    ) {
        val form = _uiState.value.registerForm
        _uiState.value =
            _uiState.value.copy(
                registerForm =
                    form.copy(
                        name = name ?: form.name,
                        deviceType = deviceType ?: form.deviceType,
                        deviceSubType = deviceSubType ?: form.deviceSubType,
                        location = location ?: form.location,
                    ),
            )
    }

    fun beginRegisterPairing() {
        val deviceId = _uiState.value.registerForm.deviceId
        if (deviceId.isBlank()) {
            _uiState.value = _uiState.value.copy(statusMessage = "Device ID required")
            return
        }
        viewModelScope.launch {
            try {
                val pairing = api.devicePairing(deviceId)
                connectPairingWs(pairing.accessToken, "device:$deviceId")
                _uiState.value =
                    _uiState.value.copy(
                        registerStep = 1,
                        statusMessage = "Waiting for admin to send TV login…",
                    )
            } catch (error: Exception) {
                _uiState.value =
                    _uiState.value.copy(statusMessage = "Pairing failed: ${error.message}")
            }
        }
    }

    fun provisionDevice() {
        val staffToken = _uiState.value.staffToken ?: return
        val form = _uiState.value.registerForm
        viewModelScope.launch {
            try {
                val fingerprint =
                    DeviceFingerprint(
                        mac = "android-tv",
                        serial = Build.SERIAL.ifBlank { UUID.randomUUID().toString() },
                        biosUuid = UUID.randomUUID().toString(),
                        platform = "AndroidTV",
                        collectedAt = Instant.now().toString(),
                    )
                val response =
                    api.provision(
                        staffToken,
                        ProvisionRequest(
                            fingerprint = fingerprint,
                            name = form.name,
                            deviceType = form.deviceType,
                            deviceSubType = form.deviceSubType,
                            location = form.location.ifBlank { null },
                            serialNumber = fingerprint.serial,
                        ),
                    )
                tokenStore.deviceToken = response.accessToken
                tokenStore.deviceId = response.device.id
                tokenStore.deviceName = response.device.name
                tokenStore.deviceType = response.device.deviceType
                _uiState.value = _uiState.value.copy(staffToken = null)
                enterKioskOrSession(response.accessToken)
            } catch (error: Exception) {
                _uiState.value =
                    _uiState.value.copy(statusMessage = "Provision failed: ${error.message}")
            }
        }
    }

    fun handleDeepLinkSsoToken(token: String) {
        viewModelScope.launch {
            try {
                val auth = api.redeemSsoToken(token)
                onStaffAuthenticated(auth.accessToken)
            } catch (error: Exception) {
                _uiState.value =
                    _uiState.value.copy(statusMessage = "SSO redeem failed: ${error.message}")
            }
        }
    }

    private fun onStaffAuthenticated(staffToken: String) {
        _uiState.value =
            _uiState.value.copy(
                staffToken = staffToken,
                registerStep = 2,
                statusMessage = "Complete device details",
            )
    }

    private fun connectPairingWs(token: String, channel: String) {
        realtime?.disconnect()
        realtime =
            RealtimeClient(
                onEvent = { handleRealtimeEvent(it, pairingMode = true) },
                onConnectionChanged = { connected ->
                    _uiState.value = _uiState.value.copy(wsConnected = connected)
                },
            ).also { it.connect(token, listOf(channel)) }
    }

    private fun connectDeviceWs(deviceToken: String) {
        val deviceId = tokenStore.deviceId ?: return
        realtime?.disconnect()
        realtime =
            RealtimeClient(
                onEvent = { handleRealtimeEvent(it, pairingMode = false) },
                onConnectionChanged = { connected ->
                    _uiState.value = _uiState.value.copy(wsConnected = connected)
                },
            ).also { it.connect(deviceToken, listOf("device:$deviceId")) }
    }

    private fun handleRealtimeEvent(event: RealtimeEvent, pairingMode: Boolean) {
        when (event.eventType) {
            "sso.token.created" -> {
                val token = event.payload?.get("token")?.asString ?: return
                if (pairingMode) {
                    viewModelScope.launch {
                        try {
                            val auth = api.redeemSsoToken(token)
                            onStaffAuthenticated(auth.accessToken)
                        } catch (error: Exception) {
                            _uiState.value =
                                _uiState.value.copy(
                                    statusMessage = "SSO redeem failed: ${error.message}",
                                )
                        }
                    }
                }
            }
            "session.started" -> {
                val sessionId = event.payload?.get("sessionId")?.asString ?: return
                val remaining = event.payload?.get("remainingMinutes")?.asDouble ?: 0.0
                val profile =
                    event.payload?.get("deductionProfile")?.let {
                        gson.fromJson(it, DeductionProfile::class.java)
                    }
                val timezone = event.payload?.get("cafeTimezone")?.asString ?: "UTC"
                val playerId = event.payload?.get("playerId")?.asString ?: "Player"
                startSessionUi(
                    TvSessionResponse(
                        sessionId = sessionId,
                        remainingMinutes = remaining,
                        startTime = Instant.now().toString(),
                        deductionProfile = profile,
                        cafeTimezone = timezone,
                        playerUsername = playerId,
                    ),
                )
                val switched = cec.switchToConsole()
                _uiState.value = _uiState.value.copy(cecDegraded = !switched)
            }
            "balance.updated" -> {
                val sessionId = event.payload?.get("sessionId")?.asString
                val remaining = event.payload?.get("remainingMinutes")?.asDouble ?: return
                if (sessionId != null && sessionId == _uiState.value.activeSession?.sessionId) {
                    clock?.reanchor(remaining)
                    updateRemaining(remaining)
                }
            }
            "session.ended" -> endSessionLocally()
            "device.status_changed" -> {
                val status = event.payload?.get("status")?.asString.orEmpty()
                if (status == "under_maintenance") {
                    _uiState.value =
                        _uiState.value.copy(statusMessage = "Station under maintenance")
                }
            }
        }
    }

    private fun startSessionUi(session: TvSessionResponse) {
        playedReminders.clear()
        clock =
            SessionClockTicker(
                session.remainingMinutes,
                System.currentTimeMillis(),
                session.deductionProfile,
                session.cafeTimezone,
            )
        _uiState.value =
            _uiState.value.copy(
                phase = AppPhase.SESSION,
                activeSession =
                    ActiveSessionUi(
                        sessionId = session.sessionId,
                        remainingMinutes = session.remainingMinutes,
                        playerLabel = session.playerUsername ?: "Player",
                    ),
                statusMessage = "Session active",
            )
        startTickLoop(session.sessionId)
    }

    private fun startTickLoop(sessionId: String) {
        tickJob?.cancel()
        tickJob =
            viewModelScope.launch {
                while (true) {
                    delay(1_000)
                    val ticker = clock ?: break
                    val remaining = ticker.remainingNow()
                    updateRemaining(remaining)
                    maybePlayReminder(remaining.toInt())
                    if (remaining <= 0.0) {
                        autoEndSession(sessionId)
                        break
                    }
                }
            }
    }

    private fun maybePlayReminder(remainingMinutes: Int) {
        listOf(10, 5, 2).forEach { threshold ->
            if (remainingMinutes == threshold && playedReminders.add(threshold)) {
                sounds.play(threshold)
                _uiState.value =
                    _uiState.value.copy(statusMessage = "$threshold minutes remaining")
            }
        }
        if (remainingMinutes == 1 && playedReminders.add(1)) {
            _uiState.value = _uiState.value.copy(statusMessage = "1 minute remaining")
        }
    }

    private fun updateRemaining(remaining: Double) {
        val current = _uiState.value.activeSession ?: return
        _uiState.value =
            _uiState.value.copy(
                activeSession = current.copy(remainingMinutes = remaining),
            )
    }

    private fun autoEndSession(sessionId: String) {
        val deviceToken = tokenStore.deviceToken ?: return
        viewModelScope.launch {
            try {
                api.endTvSession(deviceToken, sessionId, "auto")
            } catch (_: Exception) {
                // Queue intent best-effort; session.ended WS is authoritative.
            }
        }
    }

    private fun endSessionLocally() {
        tickJob?.cancel()
        clock = null
        cec.switchToTv()
        _uiState.value =
            _uiState.value.copy(
                phase = AppPhase.KIOSK_HOME,
                activeSession = null,
                statusMessage = "Waiting for session",
            )
    }

    override fun onCleared() {
        realtime?.disconnect()
        sounds.release()
        super.onCleared()
    }
}
