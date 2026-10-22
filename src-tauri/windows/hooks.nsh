; Hooks into the installer Tauri writes (tauri.windows.conf.json).
;
; pktito's model files, .pkf, as a type of their own: Explorer calls them
; "pktito model" and shows their icon (icons/model.ico, scripts/model-icon.py).
; They open with nothing, as a model is chosen in the app. The uninstaller
; takes the type away, and .pkf with it unless another program has taken it.

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "Software\Classes\.pkf" "" "pktito.model"
  WriteRegStr SHCTX "Software\Classes\pktito.model" "" "pktito model"
  WriteRegStr SHCTX "Software\Classes\pktito.model\DefaultIcon" "" "$INSTDIR\model.ico"
  ; SHCNE_ASSOCCHANGED, SHCNF_FLUSH: Explorer draws the new icon now
  System::Call "shell32::SHChangeNotify(i,i,i,i) (0x08000000, 0x1000, 0, 0)"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ReadRegStr $R0 SHCTX "Software\Classes\.pkf" ""
  ${If} $R0 == "pktito.model"
    DeleteRegKey SHCTX "Software\Classes\.pkf"
  ${EndIf}
  DeleteRegKey SHCTX "Software\Classes\pktito.model"
  System::Call "shell32::SHChangeNotify(i,i,i,i) (0x08000000, 0x1000, 0, 0)"
!macroend
