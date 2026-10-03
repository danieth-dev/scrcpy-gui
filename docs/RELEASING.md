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

## 4. Build del .exe

```bash
npm run vendor:sync -- "C:\ruta\scrcpy-win64-vX.Y"
npm run build
```

El portable queda en `dist/`.

## 5. GitHub Release

1. GitHub → tu repo → **Releases** → **Draft a new release**  
2. Tag: `v1.1.0` (igual que en package.json)  
3. Título / notas (pega desde CHANGELOG)  
4. Adjunta el `.exe` de `dist/`  
5. **Publish release**

A partir de ahí, la app detecta la nueva versión al abrir (1 vez al día) o con el botón **Buscar actualizaciones**.

## Checklist rápido

- [ ] `package.json` version subida  
- [ ] `CHANGELOG.md` actualizado  
- [ ] Tag `vX.Y.Z`  
- [ ] Build portable OK  
- [ ] Release en GitHub con el `.exe`  
