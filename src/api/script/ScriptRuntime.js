import { ScriptManager } from './ScriptManager.js';
import { ScriptAPI } from './ScriptAPI.js';
import { getScriptLoader } from './ScriptLoader.js';

/**
 * ScriptRuntime - Main runtime system that coordinates script execution
 * This is the public API that other parts of the application use
 */
class ScriptRuntime {
  constructor() {
    this.scriptManager = null;
    this.scriptLoader = getScriptLoader();
    this.scene = null;
    this.isInitialized = false;
  }
  
  /**
   * Initialize the script runtime with a Babylon.js scene
   * @param {Scene} scene - The Babylon.js scene
   */
  initialize(scene) {
    if (this.isInitialized) {
      console.warn('🔧 ScriptRuntime: Already initialized');
      return;
    }
    
    this.scene = scene;
    this.scriptManager = new ScriptManager(scene);
    this.isInitialized = true;
    
    console.log('🔧 ScriptRuntime: Initialized with scene');
    
    // Start the script manager
    this.start();
  }
  
  /**
   * Start script execution
   */
  start() {
    if (!this.isInitialized) {
      console.error('🔧 ScriptRuntime: Not initialized');
      return false;
    }
    
    this.scriptManager.start();
    console.log('🔧 ScriptRuntime: Started');
    return true;
  }
  
  /**
   * Pause script execution (keeps scripts attached)
   */
  pause() {
    if (!this.isInitialized) return;
    
    this.scriptManager.pause();
    console.log('🔧 ScriptRuntime: Paused');
  }
  
  /**
   * Stop script execution
   */
  stop() {
    if (!this.isInitialized) return;
    
    this.scriptManager.stop();
    console.log('🔧 ScriptRuntime: Stopped');
  }
  
  /**
   * Shutdown the runtime and clean up resources
   */
  shutdown() {
    if (!this.isInitialized) return;
    
    this.stop();
    this.scriptManager = null;
    this.scene = null;
    this.isInitialized = false;
    
    console.log('🔧 ScriptRuntime: Shutdown complete');
  }
  
  /**
   * Attach a script to an object
   * @param {string} objectId - ID of the object
   * @param {string} scriptPath - Path to the script file
   * @returns {Promise<boolean>} Success status
   */
  async attachScript(objectId, scriptPath) {
    if (!this.isInitialized) {
      console.error('🔧 ScriptRuntime: Not initialized');
      return false;
    }
    
    try {
      console.log('🔧 ScriptRuntime: Attaching script', scriptPath, 'to', objectId);
      
      // Load the script if not already loaded
      const ScriptClass = await this.scriptLoader.loadScript(scriptPath);
      
      // Register with script manager
      this.scriptManager.registerScript(scriptPath, ScriptClass);
      
      // Attach to object
      const success = this.scriptManager.addScriptToObject(objectId, scriptPath);
      
      if (success) {
        console.log('🔧 ScriptRuntime: Script attached successfully');
      }
      
      return success;
      
    } catch (error) {
      console.error('🔧 ScriptRuntime: Failed to attach script', error);
      return false;
    }
  }
  
  /**
   * Detach a script from an object
   * @param {string} objectId - ID of the object
   * @param {string} scriptPath - Path to the script file
   * @returns {boolean} Success status
   */
  detachScript(objectId, scriptPath) {
    if (!this.isInitialized) {
      console.error('🔧 ScriptRuntime: Not initialized');
      return false;
    }
    
    console.log('🔧 ScriptRuntime: Detaching script', scriptPath, 'from', objectId);
    
    const success = this.scriptManager.removeScriptFromObject(objectId, scriptPath);
    
    if (success) {
      console.log('🔧 ScriptRuntime: Script detached successfully');
    }
    
    return success;
  }
  
  /**
   * Detach all scripts from an object
   * @param {string} objectId - ID of the object
   */
  detachAllScripts(objectId) {
    if (!this.isInitialized) {
      console.error('🔧 ScriptRuntime: Not initialized');
      return;
    }
    
    console.log('🔧 ScriptRuntime: Detaching all scripts from', objectId);
    this.scriptManager.removeAllScriptsFromObject(objectId);
  }
  
  /**
   * Reload a script (useful for development)
   * @param {string} scriptPath - Path to the script file
   * @returns {Promise<boolean>} Success status
   */
  async reloadScript(scriptPath) {
    if (!this.isInitialized) {
      console.error('🔧 ScriptRuntime: Not initialized');
      return false;
    }
    
    try {
      console.log('🔧 ScriptRuntime: Reloading script', scriptPath);
      
      // Find all objects using this script
      const objectsWithScript = [];
      this.scriptManager.activeScripts.forEach((scripts, objectId) => {
        scripts.forEach(script => {
          if (script._scriptPath === scriptPath) {
            objectsWithScript.push(objectId);
          }
        });
      });
      
      // Detach from all objects
      objectsWithScript.forEach(objectId => {
        this.scriptManager.removeScriptFromObject(objectId, scriptPath);
      });
      
      // Clear from loader cache
      this.scriptLoader.loadedScripts.delete(scriptPath);
      
      // Reload the script
      const ScriptClass = await this.scriptLoader.loadScript(scriptPath);
      this.scriptManager.registerScript(scriptPath, ScriptClass);
      
      // Reattach to all objects
      for (const objectId of objectsWithScript) {
        this.scriptManager.addScriptToObject(objectId, scriptPath);
      }
      
      console.log('🔧 ScriptRuntime: Script reloaded successfully');
      return true;
      
    } catch (error) {
      console.error('🔧 ScriptRuntime: Failed to reload script', error);
      return false;
    }
  }
  
  /**
   * Pause a specific script on an object
   * @param {string} objectId - ID of the object
   * @param {string} scriptPath - Path to the script file
   */
  pauseScript(objectId, scriptPath) {
    if (!this.isInitialized) {
      console.error('🔧 ScriptRuntime: Not initialized');
      return false;
    }
    
    this.scriptManager.pauseScript(objectId, scriptPath);
    console.log('🔧 ScriptRuntime: Paused script', scriptPath, 'on object', objectId);
    return true;
  }
  
  /**
   * Resume a specific script on an object
   * @param {string} objectId - ID of the object
   * @param {string} scriptPath - Path to the script file
   */
  resumeScript(objectId, scriptPath) {
    if (!this.isInitialized) {
      console.error('🔧 ScriptRuntime: Not initialized');
      return false;
    }
    
    this.scriptManager.resumeScript(objectId, scriptPath);
    console.log('🔧 ScriptRuntime: Resumed script', scriptPath, 'on object', objectId);
    return true;
  }
  
  /**
   * Check if a specific script is paused
   * @param {string} objectId - ID of the object
   * @param {string} scriptPath - Path to the script file
   * @returns {boolean} True if the script is paused
   */
  isScriptPaused(objectId, scriptPath) {
    if (!this.isInitialized) return false;
    
    return this.scriptManager.isScriptPaused(objectId, scriptPath);
  }
  
  /**
   * Get scripts attached to an object
   * @param {string} objectId - ID of the object
   * @returns {Array} Array of script info objects
   */
  getScriptsForObject(objectId) {
    if (!this.isInitialized) return [];
    
    return this.scriptManager.getScriptsForObject(objectId);
  }
  
  /**
   * Get runtime statistics
   * @returns {Object} Statistics about the runtime
   */
  getStats() {
    if (!this.isInitialized) {
      return {
        initialized: false,
        running: false
      };
    }
    
    return {
      initialized: this.isInitialized,
      running: this.scriptManager.isRunning,
      scriptManager: this.scriptManager.getStats(),
      scriptLoader: this.scriptLoader.getStats()
    };
  }
  
  /**
   * Get script instance for an object
   * @param {string} objectId - ID of the object
   * @param {string} scriptPath - Path to the script file
   * @returns {Object|null} Script instance or null if not found
   */
  getScriptInstance(objectId, scriptPath) {
    if (!this.isInitialized) {
      console.error('🔧 ScriptRuntime: Not initialized');
      return null;
    }
    
    return this.scriptManager.getScriptInstance(objectId, scriptPath);
  }
  
  /**
   * Enable debug mode for more verbose logging
   * @param {boolean} enabled - Whether to enable debug mode
   */
  setDebugMode(enabled) {
    // Could be used to enable more verbose logging in the future
    console.log('🔧 ScriptRuntime: Debug mode', enabled ? 'enabled' : 'disabled');
  }
}

// Global instance
let runtimeInstance = null;

/**
 * Get the global script runtime instance
 * @returns {ScriptRuntime} The script runtime instance
 */
export function getScriptRuntime() {
  if (!runtimeInstance) {
    runtimeInstance = new ScriptRuntime();
  }
  return runtimeInstance;
}

/**
 * Initialize the script runtime (convenience function)
 * @param {Scene} scene - The Babylon.js scene
 */
export function initializeScriptRuntime(scene) {
  const runtime = getScriptRuntime();
  runtime.initialize(scene);
  return runtime;
}

export { ScriptRuntime };