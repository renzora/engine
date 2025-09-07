import { createSignal, createEffect, onCleanup } from 'solid-js';

/**
 * WebSocket Client for Renzora Server
 * Provides high-performance WebSocket communication with the new server
 */
class WebSocketClient {
  constructor(url = 'ws://localhost:3002/ws') {
    this.url = url;
    this.ws = null;
    this.reconnectAttempts = 0;
    this.maxReconnectAttempts = 5;
    this.reconnectDelay = 2000;
    this.reconnectTimer = null;
    this.isConnecting = false;
    this.messageQueue = [];
    this.requestCallbacks = new Map();
    this.listeners = new Map();
    this.requestId = 0;
    
    // Connection state
    this.connected = false;
    this.serverInfo = null;
  }

  connect() {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      console.log('🔌 WebSocket already connected');
      return Promise.resolve();
    }

    if (this.isConnecting) {
      console.log('🔌 WebSocket connection already in progress');
      return Promise.resolve();
    }

    return new Promise((resolve, reject) => {
      this.isConnecting = true;
      console.log('🔌 Connecting to WebSocket server:', this.url);

      try {
        this.ws = new WebSocket(this.url);
        
        const connectTimeout = setTimeout(() => {
          reject(new Error('WebSocket connection timeout'));
        }, 10000);

        this.ws.onopen = () => {
          clearTimeout(connectTimeout);
          this.isConnecting = false;
          this.connected = true;
          this.reconnectAttempts = 0;
          console.log('✅ WebSocket connected successfully');
          
          // Process queued messages
          this.processMessageQueue();
          
          resolve();
        };

        this.ws.onmessage = (event) => {
          this.handleMessage(event);
        };

        this.ws.onclose = (event) => {
          clearTimeout(connectTimeout);
          this.isConnecting = false;
          this.connected = false;
          console.log('🔚 WebSocket connection closed:', event.code, event.reason);
          
          if (!event.wasClean && this.reconnectAttempts < this.maxReconnectAttempts) {
            this.scheduleReconnect();
          }
        };

        this.ws.onerror = (error) => {
          clearTimeout(connectTimeout);
          this.isConnecting = false;
          console.error('❌ WebSocket error:', error);
          reject(error);
        };

      } catch (error) {
        this.isConnecting = false;
        console.error('❌ Failed to create WebSocket:', error);
        reject(error);
      }
    });
  }

  disconnect() {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }

    if (this.ws) {
      this.ws.close(1000, 'Client disconnect');
      this.ws = null;
    }

    this.connected = false;
    this.isConnecting = false;
    this.reconnectAttempts = 0;
    console.log('👋 WebSocket disconnected');
  }

  scheduleReconnect() {
    if (this.reconnectTimer) return;

    this.reconnectAttempts++;
    const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);
    
    console.log(`🔄 Scheduling reconnection attempt ${this.reconnectAttempts}/${this.maxReconnectAttempts} in ${delay}ms`);
    
    this.reconnectTimer = setTimeout(async () => {
      this.reconnectTimer = null;
      try {
        await this.connect();
      } catch (error) {
        console.error('❌ Reconnection failed:', error);
        if (this.reconnectAttempts < this.maxReconnectAttempts) {
          this.scheduleReconnect();
        }
      }
    }, delay);
  }

  send(message) {
    if (!this.connected || !this.ws || this.ws.readyState !== WebSocket.OPEN) {
      console.warn('⚠️  WebSocket not connected, queuing message:', message);
      this.messageQueue.push(message);
      return;
    }

    try {
      const jsonMessage = JSON.stringify(message);
      this.ws.send(jsonMessage);
      console.log('📤 Sent WebSocket message:', message.type);
    } catch (error) {
      console.error('❌ Failed to send WebSocket message:', error);
    }
  }

  sendRequest(message, timeout = 30000) {
    return new Promise((resolve, reject) => {
      const requestId = ++this.requestId;
      const messageWithId = { ...message, requestId };

      const timeoutId = setTimeout(() => {
        this.requestCallbacks.delete(requestId);
        reject(new Error(`Request timeout: ${message.type}`));
      }, timeout);

      this.requestCallbacks.set(requestId, (response) => {
        clearTimeout(timeoutId);
        resolve(response);
      });

      this.send(messageWithId);
    });
  }

  processMessageQueue() {
    while (this.messageQueue.length > 0) {
      const message = this.messageQueue.shift();
      this.send(message);
    }
  }

  handleMessage(event) {
    try {
      const data = JSON.parse(event.data);
      console.log('📨 Received WebSocket message:', data.type);

      // Handle request responses
      if (data.requestId && this.requestCallbacks.has(data.requestId)) {
        const callback = this.requestCallbacks.get(data.requestId);
        this.requestCallbacks.delete(data.requestId);
        callback(data);
        return;
      }

      // Handle server-initiated messages
      switch (data.type) {
        case 'Connected':
          this.serverInfo = data.data;
          console.log('🎉 Server connection confirmed:', this.serverInfo);
          
          // Tell server where the engine is located (frontend knows best)
          this.initializeServerPaths();
          
          this.emit('connected', this.serverInfo);
          break;

        case 'FileChanges':
          console.log('📁 File changes received:', data.data.changes);
          this.emit('file-changes', data.data.changes);
          break;

        case 'Error':
          console.error('❌ Server error:', data.data);
          this.emit('error', data.data);
          break;

        default:
          console.log('📨 Unhandled message type:', data.type);
          this.emit('message', data);
      }

    } catch (error) {
      console.error('❌ Failed to parse WebSocket message:', error);
    }
  }

  // Event system
  on(event, callback) {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event).add(callback);
    
    return () => {
      const eventListeners = this.listeners.get(event);
      if (eventListeners) {
        eventListeners.delete(callback);
        if (eventListeners.size === 0) {
          this.listeners.delete(event);
        }
      }
    };
  }

  emit(event, data) {
    const eventListeners = this.listeners.get(event);
    if (eventListeners) {
      eventListeners.forEach(callback => {
        try {
          callback(data);
        } catch (error) {
          console.error('❌ Error in event callback:', error);
        }
      });
    }
  }

  // API Methods
  async healthCheck() {
    return this.sendRequest({ type: 'HealthCheck' });
  }

  async listProjects() {
    return this.sendRequest({ type: 'ListProjects' });
  }

  async createProject(name, template = 'basic') {
    return this.sendRequest({ 
      type: 'CreateProject', 
      data: { name, template }
    });
  }

  async readFile(path) {
    return this.sendRequest({ 
      type: 'FileRead', 
      data: { path }
    });
  }

  async writeFile(path, content) {
    return this.sendRequest({
      type: 'FileWrite',
      data: { path, content }
    });
  }

  async deleteFile(path) {
    return this.sendRequest({
      type: 'FileDelete',
      data: { path }
    });
  }

  async listDirectory(path) {
    return this.sendRequest({
      type: 'ListDirectory',
      data: { path }
    });
  }

  async getSystemStats() {
    return this.sendRequest({ type: 'SystemStats' });
  }

  startWatching(projectName = null) {
    this.send({
      type: 'StartWatching',
      data: { project_name: projectName }
    });
  }

  stopWatching() {
    this.send({ type: 'StopWatching' });
  }

  // Configuration Management
  async getServerConfig() {
    return this.sendRequest({ type: 'GetConfig' });
  }

  async setBasePath(path) {
    return this.sendRequest({
      type: 'SetBasePath',
      data: { path }
    });
  }

  async setProjectsPath(path) {
    return this.sendRequest({
      type: 'SetProjectsPath',
      data: { path }
    });
  }

  async scanForEngineRoots() {
    return this.sendRequest({ type: 'ScanForEngineRoot' });
  }

  // Initialize server with frontend's known paths
  async initializeServerPaths() {
    try {
      // Frontend knows where it's running from
      const currentPath = window.location.origin;
      
      // Derive engine path from frontend location
      // Frontend typically runs from engine root or engine/dist
      let enginePath;
      if (typeof window !== 'undefined') {
        // In browser, we can't directly access filesystem, but we can make educated guesses
        // The user will set the correct path via UI if needed
        enginePath = '../'; // Assume we're in a subdirectory
      } else {
        enginePath = process.cwd(); // In Node.js environments
      }

      console.log('📍 Initializing server with engine path:', enginePath);
      
      // Set the base path (frontend tells server where engine is)
      await this.setBasePath(enginePath);
      
      // Projects typically go in engine/projects
      const projectsPath = `${enginePath}/projects`;
      await this.setProjectsPath(projectsPath);
      
    } catch (error) {
      console.log('⚠️ Could not auto-initialize server paths:', error.message);
      console.log('💡 User can set paths manually via Settings');
    }
  }
}

// Singleton instance
let wsClient = null;

export function getWebSocketClient() {
  if (!wsClient) {
    wsClient = new WebSocketClient();
  }
  return wsClient;
}

export function createWebSocketConnection() {
  const [connected, setConnected] = createSignal(false);
  const [error, setError] = createSignal(null);
  const client = getWebSocketClient();

  // Auto-connect and handle connection state
  createEffect(async () => {
    try {
      await client.connect();
      setConnected(true);
      setError(null);
    } catch (err) {
      setError(err.message);
      setConnected(false);
    }
  });

  // Listen for connection events
  const unsubscribeConnected = client.on('connected', () => {
    setConnected(true);
    setError(null);
  });

  const unsubscribeError = client.on('error', (errorData) => {
    setError(errorData.message);
  });

  onCleanup(() => {
    unsubscribeConnected();
    unsubscribeError();
  });

  return {
    connected,
    error,
    client
  };
}

export default WebSocketClient;