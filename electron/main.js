import { app, BrowserWindow, ipcMain, shell, dialog } from 'electron';
import { spawn } from 'child_process';
import { join, dirname, basename, extname, resolve, relative } from 'path';
import { fileURLToPath } from 'url';
import { existsSync, promises as fs, statSync, readdirSync, watch } from 'fs';
import { createReadStream } from 'fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Keep a global reference of the window object
let mainWindow;
let serverProcess;

const isDev = process.env.NODE_ENV === 'development';
const port = process.env.PORT || 3000;

function createWindow() {
  // Create the browser window
  mainWindow = new BrowserWindow({
    width: 1400,
    height: 900,
    minWidth: 1000,
    minHeight: 600,
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      enableRemoteModule: false,
      preload: join(__dirname, 'preload.js'),
      webSecurity: true,
      allowRunningInsecureContent: false,
      experimentalFeatures: false
    },
    icon: join(__dirname, '../assets/icon.png'), // You can add an icon later
    titleBarStyle: 'hiddenInset', // Hidden but draggable area available
    frame: false, // Remove the window frame/menu bar
    titleBarOverlay: false, // Disable overlay for custom controls
    show: false, // Don't show until ready
  });

  // Show window when ready to prevent visual flash
  mainWindow.once('ready-to-show', () => {
    mainWindow.show();
    
    // Open DevTools in development
    if (isDev) {
      mainWindow.webContents.openDevTools();
    }
  });

  // Load the app - detect HTTPS vs HTTP based on certificates
  const keyPath = join(__dirname, '../localhost+2-key.pem');
  const certPath = join(__dirname, '../localhost+2.pem');
  const hasSSL = existsSync(keyPath) && existsSync(certPath);
  const protocol = hasSSL ? 'https' : 'http';
  
  const startUrl = isDev 
    ? `${protocol}://localhost:${port}` 
    : `${protocol}://127.0.0.1:${port}`;
    
  console.log(`Loading Electron app from: ${startUrl}`);
  mainWindow.loadURL(startUrl);

  // Handle window closed
  mainWindow.on('closed', () => {
    mainWindow = null;
  });

  // Handle external links
  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    // Open external links in default browser
    if (url.startsWith('http://') || url.startsWith('https://')) {
      require('electron').shell.openExternal(url);
      return { action: 'deny' };
    }
    return { action: 'allow' };
  });
}

function startServer() {
  return new Promise((resolve, reject) => {
    // In production, use the original server.js file (not the built one)
    const serverPath = join(__dirname, '../server.js');
    
    if (!existsSync(serverPath)) {
      reject(new Error(`Server file not found: ${serverPath}`));
      return;
    }

    console.log('Starting server process...');
    
    // Start the Fastify server process
    serverProcess = spawn('node', [serverPath], {
      cwd: join(__dirname, '..'),
      env: {
        ...process.env,
        NODE_ENV: isDev ? 'development' : 'production',
        PORT: port.toString(),
        ELECTRON_MODE: 'true'
      },
      stdio: ['pipe', 'pipe', 'pipe']
    });

    // Handle server output
    serverProcess.stdout.on('data', (data) => {
      const output = data.toString();
      console.log('[Server Output]', output.trim());
      
      // Check if server is ready - look for the actual message from server.js
      if (output.includes('Server running') || output.includes('🚀 Server running')) {
        console.log('Server is ready!');
        resolve();
      }
    });

    serverProcess.stderr.on('data', (data) => {
      const error = data.toString();
      console.error('[Server Error]', error.trim());
      
      // If we get an error before resolution, reject
      if (!hasResolved && error.includes('Error')) {
        reject(new Error(`Server startup error: ${error.trim()}`));
      }
    });

    serverProcess.on('error', (error) => {
      console.error('Server process error:', error);
      reject(error);
    });

    let hasResolved = false;
    serverProcess.on('exit', (code, signal) => {
      console.log(`Server process exited with code ${code}, signal: ${signal}`);
      
      // Any exit before successful resolution is an error
      if (!hasResolved) {
        if (code === 0) {
          reject(new Error(`Server exited unexpectedly (code 0) - likely a startup issue`));
        } else if (code !== null) {
          reject(new Error(`Server exited with error code ${code}`));
        } else {
          reject(new Error(`Server was killed with signal ${signal}`));
        }
      }
    });

    // Backup timeout in case we don't see the ready message
    setTimeout(() => {
      if (!hasResolved) {
        // Try to check if server is responding
        checkServerHealth()
          .then(() => {
            hasResolved = true;
            resolve();
          })
          .catch(() => {
            reject(new Error('Server failed to start within timeout'));
          });
      }
    }, 15000); // Increased timeout to 15 seconds
    
    // Prevent multiple resolves
    const originalResolve = resolve;
    resolve = (...args) => {
      if (!hasResolved) {
        hasResolved = true;
        originalResolve(...args);
      }
    };
  });
}

async function checkServerHealth() {
  let retries = 0;
  const maxRetries = 5;
  
  while (retries < maxRetries) {
    try {
      const response = await fetch(`http://localhost:${port}`);
      if (response.ok) {
        return true;
      }
    } catch (error) {
      // Server not ready yet
    }
    
    retries++;
    await new Promise(resolve => setTimeout(resolve, 1000));
  }
  
  throw new Error('Server health check failed');
}

function stopServer() {
  if (serverProcess) {
    console.log('Stopping server process...');
    serverProcess.kill('SIGTERM');
    
    // Force kill after 5 seconds if not stopped gracefully
    setTimeout(() => {
      if (serverProcess && !serverProcess.killed) {
        console.log('Force killing server process...');
        serverProcess.kill('SIGKILL');
      }
    }, 5000);
    
    serverProcess = null;
  }
}

// App event handlers
app.whenReady().then(async () => {
  try {
    // In development, the server is already running via the dev script
    // In production, we need to start our own server
    if (!isDev) {
      await startServer();
    } else {
      // In development, just wait a moment for the dev server to be ready
      console.log('Development mode - using existing dev server');
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
    
    // Then create the window
    createWindow();
    
    app.on('activate', () => {
      // On macOS, re-create window when dock icon is clicked
      if (BrowserWindow.getAllWindows().length === 0) {
        createWindow();
      }
    });
  } catch (error) {
    console.error('Failed to start application:', error);
    app.quit();
  }
});

app.on('window-all-closed', () => {
  // On macOS, keep app running even when all windows are closed
  if (process.platform !== 'darwin') {
    stopServer();
    app.quit();
  }
});

app.on('before-quit', () => {
  stopServer();
});

app.on('will-quit', () => {
  stopServer();
});

// File system watchers and project path tracking
let fileWatchers = new Map();
let currentProjectPath = null;

// Helper functions for file system operations
function isValidPath(filePath) {
  if (!filePath || typeof filePath !== 'string') return false;
  // Basic security check - no path traversal
  const normalizedPath = resolve(filePath);
  return !normalizedPath.includes('..');
}

function getMimeType(filePath) {
  const ext = extname(filePath).toLowerCase();
  const mimeTypes = {
    '.glb': 'model/gltf-binary',
    '.gltf': 'model/gltf+json',
    '.obj': 'application/object',
    '.fbx': 'application/octet-stream',
    '.jpg': 'image/jpeg',
    '.jpeg': 'image/jpeg',
    '.png': 'image/png',
    '.webp': 'image/webp',
    '.bmp': 'image/bmp',
    '.tga': 'image/targa',
    '.mp3': 'audio/mpeg',
    '.wav': 'audio/wav',
    '.ogg': 'audio/ogg',
    '.m4a': 'audio/mp4',
    '.js': 'application/javascript',
    '.ts': 'application/typescript',
    '.json': 'application/json',
    '.xml': 'application/xml',
    '.txt': 'text/plain',
    '.md': 'text/markdown'
  };
  return mimeTypes[ext] || 'application/octet-stream';
}

async function buildFolderTree(folderPath, basePath = folderPath) {
  try {
    const items = await fs.readdir(folderPath);
    const relativePath = relative(basePath, folderPath);
    
    const tree = {
      name: basename(folderPath) || 'assets',
      path: relativePath,
      children: [],
      files: []
    };

    for (const item of items) {
      const itemPath = join(folderPath, item);
      const stat = await fs.stat(itemPath);
      const itemRelativePath = relative(basePath, itemPath);
      
      if (stat.isDirectory()) {
        const subtree = await buildFolderTree(itemPath, basePath);
        tree.children.push(subtree);
      } else {
        tree.files.push({
          id: itemRelativePath,
          name: item,
          path: itemRelativePath,
          size: stat.size,
          type: 'file',
          extension: extname(item),
          mimeType: getMimeType(item),
          fileName: basename(item, extname(item)),
          lastModified: stat.mtime.toISOString()
        });
      }
    }

    // Sort children and files
    tree.children.sort((a, b) => a.name.localeCompare(b.name));
    tree.files.sort((a, b) => a.name.localeCompare(b.name));

    return tree;
  } catch (error) {
    console.error('Error building folder tree:', error);
    throw error;
  }
}

async function getAssetsInFolder(folderPath, basePath) {
  try {
    const items = await fs.readdir(folderPath);
    const assets = [];

    for (const item of items) {
      const itemPath = join(folderPath, item);
      const stat = await fs.stat(itemPath);
      const itemRelativePath = relative(basePath, itemPath);
      
      if (stat.isDirectory()) {
        assets.push({
          id: itemRelativePath,
          name: item,
          path: itemRelativePath,
          type: 'folder',
          lastModified: stat.mtime.toISOString()
        });
      } else {
        assets.push({
          id: itemRelativePath,
          name: item,
          path: itemRelativePath,
          size: stat.size,
          type: 'file',
          extension: extname(item),
          mimeType: getMimeType(item),
          fileName: basename(item, extname(item)),
          lastModified: stat.mtime.toISOString()
        });
      }
    }

    // Sort folders first, then files
    assets.sort((a, b) => {
      if (a.type !== b.type) {
        return a.type === 'folder' ? -1 : 1;
      }
      return a.name.localeCompare(b.name);
    });

    return assets;
  } catch (error) {
    console.error('Error getting assets in folder:', error);
    throw error;
  }
}

// File system IPC handlers
ipcMain.handle('fs-set-project-path', async (event, projectPath) => {
  // If projectPath is just a name (not a full path), resolve it to the projects directory
  let fullProjectPath = projectPath;
  if (!projectPath.includes('/') && !projectPath.includes('\\')) {
    fullProjectPath = join(__dirname, '../projects', projectPath);
  }
  
  if (!isValidPath(fullProjectPath) || !existsSync(fullProjectPath)) {
    throw new Error(`Invalid project path: ${fullProjectPath}`);
  }
  
  console.log(`Setting Electron project path: ${projectPath} -> ${fullProjectPath}`);
  
  // Clear existing watchers
  fileWatchers.forEach(watcher => watcher.close());
  fileWatchers.clear();
  
  currentProjectPath = fullProjectPath;
  
  // Set up file watcher for the project assets folder
  const assetsPath = join(fullProjectPath, 'assets');
  if (existsSync(assetsPath)) {
    try {
      const watcher = watch(assetsPath, { recursive: true }, (eventType, filename) => {
        if (filename) {
          console.log(`File watcher detected: ${eventType} - ${filename}`);
          
          // Debounce rapid file changes
          setTimeout(() => {
            // Notify renderer of file changes
            if (mainWindow) {
              mainWindow.webContents.send('fs-file-changed', {
                type: eventType,
                path: relative(assetsPath, join(assetsPath, filename)),
                fullPath: join(assetsPath, filename),
                timestamp: Date.now()
              });
            }
          }, 100); // 100ms delay to debounce rapid changes
        }
      });
      
      watcher.on('error', (error) => {
        console.error('File watcher error:', error);
      });
      
      fileWatchers.set(assetsPath, watcher);
      console.log(`File watcher set up for: ${assetsPath}`);
    } catch (error) {
      console.warn('Could not set up file watcher:', error);
    }
  }
  
  return { success: true, projectPath, fullPath: fullProjectPath };
});

ipcMain.handle('fs-get-project-assets-tree', async (event) => {
  if (!currentProjectPath) {
    throw new Error('No project path set');
  }
  
  const assetsPath = join(currentProjectPath, 'assets');
  if (!existsSync(assetsPath)) {
    // Create assets folder if it doesn't exist
    await fs.mkdir(assetsPath, { recursive: true });
  }
  
  const tree = await buildFolderTree(assetsPath);
  return { tree };
});

ipcMain.handle('fs-get-assets-in-folder', async (event, folderPath = '') => {
  if (!currentProjectPath) {
    throw new Error('No project path set');
  }
  
  const assetsPath = join(currentProjectPath, 'assets');
  const targetPath = folderPath ? join(assetsPath, folderPath) : assetsPath;
  
  if (!isValidPath(targetPath) || !existsSync(targetPath)) {
    throw new Error('Invalid folder path');
  }
  
  const assets = await getAssetsInFolder(targetPath, assetsPath);
  return { assets };
});

ipcMain.handle('fs-get-asset-categories', async (event) => {
  if (!currentProjectPath) {
    throw new Error('No project path set');
  }
  
  const assetsPath = join(currentProjectPath, 'assets');
  if (!existsSync(assetsPath)) {
    await fs.mkdir(assetsPath, { recursive: true });
  }
  
  // Build categories by scanning all files
  const categories = {
    '3d-models': { name: '3D Models', files: [] },
    'textures': { name: 'Textures', files: [] },
    'audio': { name: 'Audio', files: [] },
    'scripts': { name: 'Scripts', files: [] },
    'data': { name: 'Data Files', files: [] },
    'misc': { name: 'Miscellaneous', files: [] }
  };
  
  async function categorizeFiles(folderPath, basePath) {
    const items = await fs.readdir(folderPath);
    
    for (const item of items) {
      const itemPath = join(folderPath, item);
      const stat = await fs.stat(itemPath);
      
      if (stat.isDirectory()) {
        await categorizeFiles(itemPath, basePath);
      } else {
        const ext = extname(item).toLowerCase();
        const relativePath = relative(basePath, itemPath);
        
        const file = {
          id: relativePath,
          name: item,
          path: relativePath,
          size: stat.size,
          type: 'file',
          extension: ext,
          mimeType: getMimeType(item),
          fileName: basename(item, ext),
          lastModified: stat.mtime.toISOString()
        };
        
        // Categorize by extension
        if (['.glb', '.gltf', '.obj', '.fbx'].includes(ext)) {
          categories['3d-models'].files.push(file);
        } else if (['.jpg', '.jpeg', '.png', '.webp', '.bmp', '.tga'].includes(ext)) {
          categories['textures'].files.push(file);
        } else if (['.mp3', '.wav', '.ogg', '.m4a'].includes(ext)) {
          categories['audio'].files.push(file);
        } else if (['.js', '.ts'].includes(ext)) {
          categories['scripts'].files.push(file);
        } else if (['.json', '.xml'].includes(ext)) {
          categories['data'].files.push(file);
        } else {
          categories['misc'].files.push(file);
        }
      }
    }
  }
  
  await categorizeFiles(assetsPath, assetsPath);
  
  // Sort files in each category
  Object.values(categories).forEach(category => {
    category.files.sort((a, b) => a.name.localeCompare(b.name));
  });
  
  return { categories };
});

ipcMain.handle('fs-search-assets', async (event, query) => {
  if (!currentProjectPath || !query) {
    return { results: [] };
  }
  
  const assetsPath = join(currentProjectPath, 'assets');
  if (!existsSync(assetsPath)) {
    return { results: [] };
  }
  
  const results = [];
  const searchLower = query.toLowerCase();
  
  async function searchFiles(folderPath, basePath) {
    const items = await fs.readdir(folderPath);
    
    for (const item of items) {
      const itemPath = join(folderPath, item);
      const stat = await fs.stat(itemPath);
      
      if (stat.isDirectory()) {
        await searchFiles(itemPath, basePath);
      } else if (item.toLowerCase().includes(searchLower)) {
        const relativePath = relative(basePath, itemPath);
        results.push({
          id: relativePath,
          name: item,
          path: relativePath,
          size: stat.size,
          type: 'file',
          extension: extname(item),
          mimeType: getMimeType(item),
          fileName: basename(item, extname(item)),
          lastModified: stat.mtime.toISOString()
        });
      }
    }
  }
  
  await searchFiles(assetsPath, assetsPath);
  results.sort((a, b) => a.name.localeCompare(b.name));
  
  return { results };
});

ipcMain.handle('fs-create-folder', async (event, folderName, parentPath = '') => {
  if (!currentProjectPath || !folderName) {
    throw new Error('Invalid parameters');
  }
  
  const assetsPath = join(currentProjectPath, 'assets');
  const targetPath = parentPath ? join(assetsPath, parentPath, folderName) : join(assetsPath, folderName);
  
  if (!isValidPath(targetPath)) {
    throw new Error('Invalid folder path');
  }
  
  await fs.mkdir(targetPath, { recursive: true });
  return { success: true, path: relative(assetsPath, targetPath) };
});

ipcMain.handle('fs-move-asset', async (event, sourcePath, targetPath) => {
  if (!currentProjectPath || !sourcePath || !targetPath) {
    throw new Error('Invalid parameters');
  }
  
  const assetsPath = join(currentProjectPath, 'assets');
  const sourceFullPath = join(assetsPath, sourcePath);
  const targetFullPath = join(assetsPath, targetPath);
  
  if (!isValidPath(sourceFullPath) || !isValidPath(targetFullPath) || !existsSync(sourceFullPath)) {
    throw new Error('Invalid source or target path');
  }
  
  await fs.rename(sourceFullPath, targetFullPath);
  return { success: true };
});

ipcMain.handle('fs-delete-asset', async (event, assetPath) => {
  if (!currentProjectPath || !assetPath) {
    throw new Error('Invalid parameters');
  }
  
  const assetsPath = join(currentProjectPath, 'assets');
  const fullPath = join(assetsPath, assetPath);
  
  if (!isValidPath(fullPath) || !existsSync(fullPath)) {
    throw new Error('Invalid asset path');
  }
  
  const stat = await fs.stat(fullPath);
  if (stat.isDirectory()) {
    await fs.rmdir(fullPath, { recursive: true });
  } else {
    await fs.unlink(fullPath);
  }
  
  return { success: true };
});

ipcMain.handle('fs-get-asset-content', async (event, assetPath) => {
  if (!currentProjectPath || !assetPath) {
    throw new Error('Invalid parameters');
  }
  
  const assetsPath = join(currentProjectPath, 'assets');
  const fullPath = join(assetsPath, assetPath);
  
  if (!isValidPath(fullPath) || !existsSync(fullPath)) {
    throw new Error('Asset not found');
  }
  
  const stat = await fs.stat(fullPath);
  if (stat.isDirectory()) {
    throw new Error('Cannot read directory as file');
  }
  
  // For small text files, return content directly
  const ext = extname(fullPath).toLowerCase();
  if (['.txt', '.md', '.json', '.js', '.ts', '.xml'].includes(ext) && stat.size < 1024 * 1024) {
    const content = await fs.readFile(fullPath, 'utf8');
    return { type: 'text', content };
  }
  
  // For binary files, return file URL
  return { 
    type: 'binary', 
    url: `file:///${fullPath.replace(/\\/g, '/')}`,
    size: stat.size,
    mimeType: getMimeType(fullPath)
  };
});

// Script creation handler
ipcMain.handle('create-script', async (event, scriptData) => {
  const { projectName, scriptName, scriptContent, targetPath } = scriptData;
  
  if (!projectName || !scriptName || !scriptContent) {
    throw new Error('Missing required parameters');
  }
  
  // Validate script name
  if (!scriptName.match(/^[a-zA-Z0-9_.-]+\.(js|ts|jsx|tsx)$/)) {
    throw new Error('Invalid script name. Must end with .js, .ts, .jsx, or .tsx');
  }
  
  try {
    // Determine full path
    const projectPath = join(__dirname, '../projects', projectName);
    if (!existsSync(projectPath)) {
      throw new Error('Project not found');
    }
    
    const assetsPath = join(projectPath, 'assets');
    if (!existsSync(assetsPath)) {
      await fs.mkdir(assetsPath, { recursive: true });
    }
    
    const fullTargetPath = targetPath ? join(assetsPath, targetPath) : assetsPath;
    const scriptFilePath = join(fullTargetPath, scriptName);
    
    // Ensure target directory exists
    await fs.mkdir(fullTargetPath, { recursive: true });
    
    // Check if file already exists
    if (existsSync(scriptFilePath)) {
      throw new Error(`Script "${scriptName}" already exists in this location`);
    }
    
    // Write the script file
    await fs.writeFile(scriptFilePath, scriptContent, 'utf8');
    
    console.log(`Created script: ${scriptFilePath}`);
    
    return {
      success: true,
      filePath: relative(join(projectPath, 'assets'), scriptFilePath),
      fullPath: scriptFilePath
    };
  } catch (error) {
    console.error('Error creating script:', error);
    return {
      success: false,
      error: error.message
    };
  }
});

// Project creation handlers
ipcMain.handle('fs-create-project', async (event, projectName) => {
  if (!projectName || typeof projectName !== 'string') {
    throw new Error('Invalid project name');
  }
  
  // Sanitize project name
  const sanitizedName = projectName.replace(/[^a-zA-Z0-9_-]/g, '');
  if (!sanitizedName) {
    throw new Error('Invalid project name');
  }
  
  // Create project in projects directory
  const projectsDir = join(__dirname, '../projects');
  const projectPath = join(projectsDir, sanitizedName);
  
  // Check if project already exists
  if (existsSync(projectPath)) {
    throw new Error('Project already exists');
  }
  
  try {
    // Create project directory structure
    await fs.mkdir(projectPath, { recursive: true });
    await fs.mkdir(join(projectPath, 'assets'), { recursive: true });
    await fs.mkdir(join(projectPath, 'assets', 'textures'), { recursive: true });
    await fs.mkdir(join(projectPath, 'assets', 'models'), { recursive: true });
    await fs.mkdir(join(projectPath, 'assets', 'audio'), { recursive: true });
    await fs.mkdir(join(projectPath, 'assets', 'scripts'), { recursive: true });
    
    // Create project.json file
    const projectData = {
      name: sanitizedName,
      version: '1.0.0',
      engineVersion: '0.1.0',
      created: new Date().toISOString(),
      lastModified: new Date().toISOString(),
      description: `Project: ${sanitizedName}`
    };
    
    await fs.writeFile(
      join(projectPath, 'project.json'),
      JSON.stringify(projectData, null, 2),
      'utf8'
    );
    
    // Create empty scene.json file
    const sceneData = {
      entities: {},
      components: {},
      entityCounter: 0,
      sceneRoot: null
    };
    
    await fs.writeFile(
      join(projectPath, 'scene.json'),
      JSON.stringify(sceneData, null, 2),
      'utf8'
    );
    
    console.log(`Electron: Created project "${sanitizedName}" at ${projectPath}`);
    
    return {
      success: true,
      projectPath: sanitizedName, // Return relative path for consistency
      fullPath: projectPath
    };
  } catch (error) {
    console.error('Error creating project:', error);
    
    // Clean up on failure
    try {
      if (existsSync(projectPath)) {
        await fs.rmdir(projectPath, { recursive: true });
      }
    } catch (cleanupError) {
      console.warn('Failed to cleanup failed project creation:', cleanupError);
    }
    
    throw new Error(`Failed to create project: ${error.message}`);
  }
});

ipcMain.handle('fs-list-projects', async (event) => {
  const projectsDir = join(__dirname, '../projects');
  
  try {
    // Ensure projects directory exists
    if (!existsSync(projectsDir)) {
      await fs.mkdir(projectsDir, { recursive: true });
      return { projects: [] };
    }
    
    const items = await fs.readdir(projectsDir);
    const projects = [];
    
    for (const item of items) {
      const itemPath = join(projectsDir, item);
      const stat = await fs.stat(itemPath);
      
      if (stat.isDirectory()) {
        // Try to read project.json
        const projectJsonPath = join(itemPath, 'project.json');
        let projectInfo = {
          name: item,
          version: '1.0.0',
          created: stat.birthtime.toISOString(),
          lastModified: stat.mtime.toISOString()
        };
        
        if (existsSync(projectJsonPath)) {
          try {
            const projectData = JSON.parse(await fs.readFile(projectJsonPath, 'utf8'));
            projectInfo = { ...projectInfo, ...projectData };
          } catch (error) {
            console.warn(`Failed to read project.json for ${item}:`, error);
          }
        }
        
        projects.push({
          ...projectInfo,
          path: item,
          fullPath: itemPath
        });
      }
    }
    
    // Sort by last modified (newest first)
    projects.sort((a, b) => new Date(b.lastModified) - new Date(a.lastModified));
    
    return { projects };
  } catch (error) {
    console.error('Error listing projects:', error);
    throw new Error(`Failed to list projects: ${error.message}`);
  }
});

ipcMain.handle('fs-load-project', async (event, projectName) => {
  if (!projectName || typeof projectName !== 'string') {
    throw new Error('Invalid project name');
  }
  
  const projectsDir = join(__dirname, '../projects');
  const projectPath = join(projectsDir, projectName);
  
  try {
    // Check if project exists
    if (!existsSync(projectPath)) {
      throw new Error('Project not found');
    }
    
    // Read project.json
    const projectJsonPath = join(projectPath, 'project.json');
    if (!existsSync(projectJsonPath)) {
      throw new Error('Project configuration not found');
    }
    
    const projectData = JSON.parse(await fs.readFile(projectJsonPath, 'utf8'));
    
    // Read scene.json
    const sceneJsonPath = join(projectPath, 'scene.json');
    let sceneData = { entities: {}, components: {}, entityCounter: 0, sceneRoot: null };
    
    if (existsSync(sceneJsonPath)) {
      try {
        sceneData = JSON.parse(await fs.readFile(sceneJsonPath, 'utf8'));
      } catch (error) {
        console.warn(`Failed to read scene.json for ${projectName}:`, error);
      }
    }
    
    console.log(`Electron: Loaded project "${projectName}" from ${projectPath}`);
    
    return {
      success: true,
      projectPath: projectName,
      fullPath: projectPath,
      project: projectData,
      scene: sceneData,
      editor: { selectedEntity: null },
      render: { wireframe: false },
      assets: { selectedAsset: null }
    };
    
  } catch (error) {
    console.error('Error loading project:', error);
    throw new Error(`Failed to load project: ${error.message}`);
  }
});

// IPC handlers
ipcMain.handle('get-app-info', () => {
  return {
    name: app.getName(),
    version: app.getVersion(),
    platform: process.platform,
    arch: process.arch,
    isDev
  };
});

ipcMain.handle('show-item-in-folder', (event, path) => {
  require('electron').shell.showItemInFolder(path);
});

ipcMain.handle('open-external', (event, url) => {
  require('electron').shell.openExternal(url);
});

// Window control handlers
ipcMain.handle('window-minimize', () => {
  if (mainWindow) {
    mainWindow.minimize();
  }
});

ipcMain.handle('window-maximize', () => {
  if (mainWindow) {
    if (mainWindow.isMaximized()) {
      mainWindow.unmaximize();
    } else {
      mainWindow.maximize();
    }
  }
});

ipcMain.handle('window-close', () => {
  if (mainWindow) {
    mainWindow.close();
  }
});

ipcMain.handle('window-is-maximized', () => {
  return mainWindow ? mainWindow.isMaximized() : false;
});

// Security: Prevent navigation to external URLs
app.on('web-contents-created', (event, contents) => {
  contents.on('will-navigate', (navigationEvent, navigationUrl) => {
    const parsedUrl = new URL(navigationUrl);
    
    // Allow both HTTP and HTTPS on localhost/127.0.0.1
    const allowedOrigins = [
      `http://localhost:${port}`,
      `https://localhost:${port}`,
      `http://127.0.0.1:${port}`,
      `https://127.0.0.1:${port}`
    ];
    
    if (!allowedOrigins.includes(parsedUrl.origin)) {
      navigationEvent.preventDefault();
    }
  });
});

// Handle certificate errors for self-signed certificates in development only
if (isDev) {
  app.commandLine.appendSwitch('ignore-certificate-errors');
  app.commandLine.appendSwitch('ignore-ssl-errors');
  app.commandLine.appendSwitch('ignore-certificate-errors-spki-list');
}