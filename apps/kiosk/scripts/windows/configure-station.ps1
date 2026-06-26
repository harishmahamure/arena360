#Requires -Version 5.1
<#
.SYNOPSIS
  Idempotent post-install configuration for Arena360 kiosk stations.

.DESCRIPTION
  Registers the kiosk logon scheduled task, optional HKLM policy hardening, and optional
  auto-logon for a single Windows kiosk user. Invoked from NSIS post-install or manually
  for fleet rollout. Never logs passwords.

  Launches Arena360 at logon via the Arena360 Watchdog sidecar (ADR-0048), which
  relaunches the main kiosk if it exits unexpectedly. Does not modify Winlogon\Shell
  or replace explorer.exe as the Windows shell.

.PARAMETER Uninstall
  Remove autostart task and registry values recorded in the Arena360 marker file.
#>
[CmdletBinding()]
param(
    [string]$KioskUser = $env:USERNAME,
    [switch]$KioskUserExplicit,
    [Parameter(Mandatory = $false)]
    [string]$InstallDir,
    [SecureString]$AutoLogonPassword,
    [switch]$SkipAutostart,
    [alias('SkipWatchdog')]
    [switch]$SkipWatchdog,
    [switch]$SkipHardening,
    [switch]$Uninstall,
    [switch]$RefreshAutostartOnly,
    [ValidateSet('RunKey', 'ReplaceShell', 'None')]
    [string]$ShellMode = 'None'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$WatchdogTaskName = 'Arena360 Watchdog'
$LegacyKioskTaskName = 'Arena360 Kiosk'
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

function Get-SamAccountName([string]$QualifiedName) {
    if ([string]::IsNullOrWhiteSpace($QualifiedName)) {
        return $null
    }
    $name = $QualifiedName.Trim()
    if ($name -match '\\') {
        return $name.Split('\')[-1]
    }
    return $name
}

function Test-IsServiceAccount([string]$User) {
    if ([string]::IsNullOrWhiteSpace($User)) {
        return $true
    }
    $normalized = $User.Trim().ToUpperInvariant()
    return $normalized -eq 'SYSTEM' -or $normalized -eq 'LOCAL SERVICE' -or $normalized -eq 'NETWORK SERVICE' -or $normalized.EndsWith('$')
}

function Get-ConsoleLoggedOnUser {
    try {
        $cs = Get-CimInstance -ClassName Win32_ComputerSystem -ErrorAction Stop
        if ($cs.UserName) {
            $sam = Get-SamAccountName $cs.UserName
            if ($sam) {
                return $sam
            }
        }
    } catch {
        Write-Warn "Win32_ComputerSystem query failed: $_"
    }

    try {
        $output = @(query user 2>$null)
        if ($LASTEXITCODE -eq 0 -and $output.Count -gt 1) {
            foreach ($line in $output | Select-Object -Skip 1) {
                if ($line -match '^\s*(\S+)\s+\S+\s+\d+\s+(\S+)') {
                    if ($matches[2] -eq 'Active') {
                        $sam = Get-SamAccountName $matches[1]
                        if ($sam) {
                            return $sam
                        }
                    }
                }
            }
        }
    } catch {
        Write-Warn "query user failed: $_"
    }

    return $null
}

function Test-LocalUserExists([string]$User) {
    if ([string]::IsNullOrWhiteSpace($User)) {
        return $false
    }
    try {
        $null = Get-LocalUser -Name $User -ErrorAction Stop
        return $true
    } catch {
        try {
            $principal = New-Object System.Security.Principal.NTAccount('.', $User)
            $null = $principal.Translate([System.Security.Principal.SecurityIdentifier])
            return $true
        } catch {
            try {
                $principal = New-Object System.Security.Principal.NTAccount($User)
                $null = $principal.Translate([System.Security.Principal.SecurityIdentifier])
                return $true
            } catch {
                return $false
            }
        }
    }
}

function Write-InstallNeedsRepair([string]$Reason) {
    Ensure-MarkerDir
    $repairPath = Join-Path $MarkerDir 'install-needs-repair.json'
    [ordered]@{
        reason = $Reason
        at     = (Get-Date).ToString('o')
    } | ConvertTo-Json | Set-Content -LiteralPath $repairPath -Encoding UTF8
    Write-Warn "Wrote repair marker to $repairPath"
}

function Get-AutoLogonUser {
    if (-not (Test-Path -LiteralPath $WinlogonKey)) {
        return $null
    }
    $props = Get-ItemProperty -LiteralPath $WinlogonKey
    if ($props.AutoAdminLogon -ne '1') {
        return $null
    }
    $name = [string]$props.DefaultUserName
    if ([string]::IsNullOrWhiteSpace($name)) {
        return $null
    }
    return $name.Trim()
}

function Resolve-EffectiveKioskUser([string]$RequestedUser, [switch]$ExplicitRequest) {
    $resolved = $RequestedUser
    if ([string]::IsNullOrWhiteSpace($resolved)) {
        $resolved = $env:USERNAME
    }

    if (-not $ExplicitRequest) {
        if (Test-IsServiceAccount $resolved) {
            $consoleUser = Get-ConsoleLoggedOnUser
            if ($consoleUser) {
                Write-Info "Using console logged-on user '$consoleUser' (install context was '$resolved')."
                $resolved = $consoleUser
            }
        } elseif (Test-IsServiceAccount $env:USERNAME) {
            $consoleUser = Get-ConsoleLoggedOnUser
            if ($consoleUser -and ($resolved -ne $consoleUser)) {
                Write-Info "Using console logged-on user '$consoleUser' instead of deploy account '$resolved'."
                $resolved = $consoleUser
            }
        }
    }

    if (Test-IsServiceAccount $resolved) {
        $marker = Read-Marker
        if ($marker -and $marker.kioskUser) {
            $markerUser = [string]$marker.kioskUser
            if ($markerUser) {
                Write-Info "Using marker kiosk user '$markerUser'."
                $resolved = $markerUser
            }
        }
    }

    if (-not $ExplicitRequest) {
        $autoLogonUser = Get-AutoLogonUser
        if ($autoLogonUser -and ($resolved -ne $autoLogonUser)) {
            $marker = Read-Marker
            if ($marker -and $marker.autologon -and $marker.autologon.configured) {
                Write-Warn "Using Arena360-configured auto-logon user '$autoLogonUser' for scheduled task."
                return $autoLogonUser
            }
        }
    }

    return $resolved
}

function Resolve-WatchdogExe([string]$Dir) {
    if ([string]::IsNullOrWhiteSpace($Dir)) {
        return $null
    }
    $preferred = Join-Path $Dir 'arena360-watchdog.exe'
    if (Test-Path -LiteralPath $preferred) {
        return $preferred
    }
    return $null
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

function Remove-LegacyKioskTask {
    $null = schtasks /Delete /TN $LegacyKioskTaskName /F 2>&1
    Write-Info "Removed legacy direct kiosk logon task (if present)."
}

function Remove-WatchdogLogonTask {
    try {
        Unregister-ScheduledTask -TaskName $WatchdogTaskName -Confirm:$false -ErrorAction SilentlyContinue
    } catch {
        $null = schtasks /Delete /TN $WatchdogTaskName /F 2>&1
    }
}

function Remove-LegacyWatchdogTask {
    Remove-LegacyKioskTask
}

function Clear-LegacyWatchdogPause {
    $pausePath = Join-Path $MarkerDir 'watchdog.pause'
    if (Test-Path -LiteralPath $pausePath) {
        Remove-Item -LiteralPath $pausePath -Force
        Write-Info 'Removed legacy watchdog.pause file.'
    }
}

function Test-WatchdogLogonTaskInteractive {
    $query = schtasks /Query /TN $WatchdogTaskName /V /FO LIST 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Warn "Could not verify $WatchdogTaskName task: $query"
        return
    }
    $logonLine = $query | Where-Object { $_ -match 'Logon Mode' }
    if ($logonLine -match 'Interactive') {
        Write-Info "Verified $WatchdogTaskName logon mode is interactive."
    } else {
        Write-Warn "$WatchdogTaskName may not run in the user desktop session. Logon Mode: $logonLine"
    }
}

function Install-WatchdogLogonTask([string]$InstallDirectory, [string]$RunAsUser) {
    $watchdogExe = Resolve-WatchdogExe $InstallDirectory
    if (-not $watchdogExe) {
        Write-Warn "Watchdog binary not found under $InstallDirectory; skipping scheduled task."
        return $false
    }

    $RunAsUser = Resolve-EffectiveKioskUser $RunAsUser -ExplicitRequest:$KioskUserExplicit

    if (-not (Test-LocalUserExists $RunAsUser)) {
        Write-Warn "Windows account '$RunAsUser' was not found; cannot register logon task."
        return $false
    }

    Remove-LegacyKioskTask
    Clear-LegacyWatchdogPause
    Remove-WatchdogLogonTask

    Write-Info "Registering $WatchdogTaskName for user '$RunAsUser' at logon (interactive, 30s delay)."

    $action = New-ScheduledTaskAction -Execute $watchdogExe
    $trigger = New-ScheduledTaskTrigger -AtLogOn -User $RunAsUser
    $trigger.Delay = 'PT30S'
    $principal = New-ScheduledTaskPrincipal -UserId $RunAsUser -LogonType Interactive -RunLevel Limited
    $settings = New-ScheduledTaskSettingsSet `
        -AllowStartIfOnBatteries `
        -DontStopIfGoingOnBatteries `
        -ExecutionTimeLimit ([TimeSpan]::Zero)

    try {
        Register-ScheduledTask `
            -TaskName $WatchdogTaskName `
            -Action $action `
            -Trigger $trigger `
            -Principal $principal `
            -Settings $settings `
            -Force | Out-Null
    } catch {
        Write-Warn "Register-ScheduledTask failed; falling back to schtasks: $_"
        $quoted = "`"$watchdogExe`""
        $result = schtasks /Create /TN $WatchdogTaskName /TR $quoted /SC ONLOGON /RU $RunAsUser /IT /DELAY 0000:30 /RL LIMITED /F 2>&1
        if ($LASTEXITCODE -ne 0) {
            Write-Warn "Could not register watchdog logon task (exit $LASTEXITCODE): $result"
            return $false
        }
    }

    $kioskExe = Resolve-KioskExe $InstallDirectory
    $marker = Read-Marker
    if (-not $marker) {
        $marker = [ordered]@{}
    }
    $marker.kioskUser = $RunAsUser
    if ($kioskExe) {
        $marker.installDir = Split-Path -Parent $kioskExe
    } else {
        $marker.installDir = $InstallDirectory
    }
    Write-Marker $marker

    Write-Info "Registered $WatchdogTaskName to start at logon."
    Test-WatchdogLogonTaskInteractive
    return $true
}

function Install-KioskLogonTask([string]$KioskExe, [string]$RunAsUser) {
    if ([string]::IsNullOrWhiteSpace($KioskExe)) {
        return $false
    }
    $installDir = Split-Path -Parent $KioskExe
    return Install-WatchdogLogonTask -InstallDirectory $installDir -RunAsUser $RunAsUser
}

function Remove-KioskLogonTask {
    Remove-WatchdogLogonTask
    Remove-LegacyKioskTask
    Write-Info 'Removed kiosk autostart scheduled tasks (if present).'
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

if ($RefreshAutostartOnly) {
    if ([string]::IsNullOrWhiteSpace($InstallDir)) {
        Write-Warn 'InstallDir not provided; cannot refresh autostart.'
        exit 1
    }
    $kioskExe = Resolve-KioskExe $InstallDir
    if (-not $kioskExe) {
        Write-Warn "No kiosk binary under $InstallDir; cannot refresh autostart."
        exit 1
    }
    $KioskUser = Resolve-EffectiveKioskUser $KioskUser -ExplicitRequest:$KioskUserExplicit
    if (-not (Install-KioskLogonTask -KioskExe $kioskExe -RunAsUser $KioskUser)) {
        exit 1
    }
    Write-Info 'Autostart task refreshed.'
    exit 0
}

if ($ShellMode -eq 'ReplaceShell') {
    Write-Warn 'Shell replacement (-ShellMode ReplaceShell) is not automated; see KIOSK-WINDOWS-DEPLOYMENT.md Layer 1 Option B.'
} elseif ($ShellMode -eq 'RunKey') {
    Write-Warn 'Run-key shell mode is optional; the Arena360 Watchdog scheduled task launches the kiosk at logon.'
}

$effectiveKioskUser = Resolve-EffectiveKioskUser $KioskUser -ExplicitRequest:$KioskUserExplicit

$skipStart = $SkipAutostart -or $SkipWatchdog
$autostartRegistered = $true
if (-not $skipStart) {
    if ([string]::IsNullOrWhiteSpace($InstallDir)) {
        Write-Warn 'InstallDir not provided; skipping watchdog logon task.'
        $autostartRegistered = $false
    } else {
        $autostartRegistered = Install-WatchdogLogonTask -InstallDirectory $InstallDir -RunAsUser $effectiveKioskUser
    }
}

if (-not $SkipHardening) {
    try {
        Install-Hardening
    } catch {
        Write-Warn "HKLM hardening failed (non-fatal): $_"
    }
}

if ($AutoLogonPassword) {
    Install-AutoLogon -User $effectiveKioskUser -Password $AutoLogonPassword
} else {
    $prompted = Prompt-AutoLogonPassword
    if ($prompted) {
        Install-AutoLogon -User $effectiveKioskUser -Password $prompted
    } else {
        $autoLogonUser = Get-AutoLogonUser
        if ($autoLogonUser) {
            Write-Info "Auto-logon already configured for '$autoLogonUser'."
        } else {
            Write-Warn "Auto-logon not configured. The kiosk will not start until a user logs on. Use Sysinternals Autologon, re-run with -AutoLogonPassword, or ensure the auto-logon user matches -KioskUser ($effectiveKioskUser)."
        }
    }
}

if (-not $skipStart -and -not $autostartRegistered) {
    Write-Warn 'Failed to register watchdog logon autostart.'
    Write-InstallNeedsRepair 'Failed to register watchdog logon autostart.'
    exit 1
}

Write-Info 'Station configuration complete.'
