# scrcpy Cam — App para iPhone (iOS)

App complementaria nativa para iOS diseñada específicamente para conectar tu iPhone a **scrcpy GUI** por **cable USB físico**, logrando **0 delay**, transmisión a **1080p / 4K a 60 fps** y modo de ahorro de pantalla OLED.

---

## 🚀 Características
* **0 Delay real:** Transmisión a través del bus físico USB usando `usbmuxd`.
* **Codificación H.264 por Hardware:** Impulsada por `VideoToolbox` (latencia < 2 ms).
* **Modo Pantalla Negra (OLED Save):** Un toque apaga los píxeles de la pantalla para evitar que el teléfono caliente o gaste batería mientras la cámara sigue transmitiendo.
* **Control de lentes:** Cambio entre cámara trasera y frontal, linterna y selector de resolución (720p, 1080p, 4K).

---

## 📲 Cómo instalar en tu iPhone

### Opción A: Desde Windows usando Sideloadly (Sin Mac, 100% Gratis)
1. Descarga e instala **[Sideloadly](https://sideloadly.io/)** en tu PC con Windows.
2. Conecta tu iPhone por cable USB a la PC.
3. Abre Sideloadly y selecciona el archivo `ScrcpyCam.ipa`.
4. Introduce tu Apple ID gratuito.
5. Pulsa **Start** y la app se instalará en tu iPhone.
6. En el iPhone: Ve a **Ajustes** → **General** → **VPN y gestión de dispositivos** → Toca tu correo y pulsa **Confiar**.

---

### Opción B: Desde una Mac con Xcode (Desarrolladores)
1. Abre esta carpeta en **Xcode**.
2. Conecta tu iPhone por cable a la Mac.
3. En *Signing & Capabilities*, selecciona tu equipo o cuenta de Apple ID personal.
4. Presiona el botón **Play (Run)** para compilar e instalar directamente en el iPhone.

---

## 🔌 Uso con scrcpy GUI
1. Abre **scrcpy GUI** en tu PC y selecciona la pestaña **iPhone**.
2. Elige el modo **Cable USB (Cero Latencia)**.
3. Abre la app **scrcpy Cam** en tu iPhone.
4. Pulsa **Iniciar Cámara USB** en tu PC.
5. *(Opcional)* Toca el botón **Apagar** en el iPhone para que la pantalla quede en negro mientras transmite.
