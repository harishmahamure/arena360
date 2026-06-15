#Requires -Version 5.1
<#
.SYNOPSIS
  Diagnose and optionally repair Arena360 kiosk logon autostart.

.DESCRIPTION
  Checks auto-logon user, the Arena360 Kiosk scheduled task, executable path, and
  legacy watchdog.pause. With -Repair, re-registers the logon task via configure-station.ps1.

.PARAMETER Repair
  Re-register the logon scheduled task for the effective kiosk user.

.PARAMETER InstallDir
  Kiosk install directory (default: C:\Program Files\Arena360\kiosk).
#>
[CmdletBinding()]
param(
    [switch]$Repair,
    [string]$InstallDir = 'C:\Program Files\Arena360\kiosk'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$KioskTaskName = 'Arena360 Kiosk'
$WinlogonKey = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon'
$MarkerDir = Join-Path $env:ProgramData 'Arena360'
$PausePath = Join-Path $MarkerDir 'watchdog.pause'
$ConfigureScript = Join-Path $InstallDir 'scripts\configure-station.ps1'

function Write-Info([string]$Message) {
    Write-Host "[Arena360 verify] $Message"
}

function Write-Issue([string]$Message) {
    Write-Host "[Arena360 verify] ISSUE: $Message" -ForegroundColor Yellow
    $script:Issues += $Message
}

$Issues = @()

Write-Info '--- Arena360 kiosk startup diagnosis ---'

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
    Write-Issue 'Auto-logon is not configured. The kiosk only starts after a user logs on.'
}

$preferredExe = Join-Path $InstallDir 'Arena360 Station Management.exe'
$kioskExe = $null
if (Test-Path -LiteralPath $preferredExe) {
    $kioskExe = $preferredExe
} else {
    $match = Get-ChildItem -LiteralPath $InstallDir -Filter '*.exe' -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -notlike 'arena360-watchdog*' -and $_.Name -notlike 'uninstall*' } |
        Select-Object -First 1
    if ($match) {
        $kioskExe = $match.FullName
    }
}
Write-Info "Kiosk executable: $(if ($kioskExe) { $kioskExe } else { '(not found)' })"
if (-not $kioskExe) {
    Write-Issue "Kiosk binary not found under $InstallDir"
}

if (Test-Path -LiteralPath $PausePath) {
    Write-Issue "Legacy watchdog.pause exists at $PausePath (blocks old watchdog builds)"
}

$taskQuery = schtasks /Query /TN $KioskTaskName /V /FO LIST 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Issue "Scheduled task '$KioskTaskName' is missing"
} else {
    $taskText = ($taskQuery | Out-String)
    Write-Info "Scheduled task '$KioskTaskName' exists"

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

    if ($kioskExe -and $taskToRun -and ($taskToRun.Trim('"') -ne $kioskExe)) {
        Write-Issue "Task action '$taskToRun' does not match installed exe '$kioskExe'"
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
        $kioskUser = if ($autoLogonUser) { $autoLogonUser } else { $env:USERNAME }
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
    Write-Info 'No issues detected. After reboot, kiosk should launch ~30s after auto-logon.'
    exit 0
}

Write-Info "$($Issues.Count) issue(s) found."
if (-not $Repair) {
    Write-Info 'Re-run with -Repair to re-register the logon task (requires Administrator).'
}
exit 1
