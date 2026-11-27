# Cómo publicar una versión

Los usuarios ven las versiones nuevas con **Ajustes › Acerca de › Buscar actualizaciones**. La app compara su versión con la del último release de GitHub.

## 1. Sube la versión (SemVer)

| Cambio | Ejemplo |
|---|---|
| Arreglo pequeño | `2.0.1` |
| Función nueva compatible | `2.1.0` |
| Cambio grande | `3.0.0` |

La versión está en **tres** archivos y tienen que coincidir:

- `package.json` → `"version"`
- `src-tauri/tauri.conf.json` → `"version"` (es la del instalador)
- `src-tauri/Cargo.toml` → `version` (es la que muestra la app y la que se usa para buscar actualizaciones)

Después añade la entrada en `CHANGELOG.md`.

## 2. Comprueba y genera el instalador

```bash
npm run vendor:sync -- "C:\ruta\a\scrcpy-win64-vX.Y"   # solo si cambias de versión de scrcpy
cd src-tauri && cargo test --release --lib && cd ..
npm run build
```

Se genera `src-tauri/target/release/bundle/nsis/scrcpy GUI_X.Y.Z_x64-setup.exe`. Este instalador:

- Instala para todos los usuarios (pide permisos de Administrador una vez).
- Copia `vendor/` (scrcpy, adb y el driver) junto al programa.
- Registra la cámara virtual **Unity Video Capture** de 32 y 64 bits, y la quita al desinstalar.
- Incluye `LICENSE`, `NOTICE` y `THIRD_PARTY_LICENSES.md`.

No hace falta `ffmpeg.exe`: la cámara virtual usa las DLL de FFmpeg que trae scrcpy. Si actualizas scrcpy y su FFmpeg cambia de versión mayor, la cámara virtual avisará y no arrancará hasta comprobar los offsets en `decoder.rs`.

Si cambias de versión de scrcpy, actualiza también la tabla de `THIRD_PARTY_LICENSES.md`.

## 3. Commit, etiqueta y release

```bash
git commit -am "Release vX.Y.Z"
git tag vX.Y.Z
git push && git push --tags
gh release create vX.Y.Z "src-tauri/target/release/bundle/nsis/scrcpy GUI_X.Y.Z_x64-setup.exe" --title "scrcpy GUI vX.Y.Z" --notes-file notas.md
```

También puedes hacerlo desde la web: GitHub › Releases › **Draft a new release**, eliges la etiqueta, pegas las notas del CHANGELOG y adjuntas el instalador.

## Checklist

- [ ] Versión igual en `package.json`, `tauri.conf.json` y `Cargo.toml`
- [ ] `CHANGELOG.md` actualizado
- [ ] `cargo test` en verde
- [ ] Instalador generado y probado
- [ ] Etiqueta `vX.Y.Z` y release con el instalador adjunto
