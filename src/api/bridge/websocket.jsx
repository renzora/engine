import { getWebSocketClient } from '../websocket/WebSocketClient.jsx';

/**
 * WebSocket-based Bridge API
 * High-performance replacement for HTTP-based bridge communication
 */

const client = getWebSocketClient();

// Ensure connection before API calls
async function ensureConnection() {
  if (!client.connected) {
    await client.connect();
  }
}

// Project Management
export async function getProjects() {
  await ensureConnection();
  const response = await client.listProjects();
  
  if (response.data?.error) {
    throw new Error(response.data.error);
  }
  
  return response.data?.projects || [];
}

export async function createProject(name, template = 'basic', settings = null) {
  await ensureConnection();
  const response = await client.createProject(name, template);
  
  if (response.data?.error) {
    throw new Error(response.data.error);
  }
  
  return response.data?.project;
}

// File Operations
export async function listDirectory(path) {
  await ensureConnection();
  const response = await client.listDirectory(path);
  
  if (response.data?.error) {
    throw new Error(response.data.error);
  }
  
  return response.data?.items || [];
}

export async function readFile(path) {
  await ensureConnection();
  const response = await client.readFile(path);
  
  if (response.data?.error) {
    throw new Error(response.data.error);
  }
  
  return response.data?.content;
}

export async function writeFile(path, content, createDirs = false) {
  await ensureConnection();
  const response = await client.writeFile(path, content);
  
  if (response.data?.error) {
    throw new Error(response.data.error);
  }
  
  return response.data?.success;
}

export async function writeBinaryFile(path, base64Content, createDirs = false) {
  await ensureConnection();
  // For binary files, we'll use a different message type
  const response = await client.sendRequest({
    type: 'FileBinaryWrite',
    data: { path, data: base64Content, create_dirs: createDirs }
  });
  
  if (response.data?.error) {
    throw new Error(response.data.error);
  }
  
  return response.data?.success;
}

export async function readBinaryFile(path) {
  await ensureConnection();
  const response = await client.sendRequest({
    type: 'FileBinaryRead',
    data: { path }
  });
  
  if (response.data?.error) {
    throw new Error(response.data.error);
  }
  
  return response.data?.content;
}

export async function deleteFile(path) {
  await ensureConnection();
  const response = await client.deleteFile(path);
  
  if (response.data?.error) {
    throw new Error(response.data.error);
  }
  
  return response.data?.success;
}

// Utility Functions
export function getFileUrl(path) {
  // For WebSocket, we might need to read the file as base64 or provide a different mechanism
  // For now, we'll provide a data URL approach
  return `ws-file://${path}`;
}

// File Watching
export function startFileWatcher(projectName = null) {
  client.startWatching(projectName);
}

export function stopFileWatcher() {
  client.stopWatching();
}

export function onFileChange(callback) {
  return client.on('file-changes', callback);
}

// Health and System
export async function getHealth() {
  await ensureConnection();
  return await client.healthCheck();
}

export async function getSystemStats() {
  await ensureConnection();
  return await client.getSystemStats();
}

// Connection Management
export function onConnectionChange(callback) {
  return client.on('connected', (serverInfo) => {
    callback({ connected: true, serverInfo });
  });
}

export function onConnectionError(callback) {
  return client.on('error', callback);
}

export async function connectToServer() {
  return await client.connect();
}

export function disconnectFromServer() {
  client.disconnect();
}

export function isConnected() {
  return client.connected;
}

// Get the underlying WebSocket client for advanced usage
export function getClient() {
  return client;
}