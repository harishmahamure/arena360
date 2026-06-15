#Requires -Version 5.1
<#
.SYNOPSIS
  Idempotent post-install configuration for Arena360 kiosk stations.

.DESCRIPTION
  Registers the kiosk logon scheduled task, optional HKLM policy hardening, and optional
  auto-logon for a single Windows kiosk user. Invoked from NSIS post-install or manually
  for fleet rollout. Never logs passwords.

  Launches Arena360 at logon via the Arena360 Kiosk scheduled task only. Does not modify
  Winlogon\Shell or replace explorer.exe as the Windows shell.

.PARAMETER Uninstall
  Remove autostart task and registry values recorded in the Arena360 marker file.
#>
[CmdletBinding()]
param(
    [string]$KioskUser = $env:USERNAME,
    [Parameter(Mandatory = $false)]
    [string]$InstallDir,
    [SecureString]$AutoLogonPassword,
    [switch]$SkipAutostart,
    [alias('SkipWatchdog')]
    [switch]$SkipWatchdog,
    [switch]$SkipHardening,
    [switch]$Uninstall,
    [ValidateSet('RunKey', 'ReplaceShell', 'None')]
    [string]$ShellMode = 'None'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$KioskTaskName = 'Arena360 Kiosk'
$LegacyWatchdogTaskName = 'Arena360 Watchdog'
$MarkerDir = Join-Path $env:ProgramData 'Arena360'
$MarkerPath = Join-Path $MarkerDir 'registry-hardening.json'
$WinlogonKey = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon'
$SystemPolicyKey = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
$ExplorerPolicyKey = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\Explorer'

function Write-Info([string]$Message) {
    Write-Host "[Arena360] $Message"
}

function Write-Warn([string]$Message) {
    Write-Warning "[Arena360] $Message"
}

function Ensure-MarkerDir {
    if (-not (Test-Path -LiteralPath $MarkerDir)) {
        New-Item -ItemType Directory -Path $MarkerDir -Force | Out-Null
    }
}

function Read-Marker {
    if (-not (Test-Path -LiteralPath $MarkerPath)) {
        return $null
    }
    try {
        return Get-Content -LiteralPath $MarkerPath -Raw | ConvertFrom-Json
    } catch {
        Write-Warn "Could not parse marker file; uninstall cleanup may be incomplete."
        return $null
    }
}

function Write-Marker($Data) {
    Ensure-MarkerDir
    $Data | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $MarkerPath -Encoding UTF8
}

function Resolve-KioskExe([string]$Dir) {
    if ([string]::IsNullOrWhiteSpace($Dir)) {
        return $null
    }
    $preferred = Join-Path $Dir 'Arena360 Station Management.exe'
    if (Test-Path -LiteralPath $preferred) {
        return $preferred
    }
    $match = Get-ChildItem -LiteralPath $Dir -Filter '*.exe' -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -notlike 'arena360-watchdog*' -and $_.Name -notlike 'uninstall*' } |
        Sort-Object Name |
        Select-Object -First 1
    if ($match) {
        return $match.FullName
    }
    return $null
}

function Remove-LegacyWatchdogTask {
    $null = schtasks /Delete /TN $LegacyWatchdogTaskName /F 2>&1
    Write-Info "Removed legacy watchdog scheduled task (if present)."
}

function Clear-LegacyWatchdogPause {
    $pausePath = Join-Path $MarkerDir 'watchdog.pause'
    if (Test-Path -LiteralPath $pausePath) {
        Remove-Item -LiteralPath $pausePath -Force
        Write-Info 'Removed legacy watchdog.pause file.'
    }
}

function Test-KioskLogonTaskInteractive {
    $query = schtasks /Query /TN $KioskTaskName /V /FO LIST 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Warn "Could not verify $KioskTaskName task: $query"
        return
    }
    $logonLine = $query | Where-Object { $_ -match 'Logon Mode' }
    if ($logonLine -match 'Interactive') {
        Write-Info "Verified $KioskTaskName logon mode is interactive."
    } else {
        Write-Warn "$KioskTaskName may not run in the user desktop session. Logon Mode: $logonLine"
    }
}

function Install-KioskLogonTask([string]$KioskExe, [string]$RunAsUser) {
    if (-not (Test-Path -LiteralPath $KioskExe)) {
        Write-Warn "Kiosk binary not found at $KioskExe; skipping scheduled task."
        return
    }

    if ([string]::IsNullOrWhiteSpace($RunAsUser)) {
        $RunAsUser = $env:USERNAME
    }

    Remove-LegacyWatchdogTask
    Clear-LegacyWatchdogPause

    try {
        Unregister-ScheduledTask -TaskName $KioskTaskName -Confirm:$false -ErrorAction SilentlyContinue
    } catch {
        $null = schtasks /Delete /TN $KioskTaskName /F 2>&1
    }

    Write-Info "Registering $KioskTaskName for user '$RunAsUser' at logon (interactive, 30s delay)."

    $action = New-ScheduledTaskAction -Execute $KioskExe
    $trigger = New-ScheduledTaskTrigger -AtLogOn -User $RunAsUser
    $trigger.Delay = 'PT30S'
    $principal = New-ScheduledTaskPrincipal -UserId $RunAsUser -LogonType Interactive -RunLevel Limited
    $settings = New-ScheduledTaskSettingsSet `
        -AllowStartIfOnBatteries `
        -DontStopIfGoingOnBatteries `
        -ExecutionTimeLimit ([TimeSpan]::Zero)

    try {
        Register-ScheduledTask `
            -TaskName $KioskTaskName `
            -Action $action `
            -Trigger $trigger `
            -Principal $principal `
            -Settings $settings `
            -Force | Out-Null
    } catch {
        Write-Warn "Register-ScheduledTask failed; falling back to schtasks: $_"
        $quoted = "`"$KioskExe`""
        $result = schtasks /Create /TN $KioskTaskName /TR $quoted /SC ONLOGON /RU $RunAsUser /IT /DELAY 0000:30 /RL LIMITED /F 2>&1
        if ($LASTEXITCODE -ne 0) {
            Write-Warn "Could not register kiosk logon task (exit $LASTEXITCODE): $result"
            return
        }
    }

    Write-Info "Registered $KioskTaskName to start at logon."
    Test-KioskLogonTaskInteractive
}

function Remove-KioskLogonTask {
    try {
        Unregister-ScheduledTask -TaskName $KioskTaskName -Confirm:$false -ErrorAction SilentlyContinue
    } catch {
        $null = schtasks /Delete /TN $KioskTaskName /F 2>&1
    }
    Remove-LegacyWatchdogTask
    Write-Info "Removed kiosk autostart scheduled tasks (if present)."
}

function Install-Hardening {
    Ensure-MarkerDir
    $entries = @(
        @{ hive = 'HKLM'; path = 'SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'; name = 'DisableTaskMgr'; value = 1 },
        @{ hive = 'HKLM'; path = 'SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'; name = 'DisableLockWorkstation'; value = 1 },
        @{ hive = 'HKLM'; path = 'SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'; name = 'DisableChangePassword'; value = 1 },
        @{ hive = 'HKLM'; path = 'SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\Explorer'; name = 'NoRun'; value = 1 }
    )

    foreach ($entry in $entries) {
        $regPath = "Registry::$($entry.hive)\$($entry.path)"
        if (-not (Test-Path -LiteralPath $regPath)) {
            New-Item -Path $regPath -Force | Out-Null
        }
        Set-ItemProperty -LiteralPath $regPath -Name $entry.name -Value $entry.value -Type DWord -Force
    }

    $marker = Read-Marker
    if (-not $marker) {
        $marker = [ordered]@{}
    }
    $marker.hardening = $entries
    if (-not $marker.autologon) {
        $marker.autologon = [ordered]@{ configured = $false; user = $null }
    }
    Write-Marker $marker
    Write-Info 'Applied HKLM policy hardening (Task Manager, lock workstation, change password, Run dialog).'
}

function Remove-HardeningFromMarker {
    $marker = Read-Marker
    if (-not $marker -or -not $marker.hardening) {
        return
    }
    foreach ($entry in $marker.hardening) {
        $regPath = "Registry::$($entry.hive)\$($entry.path)"
        if (Test-Path -LiteralPath $regPath) {
            Remove-ItemProperty -LiteralPath $regPath -Name $entry.name -ErrorAction SilentlyContinue
        }
    }
    Write-Info 'Removed installer-managed policy registry values.'
}

function Clear-AutoLogonFromMarker {
    $marker = Read-Marker
    if (-not $marker -or -not $marker.autologon -or -not $marker.autologon.configured) {
        return
    }
    if (Test-Path -LiteralPath $WinlogonKey) {
        Set-ItemProperty -LiteralPath $WinlogonKey -Name 'AutoAdminLogon' -Value '0' -Type String -Force
        Remove-ItemProperty -LiteralPath $WinlogonKey -Name 'DefaultPassword' -ErrorAction SilentlyContinue
        Remove-ItemProperty -LiteralPath $WinlogonKey -Name 'DefaultUserName' -ErrorAction SilentlyContinue
        Remove-ItemProperty -LiteralPath $WinlogonKey -Name 'DefaultDomainName' -ErrorAction SilentlyContinue
    }
    Write-Info 'Cleared auto-logon registry values set by Arena360 installer.'
}

function Install-AutoLogon([string]$User, [SecureString]$Password) {
    if ([string]::IsNullOrWhiteSpace($User)) {
        Write-Warn 'KioskUser is empty; skipping auto-logon.'
        return
    }

    $plain = $null
    try {
        $plain = [Runtime.InteropServices.Marshal]::PtrToStringAuto(
            [Runtime.InteropServices.Marshal]::SecureStringToBSTR($Password)
        )
        if ([string]::IsNullOrEmpty($plain)) {
            Write-Warn 'Auto-logon password is empty; skipping auto-logon.'
            return
        }

        if (-not (Test-Path -LiteralPath $WinlogonKey)) {
            New-Item -Path $WinlogonKey -Force | Out-Null
        }
        Set-ItemProperty -LiteralPath $WinlogonKey -Name 'AutoAdminLogon' -Value '1' -Type String -Force
        Set-ItemProperty -LiteralPath $WinlogonKey -Name 'DefaultUserName' -Value $User -Type String -Force
        Set-ItemProperty -LiteralPath $WinlogonKey -Name 'DefaultDomainName' -Value '.' -Type String -Force
        Set-ItemProperty -LiteralPath $WinlogonKey -Name 'DefaultPassword' -Value $plain -Type String -Force

        $marker = Read-Marker
        if (-not $marker) {
            $marker = [ordered]@{ hardening = @() }
        }
        $marker.autologon = [ordered]@{ configured = $true; user = $User }
        Write-Marker $marker
        Write-Info "Configured auto-logon for user '$User'. Reboot to apply."
    } finally {
        if ($plain) {
            $plain = $null
        }
    }
}

function Prompt-AutoLogonPassword {
    if (-not [Environment]::UserInteractive) {
        return $null
    }
    Write-Info "Optional: enable auto-logon for '$KioskUser' (press Enter to skip)."
    $secure = Read-Host 'Auto-logon password' -AsSecureString
    if ($secure.Length -eq 0) {
        return $null
    }
    return $secure
}

function Invoke-Uninstall {
    Remove-KioskLogonTask
    Remove-HardeningFromMarker
    Clear-AutoLogonFromMarker
    if (Test-Path -LiteralPath $MarkerPath) {
        Remove-Item -LiteralPath $MarkerPath -Force
    }
    $pausePath = Join-Path $MarkerDir 'watchdog.pause'
    if (Test-Path -LiteralPath $pausePath) {
        Remove-Item -LiteralPath $pausePath -Force
    }
    Write-Info 'Station configuration removed.'
}

if ($Uninstall) {
    Invoke-Uninstall
    exit 0
}

if ($ShellMode -eq 'ReplaceShell') {
    Write-Warn 'Shell replacement (-ShellMode ReplaceShell) is not automated; see KIOSK-WINDOWS-DEPLOYMENT.md Layer 1 Option B.'
} elseif ($ShellMode -eq 'RunKey') {
    Write-Warn 'Run-key shell mode is optional; the Arena360 Kiosk scheduled task launches the app at logon.'
}

$skipStart = $SkipAutostart -or $SkipWatchdog
if (-not $skipStart) {
    if ([string]::IsNullOrWhiteSpace($InstallDir)) {
        Write-Warn 'InstallDir not provided; skipping kiosk logon task.'
    } else {
        $kioskExe = Resolve-KioskExe $InstallDir
        if ($kioskExe) {
            Install-KioskLogonTask -KioskExe $kioskExe -RunAsUser $KioskUser
        } else {
            Write-Warn "No kiosk binary under $InstallDir; skipping scheduled task."
        }
    }
}

if (-not $SkipHardening) {
    Install-Hardening
}

if ($AutoLogonPassword) {
    Install-AutoLogon -User $KioskUser -Password $AutoLogonPassword
} else {
    $prompted = Prompt-AutoLogonPassword
    if ($prompted) {
        Install-AutoLogon -User $KioskUser -Password $prompted
    } else {
        Write-Warn "Auto-logon not configured. The kiosk will not start until a user logs on. Use Sysinternals Autologon, re-run with -AutoLogonPassword, or ensure the auto-logon user matches -KioskUser ($KioskUser)."
    }
}

Write-Info 'Station configuration complete.'
