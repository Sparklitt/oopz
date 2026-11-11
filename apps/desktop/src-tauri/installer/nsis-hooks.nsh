; The NSIS installer's additions (per user; added by tauri.cli.conf.json): `oopz` in a terminal runs the CLI, oopz.exe
; beside the app (oopz-desktop.exe, mainBinaryName). The user PATH is edited by nsis-path.ps1, never by NSIS strings,
; which stop at 1024 characters and would cut a long PATH.

; This file's folder, for File: inside a macro ${__FILEDIR__} would name the installer script's.
!define OOPZ_HOOKS_DIR "${__FILEDIR__}"

; Runs nsis-path.ps1 add|remove for $INSTDIR, logs what it said, then tells open windows the environment changed
; (WM_SETTINGCHANGE, "Environment") so a new terminal sees it.
!macro OOPZ_PATH action
  InitPluginsDir
  File "/oname=$PLUGINSDIR\nsis-path.ps1" "${OOPZ_HOOKS_DIR}\nsis-path.ps1"
  System::Call 'kernel32::SetEnvironmentVariable(t "OOPZ_DIR", t "$INSTDIR")'
  nsExec::ExecToLog 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\nsis-path.ps1" ${action}'
  Pop $0
  ${If} $0 != 0
    DetailPrint "oopz: couldn't change the user PATH (${action}: $0); the oopz command may not be found in a terminal"
  ${EndIf}
  SendMessage 0xFFFF 0x001A 0 "STR:Environment" /TIMEOUT=5000
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro OOPZ_PATH add
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; An update runs the old uninstaller first (/UPDATE): the folder stays, and so do PATH and the install key.
  ${If} $UpdateMode <> 1
    !insertmacro OOPZ_PATH remove
    ; Where the installer remembered its folder ("Software\<publisher>\oopz"); left behind, it would send a later MSI
    ; install to this per-user folder.
    DeleteRegKey HKCU "${MANUPRODUCTKEY}"
    DeleteRegKey /ifempty HKCU "${MANUKEY}"
  ${EndIf}
!macroend
