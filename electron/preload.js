const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('electronAPI', {
  getAppInfo: () => ipcRenderer.invoke('get-app-info'),
  showItemInFolder: (path) => ipcRenderer.invoke('show-item-in-folder', path),
  openExternal: (url) => ipcRenderer.invoke('open-external', url),
  platform: process.platform,
  isElectron: true,

  createScript: (scriptData) => ipcRenderer.invoke('create-script', scriptData),
  
  versions: {
    node: process.versions.node,
    chrome: process.versions.chrome,
    electron: process.versions.electron
  }
});

contextBridge.exposeInMainWorld('nodeAPI', {
  path: {
    join: (...args) => require('path').join(...args),
    dirname: (path) => require('path').dirname(path),
    basename: (path) => require('path').basename(path),
    extname: (path) => require('path').extname(path)
  }
});

contextBridge.exposeInMainWorld('windowAPI', {
  minimize: () => ipcRenderer.invoke('window-minimize'),
  maximize: () => ipcRenderer.invoke('window-maximize'),
  close: () => ipcRenderer.invoke('window-close'),
  isMaximized: () => ipcRenderer.invoke('window-is-maximized')
});

contextBridge.exposeInMainWorld('fileSystemAPI', {
  setProjectPath: (projectPath) => ipcRenderer.invoke('fs-set-project-path', projectPath),
  createProject: (projectName) => ipcRenderer.invoke('fs-create-project', projectName),
  listProjects: () => ipcRenderer.invoke('fs-list-projects'),
  loadProject: (projectName) => ipcRenderer.invoke('fs-load-project', projectName),
  getProjectAssetsTree: () => ipcRenderer.invoke('fs-get-project-assets-tree'),
  getAssetsInFolder: (folderPath) => ipcRenderer.invoke('fs-get-assets-in-folder', folderPath),
  getAssetCategories: () => ipcRenderer.invoke('fs-get-asset-categories'),
  searchAssets: (query) => ipcRenderer.invoke('fs-search-assets', query),
  createFolder: (folderName, parentPath) => ipcRenderer.invoke('fs-create-folder', folderName, parentPath),
  moveAsset: (sourcePath, targetPath) => ipcRenderer.invoke('fs-move-asset', sourcePath, targetPath),
  deleteAsset: (assetPath) => ipcRenderer.invoke('fs-delete-asset', assetPath),
  getAssetContent: (assetPath) => ipcRenderer.invoke('fs-get-asset-content', assetPath),
  onFileChanged: (callback) => { ipcRenderer.on('fs-file-changed', (event, data) => callback(data))},
  removeFileChangeListener: () => { ipcRenderer.removeAllListeners('fs-file-changed')}
});

console.log('Preload script loaded successfully');