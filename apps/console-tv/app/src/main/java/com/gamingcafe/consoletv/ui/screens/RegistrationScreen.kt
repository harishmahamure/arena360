package com.gamingcafe.consoletv.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.MenuAnchorType
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.gamingcafe.consoletv.RegisterForm
import com.gamingcafe.consoletv.RegistrationStep

private data class SelectOption(
    val value: String,
    val label: String,
)

private val PS_DEVICE_TYPE_OPTIONS =
    listOf(
        SelectOption("PS5", "PS5"),
        SelectOption("PS4", "PS4"),
    )

private val PS_DEVICE_SUB_TYPE_OPTIONS =
    listOf(
        SelectOption("PREMIUM_TV_CONSOLES", "Premium TV consoles"),
        SelectOption("STANDARD_TV_CONSOLES", "Standard TV consoles"),
    )

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RegistrationDropdownField(
    label: String,
    value: String,
    options: List<SelectOption>,
    enabled: Boolean,
    onValueChange: (String) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    val selectedLabel = options.firstOrNull { it.value == value }?.label ?: value

    ExposedDropdownMenuBox(
        expanded = expanded,
        onExpandedChange = { if (enabled) expanded = !expanded },
        modifier = Modifier.fillMaxWidth(),
    ) {
        OutlinedTextField(
            value = selectedLabel,
            onValueChange = {},
            readOnly = true,
            label = { Text(label) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
            modifier =
                Modifier
                    .menuAnchor(type = MenuAnchorType.PrimaryNotEditable, enabled = enabled)
                    .fillMaxWidth(),
            enabled = enabled,
        )
        ExposedDropdownMenu(
            expanded = expanded,
            onDismissRequest = { expanded = false },
        ) {
            options.forEach { option ->
                DropdownMenuItem(
                    text = { Text(option.label) },
                    onClick = {
                        onValueChange(option.value)
                        expanded = false
                    },
                )
            }
        }
    }
}

@Composable
fun RegistrationScreen(
    step: RegistrationStep,
    form: RegisterForm,
    statusMessage: String,
    adminAuthenticated: Boolean,
    isBusy: Boolean,
    onUsernameChange: (String) -> Unit,
    onPasswordChange: (String) -> Unit,
    onTotpChange: (String) -> Unit,
    onRegisterFieldChange: (name: String?, deviceType: String?, deviceSubType: String?, location: String?) -> Unit,
    onSubmitCredentials: () -> Unit,
    onSubmitTotp: () -> Unit,
    onProvision: () -> Unit,
    onBackToCredentials: () -> Unit,
) {
    Column(
        modifier =
            Modifier
                .fillMaxWidth(0.75f)
                .verticalScroll(rememberScrollState())
                .padding(vertical = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        when (step) {
            RegistrationStep.CREDENTIALS -> {
                Text("Sign in with an administrator account to register this station.")
                OutlinedTextField(
                    value = form.username,
                    onValueChange = onUsernameChange,
                    label = { Text("Admin username") },
                    modifier = Modifier.fillMaxWidth(),
                    enabled = !isBusy,
                    singleLine = true,
                )
                OutlinedTextField(
                    value = form.password,
                    onValueChange = onPasswordChange,
                    label = { Text("Password") },
                    modifier = Modifier.fillMaxWidth(),
                    enabled = !isBusy,
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                )
                if (statusMessage.isNotBlank() && !statusMessage.startsWith("Sign in")) {
                    Text(text = statusMessage)
                }
                Button(onClick = onSubmitCredentials, enabled = !isBusy) {
                    Text(if (isBusy) "Signing in…" else "Continue")
                }
            }
            RegistrationStep.TOTP -> {
                Text("Enter the authenticator code for ${form.username}.")
                OutlinedTextField(
                    value = form.totp,
                    onValueChange = onTotpChange,
                    label = { Text("Authenticator code") },
                    modifier = Modifier.fillMaxWidth(),
                    enabled = !isBusy,
                    singleLine = true,
                )
                if (statusMessage.isNotBlank()) {
                    Text(text = statusMessage)
                }
                Button(onClick = onSubmitTotp, enabled = !isBusy) {
                    Text(if (isBusy) "Verifying…" else "Verify")
                }
                TextButton(onClick = onBackToCredentials, enabled = !isBusy) {
                    Text("Back")
                }
            }
            RegistrationStep.DEVICE -> {
                Text(
                    if (adminAuthenticated) {
                        "Administrator verified. Name this PlayStation station."
                    } else {
                        "Name this PlayStation station."
                    },
                )
                OutlinedTextField(
                    value = form.name,
                    onValueChange = { onRegisterFieldChange(it, null, null, null) },
                    label = { Text("Station name") },
                    modifier = Modifier.fillMaxWidth(),
                    enabled = !isBusy,
                    singleLine = true,
                )
                RegistrationDropdownField(
                    label = "Device type",
                    value = form.deviceType,
                    options = PS_DEVICE_TYPE_OPTIONS,
                    enabled = !isBusy,
                    onValueChange = { onRegisterFieldChange(null, it, null, null) },
                )
                RegistrationDropdownField(
                    label = "Device sub-type",
                    value = form.deviceSubType,
                    options = PS_DEVICE_SUB_TYPE_OPTIONS,
                    enabled = !isBusy,
                    onValueChange = { onRegisterFieldChange(null, null, it, null) },
                )
                OutlinedTextField(
                    value = form.location,
                    onValueChange = { onRegisterFieldChange(null, null, null, it) },
                    label = { Text("Location (optional)") },
                    modifier = Modifier.fillMaxWidth(),
                    enabled = !isBusy,
                    singleLine = true,
                )
                if (statusMessage.isNotBlank() && !statusMessage.startsWith("Administrator")) {
                    Text(text = statusMessage)
                }
                Button(
                    onClick = onProvision,
                    enabled = !isBusy && form.name.isNotBlank(),
                ) {
                    Text(if (isBusy) "Registering…" else "Register device")
                }
            }
        }
    }
}
