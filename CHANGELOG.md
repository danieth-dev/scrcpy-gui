# Changelog

Todos los cambios notables de **scrcpy GUI** se documentan aquí.
Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/).
Versionado: [SemVer](https://semver.org/lang/es/).

## [1.3.0] — 2026-10-10

### Añadido
- Soporte para cámara de iPhone mediante cable USB físico nativo (usbmuxd) con cero latencia (<10 ms).
- Modo pantalla negra OLED en la app complementaria iOS ScrcpyCam para máximo ahorro de batería y cero calor.
- Selector dinámico en la interfaz: Cable USB (Cero Latencia) vs AirPlay (Wi-Fi).
- Monitor de bitrate USB y estado de dispositivo en tiempo real.
- Corrección de lanzamiento de OBS en Windows con bypass de bloqueo por permisos.
- Corrección de diseño del panel de iPhone y resolución de errores visuales.

## [1.2.0] — 2026-10-02

### Añadido
- Primer release público en GitHub con repositorio oficial
- Identidad de autor configurada (`danieth` / `danieth-dev`)
- Portable `.exe` listo para descarga directa

## [1.0.0] — 2026-10-02

### Añadido
- Interfaz Electron para scrcpy (dispositivos, WiFi, sesiones, ajustes)
- Modo cámara / webcam con presets (Normal, Webcam, Streams, Gaming)
- Ajustes en vivo: resolución, FPS, bitrate, codec, facing, orientación, zoom, linterna
- WiFi: vincular, conectar, y “Activar y conectar” (USB → tcpip 5555)
- Recientes con Mirror y Cámara
- Errores de scrcpy visibles en la UI
- Aviso si el mismo teléfono está por USB y WiFi
- Empaquetado portable con `vendor/scrcpy`
- Comprobación de actualizaciones vía GitHub Releases

### Créditos
- Motor de espejo/cámara: [scrcpy](https://github.com/Genymobile/scrcpy) (Genymobile)
