const { contextBridge, ipcRenderer } = require('electron');

// Expose protected methods that allow the renderer process to use
// the ipcRenderer without exposing the entire object
contextBridge.exposeInMainWorld('electronAPI', {
  // App information
  getAppInfo: () => ipcRenderer.invoke('get-app-info'),
  
  // File system operations
  showItemInFolder: (path) => ipcRenderer.invoke('show-item-in-folder', path),
  openExternal: (url) => ipcRenderer.invoke('open-external', url),
  
  // Platform information
  platform: process.platform,
  
  // Check if running in Electron
  isElectron: true,

  // Script creation
  createScript: (scriptData) => ipcRenderer.invoke('create-script', scriptData),
  
  // Version information
  versions: {
    node: process.versions.node,
    chrome: process.versions.chrome,
    electron: process.versions.electron
  }
});

// Expose a limited set of Node.js APIs if needed
contextBridge.exposeInMainWorld('nodeAPI', {
  // Path utilities (safe to expose)
  path: {
    join: (...args) => require('path').join(...args),
    dirname: (path) => require('path').dirname(path),
    basename: (path) => require('path').basename(path),
    extname: (path) => require('path').extname(path)
  }
});

// Window management
contextBridge.exposeInMainWorld('windowAPI', {
  minimize: () => ipcRenderer.invoke('window-minimize'),
  maximize: () => ipcRenderer.invoke('window-maximize'),
  close: () => ipcRenderer.invoke('window-close'),
  isMaximized: () => ipcRenderer.invoke('window-is-maximized')
});

// File system API for direct project asset access
contextBridge.exposeInMainWorld('fileSystemAPI', {
  // Project management
  setProjectPath: (projectPath) => ipcRenderer.invoke('fs-set-project-path', projectPath),
  createProject: (projectName) => ipcRenderer.invoke('fs-create-project', projectName),
  listProjects: () => ipcRenderer.invoke('fs-list-projects'),
  loadProject: (projectName) => ipcRenderer.invoke('fs-load-project', projectName),
  
  // Asset tree and folder operations
  getProjectAssetsTree: () => ipcRenderer.invoke('fs-get-project-assets-tree'),
  getAssetsInFolder: (folderPath) => ipcRenderer.invoke('fs-get-assets-in-folder', folderPath),
  getAssetCategories: () => ipcRenderer.invoke('fs-get-asset-categories'),
  
  // Search
  searchAssets: (query) => ipcRenderer.invoke('fs-search-assets', query),
  
  // File operations
  createFolder: (folderName, parentPath) => ipcRenderer.invoke('fs-create-folder', folderName, parentPath),
  moveAsset: (sourcePath, targetPath) => ipcRenderer.invoke('fs-move-asset', sourcePath, targetPath),
  deleteAsset: (assetPath) => ipcRenderer.invoke('fs-delete-asset', assetPath),
  getAssetContent: (assetPath) => ipcRenderer.invoke('fs-get-asset-content', assetPath),
  
  // File change listener
  onFileChanged: (callback) => {
    ipcRenderer.on('fs-file-changed', (event, data) => callback(data));
  },
  
  // Remove file change listener
  removeFileChangeListener: () => {
    ipcRenderer.removeAllListeners('fs-file-changed');
  }
});

console.log('Preload script loaded successfully');