package com.gamingcafe.consoletv

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.gamingcafe.consoletv.data.ApiClient
import com.gamingcafe.consoletv.data.ApiException
import com.gamingcafe.consoletv.data.DeviceIdentity
import com.gamingcafe.consoletv.data.ProvisionRequest
import com.gamingcafe.consoletv.data.ProvisionResponse
import com.gamingcafe.consoletv.data.RealtimeClient
import com.gamingcafe.consoletv.data.RealtimeEvent
import com.gamingcafe.consoletv.data.TokenStore
import com.gamingcafe.consoletv.data.TvSessionResponse
import com.gamingcafe.consoletv.domain.AUTO_END_REMAINING_SECONDS
import com.gamingcafe.consoletv.domain.SESSION_CLOCK_TICK_MS
import com.gamingcafe.consoletv.domain.DeductionProfile
import com.gamingcafe.consoletv.domain.SessionClockTicker
import com.gamingcafe.consoletv.hdmi.CecController
import com.gamingcafe.consoletv.media.SessionSounds
import com.google.gson.Gson
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.time.Instant
import java.util.concurrent.atomic.AtomicBoolean

enum class AppPhase {
    LOADING,
    REGISTER,
    KIOSK_HOME,
    SESSION,
}

enum class RegistrationStep {
    CREDENTIALS,
    TOTP,
    DEVICE,
}

data class RegisterForm(
    val username: String = "",
    val password: String = "",
    val totp: String = "",
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
    val registrationStep: RegistrationStep = RegistrationStep.CREDENTIALS,
    val registerForm: RegisterForm = RegisterForm(),
    val adminAuthenticated: Boolean = false,
    val wsConnected: Boolean = false,
    val statusMessage: String = "Starting…",
    val deviceName: String = "",
    val deviceType: String = "",
    val activeSession: ActiveSessionUi? = null,
    val cecDegraded: Boolean = false,
    val isBusy: Boolean = false,
)

class ConsoleTvViewModel(application: Application) : AndroidViewModel(application) {
    private val tokenStore = TokenStore(application)
    private val api = ApiClient()
    private val gson = Gson()
    private val cec = CecController(application)
    private val sounds = SessionSounds(application)

    private val _uiState = MutableStateFlow(ConsoleTvUiState())
    val uiState: StateFlow<ConsoleTvUiState> = _uiState.asStateFlow()

    private var adminToken: String? = null
    private var realtime: RealtimeClient? = null
    private var clock: SessionClockTicker? = null
    private var tickJob: Job? = null
    private val playedReminders = mutableSetOf<Int>()
    private val actionInFlight = AtomicBoolean(false)

    private fun setBusy(busy: Boolean) {
        _uiState.value = _uiState.value.copy(isBusy = busy)
    }

    private inline fun runIfIdle(block: () -> Unit) {
        if (!actionInFlight.compareAndSet(false, true)) return
        setBusy(true)
        try {
            block()
        } catch (_: Exception) {
            actionInFlight.set(false)
            setBusy(false)
        }
    }

    private fun clearBusy() {
        actionInFlight.set(false)
        setBusy(false)
    }

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
                        registrationStep = RegistrationStep.CREDENTIALS,
                        statusMessage = "Sign in as administrator to register this station",
                    )
                return@launch
            }
            enterKioskOrSession(deviceToken)
        }
    }

    private suspend fun enterKioskOrSession(deviceToken: String) {
        cec.discoverPlayStation()
        connectDeviceWs(deviceToken)
        val current =
            withContext(Dispatchers.IO) {
                api.currentTvSession(deviceToken)
            }
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

    fun updateRegisterCredentials(
        username: String? = null,
        password: String? = null,
        totp: String? = null,
    ) {
        val form = _uiState.value.registerForm
        _uiState.value =
            _uiState.value.copy(
                registerForm =
                    form.copy(
                        username = username ?: form.username,
                        password = password ?: form.password,
                        totp = totp ?: form.totp,
                    ),
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

    fun backToCredentials() {
        adminToken = null
        _uiState.value =
            _uiState.value.copy(
                registrationStep = RegistrationStep.CREDENTIALS,
                adminAuthenticated = false,
                registerForm = _uiState.value.registerForm.copy(totp = ""),
                statusMessage = "Sign in as administrator to register this station",
            )
    }

    fun submitAdminLogin() {
        val form = _uiState.value.registerForm
        if (form.username.isBlank() || form.password.isBlank()) {
            _uiState.value = _uiState.value.copy(statusMessage = "Username and password required")
            return
        }
        val totp =
            when (_uiState.value.registrationStep) {
                RegistrationStep.TOTP -> form.totp.trim().ifBlank { null }
                else -> null
            }
        runIfIdle {
            viewModelScope.launch {
                try {
                    val auth =
                        withContext(Dispatchers.IO) {
                            api.loginAdmin(form.username, form.password, totp)
                        }
                    adminToken = auth.accessToken
                    _uiState.value =
                        _uiState.value.copy(
                            registrationStep = RegistrationStep.DEVICE,
                            adminAuthenticated = true,
                            registerForm = form.copy(password = "", totp = ""),
                            statusMessage = "Administrator verified",
                        )
                } catch (error: ApiException) {
                    if (error.message == "TOTP code is required" && totp.isNullOrBlank()) {
                        _uiState.value =
                            _uiState.value.copy(
                                registrationStep = RegistrationStep.TOTP,
                                statusMessage = "",
                            )
                    } else {
                        _uiState.value =
                            _uiState.value.copy(
                                statusMessage = error.message ?: "Login failed",
                            )
                    }
                } catch (error: Exception) {
                    val detail =
                        error.message?.takeIf { it.isNotBlank() }
                            ?: error.javaClass.simpleName
                    _uiState.value =
                        _uiState.value.copy(statusMessage = "Login failed: $detail")
                } finally {
                    clearBusy()
                }
            }
        }
    }

    fun provisionDevice() {
        val token = adminToken
        if (token.isNullOrBlank()) {
            _uiState.value =
                _uiState.value.copy(
                    registrationStep = RegistrationStep.CREDENTIALS,
                    statusMessage = "Sign in as administrator to register this station",
                )
            return
        }
        val form = _uiState.value.registerForm
        if (form.name.isBlank()) {
            _uiState.value = _uiState.value.copy(statusMessage = "Station name required")
            return
        }
        val deviceType = form.deviceType.uppercase()
        if (deviceType !in setOf("PS5", "PS4")) {
            _uiState.value = _uiState.value.copy(statusMessage = "Device type must be PS5 or PS4")
            return
        }
        runIfIdle {
            viewModelScope.launch {
                try {
                    val app = getApplication<Application>()
                    val fingerprint = DeviceIdentity.buildFingerprint(app, tokenStore)
                    val request =
                        ProvisionRequest(
                            fingerprint = fingerprint,
                            name = form.name.trim(),
                            deviceType = deviceType,
                            deviceSubType = form.deviceSubType.uppercase(),
                            location = form.location.ifBlank { null },
                            serialNumber = fingerprint.serial,
                        )
                    val response =
                        withContext(Dispatchers.IO) {
                            provisionWithOrphanRetry(token, request, form.name.trim())
                        }
                    completeProvision(response)
                } catch (error: ApiException) {
                    _uiState.value =
                        _uiState.value.copy(
                            statusMessage = provisionErrorMessage(error, form.name.trim()),
                        )
                } catch (error: Exception) {
                    val detail =
                        error.message?.takeIf { it.isNotBlank() }
                            ?: error.javaClass.simpleName
                    _uiState.value =
                        _uiState.value.copy(statusMessage = "Provision failed: $detail")
                } finally {
                    clearBusy()
                }
            }
        }
    }

    private suspend fun provisionWithOrphanRetry(
        adminToken: String,
        request: ProvisionRequest,
        stationName: String,
    ): ProvisionResponse {
        try {
            return api.provision(adminToken, request)
        } catch (first: ApiException) {
            if (!canCleanupOrphan()) throw first
            cleanupOrphanDevice(adminToken, stationName)
            return api.provision(adminToken, request)
        }
    }

    private fun canCleanupOrphan(): Boolean = tokenStore.deviceToken.isNullOrBlank()

    private fun cleanupOrphanDevice(
        adminToken: String,
        stationName: String,
    ) {
        val matches =
            api.findDeviceByName(adminToken, stationName)
                .filter { it.name.trim() == stationName }
        val orphan = matches.firstOrNull() ?: return
        api.deleteDevice(adminToken, orphan.id)
    }

    private suspend fun completeProvision(response: ProvisionResponse) {
        adminToken = null
        tokenStore.deviceToken = response.accessToken
        tokenStore.deviceId = response.device.id
        tokenStore.deviceName = response.device.name
        tokenStore.deviceType = response.device.deviceType
        try {
            enterKioskOrSession(response.accessToken)
        } catch (_: Exception) {
            _uiState.value =
                _uiState.value.copy(
                    phase = AppPhase.KIOSK_HOME,
                    deviceName = response.device.name,
                    deviceType = response.device.deviceType,
                    statusMessage = "Waiting for session",
                    activeSession = null,
                )
            sounds.prepare()
        }
    }

    private fun provisionErrorMessage(
        error: ApiException,
        stationName: String,
    ): String {
        val message = error.message.orEmpty()
        if (message.contains("already exists", ignoreCase = true)) {
            return "Station name already registered — remove \"$stationName\" in admin or pick a new name"
        }
        return message.ifBlank { "Provision failed" }
    }

    private fun connectDeviceWs(deviceToken: String) {
        val deviceId = tokenStore.deviceId ?: return
        realtime?.disconnect()
        realtime =
            RealtimeClient(
                onEvent = { handleRealtimeEvent(it) },
                onConnectionChanged = { connected ->
                    _uiState.value = _uiState.value.copy(wsConnected = connected)
                },
            ).also { it.connect(deviceToken, listOf("device:$deviceId")) }
    }

    private fun handleRealtimeEvent(event: RealtimeEvent) {
        when (event.eventType) {
            "session.started" -> {
                val sessionId = event.payload?.get("sessionId")?.asString ?: return
                val remaining = event.payload?.get("remainingMinutes")?.asDouble ?: 0.0
                val profile =
                    event.payload?.get("deductionProfile")?.let {
                        gson.fromJson(it, DeductionProfile::class.java)
                    }
                val timezone = event.payload?.get("cafeTimezone")?.asString ?: "UTC"
                val expiryDate = event.payload?.get("expiryDate")?.asString
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
                session.startTime,
                session.remainingMinutes,
                session.timeCreditsConsumed,
                session.deductionProfile,
                session.cafeTimezone,
                session.expiryDate,
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
                    delay(SESSION_CLOCK_TICK_MS)
                    val ticker = clock ?: break
                    val remaining = ticker.remainingNow()
                    updateRemaining(remaining)
                    maybePlayReminder(remaining.toInt())
                    if (remaining * 60 <= AUTO_END_REMAINING_SECONDS) {
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
                withContext(Dispatchers.IO) {
                    api.endTvSession(deviceToken, sessionId, "auto")
                }
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
