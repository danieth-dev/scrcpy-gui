# Licencias de terceros

scrcpy GUI es una interfaz **independiente y no oficial**. No está afiliada a
Genymobile ni a los autores de los componentes de abajo. El código propio de la
app se distribuye bajo la licencia Apache 2.0 (ver `LICENSE` y `NOTICE`).

El instalador incluye estos componentes de terceros sin modificar, en la
carpeta `vendor\`. Cada uno conserva su propia licencia. La app solo los
**ejecuta como programas aparte** o **carga sus DLL sin modificarlas**.

---

## scrcpy (`vendor\scrcpy\`)

- **Versión:** 4.1 (release oficial `scrcpy-win64`).
- **Autor:** Copyright (C) Genymobile y colaboradores.
- **Licencia:** Apache License 2.0. Texto completo en `vendor\scrcpy\LICENSE.txt`.
- **Código fuente:** <https://github.com/Genymobile/scrcpy>.

El release de scrcpy trae también estos componentes, cada uno con su licencia:

| Componente | Archivos | Licencia | Código fuente |
|---|---|---|---|
| Android Debug Bridge (platform-tools) | `adb.exe`, `AdbWinApi.dll`, `AdbWinUsbApi.dll` | Apache 2.0 | <https://android.googlesource.com/platform/packages/modules/adb> |
| FFmpeg | `avcodec-62.dll`, `avformat-62.dll`, `avutil-60.dll`, `swresample-6.dll` | LGPL 2.1 o posterior | <https://ffmpeg.org/download.html> (compilación: <https://github.com/Genymobile/scrcpy/tree/master/app/deps>) |
| SDL 3 | `SDL3.dll` | zlib | <https://github.com/libsdl-org/SDL> |
| libusb | `libusb-1.0.dll` | LGPL 2.1 o posterior | <https://github.com/libusb/libusb> |

Las DLL de FFmpeg se cargan dinámicamente y se pueden sustituir por otra
compilación compatible, como permite la LGPL.

---

## Unity Capture (`vendor\UnityCaptureFilter32.dll`, `vendor\UnityCaptureFilter64.dll`)

Es el driver DirectShow que crea la cámara virtual **"Unity Video Capture"**.

- **Código fuente:** <https://github.com/schellingb/UnityCapture>.
- **Licencia:** MIT (solo el filtro; el plugin de Unity no se incluye).

```
Unity Capture
Copyright (c) 2018 Bernhard Schelling

Based on UnityCam
https://github.com/mrayy/UnityCam
Copyright (c) 2016 MHD Yamen Saraiji

The MIT License (MIT)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

---

## UxPlay para Windows (`vendor\uxplay\`)

Es el receptor AirPlay que se usa para la función de iPhone.

- **Licencia:** GNU General Public License v3.0 (GPLv3). El detalle de sus
  componentes está en `vendor\uxplay\LICENSE.rtf`.
- **Código fuente:**
  - Port para Windows: <https://github.com/leapbtw/uxplay-windows> (el
    código de cada versión está en su página de releases).
  - Proyecto original: <https://github.com/FDH2/UxPlay>.
- **Componentes que incluye:**
  - GStreamer (LGPL).
  - Qt 6 (LGPL 3).
  - FFmpeg con x264 y x265 (GPL).
  - mDNSResponder (BSD de 3 cláusulas / Apache 2.0).
  - Otras librerías, cada una con su licencia.

UxPlay es un programa aparte que scrcpy GUI lanza como proceso independiente.
Se distribuye **sin modificar** junto a la app, como "agregado" en el sentido
de la GPLv3 (sección 5). Por eso la licencia GPL no se extiende al código de
scrcpy GUI.

**Oferta de código fuente:** si no puedes conseguir el código de la versión
incluida en los enlaces de arriba, abre un issue en
<https://github.com/danieth-dev/scrcpy-gui/issues> y te lo facilitaremos, como
exige la GPLv3, durante al menos tres años desde la publicación de cada
versión del instalador.

---

## Componentes de la propia app

El ejecutable `scrcpy-gui.exe` se compila con [Tauri](https://tauri.app)
(MIT / Apache 2.0) y con crates de Rust publicados en crates.io, casi todos
bajo licencias MIT y/o Apache 2.0. La lista exacta está en
`src-tauri/Cargo.lock` del repositorio.

La interfaz usa el WebView2 de Microsoft, que viene con Windows y no se
redistribuye.
