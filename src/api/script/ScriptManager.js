import { ScriptAPI } from './ScriptAPI.js';

/**
 * ScriptManager - Manages script execution and lifecycle for Babylon.js objects
 */
class ScriptManager {
  constructor(scene) {
    this.scene = scene;
    this.activeScripts = new Map(); // objectId -> Set of script instances
    this.scriptClasses = new Map(); // scriptPath -> script class constructor
    this.isRunning = false;
    this.updateObserver = null;
    
    this.bindMethods();
  }
  
  bindMethods() {
    this.update = this.update.bind(this);
  }
  
  /**
   * Start the script manager and begin executing scripts
   */
  start() {
    if (this.isRunning) return;
    
    this.isRunning = true;
    console.log('🔧 ScriptManager: Starting script execution');
    
    // Register for scene updates
    this.updateObserver = this.scene.onBeforeRenderObservable.add(() => {
      this.update();
    });
  }
  
  /**
   * Stop the script manager and clean up all scripts
   */
  stop() {
    if (!this.isRunning) return;
    
    this.isRunning = false;
    console.log('🔧 ScriptManager: Stopping script execution');
    
    // Dispose update observer
    if (this.updateObserver) {
      this.scene.onBeforeRenderObservable.remove(this.updateObserver);
      this.updateObserver = null;
    }
    
    // Clean up all scripts
    this.activeScripts.forEach((scripts, objectId) => {
      this.removeAllScriptsFromObject(objectId);
    });
    
    this.activeScripts.clear();
    this.scriptClasses.clear();
  }
  
  /**
   * Register a script class from a loaded script file
   */
  registerScript(scriptPath, ScriptClass) {
    if (typeof ScriptClass !== 'function') {
      console.error('🔧 ScriptManager: Script must export a class', scriptPath);
      return false;
    }
    
    this.scriptClasses.set(scriptPath, ScriptClass);
    console.log('🔧 ScriptManager: Registered script class', scriptPath);
    return true;
  }
  
  /**
   * Add a script to an object
   */
  addScriptToObject(objectId, scriptPath) {
    const babylonObject = this.findBabylonObject(objectId);
    if (!babylonObject) {
      console.error('🔧 ScriptManager: Object not found', objectId);
      return false;
    }
    
    const ScriptClass = this.scriptClasses.get(scriptPath);
    if (!ScriptClass) {
      console.error('🔧 ScriptManager: Script class not registered', scriptPath);
      return false;
    }
    
    // Check if script is already attached
    if (this.activeScripts.has(objectId)) {
      const scripts = this.activeScripts.get(objectId);
      for (const script of scripts) {
        if (script._scriptPath === scriptPath) {
          console.warn('🔧 ScriptManager: Script already attached', scriptPath, objectId);
          return false;
        }
      }
    }
    
    try {
      // Create script API wrapper
      const scriptAPI = new ScriptAPI(this.scene, babylonObject);
      
      // Create script instance with API wrapper
      const scriptInstance = new ScriptClass(this.scene, scriptAPI);
      scriptInstance._scriptPath = scriptPath;
      scriptInstance._objectId = objectId;
      scriptInstance._babylonObject = babylonObject;
      scriptInstance._scriptAPI = scriptAPI;
      
      // Initialize script set for this object if needed
      if (!this.activeScripts.has(objectId)) {
        this.activeScripts.set(objectId, new Set());
      }
      
      // Add to active scripts
      this.activeScripts.get(objectId).add(scriptInstance);
      
      // Call onStart if available
      if (typeof scriptInstance.onStart === 'function') {
        scriptInstance.onStart();
      }
      
      console.log('🔧 ScriptManager: Script attached', scriptPath, 'to', objectId);
      return true;
      
    } catch (error) {
      console.error('🔧 ScriptManager: Failed to instantiate script', scriptPath, error);
      return false;
    }
  }
  
  /**
   * Remove a specific script from an object
   */
  removeScriptFromObject(objectId, scriptPath) {
    if (!this.activeScripts.has(objectId)) return false;
    
    const scripts = this.activeScripts.get(objectId);
    let scriptToRemove = null;
    
    for (const script of scripts) {
      if (script._scriptPath === scriptPath) {
        scriptToRemove = script;
        break;
      }
    }
    
    if (scriptToRemove) {
      // Call onDestroy if available
      if (typeof scriptToRemove.onDestroy === 'function') {
        try {
          scriptToRemove.onDestroy();
        } catch (error) {
          console.error('🔧 ScriptManager: Error in onDestroy', scriptPath, error);
        }
      }
      
      scripts.delete(scriptToRemove);
      
      // Clean up empty script sets
      if (scripts.size === 0) {
        this.activeScripts.delete(objectId);
      }
      
      console.log('🔧 ScriptManager: Script removed', scriptPath, 'from', objectId);
      return true;
    }
    
    return false;
  }
  
  /**
   * Remove all scripts from an object
   */
  removeAllScriptsFromObject(objectId) {
    if (!this.activeScripts.has(objectId)) return;
    
    const scripts = this.activeScripts.get(objectId);
    scripts.forEach(script => {
      if (typeof script.onDestroy === 'function') {
        try {
          script.onDestroy();
        } catch (error) {
          console.error('🔧 ScriptManager: Error in onDestroy', script._scriptPath, error);
        }
      }
    });
    
    this.activeScripts.delete(objectId);
    console.log('🔧 ScriptManager: All scripts removed from', objectId);
  }
  
  /**
   * Update all active scripts
   */
  update() {
    if (!this.isRunning) return;
    
    const deltaTime = this.scene.getEngine().getDeltaTime() / 1000; // Convert to seconds
    
    this.activeScripts.forEach((scripts, objectId) => {
      scripts.forEach(script => {
        // Update the API's delta time
        if (script._scriptAPI && typeof script._scriptAPI._updateDeltaTime === 'function') {
          script._scriptAPI._updateDeltaTime(deltaTime);
        }
        
        if (typeof script.onUpdate === 'function') {
          try {
            script.onUpdate(deltaTime);
          } catch (error) {
            console.error('🔧 ScriptManager: Error in onUpdate', script._scriptPath, error);
          }
        }
      });
    });
  }
  
  /**
   * Find a Babylon.js object by ID
   */
  findBabylonObject(objectId) {
    // Check meshes
    for (const mesh of this.scene.meshes) {
      if ((mesh.uniqueId || mesh.name) === objectId) {
        return mesh;
      }
    }
    
    // Check transform nodes
    for (const node of this.scene.transformNodes) {
      if ((node.uniqueId || node.name) === objectId) {
        return node;
      }
    }
    
    // Check lights
    for (const light of this.scene.lights) {
      if ((light.uniqueId || light.name) === objectId) {
        return light;
      }
    }
    
    // Check cameras
    for (const camera of this.scene.cameras) {
      if ((camera.uniqueId || camera.name) === objectId) {
        return camera;
      }
    }
    
    return null;
  }
  
  /**
   * Get all scripts attached to an object
   */
  getScriptsForObject(objectId) {
    if (!this.activeScripts.has(objectId)) return [];
    
    return Array.from(this.activeScripts.get(objectId)).map(script => ({
      path: script._scriptPath,
      instance: script
    }));
  }
  
  /**
   * Get statistics about active scripts
   */
  getStats() {
    let totalScripts = 0;
    this.activeScripts.forEach(scripts => {
      totalScripts += scripts.size;
    });
    
    return {
      objectsWithScripts: this.activeScripts.size,
      totalActiveScripts: totalScripts,
      registeredScriptClasses: this.scriptClasses.size,
      isRunning: this.isRunning
    };
  }
}

export { ScriptManager };