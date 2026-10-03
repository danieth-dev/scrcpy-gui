const { app, BrowserWindow, ipcMain, shell, dialog } = require('electron');
const path = require('path');
const { exec, spawn, execSync } = require('child_process');
const fs = require('fs');

let mainWindow;
let activeScrcpyProcesses = {};

// ─── Tool discovery ──────────────────────────────────────────────────────────

function toolCandidates(exeName) {
  const list = [
    path.join(__dirname, 'vendor', 'scrcpy', exeName),
    path.join(__dirname, '..', 'release', exeName),
    path.join(__dirname, '..', exeName),
    exeName.replace(/\.exe$/i, ''),
  ];
  if (process.resourcesPath) {
    list.unshift(path.join(process.resourcesPath, 'scrcpy', exeName));
  }
  try {
    list.unshift(path.join(path.dirname(process.execPath), 'scrcpy', exeName));
  } catch (_) {}
  return list;
}

function loadPkg() {
  try {
    return JSON.parse(fs.readFileSync(path.join(__dirname, 'package.json'), 'utf8'));
  } catch (_) {
    return { version: app.getVersion() };
  }
}

function parseGithubRepo(pkg) {
  const url = pkg.repository?.url || pkg.repository || '';
  const m = String(url).match(/github\.com[/:]([^/]+)\/([^/.]+)/i);
  if (!m) return null;
  return { owner: m[1], repo: m[2].replace(/\.git$/i, '') };
}

function cmpSemver(a, b) {
  const pa = String(a).replace(/^v/i, '').split('.').map(n => parseInt(n, 10) || 0);
  const pb = String(b).replace(/^v/i, '').split('.').map(n => parseInt(n, 10) || 0);
  for (let i = 0; i < 3; i++) {
    const d = (pa[i] || 0) - (pb[i] || 0);
    if (d !== 0) return d;
  }
  return 0;
}

function resolveTool(exeName, versionArgs) {
  for (const c of toolCandidates(exeName)) {
    try {
      if (c.includes('\\') || c.includes('/')) {
        if (!fs.existsSync(c)) continue;
      }
      execSync(`"${c}" ${versionArgs}`, { stdio: 'ignore', timeout: 3000 });
      return c;
    } catch (_) {}
  }
  return null;
}

function findScrcpy() {
  return resolveTool('scrcpy.exe', '--version');
}

function findAdb() {
  return resolveTool('adb.exe', 'version');
}

let SCRCPY = findScrcpy();
let ADB = findAdb();

function runAdb(args, timeoutMs = 8000) {
  return new Promise((resolve, reject) => {
    if (!ADB) return reject(new Error('adb no encontrado'));
    exec(`"${ADB}" ${args}`, { timeout: timeoutMs }, (err, stdout, stderr) => {
      const out = `${stdout || ''}${stderr || ''}`.trim();
      if (err) return reject(new Error(out || err.message));
      resolve(out);
    });
  });
}

async function detectDeviceIp(serial) {
  const tries = [
    async () => {
      const ipOut = await runAdb(`-s ${serial} shell ip route`);
      const match = ipOut.match(/src\s+(\d+\.\d+\.\d+\.\d+)/);
      return match ? match[1] : '';
    },
    async () => {
      const out = await runAdb(`-s ${serial} shell ip -f inet addr show wlan0`);
      const match = out.match(/inet\s+(\d+\.\d+\.\d+\.\d+)/);
      return match ? match[1] : '';
    },
    async () => {
      const out = await runAdb(`-s ${serial} shell getprop dhcp.wlan0.ipaddress`);
      const ip = (out || '').trim();
      return /^\d+\.\d+\.\d+\.\d+$/.test(ip) ? ip : '';
    },
    async () => {
      const out = await runAdb(`-s ${serial} shell getprop dhcp.eth0.ipaddress`);
      const ip = (out || '').trim();
      return /^\d+\.\d+\.\d+\.\d+$/.test(ip) ? ip : '';
    },
  ];
  for (const t of tries) {
    try {
      const ip = await t();
      if (ip) return ip;
    } catch (_) {}
  }
  return '';
}

// ─── Window ──────────────────────────────────────────────────────────────────

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 960,
    height: 720,
    minWidth: 800,
    minHeight: 580,
    frame: false,
    transparent: false,
    backgroundColor: '#0b1512',
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, 'preload.js'),
    },
    icon: path.join(__dirname, 'logo.png'),
    show: false,
  });

  mainWindow.loadFile('index.html');
  mainWindow.once('ready-to-show', () => mainWindow.show());

  mainWindow.on('closed', () => {
    Object.values(activeScrcpyProcesses).forEach(p => { try { p.kill(); } catch (_) {} });
    mainWindow = null;
  });
}

app.disableHardwareAcceleration();
app.whenReady().then(() => {
  SCRCPY = findScrcpy();
  ADB = findAdb();
  createWindow();
});
app.on('window-all-closed', () => app.quit());

// ─── IPC ─────────────────────────────────────────────────────────────────────

ipcMain.on('win:minimize', () => mainWindow?.minimize());
ipcMain.on('win:maximize', () => {
  if (mainWindow?.isMaximized()) mainWindow.unmaximize();
  else mainWindow?.maximize();
});
ipcMain.on('win:close', () => mainWindow?.close());

ipcMain.handle('tools:status', () => ({
  scrcpy: !!SCRCPY,
  adb: !!ADB,
  scrcpyPath: SCRCPY,
  adbPath: ADB,
}));

ipcMain.handle('app:info', () => {
  const pkg = loadPkg();
  const gh = parseGithubRepo(pkg);
  return {
    version: pkg.version || app.getVersion(),
    name: pkg.productName || pkg.build?.productName || pkg.name,
    author: typeof pkg.author === 'string' ? pkg.author : (pkg.author?.name || ''),
    homepage: pkg.homepage || (gh ? `https://github.com/${gh.owner}/${gh.repo}` : ''),
    repository: gh,
  };
});

ipcMain.handle('app:openExternal', async (_, url) => {
  if (!url || !/^https?:\/\//i.test(url)) return { ok: false };
  await shell.openExternal(url);
  return { ok: true };
});

ipcMain.handle('app:checkUpdates', async () => {
  const pkg = loadPkg();
  const current = pkg.version || app.getVersion();
  const gh = parseGithubRepo(pkg);
  if (!gh || gh.repo === 'TU_REPO') {
    return {
      ok: false,
      current,
      error: 'Configura "repository" en package.json con tu GitHub (owner/repo) para buscar actualizaciones.',
    };
  }

  try {
    const api = `https://api.github.com/repos/${gh.owner}/${gh.repo}/releases/latest`;
    const res = await fetch(api, {
      headers: {
        Accept: 'application/vnd.github+json',
        'User-Agent': `${pkg.name || 'scrcpy-gui'}/${current}`,
      },
    });
    if (res.status === 404) {
      return {
        ok: true,
        current,
        updateAvailable: false,
        message: 'Aún no hay releases en GitHub. Publica un Release (ej. v1.0.0) para habilitar actualizaciones.',
        releasesUrl: `https://github.com/${gh.owner}/${gh.repo}/releases`,
      };
    }
    if (!res.ok) {
      return { ok: false, current, error: `GitHub API: ${res.status}` };
    }
    const data = await res.json();
    const latest = String(data.tag_name || data.name || '').replace(/^v/i, '');
    const updateAvailable = latest && cmpSemver(latest, current) > 0;
    return {
      ok: true,
      current,
      latest,
      updateAvailable,
      releaseName: data.name || data.tag_name,
      releaseNotes: data.body || '',
      htmlUrl: data.html_url,
      releasesUrl: `https://github.com/${gh.owner}/${gh.repo}/releases`,
      message: updateAvailable
        ? `Hay una nueva versión: v${latest} (tienes v${current})`
        : `Estás al día (v${current})`,
    };
  } catch (e) {
    return { ok: false, current, error: e.message || 'No se pudo comprobar actualizaciones' };
  }
});

ipcMain.handle('adb:devices', async () => {
  try {
    const output = await runAdb('devices -l');
    const lines = output.split('\n').slice(1);
    const devices = [];
    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith('*')) continue;
      const parts = trimmed.split(/\s+/);
      if (parts.length < 2) continue;
      const serial = parts[0];
      const state = parts[1];
      if (state !== 'device') continue;

      let model = serial;
      try { model = (await runAdb(`-s "${serial}" shell getprop ro.product.model`)).trim(); } catch (_) {}
      let brand = '';
      try { brand = (await runAdb(`-s "${serial}" shell getprop ro.product.brand`)).trim(); } catch (_) {}
      let androidVersion = '';
      try { androidVersion = (await runAdb(`-s "${serial}" shell getprop ro.build.version.release`)).trim(); } catch (_) {}
      let batteryLevel = '';
      try {
        const bat = await runAdb(`-s "${serial}" shell dumpsys battery`);
        const match = bat.match(/level:\s*(\d+)/);
        if (match) batteryLevel = match[1];
      } catch (_) {}
      let hardwareSerial = '';
      try { hardwareSerial = (await runAdb(`-s "${serial}" shell getprop ro.serialno`)).trim(); } catch (_) {}

      const isWifi = /^\d+\.\d+\.\d+\.\d+(:\d+)?$/.test(serial);
      let ip = '';
      if (!isWifi) {
        try { ip = await detectDeviceIp(serial); } catch (_) {}
      }
      devices.push({
        serial, model, brand, androidVersion, batteryLevel, isWifi, state, hardwareSerial, ip,
      });
    }
    return { ok: true, devices };
  } catch (e) {
    return { ok: false, error: e.message };
  }
});

ipcMain.handle('adb:pair', async (_, { host, port, code }) => {
  try {
    const h = String(host || '').trim();
    const p = String(port || '').trim();
    const c = String(code || '').trim().replace(/\s+/g, '');
    if (!h || !p || !c) return { ok: false, error: 'Faltan IP, puerto de vinculación o código' };
    if (!/^\d{6}$/.test(c)) return { ok: false, error: 'El código debe tener 6 dígitos' };
    if (!ADB) return { ok: false, error: 'adb no encontrado' };

    const output = await new Promise((resolve) => {
      exec(`"${ADB}" pair ${h}:${p} ${c}`, { timeout: 30000 }, (err, stdout, stderr) => {
        resolve(`${stdout || ''}\n${stderr || ''}`.trim());
      });
    });
    const ok = /successfully paired/i.test(output);
    return {
      ok,
      message: output || (ok ? 'Vinculado' : 'No se pudo vincular'),
      error: ok ? undefined : (output || 'No se pudo vincular'),
    };
  } catch (e) {
    return { ok: false, error: e.message };
  }
});

ipcMain.handle('adb:connect', async (_, { ip, port }) => {
  try {
    const p = port || 5555;
    const output = await runAdb(`connect ${ip}:${p}`, 15000);
    const ok = /connected to|already connected/i.test(output);
    return { ok, message: output, serial: `${ip}:${p}`, error: ok ? undefined : output };
  } catch (e) {
    return { ok: false, error: e.message };
  }
});

ipcMain.handle('adb:disconnect', async (_, { serial }) => {
  try {
    const output = await runAdb(`disconnect ${serial}`);
    return { ok: true, message: output };
  } catch (e) {
    return { ok: false, error: e.message };
  }
});

ipcMain.handle('adb:tcpip', async (_, { serial }) => {
  try {
    await runAdb(`-s "${serial}" tcpip 5555`);
    await new Promise(r => setTimeout(r, 600));
    const ip = await detectDeviceIp(serial);
    return { ok: true, ip };
  } catch (e) {
    return { ok: false, error: e.message };
  }
});

ipcMain.handle('scrcpy:launch', async (_, { serial, options }) => {
  if (!SCRCPY) {
    SCRCPY = findScrcpy();
    if (!SCRCPY) return { ok: false, error: 'scrcpy.exe no encontrado. Colócalo en gui/vendor/scrcpy/' };
  }

  const args = ['-s', serial];
  if (options.windowTitle) {
    args.push('--window-title', options.windowTitle);
  } else {
    args.push('--window-title', 'scrcpy');
  }

  if (options.cameraOnly) {
    args.push('--video-source=camera');
    args.push('--no-audio');
    if (options.cameraFacing) args.push('--camera-facing', options.cameraFacing);
    if (options.maxFps) args.push('--camera-fps', String(options.maxFps));
    if (options.cameraZoom && Number(options.cameraZoom) > 1) {
      args.push('--camera-zoom', String(options.cameraZoom));
    }
    if (options.cameraTorch) args.push('--camera-torch');
  } else {
    if (options.noAudio) args.push('--no-audio');
    if (options.stayAwake) args.push('--stay-awake');
    if (options.screenOff) args.push('--turn-screen-off');
    if (options.maxFps) args.push('--max-fps', options.maxFps);
  }

  if (options.maxSize) args.push('--max-size', options.maxSize);
  if (options.bitrate) args.push('--video-bit-rate', `${options.bitrate}M`);
  if (options.codec === 'h265') args.push('--video-codec=h265');
  else if (options.codec) args.push('--video-codec', options.codec);

  if (options.orientation !== undefined && options.orientation !== null && String(options.orientation) !== '0') {
    args.push('--orientation', String(options.orientation));
  }

  if (options.record) {
    const { filePath } = await dialog.showSaveDialog(mainWindow, {
      title: 'Guardar grabación como…',
      defaultPath: `scrcpy-${Date.now()}.mp4`,
      filters: [{ name: 'Video', extensions: ['mp4', 'mkv'] }],
    });
    if (filePath) args.push('--record', filePath);
  }

  try {
    // Kill previous session for same serial
    if (activeScrcpyProcesses[serial]) {
      try { activeScrcpyProcesses[serial].kill(); } catch (_) {}
      delete activeScrcpyProcesses[serial];
    }

    const startedAt = Date.now();
    let stderrBuf = '';
    let stdoutBuf = '';

    const proc = spawn(SCRCPY, args, {
      detached: false,
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
    });

    proc.stderr?.on('data', (d) => { stderrBuf += d.toString(); });
    proc.stdout?.on('data', (d) => { stdoutBuf += d.toString(); });

    activeScrcpyProcesses[serial] = proc;

    // Early-fail wait: if process dies quickly, return error with log
    const early = await new Promise((resolve) => {
      const timer = setTimeout(() => resolve({ alive: true }), 900);
      proc.once('exit', (code) => {
        clearTimeout(timer);
        resolve({ alive: false, code });
      });
    });

    if (!early.alive) {
      delete activeScrcpyProcesses[serial];
      const log = `${stderrBuf}\n${stdoutBuf}`.trim();
      return {
        ok: false,
        error: log || `scrcpy salió de inmediato (código ${early.code})`,
      };
    }

    proc.on('exit', (code) => {
      delete activeScrcpyProcesses[serial];
      const log = `${stderrBuf}\n${stdoutBuf}`.trim();
      const ms = Date.now() - startedAt;
      const earlyFail = ms < 2500;
      const looksError = /error|failed|exception|unable|not found|denied/i.test(log);
      const error = (earlyFail && (code !== 0 || looksError || log))
        ? (log || `scrcpy terminó (código ${code})`)
        : (code && code !== 0 && looksError ? log : undefined);
      mainWindow?.webContents.send('scrcpy:stopped', { serial, error, code, earlyFail });
    });

    return { ok: true, pid: proc.pid };
  } catch (e) {
    return { ok: false, error: e.message };
  }
});

ipcMain.handle('scrcpy:stop', async (_, { serial }) => {
  const proc = activeScrcpyProcesses[serial];
  if (proc) {
    try { proc.kill(); } catch (_) {}
    delete activeScrcpyProcesses[serial];
  }
  return { ok: true };
});

ipcMain.handle('scrcpy:sessions', () => Object.keys(activeScrcpyProcesses));

ipcMain.handle('adb:screenshot', async (_, { serial }) => {
  try {
    const { filePath } = await dialog.showSaveDialog(mainWindow, {
      title: 'Guardar captura como…',
      defaultPath: `screenshot-${Date.now()}.png`,
      filters: [{ name: 'Imagen', extensions: ['png'] }],
    });
    if (!filePath) return { ok: false, cancelled: true };
    await runAdb(`-s "${serial}" exec-out screencap -p > "${filePath}"`);
    shell.showItemInFolder(filePath);
    return { ok: true, path: filePath };
  } catch (e) {
    return { ok: false, error: e.message };
  }
});
