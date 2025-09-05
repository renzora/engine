// Using Rust RenScript compiler via bridge API

/**
 * ScriptLoader - Handles loading and evaluation of script files
 * Supports JavaScript (.js) and RenScript (.ren) files
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
    console.log('📂 ScriptLoader: ========== SCRIPT LOAD START ==========');
    console.log('📂 ScriptLoader: Script path:', scriptPath);
    console.log('📂 ScriptLoader: Already loading?', this.loadingPromises.has(scriptPath));
    console.log('📂 ScriptLoader: Already loaded?', this.loadedScripts.has(scriptPath));
    
    // Check if already loading
    if (this.loadingPromises.has(scriptPath)) {
      console.log('⏳ ScriptLoader: Script is already loading, waiting for existing promise...');
      return this.loadingPromises.get(scriptPath);
    }
    
    // Check if already loaded
    if (this.loadedScripts.has(scriptPath)) {
      console.log('💾 ScriptLoader: Script already loaded, returning cached version');
      return this.loadedScripts.get(scriptPath).module;
    }
    
    console.log('📥 ScriptLoader: Starting fresh script load...');
    
    const loadPromise = this._loadScriptFromServer(scriptPath);
    this.loadingPromises.set(scriptPath, loadPromise);
    console.log('🔄 ScriptLoader: Added to loading promises map');
    
    try {
      console.log('⚙️ ScriptLoader: Awaiting script class from server...');
      const scriptClass = await loadPromise;
      console.log('✅ ScriptLoader: Script class received:', typeof scriptClass);
      
      this.loadedScripts.set(scriptPath, {
        module: scriptClass,
        timestamp: Date.now()
      });
      this.loadingPromises.delete(scriptPath);
      
      console.log('💾 ScriptLoader: Script cached successfully');
      console.log('📂 ScriptLoader: ========== SCRIPT LOAD SUCCESS ==========');
      return scriptClass;
      
    } catch (error) {
      this.loadingPromises.delete(scriptPath);
      console.error('❌ ScriptLoader: ========== SCRIPT LOAD ERROR ==========');
      console.error('❌ ScriptLoader: Script path:', scriptPath);
      console.error('❌ ScriptLoader: Error:', error.message);
      console.error('❌ ScriptLoader: Stack:', error.stack);
      console.error('❌ ScriptLoader: =====================================');
      throw error;
    }
  }
  
  /**
   * Load script content from the server and evaluate it
   * @private
   */
  async _loadScriptFromServer(scriptPath) {
    console.log('🌐 ScriptLoader: ========== SERVER LOAD START ==========');
    console.log('🌐 ScriptLoader: Script path:', scriptPath);
    
    // Check if we have a pre-compiled script from scene bundle
    const bulkScripts = window._sceneBundledScripts;
    if (bulkScripts && bulkScripts[scriptPath]) {
      console.log('📦 ScriptLoader: Using pre-compiled script from bundle cache:', scriptPath);
      
      const compiledScript = bulkScripts[scriptPath];
      
      // Check if compilation failed
      if (typeof compiledScript === 'object' && compiledScript.error) {
        console.error('❌ ScriptLoader: Pre-compiled script has error:', compiledScript.error);
        throw new Error(`Script compilation failed: ${compiledScript.error}`);
      }
      
      console.log('📦 ScriptLoader: Using bulk compiled script (length:', compiledScript.length, ')');
      
      // For RenScript files, use the pre-compiled JavaScript directly
      if (scriptPath.endsWith('.ren')) {
        return this._evaluateCompiledRenScript(compiledScript, scriptPath);
      } else {
        // For regular JS files, evaluate directly
        return this._evaluateJavaScript(compiledScript, scriptPath);
      }
    }
    
    console.log('🌐 ScriptLoader: No bulk cache found, requesting from server');
    
    try {
      let url;
      
      // Check if this is a RenScript file that needs compilation
      if (scriptPath.endsWith('.ren')) {
        // Extract script name for compilation endpoint
        const scriptName = scriptPath.replace(/.*\/([^\/]+)\.ren$/, '$1');
        url = `http://localhost:3001/script/${scriptName}`;
        console.log('🔥 ScriptLoader: RenScript detected, using compilation endpoint for:', scriptName);
      } else {
        // Regular JavaScript file - read directly
        console.log('📁 ScriptLoader: JavaScript file detected, getting current project...');
        const { getCurrentProject } = await import('@/api/bridge/projects');
        const projectName = getCurrentProject()?.name || 'demo';
        console.log('📁 ScriptLoader: Current project:', projectName);
        url = `http://localhost:3001/read/projects/${projectName}/${scriptPath}`;
      }
      
      console.log('🌐 ScriptLoader: Fetching from URL:', url);
      
      const response = await fetch(url);
      console.log('🌐 ScriptLoader: Response status:', response.status, response.statusText);
      
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }
      
      let scriptContent;
      
      // Handle compilation endpoint (returns JavaScript directly)
      if (scriptPath.endsWith('.ren')) {
        scriptContent = await response.text();
        console.log('🔥 ScriptLoader: Received compiled JavaScript from Rust server');
        console.log('🔥 ScriptLoader: Compiled JS length:', scriptContent?.length || 0);
        console.log('🔥 ScriptLoader: Compiled JS preview:', scriptContent?.substring(0, 100) + '...');
      } else {
        // Handle regular file reading (returns JSON response)
        const responseData = await response.json();
        console.log('🌐 ScriptLoader: Response success:', responseData.success);
        
        if (!responseData.success) {
          throw new Error(`Failed to read script: ${responseData.error || 'Unknown error'}`);
        }
        
        scriptContent = responseData.content;
      }
      console.log('🌐 ScriptLoader: Script content length:', scriptContent?.length || 0);
      console.log('🌐 ScriptLoader: Script content preview:', scriptContent?.substring(0, 100) + '...');
      
      // Check if it's a RenScript file
      if (scriptPath.endsWith('.ren')) {
        console.log('🎯 ScriptLoader: RenScript file detected, using pre-compiled JavaScript from server');
        return this._evaluateCompiledRenScript(scriptContent, scriptPath);
      } else {
        console.log('📜 ScriptLoader: JavaScript file detected, starting evaluation...');
        return this._evaluateScript(scriptContent, scriptPath);
      }
      
    } catch (error) {
      console.error('❌ ScriptLoader: Server load failed:', error.message);
      throw new Error(`Failed to load script ${scriptPath}: ${error.message}`);
    }
  }
  
  /**
   * Evaluate compiled RenScript JavaScript and return script class
   * @private
   */
  async _evaluateCompiledRenScript(compiledJS, scriptPath) {
    console.log('🎯 ScriptLoader: ========== COMPILED RENSCRIPT EVAL START ==========');
    console.log('🎯 ScriptLoader: Script path:', scriptPath);
    console.log('🎯 ScriptLoader: Compiled JS length:', compiledJS?.length || 0);
    
    try {
      // Create evaluation context
      const scriptModule = { exports: {} };
      const require = this._createRequireFunction();
      
      // The compiled JavaScript contains a createRenScript function
      // We need to make it available but not call it yet (ScriptManager will call it)
      const wrappedCode = `
        ${compiledJS}
        
        // Export the createRenScript function itself, not its result
        module.exports = createRenScript;
      `;
      
      console.log('🏗️ ScriptLoader: Creating script function wrapper...');
      
      // Execute the script
      const scriptFunction = new Function(
        'exports',
        'module', 
        'require',
        'console',
        'BABYLON',
        'scene',
        'api',
        wrappedCode
      );
      
      console.log('🚀 ScriptLoader: Executing compiled RenScript...');
      
      scriptFunction(
        scriptModule.exports,
        scriptModule,
        require,
        this._createSafeConsole(scriptPath),
        this._createBabylonAPI(),
        null, // scene will be provided by ScriptManager
        null  // api will be provided by ScriptManager
      );
      
      const ScriptClass = scriptModule.exports;
      console.log('✅ ScriptLoader: Compiled RenScript execution completed');
      console.log('🔍 ScriptLoader: Exported class type:', typeof ScriptClass);
      console.log('🔍 ScriptLoader: Exported class name:', ScriptClass?.name || 'unknown');
      
      if (typeof ScriptClass !== 'function') {
        console.error('❌ ScriptLoader: Invalid script class - expected function, got:', typeof ScriptClass);
        throw new Error('Compiled RenScript did not produce a valid script class');
      }
      
      console.log('🎯 ScriptLoader: ========== COMPILED RENSCRIPT EVAL SUCCESS ==========');
      return ScriptClass;
      
    } catch (error) {
      console.error('❌ ScriptLoader: ========== COMPILED RENSCRIPT EVAL ERROR ==========');
      console.error('❌ ScriptLoader: Script path:', scriptPath);
      console.error('❌ ScriptLoader: Error message:', error.message);
      console.error('❌ ScriptLoader: Error stack:', error.stack);
      console.error('❌ ScriptLoader: ===============================================');
      throw new Error(`Failed to evaluate compiled RenScript ${scriptPath}: ${error.message}`);
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
      
      // Check if it's a class-based script
      const classNameMatch = scriptContent.match(/export\s+default\s+class\s+(\w+)/);
      // Check if it's a functional script with export
      const functionMatch = scriptContent.match(/export\s+default\s+(\w+)/);
      // Check for inline script (object literal or simple script)
      const hasOnStartUpdate = scriptContent.includes('onStart') || scriptContent.includes('onUpdate');
      const hasReturnStatement = scriptContent.includes('return {');
      // Check for const/let/var declaration with object literal
      const constObjectMatch = scriptContent.match(/(?:const|let|var)\s+\w+\s*=\s*\{/);
      const hasConstObject = !!constObjectMatch;
      // Check for bare function declarations
      const hasBareFunction = scriptContent.match(/^function\s+(onStart|onUpdate|onDestroy)/m);
      // Check for bare method declarations (object method syntax without object wrapper)
      const hasBareMethod = scriptContent.match(/^\s*(onStart|onUpdate|onDestroy)\s*\(/m);
      
      if (classNameMatch) {
        // Class-based script
        transformedScript = transformedScript.replace(
          /export\s+default\s+class\s+(\w+)/g, 
          'class $1'
        );
        const className = classNameMatch[1];
        transformedScript += `\n\nmodule.exports = ${className};`;
      } else if (functionMatch) {
        // Functional script with export - wrap in a class automatically
        const functionName = functionMatch[1];
        transformedScript = transformedScript.replace(
          /export\s+default\s+(\w+)/g, 
          ''
        );
        
        // Wrap the function in a class
        transformedScript += `
        
class FunctionalScriptWrapper {
  constructor(scene, api) {
    this.scene = scene;
    this.api = api;
    this._scriptFunction = ${functionName};
    this._state = this._scriptFunction(scene, api) || {};
  }
  
  onStart() {
    if (this._state.onStart) {
      this._state.onStart();
    }
  }
  
  onUpdate(deltaTime) {
    if (this._state.onUpdate) {
      this._state.onUpdate(deltaTime);
    }
  }
  
  onDestroy() {
    if (this._state.onDestroy) {
      this._state.onDestroy();
    }
  }
}

module.exports = FunctionalScriptWrapper;`;
      } else if (hasOnStartUpdate) {
        // Inline script - handle different patterns
        if (hasConstObject) {
          // Script uses const/let/var name = { ... } pattern - extract the variable name
          const variableName = constObjectMatch[0].match(/(?:const|let|var)\s+(\w+)/)[1];
          transformedScript = `
function inlineScript(scene, api) {
  ${transformedScript}
  return ${variableName};
}`;
        } else if (hasBareMethod) {
          // Script has bare method declarations - wrap in object literal
          transformedScript = `
function inlineScript(scene, api) {
  return {
    ${transformedScript}
  };
}`;
        } else if (hasBareFunction) {
          // Script has bare function declarations - wrap and collect them into an object
          transformedScript = `
function inlineScript(scene, api) {
  // Wrap the script content in function scope
  ${transformedScript}
  
  const scriptObject = {};
  if (typeof onStart === 'function') scriptObject.onStart = onStart;
  if (typeof onUpdate === 'function') scriptObject.onUpdate = onUpdate;
  if (typeof onDestroy === 'function') scriptObject.onDestroy = onDestroy;
  
  return scriptObject;
}`;
        } else if (hasReturnStatement) {
          // Script has return statement - wrap in function
          transformedScript = `
function inlineScript(scene, api) {
  ${transformedScript}
}`;
        } else {
          // Script is just an object literal - wrap it in return statement
          transformedScript = `
function inlineScript(scene, api) {
  return ${transformedScript.trim()};
}`;
        }

        transformedScript += `

class InlineScriptWrapper {
  constructor(scene, api) {
    this.scene = scene;
    this.api = api;
    this._state = inlineScript(scene, api) || {};
  }
  
  onStart() {
    if (this._state.onStart) {
      this._state.onStart();
    }
  }
  
  onUpdate(deltaTime) {
    if (this._state.onUpdate) {
      this._state.onUpdate(deltaTime);
    }
  }
  
  onDestroy() {
    if (this._state.onDestroy) {
      this._state.onDestroy();
    }
  }
}

module.exports = InlineScriptWrapper;`;
      } else {
        throw new Error('Script must export a class/function or contain onStart/onUpdate methods');
      }
      
      console.log('🔧 ScriptLoader: Transformed script for', scriptPath);
      console.log('🔧 ScriptLoader: Detection results:', {
        classNameMatch: !!classNameMatch,
        functionMatch: !!functionMatch,
        hasOnStartUpdate,
        hasReturnStatement,
        hasConstObject,
        hasBareFunction: !!hasBareFunction,
        hasBareMethod: !!hasBareMethod,
        constObjectMatch: constObjectMatch ? constObjectMatch[0] : null,
        bareFunctionMatch: hasBareFunction ? hasBareFunction[0] : null,
        bareMethodMatch: hasBareMethod ? hasBareMethod[0] : null
      });
      
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
        throw new Error('Script must export a class or function');
      }
      
      console.log('🔧 ScriptLoader: Successfully evaluated script for', scriptPath);
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