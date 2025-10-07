import { ScriptManager } from './ScriptManager.js';
import { getScriptLoader } from './ScriptLoader.js';
import { editorStore } from '@/layout/stores/EditorStore.jsx';

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
    this.executionToggleListener = null;
  }
  
  /**
   * Initialize the script runtime with a Babylon.js scene
   * @param {Scene} scene - The Babylon.js scene
   */
  initialize(scene) {
    // Initializing script runtime
    // Scene provided for initialization
    // Checking initialization status
    
    if (this.isInitialized) {
      console.warn('🔧 ScriptRuntime: Already initialized');
      return;
    }
    
    this.scene = scene;
    
    // Creating ScriptManager
    try {
      this.scriptManager = new ScriptManager(scene);
      // ScriptManager created successfully
    } catch (error) {
      console.error('❌ ScriptRuntime: Failed to create ScriptManager:', error);
      throw error;
    }
    
    this.isInitialized = true;
    
    // Runtime initialized with scene
    
    // Set up event listener for live property updates
    // Setting up property update listener
    this.setupPropertyUpdateListener();
    
    // Set up event listener for script execution toggle
    this.setupExecutionToggleListener();
    
    // Start the script manager based on editor state
    // Check if scripts should be playing from editor store
    if (editorStore.scripts.isPlaying) {
      // Starting script manager
      this.start();
    } else {
      console.log('🔧 ScriptRuntime: Scripts set to paused by default');
    }
  }
  
  /**
   * Set up event listener for live script property updates
   */
  setupPropertyUpdateListener() {
    this.propertyUpdateListener = (event) => {
      const { scriptPath, properties, propertyChanges } = event.detail;
      
      if (!scriptPath || !properties || !propertyChanges) return;
      
      // Received property update event
      
      // Update script properties using the script manager
      this.scriptManager.updateScriptProperties(scriptPath, properties, propertyChanges);
    };
    
    document.addEventListener('engine:script-properties-updated', this.propertyUpdateListener);
    // Property update listener registered
  }

  /**
   * Set up event listener for script execution toggle
   */
  setupExecutionToggleListener() {
    this.executionToggleListener = (event) => {
      const { isPlaying } = event.detail;
      
      if (isPlaying) {
        console.log('🎯 ScriptRuntime: Starting script execution');
        this.start();
      } else {
        console.log('⏸️ ScriptRuntime: Pausing script execution');
        this.pause();
      }
    };
    
    document.addEventListener('engine:script-execution-toggle', this.executionToggleListener);
    // Script execution toggle listener registered
  }
  
  /**
   * Clean up property update listener
   */
  cleanupPropertyUpdateListener() {
    if (this.propertyUpdateListener) {
      document.removeEventListener('engine:script-properties-updated', this.propertyUpdateListener);
      this.propertyUpdateListener = null;
      // Property update listener removed
    }
  }

  /**
   * Clean up execution toggle listener
   */
  cleanupExecutionToggleListener() {
    if (this.executionToggleListener) {
      document.removeEventListener('engine:script-execution-toggle', this.executionToggleListener);
      this.executionToggleListener = null;
      // Script execution toggle listener removed
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
    // Script runtime started
    return true;
  }
  
  /**
   * Pause script execution (keeps scripts attached)
   */
  pause() {
    if (!this.isInitialized) return;
    
    this.scriptManager.pause();
    // Script runtime paused
  }
  
  /**
   * Stop script execution
   */
  stop() {
    if (!this.isInitialized) return;
    
    this.scriptManager.stop();
    // Script runtime stopped
  }
  
  /**
   * Shutdown the runtime and clean up resources
   */
  shutdown() {
    if (!this.isInitialized) return;
    
    this.stop();
    this.cleanupPropertyUpdateListener();
    this.cleanupExecutionToggleListener();
    this.scriptManager = null;
    this.scene = null;
    this.isInitialized = false;
    
    // Script runtime shutdown complete
  }
  
  /**
   * Attach a script to an object
   * @param {string} objectId - ID of the object
   * @param {string} scriptPath - Path to the script file
   * @param {boolean} deferStart - If true, don't call onStart() immediately (for property restoration)
   * @returns {Promise<boolean>} Success status
   */
  async attachScript(objectId, scriptPath, deferStart = false) {
    // Starting script attach process
    // Attaching script to object
    // Processing script path
    // Checking runtime initialization
    
    if (!this.isInitialized) {
      console.error('❌ ScriptRuntime: Not initialized');
      return false;
    }
    
    try {
      // Loading script class
      
      // Load the script if not already loaded
      const ScriptClass = await this.scriptLoader.loadScript(scriptPath);
      // Script class loaded successfully
      // Checking script class type
      // Got script class name
      
      // Registering with script manager
      
      // Register with script manager
      this.scriptManager.registerScript(scriptPath, ScriptClass);
      // Script registration completed
      
      // Attaching script to object
      
      // Attach to object
      const success = this.scriptManager.addScriptToObject(objectId, scriptPath, deferStart);
      // Script attachment completed
      
      if (success) {
        // Script attached successfully
      } else {
        console.error('❌ ScriptRuntime: Script attachment failed');
      }
      
      // Script attach process completed
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
    
    // Detaching script from object
    
    const success = this.scriptManager.removeScriptFromObject(objectId, scriptPath);
    
    if (success) {
      // Script detached successfully
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
    
    // Detaching all scripts from object
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
      // Reloading script
      
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
      
      // Script reloaded successfully
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
    // Paused script on object
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
    // Resumed script on object
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
   * Start a script instance (call onStart if available)
   * @param {Object} scriptInstance - Script instance to start
   */
  startScriptInstance(scriptInstance) {
    if (!this.isInitialized) {
      console.error('🔧 ScriptRuntime: Not initialized');
      return;
    }
    
    this.scriptManager.startScriptInstance(scriptInstance);
  }
  
  /**
   * Enable debug mode for more verbose logging
   * @param {boolean} enabled - Whether to enable debug mode
   */
  setDebugMode() {
    // Could be used to enable more verbose logging in the future
    // Debug mode toggle
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