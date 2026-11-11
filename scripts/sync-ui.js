const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..');
const dist = path.join(root, 'dist-ui');

if (!fs.existsSync(dist)) {
  fs.mkdirSync(dist, { recursive: true });
}

const files = [
  'index.html',
  'style.css',
  'renderer.js',
  'tauri-bridge.js',
  'logo.png',
  'logo.ico'
];

for (const file of files) {
  const src = path.join(root, file);
  const dest = path.join(dist, file);
  if (fs.existsSync(src)) {
    fs.copyFileSync(src, dest);
    console.log(`[sync-ui] Copiado: ${file}`);
  } else {
    console.warn(`[sync-ui] Advertencia: no se encontró ${file}`);
  }
}

console.log('[sync-ui] Sincronización completada con éxito.');
