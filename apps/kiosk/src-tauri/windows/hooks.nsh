!include "LogicLib.nsh"
!include "FileFunc.nsh"

Var WatchdogExePath

!macro ResolveWatchdogExe
  StrCpy $WatchdogExePath "$INSTDIR\arena360-watchdog.exe"
  ${IfNot} ${FileExists} "$WatchdogExePath"
    FindFirst $0 $1 "$INSTDIR\arena360-watchdog*.exe"
    ${If} $0 != ""
      StrCpy $WatchdogExePath "$INSTDIR\$1"
      FindClose $0
    ${EndIf}
  ${EndIf}
!macroend

!macro RemoveWatchdogTask
  nsExec::ExecToLog 'schtasks /Delete /TN "Arena360 Watchdog" /F'
  Pop $0
!macroend

!macro InstallWatchdogTask
  !insertmacro ResolveWatchdogExe
  ${IfNot} ${FileExists} "$WatchdogExePath"
    DetailPrint "Arena360 watchdog binary not found; skipping auto-start task"
    Goto watchdog_done
  ${EndIf}

  !insertmacro RemoveWatchdogTask
  nsExec::ExecToLog 'schtasks /Create /TN "Arena360 Watchdog" /TR "\"$WatchdogExePath\"" /SC ONLOGON /RL LIMITED /F'
  Pop $0
  ${If} $0 != 0
    DetailPrint "Warning: could not register Arena360 Watchdog scheduled task (code $0)"
  ${Else}
    DetailPrint "Registered Arena360 Watchdog to start at logon"
  ${EndIf}
  watchdog_done:
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${GetParameters} $0
  ${GetOptions} $0 "/NOAUTOSTART" $1
  ${IfNot} ${Errors}
    DetailPrint "Skipping Arena360 auto-start (/NOAUTOSTART)"
    Goto postinstall_done
  ${EndIf}

  !insertmacro InstallWatchdogTask
  postinstall_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro RemoveWatchdogTask
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$COMMONAPPDATA\Arena360\watchdog.pause"
!macroend
