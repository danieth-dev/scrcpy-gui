# Correcciones y Mejoras Aplicadas — scrcpy GUI

## 🚀 Migración de Arquitectura: De Electron a Rust (Tauri v2)

| Componente | Estado Anterior | Estado Actual (Rust + Tauri v2) |
|---|---|---|
| **Backend Core** | `main.js` (1.154 líneas en Node.js) | Módulos Rust en `src-tauri/src/` (`tools.rs`, `adb.rs`, `scrcpy.rs`, `iphone.rs`, `commands.rs`, `lib.rs`) |
| **Puente IPC** | `preload.js` con `contextBridge` | `tauri-bridge.js` que emula de forma 100% transparente `window.api` mediante comandos de Tauri v2 |
| **Consumo de Memoria** | ~180 – 250 MB | **~30 – 35 MB** (reducción del ~85%) |
| **Tamaño de Binario** | ~180 MB | **~14 MB** standalone (`scrcpy-gui.exe`) |
| **Dependencias Node.js** | 315 paquetes (`electron`, `electron-builder`, Chromium) | 3 paquetes (únicamente `@tauri-apps/cli`) |
| **Búsqueda de Herramientas** | Búsqueda estática propensa a fallos | Búsqueda recursiva en ancestros de directorios (`ancestors()`) garantizando detección de `vendor/` |
| **Directorio de Trabajo** | Heredado del lanzador | `current_dir` fijado al directorio del ejecutable de scrcpy para cargar `scrcpy-server` y DLLs de video sin depender de PATH |

---

## Histórico de Bugs Corregidos (v1.3.1)

### 🔴 Críticos corregidos

#### C1 — Info.plist: app no arrancaba en iPhone
- `CFBundleExecutable` era `$(EXECUTABLE_NAME)` (variable de Xcode sin sustituir).
- **Fix:** hardcodeado a `ScrcpyCam`; añadidos `MinimumOSVersion` (16.0) y `UIDeviceFamily`.

#### C3 — Doble conexión USB al iPhone
- El backend abría un socket directo y un port-forwarder simultáneo al iPhone, que solo acepta 1 cliente.
- **Fix:** eliminada la conexión directa; ahora solo existe un forwarder TCP `tcp://127.0.0.1:50005` para OBS.

#### C4 — UxPlay: `-p` no es portrait
- El modo "Solo Cámara" pasaba `-p` creyendo que era portrait, pero `-p` activa puertos legacy.
- **Fix:** eliminado el argumento; AirPlay envía la orientación del iPhone directamente.

#### C5 — UxPlay se congelaba
- `stdio: 'pipe'` sin leer stdout/stderr provocaba saturación del buffer y deadlock.
- **Fix:** se drenan ambos streams de manera asíncrona con buffers acotados (máximo 16 KB).

---

### 🟠 Altos corregidos

#### A1 — Race condition al reiniciar scrcpy
- El evento de salida de un proceso anterior borraba del mapa la referencia del proceso nuevo.
- **Fix:** guard `SessionControl.generation` para verificar que el proceso que finaliza sea exactamente el proceso registrado.

#### A2 — Botones iPhone se desactivaban mutuamente
- Selectores del DOM globales afectaban simultáneamente a los botones de Pantalla/Cámara y USB/AirPlay.
- **Fix:** aislamiento con selector scoped `#iphoneModeSelector`.

#### A3 — Botón "Iniciar" se quedaba en "Iniciando…" para siempre
- Modificación destructiva de innerHTML borraba el `<span id="btnStartIphoneLabel">`.
- **Fix:** preservación estricta del ID del elemento span.

#### A4 — Grabar: cancelar diálogo igual lanzaba scrcpy
- **Fix:** control de cancelación explícita del diálogo de selección de archivo.

#### A5 — usbmux error sin listener provocaba excepciones
- En PCs sin servicio Apple Mobile Device se generaban excepciones continuas cada 3 segundos.
- **Fix:** manejo silencioso y controlado del sondeo de dispositivos.

#### A6 — Cambiar ajuste durante reinicio de cámara se descartaba
- **Fix:** cola de espera `settingsPendingCommit` que aplica automáticamente los ajustes diferidos una vez completado el ciclo de reinicio.
