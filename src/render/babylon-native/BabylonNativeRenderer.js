// Babylon Native renderer API implementation for Renzora engine

import { BaseRenderer } from '../../api/render/BaseRenderer.js';

export class BabylonNativeRenderer extends BaseRenderer {
  constructor(config) {
    super(config);
    this.nativeEngine = null;
    this.scene = null;
    this.canvas = null;
  }

  async initialize(canvas, options = {}) {
    try {
      this.canvas = canvas;
      
      // Check if running in Tauri environment
      if (!this.isTauriEnvironment()) {
        throw new Error('Babylon Native requires Tauri environment');
      }

      // Import Tauri API for native bridge
      const { invoke } = await import('@tauri-apps/api/core');
      
      // Initialize Babylon Native bridge
      const result = await invoke('init_babylon_native', {
        width: canvas.width || 800,
        height: canvas.height || 600,
        ...options
      });
      
      console.log('Babylon Native initialized:', result);
      
      // Set up native rendering context
      this.nativeEngine = {
        invoke,
        canvas,
        initialized: true
      };

      this._notifyReady();
      return true;
    } catch (error) {
      console.error('Failed to initialize Babylon Native renderer:', error);
      this._notifyError(error);
      throw error;
    }
  }

  async render(sceneData) {
    if (!this.nativeEngine?.initialized) {
      throw new Error('Babylon Native renderer not initialized');
    }

    try {
      const serializedScene = JSON.stringify(sceneData);
      const result = await this.nativeEngine.invoke('babylon_native_render', serializedScene);
      return result;
    } catch (error) {
      console.error('Babylon Native render error:', error);
      this._notifyError(error);
      throw error;
    }
  }

  async resize(width, height) {
    if (this.canvas) {
      this.canvas.width = width;
      this.canvas.height = height;
    }
    
    if (this.nativeEngine?.invoke) {
      try {
        await this.nativeEngine.invoke('babylon_native_resize', { width, height });
      } catch (error) {
        console.warn('Babylon Native resize failed:', error);
      }
    }
  }

  async loadScene(sceneData) {
    return await this.render(sceneData);
  }

  async updateScene(sceneData) {
    return await this.render(sceneData);
  }

  async updateCamera(cameraData) {
    if (this.nativeEngine?.invoke) {
      try {
        await this.nativeEngine.invoke('babylon_native_update_camera', cameraData);
        return true;
      } catch (error) {
        console.warn('Camera update failed:', error);
        return false;
      }
    }
    return false;
  }

  async updateLights(lightData) {
    if (this.nativeEngine?.invoke) {
      try {
        await this.nativeEngine.invoke('babylon_native_update_lights', lightData);
        return true;
      } catch (error) {
        console.warn('Lights update failed:', error);
        return false;
      }
    }
    return false;
  }

  async addObject(objectData) {
    if (this.nativeEngine?.invoke) {
      try {
        await this.nativeEngine.invoke('babylon_native_add_object', objectData);
        return true;
      } catch (error) {
        console.warn('Add object failed:', error);
        return false;
      }
    }
    return false;
  }

  async removeObject(objectId) {
    if (this.nativeEngine?.invoke) {
      try {
        await this.nativeEngine.invoke('babylon_native_remove_object', { id: objectId });
        return true;
      } catch (error) {
        console.warn('Remove object failed:', error);
        return false;
      }
    }
    return false;
  }

  async updateObject(objectId, objectData) {
    if (this.nativeEngine?.invoke) {
      try {
        await this.nativeEngine.invoke('babylon_native_update_object', { id: objectId, data: objectData });
        return true;
      } catch (error) {
        console.warn('Update object failed:', error);
        return false;
      }
    }
    return false;
  }

  async updateMaterial(materialId, materialData) {
    if (this.nativeEngine?.invoke) {
      try {
        await this.nativeEngine.invoke('babylon_native_update_material', { id: materialId, data: materialData });
        return true;
      } catch (error) {
        console.warn('Update material failed:', error);
        return false;
      }
    }
    return false;
  }

  getStats() {
    return {
      renderer: 'babylon-native',
      initialized: this.initialized,
      backend: 'babylon-native-cpp',
      platform: 'tauri'
    };
  }

  async captureFrame() {
    if (this.nativeEngine?.invoke) {
      try {
        return await this.nativeEngine.invoke('babylon_native_capture_frame');
      } catch (error) {
        console.warn('Frame capture failed:', error);
        return null;
      }
    }
    return null;
  }

  isTauriEnvironment() {
    return typeof window !== 'undefined' && window.__TAURI_INTERNALS__;
  }

  async dispose() {
    if (this.nativeEngine?.invoke) {
      try {
        await this.nativeEngine.invoke('babylon_native_cleanup');
      } catch (error) {
        console.warn('Babylon Native cleanup failed:', error);
      }
    }

    if (this.engine) {
      this.engine.dispose();
    }

    this.nativeEngine = null;
    this.scene = null;
    this.canvas = null;
    this.initialized = false;
  }
}