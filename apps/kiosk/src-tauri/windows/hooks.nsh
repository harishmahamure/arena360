!include "LogicLib.nsh"

!macro RemoveAutostartTasks
  nsExec::ExecToLog 'schtasks /Delete /TN "Arena360 Kiosk" /F'
  Pop $0
  nsExec::ExecToLog 'schtasks /Delete /TN "Arena360 Watchdog" /F'
  Pop $0
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro RemoveAutostartTasks
!macroend
