!include "LogicLib.nsh"

!macro RemoveAutostartTasks
  nsExec::ExecToLog 'schtasks /Delete /TN "Arena360 Kiosk" /F'
  Pop $0
  nsExec::ExecToLog 'schtasks /Delete /TN "Arena360 Watchdog" /F'
  Pop $0
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Every installed station starts at Windows login. The runtime also repairs
  ; HKCU startup on first launch for existing installations.
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "Arena360 Kiosk" '$\"$INSTDIR\${MAINBINARYNAME}.exe$\"'
  ; Configure secure-desktop suppression if Keyboard Filter is already enabled.
  nsExec::ExecToLog 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\scripts\configure-keyboard-filter.ps1"'
  Pop $0
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegValue HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "Arena360 Kiosk"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Arena360 Kiosk"
  nsExec::ExecToLog 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\scripts\configure-keyboard-filter.ps1" -Uninstall'
  Pop $0
  !insertmacro RemoveAutostartTasks
!macroend
