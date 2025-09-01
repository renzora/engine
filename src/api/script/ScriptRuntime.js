import { ScriptManager } from './ScriptManager.js';
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
    this.propertyUpdateListener = null;
  }
  
  /**
   * Initialize the script runtime with a Babylon.js scene
   * @param {Scene} scene - The Babylon.js scene
   */
  initialize(scene) {
    console.log('🚀 ScriptRuntime: Initialize called');
    console.log('🚀 ScriptRuntime: Scene provided:', !!scene);
    console.log('🚀 ScriptRuntime: Already initialized:', this.isInitialized);
    
    if (this.isInitialized) {
      console.warn('🔧 ScriptRuntime: Already initialized');
      return;
    }
    
    this.scene = scene;
    
    console.log('🚀 ScriptRuntime: Creating ScriptManager...');
    try {
      this.scriptManager = new ScriptManager(scene);
      console.log('✅ ScriptRuntime: ScriptManager created successfully');
    } catch (error) {
      console.error('❌ ScriptRuntime: Failed to create ScriptManager:', error);
      throw error;
    }
    
    this.isInitialized = true;
    
    console.log('🔧 ScriptRuntime: Initialized with scene');
    
    // Set up event listener for live property updates
    console.log('🚀 ScriptRuntime: Setting up property update listener...');
    this.setupPropertyUpdateListener();
    
    // Start the script manager
    console.log('🚀 ScriptRuntime: Starting script manager...');
    this.start();
  }
  
  /**
   * Set up event listener for live script property updates
   */
  setupPropertyUpdateListener() {
    this.propertyUpdateListener = (event) => {
      const { scriptPath, properties, propertyChanges } = event.detail;
      
      if (!scriptPath || !properties || !propertyChanges) return;
      
      console.log('🔧 ScriptRuntime: Received property update event for', scriptPath);
      
      // Update script properties using the script manager
      this.scriptManager.updateScriptProperties(scriptPath, properties, propertyChanges);
    };
    
    document.addEventListener('engine:script-properties-updated', this.propertyUpdateListener);
    console.log('🔧 ScriptRuntime: Property update listener registered');
  }
  
  /**
   * Clean up property update listener
   */
  cleanupPropertyUpdateListener() {
    if (this.propertyUpdateListener) {
      document.removeEventListener('engine:script-properties-updated', this.propertyUpdateListener);
      this.propertyUpdateListener = null;
      console.log('🔧 ScriptRuntime: Property update listener removed');
    }
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
    this.cleanupPropertyUpdateListener();
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
    console.log('🎬 ScriptRuntime: ========== SCRIPT ATTACH PROCESS START ==========');
    console.log('🎬 ScriptRuntime: Target object ID:', objectId);
    console.log('🎬 ScriptRuntime: Script path:', scriptPath);
    console.log('🎬 ScriptRuntime: Runtime initialized:', this.isInitialized);
    
    if (!this.isInitialized) {
      console.error('❌ ScriptRuntime: Not initialized');
      return false;
    }
    
    try {
      console.log('📋 ScriptRuntime: Step 1 - Loading script class...');
      
      // Load the script if not already loaded
      const ScriptClass = await this.scriptLoader.loadScript(scriptPath);
      console.log('✅ ScriptRuntime: Script class loaded:', !!ScriptClass);
      console.log('🔍 ScriptRuntime: Script class type:', typeof ScriptClass);
      console.log('🔍 ScriptRuntime: Script class name:', ScriptClass?.name || 'unknown');
      
      console.log('📋 ScriptRuntime: Step 2 - Registering with script manager...');
      
      // Register with script manager
      const registerSuccess = this.scriptManager.registerScript(scriptPath, ScriptClass);
      console.log('✅ ScriptRuntime: Script registration result:', registerSuccess);
      
      console.log('📋 ScriptRuntime: Step 3 - Attaching to object...');
      
      // Attach to object
      const success = this.scriptManager.addScriptToObject(objectId, scriptPath);
      console.log('✅ ScriptRuntime: Attachment result:', success);
      
      if (success) {
        console.log('🎉 ScriptRuntime: Script attached successfully');
      } else {
        console.error('❌ ScriptRuntime: Script attachment failed');
      }
      
      console.log('🎬 ScriptRuntime: ========== SCRIPT ATTACH PROCESS END ==========');
      return success;
      
    } catch (error) {
      console.error('❌ ScriptRuntime: ========== SCRIPT ATTACH ERROR ==========');
      console.error('❌ ScriptRuntime: Script path:', scriptPath);
      console.error('❌ ScriptRuntime: Object ID:', objectId);
      console.error('❌ ScriptRuntime: Error:', error);
      console.error('❌ ScriptRuntime: Stack:', error.stack);
      console.error('❌ ScriptRuntime: ============================================');
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