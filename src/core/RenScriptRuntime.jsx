import { engineStore, engineActions, engineGetters } from '@/stores/EngineStore.jsx';

/**
 * RenScript runtime - provides the complete API for script execution
 * Exposes the entire engine and Babylon.js ecosystem to scripts
 */
export class RenScriptRuntime {
  constructor(babylonBridge) {
    this.bridge = babylonBridge;
    this.scriptInstances = new Map(); // Track running script instances
    this.globalAPI = this.createGlobalAPI();
    this.eventSystem = this.createEventSystem();
    
    console.log('🎯 RenScriptRuntime: Initialized');
  }

  /**
   * Create the global API available to all RenScript scripts
   */
  createGlobalAPI() {
    return {
      // Engine Core API
      Engine: {
        // Object management
        getObject: (id) => this.getEngineObject(id),
        createObject: (data) => this.createEngineObject(data),
        deleteObject: (id) => this.deleteEngineObject(id),
        findObjectsByTag: (tag) => this.findObjectsByTag(tag),
        findObjectsByName: (name) => this.findObjectsByName(name),
        findObjectsByLayer: (layer) => this.findObjectsByLayer(layer),
        
        // Scene management
        getCurrentScene: () => engineGetters.getCurrentSceneId(),
        getSceneData: (sceneId) => engineStore.scenes.sceneData[sceneId],
        loadScene: (sceneId) => this.loadScene(sceneId),
        unloadScene: (sceneId) => this.unloadScene(sceneId),
        createScene: (sceneData) => engineActions.createScene(sceneData),
        
        // Asset system
        getAsset: (category, assetId) => engineGetters.getAsset(category, assetId),
        getAllAssets: (category) => engineGetters.getAllAssets(category),
        loadAsset: (path) => this.loadAsset(path),
        
        // Selection and editor
        getSelection: () => engineGetters.getSelection(),
        setSelection: (objectIds) => engineActions.setSelection(objectIds),
        addToSelection: (objectId) => engineActions.addToSelection(objectId),
        clearSelection: () => engineActions.clearSelection(),
        
        // Runtime state
        isPlaying: () => engineGetters.isPlaying(),
        isPaused: () => engineGetters.isPaused(),
        getDeltaTime: () => engineStore.runtime.deltaTime,
        getTotalTime: () => engineStore.runtime.totalTime,
        getFrameCount: () => engineStore.runtime.currentFrame,
        
        // Performance
        getPerformanceStats: () => engineStore.runtime.performance,
        getFPS: () => engineStore.runtime.performance.fps
      },

      // Full Babylon.js API
      Babylon: this.bridge ? this.bridge.getBabylonAPI() : {},

      // Math utilities
      Math: {
        ...Math,
        
        // Additional math functions
        lerp: (a, b, t) => a + (b - a) * t,
        clamp: (value, min, max) => Math.max(min, Math.min(max, value)),
        remap: (value, fromMin, fromMax, toMin, toMax) => {
          const t = (value - fromMin) / (fromMax - fromMin);
          return toMin + t * (toMax - toMin);
        },
        
        // Vector utilities
        distance: (a, b) => {
          if (a.length && b.length) {
            let sum = 0;
            for (let i = 0; i < Math.min(a.length, b.length); i++) {
              sum += (a[i] - b[i]) ** 2;
            }
            return Math.sqrt(sum);
          }
          return 0;
        }
      },

      // Vector3 utilities (Babylon.js Vector3 with shortcuts)
      Vector3: this.bridge ? this.bridge.scene.getEngine().constructor.Vector3 : null,
      
      // Input system
      Input: this.createInputAPI(),
      
      // Audio system
      Audio: this.createAudioAPI(),
      
      // Physics system
      Physics: this.createPhysicsAPI(),
      
      // Events and messaging
      Events: this.eventSystem,
      
      // Time and animation
      Time: {
        deltaTime: () => engineStore.runtime.deltaTime,
        totalTime: () => engineStore.runtime.totalTime,
        timeScale: () => engineStore.runtime.playbackSpeed,
        setTimeScale: (scale) => engineActions.setPlaybackSpeed(scale)
      },
      
      // Logging and debugging
      Debug: {
        log: (...args) => console.log('[RenScript]', ...args),
        warn: (...args) => console.warn('[RenScript]', ...args),
        error: (...args) => console.error('[RenScript]', ...args),
        drawLine: (from, to, color) => this.drawDebugLine(from, to, color),
        drawSphere: (position, radius, color) => this.drawDebugSphere(position, radius, color)
      },

      // Utility functions
      Utils: {
        generateId: () => `${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
        delay: (ms) => new Promise(resolve => setTimeout(resolve, ms)),
        nextFrame: () => new Promise(resolve => requestAnimationFrame(resolve)),
        
        // Type checking
        isNumber: (value) => typeof value === 'number' && !isNaN(value),
        isString: (value) => typeof value === 'string',
        isArray: (value) => Array.isArray(value),
        isObject: (value) => value !== null && typeof value === 'object' && !Array.isArray(value),
        
        // JSON utilities
        parseJSON: (str) => {
          try { return JSON.parse(str); } catch { return null; }
        },
        stringifyJSON: (obj) => {
          try { return JSON.stringify(obj); } catch { return null; }
        }
      },

      // Blueprint system integration (placeholder for now)
      Blueprints: this.createBlueprintAPI(),
      
      // Component system access
      Components: {
        get: (objectId, componentType) => this.getComponent(objectId, componentType),
        add: (objectId, componentType, data) => this.addComponent(objectId, componentType, data),
        remove: (objectId, componentType) => this.removeComponent(objectId, componentType),
        has: (objectId, componentType) => this.hasComponent(objectId, componentType)
      }
    };
  }

  /**
   * Create input system API
   */
  createInputAPI() {
    return {
      // Keyboard
      isKeyDown: (key) => this.bridge?.scene.actionManager?.isKeyDown?.(key) || false,
      isKeyUp: (key) => this.bridge?.scene.actionManager?.isKeyUp?.(key) || false,
      
      // Mouse
      getMousePosition: () => {
        if (this.bridge?.scene.pointerX !== undefined) {
          return [this.bridge.scene.pointerX, this.bridge.scene.pointerY];
        }
        return [0, 0];
      },
      isMouseButtonDown: () => false, // TODO: Implement
      
      // Touch
      getTouchCount: () => 0, // TODO: Implement
      getTouch: () => null // TODO: Implement
    };
  }

  /**
   * Create audio system API
   */
  createAudioAPI() {
    return {
      play: (audioId, volume = 1.0) => {
        // TODO: Implement using Babylon.js Sound API directly
        console.log(`🔊 Playing audio: ${audioId} at volume ${volume}`);
        return null;
      },
      stop: (audioId) => {
        // TODO: Implement using Babylon.js Sound API directly
        console.log(`🔇 Stopping audio: ${audioId}`);
        return null;
      },
      setMasterVolume: () => {
        // TODO: Implement
      },
      getMasterVolume: () => engineStore.project.settings.audio.masterVolume || 1.0
    };
  }

  /**
   * Create physics system API
   */
  createPhysicsAPI() {
    return {
      raycast: () => {
        // TODO: Implement raycast using Babylon.js physics
        return null;
      },
      setGravity: (gravity) => {
        // TODO: Implement using Babylon.js physics API directly
        console.log(`🌍 Setting gravity to:`, gravity);
        return null;
      },
      getGravity: () => engineStore.project.settings.physics.gravity || [0, -9.81, 0]
    };
  }

  /**
   * Create event system for cross-script communication
   */
  createEventSystem() {
    const events = new Map();
    
    return {
      emit: (eventName, data = null) => {
        const listeners = events.get(eventName);
        if (listeners) {
          listeners.forEach(callback => {
            try {
              callback(data);
            } catch (error) {
              console.error(`❌ Error in event listener for ${eventName}:`, error);
            }
          });
        }
      },
      
      on: (eventName, callback) => {
        if (!events.has(eventName)) {
          events.set(eventName, []);
        }
        events.get(eventName).push(callback);
      },
      
      off: (eventName, callback) => {
        const listeners = events.get(eventName);
        if (listeners) {
          const index = listeners.indexOf(callback);
          if (index !== -1) {
            listeners.splice(index, 1);
          }
        }
      },
      
      once: (eventName, callback) => {
        const onceCallback = (data) => {
          callback(data);
          this.off(eventName, onceCallback);
        };
        this.on(eventName, onceCallback);
      }
    };
  }

  /**
   * Create blueprint system API (placeholder)
   */
  createBlueprintAPI() {
    return {
      execute: (blueprintId, inputData = {}) => {
        // TODO: Implement blueprint execution
        console.log(`🔵 Blueprint ${blueprintId} executed with data:`, inputData);
        return null;
      },
      
      compile: () => {
        // TODO: Implement blueprint to RenScript compilation
        console.log('🔵 Blueprint compiled to RenScript');
        return '';
      }
    };
  }

  /**
   * Execute a RenScript with full API access
   */
  executeScript(scriptPath, context = {}, objectContext = null) {
    try {
      console.log(`🎯 Executing RenScript: ${scriptPath}`);
      
      // Get script from assets
      const scriptAsset = engineStore.assets.scripts[scriptPath];
      if (!scriptAsset) {
        console.error(`❌ Script not found: ${scriptPath}`);
        return null;
      }

      // Create execution context
      const executionContext = {
        ...this.globalAPI,
        ...context
      };

      // Add object-specific context if provided
      if (objectContext) {
        executionContext.self = this.createObjectAPI(objectContext);
        executionContext.transform = executionContext.self.transform;
      }

      // TODO: Implement actual RenScript interpreter
      // For now, this is a placeholder that would call your RenScript compiler/interpreter
      console.log(`🎯 Would execute script with context:`, Object.keys(executionContext));
      
      return { success: true, result: null };

    } catch (error) {
      console.error(`❌ Script execution failed for ${scriptPath}:`, error);
      return { success: false, error: error.message };
    }
  }

  /**
   * Create object-specific API for scripts attached to objects
   */
  createObjectAPI(babylonObject) {
    const engineId = this.bridge?.engineMap?.get(babylonObject);
    if (!engineId) return null;

    return {
      // Basic object info
      getId: () => engineId,
      getName: () => babylonObject.name,
      
      // Transform access
      transform: {
        position: babylonObject.position,
        rotation: babylonObject.rotation,
        scale: babylonObject.scaling,
        
        setPosition: (x, y, z) => {
          if (typeof x === 'object') {
            babylonObject.position = x;
          } else {
            babylonObject.position.set(x || 0, y || 0, z || 0);
          }
        },
        
        setRotation: (x, y, z) => {
          if (typeof x === 'object') {
            babylonObject.rotation = x;
          } else {
            babylonObject.rotation.set(x || 0, y || 0, z || 0);
          }
        },
        
        setScale: (x, y, z) => {
          if (typeof x === 'object') {
            babylonObject.scaling = x;
          } else if (typeof x === 'number' && y === undefined) {
            babylonObject.scaling.setAll(x);
          } else {
            babylonObject.scaling.set(x || 1, y || 1, z || 1);
          }
        }
      },

      // Component access
      getComponent: (componentType) => {
        const component = babylonObject.engineComponents?.[componentType];
        return component ? component.getRenScriptAPI() : null;
      },

      // Hierarchy navigation
      getParent: () => {
        const parent = babylonObject.parent;
        return parent ? this.createObjectAPI(parent) : null;
      },
      
      getChildren: () => {
        return babylonObject.getChildren().map(child => this.createObjectAPI(child));
      },

      // Babylon.js object access
      getBabylonObject: () => babylonObject
    };
  }

  // Engine object management methods
  getEngineObject(objectId) {
    const currentScene = engineGetters.getCurrentSceneData();
    return currentScene?.sceneGraph?.nodes?.[objectId] || null;
  }

  createEngineObject(objectData) {
    const currentSceneId = engineGetters.getCurrentSceneId();
    if (!currentSceneId) {
      console.error('❌ No current scene to create object in');
      return null;
    }

    return engineActions.addObjectToScene(currentSceneId, objectData);
  }

  deleteEngineObject(objectId) {
    const currentSceneId = engineGetters.getCurrentSceneId();
    if (!currentSceneId) {
      console.error('❌ No current scene to delete object from');
      return false;
    }

    engineActions.removeObjectFromScene(currentSceneId, objectId);
    return true;
  }

  findObjectsByTag(tag) {
    const objects = engineGetters.getCurrentSceneObjects();
    return objects.filter(obj => obj.metadata?.tags?.includes(tag));
  }

  findObjectsByName(name) {
    const objects = engineGetters.getCurrentSceneObjects();
    return objects.filter(obj => obj.name === name || obj.name.includes(name));
  }

  findObjectsByLayer(layer) {
    const objects = engineGetters.getCurrentSceneObjects();
    return objects.filter(obj => obj.metadata?.layer === layer);
  }

  // Component management
  getComponent(objectId, componentType) {
    const babylonObject = engineGetters.getBabylonObject(objectId);
    if (!babylonObject) return null;

    const component = babylonObject.engineComponents?.[componentType];
    return component ? component.getRenScriptAPI() : null;
  }

  addComponent(objectId, componentType, data = {}) {
    // This would trigger the engine to add a component to an object
    const currentSceneId = engineGetters.getCurrentSceneId();
    if (!currentSceneId) return false;

    const object = engineActions.getObjectById(currentSceneId, objectId);
    if (!object) return false;

    const updatedComponents = { ...object.components, [componentType]: data };
    engineActions.updateObjectInScene(currentSceneId, objectId, { components: updatedComponents });
    return true;
  }

  removeComponent(objectId, componentType) {
    const currentSceneId = engineGetters.getCurrentSceneId();
    if (!currentSceneId) return false;

    const object = engineActions.getObjectById(currentSceneId, objectId);
    if (!object || !object.components[componentType]) return false;

    const updatedComponents = { ...object.components };
    delete updatedComponents[componentType];
    engineActions.updateObjectInScene(currentSceneId, objectId, { components: updatedComponents });
    return true;
  }

  hasComponent(objectId, componentType) {
    const currentSceneId = engineGetters.getCurrentSceneId();
    if (!currentSceneId) return false;

    const object = engineActions.getObjectById(currentSceneId, objectId);
    return object?.components?.[componentType] !== undefined;
  }

  // Debug utilities
  drawDebugLine(from, to, color = [1, 0, 0]) {
    // TODO: Implement debug line drawing
    console.log(`🔍 Debug line from ${from} to ${to} color ${color}`);
  }

  drawDebugSphere(position, radius, color = [1, 0, 0]) {
    // TODO: Implement debug sphere drawing
    console.log(`🔍 Debug sphere at ${position} radius ${radius} color ${color}`);
  }

  // Scene management
  loadScene(sceneId) {
    engineActions.loadScene(sceneId);
    return true;
  }

  unloadScene(sceneId) {
    engineActions.unloadScene(sceneId);
    return true;
  }

  loadAsset(path) {
    // TODO: Implement dynamic asset loading
    console.log(`📦 Loading asset: ${path}`);
    return null;
  }

  /**
   * Get runtime statistics
   */
  getStats() {
    return {
      activeScripts: this.scriptInstances.size,
      totalAPISize: Object.keys(this.globalAPI).length
    };
  }

  /**
   * Dispose the runtime
   */
  dispose() {
    console.log('🗑️ RenScriptRuntime: Disposing...');
    
    // Clear script instances
    this.scriptInstances.clear();
    
    // Clear event system
    this.eventSystem = null;
    
    console.log('✅ RenScriptRuntime: Disposed');
  }
}

export default RenScriptRuntime;