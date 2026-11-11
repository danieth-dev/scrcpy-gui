; Hooks del instalador NSIS de scrcpy GUI.
; Registra el driver DirectShow de la cámara virtual (Unity Capture) para que
; Zoom, Teams, Discord, OBS y el navegador vean "Unity Video Capture" sin pasos
; extra. Requiere instalación para todos los usuarios (permisos de admin).
; regsvr32 de 32 bits detecta la DLL de 64 bits y delega en el de 64 bits.

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Registrando la cámara virtual (Unity Video Capture)..."
  IfFileExists "$INSTDIR\vendor\UnityCaptureFilter64.dll" 0 +2
    ExecWait '"$WINDIR\System32\regsvr32.exe" /s "$INSTDIR\vendor\UnityCaptureFilter64.dll"'
  IfFileExists "$INSTDIR\vendor\UnityCaptureFilter32.dll" 0 +2
    ExecWait '"$WINDIR\SysWOW64\regsvr32.exe" /s "$INSTDIR\vendor\UnityCaptureFilter32.dll"'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Quitando la cámara virtual (Unity Video Capture)..."
  IfFileExists "$INSTDIR\vendor\UnityCaptureFilter64.dll" 0 +2
    ExecWait '"$WINDIR\System32\regsvr32.exe" /u /s "$INSTDIR\vendor\UnityCaptureFilter64.dll"'
  IfFileExists "$INSTDIR\vendor\UnityCaptureFilter32.dll" 0 +2
    ExecWait '"$WINDIR\SysWOW64\regsvr32.exe" /u /s "$INSTDIR\vendor\UnityCaptureFilter32.dll"'
!macroend
