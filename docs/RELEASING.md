# Cómo publicar una mejora (actualización)

Cada vez que mejores la app, sigue este flujo. Los usuarios verán el aviso en **Ajustes → Buscar actualizaciones**.

## 1. Identidad ya configurada

El `package.json` ya tiene tus datos:

- **owner**: `danieth-dev`
- **author**: `danieth`
- **repo**: `scrcpy-gui`

Configura tu git local (una vez, si no lo has hecho):

```bash
git config user.name "danieth"
git config user.email "tu@email.com"
```

## 2. Sube el código

```bash
cd gui
git init   # solo la primera vez si el repo es nuevo
git add .
git commit -m "Describe la mejora"
git remote add origin https://github.com/danieth-dev/scrcpy-gui.git
git push -u origin main
```

## 3. Sube la versión (SemVer)

| Cambio | Versión |
|--------|---------|
| Arreglo pequeño | `1.0.1` |
| Feature nueva compatible | `1.1.0` |
| Cambio grande / rompe algo | `2.0.0` |

1. Edita `version` en `package.json`  
2. Añade entrada en `CHANGELOG.md`  
3. Commit: `git commit -am "Release v1.1.0"`  
4. Tag: `git tag v1.1.0`  
5. `git push && git push --tags`

## 4. Build del ejecutable (.exe en Rust)

```bash
cd gui
npm run vendor:sync -- "C:\ruta\scrcpy-win64-vX.Y"
npm run build
```

Se generan:

- **Instalador (recomendado para usuarios):**
  `src-tauri/target/release/bundle/nsis/scrcpy GUI_X.Y.Z_x64-setup.exe`
  Instala para todos los usuarios (pide permisos de administrador una vez),
  copia `vendor/` (scrcpy, adb, driver) junto al programa y **registra la
  cámara virtual "Unity Video Capture" automáticamente**. Al desinstalar, la
  quita.
- **Ejecutable suelto (desarrollo):** `src-tauri/target/release/scrcpy-gui.exe`
  (~15 MB; necesita la carpeta `vendor/` cerca).

No hace falta `ffmpeg.exe`: la cámara virtual decodifica en proceso con las
DLL de FFmpeg que ya trae el release de scrcpy (`avcodec-*.dll`, `avutil-*.dll`).

> Tamaño: `vendor/uxplay` (≈256 MB) es la mayor parte del instalador. Si no
> se distribuye la función de iPhone/AirPlay, quitarlo reduce mucho la descarga.

## 5. GitHub Release

1. GitHub → tu repo → **Releases** → **Draft a new release**  
2. Tag: `vX.Y.Z` (igual que en package.json)  
3. Título / notas (pega desde CHANGELOG)  
4. Adjunta el instalador `src-tauri/target/release/bundle/nsis/scrcpy GUI_X.Y.Z_x64-setup.exe`  
5. **Publish release**

A partir de ahí, la app detecta la nueva versión al abrir (1 vez al día) o con el botón **Buscar actualizaciones**.

## Checklist rápido

- [ ] `package.json` version subida  
- [ ] `CHANGELOG.md` actualizado  
- [ ] Tag `vX.Y.Z`  
- [ ] Build portable OK  
- [ ] Release en GitHub con el `.exe`  
