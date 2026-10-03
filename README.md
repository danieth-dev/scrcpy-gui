# Scrcpy GUI

Interfaz gráfica para [scrcpy](https://github.com/Genymobile/scrcpy): mirror de Android, cámara como webcam, conexión WiFi y presets de calidad.

> **No es el proyecto oficial de scrcpy.** Es un frontend independiente. El motor sigue siendo scrcpy (Genymobile, Apache-2.0).

![Demo de scrcpy GUI](docs/demo.gif)

## Características

- Dispositivos USB y WiFi (vincular, conectar, Activar y conectar → `:5555`)
- Mirror, cámara, captura y grabación
- Presets: Normal, Webcam, Streams, Gaming (aplicación en vivo a la cámara)
- Facing, orientación, zoom, linterna
- Recientes con un clic (Mirror / Cámara)
- Errores de scrcpy visibles
- **Actualizaciones** vía GitHub Releases

## Requisitos

- Windows 10/11 x64  
- Teléfono Android con depuración USB  
- Cámara del teléfono: Android 12+  
- Para build: [Node.js](https://nodejs.org/) 18+ y el zip oficial [scrcpy-win64](https://github.com/Genymobile/scrcpy/releases)

## Uso rápido (desarrollo)

```bash
cd gui
npm install
npm run vendor:sync -- "C:\ruta\a\scrcpy-win64-vX.Y"
npm start
```

1. Conecta el teléfono por USB (depuración activa).  
2. **WiFi → Activar y conectar** (una vez).  
3. Quita el cable si quieres.  
4. **Recientes → Cámara** o Dispositivos → Cámara.

## Build portable

```bash
npm run vendor:sync -- "C:\ruta\scrcpy-win64-vX.Y"
npm run build
```

Salida en `dist/`.

## Actualizar la app (como autor)

Cuando hagas mejoras:

1. Sube la `version` en `package.json` (SemVer).  
2. Documenta en `CHANGELOG.md`.  
3. Commit + tag `vX.Y.Z` + push.  
4. `npm run build` y sube el `.exe` a un **GitHub Release**.  

Guía detallada: [docs/RELEASING.md](docs/RELEASING.md).

Los usuarios pulsan **Ajustes → Buscar actualizaciones** (o la app comprueba una vez al día).


### Repositorio oficial

[github.com/danieth-dev/scrcpy-gui](https://github.com/danieth-dev/scrcpy-gui)

## Licencia

Apache License 2.0 — ver [LICENSE](LICENSE) y [NOTICE](NOTICE).

scrcpy © Genymobile. Esta GUI © el autor indicado en `package.json`.
