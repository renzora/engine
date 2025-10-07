/**
 * Bridge API - Adaptive Backend Communication
 * Automatically switches between WebSocket (new server) and HTTP (legacy bridge)
 */

import { createSignal } from 'solid-js';

// Import both implementations
import * as httpBridge from './files.js';
import * as wsBridge from './websocket.jsx';

// Direct imports and re-exports for critical functions
import { 
  connectToServer as wsConnectToServer,
  isConnected as wsIsConnected
} from './websocket.jsx';

// Immediately export the critical functions
export { wsConnectToServer as connectToServer };

export const getCurrentTransport = () => {
  try {
    const useWS = useWebSocket();
    if (useWS && wsIsConnected && wsIsConnected()) {
      return 'websocket';
    } else if (useWS) {
      return 'websocket-connecting';
    } else {
      return 'http';
    }
  } catch (error) {
    console.error('getCurrentTransport error:', error);
    return 'unknown';
  }
};

export const isConnected = () => {
  try {
    if (useWebSocket()) {
      return wsIsConnected ? wsIsConnected() : false;
    } else {
      return true; // HTTP is always "connected"
    }
  } catch (error) {
    console.error('isConnected error:', error);
    return false;
  }
};

// Configuration
const [useWebSocket, setUseWebSocket] = createSignal(
  // Check environment variable or default to WebSocket
  // Use native browser environment or default to true
  typeof window !== 'undefined' ? 
    (localStorage.getItem('renzora_use_websocket') !== 'false') : true
);

const [serverStatus, setServerStatus] = createSignal('checking');

// Server detection and fallback logic
async function detectBestServer() {
  try {
    // Try WebSocket server first (port 3002)
    console.log('🔍 Detecting best server...');
    
    if (useWebSocket()) {
      console.log('🔌 Trying WebSocket server (port 3002)...');
      await wsBridge.connectToServer();
      
      // Test with a simple health check
      const health = await wsBridge.getHealth();
      if (health?.status === 'healthy') {
        console.log('✅ WebSocket server available and healthy');
        setServerStatus('websocket');
        return 'websocket';
      }
    }
  } catch (error) {
    console.warn('⚠️ WebSocket server not available:', error.message);
  }

  try {
    // Fallback to HTTP bridge (port 3001)
    console.log('🌐 Falling back to HTTP bridge (port 3001)...');
    const response = await fetch('http://localhost:3001/health');
    
    if (response.ok) {
      console.log('✅ HTTP bridge available');
      setUseWebSocket(false);
      setServerStatus('http');
      return 'http';
    }
  } catch (error) {
    console.warn('⚠️ HTTP bridge not available:', error.message);
  }

  console.error('❌ No servers available');
  setServerStatus('none');
  throw new Error('No bridge servers available');
}

// Initialize server detection
let serverDetectionPromise = null;

// Export configuration function
export const setWebSocketPreference = (enabled) => {
  setUseWebSocket(enabled);
  if (typeof window !== 'undefined') {
    localStorage.setItem('renzora_use_websocket', enabled ? 'true' : 'false');
  }
  // Reset server detection to re-evaluate with new preference
  serverDetectionPromise = null;
};

async function ensureServer() {
  if (!serverDetectionPromise) {
    serverDetectionPromise = detectBestServer();
  }
  return await serverDetectionPromise;
}

// Adaptive API functions that switch between implementations
export async function getProjects() {
  await ensureServer();
  
  if (useWebSocket()) {
    return wsBridge.getProjects();
  } else {
    return httpBridge.getProjects();
  }
}

export async function createProject(name, template = 'basic', settings = null) {
  await ensureServer();
  
  if (useWebSocket()) {
    return wsBridge.createProject(name, template, settings);
  } else {
    return httpBridge.createProject(name, template, settings);
  }
}

export async function listDirectory(path) {
  await ensureServer();
  
  if (useWebSocket()) {
    return wsBridge.listDirectory(path);
  } else {
    return httpBridge.listDirectory(path);
  }
}

export async function readFile(path) {
  await ensureServer();
  
  if (useWebSocket()) {
    return wsBridge.readFile(path);
  } else {
    return httpBridge.readFile(path);
  }
}

export async function writeFile(path, content, createDirs = false) {
  await ensureServer();
  
  if (useWebSocket()) {
    return wsBridge.writeFile(path, content, createDirs);
  } else {
    return httpBridge.writeFile(path, content, createDirs);
  }
}

export async function writeBinaryFile(path, base64Content, createDirs = false) {
  await ensureServer();
  
  if (useWebSocket()) {
    return wsBridge.writeBinaryFile(path, base64Content, createDirs);
  } else {
    return httpBridge.writeBinaryFile(path, base64Content, createDirs);
  }
}

export async function readBinaryFile(path) {
  await ensureServer();
  
  if (useWebSocket()) {
    return wsBridge.readBinaryFile(path);
  } else {
    return httpBridge.readBinaryFile(path);
  }
}

export async function deleteFile(path) {
  await ensureServer();
  
  if (useWebSocket()) {
    return wsBridge.deleteFile(path);
  } else {
    return httpBridge.deleteFile(path);
  }
}

export function getFileUrl(path) {
  if (useWebSocket()) {
    return wsBridge.getFileUrl(path);
  } else {
    return httpBridge.getFileUrl(path);
  }
}

// File watching with automatic fallback
export function startFileWatcher(projectName = null) {
  if (useWebSocket()) {
    return wsBridge.startFileWatcher(projectName);
  } else {
    // HTTP bridge doesn't have file watching, so this is a no-op
    console.warn('⚠️ File watching not available with HTTP bridge');
  }
}

export function stopFileWatcher() {
  if (useWebSocket()) {
    return wsBridge.stopFileWatcher();
  }
}

export function onFileChange(callback) {
  if (useWebSocket()) {
    return wsBridge.onFileChange(callback);
  } else {
    // For HTTP bridge, we'd need to use the SSE endpoint
    // Return a no-op unsubscribe function
    return () => {};
  }
}

// Health and system info
export async function getHealth() {
  if (useWebSocket()) {
    return wsBridge.getHealth();
  } else {
    try {
      const response = await fetch('http://localhost:3001/health');
      return await response.json();
    } catch {
      throw new Error('HTTP bridge health check failed');
    }
  }
}

export async function getSystemStats() {
  if (useWebSocket()) {
    return wsBridge.getSystemStats();
  } else {
    try {
      const response = await fetch('http://localhost:3001/system/stats');
      return await response.json();
    } catch {
      throw new Error('HTTP bridge system stats failed');
    }
  }
}

// Connection management
export function onConnectionChange(callback) {
  if (useWebSocket()) {
    return wsBridge.onConnectionChange(callback);
  } else {
    // HTTP doesn't have persistent connections
    callback({ connected: true, serverInfo: { transport: 'http' }});
    return () => {};
  }
}

export function onConnectionError(callback) {
  if (useWebSocket()) {
    return wsBridge.onConnectionError(callback);
  } else {
    return () => {};
  }
}

export function disconnectFromServer() {
  if (useWebSocket()) {
    return wsBridge.disconnectFromServer();
  }
}

// Configuration and status - duplicates removed, using top-level exports

export function getServerStatus() {
  try {
    return serverStatus();
  } catch (error) {
    console.error('getServerStatus error:', error);
    return 'unknown';
  }
}

export function forceUseWebSocket(force = true) {
  setUseWebSocket(force);
  // Reset server detection
  serverDetectionPromise = null;
}

export function forceUseHTTP() {
  setUseWebSocket(false);
  // Reset server detection  
  serverDetectionPromise = null;
}

// Note: HTTP bridge exports removed - all functions now use WebSocket
// Legacy exports have been replaced with WebSocket implementations above