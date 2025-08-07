import fs from 'fs/promises'
import fsSync from 'fs'
import path from 'path'
import { fileURLToPath } from 'url'
import chokidar from 'chokidar'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

// Projects directory relative to server root
const PROJECTS_DIR = path.join(__dirname, '../../projects')

// Store active watchers for cleanup
const assetWatchers = new Map()
// Store SSE connections by project
const projectSSEConnections = new Map()
// Store server log connections
const serverLogConnections = new Set()
// Store server logs in memory (last 100 logs)
const serverLogs = []
const MAX_LOGS = 100

// Function to ensure assets directory exists for a project
async function ensureAssetsDirectory(projectPath) {
  const assetsPath = path.join(projectPath, 'assets')
  try {
    await fs.access(assetsPath)
  } catch {
    console.log(`Assets directory missing for project, recreating: ${assetsPath}`)
    await fs.mkdir(assetsPath, { recursive: true })
    await fs.mkdir(path.join(assetsPath, 'textures'), { recursive: true })
    await fs.mkdir(path.join(assetsPath, 'models'), { recursive: true })
    await fs.mkdir(path.join(assetsPath, 'audio'), { recursive: true })
    await fs.mkdir(path.join(assetsPath, 'scripts'), { recursive: true })
  }
}

// Function to broadcast message to all SSE clients watching a project
function broadcastToProject(projectName, message) {
  const connections = projectSSEConnections.get(projectName)
  if (connections && connections.size > 0) {
    const messageStr = `data: ${JSON.stringify(message)}\n\n`
    connections.forEach(reply => {
      try {
        if (!reply.sent) {
          reply.raw.write(messageStr)
        }
      } catch (error) {
        console.warn(`Failed to send SSE message to ${projectName}:`, error)
        // Remove dead connection
        connections.delete(reply)
      }
    })
  }
}

// Function to add server log and broadcast to console clients
function addServerLog(level, source, message, data = null) {
  const logEntry = {
    id: Date.now() + Math.random(),
    timestamp: new Date().toISOString(),
    level, // 'info', 'warn', 'error', 'debug'
    source, // 'Server', 'FileWatcher', 'Projects', 'Assets', etc.
    message,
    data // Optional additional data
  }

  // Add to in-memory logs
  serverLogs.push(logEntry)
  if (serverLogs.length > MAX_LOGS) {
    serverLogs.shift() // Remove oldest log
  }

  // Broadcast to all server log connections
  const messageStr = `data: ${JSON.stringify(logEntry)}\n\n`
  serverLogConnections.forEach(reply => {
    try {
      if (!reply.sent) {
        reply.raw.write(messageStr)
      }
    } catch (error) {
      // Remove dead connection
      serverLogConnections.delete(reply)
    }
  })
}

// Override console methods to capture server logs
const originalConsole = {
  log: console.log,
  warn: console.warn,
  error: console.error,
  info: console.info
}

console.log = (...args) => {
  const message = args.join(' ')
  addServerLog('info', 'Server', message)
  originalConsole.log(...args)
}

console.warn = (...args) => {
  const message = args.join(' ')
  addServerLog('warn', 'Server', message)
  originalConsole.warn(...args)
}

console.error = (...args) => {
  const message = args.join(' ')
  addServerLog('error', 'Server', message)
  originalConsole.error(...args)
}

console.info = (...args) => {
  const message = args.join(' ')
  addServerLog('info', 'Server', message)
  originalConsole.info(...args)
}

// Function to start watching a project directory for changes
function startProjectWatcher(projectPath) {
  const projectName = path.basename(projectPath)
  
  // Ensure we're only watching within the projects directory
  if (!projectPath.includes('/projects/')) {
    console.warn(`Skipping watcher for invalid project path: ${projectPath}`)
    return
  }
  
  // Clean up existing watcher if any
  if (assetWatchers.has(projectName)) {
    assetWatchers.get(projectName).close()
  }

  console.log(`Starting file watcher for project: ${projectPath}`)
  const watcher = chokidar.watch(projectPath, {
    ignored: [
      /(^|[\/\\])\../, // ignore dotfiles
      '**/node_modules/**', // ignore node_modules
      '**/.git/**', // ignore git
      '**/.vscode/**', // ignore vscode
      '**/dist/**', // ignore build outputs
      '**/build/**' // ignore build outputs
    ],
    persistent: true,
    ignoreInitial: true, // don't emit events for initial scan
    depth: 3, // limit recursion depth
    usePolling: false, // use native events for better performance
    atomic: true // wait for write operations to complete
  })

  // Handle file/directory changes
  watcher.on('add', (filePath) => {
    const relativePath = path.relative(projectPath, filePath)
    addServerLog('info', 'FileWatcher', `File added: ${relativePath}`)
    broadcastToProject(projectName, {
      type: 'file_added',
      path: relativePath,
      timestamp: Date.now()
    })
  })

  watcher.on('change', (filePath) => {
    const relativePath = path.relative(projectPath, filePath)
    addServerLog('info', 'FileWatcher', `File changed: ${relativePath}`)
    broadcastToProject(projectName, {
      type: 'file_changed',
      path: relativePath,
      timestamp: Date.now()
    })
  })

  watcher.on('unlink', (filePath) => {
    const relativePath = path.relative(projectPath, filePath)
    console.log(`File deleted: ${relativePath}`)
    broadcastToProject(projectName, {
      type: 'file_deleted',
      path: relativePath,
      timestamp: Date.now()
    })
  })

  watcher.on('addDir', (dirPath) => {
    const relativePath = path.relative(projectPath, dirPath)
    console.log(`Directory added: ${relativePath}`)
    broadcastToProject(projectName, {
      type: 'directory_added',
      path: relativePath,
      timestamp: Date.now()
    })
  })

  watcher.on('unlinkDir', async (deletedPath) => {
    const relativePath = path.relative(projectPath, deletedPath)
    console.log(`Directory deleted: ${relativePath}`)
    
    // Special handling for assets directory
    if (relativePath === 'assets') {
      console.log(`Assets directory deleted, recreating: ${deletedPath}`)
      await ensureAssetsDirectory(projectPath)
      broadcastToProject(projectName, {
        type: 'assets_directory_recreated',
        path: relativePath,
        timestamp: Date.now()
      })
    } else {
      broadcastToProject(projectName, {
        type: 'directory_deleted',
        path: relativePath,
        timestamp: Date.now()
      })
    }
  })

  assetWatchers.set(projectName, watcher)
  return watcher
}

// Function to cleanup all watchers and connections
function cleanupAllWatchers() {
  console.log('Cleaning up all file watchers and SSE connections...')
  
  // Close all SSE connections
  let sseConnectionCount = 0
  projectSSEConnections.forEach((connections, projectName) => {
    sseConnectionCount += connections.size
    connections.forEach(reply => {
      try {
        if (!reply.sent) {
          reply.raw.end()
        }
      } catch (error) {
        // Ignore errors when closing connections
      }
    })
    connections.clear()
  })
  projectSSEConnections.clear()
  
  if (sseConnectionCount > 0) {
    console.log(`Closed ${sseConnectionCount} SSE connections`)
  }
  
  if (assetWatchers.size === 0) {
    console.log('No file watchers to clean up')
    process.exit(0)
    return
  }

  const cleanupPromises = []
  assetWatchers.forEach((watcher, projectName) => {
    console.log(`Closing watcher for project: ${projectName}`)
    
    // Create a promise that resolves when watcher is closed
    const cleanupPromise = new Promise((resolve) => {
      watcher.on('close', resolve)
      watcher.close()
      // Force resolve after timeout to prevent hanging
      setTimeout(resolve, 1000)
    })
    
    cleanupPromises.push(cleanupPromise)
  })
  
  // Wait for all watchers to close or timeout
  Promise.allSettled(cleanupPromises).then(() => {
    assetWatchers.clear()
    console.log('✅ All file watchers cleaned up')
    process.exit(0)
  })
}

// Handle process cleanup
process.on('SIGINT', () => {
  console.log('\n🛑 Received SIGINT, shutting down gracefully...')
  cleanupAllWatchers()
})

process.on('SIGTERM', () => {
  console.log('\n🛑 Received SIGTERM, shutting down gracefully...')
  cleanupAllWatchers()
})

// Ensure projects directory exists
async function ensureProjectsDir() {
  try {
    await fs.access(PROJECTS_DIR)
  } catch {
    await fs.mkdir(PROJECTS_DIR, { recursive: true })
  }
}

export default async function projectRoutes(fastify, options = {}) {
  const isElectron = options.isElectron || false;
  
  console.log(`🔧 Project routes loaded - Electron mode: ${isElectron}`);
  // Register multipart plugin for file uploads
  await fastify.register(import('@fastify/multipart'), {
    limits: {
      fileSize: 1024 * 1024 * 1024 * 5 // 1GB limit
    }
  })

  // Ensure projects directory exists on startup
  await ensureProjectsDir()

  // Start watchers for existing projects (re-enabled with restrictions)
  try {
    const entries = await fs.readdir(PROJECTS_DIR, { withFileTypes: true })
    for (const entry of entries) {
      if (entry.isDirectory()) {
        const projectPath = path.join(PROJECTS_DIR, entry.name)
        await ensureAssetsDirectory(projectPath)
        // Only watch if it's clearly within projects directory and not near node_modules
        if (projectPath.includes('/projects/') && !projectPath.includes('node_modules')) {
          startProjectWatcher(projectPath)
        }
      }
    }
  } catch (error) {
    console.warn('Error initializing project watchers:', error)
  }

  // Server-Sent Events endpoint for real-time project updates
  fastify.get('/api/projects/:projectName/watch', async (request, reply) => {
    try {
      const { projectName } = request.params
      console.log(`SSE connection established for project: ${projectName}`)
      
      // Check if project exists
      const projectPath = path.join(PROJECTS_DIR, projectName)
      try {
        await fs.access(projectPath)
      } catch (error) {
        console.error(`Project not found for SSE: ${projectPath}`)
        return reply.code(404).send({ error: 'Project not found' })
      }
      
      // Set SSE headers
      reply.raw.writeHead(200, {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        'Connection': 'keep-alive',
        'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Headers': 'Cache-Control'
      })
    
    // Add connection to project connections
    if (!projectSSEConnections.has(projectName)) {
      projectSSEConnections.set(projectName, new Set())
    }
    projectSSEConnections.get(projectName).add(reply)
    
    // Send initial connection success message
    const initialMessage = `data: ${JSON.stringify({
      type: 'connected',
      project: projectName,
      timestamp: Date.now()
    })}\n\n`
    reply.raw.write(initialMessage)
    
    // Handle connection close
    request.raw.on('close', () => {
      console.log(`SSE connection closed for project: ${projectName}`)
      const connections = projectSSEConnections.get(projectName)
      if (connections) {
        connections.delete(reply)
        if (connections.size === 0) {
          projectSSEConnections.delete(projectName)
        }
      }
    })
    
    // Keep connection alive with periodic heartbeat
    const heartbeat = setInterval(() => {
      try {
        if (!reply.sent) {
          reply.raw.write(`: heartbeat\n\n`)
        } else {
          clearInterval(heartbeat)
        }
      } catch (error) {
        clearInterval(heartbeat)
      }
    }, 30000)
    
    // Don't end the response - keep it open for streaming
    return reply
    } catch (error) {
      console.error('SSE endpoint error:', error)
      reply.code(500).send({ error: 'SSE connection failed', details: error.message })
    }
  })

  // Server-Sent Events endpoint for server logs
  fastify.get('/api/server/logs', async (request, reply) => {
    console.log('Server logs SSE connection established')
    
    // Set SSE headers
    reply.raw.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      'Connection': 'keep-alive',
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Headers': 'Cache-Control'
    })
    
    // Add connection to server log connections
    serverLogConnections.add(reply)
    
    // Send initial connection success message
    const initialMessage = `data: ${JSON.stringify({
      type: 'connected',
      timestamp: Date.now()
    })}\n\n`
    reply.raw.write(initialMessage)
    
    // Send existing logs
    serverLogs.forEach(log => {
      const messageStr = `data: ${JSON.stringify(log)}\n\n`
      reply.raw.write(messageStr)
    })
    
    // Handle connection close
    request.raw.on('close', () => {
      console.log('Server logs SSE connection closed')
      serverLogConnections.delete(reply)
    })
    
    // Keep connection alive with periodic heartbeat
    const heartbeat = setInterval(() => {
      try {
        if (!reply.sent) {
          reply.raw.write(`: heartbeat\n\n`)
        } else {
          clearInterval(heartbeat)
        }
      } catch (error) {
        clearInterval(heartbeat)
      }
    }, 30000)
    
    // Don't end the response - keep it open for streaming
    return reply
  })

  // Console command execution endpoint
  fastify.post('/api/console/command', async (request, reply) => {
    try {
      const { command, args = [] } = request.body
      addServerLog('info', 'Console', `Executing command: ${command} ${args.join(' ')}`)
      
      let result = { success: false, message: '', data: null }
      
      switch (command.toLowerCase()) {
        case 'restart':
        case 'server:restart':
          result = await handleServerRestart()
          break
          
        case 'status':
        case 'server:status':
          result = await handleServerStatus()
          break
          
        case 'logs:clear':
          result = handleClearLogs()
          break
          
        case 'projects:list':
          result = await handleListProjects()
          break
          
        case 'memory':
        case 'server:memory':
          result = handleMemoryStatus()
          break
          
        case 'help':
          result = handleHelpCommand()
          break
          
        case 'version':
          result = handleVersionCommand()
          break
          
        default:
          result = {
            success: false,
            message: `Unknown command: ${command}. Type 'help' for available commands.`
          }
      }
      
      addServerLog(result.success ? 'info' : 'warn', 'Console', result.message)
      reply.send(result)
      
    } catch (error) {
      const errorMsg = `Command execution failed: ${error.message}`
      addServerLog('error', 'Console', errorMsg)
      reply.code(500).send({ success: false, message: errorMsg })
    }
  })

  // Command handlers
  async function handleServerRestart() {
    addServerLog('warn', 'Server', 'Server restart initiated by console command')
    
    // Schedule restart after a short delay to allow response to be sent
    setTimeout(() => {
      addServerLog('warn', 'Server', 'Restarting server...')
      process.exit(0) // Let process manager (nodemon, pm2, etc.) restart
    }, 1000)
    
    return {
      success: true,
      message: 'Server restart initiated. The server will restart in 1 second.'
    }
  }

  async function handleServerStatus() {
    const uptime = process.uptime()
    const memory = process.memoryUsage()
    const projectCount = await fs.readdir(PROJECTS_DIR).then(files => files.length).catch(() => 0)
    
    return {
      success: true,
      message: `Server Status: Running | Uptime: ${Math.floor(uptime)}s | Projects: ${projectCount} | Memory: ${Math.round(memory.rss / 1024 / 1024)}MB`,
      data: {
        uptime: Math.floor(uptime),
        memory: memory,
        projectCount,
        connections: {
          projects: projectSSEConnections.size,
          serverLogs: serverLogConnections.size
        }
      }
    }
  }

  function handleClearLogs() {
    const clearedCount = serverLogs.length
    serverLogs.length = 0 // Clear the array
    
    return {
      success: true,
      message: `Cleared ${clearedCount} server logs from memory.`
    }
  }

  async function handleListProjects() {
    try {
      const entries = await fs.readdir(PROJECTS_DIR, { withFileTypes: true })
      const projects = entries.filter(entry => entry.isDirectory()).map(entry => entry.name)
      
      return {
        success: true,
        message: `Found ${projects.length} projects: ${projects.join(', ')}`,
        data: projects
      }
    } catch (error) {
      return {
        success: false,
        message: `Failed to list projects: ${error.message}`
      }
    }
  }

  function handleMemoryStatus() {
    const memory = process.memoryUsage()
    const formatBytes = (bytes) => Math.round(bytes / 1024 / 1024) + 'MB'
    
    return {
      success: true,
      message: `Memory Usage - RSS: ${formatBytes(memory.rss)} | Heap Used: ${formatBytes(memory.heapUsed)} | Heap Total: ${formatBytes(memory.heapTotal)} | External: ${formatBytes(memory.external)}`,
      data: memory
    }
  }

  function handleHelpCommand() {
    const commands = [
      'help - Show this help message',
      'restart - Restart the server',
      'status - Show server status and uptime',
      'memory - Show memory usage',
      'projects:list - List all projects',
      'logs:clear - Clear server logs from memory',
      'version - Show server version'
    ]
    
    return {
      success: true,
      message: `Available commands:\n${commands.join('\n')}`,
      data: commands
    }
  }

  function handleVersionCommand() {
    return {
      success: true,
      message: `Engine Server v1.0.0 - Node.js ${process.version}`
    }
  }

  // Create new project
  fastify.post('/api/projects/create', async (request, reply) => {
    try {
      const { projectName } = request.body
      
      if (!projectName || typeof projectName !== 'string') {
        return reply.code(400).send({ error: 'Project name is required' })
      }

      // Sanitize project name
      const safeName = projectName.replace(/[^a-zA-Z0-9_-]/g, '_')
      const projectPath = path.join(PROJECTS_DIR, safeName)

      // Check if project already exists
      try {
        await fs.access(projectPath)
        return reply.code(409).send({ error: 'Project already exists' })
      } catch {
        // Project doesn't exist, continue
      }

      // Create project directory structure
      await fs.mkdir(projectPath, { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets', 'models'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets', 'textures'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets', 'materials'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets', 'scripts'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets', 'images'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets', 'audio'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets', 'media'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'scenes'), { recursive: true })

      // Create default project metadata
      const projectInfo = {
        name: projectName,
        version: '1.0.0',
        engineVersion: '0.1.0',
        created: new Date().toISOString(),
        lastModified: new Date().toISOString()
      }

      await fs.writeFile(
        path.join(projectPath, 'project.json'),
        JSON.stringify(projectInfo, null, 2)
      )

      // Create default scene
      const defaultScene = {
        entities: {
          "1": {
            id: 1,
            name: "Scene_1",
            active: true,
            parent: null,
            children: [],
            components: ["transform"]
          }
        },
        components: {
          transform: {
            "1": { position: [0, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1] }
          }
        },
        sceneObjects: [
          {
            id: 'cube-1',
            name: 'Cube',
            type: 'mesh',
            position: [0, 0, 0],
            rotation: [0, 0, 0],
            scale: [1, 1, 1],
            geometry: 'box',
            material: { color: 'orange' },
            visible: true
          }
        ],
        entityCounter: 1,
        sceneRoot: 1
      }

      await fs.writeFile(
        path.join(projectPath, 'scenes', 'main.json'),
        JSON.stringify(defaultScene, null, 2)
      )

      // Start watching the new project (re-enabled with restrictions)
      if (projectPath.includes('/projects/') && !projectPath.includes('node_modules')) {
        startProjectWatcher(projectPath)
      }

      reply.send({
        success: true,
        projectPath: safeName,
        projectName: projectName
      })

    } catch (error) {
      console.error('Error creating project:', error)
      reply.code(500).send({ error: 'Failed to create project' })
    }
  })

  // Load project
  fastify.get('/api/projects/:projectName', async (request, reply) => {
    try {
      const { projectName } = request.params
      const projectPath = path.join(PROJECTS_DIR, projectName)

      // Check if project directory exists
      try {
        await fs.access(projectPath)
      } catch {
        return reply.code(404).send({ error: 'Project not found' })
      }

      // Ensure assets directory exists and start watching if not already (re-enabled with restrictions)
      await ensureAssetsDirectory(projectPath)
      if (!assetWatchers.has(projectName) && projectPath.includes('/projects/') && !projectPath.includes('node_modules')) {
        startProjectWatcher(projectPath)
      }

      // Read project metadata
      const projectInfoPath = path.join(projectPath, 'project.json')
      let projectInfo
      try {
        projectInfo = JSON.parse(await fs.readFile(projectInfoPath, 'utf8'))
      } catch {
        return reply.code(404).send({ error: 'Project metadata not found' })
      }

      // Read main scene
      const mainScenePath = path.join(projectPath, 'scenes', 'main.json')
      let sceneData = {}
      try {
        sceneData = JSON.parse(await fs.readFile(mainScenePath, 'utf8'))
      } catch {
        // Scene file doesn't exist, use empty scene
      }

      // Read editor settings
      const editorSettingsPath = path.join(projectPath, 'editor-settings.json')
      let editorData = {}
      try {
        editorData = JSON.parse(await fs.readFile(editorSettingsPath, 'utf8'))
      } catch {
        // Editor settings don't exist, use empty object
      }

      // Read render settings
      const renderSettingsPath = path.join(projectPath, 'render-settings.json')
      let renderData = {}
      try {
        renderData = JSON.parse(await fs.readFile(renderSettingsPath, 'utf8'))
      } catch {
        // Render settings don't exist, use empty object
      }

      // Read other plugin settings
      const pluginData = {}
      try {
        const files = await fs.readdir(projectPath)
        for (const file of files) {
          if (file.endsWith('-settings.json') && file !== 'editor-settings.json' && file !== 'render-settings.json') {
            const pluginName = file.replace('-settings.json', '')
            try {
              const pluginFilePath = path.join(projectPath, file)
              pluginData[pluginName] = JSON.parse(await fs.readFile(pluginFilePath, 'utf8'))
            } catch (error) {
              console.warn(`Failed to read plugin settings for ${pluginName}:`, error)
            }
          }
        }
      } catch (error) {
        console.warn('Failed to read plugin settings:', error)
      }

      // TODO: Read assets manifest
      const assets = {}

      reply.send({
        project: projectInfo,
        scene: sceneData,
        editor: editorData,
        render: renderData,
        assets: assets,
        projectPath: projectName,
        ...pluginData // Include any other plugin data
      })

    } catch (error) {
      console.error('Error loading project:', error)
      reply.code(404).send({ error: 'Project not found' })
    }
  })

  // Save project data
  fastify.post('/api/projects/:projectName/save', async (request, reply) => {
    try {
      const { projectName } = request.params
      // Handle both old format {scene, editor, assets} and new format {editor, render, scene}
      const { scene, editor, assets, render, ...otherPlugins } = request.body
      const projectPath = path.join(PROJECTS_DIR, projectName)

      // Update project metadata
      const projectInfoPath = path.join(projectPath, 'project.json')
      let projectInfo = {}
      try {
        projectInfo = JSON.parse(await fs.readFile(projectInfoPath, 'utf8'))
      } catch {
        // Create default if doesn't exist
        projectInfo = {
          name: projectName,
          version: '1.0.0',
          engineVersion: '0.1.0',
          created: new Date().toISOString()
        }
      }
      
      projectInfo.lastModified = new Date().toISOString()
      await fs.writeFile(projectInfoPath, JSON.stringify(projectInfo, null, 2))

      // Save scene data
      if (scene) {
        const mainScenePath = path.join(projectPath, 'scenes', 'main.json')
        await fs.writeFile(mainScenePath, JSON.stringify(scene, null, 2))
      }

      // Save editor settings (optional)
      if (editor) {
        const editorSettingsPath = path.join(projectPath, 'editor-settings.json')
        await fs.writeFile(editorSettingsPath, JSON.stringify(editor, null, 2))
      }

      // Save render settings (optional)
      if (render) {
        const renderSettingsPath = path.join(projectPath, 'render-settings.json')
        await fs.writeFile(renderSettingsPath, JSON.stringify(render, null, 2))
      }

      // Save other plugin data
      for (const [pluginName, pluginData] of Object.entries(otherPlugins)) {
        if (pluginData && typeof pluginData === 'object') {
          const pluginSettingsPath = path.join(projectPath, `${pluginName}-settings.json`)
          await fs.writeFile(pluginSettingsPath, JSON.stringify(pluginData, null, 2))
        }
      }

      // TODO: Handle assets
      if (assets) {
        // Save assets to files
      }

      reply.send({ success: true })

    } catch (error) {
      console.error('Error saving project:', error)
      reply.code(500).send({ error: 'Failed to save project' })
    }
  })

  // Export project as .ren file
  fastify.get('/api/projects/:projectName/export', async (request, reply) => {
    try {
      const { projectName } = request.params
      const projectPath = path.join(PROJECTS_DIR, projectName)

      // Read all project data
      const projectInfo = JSON.parse(
        await fs.readFile(path.join(projectPath, 'project.json'), 'utf8')
      )

      let sceneData = {}
      try {
        sceneData = JSON.parse(
          await fs.readFile(path.join(projectPath, 'scenes', 'main.json'), 'utf8')
        )
      } catch {
        // Empty scene if file doesn't exist
      }

      // TODO: Collect all assets and encode as base64
      const assets = await collectProjectAssets(projectPath)

      // Create .ren file content
      const renData = {
        project: {
          ...projectInfo,
          lastModified: new Date().toISOString()
        },
        scene: sceneData,
        assets: assets
      }

      // Set headers for download
      reply.type('application/json')
      reply.header('Content-Disposition', `attachment; filename="${projectName}.ren"`)
      reply.send(JSON.stringify(renData, null, 2))

    } catch (error) {
      console.error('Error exporting project:', error)
      reply.code(500).send({ error: 'Failed to export project' })
    }
  })

  // Import/extract .ren file
  fastify.post('/api/projects/import', async (request, reply) => {
    try {
      const { projectName, renData } = request.body

      if (!projectName || !renData) {
        return reply.code(400).send({ error: 'Project name and .ren data required' })
      }

      const safeName = projectName.replace(/[^a-zA-Z0-9_-]/g, '_')
      const projectPath = path.join(PROJECTS_DIR, safeName)

      // Check if project already exists
      try {
        await fs.access(projectPath)
        return reply.code(409).send({ error: 'Project already exists' })
      } catch {
        // Project doesn't exist, continue
      }

      // Create project directory structure
      await fs.mkdir(projectPath, { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets', 'models'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets', 'textures'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets', 'materials'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets', 'scripts'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets', 'images'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets', 'audio'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'assets', 'media'), { recursive: true })
      await fs.mkdir(path.join(projectPath, 'scenes'), { recursive: true })

      // Extract project data
      if (renData.project) {
        await fs.writeFile(
          path.join(projectPath, 'project.json'),
          JSON.stringify(renData.project, null, 2)
        )
      }

      if (renData.scene) {
        await fs.writeFile(
          path.join(projectPath, 'scenes', 'main.json'),
          JSON.stringify(renData.scene, null, 2)
        )
      }

      // Extract assets
      if (renData.assets) {
        await extractProjectAssets(projectPath, renData.assets)
      }

      // Start watching the imported project (temporarily disabled)  
      // startProjectWatcher(projectPath)

      reply.send({
        success: true,
        projectPath: safeName,
        projectName: projectName
      })

    } catch (error) {
      console.error('Error importing project:', error)
      reply.code(500).send({ error: 'Failed to import project' })
    }
  })

  // List all projects
  fastify.get('/api/projects', async (_, reply) => {
    try {
      const projects = []
      const entries = await fs.readdir(PROJECTS_DIR, { withFileTypes: true })

      for (const entry of entries) {
        if (entry.isDirectory()) {
          try {
            const projectInfoPath = path.join(PROJECTS_DIR, entry.name, 'project.json')
            const projectInfo = JSON.parse(await fs.readFile(projectInfoPath, 'utf8'))
            projects.push({
              name: entry.name,
              displayName: projectInfo.name,
              lastModified: projectInfo.lastModified,
              created: projectInfo.created
            })
          } catch {
            // Skip invalid projects
          }
        }
      }

      reply.send({ projects })

    } catch (error) {
      console.error('Error listing projects:', error)
      reply.code(500).send({ error: 'Failed to list projects' })
    }
  })

  // Get assets list for a project (backward compatibility)
  fastify.get('/api/projects/:projectName/assets', async (request, reply) => {
    if (isElectron) {
      // In Electron mode, file system is handled by main process
      const { folder = '' } = request.query
      return reply.send({ assets: [], currentFolder: folder })
    }
    
    try {
      const { projectName } = request.params
      const { folder = '' } = request.query
      const projectPath = path.join(PROJECTS_DIR, projectName)
      
      // Check if project exists
      try {
        await fs.access(projectPath)
      } catch {
        return reply.code(404).send({ error: 'Project not found' })
      }

      const assetsList = await listProjectAssets(projectPath, folder)
      reply.send({ assets: assetsList, currentFolder: folder })
    } catch (error) {
      console.error('Error listing project assets:', error)
      reply.code(500).send({ error: 'Failed to list project assets' })
    }
  })

  // Get folder tree structure for a project
  fastify.get('/api/projects/:projectName/assets/tree', async (request, reply) => {
    if (isElectron) {
      // In Electron mode, file system is handled by main process
      return reply.send({ tree: { name: 'assets', path: '', children: [], files: [] } })
    }
    
    try {
      const { projectName } = request.params
      const projectPath = path.join(PROJECTS_DIR, projectName)
      
      // Check if project exists
      try {
        await fs.access(projectPath)
      } catch {
        return reply.code(404).send({ error: 'Project not found' })
      }

      const folderTree = await buildFolderTree(projectPath)
      reply.send({ tree: folderTree })
    } catch (error) {
      console.error('Error building folder tree:', error)
      reply.code(500).send({ error: 'Failed to build folder tree' })
    }
  })

  // Get assets categorized by type
  fastify.get('/api/projects/:projectName/assets/categories', async (request, reply) => {
    if (isElectron) {
      // In Electron mode, file system is handled by main process
      const emptyCategories = {
        '3d-models': { name: '3D Models', files: [] },
        'textures': { name: 'Textures', files: [] },
        'audio': { name: 'Audio', files: [] },
        'scripts': { name: 'Scripts', files: [] },
        'data': { name: 'Data Files', files: [] },
        'misc': { name: 'Miscellaneous', files: [] }
      }
      return reply.send({ categories: emptyCategories })
    }
    
    try {
      const { projectName } = request.params
      const projectPath = path.join(PROJECTS_DIR, projectName)
      
      console.log(`📊 Getting asset categories for project: ${projectName}`)
      
      // Check if project exists
      try {
        await fs.access(projectPath)
      } catch {
        console.error(`Project not found: ${projectPath}`)
        return reply.code(404).send({ error: 'Project not found' })
      }

      const categories = await categorizeAssetsByType(projectPath)
      console.log(`📊 Categories built:`, Object.keys(categories).map(key => `${key}: ${categories[key].files.length} files`))
      reply.send({ categories })
    } catch (error) {
      console.error('Error categorizing assets:', error)
      reply.code(500).send({ error: 'Failed to categorize assets', details: error.message })
    }
  })

  // Helper function to check allowed file types
  const isAllowedFileType = (filename) => {
    const allowedTypes = /\.(glb|gltf|obj|fbx|jpg|jpeg|png|bmp|tga|webp|mp3|wav|ogg|m4a|js|ts|json|xml|txt|md)$/i;
    return allowedTypes.test(filename);
  };

  // Upload asset files
  fastify.post('/api/projects/:projectName/assets/upload', async (request, reply) => {
    if (isElectron) {
      // In Electron mode, file uploads are handled by main process
      return reply.send({ message: 'File upload handled by Electron main process', filename: 'electron-handled' })
    }
    
    try {
      const { projectName } = request.params;
      const projectPath = path.join(PROJECTS_DIR, projectName);
      
      // Check if project exists
      try {
        await fs.access(projectPath);
      } catch {
        return reply.code(404).send({ error: 'Project not found' });
      }

      // Ensure assets directory exists
      await ensureAssetsDirectory(projectPath);
      
      // Handle multipart form data
      const data = await request.file();
      if (!data) {
        return reply.code(400).send({ error: 'No file provided' });
      }

      const filename = data.filename;
      
      // Check if file type is allowed
      if (!isAllowedFileType(filename)) {
        return reply.code(400).send({ error: 'File type not allowed' });
      }
      
      const buffer = await data.toBuffer();
      
      // Get folder path from headers - if specified, use exactly that path
      const folderPath = request.headers['x-folder-path'];
      const ext = path.extname(filename).toLowerCase();
      
      // Use the exact folder path specified, or root if none specified
      const baseTargetPath = folderPath || ''; // Empty string means root assets directory
      const targetDirPath = path.join(projectPath, 'assets', baseTargetPath);
      await fs.mkdir(targetDirPath, { recursive: true });
      
      // Generate unique filename if file already exists
      let finalFilename = filename;
      let counter = 1;
      while (await fs.access(path.join(targetDirPath, finalFilename)).then(() => true).catch(() => false)) {
        const name = path.basename(filename, ext);
        finalFilename = `${name}_${counter}${ext}`;
        counter++;
      }
      
      const finalPath = path.join(targetDirPath, finalFilename);
      await fs.writeFile(finalPath, buffer);
      
      addServerLog('info', 'Assets', `Asset uploaded: ${finalFilename} to ${baseTargetPath}`);
      
      reply.send({
        success: true,
        filename: finalFilename,
        path: path.join(baseTargetPath, finalFilename).replace(/\\/g, '/')
      });
      
    } catch (error) {
      console.error('Error uploading asset:', error);
      reply.code(500).send({ error: 'Failed to upload asset', details: error.message });
    }
  });

  // Create script file endpoint
  fastify.post('/api/projects/:projectName/assets/create-script', async (request, reply) => {
    if (isElectron) {
      // In Electron mode, script creation is handled by main process
      return reply.send({ success: true, message: 'Script creation handled by Electron main process' })
    }
    
    try {
      const { projectName } = request.params;
      const { scriptName, scriptContent, targetPath = '' } = request.body;
      
      if (!scriptName || !scriptContent) {
        return reply.code(400).send({ error: 'Script name and content are required' });
      }
      
      // Validate script name
      if (!scriptName.match(/^[a-zA-Z0-9_.-]+\.(js|ts|jsx|tsx)$/)) {
        return reply.code(400).send({ error: 'Invalid script name. Must end with .js, .ts, .jsx, or .tsx' });
      }
      
      const projectPath = path.join(PROJECTS_DIR, projectName);
      
      // Check if project exists
      try {
        await fs.access(projectPath);
      } catch {
        return reply.code(404).send({ error: 'Project not found' });
      }

      // Ensure assets directory exists
      await ensureAssetsDirectory(projectPath);
      
      const assetsPath = path.join(projectPath, 'assets');
      const fullTargetPath = targetPath ? path.join(assetsPath, targetPath) : assetsPath;
      const scriptFilePath = path.join(fullTargetPath, scriptName);
      
      // Security check - ensure the path is within the assets directory
      const normalizedAssetsPath = path.resolve(assetsPath);
      const normalizedScriptPath = path.resolve(scriptFilePath);
      
      if (!normalizedScriptPath.startsWith(normalizedAssetsPath)) {
        return reply.code(403).send({ error: 'Access denied' });
      }
      
      // Ensure target directory exists
      await fs.mkdir(fullTargetPath, { recursive: true });
      
      // Check if script already exists
      try {
        await fs.access(scriptFilePath);
        return reply.code(409).send({ error: `Script "${scriptName}" already exists in this location` });
      } catch {
        // Script doesn't exist, continue
      }
      
      // Write the script file
      await fs.writeFile(scriptFilePath, scriptContent, 'utf8');
      
      const relativePath = path.relative(assetsPath, scriptFilePath).replace(/\\/g, '/');
      
      addServerLog('info', 'Assets', `Script created: ${scriptName} at ${relativePath}`);
      
      reply.send({
        success: true,
        scriptName: scriptName,
        filePath: relativePath,
        fullPath: scriptFilePath
      });
      
    } catch (error) {
      console.error('Error creating script:', error);
      reply.code(500).send({ error: 'Failed to create script', details: error.message });
    }
  });

  // Create folder in assets directory
  fastify.post('/api/projects/:projectName/assets/folder', async (request, reply) => {
    if (isElectron) {
      // In Electron mode, folder creation is handled by main process
      return reply.send({ success: true, message: 'Folder creation handled by Electron main process' })
    }
    
    try {
      const { projectName } = request.params;
      const { folderName, parentPath = '' } = request.body;
      
      if (!folderName || typeof folderName !== 'string') {
        return reply.code(400).send({ error: 'Folder name is required' });
      }
      
      // Sanitize folder name
      const safeFolderName = folderName.replace(/[^a-zA-Z0-9_-\s]/g, '_').trim();
      if (!safeFolderName) {
        return reply.code(400).send({ error: 'Invalid folder name' });
      }
      
      const projectPath = path.join(PROJECTS_DIR, projectName);
      
      // Check if project exists
      try {
        await fs.access(projectPath);
      } catch {
        return reply.code(404).send({ error: 'Project not found' });
      }

      // Ensure assets directory exists
      await ensureAssetsDirectory(projectPath);
      
      // Construct the full folder path
      const assetsPath = path.join(projectPath, 'assets');
      const targetFolderPath = path.join(assetsPath, parentPath, safeFolderName);
      
      // Security check - ensure the path is within the assets directory
      const normalizedAssetsPath = path.resolve(assetsPath);
      const normalizedTargetPath = path.resolve(targetFolderPath);
      
      if (!normalizedTargetPath.startsWith(normalizedAssetsPath)) {
        return reply.code(403).send({ error: 'Access denied' });
      }
      
      // Check if folder already exists
      try {
        await fs.access(targetFolderPath);
        return reply.code(409).send({ error: 'Folder already exists' });
      } catch {
        // Folder doesn't exist, continue
      }
      
      // Create the folder
      await fs.mkdir(targetFolderPath, { recursive: true });
      
      const relativePath = path.relative(assetsPath, targetFolderPath).replace(/\\/g, '/');
      
      console.log(`📁 Folder created: ${relativePath}`);
      
      reply.send({
        success: true,
        folderName: safeFolderName,
        path: relativePath
      });
      
    } catch (error) {
      console.error('Error creating folder:', error);
      reply.code(500).send({ error: 'Failed to create folder', details: error.message });
    }
  });

  // Delete folder or file from assets directory
  fastify.delete('/api/projects/:projectName/assets/delete', async (request, reply) => {
    if (isElectron) {
      // In Electron mode, asset deletion is handled by main process
      return reply.send({ success: true, message: 'Asset deletion handled by Electron main process' })
    }
    
    try {
      const { projectName } = request.params;
      const { itemPath } = request.body;
      
      if (!itemPath || typeof itemPath !== 'string') {
        return reply.code(400).send({ error: 'Item path is required' });
      }
      
      const projectPath = path.join(PROJECTS_DIR, projectName);
      
      // Check if project exists
      try {
        await fs.access(projectPath);
      } catch {
        return reply.code(404).send({ error: 'Project not found' });
      }
      
      const assetsPath = path.join(projectPath, 'assets');
      const targetItemPath = path.join(assetsPath, itemPath);
      
      // Security check - ensure the path is within the assets directory
      const normalizedAssetsPath = path.resolve(assetsPath);
      const normalizedTargetPath = path.resolve(targetItemPath);
      
      if (!normalizedTargetPath.startsWith(normalizedAssetsPath)) {
        return reply.code(403).send({ error: 'Access denied' });
      }
      
      // Check if item exists
      try {
        await fs.access(targetItemPath);
      } catch {
        return reply.code(404).send({ error: 'Item not found' });
      }
      
      // Get item stats to determine if it's a file or directory
      const stats = await fs.stat(targetItemPath);
      
      if (stats.isDirectory()) {
        // Remove directory and all contents
        await fs.rm(targetItemPath, { recursive: true, force: true });
        console.log(`🗑️ Folder deleted: ${itemPath}`);
      } else {
        // Remove file
        await fs.unlink(targetItemPath);
        console.log(`🗑️ File deleted: ${itemPath}`);
      }
      
      reply.send({
        success: true,
        deleted: itemPath,
        type: stats.isDirectory() ? 'folder' : 'file'
      });
      
    } catch (error) {
      console.error('Error deleting item:', error);
      reply.code(500).send({ error: 'Failed to delete item', details: error.message });
    }
  });

  // Rename folder or file in assets directory
  fastify.put('/api/projects/:projectName/assets/rename', async (request, reply) => {
    try {
      const { projectName } = request.params;
      const { oldPath, newName } = request.body;
      
      if (!oldPath || !newName || typeof oldPath !== 'string' || typeof newName !== 'string') {
        return reply.code(400).send({ error: 'Old path and new name are required' });
      }
      
      // Sanitize new name
      const safeNewName = newName.replace(/[^a-zA-Z0-9_.-\s]/g, '_').trim();
      if (!safeNewName) {
        return reply.code(400).send({ error: 'Invalid new name' });
      }
      
      const projectPath = path.join(PROJECTS_DIR, projectName);
      
      // Check if project exists
      try {
        await fs.access(projectPath);
      } catch {
        return reply.code(404).send({ error: 'Project not found' });
      }
      
      const assetsPath = path.join(projectPath, 'assets');
      const oldItemPath = path.join(assetsPath, oldPath);
      const newItemPath = path.join(path.dirname(oldItemPath), safeNewName);
      
      // Security checks
      const normalizedAssetsPath = path.resolve(assetsPath);
      const normalizedOldPath = path.resolve(oldItemPath);
      const normalizedNewPath = path.resolve(newItemPath);
      
      if (!normalizedOldPath.startsWith(normalizedAssetsPath) || 
          !normalizedNewPath.startsWith(normalizedAssetsPath)) {
        return reply.code(403).send({ error: 'Access denied' });
      }
      
      // Check if old item exists
      try {
        await fs.access(oldItemPath);
      } catch {
        return reply.code(404).send({ error: 'Item not found' });
      }
      
      // Check if new name already exists
      try {
        await fs.access(newItemPath);
        return reply.code(409).send({ error: 'An item with that name already exists' });
      } catch {
        // New name doesn't exist, continue
      }
      
      // Rename the item
      await fs.rename(oldItemPath, newItemPath);
      
      const newRelativePath = path.relative(assetsPath, newItemPath).replace(/\\/g, '/');
      
      console.log(`✏️ Item renamed: ${oldPath} → ${newRelativePath}`);
      
      reply.send({
        success: true,
        oldPath: oldPath,
        newPath: newRelativePath,
        newName: safeNewName
      });
      
    } catch (error) {
      console.error('Error renaming item:', error);
      reply.code(500).send({ error: 'Failed to rename item', details: error.message });
    }
  });

  // Move folder or file to a new location
  fastify.put('/api/projects/:projectName/assets/move', async (request, reply) => {
    if (isElectron) {
      // In Electron mode, asset moving is handled by main process
      return reply.send({ success: true, message: 'Asset move handled by Electron main process' })
    }
    
    try {
      const { projectName } = request.params;
      const { sourcePath, targetPath } = request.body;
      
      if (!sourcePath || !targetPath || typeof sourcePath !== 'string' || typeof targetPath !== 'string') {
        return reply.code(400).send({ error: 'Source path and target path are required' });
      }
      
      const projectPath = path.join(PROJECTS_DIR, projectName);
      
      // Check if project exists
      try {
        await fs.access(projectPath);
      } catch {
        return reply.code(404).send({ error: 'Project not found' });
      }
      
      const assetsPath = path.join(projectPath, 'assets');
      const sourceItemPath = path.join(assetsPath, sourcePath);
      const targetItemPath = path.join(assetsPath, targetPath);
      
      // Security checks
      const normalizedAssetsPath = path.resolve(assetsPath);
      const normalizedSourcePath = path.resolve(sourceItemPath);
      const normalizedTargetPath = path.resolve(targetItemPath);
      
      if (!normalizedSourcePath.startsWith(normalizedAssetsPath) || 
          !normalizedTargetPath.startsWith(normalizedAssetsPath)) {
        return reply.code(403).send({ error: 'Access denied' });
      }
      
      // Check if source exists
      try {
        await fs.access(sourceItemPath);
      } catch {
        return reply.code(404).send({ error: 'Source item not found' });
      }
      
      // Check if target directory exists
      const targetDir = path.dirname(targetItemPath);
      try {
        await fs.access(targetDir);
      } catch {
        // Create target directory if it doesn't exist
        await fs.mkdir(targetDir, { recursive: true });
      }
      
      // Check if target already exists
      try {
        await fs.access(targetItemPath);
        return reply.code(409).send({ error: 'Target already exists' });
      } catch {
        // Target doesn't exist, continue
      }
      
      // Move the item
      await fs.rename(sourceItemPath, targetItemPath);
      
      console.log(`📁 Item moved: ${sourcePath} → ${targetPath}`);
      
      reply.send({
        success: true,
        sourcePath: sourcePath,
        targetPath: targetPath
      });
      
    } catch (error) {
      console.error('Error moving item:', error);
      reply.code(500).send({ error: 'Failed to move item', details: error.message });
    }
  });

  // Serve individual asset files
  fastify.get('/api/projects/:projectName/assets/file/*', async (request, reply) => {
    try {
      const { projectName } = request.params
      const assetPath = request.params['*'] // Get the wildcard path
      const projectPath = path.join(PROJECTS_DIR, projectName)
      const fullAssetPath = path.join(projectPath, 'assets', assetPath)
      
      // Security check - ensure the path is within the project assets directory
      const normalizedProjectPath = path.resolve(projectPath)
      const normalizedAssetPath = path.resolve(fullAssetPath)
      
      if (!normalizedAssetPath.startsWith(normalizedProjectPath)) {
        return reply.code(403).send({ error: 'Access denied' })
      }
      
      // Check if file exists
      try {
        await fs.access(fullAssetPath)
      } catch {
        return reply.code(404).send({ error: 'Asset not found' })
      }
      
      // Get file stats and determine content type
      const stats = await fs.stat(fullAssetPath)
      const ext = path.extname(assetPath).toLowerCase()
      const mimeType = getMimeType(ext)
      
      // Set appropriate headers
      reply.header('Content-Type', mimeType)
      reply.header('Content-Length', stats.size)
      reply.header('Cache-Control', 'public, max-age=3600') // Cache for 1 hour
      reply.header('Access-Control-Allow-Origin', '*')
      reply.header('Access-Control-Allow-Methods', 'GET')
      reply.header('Access-Control-Allow-Headers', 'Content-Type')
      
      // If this is a download request (e.g., drag to desktop), add download headers
      if (request.query.download === 'true') {
        const filename = path.basename(assetPath)
        reply.header('Content-Disposition', `attachment; filename="${filename}"`)
      }
      
      // Stream the file
      const fileStream = fsSync.createReadStream(fullAssetPath)
      
      fileStream.on('error', (error) => {
        console.error('Stream error:', error)
        if (!reply.sent) {
          reply.code(500).send({ error: 'File stream error' })
        }
      })
      
      return reply.send(fileStream)
      
    } catch (error) {
      console.error('Error serving asset file:', error)
      console.error('Asset path:', assetPath)
      console.error('Full asset path:', fullAssetPath)
      console.error('Error details:', error.message, error.stack)
      reply.code(500).send({ error: 'Failed to serve asset file', details: error.message })
    }
  })
}

// Helper function to build hierarchical folder structure
async function buildFolderTree(projectPath) {
  const assetsPath = path.join(projectPath, 'assets')
  
  const buildTree = async (dirPath, relativePath = '') => {
    const tree = {
      name: relativePath ? path.basename(relativePath) : 'assets',
      path: relativePath,
      type: 'folder',
      children: [],
      files: []
    }
    
    try {
      const entries = await fs.readdir(dirPath, { withFileTypes: true })
      
      for (const entry of entries) {
        // Skip hidden files/directories
        if (entry.name.startsWith('.') || entry.name === 'Thumbs.db') continue
        
        const fullPath = path.join(dirPath, entry.name)
        const relativeFilePath = path.join(relativePath, entry.name).replace(/\\/g, '/')
        
        if (entry.isDirectory()) {
          // Recursively build subtree for directories
          const subtree = await buildTree(fullPath, relativeFilePath)
          tree.children.push(subtree)
        } else {
          // Add file to current directory
          let stats
          try {
            stats = await fs.stat(fullPath)
          } catch (error) {
            console.warn(`Skipping broken asset: ${relativeFilePath} - ${error.message}`)
            continue
          }
          
          const ext = path.extname(entry.name).toLowerCase()
          
          tree.files.push({
            id: relativeFilePath.replace(/[^a-zA-Z0-9]/g, '-').toLowerCase(),
            name: path.basename(entry.name, ext),
            fileName: entry.name,
            path: relativeFilePath,
            type: 'file',
            extension: ext,
            size: stats.size,
            lastModified: stats.mtime.toISOString(),
            mimeType: getMimeType(ext)
          })
        }
      }
      
      // Sort children and files alphabetically
      tree.children.sort((a, b) => a.name.localeCompare(b.name))
      tree.files.sort((a, b) => a.fileName.localeCompare(b.fileName))
      
    } catch (error) {
      console.warn(`Error reading directory ${dirPath}:`, error)
    }
    
    return tree
  }
  
  return await buildTree(assetsPath)
}

// Helper function to categorize assets by type
async function categorizeAssetsByType(projectPath) {
  const assetsPath = path.join(projectPath, 'assets')
  console.log(`📊 Categorizing assets in: ${assetsPath}`)
  
  // Ensure assets directory exists
  try {
    await fs.access(assetsPath)
  } catch (error) {
    console.log(`Assets directory doesn't exist, creating: ${assetsPath}`)
    await ensureAssetsDirectory(projectPath)
  }
  
  const categories = {
    '3d-models': { name: '3D Models', files: [], extensions: ['.glb', '.gltf', '.obj', '.fbx'] },
    'textures': { name: 'Textures', files: [], extensions: ['.jpg', '.jpeg', '.png', '.bmp', '.tga', '.webp'] },
    'audio': { name: 'Audio', files: [], extensions: ['.mp3', '.wav', '.ogg', '.m4a'] },
    'scripts': { name: 'Scripts', files: [], extensions: ['.js', '.ts'] },
    'data': { name: 'Data', files: [], extensions: ['.json', '.xml'] },
    'misc': { name: 'Miscellaneous', files: [], extensions: [] }
  }
  
  const categorizeFiles = async (dirPath, relativePath = '') => {
    try {
      const entries = await fs.readdir(dirPath, { withFileTypes: true })
      
      for (const entry of entries) {
        if (entry.name.startsWith('.') || entry.name === 'Thumbs.db') continue
        
        const fullPath = path.join(dirPath, entry.name)
        const relativeFilePath = path.join(relativePath, entry.name).replace(/\\/g, '/')
        
        if (entry.isDirectory()) {
          await categorizeFiles(fullPath, relativeFilePath)
        } else {
          let stats
          try {
            stats = await fs.stat(fullPath)
          } catch (error) {
            continue
          }
          
          const ext = path.extname(entry.name).toLowerCase()
          
          // Find appropriate category
          let category = 'misc'
          for (const [categoryId, categoryData] of Object.entries(categories)) {
            if (categoryData.extensions.includes(ext)) {
              category = categoryId
              break
            }
          }
          
          const fileData = {
            id: relativeFilePath.replace(/[^a-zA-Z0-9]/g, '-').toLowerCase(),
            name: path.basename(entry.name, ext),
            fileName: entry.name,
            path: relativeFilePath,
            type: 'file',
            extension: ext,
            size: stats.size,
            lastModified: stats.mtime.toISOString(),
            mimeType: getMimeType(ext)
          }
          
          categories[category].files.push(fileData)
        }
      }
    } catch (error) {
      console.warn(`Error categorizing directory ${dirPath}:`, error)
    }
  }
  
  await categorizeFiles(assetsPath)
  
  // Sort files in each category
  Object.values(categories).forEach(category => {
    category.files.sort((a, b) => a.fileName.localeCompare(b.fileName))
  })
  
  const totalFiles = Object.values(categories).reduce((sum, cat) => sum + cat.files.length, 0)
  console.log(`📊 Categorization complete: ${totalFiles} total files categorized`)
  
  return categories
}

// Helper function to get folder contents (for backward compatibility)
async function listProjectAssets(projectPath, folderPath = '') {
  const assetsPath = path.join(projectPath, 'assets', folderPath)
  const assetsList = []
  
  try {
    const entries = await fs.readdir(assetsPath, { withFileTypes: true })
    
    for (const entry of entries) {
      // Skip hidden files/directories
      if (entry.name.startsWith('.') || entry.name === 'Thumbs.db') continue
      
      const fullPath = path.join(assetsPath, entry.name)
      const relativeFilePath = path.join(folderPath, entry.name).replace(/\\/g, '/')
      
      if (entry.isDirectory()) {
        // Add folder entry
        assetsList.push({
          id: relativeFilePath.replace(/[^a-zA-Z0-9]/g, '-').toLowerCase(),
          name: entry.name,
          fileName: entry.name,
          path: relativeFilePath,
          type: 'folder',
          extension: '',
          size: 0,
          lastModified: new Date().toISOString(),
          mimeType: 'folder'
        })
      } else {
        // Add file entry
        let stats
        try {
          stats = await fs.stat(fullPath)
        } catch (error) {
          console.warn(`Skipping broken asset: ${relativeFilePath} - ${error.message}`)
          continue
        }
        
        const ext = path.extname(entry.name).toLowerCase()
        
        assetsList.push({
          id: relativeFilePath.replace(/[^a-zA-Z0-9]/g, '-').toLowerCase(),
          name: path.basename(entry.name, ext),
          fileName: entry.name,
          path: relativeFilePath,
          type: 'file',
          extension: ext,
          size: stats.size,
          lastModified: stats.mtime.toISOString(),
          mimeType: getMimeType(ext)
        })
      }
    }
    
    // Sort folders first, then files
    assetsList.sort((a, b) => {
      if (a.type === 'folder' && b.type === 'file') return -1
      if (a.type === 'file' && b.type === 'folder') return 1
      return a.fileName.localeCompare(b.fileName)
    })
    
  } catch (error) {
    console.warn('Error listing assets:', error)
  }
  
  return assetsList
}

// Helper function to collect assets from project directory
async function collectProjectAssets(projectPath) {
  const assets = {}
  const assetsPath = path.join(projectPath, 'assets')
  
  try {
    const collectFromDir = async (dirPath, relativePath = '') => {
      const entries = await fs.readdir(dirPath, { withFileTypes: true })
      
      for (const entry of entries) {
        const fullPath = path.join(dirPath, entry.name)
        const relativeFilePath = path.join(relativePath, entry.name).replace(/\\/g, '/')
        
        if (entry.isDirectory()) {
          await collectFromDir(fullPath, relativeFilePath)
        } else {
          // Read file and encode based on type
          const fileData = await fs.readFile(fullPath)
          const ext = path.extname(entry.name).toLowerCase()
          
          if (['.js', '.json', '.txt', '.md'].includes(ext)) {
            // Text files
            assets[relativeFilePath] = fileData.toString('utf8')
          } else {
            // Binary files - encode as base64
            const mimeType = getMimeType(ext)
            assets[relativeFilePath] = `data:${mimeType};base64,${fileData.toString('base64')}`
          }
        }
      }
    }
    
    await collectFromDir(assetsPath)
  } catch (error) {
    console.warn('Error collecting assets:', error)
  }
  
  return assets
}

// Helper function to extract assets to project directory
async function extractProjectAssets(projectPath, assets) {
  const assetsPath = path.join(projectPath, 'assets')
  
  for (const [relativePath, content] of Object.entries(assets)) {
    const fullPath = path.join(assetsPath, relativePath)
    const dirPath = path.dirname(fullPath)
    
    // Ensure directory exists
    await fs.mkdir(dirPath, { recursive: true })
    
    if (typeof content === 'string' && content.startsWith('data:')) {
      // Base64 encoded binary file
      const base64Data = content.split(',')[1]
      const binaryData = Buffer.from(base64Data, 'base64')
      await fs.writeFile(fullPath, binaryData)
    } else {
      // Text file
      await fs.writeFile(fullPath, content, 'utf8')
    }
  }
}

// Helper function to get MIME type from extension
function getMimeType(ext) {
  const mimeTypes = {
    '.jpg': 'image/jpeg',
    '.jpeg': 'image/jpeg',
    '.png': 'image/png',
    '.gif': 'image/gif',
    '.webp': 'image/webp',
    '.mp3': 'audio/mpeg',
    '.wav': 'audio/wav',
    '.ogg': 'audio/ogg',
    '.glb': 'model/gltf-binary',
    '.gltf': 'model/gltf+json',
    '.obj': 'application/octet-stream',
    '.fbx': 'application/octet-stream'
  }
  
  return mimeTypes[ext] || 'application/octet-stream'
}