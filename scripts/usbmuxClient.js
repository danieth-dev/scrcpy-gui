/**
 * usbmuxClient.js — Cliente nativo de usbmuxd (Apple Mobile Device Service) para Node.js.
 * Permite detectar iPhones conectados por cable USB y crear un túnel TCP (port forwarding)
 * de alta velocidad directamente a través del cable Lightning / USB-C.
 */

const net = require('net');
const EventEmitter = require('events');

const USBMUXD_PORT = 27015;
const USBMUXD_HOST = '127.0.0.1';

/**
 * Convierte un número de puerto a orden de bytes de red (big-endian en integer)
 * usbmuxd requiere que PortNumber se pase en formato de red (ej. htons)
 */
function htons(port) {
  return ((port & 0xff) << 8) | ((port >> 8) & 0xff);
}

/**
 * Genera un paquete binario con encabezado de 16 bytes y payload XML Plist
 */
function encodePlistMessage(obj, tag = 1) {
  let xmlBody = '';
  for (const [key, val] of Object.entries(obj)) {
    if (typeof val === 'number') {
      xmlBody += `    <key>${key}</key><integer>${val}</integer>\n`;
    } else {
      xmlBody += `    <key>${key}</key><string>${val}</string>\n`;
    }
  }

  const plist = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
${xmlBody}</dict>
</plist>`;

  const payload = Buffer.from(plist, 'utf8');
  const length = 16 + payload.length;
  const header = Buffer.alloc(16);
  header.writeUInt32LE(length, 0); // Longitud total
  header.writeUInt32LE(1, 4);      // Versión 1
  header.writeUInt32LE(8, 8);      // Tipo 8 (plist)
  header.writeUInt32LE(tag, 12);   // Tag

  return Buffer.concat([header, payload]);
}

/**
 * Parsea respuestas XML simples de usbmuxd sin dependencias externas
 */
function parsePlistXml(xmlStr) {
  const result = {};
  const msgTypeMatch = xmlStr.match(/<key>MessageType<\/key>\s*<string>([^<]+)<\/string>/i);
  if (msgTypeMatch) result.MessageType = msgTypeMatch[1];

  const numMatch = xmlStr.match(/<key>Number<\/key>\s*<integer>([^<]+)<\/integer>/i);
  if (numMatch) result.Number = parseInt(numMatch[1], 10);

  const devIdMatch = xmlStr.match(/<key>DeviceID<\/key>\s*<integer>([^<]+)<\/integer>/i);
  if (devIdMatch) result.DeviceID = parseInt(devIdMatch[1], 10);

  const serialMatch = xmlStr.match(/<key>SerialNumber<\/key>\s*<string>([^<]+)<\/string>/i);
  if (serialMatch) result.SerialNumber = serialMatch[1];

  const connTypeMatch = xmlStr.match(/<key>ConnectionType<\/key>\s*<string>([^<]+)<\/string>/i);
  if (connTypeMatch) result.ConnectionType = connTypeMatch[1];

  return result;
}

class UsbmuxClient extends EventEmitter {
  constructor() {
    super();
    this.devices = new Map(); // DeviceID → { deviceId, serial, connectionType }
    this.listenSocket = null;
    this.isListening = false;
    this.reconnectTimer = null;
  }

  /**
   * Inicia la escucha persistente de eventos de conexión/desconexión de dispositivos USB
   */
  startListening() {
    if (this.isListening) return;
    this.isListening = true;
    this._connectListener();
  }

  stopListening() {
    this.isListening = false;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    if (this.listenSocket) {
      try { this.listenSocket.destroy(); } catch (_) {}
      this.listenSocket = null;
    }
  }

  _connectListener() {
    if (!this.isListening) return;

    const socket = net.createConnection({ host: USBMUXD_HOST, port: USBMUXD_PORT });
    this.listenSocket = socket;

    let buf = Buffer.alloc(0);

    socket.on('connect', () => {
      // Registrar cliente en modo Listen
      const msg = encodePlistMessage({
        MessageType: 'Listen',
        ProgName: 'scrcpy-gui',
        ClientVersionString: '1.2.0',
      });
      socket.write(msg);
    });

    socket.on('data', (chunk) => {
      buf = Buffer.concat([buf, chunk]);
      while (buf.length >= 16) {
        const length = buf.readUInt32LE(0);
        if (buf.length < length) break;

        const packet = buf.subarray(0, length);
        buf = buf.subarray(length);

        const xml = packet.subarray(16).toString('utf8');
        const parsed = parsePlistXml(xml);

        if (parsed.MessageType === 'Attached') {
          const dev = {
            deviceId: parsed.DeviceID,
            serial: parsed.SerialNumber || 'iPhone',
            connectionType: parsed.ConnectionType || 'USB',
          };
          this.devices.set(parsed.DeviceID, dev);
          this.emit('attached', dev);
        } else if (parsed.MessageType === 'Detached') {
          const dev = this.devices.get(parsed.DeviceID);
          this.devices.delete(parsed.DeviceID);
          this.emit('detached', dev || { deviceId: parsed.DeviceID });
        }
      }
    });

    socket.on('error', (err) => {
      this.emit('error', err);
    });

    socket.on('close', () => {
      this.listenSocket = null;
      if (this.isListening) {
        this.reconnectTimer = setTimeout(() => this._connectListener(), 3000);
      }
    });
  }

  /**
   * Obtiene la lista actual de dispositivos conectados por USB
   */
  getConnectedDevices() {
    return Array.from(this.devices.values());
  }

  /**
   * Crea un socket directo conectado al puerto del iPhone a través del cable USB.
   * @param {number} deviceId - ID asignado por usbmuxd
   * @param {number} targetPort - Puerto en el iPhone (ej. 50005)
   * @returns {Promise<net.Socket>}
   */
  connectToDevice(deviceId, targetPort) {
    return new Promise((resolve, reject) => {
      const socket = net.createConnection({ host: USBMUXD_HOST, port: USBMUXD_PORT });
      let handshaked = false;
      let buf = Buffer.alloc(0);

      socket.on('connect', () => {
        const msg = encodePlistMessage({
          MessageType: 'Connect',
          DeviceID: deviceId,
          PortNumber: htons(targetPort),
          ProgName: 'scrcpy-gui',
          ClientVersionString: '1.2.0',
        });
        socket.write(msg);
      });

      socket.on('data', function onData(chunk) {
        if (!handshaked) {
          buf = Buffer.concat([buf, chunk]);
          if (buf.length >= 16) {
            const length = buf.readUInt32LE(0);
            if (buf.length >= length) {
              const packet = buf.subarray(0, length);
              const remaining = buf.subarray(length);
              const xml = packet.subarray(16).toString('utf8');
              const parsed = parsePlistXml(xml);

              if (parsed.MessageType === 'Result' && parsed.Number === 0) {
                handshaked = true;
                socket.removeListener('data', onData);
                // Si quedaron datos en buffer correspondientes al stream de video, emitirlos
                if (remaining.length > 0) {
                  process.nextTick(() => socket.emit('data', remaining));
                }
                resolve(socket);
              } else {
                socket.destroy();
                reject(new Error(`Error de conexión usbmuxd: código ${parsed.Number}`));
              }
            }
          }
        }
      });

      socket.on('error', (err) => {
        if (!handshaked) reject(err);
      });
    });
  }

  /**
   * Abre un servidor proxy local en la PC que redirige todo el tráfico al iPhone por USB.
   * Por ejemplo, escuchando en localhost:50005 y enviando al puerto 50005 del iPhone.
   */
  createPortForwarder(localPort, targetPort, deviceId = null) {
    const server = net.createServer((clientSocket) => {
      let targetDevId = deviceId;
      if (!targetDevId) {
        const devs = this.getConnectedDevices();
        if (devs.length > 0) targetDevId = devs[0].deviceId;
      }

      if (!targetDevId) {
        clientSocket.destroy(new Error('No hay iPhone conectado por cable USB'));
        return;
      }

      this.connectToDevice(targetDevId, targetPort)
        .then((usbSocket) => {
          clientSocket.pipe(usbSocket).pipe(clientSocket);
          clientSocket.on('error', () => { try { usbSocket.destroy(); } catch (_) {} });
          usbSocket.on('error', () => { try { clientSocket.destroy(); } catch (_) {} });
        })
        .catch((err) => {
          clientSocket.destroy(err);
        });
    });

    return new Promise((resolve, reject) => {
      server.listen(localPort, '127.0.0.1', () => {
        resolve(server);
      });
      server.on('error', reject);
    });
  }
}

module.exports = {
  UsbmuxClient,
  USBMUXD_PORT,
  USBMUXD_HOST,
};
