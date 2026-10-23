; Hooks into the installer Tauri writes (tauri.windows.conf.json).
;
; pktito's model files, .pkf, as a type of their own: Explorer calls them
; "pktito model" and shows their icon (icons/model.ico, scripts/model-icon.py).
; One double-clicked opens the app, which uses it as its model, or goes to
; the app already open (main.rs, hand_over). The uninstaller takes the type
; away, and .pkf with it unless another program has taken it.
;
; pktito-cli, beside the window's program, on the user's PATH: path.ps1 adds
; the install folder and the uninstaller takes it out, before the files go.
; PowerShell edits the PATH, given the folder as an argument: NSIS's own
; strings stop at 1024 characters, past which a long PATH would come back
; cut short. Programs started after it see the change, which the broadcast
; tells Explorer of.

!include WinMessages.nsh

!define PKTITO_POWERSHELL '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass'

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "Software\Classes\.pkf" "" "pktito.model"
  WriteRegStr SHCTX "Software\Classes\pktito.model" "" "pktito model"
  WriteRegStr SHCTX "Software\Classes\pktito.model\DefaultIcon" "" "$INSTDIR\model.ico"
  WriteRegStr SHCTX "Software\Classes\pktito.model\shell\open\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" "%1"'
  ; SHCNE_ASSOCCHANGED, SHCNF_FLUSH: Explorer draws the new icon now
  System::Call "shell32::SHChangeNotify(i,i,i,i) (0x08000000, 0x1000, 0, 0)"
  nsExec::Exec '${PKTITO_POWERSHELL} -File "$INSTDIR\path.ps1" -Action add -Dir "$INSTDIR"'
  Pop $0
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec '${PKTITO_POWERSHELL} -File "$INSTDIR\path.ps1" -Action remove -Dir "$INSTDIR"'
  Pop $0
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ReadRegStr $R0 SHCTX "Software\Classes\.pkf" ""
  ${If} $R0 == "pktito.model"
    DeleteRegKey SHCTX "Software\Classes\.pkf"
  ${EndIf}
  DeleteRegKey SHCTX "Software\Classes\pktito.model"
  System::Call "shell32::SHChangeNotify(i,i,i,i) (0x08000000, 0x1000, 0, 0)"
!macroend
