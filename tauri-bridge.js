/**
 * tauri-bridge.js
 * Adaptador de compatibilidad transparente para scrcpy GUI.
 * Emula la API de window.api expuesta originalmente por preload.js en Electron,
 * mapeando cada llamada a comandos y eventos nativos de Tauri v2.
 */

(function () {
  // Si window.api ya existe (ej. ejecutándose dentro de Electron), no hacemos nada
  if (window.api) return;

  const getTauri = () => window.__TAURI__;

  // Invocar comando Tauri de manera segura
  async function invoke(cmd, args = {}) {
    const t = getTauri();
    if (!t || !t.core || !t.core.invoke) {
      console.error(`[tauri-bridge] Tauri core no disponible para invocar: ${cmd}`);
      return { ok: false, error: 'Tauri core no inicializado' };
    }
    try {
      return await t.core.invoke(cmd, args);
    } catch (err) {
      console.error(`[tauri-bridge] Error en invoke(${cmd}):`, err);
      return { ok: false, error: typeof err === 'string' ? err : err.message || String(err) };
    }
  }

  // Suscribirse a eventos de Tauri
  function listenEvent(eventName, callback) {
    const t = getTauri();
    if (!t || !t.event || !t.event.listen) {
      console.warn(`[tauri-bridge] Tauri event listener no disponible para: ${eventName}`);
      return () => {};
    }

    let unlistenFn = null;
    let cancelled = false;

    t.event.listen(eventName, (event) => {
      if (!cancelled && typeof callback === 'function') {
        callback(event.payload);
      }
    }).then((unlisten) => {
      if (cancelled) {
        unlisten();
      } else {
        unlistenFn = unlisten;
      }
    }).catch((err) => {
      console.error(`[tauri-bridge] Error registrando listener para ${eventName}:`, err);
    });

    return () => {
      cancelled = true;
      if (unlistenFn) unlistenFn();
    };
  }

  window.api = {
    // ── Ventana (emulando el comportamiento de Electron) ───────────
    minimize: () => invoke('win_minimize'),
    maximize: () => invoke('win_maximize'),
    close:    () => invoke('win_close'),

    // ── Herramientas e Información de la App ───────────────────────
    getToolsStatus: () => invoke('get_tools_status'),
    getAppInfo:     () => invoke('get_app_info'),
    checkUpdates:   () => invoke('check_updates'),
    openExternal:   (url) => invoke('open_external', { url }),

    // ── ADB & Conectividad ─────────────────────────────────────────
    getDevices:   () => invoke('adb_get_devices'),
    pairWifi:     (host, port, code) => invoke('adb_pair', { host, port, code }),
    connectWifi:  (ip, port) => invoke('adb_connect', { ip, port: String(port) }),
    disconnect:   (serial)   => invoke('adb_disconnect', { serial }),
    enableTcpip:  (serial)   => invoke('adb_enable_tcpip', { serial }),

    // ── scrcpy Core ────────────────────────────────────────────────
    launchScrcpy: (serial, options) => invoke('scrcpy_launch', { serial, options }),
    stopScrcpy:   (serial) => invoke('scrcpy_stop', { serial }),
    getSessions:  () => invoke('scrcpy_get_sessions'),

    onScrcpyStopped: (cb) => listenEvent('scrcpy:stopped', cb),
    onScrcpyTexture: (cb) => listenEvent('scrcpy:texture', cb),

    // ── iPhone / AirPlay (UxPlay + OBS) ────────────────────────────
    iphoneToolsStatus:    ()     => invoke('iphone_tools_status'),
    iphoneStartUxPlay:    (opts) => invoke('iphone_start_uxplay', { opts }),
    iphoneStopUxPlay:     ()     => invoke('iphone_stop_uxplay'),
    iphoneObsVirtualCam:  (opts) => invoke('iphone_obs_virtual_cam', { opts }),
    onIphoneUxPlayStopped: (cb)  => listenEvent('iphone:uxplayStopped', cb),

    // ── iPhone / USB Direct Camera ─────────────────────────────────
    iphoneUsbStatus:       ()     => invoke('iphone_usb_status'),
    iphoneUsbStartStream:  (opts) => invoke('iphone_usb_start_stream', { opts }),
    iphoneUsbStopStream:   ()     => invoke('iphone_usb_stop_stream'),
    onIphoneUsbDeviceChange: (cb) => listenEvent('iphone:usbDeviceChange', cb),
    onIphoneUsbStats:        (cb) => listenEvent('iphone:usbStats', cb),

    // ── DirectShow Virtual Camera (Windows / OBS / Discord / Zoom) ──
    vcamGetStatus:     ()                => invoke('vcam_get_status'),
    vcamStart:         (serial, options) => invoke('vcam_start', { serial, options }),
    vcamStop:          ()                => invoke('vcam_stop'),
    vcamInstallDriver: ()                => invoke('vcam_install_driver'),
    deviceProfile:     (serial, options, target) => invoke('device_profile', { serial, options, target }),
    onVcamStats:       (cb)              => listenEvent('vcam:stats', cb),
    onVcamStopped:     (cb)              => listenEvent('vcam:stopped', cb),
    onDeviceHealth:    (cb)              => listenEvent('device:health', cb),
    onReconnecting:    (cb)              => { listenEvent('scrcpy:reconnecting', (d) => cb({ ...d, kind: 'window' })); listenEvent('vcam:reconnecting', (d) => cb({ ...d, kind: 'vcam' })); },
    onReconnected:     (cb)              => { listenEvent('scrcpy:reconnected', (d) => cb({ ...d, kind: 'window' })); listenEvent('vcam:reconnected', (d) => cb({ ...d, kind: 'vcam' })); },
    onDeviceThrottle:  (cb)              => listenEvent('device:throttle', cb),
    onDeviceAlert:     (cb)              => listenEvent('device:alert', cb),
  };


  console.log('[tauri-bridge] API de compatibilidad inicializada correctamente.');
})();
