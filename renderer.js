/* ── renderer.js — scrcpy GUI logic ─────────────────────────── */

// ── State ──────────────────────────────────────────────────────
const state = {
  devices: [],
  sessions: {},         // serial → { model, brand, mode: 'mirror'|'camera'|'record' }
  selectedDevice: null, // device object for modal
  settings: loadSettings(),
  restartingSerials: new Set(), // serials being restarted by preset apply
};

// ── DOM refs ───────────────────────────────────────────────────
const $ = id => document.getElementById(id);
const $$ = sel => document.querySelectorAll(sel);

// Nav
const navItems   = $$('.nav-item');
const sections   = $$('.section');
const sessionBadge = $('sessionsBadge');

// Devices
const devicesLoading = $('devicesLoading');
const devicesEmpty   = $('devicesEmpty');
const devicesList    = $('devicesList');

// WiFi
const usbDeviceSelect     = $('usbDeviceSelect');
const btnEnableTcpip      = $('btnEnableTcpip');
const tcpipResult         = $('tcpipResult');
const wifiIp              = $('wifiIp');
const wifiPort            = $('wifiPort');
const wifiEndpoint        = $('wifiEndpoint');
const btnConnectWifi      = $('btnConnectWifi');
const wifiConnectResult   = $('wifiConnectResult');
const pairEndpoint        = $('pairEndpoint');
const pairCode            = $('pairCode');
const btnPairWifi         = $('btnPairWifi');
const pairResult          = $('pairResult');
const recentsBlock        = $('recentsBlock');
const recentsList         = $('recentsList');
const btnClearRecents     = $('btnClearRecents');

// Sessions
const sessionsList = $('sessionsList');

// Settings
const maxSizeSlider  = $('maxSize');
const maxSizeVal     = $('maxSizeVal');
const maxFpsSlider   = $('maxFps');
const maxFpsVal      = $('maxFpsVal');
const bitrateSlider  = $('bitrate');
const bitrateVal     = $('bitrateVal');
const codecSelect    = $('codec');
const noAudioCb      = $('noAudio');
const stayAwakeCb    = $('stayAwake');
const screenOffCb    = $('screenOff');

// Modal
const deviceModal       = $('deviceModal');
const modalClose        = $('modalClose');
const modalDeviceInfo   = $('modalDeviceInfo');
const modalLaunch       = $('modalLaunch');
const modalScreenshot   = $('modalScreenshot');
const modalCamera       = $('modalCamera');
const modalRecord       = $('modalRecord');
const modalDisconnect   = $('modalDisconnect');

// Status
const toolStatusPill = $('toolStatusPill');
const toolStatusText = $('toolStatusText');

// Titlebar
$('btnMin').addEventListener('click',   () => window.api.minimize());
$('btnMax').addEventListener('click',   () => window.api.maximize());
$('btnClose').addEventListener('click', () => window.api.close());

// ── Navigation ─────────────────────────────────────────────────
navItems.forEach(item => {
  item.addEventListener('click', () => {
    const target = item.dataset.section;
    navItems.forEach(n => n.classList.remove('active'));
    item.classList.add('active');
    sections.forEach(s => s.classList.remove('active'));
    const sectionEl = document.getElementById(`section-${target}`);
    if (sectionEl) {
      sectionEl.classList.add('active');
    } else {
      console.error(`[nav] Sección no encontrada: section-${target}`);
    }
  });
});

// ── Settings helpers ───────────────────────────────────────────
function loadSettings() {
  try {
    const raw = localStorage.getItem('scrcpy-settings');
    const parsed = raw ? JSON.parse(raw) : {};
    return { ...defaultSettings(), ...parsed };
  } catch (_) { return defaultSettings(); }
}
function defaultSettings() {
  return {
    maxSize: 1080,
    maxFps: 30,
    bitrate: 8,
    codec: 'h264',
    noAudio: true,
    stayAwake: true,
    screenOff: true,
    cameraFacing: 'front',
    orientation: '0',
    cameraZoom: 1,
    cameraTorch: false,
  };
}
// ======== Settings Management ========
const inputMaxSize = $('maxSize');
const valMaxSize = $('maxSizeVal');
const inputMaxFps = $('maxFps');
const valMaxFps = $('maxFpsVal');
const inputBitrate = $('bitrate');
const valBitrate = $('bitrateVal');
const inputCodec = $('codec');
const inputCameraFacing = $('cameraFacing');
const inputOrientation = $('orientation');
const inputCameraZoom = $('cameraZoom');
const valCameraZoom = $('cameraZoomVal');
const toggleCameraTorch = $('cameraTorch');
const toggleNoAudio = $('noAudio');
const toggleStayAwake = $('stayAwake');
const toggleScreenOff = $('screenOff');
const dupConnectionWarn = $('dupConnectionWarn');
const btnSaveSettings = $('btnSaveSettings');
const btnResetSettings = $('btnResetSettings');

// Preset buttons
const presetDefault = $('presetDefault');
const presetWebcam = $('presetWebcam');
const presetStream = $('presetStream');
const presetGaming = $('presetGaming');

function updateSettingsUI() {
  const s = state.settings;
  inputMaxSize.value = s.maxSize;
  valMaxSize.textContent = s.maxSize + 'p';
  inputMaxFps.value = s.maxFps;
  valMaxFps.textContent = s.maxFps + ' fps';
  inputBitrate.value = s.bitrate;
  valBitrate.textContent = s.bitrate + ' Mbps';
  inputCodec.value = s.codec;
  if (inputCameraFacing) inputCameraFacing.value = s.cameraFacing || 'front';
  if (inputOrientation) inputOrientation.value = String(s.orientation ?? '0');
  if (inputCameraZoom) {
    inputCameraZoom.value = s.cameraZoom ?? 1;
    if (valCameraZoom) valCameraZoom.textContent = `${Number(s.cameraZoom ?? 1).toFixed(1)}×`;
  }
  if (toggleCameraTorch) toggleCameraTorch.checked = !!s.cameraTorch;
  toggleNoAudio.checked = s.noAudio;
  toggleStayAwake.checked = s.stayAwake;
  toggleScreenOff.checked = s.screenOff;
}

function applySettingsToUI() {
  updateSettingsUI();
  restoreActivePreset();
}

// ── Preset names map ───────────────────────────────────────────
const PRESET_NAMES = {
  default: 'Normal',
  webcam:  'Webcam Reuniones',
  stream:  'Webcam Streams',
  gaming:  'Gaming',
};

// ── Preset definitions (optimizados Samsung A55 · modo cámara/webcam) ──
const PRESETS = {
  // Uso general: 1080p30 estable, menos calor que 60 fps
  default: { maxSize: 1080, maxFps: 30, bitrate: 8,  codec: 'h264', noAudio: true, stayAwake: true, screenOff: true },
  // Zoom/Meet/Teams: 720p basta; H.264 compatible y ligero
  webcam:  { maxSize: 720,  maxFps: 30, bitrate: 5,  codec: 'h264', noAudio: true, stayAwake: true, screenOff: true },
  // OBS/Twitch/YouTube: 1080p60 con H.265 (~14 Mbps sostenible en A55)
  stream:  { maxSize: 1080, maxFps: 60, bitrate: 14, codec: 'h265', noAudio: true, stayAwake: true, screenOff: true },
  // Gaming overlay: prioridad latencia
  gaming:  { maxSize: 720,  maxFps: 60, bitrate: 6,  codec: 'h264', noAudio: true, stayAwake: true, screenOff: true },
};

function setActivePresetUI(presetKey) {
  // Remove active from all
  document.querySelectorAll('.preset-card').forEach(card => {
    card.classList.remove('active');
    const check = card.querySelector('.preset-check');
    if (check) check.classList.remove('visible');
  });

  if (!presetKey) {
    $('activePresetBadge').style.display = 'none';
    return;
  }

  // Activate the selected
  const card = document.querySelector(`.preset-card[data-preset="${presetKey}"]`);
  if (card) {
    card.classList.add('active');
    const check = card.querySelector('.preset-check');
    if (check) check.classList.add('visible');
  }

  const badge = $('activePresetBadge');
  const nameEl = $('activePresetName');
  if (badge && nameEl) {
    badge.style.display = 'flex';
    nameEl.textContent = PRESET_NAMES[presetKey] || presetKey;
  }
}

function restoreActivePreset() {
  const savedPreset = localStorage.getItem('scrcpy-active-preset');
  if (savedPreset && PRESETS[savedPreset]) {
    setActivePresetUI(savedPreset);
  }
}

function clearActivePreset() {
  // Called when user manually tweaks sliders — removes preset highlight
  document.querySelectorAll('.preset-card').forEach(card => {
    card.classList.remove('active');
    const check = card.querySelector('.preset-check');
    if (check) check.classList.remove('visible');
  });
  $('activePresetBadge').style.display = 'none';
  localStorage.removeItem('scrcpy-active-preset');
}


function readSettingsFromUI() {
  return {
    maxSize:      parseInt(inputMaxSize.value, 10),
    maxFps:       parseInt(inputMaxFps.value, 10),
    bitrate:      parseInt(inputBitrate.value, 10),
    codec:        inputCodec.value,
    noAudio:      toggleNoAudio.checked,
    stayAwake:    toggleStayAwake.checked,
    screenOff:    toggleScreenOff.checked,
    cameraFacing: inputCameraFacing ? inputCameraFacing.value : 'front',
    orientation:  inputOrientation ? String(inputOrientation.value) : '0',
    cameraZoom:   inputCameraZoom ? parseFloat(inputCameraZoom.value) : 1,
    cameraTorch:  toggleCameraTorch ? toggleCameraTorch.checked : false,
  };
}

function getCameraLaunchOptions(settings = state.settings) {
  const s = settings;
  return {
    maxSize:      s.maxSize,
    maxFps:       s.maxFps,
    bitrate:      s.bitrate,
    codec:        s.codec,
    noAudio:      s.noAudio,
    stayAwake:    s.stayAwake,
    screenOff:    s.screenOff,
    cameraFacing: s.cameraFacing || 'front',
    orientation:  s.orientation ?? '0',
    cameraZoom:   s.cameraZoom ?? 1,
    cameraTorch:  !!s.cameraTorch,
    cameraOnly:   true,
  };
}

function getCameraSerials() {
  return Object.keys(state.sessions).filter(
    serial => state.sessions[serial]?.mode === 'camera'
  );
}

async function restartCameraSessions(label = 'ajustes') {
  const cameraSerials = getCameraSerials();
  if (cameraSerials.length === 0) return 0;

  showToast(`Aplicando ${label} a la cámara…`, 'info');
  const options = getCameraLaunchOptions();
  let okCount = 0;

  for (const serial of cameraSerials) {
    const meta = { ...state.sessions[serial], mode: 'camera' };
    state.restartingSerials.add(serial);
    try {
      await window.api.stopScrcpy(serial);
      await new Promise(r => setTimeout(r, 400));
      const result = await window.api.launchScrcpy(serial, options);
      if (result.ok) {
        state.sessions[serial] = meta;
        okCount++;
      } else {
        delete state.sessions[serial];
        showToast(`Error al reiniciar cámara ${meta?.model || serial}`, 'error');
      }
    } finally {
      state.restartingSerials.delete(serial);
    }
  }

  updateSessionsUI();
  renderDeviceCards(state.devices);
  renderRecents();
  return okCount;
}

async function applyPreset(presetKey) {
  const preset = PRESETS[presetKey];
  if (!preset) return;

  state.settings = { ...state.settings, ...preset };
  updateSettingsUI();
  localStorage.setItem('scrcpy-settings', JSON.stringify(state.settings));
  localStorage.setItem('scrcpy-active-preset', presetKey);
  setActivePresetUI(presetKey);

  const name = PRESET_NAMES[presetKey] || presetKey;
  const hadCamera = getCameraSerials().length > 0;
  const okCount = await restartCameraSessions(`"${name}"`);

  if (okCount > 0) {
    showToast(`✓ Preset "${name}" aplicado a la cámara`, 'success');
  } else if (!hadCamera) {
    showToast(`✓ Preset "${name}" guardado (para cámara)`, 'success');
  }
}

presetDefault.addEventListener('click', () => applyPreset('default'));
presetWebcam.addEventListener('click',  () => applyPreset('webcam'));
presetStream.addEventListener('click',  () => applyPreset('stream'));
presetGaming.addEventListener('click',  () => applyPreset('gaming'));


inputMaxSize.addEventListener('input',  () => { valMaxSize.textContent  = `${inputMaxSize.value}p`; clearActivePreset(); });
inputMaxFps.addEventListener('input',   () => { valMaxFps.textContent   = `${inputMaxFps.value} fps`; clearActivePreset(); });
inputBitrate.addEventListener('input',  () => { valBitrate.textContent  = `${inputBitrate.value} Mbps`; clearActivePreset(); });
if (inputCameraZoom) {
  inputCameraZoom.addEventListener('input', () => {
    if (valCameraZoom) valCameraZoom.textContent = `${Number(inputCameraZoom.value).toFixed(1)}×`;
    clearActivePreset();
  });
}
let settingsApplyLock = false;

async function commitSettings({ clearPreset = true, toastSaved = true } = {}) {
  if (settingsApplyLock) return;
  settingsApplyLock = true;
  try {
    const s = readSettingsFromUI();
    localStorage.setItem('scrcpy-settings', JSON.stringify(s));
    state.settings = s;
    if (clearPreset) clearActivePreset();

    const okCount = await restartCameraSessions('ajustes');
    if (okCount > 0) {
      showToast('✓ Ajustes aplicados a la cámara', 'success');
    } else if (toastSaved) {
      showToast('Ajustes guardados', 'success');
    }
  } finally {
    settingsApplyLock = false;
  }
}

$('btnSaveSettings').addEventListener('click', () => commitSettings({ clearPreset: true }));
$('btnResetSettings').addEventListener('click', async () => {
  state.settings = defaultSettings();
  applySettingsToUI();
  localStorage.setItem('scrcpy-settings', JSON.stringify(state.settings));
  localStorage.removeItem('scrcpy-active-preset');
  const okCount = await restartCameraSessions('defaults');
  if (okCount > 0) showToast('✓ Defaults aplicados a la cámara', 'success');
  else showToast('Ajustes restablecidos', 'info');
});

// Live-apply when controls settle (change = after slider release / select / toggle)
[inputMaxSize, inputMaxFps, inputBitrate, inputCameraZoom].filter(Boolean).forEach(el => {
  el.addEventListener('change', () => commitSettings({ clearPreset: true }));
});
[inputCodec, inputCameraFacing, inputOrientation, toggleCameraTorch, toggleNoAudio, toggleStayAwake, toggleScreenOff]
  .filter(Boolean)
  .forEach(el => {
    el.addEventListener('change', () => commitSettings({ clearPreset: true }));
  });

// ── Tool status check ──────────────────────────────────────────
async function checkTools() {
  const { scrcpy, adb } = await window.api.getToolsStatus();
  if (scrcpy && adb) {
    toolStatusPill.className = 'status-pill ok';
    toolStatusText.textContent = 'scrcpy + ADB listos';
  } else if (adb && !scrcpy) {
    toolStatusPill.className = 'status-pill warn';
    toolStatusText.textContent = 'scrcpy.exe no encontrado';
  } else {
    toolStatusPill.className = 'status-pill err';
    toolStatusText.textContent = 'ADB no encontrado';
  }
}

// ── Devices ────────────────────────────────────────────────────
async function refreshDevices() {
  // Show loading
  devicesLoading.style.display = 'flex';
  devicesEmpty.style.display   = 'none';
  devicesList.style.display    = 'none';

  const btnR = $('btnRefresh');
  btnR.classList.add('spinning');

  const result = await window.api.getDevices();

  btnR.classList.remove('spinning');

  if (!result.ok) {
    showToast('Error al obtener dispositivos: ' + result.error, 'error');
    devicesLoading.style.display = 'none';
    devicesEmpty.style.display   = 'flex';
    return;
  }

  state.devices = result.devices;
  updateDupConnectionWarn(result.devices);
  devicesLoading.style.display = 'none';
  populateUsbSelect(result.devices);
  renderRecents();

  if (result.devices.length === 0) {
    devicesEmpty.style.display = 'flex';
    devicesList.style.display  = 'none';
    return;
  }

  devicesEmpty.style.display = 'none';
  devicesList.style.display  = 'grid';
  renderDeviceCards(result.devices);
}

function updateDupConnectionWarn(devices) {
  if (!dupConnectionWarn) return;
  const groups = new Map();
  for (const d of devices) {
    const key = d.hardwareSerial || `${(d.brand || '').toLowerCase()}|${(d.model || '').toLowerCase()}`;
    if (!key || key === '|') continue;
    if (!groups.has(key)) groups.set(key, { usb: false, wifi: false });
    const g = groups.get(key);
    if (d.isWifi) g.wifi = true;
    else g.usb = true;
  }
  const hasDup = [...groups.values()].some(g => g.usb && g.wifi);
  dupConnectionWarn.style.display = hasDup ? 'block' : 'none';
}

function preferDeviceForCamera(dev) {
  if (!dev) return dev;
  // Prefer WiFi twin of the same phone when both are online
  const key = dev.hardwareSerial || `${(dev.brand || '').toLowerCase()}|${(dev.model || '').toLowerCase()}`;
  const wifiTwin = state.devices.find(d => {
    if (!d.isWifi || d.serial === dev.serial) return false;
    if (dev.hardwareSerial && d.hardwareSerial && d.hardwareSerial === dev.hardwareSerial) return true;
    const dKey = `${(d.brand || '').toLowerCase()}|${(d.model || '').toLowerCase()}`;
    return dKey === key && key !== '|';
  });
  if (!dev.isWifi && wifiTwin) {
    showToast('Usando WiFi (evita conflicto USB+WiFi)', 'info');
    return wifiTwin;
  }
  return dev;
}

function renderDeviceCards(devices) {
  updateDupConnectionWarn(devices);
  devicesList.innerHTML = '';
  for (const dev of devices) {
    const isRunning = !!state.sessions[dev.serial];
    const card = document.createElement('div');
    card.className = `device-card${isRunning ? ' running' : ''}`;
    card.innerHTML = `
      <div class="device-card-header">
        <div class="device-avatar">${dev.isWifi ? 'W' : 'U'}</div>
        <div class="device-info">
          <div class="device-brand">${escHtml(dev.brand || 'Android')}</div>
          <div class="device-model">${escHtml(dev.model || 'Dispositivo')}</div>
          <div class="device-serial">${escHtml(dev.serial)}</div>
        </div>
      </div>
      <div class="device-meta">
        ${dev.androidVersion ? `<span class="meta-chip cyan">Android ${escHtml(dev.androidVersion)}</span>` : ''}
        ${dev.batteryLevel   ? `<span class="meta-chip green">${escHtml(dev.batteryLevel)}%</span>` : ''}
        ${dev.isWifi         ? `<span class="meta-chip">WiFi</span>` : `<span class="meta-chip">USB</span>`}
        ${isRunning          ? `<span class="meta-chip green">Activo</span>` : ''}
      </div>
      <div class="device-launch-hint">${isRunning ? '✓ Activo' : 'Abrir →'}</div>
    `;
    card.addEventListener('click', () => openDeviceModal(dev));
    devicesList.appendChild(card);
  }
}

function populateUsbSelect(devices) {
  if (usbDeviceSelect) {
    usbDeviceSelect.innerHTML = '<option value="">— Selecciona un dispositivo USB —</option>';
  }
  const step2UsbSelect = $('step2UsbSelect');
  if (step2UsbSelect) {
    step2UsbSelect.innerHTML = '<option value="">— Seleccionar dispositivo USB (auto-completar IP:5555) —</option>';
  }

  const usbDevs = devices.filter(d => !d.isWifi);

  for (const d of usbDevs) {
    if (usbDeviceSelect) {
      const opt = document.createElement('option');
      opt.value = d.serial;
      opt.textContent = `${d.brand || ''} ${d.model || 'Dispositivo'} (${d.serial})`.trim();
      usbDeviceSelect.appendChild(opt);
    }

    if (step2UsbSelect) {
      const opt = document.createElement('option');
      const ip5555 = d.ip ? `${d.ip}:5555` : '';
      opt.value = ip5555;
      opt.textContent = d.ip
        ? `${d.brand || ''} ${d.model || 'Dispositivo'} (${d.ip}:5555)`
        : `${d.brand || ''} ${d.model || 'Dispositivo'} (${d.serial})`;
      step2UsbSelect.appendChild(opt);
    }
  }

  if (usbDevs.length > 0 && step2UsbSelect) {
    const firstWithIp = usbDevs.find(d => d.ip);
    if (firstWithIp) {
      step2UsbSelect.value = `${firstWithIp.ip}:5555`;
      if (wifiEndpoint && (!wifiEndpoint.value || wifiEndpoint.value.includes(':5555'))) {
        setWifiEndpoint(firstWithIp.ip, '5555');
      }
    }
    if (usbDeviceSelect) usbDeviceSelect.value = usbDevs[0].serial;
  }
}

$('btnRefresh').addEventListener('click', refreshDevices);
$('btnRefreshEmpty').addEventListener('click', refreshDevices);

// ── Device Modal ───────────────────────────────────────────────
function openDeviceModal(dev) {
  state.selectedDevice = dev;
  const isRunning = !!state.sessions[dev.serial];

  modalDeviceInfo.innerHTML = `
    <div class="device-avatar">${dev.isWifi ? 'W' : 'U'}</div>
    <div class="device-brand">${escHtml(dev.brand || 'Android')}</div>
    <div class="device-model">${escHtml(dev.model || 'Dispositivo')}</div>
    <div class="meta-chips">
      ${dev.androidVersion ? `<span class="meta-chip cyan">Android ${escHtml(dev.androidVersion)}</span>` : ''}
      ${dev.batteryLevel   ? `<span class="meta-chip green">${escHtml(dev.batteryLevel)}%</span>` : ''}
      ${dev.isWifi         ? `<span class="meta-chip">WiFi</span>` : `<span class="meta-chip">USB</span>`}
    </div>
  `;

  modalLaunch.innerHTML = isRunning
    ? `<span class="action-icon">■</span><span>Detener Mirror</span>`
    : `<span class="action-icon">▶</span><span>Iniciar Mirror</span>`;

  deviceModal.style.display = 'flex';
}

modalClose.addEventListener('click', () => { deviceModal.style.display = 'none'; });
deviceModal.addEventListener('click', e => {
  if (e.target === deviceModal) deviceModal.style.display = 'none';
});

modalLaunch.addEventListener('click', async () => {
  const dev = state.selectedDevice;
  if (!dev) return;

  if (state.sessions[dev.serial]) {
    await window.api.stopScrcpy(dev.serial);
    delete state.sessions[dev.serial];
    updateSessionsUI();
    renderDeviceCards(state.devices);
    deviceModal.style.display = 'none';
    showToast(`Mirror de ${dev.model} detenido`, 'info');
    return;
  }

  deviceModal.style.display = 'none';
  await launchMirror(dev);
});

modalScreenshot.addEventListener('click', async () => {
  const dev = state.selectedDevice;
  if (!dev) return;
  deviceModal.style.display = 'none';
  showToast('Capturando pantalla…', 'info');
  const res = await window.api.takeScreenshot(dev.serial);
  if (res.ok) showToast('📷 Captura guardada', 'success');
  else if (!res.cancelled) showToast('Error: ' + res.error, 'error');
});

modalCamera.addEventListener('click', async () => {
  const dev = state.selectedDevice;
  if (!dev) return;
  deviceModal.style.display = 'none';
  await launchCamera(dev);
});

modalRecord.addEventListener('click', async () => {
  const dev = state.selectedDevice;
  if (!dev) return;
  deviceModal.style.display = 'none';
  const s = state.settings;
  const result = await window.api.launchScrcpy(dev.serial, {
    ...s, record: true,
  });
  if (result.ok) {
    state.sessions[dev.serial] = { model: dev.model, brand: dev.brand, mode: 'record' };
    rememberDevice(dev);
    updateSessionsUI();
    renderDeviceCards(state.devices);
    showToast('Grabación iniciada', 'success');
  } else {
    showToast('Error: ' + (result.error || 'desconocido'), 'error');
  }
});

modalDisconnect.addEventListener('click', async () => {
  const dev = state.selectedDevice;
  if (!dev) return;
  deviceModal.style.display = 'none';
  const res = await window.api.disconnect(dev.serial);
  if (res.ok) {
    showToast(`Desconectado: ${dev.serial}`, 'info');
    await refreshDevices();
  } else {
    showToast('Error al desconectar: ' + res.error, 'error');
  }
});

// ── WiFi ───────────────────────────────────────────────────────
function parseEndpoint(raw) {
  const s = String(raw || '').trim()
    .replace(/^adb\s+(connect|pair)\s+/i, '')
    .replace(/\s+/g, '');
  const m = s.match(/^(\d{1,3}(?:\.\d{1,3}){3})(?::(\d{1,5}))?$/);
  if (!m) return null;
  return { ip: m[1], port: m[2] || '5555', hasPort: !!m[2] };
}

function setWifiEndpoint(ip, port) {
  const p = port || '5555';
  wifiIp.value = ip || '';
  wifiPort.value = p;
  if (wifiEndpoint) wifiEndpoint.value = ip ? `${ip}:${p}` : '';
}

// ── Recents ────────────────────────────────────────────────────
const RECENTS_KEY = 'scrcpy-recents';
const RECENTS_MAX = 8;

function loadRecents() {
  try {
    const raw = localStorage.getItem(RECENTS_KEY);
    const list = raw ? JSON.parse(raw) : [];
    return Array.isArray(list) ? list : [];
  } catch (_) {
    return [];
  }
}

function saveRecents(list) {
  localStorage.setItem(RECENTS_KEY, JSON.stringify(list.slice(0, RECENTS_MAX)));
}

function rememberDevice(dev) {
  if (!dev || !dev.serial) return;
  const isWifi = !!dev.isWifi || /^\d+\.\d+\.\d+\.\d+(:\d+)?$/.test(dev.serial);
  let host = null;
  let port = null;
  if (isWifi) {
    const m = String(dev.serial).match(/^(\d+\.\d+\.\d+\.\d+)(?::(\d+))?$/);
    if (m) {
      host = m[1];
      port = m[2] || '5555';
    }
  }
  const entry = {
    id: isWifi ? `${host}:${port}` : `usb:${dev.serial}`,
    serial: isWifi ? `${host}:${port}` : dev.serial,
    host,
    port,
    isWifi,
    brand: dev.brand || '',
    model: dev.model || 'Dispositivo',
    lastUsed: Date.now(),
  };
  const list = loadRecents().filter(r => r.id !== entry.id);
  list.unshift(entry);
  saveRecents(list);
  renderRecents();
}

function renderRecents() {
  if (!recentsBlock || !recentsList) return;
  const list = loadRecents();
  if (list.length === 0) {
    recentsBlock.style.display = 'none';
    recentsList.innerHTML = '';
    return;
  }
  recentsBlock.style.display = 'block';
  recentsList.innerHTML = '';

  for (const item of list) {
    const online = state.devices.some(d => d.serial === item.serial);
    const running = !!state.sessions[item.serial];
    const card = document.createElement('div');
    card.className = `recent-card${online ? ' online' : ''}${running ? ' running' : ''}`;
    const label = `${item.brand || ''} ${item.model || ''}`.trim() || item.serial;
    const meta = item.isWifi
      ? `${item.host}:${item.port}`
      : `USB · ${item.serial}`;
    card.innerHTML = `
      <div class="recent-info">
        <div class="recent-name">${escHtml(label)}</div>
        <div class="recent-meta">${escHtml(meta)}${online ? ' · en línea' : ''}</div>
      </div>
      <div class="recent-actions">
        ${running
          ? `<button type="button" class="btn-primary recent-action recent-stop">Detener</button>`
          : `<button type="button" class="btn-primary recent-action recent-mirror">Mirror</button>
             <button type="button" class="btn-camera-recent recent-action recent-camera">Cámara</button>`}
        <button type="button" class="btn-text recent-remove" title="Quitar">✕</button>
      </div>
    `;
    const stopBtn = card.querySelector('.recent-stop');
    const mirrorBtn = card.querySelector('.recent-mirror');
    const cameraBtn = card.querySelector('.recent-camera');
    if (stopBtn) stopBtn.addEventListener('click', (e) => { e.stopPropagation(); useRecent(item, 'stop'); });
    if (mirrorBtn) mirrorBtn.addEventListener('click', (e) => { e.stopPropagation(); useRecent(item, 'mirror'); });
    if (cameraBtn) cameraBtn.addEventListener('click', (e) => { e.stopPropagation(); useRecent(item, 'camera'); });
    card.querySelector('.recent-remove').addEventListener('click', (e) => {
      e.stopPropagation();
      saveRecents(loadRecents().filter(r => r.id !== item.id));
      renderRecents();
    });
    recentsList.appendChild(card);
  }
}

async function launchCamera(dev) {
  if (!dev?.serial) return false;
  const target = preferDeviceForCamera(dev);
  const result = await window.api.launchScrcpy(target.serial, getCameraLaunchOptions());
  if (result.ok) {
    state.sessions[target.serial] = { model: target.model, brand: target.brand, mode: 'camera' };
    rememberDevice(target);
    updateSessionsUI();
    renderDeviceCards(state.devices);
    renderRecents();
    showToast('Cámara iniciada', 'success');
    return true;
  }
  showToast('Error cámara: ' + (result.error || 'desconocido'), 'error');
  return false;
}

async function launchMirror(dev) {
  if (!dev?.serial) return false;
  const s = state.settings;
  const result = await window.api.launchScrcpy(dev.serial, {
    maxSize:   s.maxSize,
    maxFps:    s.maxFps,
    bitrate:   s.bitrate,
    codec:     s.codec,
    noAudio:   s.noAudio,
    stayAwake: s.stayAwake,
    screenOff: s.screenOff,
    orientation: s.orientation ?? '0',
  });

  if (result.ok) {
    state.sessions[dev.serial] = { model: dev.model, brand: dev.brand, mode: 'mirror' };
    rememberDevice(dev);
    updateSessionsUI();
    renderDeviceCards(state.devices);
    renderRecents();
    showToast(`Mirror iniciado: ${dev.model || dev.serial}`, 'success');
    return true;
  }
  showToast('Error al iniciar: ' + (result.error || 'desconocido'), 'error');
  return false;
}

async function ensureRecentOnline(item) {
  if (item.isWifi && item.host) {
    showToast('Reconectando WiFi…', 'info');
    const res = await window.api.connectWifi(item.host, item.port || '5555');
    if (!res.ok) {
      showToast(res.error || res.message || 'No se pudo reconectar', 'error');
      return null;
    }
    await refreshDevices();
    return state.devices.find(d => d.serial === item.serial) || {
      serial: item.serial,
      model: item.model,
      brand: item.brand,
      isWifi: true,
    };
  }
  const found = state.devices.find(d => d.serial === item.serial);
  if (!found) {
    showToast('Conecta el dispositivo por USB e inténtalo de nuevo', 'error');
    return null;
  }
  return found;
}

async function useRecent(item, mode = 'mirror') {
  if (mode === 'stop' || state.sessions[item.serial]) {
    if (state.sessions[item.serial]) {
      await window.api.stopScrcpy(item.serial);
      delete state.sessions[item.serial];
      updateSessionsUI();
      renderDeviceCards(state.devices);
      renderRecents();
      showToast('Sesión detenida', 'info');
    }
    return;
  }

  const found = await ensureRecentOnline(item);
  if (!found) return;
  if (mode === 'camera') await launchCamera(found);
  else await launchMirror(found);
}

if (btnClearRecents) {
  btnClearRecents.addEventListener('click', () => {
    saveRecents([]);
    renderRecents();
    showToast('Recientes eliminados', 'info');
  });
}

btnEnableTcpip.addEventListener('click', async () => {
  const serial = usbDeviceSelect.value;
  if (!serial) { showToast('Selecciona un dispositivo USB primero', 'error'); return; }

  btnEnableTcpip.textContent = 'Activando…';
  btnEnableTcpip.disabled = true;

  const res = await window.api.enableTcpip(serial);
  tcpipResult.style.display = 'block';

  if (!res.ok) {
    btnEnableTcpip.textContent = 'Activar y conectar';
    btnEnableTcpip.disabled = false;
    tcpipResult.className = 'result-box err';
    tcpipResult.textContent = 'Error: ' + res.error;
    return;
  }

  if (!res.ip) {
    btnEnableTcpip.textContent = 'Activar y conectar';
    btnEnableTcpip.disabled = false;
    tcpipResult.className = 'result-box warn';
    tcpipResult.textContent = 'WiFi ADB en 5555, pero no se detectó la IP. Pégala en el paso 2 como IP:5555';
    return;
  }

  setWifiEndpoint(res.ip, '5555');
  const step2UsbSelect = $('step2UsbSelect');
  if (step2UsbSelect) step2UsbSelect.value = `${res.ip}:5555`;
  btnEnableTcpip.textContent = 'Conectando…';
  const conn = await window.api.connectWifi(res.ip, '5555');

  btnEnableTcpip.textContent = 'Activar y conectar';
  btnEnableTcpip.disabled = false;

  if (conn.ok) {
    tcpipResult.className = 'result-box ok';
    tcpipResult.textContent = `Listo: ${res.ip}:5555 conectado. Ya puedes quitar el cable y usar Recientes → Cámara.`;
    showToast(`WiFi listo · ${res.ip}:5555`, 'success');
    await refreshDevices();
    const found = state.devices.find(d => d.serial === `${res.ip}:5555`);
    if (found) rememberDevice(found);
    else {
      rememberDevice({
        serial: `${res.ip}:5555`,
        model: 'Dispositivo',
        brand: '',
        isWifi: true,
      });
    }
  } else {
    tcpipResult.className = 'result-box err';
    tcpipResult.textContent = `ADB en 5555 OK (${res.ip}), pero falló conectar: ${conn.error || conn.message || ''}. ¿VPN activo?`;
    showToast('Activa ADB OK; conexión WiFi falló (revisa VPN)', 'error');
  }
});

btnPairWifi.addEventListener('click', async () => {
  const ep = parseEndpoint(pairEndpoint.value);
  const code = pairCode.value.trim().replace(/\s+/g, '');
  if (!ep || !ep.hasPort) {
    showToast('Pega IP:puerto de vinculación (el puerto es obligatorio)', 'error');
    return;
  }
  if (!/^\d{6}$/.test(code)) {
    showToast('El código debe tener 6 dígitos', 'error');
    return;
  }

  btnPairWifi.textContent = 'Vinculando…';
  btnPairWifi.disabled = true;
  const res = await window.api.pairWifi(ep.ip, ep.port, code);
  btnPairWifi.textContent = 'Vincular';
  btnPairWifi.disabled = false;

  pairResult.style.display = 'block';
  if (res.ok) {
    pairResult.className = 'result-box ok';
    pairResult.textContent = 'Vinculado. Ahora pega en el paso 2 la IP:puerto de Depuración inalámbrica (no el de vinculación).';
    showToast('Dispositivo vinculado', 'success');
    pairCode.value = '';
  } else {
    pairResult.className = 'result-box err';
    pairResult.textContent = res.error || res.message || 'No se pudo vincular';
  }
});

btnConnectWifi.addEventListener('click', async () => {
  const raw = (wifiEndpoint && wifiEndpoint.value.trim()) || '';
  const ep = parseEndpoint(raw) || parseEndpoint(`${wifiIp.value.trim()}:${wifiPort.value.trim() || '5555'}`);
  if (!ep) { showToast('Pega una dirección válida, ej. 192.168.1.6:41759', 'error'); return; }

  setWifiEndpoint(ep.ip, ep.port);
  btnConnectWifi.textContent = 'Conectando…';
  btnConnectWifi.disabled = true;

  const res = await window.api.connectWifi(ep.ip, ep.port);
  btnConnectWifi.textContent = 'Conectar';
  btnConnectWifi.disabled = false;

  wifiConnectResult.style.display = 'block';
  if (res.ok) {
    wifiConnectResult.className = 'result-box ok';
    wifiConnectResult.textContent = res.message || 'Conectado';
    showToast('Conectado por WiFi', 'success');
    try { localStorage.setItem('scrcpy-last-wifi-endpoint', `${ep.ip}:${ep.port}`); } catch (_) {}
    rememberDevice({
      serial: res.serial || `${ep.ip}:${ep.port}`,
      model: 'Dispositivo',
      brand: '',
      isWifi: true,
    });
    setTimeout(async () => {
      await refreshDevices();
      const found = state.devices.find(d => d.serial === (res.serial || `${ep.ip}:${ep.port}`));
      if (found) rememberDevice(found);
    }, 800);
  } else {
    wifiConnectResult.className = 'result-box err';
    wifiConnectResult.textContent = res.error || res.message || 'Error de conexión';
  }
});

// Allow pasting "IP:port" into endpoint and Enter to connect
if (wifiEndpoint) {
  wifiEndpoint.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') btnConnectWifi.click();
  });
}
const step2UsbSelect = $('step2UsbSelect');
if (step2UsbSelect) {
  step2UsbSelect.addEventListener('change', () => {
    const val = step2UsbSelect.value;
    if (val) {
      const ep = parseEndpoint(val);
      if (ep) {
        setWifiEndpoint(ep.ip, ep.port);
      } else {
        if (wifiEndpoint) wifiEndpoint.value = val;
      }
    }
  });
}
if (pairCode) {
  pairCode.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') btnPairWifi.click();
  });
}

// ── Sessions UI ────────────────────────────────────────────────
function updateSessionsUI() {
  const serials = Object.keys(state.sessions);
  const badge = $('sessionsBadge');
  badge.style.display = serials.length > 0 ? 'inline' : 'none';
  badge.textContent   = serials.length;

  if (serials.length === 0) {
    sessionsList.innerHTML = `
      <div class="empty-state">
        <div class="empty-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>
        </div>
        <h3>Sin sesiones activas</h3>
        <p>Inicia scrcpy desde la sección de Dispositivos.</p>
      </div>`;
    return;
  }

  sessionsList.innerHTML = '';
  for (const serial of serials) {
    const s = state.sessions[serial];
    const modeLabel = s.mode === 'camera' ? ' · Cámara' : s.mode === 'record' ? ' · Grabación' : ' · Mirror';
    const card = document.createElement('div');
    card.className = 'session-card';
    card.innerHTML = `
      <div class="session-pulse"></div>
      <div class="session-info">
        <div class="session-device">${escHtml((s.brand || '') + ' ' + (s.model || 'Dispositivo'))}</div>
        <div class="session-serial">${escHtml(serial)}${modeLabel}</div>
      </div>
      <button class="btn-stop" data-serial="${escHtml(serial)}">⏹ Detener</button>
    `;
    card.querySelector('.btn-stop').addEventListener('click', async () => {
      await window.api.stopScrcpy(serial);
      delete state.sessions[serial];
      updateSessionsUI();
      renderDeviceCards(state.devices);
      showToast('Mirror detenido', 'info');
    });
    sessionsList.appendChild(card);
  }
}

// ── Listen for scrcpy exiting ──────────────────────────────────
window.api?.onScrcpyStopped?.(({ serial, error, earlyFail }) => {
  if (state.restartingSerials.has(serial)) return;
  if (state.sessions[serial]) {
    const name = (state.sessions[serial].model || serial);
    delete state.sessions[serial];
    updateSessionsUI();
    renderDeviceCards(state.devices);
    renderRecents();
    if (error) {
      const short = String(error).split('\n').filter(Boolean).slice(-2).join(' · ');
      showToast(`Error scrcpy (${name}): ${short.slice(0, 180)}`, 'error');
    } else {
      showToast(`Sesión de ${name} terminó`, 'info');
    }
  } else if (error && earlyFail) {
    showToast(`Error scrcpy: ${String(error).slice(0, 180)}`, 'error');
  }
});

// Atajos
document.addEventListener('keydown', (e) => {
  if (e.key === 'F5') {
    e.preventDefault();
    refreshDevices();
  }
  if (e.key === 'Escape' && deviceModal?.style.display === 'flex') {
    deviceModal.style.display = 'none';
  }
});

// ── Toast ──────────────────────────────────────────────────────
function showToast(msg, type = 'info') {
  const container = $('toastContainer');
  const toast = document.createElement('div');
  toast.className = `toast ${type}`;
  const icons = { success: '✓', error: '✕', info: 'ℹ' };
  toast.innerHTML = `<span>${icons[type] || 'ℹ'}</span><span>${escHtml(msg)}</span>`;
  container.appendChild(toast);
  setTimeout(() => {
    toast.classList.add('hide');
    setTimeout(() => toast.remove(), 250);
  }, 3500);
}

// ── Utilities ──────────────────────────────────────────────────
function escHtml(str) {
  return String(str || '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}

// ── App info / updates ─────────────────────────────────────────
let appInfoCache = null;

async function loadAppInfo() {
  appInfoCache = await window.api.getAppInfo();
  const ver = appInfoCache?.version || '?';
  const label = $('appVersionLabel');
  if (label) label.textContent = `v${ver}`;
  const about = $('aboutText');
  if (about) {
    const author = appInfoCache?.author && !String(appInfoCache.author).includes('TU_')
      ? ` · ${appInfoCache.author}`
      : '';
    about.textContent = `${appInfoCache?.name || 'scrcpy GUI'} v${ver}${author}. Puedes publicar mejoras en GitHub Releases y esta app las detectará.`;
  }
  return appInfoCache;
}

async function checkForUpdates({ silent = false } = {}) {
  const box = $('updateResult');
  const btn = $('btnCheckUpdates');
  if (btn && !silent) {
    btn.disabled = true;
    btn.textContent = 'Comprobando…';
  }
  const res = await window.api.checkUpdates();
  if (btn && !silent) {
    btn.disabled = false;
    btn.textContent = 'Buscar actualizaciones';
  }

  if (box) {
    box.style.display = 'block';
    if (!res.ok) {
      box.className = 'result-box warn';
      box.textContent = res.error || 'No se pudo comprobar';
    } else if (res.updateAvailable) {
      box.className = 'result-box ok';
      box.innerHTML = `${escHtml(res.message)} <button type="button" class="btn-text" id="btnDownloadUpdate">Abrir descarga</button>`;
      const dl = $('btnDownloadUpdate');
      if (dl) {
        dl.addEventListener('click', () => window.api.openExternal(res.htmlUrl || res.releasesUrl));
      }
    } else {
      box.className = 'result-box ok';
      box.textContent = res.message || `Al día (v${res.current})`;
    }
  }

  if (!silent) {
    if (!res.ok) showToast(res.error || 'Error al buscar updates', 'error');
    else if (res.updateAvailable) showToast(res.message, 'success');
    else showToast(res.message || 'Al día', 'info');
  } else if (res.ok && res.updateAvailable) {
    showToast(res.message, 'success');
  }
  return res;
}

$('btnCheckUpdates')?.addEventListener('click', () => checkForUpdates({ silent: false }));
$('btnOpenReleases')?.addEventListener('click', async () => {
  const info = appInfoCache || await loadAppInfo();
  const url = info?.homepage
    ? `${info.homepage.replace(/\/$/, '')}/releases`
    : (info?.repository ? `https://github.com/${info.repository.owner}/${info.repository.repo}/releases` : '');
  if (url) window.api.openExternal(url);
  else showToast('No se pudo determinar la URL de releases. Revisa package.json.', 'error');
});

// ── iPhone / AirPlay + OBS Virtual Camera ─────────────────────
const iphoneState = {
  uxplayRunning: false,
  obsVcamRunning: false,
  mode: 'screen', // 'screen' | 'camera'
};

// Quality presets mapped to UxPlay args
const QUALITY_PRESETS = {
  max:    { fps: 60, res: '1920x1080@60', label: '1080p / 60 fps' },
  high:   { fps: 30, res: '1920x1080@30', label: '1080p / 30 fps' },
  medium: { fps: 30, res: '1280x720@30',  label: '720p / 30 fps'  },
};

function initIphoneModeSelector() {
  const modeBtns = $$('.iphone-mode-btn');
  modeBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      if (iphoneState.uxplayRunning) return; // can't switch while running
      modeBtns.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      iphoneState.mode = btn.dataset.mode;
      updateIphoneModeUI();
    });
  });
}

function updateIphoneModeUI() {
  const isCamera    = iphoneState.mode === 'camera';
  const label       = $('btnStartIphoneLabel');
  const title       = $('iphoneCardTitle');
  const subtitle    = $('iphoneCardSubtitle');
  const stepsScreen = $('stepsScreen');
  const stepsCamera = $('stepsCamera');
  const stepsTitle  = $('iphoneStepsTitle');

  if (label)    label.textContent    = isCamera ? 'Iniciar Cámara' : 'Iniciar como Webcam';
  if (title)    title.textContent    = isCamera ? 'Cámara AirPlay' : 'Receptor AirPlay';
  if (subtitle) subtitle.textContent = isCamera
    ? 'Transmite solo la cámara del iPhone a este PC vía Wi-Fi'
    : 'El iPhone transmite su pantalla a este PC vía Wi-Fi';
  if (stepsTitle)  stepsTitle.textContent = isCamera
    ? '📸 Cómo usar la Cámara del iPhone'
    : '📱 Cómo conectar tu iPhone';
  if (stepsScreen) stepsScreen.style.display = isCamera ? 'none' : 'block';
  if (stepsCamera) stepsCamera.style.display = isCamera ? 'block' : 'none';
}

async function initIphoneSection() {
  if (!window.api?.iphoneToolsStatus) return;
  const status = await window.api.iphoneToolsStatus();

  const uxAlert  = $('uxplayMissingAlert');
  const obsAlert = $('obsMissingAlert');
  const startBtn = $('btnStartIphone');
  const useObsCb = $('iphoneUseObs');

  if (!status.uxplay && uxAlert) { uxAlert.style.display = 'flex'; }
  if (!status.obs   && obsAlert) { obsAlert.style.display = 'flex'; }

  if (!status.uxplay && startBtn) {
    startBtn.disabled  = true;
    startBtn.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" stroke="none"><polygon points="5 3 19 12 5 21 5 3"/></svg><span>UxPlay requerido</span>';
  }
  if (!status.obs && useObsCb) {
    useObsCb.checked  = false;
    useObsCb.disabled = true;
  } else if (useObsCb) {
    const savedUseObs = localStorage.getItem('scrcpy-iphone-use-obs');
    if (savedUseObs !== null) {
      useObsCb.checked = savedUseObs === 'true';
    }
    useObsCb.addEventListener('change', () => {
      localStorage.setItem('scrcpy-iphone-use-obs', String(useObsCb.checked));
    });
  }

  initIphoneModeSelector();
  iphoneState.uxplayRunning = !!status.running;
  updateIphoneModeUI();
  updateIphoneUI();
}


function updateIphoneUI() {
  const { uxplayRunning } = iphoneState;
  const badge    = $('iphoneBadge');
  const navBadge = $('iphoneNavBadge');
  const startBtn = $('btnStartIphone');
  const stopBtn  = $('btnStopIphone');
  const useObs   = $('iphoneUseObs')?.checked;

  if (uxplayRunning) {
    if (badge) {
      badge.className   = 'status-pill ok';
      badge.textContent = useObs ? '🎥 Webcam activa' : '📡 AirPlay activo';
    }
    if (navBadge) navBadge.style.display = 'inline';
    if (startBtn) startBtn.style.display = 'none';
    if (stopBtn)  stopBtn.style.display  = 'inline-flex';
  } else {
    if (badge) {
      badge.className   = 'status-pill';
      badge.textContent = 'Inactivo';
    }
    if (navBadge) navBadge.style.display = 'none';
    if (startBtn) startBtn.style.display = 'inline-flex';
    if (stopBtn)  stopBtn.style.display  = 'none';
  }
}

function setIphoneStatus(msg, type = 'info') {
  const box = $('iphoneStatusMsg');
  if (!box) return;
  box.style.display = 'block';
  box.className = `result-box${type === 'error' ? ' err' : type === 'success' ? ' ok' : ''}`;
  box.textContent = msg;
}

$('btnStartIphone')?.addEventListener('click', async () => {
  const name     = $('iphoneName')?.value?.trim() || 'scrcpy GUI';
  const useObs   = $('iphoneUseObs')?.checked ?? true;
  const quality  = $('iphoneQuality')?.value || 'max';
  const startBtn = $('btnStartIphone');
  const qPreset  = QUALITY_PRESETS[quality] || QUALITY_PRESETS.max;

  startBtn.disabled = true;
  startBtn.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" stroke="none"><polygon points="5 3 19 12 5 21 5 3"/></svg><span>Iniciando…</span>';

  // 1) Start UxPlay con modo y calidad
  const uxRes = await window.api.iphoneStartUxPlay({
    name,
    fps: qPreset.fps,
    res: qPreset.res,
    cameraMode: iphoneState.mode === 'camera',
  });
  if (!uxRes.ok) {
    setIphoneStatus('❌ Error UxPlay: ' + uxRes.error, 'error');
    startBtn.disabled = false;
    const errLbl = $('btnStartIphoneLabel');
    if (errLbl) errLbl.textContent = iphoneState.mode === 'camera' ? 'Iniciar Cámara' : 'Iniciar como Webcam';
    return;
  }
  iphoneState.uxplayRunning = true;
  updateIphoneUI();

  // 2) OBS Virtual Camera
  if (useObs) {
    setIphoneStatus('✅ UxPlay listo. Conectando con OBS Virtual Camera…', 'info');

    // Give UxPlay a moment to open its window before OBS tries to capture it
    await new Promise(r => setTimeout(r, 1500));

    const obsRes = await window.api.iphoneObsVirtualCam({ start: true });
    if (obsRes.ok) {
      iphoneState.obsVcamRunning = true;
      const fallback = obsRes.fallback
        ? ' OBS abrirá en unos segundos. Luego añade Window Capture apuntando a la ventana UxPlay.'
        : '';
      setIphoneStatus(
        `🎥 Webcam activa${fallback}\n` +
        `→ En tu iPhone: Centro de Control → Duplicar pantalla → "${name}"\n` +
        `→ En Zoom/Teams: selecciona "OBS Virtual Camera"`,
        'success'
      );
      showToast(`📱 iPhone Webcam lista · Busca "${name}" en tu iPhone`, 'success');
    } else {
      setIphoneStatus(
        `📡 AirPlay listo, pero OBS Virtual Camera falló: ${obsRes.error}\n` +
        `→ Abre OBS manualmente → Controls → "Start Virtual Camera"\n` +
        `→ En tu iPhone: Centro de Control → Duplicar pantalla → "${name}"`,
        'info'
      );
      showToast(`AirPlay listo · Arranca OBS Virtual Camera manualmente`, 'info');
    }
  } else {
    setIphoneStatus(
      `📡 Receptor AirPlay listo.\n→ En tu iPhone: Centro de Control → Duplicar pantalla → "${name}"`,
      'success'
    );
    showToast(`AirPlay listo · Busca "${name}" en tu iPhone`, 'success');
  }

  startBtn.disabled = false;
  // Update the name hint in instructions
  const hint    = $('iphoneNameHint');
  const hintCam = $('iphoneNameHintCam');
  if (hint)    hint.textContent    = `"${name}"`;
  if (hintCam) hintCam.textContent = `"${name}"`;
  // Restore button label
  const lbl = $('btnStartIphoneLabel');
  if (lbl) lbl.textContent = iphoneState.mode === 'camera' ? 'Iniciar C\u00e1mara' : 'Iniciar como Webcam';
  updateIphoneUI();
});

$('btnStopIphone')?.addEventListener('click', async () => {
  if (iphoneState.obsVcamRunning) {
    await window.api.iphoneObsVirtualCam({ start: false });
    iphoneState.obsVcamRunning = false;
  }
  await window.api.iphoneStopUxPlay();
  iphoneState.uxplayRunning = false;
  const box = $('iphoneStatusMsg');
  if (box) box.style.display = 'none';
  updateIphoneUI();
  showToast('iPhone webcam detenida', 'info');
});

// Live-update instructions name hint as user types
$('iphoneName')?.addEventListener('input', () => {
  const v = $('iphoneName').value.trim() || 'scrcpy GUI';
  const hint = $('iphoneNameHint');
  if (hint) hint.textContent = `"${v}"`;
});

// External link buttons
$('btnGetUxPlay')?.addEventListener('click', (e) => {
  e.preventDefault();
  window.api.openExternal('https://github.com/leapbtw/uxplay-windows/releases');
});
$('btnGetObs')?.addEventListener('click', (e) => {
  e.preventDefault();
  window.api.openExternal('https://obsproject.com/download');
});

// Handle unexpected UxPlay exit
window.api?.onIphoneUxPlayStopped?.(() => {
  iphoneState.uxplayRunning   = false;
  iphoneState.obsVcamRunning  = false;
  const box = $('iphoneStatusMsg');
  if (box) box.style.display = 'none';
  updateIphoneUI();
  showToast('Receptor AirPlay detenido', 'info');
});

// ── iPhone USB Direct Camera (usbmuxd) ──────────────────────────
const iphoneUsbState = {
  devices: [],
  streaming: false,
};

function initIphoneConnSelector() {
  const connBtns = $$('#iphoneConnSelector .iphone-mode-btn');
  const usbSection = $('iphoneUsbSection');
  const airplaySection = $('iphoneAirplaySection');

  connBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      connBtns.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      const conn = btn.dataset.conn;
      if (usbSection) usbSection.style.display = (conn === 'usb') ? 'block' : 'none';
      if (airplaySection) airplaySection.style.display = (conn === 'airplay') ? 'block' : 'none';
    });
  });
}

function updateIphoneUsbUI() {
  const badge = $('iphoneUsbBadge');
  const devName = $('iphoneUsbDeviceName');
  const devSerial = $('iphoneUsbDeviceSerial');
  const startBtn = $('btnStartIphoneUsb');
  const stopBtn = $('btnStopIphoneUsb');
  const statsBox = $('iphoneUsbLiveStats');

  const hasDevice = iphoneUsbState.devices.length > 0;
  const isStreaming = iphoneUsbState.streaming;

  if (isStreaming) {
    if (badge) { badge.className = 'status-pill ok'; badge.textContent = '⚡ Transmitiendo por USB'; }
    if (startBtn) startBtn.style.display = 'none';
    if (stopBtn) stopBtn.style.display = 'inline-flex';
    if (statsBox) statsBox.style.display = 'flex';
  } else if (hasDevice) {
    if (badge) { badge.className = 'status-pill ok'; badge.textContent = '🟢 iPhone Conectado'; }
    if (devName) devName.textContent = 'iPhone detectado por cable USB';
    if (devSerial) devSerial.textContent = `ID usbmuxd: ${iphoneUsbState.devices[0].deviceId || iphoneUsbState.devices[0].serial} · Cable listo`;
    if (startBtn) {
      startBtn.disabled = false;
      startBtn.style.display = 'inline-flex';
    }
    if (stopBtn) stopBtn.style.display = 'none';
    if (statsBox) statsBox.style.display = 'none';
  } else {
    if (badge) { badge.className = 'status-pill'; badge.textContent = 'Esperando cable USB…'; }
    if (devName) devName.textContent = 'Buscando iPhone conectado por cable USB…';
    if (devSerial) devSerial.textContent = 'Conecta tu iPhone por cable USB a la PC';
    if (startBtn) {
      startBtn.disabled = false;
      startBtn.style.display = 'inline-flex';
    }
    if (stopBtn) stopBtn.style.display = 'none';
    if (statsBox) statsBox.style.display = 'none';
  }
}

async function refreshIphoneUsbStatus() {
  if (!window.api?.iphoneUsbStatus) return;
  try {
    const res = await window.api.iphoneUsbStatus();
    iphoneUsbState.devices = res.devices || [];
    iphoneUsbState.streaming = !!res.streaming;
    updateIphoneUsbUI();
  } catch (_) {}
}

$('btnStartIphoneUsb')?.addEventListener('click', async () => {
  const startBtn = $('btnStartIphoneUsb');
  const statusBox = $('iphoneUsbStatusMsg');
  const useObs = $('iphoneUsbUseObs')?.checked;

  if (startBtn) startBtn.disabled = true;
  if (statusBox) statusBox.style.display = 'none';

  const res = await window.api.iphoneUsbStartStream({ port: 50005 });
  if (!res.ok) {
    if (statusBox) {
      statusBox.style.display = 'block';
      statusBox.className = 'result-box err';
      statusBox.textContent = `❌ ${res.error}`;
    }
    if (startBtn) startBtn.disabled = false;
    showToast(res.error, 'error');
    return;
  }

  iphoneUsbState.streaming = true;
  updateIphoneUsbUI();
  showToast('Cámara iPhone conectada por cable USB', 'success');

  if (statusBox) {
    statusBox.style.display = 'block';
    statusBox.className = 'result-box ok';
    statusBox.textContent = '⚡ Transmitiendo en tiempo real por cable USB (usbmuxd). Latencia < 8 ms.';
  }

  if (useObs) {
    await window.api.iphoneObsVirtualCam({ start: true });
  }
});

$('btnStopIphoneUsb')?.addEventListener('click', async () => {
  await window.api.iphoneUsbStopStream();
  iphoneUsbState.streaming = false;
  updateIphoneUsbUI();
  const statusBox = $('iphoneUsbStatusMsg');
  if (statusBox) statusBox.style.display = 'none';
  showToast('Transmisión USB detenida', 'info');
});

window.api?.onIphoneUsbDeviceChange?.((data) => {
  if (data.device) {
    if (data.connected) {
      iphoneUsbState.devices = [data.device];
      showToast('📱 iPhone conectado por cable USB', 'info');
    } else {
      iphoneUsbState.devices = [];
      iphoneUsbState.streaming = false;
      showToast('iPhone desconectado del cable USB', 'warn');
    }
  }
  updateIphoneUsbUI();
});

window.api?.onIphoneUsbStats?.((stats) => {
  const bitrateEl = $('iphoneUsbBitrate');
  if (bitrateEl) bitrateEl.textContent = `${stats.kbps} kbps`;
});

// Init iPhone section
if (window.api?.iphoneToolsStatus) {
  initIphoneSection();
}
if (window.api?.iphoneUsbStatus) {
  initIphoneConnSelector();
  refreshIphoneUsbStatus();
  setInterval(refreshIphoneUsbStatus, 2500);
}

// ── Init ───────────────────────────────────────────────────────
(async function init() {
  applySettingsToUI();
  renderRecents();
  try {
    const lastWifi = localStorage.getItem('scrcpy-last-wifi-endpoint');
    if (lastWifi && wifiEndpoint && !wifiEndpoint.value) {
      wifiEndpoint.value = lastWifi;
    }
  } catch (_) {}
  await loadAppInfo();
  await checkTools();
  await refreshDevices();
  updateSessionsUI();

  // Auto-check at most once per day (no molesta si no hay repo configurado)
  try {
    const key = 'scrcpy-last-update-check';
    const last = parseInt(localStorage.getItem(key) || '0', 10);
    if (Date.now() - last > 24 * 60 * 60 * 1000) {
      localStorage.setItem(key, String(Date.now()));
      await checkForUpdates({ silent: true });
    }
  } catch (_) {}
})();
