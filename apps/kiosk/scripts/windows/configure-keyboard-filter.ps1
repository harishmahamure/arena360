#Requires -Version 5.1
#Requires -RunAsAdministrator
<#
.SYNOPSIS
  Suppress Windows escape keys, including Ctrl+Alt+Delete, at the OS layer.
.DESCRIPTION
  Requires Windows Enterprise, Education, or IoT Enterprise Keyboard Filter.
  Enable the optional feature with -EnableFeature, reboot, then run again.
  Saves prior rules/settings and restores them with -Uninstall. Alt+Tab and
  application shortcuts stay available. This OS configuration is independent
  of the app's staff-login relaxation; use a separate Windows admin account.
#>
[CmdletBinding()]
param([switch]$EnableFeature, [switch]$Uninstall)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Namespace = 'root\standardcimv2\embedded'
$MarkerDir = Join-Path $env:ProgramData 'Arena360'
$MarkerPath = Join-Path $MarkerDir 'keyboard-filter.json'

if ($Uninstall) {
    if (-not (Test-Path -LiteralPath $MarkerPath)) { exit 0 }
    $original = Get-Content -LiteralPath $MarkerPath -Raw | ConvertFrom-Json
    foreach ($rule in $original.rules) {
        Get-CimInstance -Namespace $Namespace -ClassName WEKF_PredefinedKey |
            Where-Object { $_.Id -eq $rule.id } |
            Set-CimInstance -Property @{ Enabled = [bool]$rule.enabled } | Out-Null
    }
    foreach ($setting in $original.settings) {
        Get-CimInstance -Namespace $Namespace -ClassName WEKF_Settings |
            Where-Object { $_.Name -eq $setting.name } |
            Set-CimInstance -Property @{ Value = [string]$setting.value } | Out-Null
    }
    Remove-Item -LiteralPath $MarkerPath -Force
    Write-Host '[Arena360] Restored prior Keyboard Filter configuration.'
    exit 0
}

try {
    $feature = Get-WindowsOptionalFeature -Online -FeatureName Client-KeyboardFilter
} catch {
    Write-Warning '[Arena360] Keyboard Filter is unavailable on this Windows edition. Ctrl+Alt+Delete cannot be blocked by the kiosk app. Use Enterprise, Education, or IoT Enterprise for full suppression.'
    exit 2
}
if ($feature.State -ne 'Enabled') {
    if ($EnableFeature) {
        Enable-WindowsOptionalFeature -Online -FeatureName Client-KeyboardFilter -All -NoRestart | Out-Null
        Write-Warning '[Arena360] Keyboard Filter feature requested. Reboot, then run configure-keyboard-filter.ps1 again to apply blocking rules.'
    } else {
        Write-Warning '[Arena360] Keyboard Filter is not enabled. Run this script with -EnableFeature, reboot, and run it again. Ctrl+Alt+Delete remains available until then.'
    }
    exit 2
}

# The app's native hook handles other shell shortcuts so staff login can
# relax those restrictions without needing Windows elevation.
$block = @('Ctrl+Alt+Del')
$rules = @(Get-CimInstance -Namespace $Namespace -ClassName WEKF_PredefinedKey |
    Where-Object { $_.Id -in $block -or $_.Id -eq 'Alt+Tab' })
foreach ($id in $block + @('Alt+Tab')) {
    if (-not ($rules | Where-Object { $_.Id -eq $id })) {
        throw "Required Keyboard Filter rule '$id' was not found."
    }
}
# Keep separate Windows administrator accounts available for station servicing.
# Disable the default five-Windows-key breakout for the standard kiosk account.
$values = @{ DisableKeyboardFilterForAdministrators = 'true'; ForceOffAccessibility = 'true'; BreakoutKeyScanCode = '0' }
$settings = @(Get-CimInstance -Namespace $Namespace -ClassName WEKF_Settings |
    Where-Object { $values.ContainsKey($_.Name) })
if (-not (Test-Path -LiteralPath $MarkerPath)) {
    New-Item -ItemType Directory -Path $MarkerDir -Force | Out-Null
    @{
        rules = @($rules | ForEach-Object { @{ id = $_.Id; enabled = $_.Enabled } })
        settings = @($settings | ForEach-Object { @{ name = $_.Name; value = $_.Value } })
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $MarkerPath -Encoding UTF8
}
foreach ($rule in $rules) {
    $rule | Set-CimInstance -Property @{ Enabled = ($rule.Id -ne 'Alt+Tab') } | Out-Null
}
foreach ($setting in $settings) {
    $setting | Set-CimInstance -Property @{ Value = $values[$setting.Name] } | Out-Null
}
Write-Host '[Arena360] Windows escape keys blocked; Alt+Tab allowed. Restart to apply breakout-key changes. Windows administrators are exempt; run the kiosk under a standard Windows account.'
