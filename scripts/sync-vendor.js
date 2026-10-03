/**
 * Copia el release oficial de scrcpy-win64 a gui/vendor/scrcpy
 * para empaquetarlo con electron-builder (extraResources).
 *
 * Uso:
 *   npm run vendor:sync -- "C:\ruta\a\scrcpy-win64-vX.Y"
 *   set SCRCPY_WIN64=C:\ruta\scrcpy-win64-vX.Y && npm run vendor:sync
 */
const fs = require('fs');
const path = require('path');

const src = process.argv[2] || process.env.SCRCPY_WIN64 || '';
const dest = path.join(__dirname, '..', 'vendor', 'scrcpy');

function copyDir(from, to) {
  fs.mkdirSync(to, { recursive: true });
  for (const name of fs.readdirSync(from)) {
    const a = path.join(from, name);
    const b = path.join(to, name);
    if (fs.statSync(a).isDirectory()) copyDir(a, b);
    else fs.copyFileSync(a, b);
  }
}

if (!src) {
  console.error('Indica la carpeta del release oficial scrcpy-win64.');
  console.error('  npm run vendor:sync -- "C:\\ruta\\a\\scrcpy-win64-vX.Y"');
  console.error('  o define la variable de entorno SCRCPY_WIN64');
  process.exit(1);
}

if (!fs.existsSync(path.join(src, 'scrcpy.exe'))) {
  console.error('No se encontró scrcpy.exe en:', src);
  process.exit(1);
}

copyDir(src, dest);
console.log('OK →', dest);
