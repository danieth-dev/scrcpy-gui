/**
 * renderer.js — scrcpy GUI
 * Lógica de frontend para el nuevo diseño (Tauri v2 + Rust)
 */

(function () {
  'use strict';

  // ── 1. Estado de la Aplicación ─────────────────────────────────────
  const state = {
    devices: [],
    sessions: {},         // serial -> { serial, mode, startedAt, resolution, fps, pid }
    vcam: { running: false, serial: null, isDriverInstalled: false, fps: 0 },
    selectedDevice: null,
    settings: loadSettings(),
    recents: loadRecents(),
    iphoneMode: 'usb',    // 'usb' | 'air'
    iphoneRunning: false,
    pollTimer: null,
    clockTimer: null,
  };


  // ── 2. Selectores y DOM ────────────────────────────────────────────
  const $ = (id) => document.getElementById(id);
  const $$ = (sel) => document.querySelectorAll(sel);

  // Nav y Vistas
  const navButtons = $$('.nav');
  const views = $$('.view');
  const sessionsBadge = $('sessionsBadge');
  const appVersion = $('appVersion');

  // Titlebar
  const toolStatusPill = $('toolStatusPill');
  const toolStatusText = $('toolStatusText');
  const btnMin = $('btnMin');
  const btnMax = $('btnMax');
  const btnClose = $('btnClose');

  // Dispositivos
  const viewDevices = $('viewDevices');
  const devicesList = $('devicesList');
  const devicesEmpty = $('devicesEmpty');
  const devicesAlert = $('devicesAlert');
  const devicesAlertTitle = $('devicesAlertTitle');
  const devicesAlertMsg = $('devicesAlertMsg');
  const devicesAlertDetails = $('devicesAlertDetails');
  const devicesAlertPre = $('devicesAlertPre');
  const btnRescan = $('btnRescan');
  const btnRefreshEmpty = $('btnRefreshEmpty');

  // Wi-Fi
  const usbDeviceSelect = $('usbDeviceSelect');
  const btnEnableTcpip = $('btnEnableTcpip');
  const pairEndpoint = $('pairEndpoint');
  const pairCode = $('pairCode');
  const btnPairWifi = $('btnPairWifi');
  const wifiEndpoint = $('wifiEndpoint');
  const btnConnectWifi = $('btnConnectWifi');
  const recentsList = $('recentsList');
  const recentsEmpty = $('recentsEmpty');

  // En uso
  const sessionsList = $('sessionsList');
  const sessionsEmpty = $('sessionsEmpty');
  const btnStopAll = $('btnStopAll');

  // iPhone
  const iphoneModeRadios = $$('input[name="iphoneMode"]');
  const modeTitle = $('mode-title');
  const modeSub = $('mode-sub');
  const probeT = $('probe-t');
  const probeS = $('probe-s');
  const iphoneStateBadge = $('iphoneStateBadge');
  const iphoneStateText = $('iphoneStateText');
  const btnStartIphone = $('btnStartIphone');
  const btnStartIphoneLabel = $('btnStartIphoneLabel');
  const chkObsVirtualCam = $('chkObsVirtualCam');

  // Ajustes
  const presetRadios = $$('input[name="preset"]');
  const selCamRes = $('camRes');
  const selScrRes = $('screenRes');
  const camFpsBox = $('camFps');
  const scrFpsBox = $('screenFps');
  const panelImage = $('panelImage');
  const autoBadge = $('autoBadge');
  const capsHint = $('capsHint');
  const specMax = $('specMax');
  const sliderBr = $('br');
  const sliderZoom = $('zoom');
  const codecRadios = $$('input[name="codec"]');
  const facingRadios = $$('input[name="facing"]');
  const rotRadios = $$('input[name="rot"]');
  const chkTorch = $('cameraTorch');
  const chkNoAudio = $('noAudio');
  const chkStayAwake = $('stayAwake');
  const chkScreenOff = $('screenOff');
  const chkProtection = $('deviceProtection');
  const btnCheckUpdates = $('btnCheckUpdates');
  const btnOpenGithub = $('btnOpenGithub');
  const btnResetSettings = $('btnResetSettings');

  // Modal y Toasts
  const deviceModal = $('deviceModal');
  const modalClose = $('modalClose');
  const modalDeviceTitle = $('modalDeviceTitle');
  const modalDeviceInfo = $('modalDeviceInfo');
  const modalLaunch = $('modalLaunch');
  const modalCamera = $('modalCamera');
  const modalVirtualCamera = $('modalVirtualCamera');
  const modalDisconnect = $('modalDisconnect');
  const vcamDriverStatus = $('vcamDriverStatus');
  const btnInstallVcam = $('btnInstallVcam');
  const toastContainer = $('toastContainer');


  // ── 3. Persistencia de Ajustes y Recientes ─────────────────────────
  function defaultSettings() {
    return {
      quality: 'max',      // 'max' = lo mejor que dé el teléfono; 'custom' = elegido
      preset: 'max',
      maxSize: 0,          // cámara: altura (0 = máxima del teléfono)
      maxFps: 30,          // cámara
      screenRes: 0,        // pantalla: lado corto (0 = nativa)
      screenFps: 60,       // pantalla
      bitrate: 20,
      codec: 'h264',
      noAudio: true,
      stayAwake: true,
      screenOff: true,
      cameraFacing: 'front',
      orientation: '0',
      cameraZoom: 1,
      cameraTorch: false,
      deviceProtection: true,
    };
  }

  function loadSettings() {
    try {
      const raw = localStorage.getItem('scrcpy_gui_settings');
      if (!raw) return defaultSettings();
      const parsed = JSON.parse(raw);
      const merged = { ...defaultSettings(), ...parsed };
      if (!('quality' in parsed)) {
        // Ajustes de antes de separar Cámara/Pantalla: se respetan tal cual.
        merged.quality = 'custom';
        merged.preset = null;
        merged.screenRes = 1080;
        merged.screenFps = parsed.maxFps || 60;
      }
      return merged;
    } catch (_) {
      return defaultSettings();
    }
  }

  function saveSettings() {
    try {
      localStorage.setItem('scrcpy_gui_settings', JSON.stringify(state.settings));
    } catch (_) {}
  }

  function loadRecents() {
    try {
      const raw = localStorage.getItem('scrcpy_gui_recents');
      return raw ? JSON.parse(raw) : [];
    } catch (_) {
      return [];
    }
  }

  function saveRecents() {
    try {
      localStorage.setItem('scrcpy_gui_recents', JSON.stringify(state.recents.slice(0, 8)));
    } catch (_) {}
  }

  function rememberDevice(dev) {
    if (!dev || !dev.serial) return;
    state.recents = state.recents.filter((r) => r.serial !== dev.serial);
    state.recents.unshift({
      serial: dev.serial,
      model: dev.model || dev.serial,
      brand: dev.brand || 'Android',
      isWifi: !!dev.isWifi,
      ip: dev.ip || (dev.serial.includes(':') ? dev.serial.split(':')[0] : ''),
      lastSeen: Date.now(),
    });
    saveRecents();
    renderRecents();
  }

  // ── 4. Notificaciones Toast ───────────────────────────────────────
  function showToast(message, type = 'info') {
    if (!toastContainer) return;
    const toast = document.createElement('div');
    toast.className = `toast ${type}`;
    toast.textContent = message;
    toastContainer.appendChild(toast);
    setTimeout(() => {
      toast.style.transition = 'opacity .3s, transform .3s';
      toast.style.opacity = '0';
      toast.style.transform = 'translateY(10px)';
      setTimeout(() => toast.remove(), 300);
    }, 3500);
  }

  function showAlert(title, msg, details) {
    if (!devicesAlert) return;
    devicesAlertTitle.textContent = title;
    devicesAlertMsg.textContent = msg;
    if (details) {
      devicesAlertPre.textContent = details;
      devicesAlertDetails.style.display = 'block';
    } else {
      devicesAlertDetails.style.display = 'none';
    }
    devicesAlert.style.display = 'flex';
  }

  function hideAlert() {
    if (devicesAlert) devicesAlert.style.display = 'none';
  }

  // ── 5. Formateadores y Utilidades ─────────────────────────────────
  function escHtml(str) {
    return String(str || '')
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  function renderBattBar(levelStr) {
    const level = parseInt(levelStr, 10) || 0;
    const filled = Math.min(Math.max(Math.round(level / 20), 0), 5);
    let html = '';
    for (let i = 0; i < 5; i++) {
      html += i < filled ? '<span></span>' : '<span class="off"></span>';
    }
    return html;
  }

  function formatDuration(startedAt) {
    if (!startedAt) return '00:00:00';
    const totalSec = Math.floor((Date.now() - startedAt) / 1000);
    const hrs = String(Math.floor(totalSec / 3600)).padStart(2, '0');
    const mins = String(Math.floor((totalSec % 3600) / 60)).padStart(2, '0');
    const secs = String(totalSec % 60).padStart(2, '0');
    return `${hrs}:${mins}:${secs}`;
  }

  function paintSlider(range) {
    if (!range) return;
    const min = parseFloat(range.min) || 0;
    const max = parseFloat(range.max) || 100;
    const val = parseFloat(range.value) || 0;
    const pct = ((val - min) / (max - min)) * 100;
    range.style.setProperty('--fill', `${pct}%`);
    const output = range.parentElement.querySelector('output');
    if (output) {
      const dec = parseInt(range.dataset.dec, 10) || 0;
      const unit = range.dataset.unit || '';
      output.textContent = `${val.toFixed(dec)}${unit}`;
    }
  }

  // ── 6. Navegación por Pestañas ─────────────────────────────────────
  function setupNavigation() {
    navButtons.forEach((btn) => {
      btn.addEventListener('click', () => {
        const targetView = btn.dataset.view;
        navButtons.forEach((b) => b.removeAttribute('aria-current'));
        btn.setAttribute('aria-current', 'page');
        views.forEach((v) => {
          v.hidden = v.dataset.view !== targetView;
        });
      });
    });
  }

  // ── 7. Titlebar & Control de Ventana ───────────────────────────────
  function setupTitlebar() {
    if (btnMin) btnMin.addEventListener('click', () => window.api.minimize());
    if (btnMax) btnMax.addEventListener('click', () => window.api.maximize());
    if (btnClose) btnClose.addEventListener('click', () => window.api.close());

    updateToolStatus();
  }

  async function updateToolStatus() {
    try {
      const status = await window.api.getToolsStatus();
      if (status.scrcpy && status.adb) {
        toolStatusPill.dataset.state = 'ok';
        toolStatusText.textContent = 'Todo listo';
        toolStatusPill.title = 'scrcpy y ADB funcionando';
      } else if (status.adb && !status.scrcpy) {
        toolStatusPill.dataset.state = 'warn';
        toolStatusText.textContent = 'Falta scrcpy';
        toolStatusPill.title = 'No se encontró scrcpy.exe en vendor/scrcpy/ ni en PATH';
      } else {
        toolStatusPill.dataset.state = 'error';
        toolStatusText.textContent = 'Falta ADB';
        toolStatusPill.title = 'No se encontró adb.exe';
      }
    } catch (_) {
      toolStatusPill.dataset.state = 'warn';
      toolStatusText.textContent = 'Desconectado';
    }
  }

  // ── 8. Renderizado de Dispositivos ────────────────────────────────
  function renderDeviceCards() {
    if (!devicesList) return;
    devicesList.innerHTML = '';

    const devices = state.devices;
    const hasDevices = devices && devices.length > 0;

    if (!hasDevices) {
      viewDevices.classList.add('is-empty');
      return;
    }

    viewDevices.classList.remove('is-empty');

    devices.forEach((dev) => {
      const isRunning = !!state.sessions[dev.serial];
      const isVcamRunning = state.vcam && state.vcam.running && state.vcam.serial === dev.serial;
      const session = state.sessions[dev.serial] || {};
      const card = document.createElement('article');
      card.className = 'device';
      card.dataset.serial = dev.serial;
      card.dataset.live = (isRunning || isVcamRunning) ? 'true' : 'false';

      card.innerHTML = `
        <div class="device-top">
          <div class="thumb"><svg class="ico"><use href="#i-phone"/></svg></div>
          <div>
            <h3>${escHtml(dev.brand ? `${dev.brand} ${dev.model}` : dev.model || dev.serial)}</h3>
            <span class="serial mono">${escHtml(dev.serial)}</span>
          </div>
          <button class="btn btn-icon btn-ghost btn-more" aria-label="Más opciones" title="Más opciones">
            <svg class="ico"><use href="#i-more"/></svg>
          </button>
        </div>
        <div class="meta">
          <span class="chip">
            <svg class="ico"><use href="${dev.isWifi ? '#i-wifi' : '#i-usb'}"/></svg>
            ${dev.isWifi ? 'Wi-Fi' : 'USB'}
          </span>
          ${dev.androidVersion ? `<span class="chip">Android ${escHtml(dev.androidVersion)}</span>` : ''}
          ${dev.batteryLevel ? `
          <span class="batt">
            <span class="batt-bar" aria-hidden="true">${renderBattBar(dev.batteryLevel)}</span>
            ${escHtml(dev.batteryLevel)}%
          </span>` : ''}
          ${isVcamRunning ? `<span class="chip" style="color:var(--accent);border-color:var(--accent)">DirectShow OBS Activo</span>` : ''}
          ${renderHealthChip(dev.serial)}
          ${(state.reconnecting || {})[dev.serial] ? `<span class="chip temp warm">Reconectando…</span>` : ''}
        </div>
        <div class="live-strip">
          <span class="tag"><span class="led"></span>${isVcamRunning ? 'Webcam OBS' : 'En vivo'}</span>
          <span class="readout mono">
            <span class="session-res">${isVcamRunning ? (state.vcam.resolution || '1920x1080').replace('x', '×') : (session.resolution || '1080×2400')}</span>
            <span${isVcamRunning ? ` class="vcam-fps" title="Decodificados: ${state.vcam.decodedFps ?? '—'} fps · Descartados (2 s): ${state.vcam.dropped ?? '—'}"` : ''}>${isVcamRunning ? (state.vcam.fps ? state.vcam.fps : '—') : (session.fps || state.settings.maxFps || 30)} fps</span>
            <span class="session-timer">${formatDuration(session.startedAt)}</span>
          </span>
        </div>
        <div class="actions">
          <button class="btn btn-primary go">
            <svg class="ico"><use href="#i-monitor"/></svg>Ver pantalla
          </button>
          <button class="btn btn-danger stop">
            <svg class="ico"><use href="#i-stop"/></svg>Detener
          </button>
          <button class="btn btn-icon btn-cam" aria-label="Cámara en ventana" title="Cámara en ventana">
            <svg class="ico"><use href="#i-camera"/></svg>
          </button>
          <button class="btn btn-icon btn-vcam ${isVcamRunning ? 'btn-danger' : ''}" aria-label="Cámara Virtual DirectShow (OBS / Windows)" title="${isVcamRunning ? 'Detener Cámara Virtual DirectShow' : 'Iniciar Cámara Virtual DirectShow (OBS / Windows)'}">
            <svg class="ico"><use href="${isVcamRunning ? '#i-stop' : '#i-vcam'}"/></svg>
          </button>
        </div>
      `;

      // Eventos directos
      card.querySelector('.go').addEventListener('click', () => launchSession(dev.serial, 'mirror'));
      card.querySelector('.stop').addEventListener('click', () => stopSession(dev.serial));
      card.querySelector('.btn-cam').addEventListener('click', () => launchSession(dev.serial, 'camera'));
      card.querySelector('.btn-vcam').addEventListener('click', () => toggleVirtualCam(dev.serial));
      card.querySelector('.btn-more').addEventListener('click', () => openDeviceModal(dev));

      devicesList.appendChild(card);
    });


    populateUsbSelect();
  }

  function populateUsbSelect() {
    if (!usbDeviceSelect) return;
    const currentVal = usbDeviceSelect.value;
    usbDeviceSelect.innerHTML = '<option value="">Elige un teléfono conectado por cable</option>';

    const usbDevs = state.devices.filter((d) => !d.isWifi);
    usbDevs.forEach((d) => {
      const opt = document.createElement('option');
      opt.value = d.serial;
      opt.textContent = `${d.brand || ''} ${d.model || d.serial} (${d.serial})`.trim();
      usbDeviceSelect.appendChild(opt);
    });

    if (usbDevs.length > 0) {
      usbDeviceSelect.value = currentVal || usbDevs[0].serial;
    }
  }

  // ── 9. En Uso (Sesiones Activas) ──────────────────────────────────
  function renderSessionsUI() {
    if (!sessionsList) return;
    sessionsList.innerHTML = '';

    const sessionSerials = Object.keys(state.sessions);
    const count = sessionSerials.length;

    // Actualizar badge en la navegación
    if (sessionsBadge) {
      sessionsBadge.textContent = count;
      sessionsBadge.style.display = count > 0 ? 'grid' : 'none';
    }

    if (count === 0) {
      if (sessionsEmpty) sessionsEmpty.style.display = 'block';
      if (btnStopAll) btnStopAll.style.display = 'none';
      return;
    }

    if (sessionsEmpty) sessionsEmpty.style.display = 'none';
    if (btnStopAll) btnStopAll.style.display = 'inline-flex';

    sessionSerials.forEach((serial) => {
      const session = state.sessions[serial];
      const dev = state.devices.find((d) => d.serial === serial) || { serial };
      const isCam = session.mode === 'camera';

      const card = document.createElement('article');
      card.className = 'device session';
      card.dataset.live = 'true';
      card.innerHTML = `
        <div class="device-top">
          <div class="thumb"><svg class="ico"><use href="#i-phone"/></svg></div>
          <div>
            <h3>${escHtml(dev.brand ? `${dev.brand} ${dev.model}` : dev.model || serial)}</h3>
            <span class="serial mono">${escHtml(serial)}</span>
          </div>
        </div>
        <div class="meta">
          <span class="chip">
            <svg class="ico"><use href="${isCam ? '#i-camera' : '#i-monitor'}"/></svg>
            ${isCam ? 'Cámara' : 'Pantalla'}
          </span>
          <span class="chip">
            <svg class="ico"><use href="${dev.isWifi ? '#i-wifi' : '#i-usb'}"/></svg>
            ${dev.isWifi ? 'Wi-Fi' : 'USB'}
          </span>
        </div>
        <div class="live-strip" style="display:flex">
          <span class="tag"><span class="led"></span>En vivo</span>
          <span class="readout mono">
            <span class="session-res">${session.resolution || '1080×2400'}</span>
            <span>${session.fps || state.settings.maxFps || 30} fps</span>
            <span class="session-timer">${formatDuration(session.startedAt)}</span>
          </span>
        </div>
        <div class="actions">
          <button class="btn btn-danger stop">
            <svg class="ico"><use href="#i-stop"/></svg>Detener
          </button>
        </div>
      `;

      card.querySelector('.stop').addEventListener('click', () => stopSession(serial));
      sessionsList.appendChild(card);
    });
  }

  // ── 10. Lista de Dispositivos Recientes (Wi-Fi) ───────────────────
  function renderRecents() {
    if (!recentsList) return;
    recentsList.innerHTML = '';

    if (!state.recents || state.recents.length === 0) {
      if (recentsEmpty) recentsEmpty.style.display = 'block';
      return;
    }

    if (recentsEmpty) recentsEmpty.style.display = 'none';

    state.recents.forEach((item) => {
      const div = document.createElement('div');
      div.className = 'recent-item';
      div.innerHTML = `
        <svg class="ico"><use href="#i-wifi"/></svg>
        <b>${escHtml(item.model || item.serial)}</b>
        <span class="mono">${escHtml(item.serial)}</span>
        <button class="btn btn-reconnect">Reconectar</button>
      `;

      div.querySelector('.btn-reconnect').addEventListener('click', async () => {
        const target = item.serial;
        const [ip, port] = target.split(':');
        showToast(`Conectando a ${item.model || target}…`, 'info');
        const res = await window.api.connectWifi(ip, port || '5555');
        if (res.ok) {
          showToast(`Conectado a ${item.model || target}`, 'success');
          refreshDevices();
        } else {
          showToast(`Error al conectar: ${res.error || res.message}`, 'error');
        }
      });

      recentsList.appendChild(div);
    });
  }

  // ── 11. Lanzamiento y Control de scrcpy ────────────────────────────
  function getLaunchOptions(mode = 'mirror') {
    const s = state.settings;
    const isCamera = mode === 'camera';
    return {
      windowTitle: isCamera ? 'scrcpy Cámara' : 'scrcpy',
      cameraOnly: isCamera,
      cameraFacing: s.cameraFacing || 'front',
      maxFps: s.maxFps || 30,
      maxSize: s.maxSize ?? 1080, // 0 = Original (no usar ||)
      cameraZoom: s.cameraZoom || 1,
      cameraTorch: !!s.cameraTorch,
      noAudio: isCamera ? true : !!s.noAudio,
      stayAwake: !!s.stayAwake,
      screenOff: !!s.screenOff,
      bitrate: s.bitrate || 8,
      codec: s.codec || 'h264',
      orientation: s.orientation || '0',
      screenRes: s.screenRes ?? 0,
      screenFps: s.screenFps || 60,
      quality: s.quality || 'custom',
    };
  }

  async function launchSession(serial, mode = 'mirror') {
    const dev = state.devices.find((d) => d.serial === serial);
    const opts = getLaunchOptions(mode);

    showToast(`Iniciando ${mode === 'camera' ? 'cámara' : 'pantalla'}…`, 'info');
    hideAlert();

    const res = await window.api.launchScrcpy(serial, opts);
    if (res.ok) {
      const p = res.profile;
      const sp = res.screenProfile;
      const rot = p && (p.orientation === 90 || p.orientation === 270);
      state.sessions[serial] = {
        serial,
        mode,
        startedAt: Date.now(),
        resolution: p ? (rot ? `${p.height}×${p.width}` : `${p.width}×${p.height}`)
          : (sp && sp.width ? `${sp.width}×${sp.height}` : 'nativa'),
        fps: p ? p.fps : (sp ? sp.fps : opts.screenFps),
        pid: res.pid,
      };
      if (dev) rememberDevice(dev);
      if (p) {
        showProfileNotes(p, 'Cámara lista');
      } else if (sp) {
        showToast(`Pantalla lista: ${describeScreen(sp)}${sp.notes && sp.notes.length ? '. ' + sp.notes.join('. ') : ''}`,
          sp.notes && sp.notes.length ? 'warn' : 'success');
      } else {
        showToast(`${mode === 'camera' ? 'Cámara' : 'Pantalla'} lista`, 'success');
      }
      renderDeviceCards();
      renderSessionsUI();
    } else {
      showToast(`Error al iniciar scrcpy: ${res.error}`, 'error');
      showAlert('Fallo al iniciar sesión en el teléfono', res.error, res.error);
    }
  }

  async function stopSession(serial) {
    delete state.sessions[serial];
    renderDeviceCards();
    renderSessionsUI();
    await window.api.stopScrcpy(serial);
    showToast('Sesión detenida', 'info');
  }

  async function stopAllSessions() {
    const serials = Object.keys(state.sessions);
    state.sessions = {};
    renderDeviceCards();
    renderSessionsUI();
    for (const s of serials) {
      await window.api.stopScrcpy(s);
    }
    showToast('Todas las sesiones detenidas', 'info');
  }

  // ── 12.5 Cámara Virtual DirectShow (OBS / Windows) ─────────────────
  async function updateVcamStatus() {
    try {
      const status = await window.api.vcamGetStatus();
      if (!status) return;
      state.vcam.running = !!status.running;
      state.vcam.serial = status.serial || null;
      state.vcam.isDriverInstalled = !!status.isDriverInstalled;
      state.vcam.fps = status.fps || 0;

      if (vcamDriverStatus) {
        if (state.vcam.isDriverInstalled) {
          vcamDriverStatus.className = 'chip';
          vcamDriverStatus.style.color = 'var(--primary)';
          vcamDriverStatus.style.borderColor = 'var(--primary)';
          vcamDriverStatus.textContent = 'Instalado ("Unity Video Capture")';
          if (btnInstallVcam) btnInstallVcam.textContent = 'Reinstalar Driver';
        } else {
          vcamDriverStatus.className = 'chip';
          vcamDriverStatus.style.color = 'var(--warn)';
          vcamDriverStatus.style.borderColor = 'var(--warn)';
          vcamDriverStatus.textContent = 'No instalado';
          if (btnInstallVcam) btnInstallVcam.textContent = 'Instalar Driver DirectShow';
        }
      }
      renderDeviceCards();
    } catch (err) {
      console.error('[vcam] Error al obtener estado:', err);
    }
  }

  async function toggleVirtualCam(serial) {
    if (state.vcam.running && state.vcam.serial === serial) {
      await window.api.vcamStop();
      state.vcam.running = false;
      state.vcam.serial = null;
      renderDeviceCards();
      showToast('Cámara Virtual detenida', 'info');
      return;
    }

    if (!state.vcam.isDriverInstalled) {
      const ok = confirm(
        'El Driver DirectShow no está instalado en Windows aún.\n\n' +
        '¿Deseas registrarlo ahora? (Aparecerá el aviso de Administrador de Windows para registrar "Unity Video Capture")'
      );
      if (ok) {
        await window.api.vcamInstallDriver();
        showToast('Abriendo instalador del driver con permisos de Administrador…', 'info');
        setTimeout(updateVcamStatus, 4000);
      }
      return;
    }

    showToast('Iniciando Cámara Virtual DirectShow…', 'info');
    state.vcam.fps = 0;
    state.vcam.decodedFps = undefined;
    state.vcam.dropped = undefined;
    // Mismos ajustes que la cámara en ventana; el backend los adapta al teléfono.
    const res = await window.api.vcamStart(serial, getLaunchOptions('camera'));

    if (res && res.ok) {
      state.vcam.running = true;
      state.vcam.serial = serial;
      state.vcam.profile = res.profile || null;
      if (res.profile) {
        const p = res.profile;
        const rot = p.orientation === 90 || p.orientation === 270;
        state.vcam.resolution = rot ? `${p.height}x${p.width}` : `${p.width}x${p.height}`;
      }
      renderDeviceCards();
      showProfileNotes(res.profile, 'Cámara Virtual activa en "Unity Video Capture"');
    } else {
      showToast('Error iniciando cámara virtual: ' + (res?.error || 'Desconocido'), 'error');
    }
  }

  // ── 13. Modal de Dispositivo ───────────────────────────────────────

  function openDeviceModal(dev) {
    state.selectedDevice = dev;
    if (!deviceModal) return;

    modalDeviceTitle.textContent = `${dev.brand || ''} ${dev.model || 'Dispositivo'}`.trim();
    modalDeviceInfo.innerHTML = `
      <div style="display:flex;align-items:center;gap:var(--s3);margin:var(--s2) 0">
        <div class="thumb"><svg class="ico"><use href="#i-phone"/></svg></div>
        <div>
          <b style="font-size:14px">${escHtml(dev.model || dev.serial)}</b>
          <p class="mono" style="color:var(--text-3);margin-top:2px">${escHtml(dev.serial)}</p>
        </div>
      </div>
      <div class="meta" style="margin-bottom:var(--s3)">
        <span class="chip"><svg class="ico"><use href="${dev.isWifi ? '#i-wifi' : '#i-usb'}"/></svg>${dev.isWifi ? 'Wi-Fi' : 'USB'}</span>
        ${dev.androidVersion ? `<span class="chip">Android ${escHtml(dev.androidVersion)}</span>` : ''}
        ${dev.batteryLevel ? `<span class="chip">Batería ${escHtml(dev.batteryLevel)}%</span>` : ''}
      </div>
    `;

    deviceModal.classList.add('active');
  }

  function closeDeviceModal() {
    if (deviceModal) deviceModal.classList.remove('active');
  }

  function setupModal() {
    if (modalClose) modalClose.addEventListener('click', closeDeviceModal);
    if (deviceModal) {
      deviceModal.addEventListener('click', (e) => {
        if (e.target === deviceModal) closeDeviceModal();
      });
    }

    if (modalLaunch) {
      modalLaunch.addEventListener('click', () => {
        if (state.selectedDevice) {
          closeDeviceModal();
          launchSession(state.selectedDevice.serial, 'mirror');
        }
      });
    }

    if (modalCamera) {
      modalCamera.addEventListener('click', () => {
        if (state.selectedDevice) {
          closeDeviceModal();
          launchSession(state.selectedDevice.serial, 'camera');
        }
      });
    }

    if (modalVirtualCamera) {
      modalVirtualCamera.addEventListener('click', () => {
        if (state.selectedDevice) {
          const serial = state.selectedDevice.serial;
          closeDeviceModal();
          toggleVirtualCam(serial);
        }
      });
    }


    if (modalDisconnect) {
      modalDisconnect.addEventListener('click', async () => {
        if (state.selectedDevice) {
          const serial = state.selectedDevice.serial;
          closeDeviceModal();
          await stopSession(serial);
          if (state.selectedDevice.isWifi) {
            await window.api.disconnect(serial);
            showToast('Dispositivo Wi-Fi desconectado', 'info');
            refreshDevices();
          }
        }
      });
    }
  }

  // ── 13. Funciones Wi-Fi ───────────────────────────────────────────
  function setupWifi() {
    if (btnEnableTcpip) {
      btnEnableTcpip.addEventListener('click', async () => {
        const serial = usbDeviceSelect.value;
        if (!serial) {
          showToast('Selecciona un teléfono conectado por cable', 'warn');
          return;
        }
        btnEnableTcpip.disabled = true;
        showToast('Activando modo Wi-Fi (:5555)…', 'info');
        const res = await window.api.enableTcpip(serial);
        btnEnableTcpip.disabled = false;
        if (res.ok && res.ip) {
          showToast(`Modo Wi-Fi listo en ${res.ip}. Conectando…`, 'info');
          const conn = await window.api.connectWifi(res.ip, '5555');
          if (conn.ok) {
            showToast('Conectado por Wi-Fi. Ya puedes quitar el cable.', 'success');
            refreshDevices();
          } else {
            showToast(`Activado pero no conectó: ${conn.error}`, 'warn');
          }
        } else {
          showToast(`Error al activar Wi-Fi: ${res.error || 'Fallo desconocido'}`, 'error');
        }
      });
    }

    if (btnPairWifi) {
      btnPairWifi.addEventListener('click', async () => {
        const ep = (pairEndpoint.value || '').trim();
        const code = (pairCode.value || '').trim();
        if (!ep || !code) {
          showToast('Introduce la dirección de vinculación y el código', 'warn');
          return;
        }
        const [host, port] = ep.split(':');
        btnPairWifi.disabled = true;
        showToast('Vinculando dispositivo…', 'info');
        const res = await window.api.pairWifi(host, port || '5555', code);
        btnPairWifi.disabled = false;
        if (res.ok) {
          showToast('Dispositivo vinculado correctamente', 'success');
        } else {
          showToast(`Error al vincular: ${res.error || res.message}`, 'error');
        }
      });
    }

    if (btnConnectWifi) {
      btnConnectWifi.addEventListener('click', async () => {
        const ep = (wifiEndpoint.value || '').trim();
        if (!ep) {
          showToast('Introduce la dirección del teléfono', 'warn');
          return;
        }
        const [ip, port] = ep.split(':');
        btnConnectWifi.disabled = true;
        showToast(`Conectando a ${ep}…`, 'info');
        const res = await window.api.connectWifi(ip, port || '5555');
        btnConnectWifi.disabled = false;
        if (res.ok) {
          showToast('Conectado por Wi-Fi con éxito', 'success');
          wifiEndpoint.value = '';
          refreshDevices();
        } else {
          showToast(`Error al conectar: ${res.error || res.message}`, 'error');
        }
      });
    }
  }

  // ── 14. Funciones iPhone ──────────────────────────────────────────
  function setupIphone() {
    iphoneModeRadios.forEach((radio) => {
      radio.addEventListener('change', () => {
        state.iphoneMode = radio.value;
        const isUsb = radio.value === 'usb';
        modeTitle.textContent = isUsb ? 'Cámara por cable USB' : 'Pantalla por AirPlay';
        modeSub.textContent = isUsb
          ? 'Imagen en alta calidad y sin retraso'
          : 'Comparte la pantalla del iPhone sin cables';
        probeT.textContent = isUsb
          ? 'Buscando iPhone conectado por cable…'
          : 'Esperando al iPhone en AirPlay…';
        probeS.textContent = isUsb
          ? 'Conecta tu iPhone al PC con el cable y desbloquéalo'
          : 'En el iPhone abre el Centro de control y elige “Duplicar pantalla”';
      });
    });

    if (btnStartIphone) {
      btnStartIphone.addEventListener('click', async () => {
        if (state.iphoneRunning) {
          btnStartIphone.disabled = true;
          if (state.iphoneMode === 'usb') {
            await window.api.iphoneUsbStopStream();
          } else {
            await window.api.iphoneStopUxPlay();
          }
          btnStartIphone.disabled = false;
          state.iphoneRunning = false;
          btnStartIphoneLabel.textContent = 'Iniciar';
          iphoneStateBadge.dataset.state = '';
          iphoneStateText.textContent = 'Detenido';
          showToast('iPhone detenido', 'info');
        } else {
          btnStartIphone.disabled = true;
          if (state.iphoneMode === 'usb') {
            const res = await window.api.iphoneUsbStartStream();
            btnStartIphone.disabled = false;
            if (res.ok) {
              state.iphoneRunning = true;
              btnStartIphoneLabel.textContent = 'Detener';
              iphoneStateBadge.dataset.state = 'ok';
              iphoneStateText.textContent = 'Transmitiendo por USB';
              showToast(res.message || 'Túnel USB listo', 'success');
            } else {
              showToast(res.error, 'error');
            }
          } else {
            const res = await window.api.iphoneStartUxPlay();
            btnStartIphone.disabled = false;
            if (res.ok) {
              state.iphoneRunning = true;
              btnStartIphoneLabel.textContent = 'Detener';
              iphoneStateBadge.dataset.state = 'ok';
              iphoneStateText.textContent = 'AirPlay activo';
              showToast('UxPlay iniciado para AirPlay', 'success');
            } else {
              showToast(res.error, 'error');
            }
          }
        }
      });
    }

    if (chkObsVirtualCam) {
      chkObsVirtualCam.addEventListener('change', async () => {
        const start = chkObsVirtualCam.checked;
        const res = await window.api.iphoneObsVirtualCam({ start });
        if (res.ok) {
          showToast(res.message || (start ? 'OBS Virtual Cam iniciada' : 'OBS Virtual Cam detenida'), 'info');
        } else {
          showToast(res.error, 'warn');
        }
      });
    }
  }

  // ── 15. Ajustes y Presets ─────────────────────────────────────────
  function setupSettings() {
    // Controles que definen la calidad: tocarlos pasa a "personalizado".
    const onQuality = () => { markCustom(); commitSettingsFromUI(); };

    if (sliderBr) {
      paintSlider(sliderBr);
      sliderBr.addEventListener('input', () => { paintSlider(sliderBr); onQuality(); });
    }
    if (sliderZoom) {
      paintSlider(sliderZoom);
      sliderZoom.addEventListener('input', () => { paintSlider(sliderZoom); commitSettingsFromUI(); });
    }
    [selCamRes, selScrRes].forEach((sel) => sel && sel.addEventListener('change', onQuality));
    [camFpsBox, scrFpsBox].forEach((box) => box && box.addEventListener('change', onQuality));
    codecRadios.forEach((r) => r.addEventListener('change', onQuality));

    [...facingRadios, ...rotRadios].forEach((r) => {
      r.addEventListener('change', commitSettingsFromUI);
    });

    [chkTorch, chkNoAudio, chkStayAwake, chkScreenOff, chkProtection].forEach((c) => {
      if (c) c.addEventListener('change', commitSettingsFromUI);
    });

    // Presets rápidos
    presetRadios.forEach((p) => {
      p.addEventListener('change', () => {
        if (p.dataset.quality !== 'max') {
          // Valores del perfil, ajustados a lo que ofrece este teléfono.
          selectNearest(selCamRes, parseInt(p.dataset.camRes, 10));
          checkNearest(camFpsBox, parseInt(p.dataset.camFps, 10));
          selectNearest(selScrRes, parseInt(p.dataset.scrRes, 10));
          checkNearest(scrFpsBox, parseInt(p.dataset.scrFps, 10));
          if (sliderBr) { sliderBr.value = parseInt(p.dataset.br, 10); paintSlider(sliderBr); }
        }

        commitSettingsFromUI();
        showToast(`Perfil aplicado: ${p.parentElement.querySelector('b')?.textContent || ''}`, 'info');
      });
    });

    // About buttons
    if (btnCheckUpdates) {
      btnCheckUpdates.addEventListener('click', async () => {
        btnCheckUpdates.disabled = true;
        showToast('Buscando actualizaciones…', 'info');
        const res = await window.api.checkUpdates();
        btnCheckUpdates.disabled = false;
        if (res.ok) {
          showToast(res.message || (res.updateAvailable ? `Nueva versión: v${res.latest}` : 'Estás al día'), 'info');
          if (res.updateAvailable && res.htmlUrl) {
            window.api.openExternal(res.htmlUrl);
          }
        } else {
          showToast(res.error || 'No se pudo comprobar', 'warn');
        }
      });
    }

    if (btnOpenGithub) {
      btnOpenGithub.addEventListener('click', () => {
        window.api.openExternal('https://github.com/danieth-dev/scrcpy-gui/releases');
      });
    }

    if (btnResetSettings) {
      btnResetSettings.addEventListener('click', () => {
        state.settings = defaultSettings();
        saveSettings();
        applySettingsToUI();
        showToast('Ajustes restablecidos', 'info');
      });
    }

    if (btnInstallVcam) {
      btnInstallVcam.addEventListener('click', async () => {
        await window.api.vcamInstallDriver();
        showToast('Solicitando permisos de Administrador para registrar el driver DirectShow…', 'info');
        setTimeout(updateVcamStatus, 4000);
      });
    }

    applySettingsToUI();

  }

  function applySettingsToUI() {
    const s = state.settings;
    presetRadios.forEach((r) => { r.checked = s.quality === 'max' ? r.value === 'max' : r.value === s.preset; });
    renderOptions(lastCaps);
    if (sliderBr) { sliderBr.value = s.bitrate || 8; paintSlider(sliderBr); }
    if (sliderZoom) { sliderZoom.value = s.cameraZoom || 1; paintSlider(sliderZoom); }

    codecRadios.forEach((r) => { r.checked = r.value === (s.codec || 'h264'); });
    facingRadios.forEach((r) => { r.checked = r.value === (s.cameraFacing || 'front'); });
    rotRadios.forEach((r) => { r.checked = r.value === String(s.orientation ?? '0'); });

    if (chkTorch) chkTorch.checked = !!s.cameraTorch;
    if (chkNoAudio) chkNoAudio.checked = !!s.noAudio;
    if (chkStayAwake) chkStayAwake.checked = !!s.stayAwake;
    if (chkScreenOff) chkScreenOff.checked = !!s.screenOff;
    if (chkProtection) chkProtection.checked = s.deviceProtection !== false;
  }

  function commitSettingsFromUI() {
    const getRadioVal = (radios, def) => {
      const checked = [...radios].find((r) => r.checked);
      return checked ? checked.value : def;
    };

    const preset = [...presetRadios].find((r) => r.checked);
    state.settings = {
      quality: preset && preset.value === 'max' ? 'max' : 'custom',
      preset: preset ? preset.value : null,
      maxSize: selCamRes && selCamRes.value !== '' ? parseInt(selCamRes.value, 10) : 0,
      maxFps: parseInt(getRadioVal($$('input[name="camFps"]'), '30'), 10),
      screenRes: selScrRes && selScrRes.value !== '' ? parseInt(selScrRes.value, 10) : 0,
      screenFps: parseInt(getRadioVal($$('input[name="screenFps"]'), '60'), 10),
      bitrate: sliderBr ? parseInt(sliderBr.value, 10) : 8,
      cameraZoom: sliderZoom ? parseFloat(sliderZoom.value) : 1,
      codec: getRadioVal(codecRadios, 'h264'),
      cameraFacing: getRadioVal(facingRadios, 'front'),
      orientation: getRadioVal(rotRadios, '0'),
      cameraTorch: chkTorch ? chkTorch.checked : false,
      noAudio: chkNoAudio ? chkNoAudio.checked : true,
      stayAwake: chkStayAwake ? chkStayAwake.checked : true,
      screenOff: chkScreenOff ? chkScreenOff.checked : true,
      deviceProtection: chkProtection ? chkProtection.checked : true,
    };

    saveSettings();
    scheduleProfilePreview();
    scheduleApplyLive();
  }

  // ── 15.4 Aplicar ajustes en vivo a las sesiones abiertas ──────────
  // El backend sólo relanza lo que cambia de verdad y conserva la posición
  // de las ventanas. Espera a que el usuario termine de mover un deslizador.
  let applyTimer = null;

  function liveSettingsPayload() {
    const s = state.settings;
    return {
      cameraFacing: s.cameraFacing || 'front',
      maxFps: s.maxFps || 30,
      maxSize: s.maxSize ?? 1080,
      cameraZoom: s.cameraZoom || 1,
      cameraTorch: !!s.cameraTorch,
      noAudio: !!s.noAudio,
      stayAwake: !!s.stayAwake,
      screenOff: !!s.screenOff,
      bitrate: s.bitrate || 8,
      codec: s.codec || 'h264',
      orientation: s.orientation || '0',
      screenRes: s.screenRes ?? 0,
      screenFps: s.screenFps || 60,
      quality: s.quality || 'custom',
      deviceProtection: s.deviceProtection !== false,
    };
  }

  function scheduleApplyLive() {
    clearTimeout(applyTimer);
    applyTimer = setTimeout(applyLive, 700);
  }

  async function applyLive({ silent = false } = {}) {
    if (!window.api.applySettings) return;
    const hasSessions = Object.keys(state.sessions).length > 0 || state.vcam.running;
    let res;
    try {
      res = await window.api.applySettings(liveSettingsPayload());
    } catch (e) {
      console.error('[applySettings]', e);
      if (hasSessions && !silent) showToast('No se pudieron aplicar los ajustes en vivo', 'error');
      return;
    }
    if (!res || !res.ok || !hasSessions || silent) return;

    const applied = [];
    const failed = [];
    (res.window || []).forEach((w) => {
      const s = state.sessions[w.serial];
      if (w.restarted && s) {
        if (w.pid) s.pid = w.pid;
        if (w.profile) {
          s.resolution = profileRes(w.profile).replace('x', '×');
          s.fps = w.profile.fps;
        }
        if (!w.camera && w.screenProfile) {
          const sp = w.screenProfile;
          s.resolution = sp.width ? `${sp.width}×${sp.height}` : 'nativa';
          s.fps = sp.fps;
        }
        applied.push(w.camera ? `Cámara en ventana: ${describeProfile(w.profile)}`
          : `Pantalla: ${w.screenProfile ? describeScreen(w.screenProfile) : 'actualizada'}`);
      } else if (w.error) {
        failed.push(w.error);
      }
    });
    const v = res.vcam;
    if (v && v.restarted && v.profile) {
      state.vcam.profile = v.profile;
      state.vcam.resolution = profileRes(v.profile);
      applied.push(`Cámara virtual: ${describeProfile(v.profile)}`);
    } else if (v && v.error) {
      failed.push(v.error);
    }

    renderDeviceCards();
    renderSessionsUI();
    if (failed.length) {
      showToast(`No se pudo aplicar: ${failed.join('. ')}`, 'error');
    } else if (applied.length) {
      showToast(`Ajustes aplicados. ${applied.join(' · ')}`, 'success');
    } else {
      showToast('Ajustes guardados (sin cambios para las sesiones abiertas)', 'info');
    }
  }

  // ── 15.5 Vista previa del perfil real por dispositivo ─────────────
  // Los ajustes son deseos; el backend los traduce a lo que el teléfono
  // soporta de verdad (cámaras, tamaños, fps, codificadores hardware).
  const CODEC_LABEL = { h264: 'H.264', h265: 'H.265', av1: 'AV1' };
  const FACING_LABEL = { front: 'frontal', back: 'trasera', external: 'externa' };
  let previewTimer = null;
  let previewSeq = 0;

  function previewDevice() {
    const online = (state.devices || []).filter((d) => !d.state || d.state === 'device');
    return state.selectedDevice && online.find((d) => d.serial === state.selectedDevice.serial)
      || online[0] || null;
  }

  function scheduleProfilePreview() {
    clearTimeout(previewTimer);
    previewTimer = setTimeout(updateProfilePreview, 250);
  }

  function describeProfile(p) {
    const w = (p.orientation === 90 || p.orientation === 270) ? p.height : p.width;
    const h = (p.orientation === 90 || p.orientation === 270) ? p.width : p.height;
    const parts = [
      `${w}×${h}`,
      `${p.fps} fps${p.highSpeed ? ' (alta velocidad)' : ''}`,
      `${CODEC_LABEL[p.codec] || p.codec}${p.encoder ? ' por hardware' : ''}`,
      `${p.bitrateMbps} Mbps`,
      `cámara ${FACING_LABEL[p.facing] || p.facing}`,
    ];
    if (p.zoom > 1) parts.push(`zoom ${Number(p.zoom).toFixed(1)}×`);
    if (p.torch) parts.push('linterna');
    return parts.join(' · ');
  }

  // Opciones sin teléfono conectado (genéricas).
  const GENERIC_CAM = { sizes: [[3840, 2160], [2560, 1440], [1920, 1080], [1280, 720]], fps: [15, 24, 30], torch: true };
  const GENERIC_SCR = { sizes: [[0, 0, 0], [1080, 0, 0], [720, 0, 0]], fps: [30, 60, 90, 120] };
  let lastCaps = null;

  function tierName(short) {
    if (short >= 2160) return '4K';
    if (short >= 1600) return '3K';
    if (short >= 1440) return '1440p';
    if (short >= 1080) return '1080p';
    if (short >= 720) return '720p';
    return `${short}p`;
  }

  // Rellena un <select> con {value, label, eq} y elige el valor pedido o el
  // más cercano por debajo (eq = valor equivalente, p. ej. nativa = 1080).
  function fillSelect(sel, items, wanted) {
    if (!sel) return;
    const sig = items.map((i) => `${i.value}:${i.label}`).join('|');
    if (sel.dataset.sig !== sig) {
      sel.innerHTML = '';
      items.forEach((i) => {
        const o = document.createElement('option');
        o.value = String(i.value);
        o.textContent = i.label;
        if (i.eq) o.dataset.eq = String(i.eq);
        sel.appendChild(o);
      });
      sel.dataset.sig = sig;
    }
    selectNearest(sel, wanted);
  }

  function selectNearest(sel, wanted) {
    if (!sel || !sel.options.length || Number.isNaN(wanted)) return;
    const opts = [...sel.options];
    const exact = opts.find((o) => +o.value === wanted || (o.dataset.eq && +o.dataset.eq <= wanted && wanted > 0));
    if (wanted === 0 || exact) { sel.value = wanted === 0 ? '0' : exact.value; return; }
    const lower = opts.filter((o) => +o.value > 0 && +o.value <= wanted).sort((x, y) => +y.value - +x.value)[0];
    sel.value = (lower || opts[opts.length - 1]).value;
  }

  function fillSeg(box, name, values, wanted) {
    if (!box) return;
    const sig = values.join(',');
    if (box.dataset.sig !== sig) {
      box.innerHTML = values.map((v) =>
        `<label><input type="radio" name="${name}" value="${v}"><span>${v} fps</span></label>`).join('');
      box.dataset.sig = sig;
    }
    checkNearest(box, wanted);
  }

  function checkNearest(box, wanted) {
    if (!box || Number.isNaN(wanted)) return;
    const radios = [...box.querySelectorAll('input[type=radio]')];
    if (!radios.length) return;
    const vals = radios.map((r) => +r.value);
    const pick = vals.includes(wanted) ? wanted
      : (vals.filter((v) => v <= wanted).sort((x, y) => y - x)[0] ?? Math.min(...vals));
    radios.forEach((r) => { r.checked = +r.value === pick; });
  }

  // Construye las opciones de Cámara y Pantalla a partir de lo que ofrece el
  // teléfono (o genéricas si no hay ninguno conectado).
  function renderOptions(res) {
    const s = state.settings;
    const camList = res && res.cameraOptions && res.cameraOptions.length ? res.cameraOptions : null;
    const cam = camList ? (camList.find((c) => c.facing === s.cameraFacing) || camList[0]) : GENERIC_CAM;
    const best = cam.sizes[0];
    fillSelect(selCamRes, [
      { value: 0, label: camList && best ? `Máxima del teléfono (${best[0]}×${best[1]})` : 'Máxima del teléfono' },
      ...cam.sizes.map(([w, h]) => ({ value: h, label: `${tierName(h)} · ${w}×${h}` })),
    ], s.maxSize ?? 0);
    fillSeg(camFpsBox, 'camFps', cam.fps, s.maxFps || 30);

    const so = res && res.screenOptions ? res.screenOptions : null;
    const native = so ? so.sizes[0] : null;
    const nativeShort = native ? Math.min(native[1], native[2]) : 0;
    fillSelect(selScrRes, (so || GENERIC_SCR).sizes.map(([short, w, h]) => short === 0
      ? { value: 0, label: so ? `Nativa (${w}×${h})` : 'Nativa', eq: nativeShort || undefined }
      : { value: short, label: so ? `${tierName(short)} (${w}×${h})` : tierName(short) }), s.screenRes ?? 0);
    fillSeg(scrFpsBox, 'screenFps', (so || GENERIC_SCR).fps, s.screenFps || 60);

    // Linterna: sólo si la cámara elegida la tiene.
    if (chkTorch) {
      const ok = camList ? !!cam.torch : true;
      chkTorch.disabled = !ok;
      const row = chkTorch.closest('.row');
      if (row) {
        row.classList.toggle('is-disabled', !ok);
        row.title = ok ? '' : 'Esta cámara no tiene linterna';
      }
    }

    // Formatos sin codificador por hardware: no se ofrecen.
    const hw = (res && res.hwCodecs) || [];
    codecRadios.forEach((r) => {
      const missing = hw.length > 0 && !hw.includes(r.value);
      r.disabled = missing;
      const lbl = r.closest('label');
      lbl.classList.toggle('unavailable', missing);
      lbl.title = missing ? `Este teléfono no tiene codificador por hardware para ${CODEC_LABEL[r.value]}` : '';
    });
    if (hw.length && codecRadios.length && [...codecRadios].every((r) => !r.checked || r.disabled)) {
      const first = [...codecRadios].find((r) => !r.disabled);
      if (first) first.checked = true;
    }

    // Zoom máximo real de la cámara elegida
    if (sliderZoom && camList && cam.zoomMax) {
      const max = Math.max(1, Math.min(10, cam.zoomMax));
      if (String(sliderZoom.max) !== String(max)) {
        sliderZoom.max = max;
        if (parseFloat(sliderZoom.value) > max) sliderZoom.value = max;
        paintSlider(sliderZoom);
      }
    }

    if (capsHint) {
      capsHint.hidden = !!camList;
      if (res && res.capsError && !camList) capsHint.textContent = `No se pudieron leer las opciones del teléfono: ${res.capsError}`;
    }
  }

  // Al tocar un control de calidad se abandona el perfil (pasa a personalizado).
  function markCustom() {
    presetRadios.forEach((r) => { r.checked = false; });
  }

  function describeScreen(sp) {
    const size = sp.width ? `${sp.width}×${sp.height}` : 'nativa';
    return `${size} · ${sp.fps} fps · ${CODEC_LABEL[sp.codec] || sp.codec}${sp.encoder ? ' por hardware' : ''} · ${sp.bitrateMbps} Mbps`;
  }

  async function updateProfilePreview() {
    const box = $('profilePreview');
    if (!window.api.deviceProfile) return;
    const dev = previewDevice();
    if (!dev) {
      lastCaps = null;
      renderOptions(null);
      if (box) box.hidden = true;
      if (specMax) specMax.textContent = 'Automático';
      return;
    }

    const seq = ++previewSeq;
    let res;
    try {
      res = await window.api.deviceProfile(dev.serial, getLaunchOptions('camera'), 'window');
    } catch (e) {
      console.error('[profile]', e);
      return;
    }
    if (seq !== previewSeq || !res || !res.ok) return;
    lastCaps = res;
    renderOptions(res);

    const p = res.profile;
    const sp = res.screenProfile;
    const isMax = state.settings.quality === 'max';

    // En "Máxima calidad" los controles muestran lo que se aplicará.
    if (isMax) {
      selectNearest(selCamRes, p.height);
      checkNearest(camFpsBox, p.fps);
      codecRadios.forEach((r) => { r.checked = r.value === p.codec; });
      if (sliderBr) { sliderBr.value = p.bitrateMbps; paintSlider(sliderBr); }
      if (selScrRes) selScrRes.value = '0';
      if (sp) checkNearest(scrFpsBox, sp.fps);
    }
    if (panelImage) panelImage.classList.toggle('auto', isMax);
    if (autoBadge) autoBadge.hidden = !isMax;

    const mp = res.maxProfile;
    if (specMax && mp) specMax.textContent = `${tierName(Math.min(mp.width, mp.height))} · ${mp.fps} fps`;

    if (!box) return;
    box.hidden = false;
    $('ppDevice').textContent = dev.model || dev.serial;
    $('ppSummary').textContent = `Cámara: ${describeProfile(p)}` + (sp ? `\nPantalla: ${describeScreen(sp)}` : '');
    const notes = $('ppNotes');
    notes.innerHTML = '';
    const all = [
      ...(p.verified ? [] : [res.capsError || 'No se pudieron leer las capacidades']),
      ...p.notes,
      ...((sp && sp.notes) || []).map((n) => `Pantalla: ${n}`),
    ];
    all.forEach((n) => {
      const li = document.createElement('li');
      li.textContent = n;
      notes.appendChild(li);
    });
  }

  function showProfileNotes(profile, what) {
    if (!profile) return;
    const summary = describeProfile(profile);
    if (profile.notes && profile.notes.length) {
      showToast(`${what}: ${summary}. ${profile.notes.join('. ')}`, 'warn');
    } else {
      showToast(`${what}: ${summary}`, 'success');
    }
  }

  // ── 15.6 Salud del teléfono (temperatura / batería) ──────────────
  // El backend la vigila durante las sesiones y baja la calidad si se calienta.
  const THROTTLE_LABEL = ['', 'calidad reducida', 'calidad mínima', 'detenido'];
  state.health = {};

  function renderHealthChip(serial) {
    const h = (state.health || {})[serial];
    if (!h || h.active === false || h.batteryTemp == null) return '';
    const lvl = h.protection === false ? 0 : (h.level || 0);
    const hotUnprotected = h.protection === false && (h.thermalStatus ?? 0) >= 2;
    const cls = lvl >= 2 ? 'hot' : (lvl === 1 || hotUnprotected) ? 'warm' : '';
    const status = ['normal', 'leve', 'moderado', 'grave', 'crítico', 'emergencia', 'apagado'][h.thermalStatus ?? 0] || '';
    const title = `Temperatura de la batería · estado térmico ${status}` +
      (h.protection === false ? ' · Protección desactivada'
        : lvl ? ` · Protección activa: ${THROTTLE_LABEL[lvl]}` : '');
    return `<span class="chip temp ${cls}" title="${escHtml(title)}">${Number(h.batteryTemp).toFixed(1)} °C${lvl ? ` · ${THROTTLE_LABEL[lvl]}` : ''}</span>`;
  }

  function profileRes(p) {
    if (!p) return null;
    const rot = p.orientation === 90 || p.orientation === 270;
    return rot ? `${p.height}x${p.width}` : `${p.width}x${p.height}`;
  }

  // Reconexión automática tras un corte (cable, cargador, Wi-Fi).
  state.reconnecting = {};

  function setupReconnectEvents() {
    if (window.api.onReconnecting) {
      window.api.onReconnecting((r) => {
        if (!r || !r.serial) return;
        state.reconnecting[r.serial] = r;
        const what = r.kind === 'vcam' ? 'Cámara Virtual' : 'Sesión';
        showToast(`${what}: se perdió la conexión con el teléfono. Reconectando (${r.attempt}/${r.of})…`, 'warn');
        renderDeviceCards();
      });
    }
    if (window.api.onReconnected) {
      window.api.onReconnected((r) => {
        if (!r || !r.serial) return;
        delete state.reconnecting[r.serial];
        if (r.kind === 'vcam') {
          if (r.profile) {
            state.vcam.profile = r.profile;
            state.vcam.resolution = profileRes(r.profile);
          }
        } else if (state.sessions[r.serial]) {
          const s = state.sessions[r.serial];
          if (r.pid) s.pid = r.pid;
          if (r.profile) {
            s.resolution = profileRes(r.profile).replace('x', '×');
            s.fps = r.profile.fps;
          }
        }
        showToast(r.kind === 'vcam' ? 'Cámara Virtual reconectada' : 'Sesión reconectada', 'success');
        renderDeviceCards();
        renderSessionsUI();
      });
    }
  }

  function setupHealthEvents() {
    setupReconnectEvents();
    if (window.api.onDeviceHealth) {
      window.api.onDeviceHealth((h) => {
        if (!h || !h.serial) return;
        state.health[h.serial] = { ...(state.health[h.serial] || {}), ...h };
        renderDeviceCards();
      });
    }
    if (window.api.onDeviceAlert) {
      window.api.onDeviceAlert((a) => a && a.message && showToast(a.message, 'warn'));
    }
    if (window.api.onDeviceThrottle) {
      window.api.onDeviceThrottle((t) => {
        if (!t || !t.serial) return;
        state.health[t.serial] = { ...(state.health[t.serial] || {}), level: t.level };
        const why = t.reason ? ` (${t.reason})` : '';
        if (t.action === 'stopped') {
          delete state.sessions[t.serial];
          if (state.vcam.serial === t.serial) {
            state.vcam.running = false;
            state.vcam.serial = null;
          }
          renderSessionsUI();
          showToast(`Cámara detenida para proteger el teléfono${why}. Déjalo enfriar unos minutos.`, 'error');
        } else if (t.action === 'restarted') {
          const s = state.sessions[t.serial];
          if (s && t.windowProfile) {
            s.resolution = profileRes(t.windowProfile).replace('x', '×');
            s.fps = t.windowProfile.fps;
          }
          if (t.vcamProfile && state.vcam.serial === t.serial) {
            state.vcam.profile = t.vcamProfile;
            state.vcam.resolution = profileRes(t.vcamProfile);
          }
          showToast(t.level > 0
            ? `Protección térmica: ${THROTTLE_LABEL[t.level]}${why}`
            : 'El teléfono se enfrió: calidad completa restaurada', t.level > 0 ? 'warn' : 'success');
        } else if (t.level > 0) {
          showToast(`El teléfono se está calentando${why}. Considera bajar la calidad.`, 'warn');
        }
        renderDeviceCards();
      });
    }
  }

  // ── 16. Actualización Periódica de Dispositivos ────────────────────
  let lastPreviewSerial = null;
  async function refreshDevices() {
    try {
      const res = await window.api.getDevices();
      if (res.ok) {
        state.devices = res.devices || [];
        renderDeviceCards();
        const dev = previewDevice();
        const serial = dev ? dev.serial : null;
        if (serial !== lastPreviewSerial) {
          lastPreviewSerial = serial;
          scheduleProfilePreview();
        }
      }
    } catch (e) {
      console.error('[refreshDevices error]', e);
    }
  }

  // ── 17. Eventos de scrcpy del Backend ──────────────────────────────
  function setupTauriEvents() {
    window.api.onScrcpyStopped((data) => {
      const serial = data.serial;
      if (state.reconnecting) delete state.reconnecting[serial];
      if (state.sessions[serial]) {
        delete state.sessions[serial];
        renderDeviceCards();
        renderSessionsUI();
        if (data.error) {
          showToast(`scrcpy finalizó con aviso: ${data.error}`, 'warn');
          showAlert('scrcpy se detuvo con aviso', data.error, data.error);
        } else {
          showToast(`Sesión de ${serial} cerrada`, 'info');
        }
      }
    });

    window.api.onScrcpyTexture((data) => {
      const session = state.sessions[data.serial];
      if (session) {
        session.resolution = data.resolution || `${data.width}×${data.height}`;
        renderDeviceCards();
        renderSessionsUI();
      }
    });

    window.api.onIphoneUxPlayStopped(() => {
      state.iphoneRunning = false;
      if (btnStartIphoneLabel) btnStartIphoneLabel.textContent = 'Iniciar';
      if (iphoneStateBadge) {
        iphoneStateBadge.dataset.state = '';
        iphoneStateText.textContent = 'Detenido';
      }
      showToast('UxPlay se cerró', 'info');
    });

    setupHealthEvents();

    if (window.api.onVcamStats) {
      window.api.onVcamStats((stats) => {
        state.vcam.fps = stats.fps;
        state.vcam.decodedFps = stats.decodedFps;
        state.vcam.dropped = stats.dropped;
        renderDeviceCards();
      });
    }

    if (window.api.onVcamStopped) {
      window.api.onVcamStopped((info) => {
        const wasRunning = state.vcam.running;
        if (state.reconnecting && info && info.serial) delete state.reconnecting[info.serial];
        state.vcam.running = false;
        state.vcam.serial = null;
        renderDeviceCards();
        if (info && info.error) {
          console.error('[vcam]', info.error);
          showToast('Cámara Virtual detenida: ' + info.error, 'error');
        } else if (wasRunning) {
          showToast('Cámara Virtual DirectShow detenida', 'info');
        }
      });
    }
  }

  // ── 18. Inicialización General ─────────────────────────────────────
  async function init() {
    setupNavigation();
    setupTitlebar();
    setupWifi();
    setupIphone();
    setupSettings();
    setupModal();
    setupTauriEvents();

    if (btnRescan) btnRescan.addEventListener('click', refreshDevices);
    if (btnRefreshEmpty) btnRefreshEmpty.addEventListener('click', refreshDevices);
    if (btnStopAll) btnStopAll.addEventListener('click', stopAllSessions);

    renderRecents();
    await updateVcamStatus();
    await refreshDevices();
    // El backend arranca con la protección activada: sincronizar la elección.
    applyLive({ silent: true });


    // Timer de actualización de dispositivos cada 4s
    state.pollTimer = setInterval(refreshDevices, 4000);

    // Timer de reloj para duraciones en vivo cada 1s
    state.clockTimer = setInterval(() => {
      if (Object.keys(state.sessions).length > 0) {
        $$('.session-timer').forEach((el) => {
          const card = el.closest('.device');
          const serial = card?.dataset?.serial;
          if (serial && state.sessions[serial]) {
            el.textContent = formatDuration(state.sessions[serial].startedAt);
          }
        });
      }
    }, 1000);
  }

  // Arrancar al cargar el DOM
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
