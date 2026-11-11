# scrcpy GUI

Interfaz gráfica nativa de alto rendimiento para [scrcpy](https://github.com/Genymobile/scrcpy): mirror de Android, cámara como webcam, conexión WiFi y presets de calidad.

> **Arquitectura nativa en Rust con Tauri v2.** Consume ~30 MB de RAM (un 85% menos que Electron) con arranque instantáneo. El motor de streaming sigue siendo scrcpy oficial (Genymobile, Apache-2.0).

![Demo de scrcpy GUI](docs/demo.gif)

## Características

- **Ultra ligero:** Backend nativo en Rust con Tauri v2 y WebView2 del sistema.
- **Dispositivos USB y WiFi:** Detección automática en paralelo, vinculación (`adb pair`), conexión y "Activar y conectar" (`:5555`).
- **Control de cámara y pantalla:** Mirror, modo webcam, captura de pantalla y grabación.
- **Presets de calidad:** Normal, Webcam, Streams y Gaming con aplicación de ajustes en vivo.
- **Ajustes avanzados:** Control de orientación, facing (frontal/trasera), zoom, linterna, bitrate, codec y FPS.
- **iPhone / AirPlay y USB:** Soporte para UxPlay y túnel USB directo hacia la app ScrcpyCam.
- **Bandeja del sistema (Tray):** Minimización a segundo plano en los iconos ocultos de Windows para mantener las sesiones activas.
- **Actualizaciones automáticas:** Chequeo directo con GitHub Releases.

## Métricas de rendimiento

| Métrica | Versión anterior (Electron) | Versión actual (Rust + Tauri v2) |
|---|---|---|
| **Memoria RAM en reposo** | ~180 – 250 MB | **~30 – 35 MB** (reducción del 85%) |
| **Tamaño del binario base** | ~180 MB | **~14 MB** |
| **Tiempo de arranque** | 1.8 – 2.5 s | **< 0.3 s (instantáneo)** |
| **Dependencias `node_modules`** | 315 paquetes | **3 paquetes** (sólo CLI de Tauri) |

## Requisitos

- Windows 10/11 x64 (con WebView2, instalado por defecto en el sistema).
- Teléfono Android con depuración USB activada.
- Cámara del teléfono como webcam: Android 12+ (soporte Camera2).
- Para desarrollo y compilación: [Rust 1.80+](https://rustup.rs/) y [Node.js 18+](https://nodejs.org/).

## Uso rápido (desarrollo)

```bash
cd gui
npm install
npm run vendor:sync -- "C:\ruta\a\scrcpy-win64-vX.Y"
npm run dev
```

O haciendo doble clic en **`Abrir_scrcpy_GUI.bat`** en la raíz del proyecto.

1. Conecta el teléfono por USB (depuración activa).  
2. **WiFi → Activar y conectar** (una sola vez).  
3. Desconecta el cable.  
4. **Recientes → Pantalla** o **Cámara**.

## Compilación de producción

Para generar el ejecutable optimizado (`.exe` standalone de ~14 MB):

```bash
npm run build
```

El ejecutable se genera en:
`gui/src-tauri/target/release/scrcpy-gui.exe`

## Publicar una nueva versión

Sigue la guía detallada en [docs/RELEASING.md](docs/RELEASING.md).

Los usuarios pueden comprobar versiones en **Ajustes → Buscar actualizaciones**.

### Repositorio oficial

[github.com/danieth-dev/scrcpy-gui](https://github.com/danieth-dev/scrcpy-gui)

## Licencia

Apache License 2.0 — ver [LICENSE](LICENSE) y [NOTICE](NOTICE).

scrcpy © Genymobile. Esta GUI © el autor indicado en `package.json`.
