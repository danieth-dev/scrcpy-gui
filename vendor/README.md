# Vendor scrcpy

Aquí debe vivir el contenido del release oficial **scrcpy-win64** (`scrcpy.exe`, `adb.exe`, DLLs, `scrcpy-server`, etc.).

```bash
npm run vendor:sync -- "C:\ruta\a\scrcpy-win64-vX.Y"
```

Los binarios están ignorados por git (ver `.gitignore`). Cada quien los genera en local o los incluye en el Release de GitHub dentro del portable.
