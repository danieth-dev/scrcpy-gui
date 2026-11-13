# Changelog

Todos los cambios notables de **scrcpy GUI** se documentan aquí.
Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/).
Versionado: [SemVer](https://semver.org/lang/es/).

## [2.0.0] — 2026-11-10

### Reescritura de Arquitectura (Migración a Rust + Tauri v2)
- **Migración a Tauri v2 (Rust)**: Eliminado completamente Electron y Node.js del backend. El motor de ventana ahora utiliza WebView2 nativo de Windows y el backend está compilado en Rust.
- **Reducción masiva de memoria**: El consumo de RAM en reposo disminuyó de ~180-250 MB a **~30-35 MB** (reducción del 85%).
- **Ejecutable ultraligero**: El binario standalone compilado (`scrcpy-gui.exe`) se redujo de más de 180 MB a tan solo **~14 MB**.
- **Inicio instantáneo**: Tiempo de carga reducido a menos de 0.3 segundos.
- **Puente de compatibilidad transparente (`tauri-bridge.js`)**: Adaptador que implementa la API de `window.api` conectando el frontend existente con los comandos nativos de Tauri v2 sin requerir cambios en `renderer.js`.
- **Backend asíncrono y seguro**:
  - `tools.rs`: Búsqueda de ejecutables en árbol de ancestros (`vendor/scrcpy/`, `vendor/uxplay/`) y comprobación de actualizaciones contra GitHub Releases.
  - `adb.rs`: Ejecución segura de comandos ADB sin invocación de shell (mitigando inyección de comandos), detección en paralelo de dispositivos y gestión de reconexiones Wi-Fi.
  - `scrcpy.rs`: Gestor concurrente de sesiones (`ScrcpyManager`), detección de tamaños nativos de cámara (`--list-camera-sizes`), streaming de eventos de textura y auto-recuperación ante microcortes.
  - `iphone.rs`: Ciclo de vida de UxPlay con drenado continuo de pipes y soporte para OBS Virtual Camera.
  - `lib.rs`: Bandeja del sistema (Tray) nativa con menú contextual y soporte para minimizar a segundo plano.
- **Limpieza de dependencias**: Eliminados 312 paquetes obsoletos de `node_modules` (incluyendo Chromium y Electron Builder) y eliminados más de 600 MB de antiguos binarios empaquetados.

### Cámara virtual nativa
- **Sin ffmpeg.exe**: el vídeo del servidor de scrcpy se decodifica dentro de la app con las DLL de FFmpeg que trae scrcpy y se envía directamente a "Unity Video Capture". 1080p a 30 fps sin pérdidas y hasta 4K en H.265.
- **Reconexión automática** de la ventana y de la cámara virtual tras un corte de USB o Wi-Fi.
- **Control de versión de FFmpeg**: la cámara virtual se niega a arrancar, con un mensaje claro, si las DLL no son de una versión probada.

### Ajustes que se adaptan a cada teléfono
- **Opciones reales del dispositivo**: resoluciones, fps, códecs por hardware, zoom y linterna se leen del teléfono. No se ofrece nada que no soporte.
- **Cámara y pantalla por separado**. "Ver pantalla" interpreta la resolución por el lado corto (1080p = nativa en un 1080×2340).
- **Perfil "Máxima calidad"** (por defecto): elige automáticamente lo mejor que da cada teléfono.
- **Ajustes en vivo**: los cambios se aplican a las sesiones abiertas sin cerrarlas y conservan la posición de la ventana.
- **Protección del dispositivo opcional**: vigila la temperatura y la batería. El freno de emergencia sigue activo siempre.

### Instalador y licencias
- Instalador NSIS que registra el driver de la cámara virtual (32 y 64 bits) al instalar y lo elimina al desinstalar.
- Añadido `THIRD_PARTY_LICENSES.md` (scrcpy, FFmpeg, SDL, libusb, adb, Unity Capture y UxPlay), que también se instala junto a la app.
- La versión mostrada en la app y la que usa la comprobación de actualizaciones salen de `Cargo.toml`. Antes estaba escrita a mano en tres sitios distintos.


## [1.3.1] — 2026-10-23

### Corregido
- **iOS / Info.plist**: Reemplazado `$(EXECUTABLE_NAME)` por nombre estático `ScrcpyCam` y añadidos `MinimumOSVersion` (16.0) y `UIDeviceFamily` para permitir arranque correcto en dispositivos reales.
- **iOS / H264Encoder**: Corregida orientación de dimensiones (intercambio ancho/alto en modo portrait) y ciclo de vida de reinicio en `reconfigurePipeline` para evitar conflicto de puertos.
- **iPhone / usbmuxd**: Eliminada la doble conexión de socket simultánea; ahora se mantiene un único forwarder TCP limpio hacia OBS (`127.0.0.1:50005`).
- **AirPlay / UxPlay**: Eliminado flag `-p` erróneo en modo cámara (que activaba puertos legacy en lugar de portrait) y resuelto congelamiento por saturación de buffer mediante drenado con `appendCapped`.
- **UI / Selectores iPhone**: Aislado el selector de modo (`#iphoneModeSelector`) para que la selección entre Pantalla/Cámara no deseleccione Cable USB/AirPlay.
- **UI / Botón Iniciar iPhone**: Preservado el id `btnStartIphoneLabel` al cambiar el texto a "Iniciando…" evitando que quede bloqueado en estado de carga.
- **ADB / Reinicio scrcpy**: Solucionada condición de carrera en la que el evento `exit` de un proceso anterior eliminaba la referencia del nuevo proceso activo.
- **Grabación**: Al cancelar el diálogo de guardado ya no se inicia scrcpy por error.
- **Estabilidad usbmux**: Manejo silencioso de errores de conexión sin romper el ciclo de sondeo cada 3 segundos.
- **Ajustes en vivo**: Implementado `settingsPendingCommit` para no perder configuraciones aplicadas mientras la cámara se está reiniciando.

### Añadido
- **Bandeja del sistema (System Tray / Iconos ocultos)**: Al minimizar o cerrar la ventana, la aplicación permanece activa en segundo plano en los iconos ocultos de Windows para mantener las sesiones de scrcpy o cámara sin ocupar la barra de tareas.
- **Menú contextual en la bandeja**: Opciones de mostrar ventana, ver contador de sesiones activas, detener todas las sesiones o salir por completo.
- **Bloqueo de instancia única**: Si la app ya está corriendo en segundo plano y se ejecuta de nuevo el acceso directo, enfoca y restaura la ventana existente en vez de abrir un proceso duplicado.
- **Rediseño completo de interfaz UX/UI**: Catálogo unificado de iconos vectoriales minimalistas SVG (reemplazando todos los emojis y glifos), guardado automático de ajustes con aviso discreto, selectores tipo chip/segmentados y tema oscuro completo.
- **Optimización de compilación**: Exclusión de duplicados en el empaquetado y compresión máxima, reduciendo el peso final del `.exe` de 276 MB a 180 MB.

### Seguridad y Rendimiento
- **Sanitización de comandos**: Sustitución de `exec(string)` por `execFile(cmd, [args])` con validación estricta de parámetros IP, puerto y serial para prevenir inyecciones de comandos.
- **Captura de pantalla**: Migrado a `adb exec-out screencap -p` con stream binario directo sin invocación de shell.
- **Paralelismo ADB**: Consultas de estado de dispositivo (`describeDevice`) paralelizadas con `Promise.all` mejorando notablemente el tiempo de respuesta.
- **Gestión de memoria**: Buffers de salida de procesos hijos acotados a un máximo de 16 KB.
- **Limpieza de código**: Eliminadas variables duplicadas y listeners IPC huérfanos (`onIphoneUsbVideoData`, `iphoneNameHintCam`).

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
