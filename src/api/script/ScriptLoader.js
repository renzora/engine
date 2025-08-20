/**
 * ScriptLoader - Handles loading and evaluation of script files
 */
class ScriptLoader {
  constructor() {
    this.loadedScripts = new Map(); // scriptPath -> { module, timestamp }
    this.loadingPromises = new Map(); // scriptPath -> Promise
  }
  
  /**
   * Load a script from the server
   * @param {string} scriptPath - Path to the script file
   * @returns {Promise<Function>} Promise that resolves to the script class
   */
  async loadScript(scriptPath) {
    // Check if already loading
    if (this.loadingPromises.has(scriptPath)) {
      return this.loadingPromises.get(scriptPath);
    }
    
    // Check if already loaded
    if (this.loadedScripts.has(scriptPath)) {
      return this.loadedScripts.get(scriptPath).module;
    }
    
    console.log('🔧 ScriptLoader: Loading script', scriptPath);
    
    const loadPromise = this._loadScriptFromServer(scriptPath);
    this.loadingPromises.set(scriptPath, loadPromise);
    
    try {
      const scriptClass = await loadPromise;
      this.loadedScripts.set(scriptPath, {
        module: scriptClass,
        timestamp: Date.now()
      });
      this.loadingPromises.delete(scriptPath);
      
      console.log('🔧 ScriptLoader: Script loaded successfully', scriptPath);
      return scriptClass;
      
    } catch (error) {
      this.loadingPromises.delete(scriptPath);
      console.error('🔧 ScriptLoader: Failed to load script', scriptPath, error);
      throw error;
    }
  }
  
  /**
   * Load script content from the server and evaluate it
   * @private
   */
  async _loadScriptFromServer(scriptPath) {
    try {
      // Get current project from bridge service
      const bridgeService = await import('@/plugins/core/bridge').then(m => m.bridgeService);
      const projectName = bridgeService?.getCurrentProject()?.name || 'demo';
      const url = `http://localhost:3001/read/projects/${projectName}/assets/${scriptPath}`;
      
      console.log('🔧 ScriptLoader: Fetching script from URL:', url);
      
      const response = await fetch(url);
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }
      
      const responseData = await response.json();
      if (!responseData.success) {
        throw new Error(`Failed to read script: ${responseData.error || 'Unknown error'}`);
      }
      
      const scriptContent = responseData.content;
      return this._evaluateScript(scriptContent, scriptPath);
      
    } catch (error) {
      throw new Error(`Failed to load script ${scriptPath}: ${error.message}`);
    }
  }
  
  /**
   * Evaluate script content and extract the default export
   * @private
   */
  _evaluateScript(scriptContent, scriptPath) {
    try {
      // Transform ES6 export to CommonJS for evaluation
      let transformedScript = scriptContent;
      
      // Replace ES6 export default with module.exports
      transformedScript = transformedScript.replace(
        /export\s+default\s+class\s+(\w+)/g, 
        'class $1'
      );
      
      // Add module.exports at the end
      const classNameMatch = scriptContent.match(/export\s+default\s+class\s+(\w+)/);
      if (classNameMatch) {
        const className = classNameMatch[1];
        transformedScript += `\n\nmodule.exports = ${className};`;
      } else {
        throw new Error('Script must export a class using "export default class ClassName" syntax');
      }
      
      console.log('🔧 ScriptLoader: Transformed script for', scriptPath);
      
      // Create a safe evaluation context
      const scriptModule = { exports: {} };
      const require = this._createRequireFunction();
      
      // Create a function wrapper for the script
      const scriptFunction = new Function(
        'exports',
        'module',
        'require',
        'console',
        'BABYLON',
        transformedScript
      );
      
      // Execute the script in controlled environment
      scriptFunction(
        scriptModule.exports,
        scriptModule,
        require,
        this._createSafeConsole(scriptPath),
        this._createBabylonAPI()
      );
      
      const ScriptClass = scriptModule.exports;
      
      if (typeof ScriptClass !== 'function') {
        throw new Error('Script must export a class');
      }
      
      console.log('🔧 ScriptLoader: Successfully evaluated script class for', scriptPath);
      return ScriptClass;
      
    } catch (error) {
      throw new Error(`Failed to evaluate script ${scriptPath}: ${error.message}`);
    }
  }
  
  /**
   * Create a require function for scripts (limited functionality)
   * @private
   */
  _createRequireFunction() {
    return (moduleName) => {
      // Only allow specific safe modules
      const allowedModules = {
        'babylonjs': this._createBabylonAPI(),
        '@babylonjs/core': this._createBabylonAPI()
      };
      
      if (allowedModules[moduleName]) {
        return allowedModules[moduleName];
      }
      
      throw new Error(`Module "${moduleName}" is not available in script environment`);
    };
  }
  
  /**
   * Create a safe console object for scripts
   * @private
   */
  _createSafeConsole(scriptPath) {
    return {
      log: (...args) => console.log(`[Script:${scriptPath}]`, ...args),
      warn: (...args) => console.warn(`[Script:${scriptPath}]`, ...args),
      error: (...args) => console.error(`[Script:${scriptPath}]`, ...args),
      info: (...args) => console.info(`[Script:${scriptPath}]`, ...args)
    };
  }
  
  /**
   * Create a limited Babylon.js API for scripts
   * @private
   */
  _createBabylonAPI() {
    // Import what we need dynamically
    return {
      Vector3: () => import('@babylonjs/core/Maths/math.vector.js').then(m => m.Vector3),
      Color3: () => import('@babylonjs/core/Maths/math.color.js').then(m => m.Color3),
      // Add more as needed, but keep it limited for security
    };
  }
  
  /**
   * Reload a script (useful for development)
   * @param {string} scriptPath - Path to the script file
   * @returns {Promise<Function>} Promise that resolves to the reloaded script class
   */
  async reloadScript(scriptPath) {
    console.log('🔧 ScriptLoader: Reloading script', scriptPath);
    
    // Remove from cache
    this.loadedScripts.delete(scriptPath);
    this.loadingPromises.delete(scriptPath);
    
    // Load fresh copy
    return this.loadScript(scriptPath);
  }
  
  /**
   * Check if a script is loaded
   * @param {string} scriptPath - Path to the script file
   * @returns {boolean} True if script is loaded
   */
  isLoaded(scriptPath) {
    return this.loadedScripts.has(scriptPath);
  }
  
  /**
   * Get information about loaded scripts
   * @returns {Object} Statistics about loaded scripts
   */
  getStats() {
    return {
      loadedScripts: this.loadedScripts.size,
      currentlyLoading: this.loadingPromises.size,
      scripts: Array.from(this.loadedScripts.keys())
    };
  }
  
  /**
   * Clear all loaded scripts from cache
   */
  clearCache() {
    console.log('🔧 ScriptLoader: Clearing script cache');
    this.loadedScripts.clear();
    this.loadingPromises.clear();
  }
}

// Singleton instance
let scriptLoaderInstance = null;

/**
 * Get the global script loader instance
 * @returns {ScriptLoader} The script loader instance
 */
export function getScriptLoader() {
  if (!scriptLoaderInstance) {
    scriptLoaderInstance = new ScriptLoader();
  }
  return scriptLoaderInstance;
}

export { ScriptLoader };