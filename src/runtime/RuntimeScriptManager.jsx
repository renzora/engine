/**
 * RuntimeScriptManager - Minimal script execution for exported projects
 * Runs compiled scripts without bridge dependencies
 */
export class RuntimeScriptManager {
  constructor(scene) {
    this.scene = scene;
    this.scriptInstances = new Map(); // objectId -> script instances
    this.loadedScripts = new Map(); // scriptName -> script class
    this.isRunning = false;
    this.updateObserver = null;
  }

  /**
   * Load project scripts from bundle data
   * @param {Object} scriptsData - Scripts from project bundle
   */
  async loadProjectScripts(scriptsData) {
    try {
      // Load all project scripts
      
      for (const [scriptName, scriptData] of Object.entries(scriptsData)) {
        await this.loadScript(scriptName, scriptData);
      }
      
      // Scripts loaded successfully
      
    } catch (error) {
      console.error('❌ RuntimeScriptManager: Script loading failed:', error);
      throw error;
    }
  }

  /**
   * Load a single script from compiled code
   * @param {string} scriptName - Name of the script
   * @param {Object} scriptData - Script data containing compiled code
   */
  async loadScript(scriptName, scriptData) {
    try {
      // Load individual script
      
      // Create a function from the compiled code
      const scriptFunction = new Function(
        'scene',
        'console',
        `
        ${scriptData.code}
        return createRenScript;
        `
      );
      
      // Execute to get the script class constructor
      const ScriptClass = scriptFunction(
        this.scene,
        this.createSafeConsole(scriptName)
      );
      
      if (typeof ScriptClass !== 'function') {
        throw new Error(`Script ${scriptName} did not return a valid constructor`);
      }
      
      this.loadedScripts.set(scriptName, ScriptClass);
      // Script compilation complete
      
    } catch (error) {
      console.error('❌ RuntimeScriptManager: Failed to load script:', scriptName, error);
      throw error;
    }
  }

  /**
   * Attach a script to an object
   * @param {string} objectId - ID of the target object
   * @param {string} scriptName - Name of the script to attach
   * @returns {boolean} Success status
   */
  attachScript(objectId, scriptName) {
    try {
      // Attach script to object
      
      const ScriptClass = this.loadedScripts.get(scriptName);
      if (!ScriptClass) {
        console.error('❌ Script not found:', scriptName);
        return false;
      }
      
      // Find the target object
      const targetObject = this.findObject(objectId);
      if (!targetObject) {
        console.error('❌ Object not found:', objectId);
        return false;
      }
      
      // Create embedded API for this object
      const embeddedAPI = this.createEmbeddedAPI(targetObject);
      
      // Create script instance
      const ScriptConstructor = ScriptClass(this.scene, embeddedAPI);
      const scriptInstance = new ScriptConstructor();
      
      // Store references
      scriptInstance._objectId = objectId;
      scriptInstance._scriptName = scriptName;
      scriptInstance._babylonObject = targetObject;
      
      // Add to active scripts
      if (!this.scriptInstances.has(objectId)) {
        this.scriptInstances.set(objectId, new Set());
      }
      this.scriptInstances.get(objectId).add(scriptInstance);
      
      // Call onStart if available
      if (typeof scriptInstance.onStart === 'function') {
        scriptInstance.onStart();
      }
      
      // Script attachment complete
      return true;
      
    } catch (error) {
      console.error('❌ RuntimeScriptManager: Script attachment failed:', error);
      return false;
    }
  }

  /**
   * Start script execution
   */
  start() {
    if (this.isRunning) return;
    
    // Start script execution loop
    this.isRunning = true;
    
    // Register update loop
    this.updateObserver = this.scene.onBeforeRenderObservable.add(() => {
      this.update();
    });
  }

  /**
   * Stop script execution
   */
  stop() {
    if (!this.isRunning) return;
    
    // Stop script execution loop
    this.isRunning = false;
    
    if (this.updateObserver) {
      this.scene.onBeforeRenderObservable.remove(this.updateObserver);
      this.updateObserver = null;
    }
  }

  /**
   * Update all active scripts
   */
  update() {
    if (!this.isRunning) return;
    
    const deltaTime = this.scene.getEngine().getDeltaTime() / 1000; // Convert to seconds
    
    this.scriptInstances.forEach((scripts) => {
      scripts.forEach((script) => {
        if (typeof script.onUpdate === 'function') {
          try {
            script.onUpdate(deltaTime);
          } catch (error) {
            console.error('❌ RuntimeScriptManager: Script update error:', script._scriptName, error);
            // Continue execution for other scripts
          }
        }
      });
    });
  }

  /**
   * Find object in scene by ID
   * @param {string} objectId - Object ID to find
   * @returns {Object|null} Found object or null
   */
  findObject(objectId) {
    // Check meshes
    for (const mesh of this.scene.meshes) {
      if (mesh.uniqueId === objectId || mesh.name === objectId) {
        return mesh;
      }
    }
    
    // Check transform nodes
    for (const node of this.scene.transformNodes) {
      if (node.uniqueId === objectId || node.name === objectId) {
        return node;
      }
    }
    
    // Check lights and cameras
    for (const light of this.scene.lights) {
      if (light.uniqueId === objectId || light.name === objectId) {
        return light;
      }
    }
    
    for (const camera of this.scene.cameras) {
      if (camera.uniqueId === objectId || camera.name === objectId) {
        return camera;
      }
    }
    
    return null;
  }

  /**
   * Create embedded API for runtime scripts
   * @param {Object} babylonObject - Target Babylon.js object
   * @returns {Object} Embedded API object
   */
  createEmbeddedAPI(babylonObject) {
    return {
      // Core API
      log: (...args) => console.log('[RenScript]', ...args),
      time: () => performance.now() / 1000,
      
      // Transform API
      getPosition: () => babylonObject.position ? {
        x: babylonObject.position.x,
        y: babylonObject.position.y,
        z: babylonObject.position.z
      } : { x: 0, y: 0, z: 0 },
      
      setPosition: (x, y, z) => {
        if (!babylonObject.position) return;
        
        if (typeof x === 'object') {
          babylonObject.position.x = x.x || 0;
          babylonObject.position.y = x.y || 0;
          babylonObject.position.z = x.z || 0;
        } else {
          babylonObject.position.x = x || 0;
          babylonObject.position.y = y || 0;
          babylonObject.position.z = z || 0;
        }
      },
      
      setRotation: (x, y, z) => {
        if (!babylonObject.rotation) return;
        
        if (typeof x === 'object') {
          babylonObject.rotation.x = x.x || 0;
          babylonObject.rotation.y = x.y || 0;
          babylonObject.rotation.z = x.z || 0;
        } else {
          babylonObject.rotation.x = x || 0;
          babylonObject.rotation.y = y || 0;
          babylonObject.rotation.z = z || 0;
        }
      },
      
      // Scene Query API
      findObjectByName: (name) => {
        return this.scene.getMeshByName(name) || 
               this.scene.getNodeByName(name) ||
               this.scene.getLightByName(name) ||
               this.scene.getCameraByName(name);
      },
      
      // Math utilities
      random: () => Math.random(),
      randomRange: (min, max) => Math.random() * (max - min) + min,
      clamp: (value, min, max) => Math.max(min, Math.min(max, value)),
      lerp: (a, b, t) => a + (b - a) * t,
      distance: (a, b) => {
        const dx = a.x - b.x;
        const dy = a.y - b.y;
        const dz = (a.z || 0) - (b.z || 0);
        return Math.sqrt(dx * dx + dy * dy + dz * dz);
      },
      
      // Input API (basic)
      isKeyPressed: (key) => {
        // TODO: Implement basic keyboard input tracking
        return false;
      },
      
      getMousePosition: () => {
        // TODO: Implement mouse position tracking
        return { x: 0, y: 0 };
      },
      
      // References
      _object: babylonObject,
      _scene: this.scene
    };
  }

  /**
   * Create safe console for scripts
   * @param {string} scriptName - Name of the script
   * @returns {Object} Safe console object
   */
  createSafeConsole(scriptName) {
    return {
      log: (...args) => console.log(`[${scriptName}]`, ...args),
      warn: (...args) => console.warn(`[${scriptName}]`, ...args),
      error: (...args) => console.error(`[${scriptName}]`, ...args),
      info: (...args) => console.info(`[${scriptName}]`, ...args)
    };
  }

  /**
   * Pause rendering
   */
  pause() {
    this.engine.renderEvenInBackground = false;
  }

  /**
   * Resume rendering
   */
  resume() {
    this.engine.renderEvenInBackground = true;
  }

  /**
   * Dispose of renderer resources
   */
  dispose() {
    // Dispose of script manager resources
    
    if (this.scene) {
      this.scene.dispose();
      this.scene = null;
    }
    
    if (this.engine) {
      this.engine.dispose();
      this.engine = null;
    }
    
    this.camera = null;
    this.canvas = null;
  }
}