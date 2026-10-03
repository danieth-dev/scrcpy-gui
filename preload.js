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
});
