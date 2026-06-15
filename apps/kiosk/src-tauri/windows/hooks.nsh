!include "LogicLib.nsh"
!include "FileFunc.nsh"

Var KioskUserName
Var ConfigureScriptPath

!macro RemoveAutostartTasks
  nsExec::ExecToLog 'schtasks /Delete /TN "Arena360 Kiosk" /F'
  Pop $0
  nsExec::ExecToLog 'schtasks /Delete /TN "Arena360 Watchdog" /F'
  Pop $0
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
  ${GetParameters} $R0
  ${GetOptions} $R0 "/NOAUTOSTART" $R1
  ${IfNot} ${Errors}
    StrCpy $R2 "-SkipAutostart"
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
  ${If} $KioskUserName == ""
    ReadEnvStr $KioskUserName USERNAME
  ${EndIf}

  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$ConfigureScriptPath" -InstallDir "$INSTDIR" -KioskUser "$KioskUserName" $R2'
  Pop $0
  ${If} $0 != 0
    DetailPrint "Warning: configure-station.ps1 returned code $0"
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

  !insertmacro RunConfigureStation "0"
  postinstall_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro RemoveAutostartTasks
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  !insertmacro RunConfigureStation "1"
!macroend
