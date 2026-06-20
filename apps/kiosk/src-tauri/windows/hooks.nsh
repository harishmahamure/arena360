!include "LogicLib.nsh"
!include "FileFunc.nsh"

Var KioskUserName
Var ConfigureScriptPath
Var KioskUserExplicit

!macro RemoveAutostartTasks
  nsExec::ExecToLog 'schtasks /Delete /TN "Arena360 Kiosk" /F'
  Pop $0
  nsExec::ExecToLog 'schtasks /Delete /TN "Arena360 Watchdog" /F'
  Pop $0
!macroend

!macro RunRefreshAutostart
  StrCpy $ConfigureScriptPath "$INSTDIR\scripts\configure-station.ps1"
  ${IfNot} ${FileExists} "$ConfigureScriptPath"
    DetailPrint "configure-station.ps1 not found; skipping autostart refresh"
    Goto refresh_done
  ${EndIf}

  StrCpy $KioskUserName ""
  StrCpy $KioskUserExplicit "0"
  ${GetParameters} $R0
  ${GetOptions} $R0 "/KIOSKUSER=" $KioskUserName
  ${IfNot} ${Errors}
    StrCpy $KioskUserExplicit "1"
  ${EndIf}

  StrCpy $R1 ""
  ${If} $KioskUserExplicit == "1"
    StrCpy $R1 "-KioskUserExplicit"
  ${EndIf}

  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$ConfigureScriptPath" -RefreshAutostartOnly -InstallDir "$INSTDIR" -KioskUser "$KioskUserName" $R1'
  Pop $0
  ${If} $0 != 0
    DetailPrint "Warning: autostart refresh returned code $0 (update continues)"
  ${Else}
    DetailPrint "Arena360 autostart task refreshed for update"
  ${EndIf}

  refresh_done:
!macroend

!macro RunConfigureStation UNINSTALL
  StrCpy $ConfigureScriptPath "$INSTDIR\scripts\configure-station.ps1"
  ${IfNot} ${FileExists} "$ConfigureScriptPath"
    DetailPrint "configure-station.ps1 not found; skipping station configuration"
    Goto configure_done
  ${EndIf}

  !if "${UNINSTALL}" == "1"
    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$ConfigureScriptPath" -Uninstall'
    Pop $0
    ${If} $0 != 0
      DetailPrint "Warning: configure-station.ps1 -Uninstall returned code $0"
    ${Else}
      DetailPrint "Removed Arena360 station configuration"
    ${EndIf}
    Goto configure_done
  !endif

  StrCpy $R0 ""
  StrCpy $R1 ""
  StrCpy $R2 ""
  StrCpy $R3 "0"
  StrCpy $KioskUserExplicit "0"
  ${GetParameters} $R0
  ${GetOptions} $R0 "/NOAUTOSTART" $R1
  ${IfNot} ${Errors}
    StrCpy $R2 "-SkipAutostart"
    StrCpy $R3 "1"
  ${EndIf}
  ${GetOptions} $R0 "/NOHARDENING" $R1
  ${IfNot} ${Errors}
    ${If} $R2 == ""
      StrCpy $R2 "-SkipHardening"
    ${Else}
      StrCpy $R2 "$R2 -SkipHardening"
    ${EndIf}
  ${EndIf}

  StrCpy $KioskUserName ""
  ${GetOptions} $R0 "/KIOSKUSER=" $KioskUserName
  ${IfNot} ${Errors}
    StrCpy $KioskUserExplicit "1"
  ${EndIf}

  StrCpy $R4 ""
  ${If} $KioskUserExplicit == "1"
    StrCpy $R4 "-KioskUserExplicit"
  ${EndIf}

  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$ConfigureScriptPath" -InstallDir "$INSTDIR" -KioskUser "$KioskUserName" $R4 $R2'
  Pop $0
  ${If} $0 != 0
    DetailPrint "ERROR: configure-station.ps1 returned code $0"
    ${If} $R3 == "0"
      ${If} ${Silent}
        DetailPrint "Warning: autostart configuration failed; install continues (silent). Run scripts\verify-station-startup.ps1 -Repair (elevated)."
      ${Else}
        MessageBox MB_ICONSTOP "Arena360 could not register kiosk autostart at logon.$\n$\nInstall while logged in as the account that will run the kiosk, or pass /KIOSKUSER=AccountName.$\n$\nYou can also repair with scripts\verify-station-startup.ps1 -Repair (elevated)."
        Abort "Station autostart configuration failed"
      ${EndIf}
    ${Else}
      DetailPrint "Warning: configure-station.ps1 returned code $0 (autostart was skipped via /NOAUTOSTART)"
    ${EndIf}
  ${Else}
    DetailPrint "Arena360 station configuration applied"
  ${EndIf}

  configure_done:
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${GetParameters} $0
  ${GetOptions} $0 "/NOCONFIGURE" $1
  ${IfNot} ${Errors}
    DetailPrint "Skipping Arena360 station configuration (/NOCONFIGURE)"
    Goto postinstall_done
  ${EndIf}

  ${GetOptions} $0 "/UPDATE" $1
  ${IfNot} ${Errors}
    DetailPrint "In-app update: refreshing autostart task only"
    !insertmacro RunRefreshAutostart
    Goto postinstall_done
  ${EndIf}

  !insertmacro RunConfigureStation "0"
  postinstall_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro RemoveAutostartTasks
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  !insertmacro RunConfigureStation "1"
!macroend
