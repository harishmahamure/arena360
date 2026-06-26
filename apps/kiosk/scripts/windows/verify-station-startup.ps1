#Requires -Version 5.1
<#
.SYNOPSIS
  Diagnose and optionally repair Arena360 kiosk logon autostart.

.DESCRIPTION
  Checks the Arena360 Watchdog scheduled task, kiosk/watchdog executables, and pause file TTL.
  With -Repair, re-registers the logon task via configure-station.ps1.

.PARAMETER Repair
  Re-register the logon scheduled task for the effective kiosk user.

.PARAMETER InstallDir
  Kiosk install directory. When omitted, resolved from marker, uninstall registry, or known paths.
#>
[CmdletBinding()]
param(
    [switch]$Repair,
    [string]$InstallDir
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$WatchdogTaskName = 'Arena360 Watchdog'
$LegacyKioskTaskName = 'Arena360 Kiosk'
$WinlogonKey = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon'
$MarkerDir = Join-Path $env:ProgramData 'Arena360'
$MarkerPath = Join-Path $MarkerDir 'registry-hardening.json'
$PausePath = Join-Path $MarkerDir 'watchdog.pause'

function Write-Info([string]$Message) {
    Write-Host "[Arena360 verify] $Message"
}

function Write-Issue([string]$Message) {
    Write-Host "[Arena360 verify] ISSUE: $Message" -ForegroundColor Yellow
    $script:Issues += $Message
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

function Get-ConsoleLoggedOnUser {
    try {
        $cs = Get-CimInstance -ClassName Win32_ComputerSystem -ErrorAction Stop
        if ($cs.UserName) {
            return Get-SamAccountName $cs.UserName
        }
    } catch {
        return $null
    }
    return $null
}

function Resolve-InstallDir([string]$RequestedDir) {
    if (-not [string]::IsNullOrWhiteSpace($RequestedDir) -and (Test-Path -LiteralPath $RequestedDir)) {
        return $RequestedDir
    }

    if (Test-Path -LiteralPath $MarkerPath) {
        try {
            $marker = Get-Content -LiteralPath $MarkerPath -Raw | ConvertFrom-Json
            if ($marker.installDir -and (Test-Path -LiteralPath $marker.installDir)) {
                return [string]$marker.installDir
            }
        } catch {
            Write-Info 'Could not read installDir from marker file.'
        }
    }

    $uninstallRoots = @(
        'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*'
    )
    foreach ($root in $uninstallRoots) {
        $matches = Get-ItemProperty $root -ErrorAction SilentlyContinue |
            Where-Object { $_.DisplayName -like 'Arena360*' -and $_.InstallLocation }
        foreach ($entry in $matches) {
            $loc = [string]$entry.InstallLocation
            if ($loc) {
                $loc = $loc.TrimEnd('\')
            }
            if ($loc -and (Test-Path -LiteralPath $loc)) {
                return $loc
            }
        }
    }

    $candidates = @(
        'C:\Program Files\Arena360 Station Management',
        'C:\Program Files\Arena360\kiosk'
    )
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate) {
            return $candidate
        }
    }

    if (-not [string]::IsNullOrWhiteSpace($RequestedDir)) {
        return $RequestedDir
    }
    return 'C:\Program Files\Arena360 Station Management'
}

function Resolve-RepairUser {
    if (Test-Path -LiteralPath $MarkerPath) {
        try {
            $marker = Get-Content -LiteralPath $MarkerPath -Raw | ConvertFrom-Json
            if ($marker.kioskUser) {
                return [string]$marker.kioskUser
            }
        } catch { }
    }

    $consoleUser = Get-ConsoleLoggedOnUser
    if ($consoleUser) {
        return $consoleUser
    }

    if (Test-Path -LiteralPath $WinlogonKey) {
        $winlogon = Get-ItemProperty -LiteralPath $WinlogonKey
        if ($winlogon.AutoAdminLogon -eq '1' -and $winlogon.DefaultUserName) {
            return [string]$winlogon.DefaultUserName
        }
    }

    return $env:USERNAME
}

function Test-PauseFileActive {
    if (-not (Test-Path -LiteralPath $PausePath)) {
        return $false
    }
    try {
        $pause = Get-Content -LiteralPath $PausePath -Raw | ConvertFrom-Json
        if (-not $pause.expiresAt) {
            return $true
        }
        $expires = [DateTime]::Parse($pause.expiresAt)
        return $expires.ToUniversalTime() -gt [DateTime]::UtcNow
    } catch {
        return $true
    }
}

$Issues = @()
$InstallDir = Resolve-InstallDir $InstallDir
$ConfigureScript = Join-Path $InstallDir 'scripts\configure-station.ps1'

Write-Info '--- Arena360 kiosk startup diagnosis ---'
Write-Info "Resolved install directory: $InstallDir"

$autoLogonEnabled = $false
$autoLogonUser = $null
if (Test-Path -LiteralPath $WinlogonKey) {
    $winlogon = Get-ItemProperty -LiteralPath $WinlogonKey
    $autoLogonEnabled = $winlogon.AutoAdminLogon -eq '1'
    $autoLogonUser = $winlogon.DefaultUserName
}
Write-Info "Auto-logon enabled: $autoLogonEnabled"
Write-Info "Auto-logon user: $(if ($autoLogonUser) { $autoLogonUser } else { '(none)' })"

if (-not $autoLogonEnabled -or [string]::IsNullOrWhiteSpace($autoLogonUser)) {
    Write-Info 'Auto-logon is not configured. The watchdog starts after the installing user logs on.'
}

$preferredExe = Join-Path $InstallDir 'Arena360 Station Management.exe'
$watchdogExe = Join-Path $InstallDir 'arena360-watchdog.exe'
$kioskExe = $null
if (Test-Path -LiteralPath $preferredExe) {
    $kioskExe = $preferredExe
} elseif (Test-Path -LiteralPath $InstallDir) {
    $match = Get-ChildItem -LiteralPath $InstallDir -Filter '*.exe' -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -notlike 'arena360-watchdog*' -and $_.Name -notlike 'uninstall*' } |
        Select-Object -First 1
    if ($match) {
        $kioskExe = $match.FullName
    }
}
Write-Info "Kiosk executable: $(if ($kioskExe) { $kioskExe } else { '(not found)' })"
Write-Info "Watchdog executable: $(if (Test-Path -LiteralPath $watchdogExe) { $watchdogExe } else { '(not found)' })"
if (-not $kioskExe) {
    Write-Issue "Kiosk binary not found under $InstallDir"
}
if (-not (Test-Path -LiteralPath $watchdogExe)) {
    Write-Issue "Watchdog binary not found at $watchdogExe"
}

if (Test-PauseFileActive) {
    Write-Issue "watchdog.pause is active at $PausePath (watchdog will not relaunch kiosk until it expires)"
}

$legacyTask = schtasks /Query /TN $LegacyKioskTaskName /V /FO LIST 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Issue "Legacy direct-launch task '$LegacyKioskTaskName' still exists (re-run configure-station.ps1)"
}

$taskQuery = schtasks /Query /TN $WatchdogTaskName /V /FO LIST 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Issue "Scheduled task '$WatchdogTaskName' is missing"
} else {
    Write-Info "Scheduled task '$WatchdogTaskName' exists"

    $runAsUser = ($taskQuery | Where-Object { $_ -match 'Run As User' }) -replace '^\s*Run As User:\s*', ''
    $taskToRun = ($taskQuery | Where-Object { $_ -match 'Task To Run' }) -replace '^\s*Task To Run:\s*', ''
    $status = ($taskQuery | Where-Object { $_ -match 'Scheduled Task State' }) -replace '^\s*Scheduled Task State:\s*', ''
    $logonMode = ($taskQuery | Where-Object { $_ -match 'Logon Mode' }) -replace '^\s*Logon Mode:\s*', ''

    Write-Info "  Run As User: $runAsUser"
    Write-Info "  Task To Run: $taskToRun"
    Write-Info "  State: $status"
    Write-Info "  Logon Mode: $logonMode"

    if ($autoLogonUser -and $runAsUser -and ($runAsUser -ne $autoLogonUser)) {
        Write-Issue "Task user '$runAsUser' does not match auto-logon user '$autoLogonUser'"
    }

    if ((Test-Path -LiteralPath $watchdogExe) -and $taskToRun -and ($taskToRun.Trim('"') -ne $watchdogExe)) {
        Write-Issue "Task action '$taskToRun' does not match installed watchdog '$watchdogExe'"
    }

    if ($status -notmatch 'Ready|Running') {
        Write-Issue "Task state is '$status' (expected Ready)"
    }

    if ($logonMode -notmatch 'Interactive') {
        Write-Issue "Task logon mode is '$logonMode' (expected Interactive only)"
    }
}

if ($Repair) {
    if (-not (Test-Path -LiteralPath $ConfigureScript)) {
        Write-Issue "Cannot repair: $ConfigureScript not found"
    } else {
        $kioskUser = Resolve-RepairUser
        Write-Info "Repairing autostart for user '$kioskUser'..."
        & powershell -NoProfile -ExecutionPolicy Bypass -File $ConfigureScript `
            -InstallDir $InstallDir `
            -KioskUser $kioskUser `
            -SkipHardening
        if ($LASTEXITCODE -ne 0) {
            Write-Issue "configure-station.ps1 failed with exit code $LASTEXITCODE"
        } else {
            Write-Info 'Repair complete. Reboot the station and wait ~30s after desktop appears.'
        }
    }
}

Write-Info '--- Summary ---'
if ($Issues.Count -eq 0) {
    Write-Info 'No issues detected. After reboot, watchdog should launch ~30s after logon and start the kiosk.'
    exit 0
}

Write-Info "$($Issues.Count) issue(s) found."
if (-not $Repair) {
    Write-Info 'Re-run with -Repair to re-register the logon task (requires Administrator).'
}
exit 1
