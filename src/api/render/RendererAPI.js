// Central API for managing renderers in the Renzora engine

import { BaseRenderer } from './BaseRenderer.js';

export class RendererAPI {
  constructor() {
    this.activeRenderer = null;
    this.renderers = new Map();
    this.eventListeners = new Map();
  }

  // Renderer registration
  registerRenderer(id, rendererInstance) {
    if (!(rendererInstance instanceof BaseRenderer)) {
      throw new Error('Renderer must extend BaseRenderer');
    }

    this.renderers.set(id, rendererInstance);
    
    // Set up event forwarding
    rendererInstance.onReady(() => this._emit('renderer-ready', { id }));
    rendererInstance.onError((error) => this._emit('renderer-error', { id, error }));
  }

  // Renderer management
  async setActiveRenderer(id) {
    const renderer = this.renderers.get(id);
    if (!renderer) {
      throw new Error(`Renderer ${id} not found`);
    }

    // Dispose current renderer if any
    if (this.activeRenderer && this.activeRenderer !== renderer) {
      await this.activeRenderer.dispose();
    }

    this.activeRenderer = renderer;
    this._emit('renderer-changed', { id, renderer });
    return renderer;
  }

  getActiveRenderer() {
    return this.activeRenderer;
  }

  getRenderer(id) {
    return this.renderers.get(id);
  }

  getRegisteredRenderers() {
    return Array.from(this.renderers.keys());
  }

  // Rendering operations (delegates to active renderer)
  async initialize(canvas, options = {}) {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.initialize(canvas, options);
  }

  async render(sceneData) {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.render(sceneData);
  }

  async resize(width, height) {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.resize(width, height);
  }

  // Scene operations
  async loadScene(sceneData) {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.loadScene(sceneData);
  }

  async updateScene(sceneData) {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.updateScene(sceneData);
  }

  // Camera operations
  async updateCamera(cameraData) {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.updateCamera(cameraData);
  }

  // Camera movement methods
  async moveCamera(direction, distance) {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.moveCamera(direction, distance);
  }

  async rotateCamera(deltaX, deltaY) {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.rotateCamera(deltaX, deltaY);
  }

  async panCamera(deltaX, deltaY) {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.panCamera(deltaX, deltaY);
  }

  getCameraPosition() {
    if (!this.activeRenderer) {
      return null;
    }
    return this.activeRenderer.getCameraPosition();
  }

  getCameraRotation() {
    if (!this.activeRenderer) {
      return null;
    }
    return this.activeRenderer.getCameraRotation();
  }

  // Lighting operations
  async updateLights(lightData) {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.updateLights(lightData);
  }

  // Object operations
  async addObject(objectData) {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.addObject(objectData);
  }

  async removeObject(objectId) {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.removeObject(objectId);
  }

  async updateObject(objectId, objectData) {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.updateObject(objectId, objectData);
  }

  // Material operations
  async updateMaterial(materialId, materialData) {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.updateMaterial(materialId, materialData);
  }

  // Performance and debugging
  getStats() {
    if (!this.activeRenderer) {
      return null;
    }
    return this.activeRenderer.getStats();
  }

  async captureFrame() {
    if (!this.activeRenderer) {
      throw new Error('No active renderer set');
    }
    return await this.activeRenderer.captureFrame();
  }

  // Event system
  on(event, callback) {
    if (!this.eventListeners.has(event)) {
      this.eventListeners.set(event, []);
    }
    this.eventListeners.get(event).push(callback);
  }

  off(event, callback) {
    const listeners = this.eventListeners.get(event);
    if (listeners) {
      const index = listeners.indexOf(callback);
      if (index > -1) {
        listeners.splice(index, 1);
      }
    }
  }

  _emit(event, data) {
    const listeners = this.eventListeners.get(event);
    if (listeners) {
      listeners.forEach(callback => callback(data));
    }
  }

  // Cleanup
  async dispose() {
    if (this.activeRenderer) {
      await this.activeRenderer.dispose();
    }
    this.activeRenderer = null;
    this.renderers.clear();
    this.eventListeners.clear();
  }
}

// Global renderer API instance
export const rendererAPI = new RendererAPI();