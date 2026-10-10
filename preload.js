const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('api', {
  minimize: () => ipcRenderer.send('win:minimize'),
  maximize: () => ipcRenderer.send('win:maximize'),
  close:    () => ipcRenderer.send('win:close'),

  getToolsStatus: () => ipcRenderer.invoke('tools:status'),
  getAppInfo:     () => ipcRenderer.invoke('app:info'),
  checkUpdates:   () => ipcRenderer.invoke('app:checkUpdates'),
  openExternal:   (url) => ipcRenderer.invoke('app:openExternal', url),

  getDevices:   () => ipcRenderer.invoke('adb:devices'),
  pairWifi:     (host, port, code) => ipcRenderer.invoke('adb:pair', { host, port, code }),
  connectWifi:  (ip, port) => ipcRenderer.invoke('adb:connect', { ip, port }),
  disconnect:   (serial)   => ipcRenderer.invoke('adb:disconnect', { serial }),
  enableTcpip:  (serial)   => ipcRenderer.invoke('adb:tcpip', { serial }),
  takeScreenshot: (serial) => ipcRenderer.invoke('adb:screenshot', { serial }),

  launchScrcpy: (serial, options) => ipcRenderer.invoke('scrcpy:launch', { serial, options }),
  stopScrcpy:   (serial) => ipcRenderer.invoke('scrcpy:stop', { serial }),
  getSessions:  () => ipcRenderer.invoke('scrcpy:sessions'),

  onScrcpyStopped: (cb) => {
    const handler = (_, data) => cb(data);
    ipcRenderer.on('scrcpy:stopped', handler);
    return () => ipcRenderer.removeListener('scrcpy:stopped', handler);
  },

  // ── iPhone / AirPlay ───────────────────────────────────────────
  iphoneToolsStatus:    ()       => ipcRenderer.invoke('iphone:toolsStatus'),
  iphoneStartUxPlay:    (opts)   => ipcRenderer.invoke('iphone:startUxPlay', opts),
  iphoneStopUxPlay:     ()       => ipcRenderer.invoke('iphone:stopUxPlay'),
  iphoneObsVirtualCam:  (opts)   => ipcRenderer.invoke('iphone:obsVirtualCam', opts),
  onIphoneUxPlayStopped: (cb) => {
    const handler = (_, data) => cb(data);
    ipcRenderer.on('iphone:uxplayStopped', handler);
    return () => ipcRenderer.removeListener('iphone:uxplayStopped', handler);
  },

  // ── iPhone / USB Direct Cam ────────────────────────────────────
  iphoneUsbStatus:       ()       => ipcRenderer.invoke('iphone:usbStatus'),
  iphoneUsbStartStream:  (opts)   => ipcRenderer.invoke('iphone:usbStartStream', opts),
  iphoneUsbStopStream:   ()       => ipcRenderer.invoke('iphone:usbStopStream'),
  onIphoneUsbDeviceChange: (cb) => {
    const handler = (_, data) => cb(data);
    ipcRenderer.on('iphone:usbDeviceChange', handler);
    return () => ipcRenderer.removeListener('iphone:usbDeviceChange', handler);
  },
  onIphoneUsbVideoData: (cb) => {
    const handler = (_, data) => cb(data);
    ipcRenderer.on('iphone:usbVideoData', handler);
    return () => ipcRenderer.removeListener('iphone:usbVideoData', handler);
  },
  onIphoneUsbStats: (cb) => {
    const handler = (_, data) => cb(data);
    ipcRenderer.on('iphone:usbStats', handler);
    return () => ipcRenderer.removeListener('iphone:usbStats', handler);
  },
});
