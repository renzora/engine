// API Modules  
import { CoreAPI } from './modules/CoreAPI.js';
import { MaterialAPI } from './modules/MaterialAPI.js';
import { MeshAPI } from './modules/MeshAPI.js';
import { AnimationAPI } from './modules/AnimationAPI.js';
import { SceneAPI } from './modules/SceneAPI.js';
import { PhysicsAPI } from './modules/PhysicsAPI.js';
import { InputAPI } from './modules/InputAPI.js';
import { TextureAPI } from './modules/TextureAPI.js';
import { ParticleAPI } from './modules/ParticleAPI.js';
import { AudioAPI } from './modules/AudioAPI.js';
import { GUIAPI } from './modules/GUIAPI.js';
import { PostProcessAPI } from './modules/PostProcessAPI.js';
import { XRAPI } from './modules/XRAPI.js';
import { DebugAPI } from './modules/DebugAPI.js';
import { AssetAPI } from './modules/AssetAPI.js';
import { UtilityAPI } from './modules/UtilityAPI.js';
import { CameraAPI } from './modules/CameraAPI.js';
import { LightingAPI } from './modules/LightingAPI.js';
import { EnvironmentAPI } from './modules/EnvironmentAPI.js';
import { DayNightAPI } from './modules/DayNightAPI.js';
import { ShadowAPI } from './modules/ShadowAPI.js';
import { TransformAPI } from './modules/TransformAPI.js';
import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';

/**
 * ScriptManager - Manages script execution and lifecycle for Babylon.js objects
 */
class ScriptManager {
  constructor(scene) {
    this.scene = scene;
    this.activeScripts = new Map(); // objectId -> Set of script instances
    this.scriptClasses = new Map(); // scriptPath -> script class constructor
    this.pausedScripts = new Map(); // objectId -> Set of paused script paths
    this.isRunning = false;
    this.updateObserver = null;
    
    this.bindMethods();
  }
  
  bindMethods() {
    this.update = this.update.bind(this);
  }
  
  /**
   * Initialize API modules for a script instance
   */
  initializeAPIModules(babylonObject) {
    try {
      // Initialize all API modules
      const apiModules = {
        core: new CoreAPI(this.scene, babylonObject),
        material: new MaterialAPI(this.scene),
        mesh: new MeshAPI(this.scene),
        animation: new AnimationAPI(this.scene, babylonObject),
        sceneQuery: new SceneAPI(this.scene, babylonObject),
        physics: new PhysicsAPI(this.scene, babylonObject),
        input: new InputAPI(this.scene, babylonObject),
        texture: new TextureAPI(this.scene),
        particle: new ParticleAPI(this.scene),
        audio: new AudioAPI(this.scene),
        gui: new GUIAPI(this.scene),
        postProcess: new PostProcessAPI(this.scene),
        xr: new XRAPI(this.scene),
        debug: new DebugAPI(this.scene),
        asset: new AssetAPI(this.scene),
        utility: new UtilityAPI(this.scene),
        camera: new CameraAPI(this.scene, babylonObject),
        lighting: new LightingAPI(this.scene, babylonObject),
        environment: new EnvironmentAPI(this.scene, babylonObject),
        daynight: new DayNightAPI(this.scene, babylonObject),
        shadow: new ShadowAPI(this.scene, babylonObject),
        transform: new TransformAPI(this.scene, babylonObject)
      };
      
      return apiModules;
    } catch (error) {
      console.error('❌ ScriptManager: Failed to initialize API modules:', error);
      throw error;
    }
  }
  
  /**
   * Bind all API module methods to a script instance for direct access
   */
  bindAPIMethodsToScript(scriptInstance, apiModules) {
    
    // Bind all methods from each module to the script instance
    Object.entries(apiModules).forEach(([moduleName, moduleInstance]) => {
      const methods = Object.getOwnPropertyNames(Object.getPrototypeOf(moduleInstance))
        .filter(name => name !== 'constructor' && typeof moduleInstance[name] === 'function');
        
      methods.forEach(methodName => {
        if (!scriptInstance[methodName]) { // Don't override existing methods
          scriptInstance[methodName] = moduleInstance[methodName].bind(moduleInstance);
        }
      });
    });
    
    // Store module references for cleanup
    scriptInstance._apiModules = apiModules;
    
    // Add property management methods
    scriptInstance.getScriptPropertiesBySection = () => {
      // Getting script properties by section
      
      const sections = {};
      if (scriptInstance._scriptProperties) {
        if (Array.isArray(scriptInstance._scriptProperties)) {
          // Processing properties array
          scriptInstance._scriptProperties.forEach(property => {
            const section = property.section || 'General';
            if (!sections[section]) {
              sections[section] = [];
            }
            sections[section].push({
              name: property.name,
              type: property.type,
              defaultValue: property.defaultValue,
              min: property.min,
              max: property.max,
              options: property.options,
              description: property.description
            });
          });
        } else {
          // Script properties not in expected array format
        }
      } else {
        // No script properties found on instance
      }
      
      // Script properties organized into sections
      return sections;
    };
    
    // Add all missing property management methods from ScriptAPI
    scriptInstance.getScriptProperties = () => {
      // Getting script properties
      const props = [];
      if (scriptInstance._scriptProperties) {
        if (Array.isArray(scriptInstance._scriptProperties)) {
          // Processing properties array
          return scriptInstance._scriptProperties.map(property => ({
            name: property.name,
            type: property.type,
            defaultValue: property.defaultValue,
            min: property.min,
            max: property.max,
            options: property.options,
            description: property.description,
            section: property.section || 'General'
          }));
        } else {
          // Converting properties from Map format
          scriptInstance._scriptProperties.forEach((property, key) => {
            props.push({
              name: key,
              type: property.type,
              defaultValue: property.default,
              min: property.min,
              max: property.max,
              options: property.options,
              description: property.description,
              section: property.section || 'General'
            });
          });
        }
      }
      // Final properties list prepared
      return props;
    };
    
    scriptInstance.setScriptProperty = (propertyName, value) => {
      // Setting script property value
      
      const hasProperty = Array.isArray(scriptInstance._scriptProperties) ? 
        scriptInstance._scriptProperties.some(prop => prop.name === propertyName) :
        scriptInstance._scriptProperties && scriptInstance._scriptProperties.has(propertyName);
        
      if (hasProperty) {
        // Property found in script definition
        scriptInstance[propertyName] = value;
        
        // Also update objectPropertiesStore to keep it in sync
        if (scriptInstance._objectId && scriptInstance._scriptPath) {
          this.updateObjectPropertiesStore(scriptInstance._objectId, scriptInstance._scriptPath, propertyName, value);
        }
        
        // Check if this property has triggerOnce and trigger onOnce if it does
        if (Array.isArray(scriptInstance._scriptProperties)) {
          const property = scriptInstance._scriptProperties.find(prop => prop.name === propertyName);
          // Found property definition with triggerOnce
          if (property && property.triggerOnce === true) {
            // Property has triggerOnce flag, calling onOnce
            if (typeof scriptInstance.onOnce === 'function') {
              try {
                // Calling onOnce method
                scriptInstance.onOnce();
                // onOnce method called successfully
              } catch (error) {
                console.error(`Error calling onOnce for property ${propertyName}:`, error);
              }
            } else {
              // No onOnce method found on script instance
            }
          }
        }
        
        return true;
      } else {
        // Property not found in script definition
      }
      return false;
    };
    
    scriptInstance.updateScriptProperty = (propertyName, value) => {
      return this.updateScriptProperty(scriptInstance, propertyName, value);
    };
    
  }
  
  // === SCRIPT PROPERTY METHODS ===
  
  /**
   * Update script property and trigger onOnce if necessary
   * @param {Object} scriptInstance - Script instance to update
   * @param {string} propertyName - Name of the property
   * @param {*} value - New value
   * @returns {boolean} True if property was updated
   */
  updateScriptProperty(scriptInstance, propertyName, value) {
    // Updating script property
    
    if (!scriptInstance || !scriptInstance._apiModules) {
      console.error('❌ Invalid script instance for property update');
      return false;
    }
    
    const hasProperty = Array.isArray(scriptInstance._scriptProperties) ? 
      scriptInstance._scriptProperties.some(prop => prop.name === propertyName) :
      scriptInstance._scriptProperties && scriptInstance._scriptProperties.has(propertyName);
      
    if (hasProperty) {
      console.log(`🔧 Property ${propertyName} found in script properties`);
      scriptInstance[propertyName] = value;
      
      console.log(`🔧 Updated script instance property: ${propertyName}`);
      
      // Check if this property has triggerOnce: true and trigger onOnce if it does
      if (Array.isArray(scriptInstance._scriptProperties)) {
        const property = scriptInstance._scriptProperties.find(prop => prop.name === propertyName);
        console.log(`🔧 Found property definition:`, property);
        if (property && property.triggerOnce === true) {
          console.log(`🔄 Property ${propertyName} has triggerOnce: true, triggering onOnce`);
          if (scriptInstance && typeof scriptInstance.onOnce === 'function') {
            try {
              console.log(`🔄 Calling onOnce() method...`);
              scriptInstance.onOnce();
              console.log(`✅ onOnce() called successfully`);
            } catch (error) {
              console.error(`Error calling onOnce for property ${propertyName}:`, error);
            }
          } else {
            console.log(`❌ No onOnce method found on script instance`);
          }
        } else {
          console.log(`🔧 Property ${propertyName} does not have triggerOnce: true (triggerOnce = ${property?.triggerOnce})`);
        }
      }
      
      return true;
    } else {
      console.log(`❌ Property ${propertyName} not found in script properties`);
    }
    return false;
  }
  
  /**
   * Add a dynamic property to a script instance
   * @param {Object} scriptInstance - Script instance to add property to
   * @param {string} name - Property name
   * @param {string} type - Property type
   * @param {Object} options - Property options
   * @returns {boolean} True if property was added
   */
  addDynamicProperty(scriptInstance, name, type, options = {}) {
    console.log(`🔧 Adding dynamic property: ${name} (${type})`);
    
    if (!scriptInstance) {
      console.error('❌ Invalid script instance for dynamic property addition');
      return false;
    }
    
    if (!scriptInstance._scriptProperties) {
      scriptInstance._scriptProperties = [];
    }
    
    const property = {
      name: name,
      type: type,
      section: options.section || 'Dynamic',
      defaultValue: options.default || (type === 'boolean' ? false : type === 'select' ? 'none' : 0),
      min: options.min || null,
      max: options.max || null,
      options: options.options || (type === 'select' ? ['none'] : null),
      description: options.description || `Dynamic ${type} property`,
      triggerOnce: options.once || false
    };
    
    scriptInstance._scriptProperties.push(property);
    
    // Initialize the property value
    scriptInstance[name] = property.defaultValue;
    
    // Trigger UI update
    this.updateScriptPropertyMetadata(scriptInstance);
    return true;
  }
  
  /**
   * Update property options for a script instance
   * @param {Object} scriptInstance - Script instance to update
   * @param {string} propertyName - Property name
   * @param {Array} newOptions - New options array
   * @returns {boolean} True if options were updated
   */
  updatePropertyOptions(scriptInstance, propertyName, newOptions) {
    console.log(`🔧 Updating property options for: ${propertyName}`, newOptions);
    
    if (!scriptInstance || !scriptInstance._scriptProperties) {
      console.error('❌ Invalid script instance for property options update');
      return false;
    }
    
    // Find and update the property
    const property = scriptInstance._scriptProperties.find(p => p.name === propertyName);
    if (property && property.type === 'select') {
      property.options = ['none', ...newOptions];
      this.updateScriptPropertyMetadata(scriptInstance);
      return true;
    }
    return false;
  }
  
  /**
   * Remove a dynamic property from a script instance
   * @param {Object} scriptInstance - Script instance to remove property from
   * @param {string} propertyName - Property name
   * @returns {boolean} True if property was removed
   */
  removeDynamicProperty(scriptInstance, propertyName) {
    console.log(`🔧 Removing dynamic property: ${propertyName}`);
    
    if (!scriptInstance || !scriptInstance._scriptProperties) {
      console.error('❌ Invalid script instance for dynamic property removal');
      return false;
    }
    
    const index = scriptInstance._scriptProperties.findIndex(p => p.name === propertyName);
    if (index >= 0) {
      scriptInstance._scriptProperties.splice(index, 1);
      delete scriptInstance[propertyName];
      this.updateScriptPropertyMetadata(scriptInstance);
      return true;
    }
    return false;
  }
  
  /**
   * Get a property value from a script instance
   * @param {Object} scriptInstance - Script instance
   * @param {string} propertyName - Property name
   * @returns {*} Property value or undefined
   */
  getPropertyValue(scriptInstance, propertyName) {
    return scriptInstance ? scriptInstance[propertyName] : undefined;
  }
  
  /**
   * Set a property value on a script instance
   * @param {Object} scriptInstance - Script instance
   * @param {string} propertyName - Property name
   * @param {*} value - New value
   * @returns {boolean} True if property was set
   */
  setPropertyValue(scriptInstance, propertyName, value) {
    if (scriptInstance) {
      scriptInstance[propertyName] = value;
      return this.updateScriptProperty(scriptInstance, propertyName, value);
    }
    return false;
  }
  
  /**
   * Update script property metadata and trigger UI refresh
   * @param {Object} scriptInstance - Script instance
   */
  updateScriptPropertyMetadata(scriptInstance) {
    // Trigger a UI refresh for script properties
    if (scriptInstance && scriptInstance._babylonObject && 
        scriptInstance._babylonObject.metadata && scriptInstance._babylonObject.metadata.entityId) {
      const event = new CustomEvent('engine:script-properties-updated', {
        detail: { entityId: scriptInstance._babylonObject.metadata.entityId }
      });
      document.dispatchEvent(event);
    }
  }
  
  /**
   * Update delta time for a script instance's API modules
   * @param {Object} scriptInstance - Script instance
   * @param {number} deltaTime - Delta time in seconds
   */
  _updateDeltaTime(scriptInstance, deltaTime) {
    if (scriptInstance && scriptInstance._apiModules && scriptInstance._apiModules.core) {
      scriptInstance._apiModules.core._updateDeltaTime(deltaTime);
    }
  }
  
  /**
   * Start the script manager and begin executing scripts
   */
  start() {
    if (this.isRunning) return;
    
    this.isRunning = true;
    // Starting script execution
    
    // Refresh scene reference from global if available
    if (window._cleanBabylonScene) {
      this.scene = window._cleanBabylonScene;
      // Refreshed scene reference from global
    }
    
    // Scene availability check
    // Scene render observable check
    
    // Ensure we have a valid scene
    if (!this.scene || !this.scene.onBeforeRenderObservable) {
      console.error('🔧 ScriptManager: No valid scene available for restart');
      this.isRunning = false;
      return;
    }
    
    // Update script API references to use the current scene
    this.activeScripts.forEach((scripts, objectId) => {
      scripts.forEach(script => {
        if (script._scriptAPI) {
          script._scriptAPI.scene = this.scene;
          // Find the current babylon object reference
          const babylonObject = this.findBabylonObject(objectId);
          if (babylonObject) {
            script._scriptAPI.babylonObject = babylonObject;
            script._babylonObject = babylonObject;
          }
        }
      });
    });
    
    // Register for scene updates
    this.updateObserver = this.scene.onBeforeRenderObservable.add(() => {
      this.update();
    });
    
    // Script update observer registered
  }
  
  /**
   * Pause script execution (keeps scripts attached)
   */
  pause() {
    if (!this.isRunning) return;
    
    this.isRunning = false;
    // Pausing script execution
    
    // Dispose update observer only
    if (this.updateObserver) {
      this.scene?.onBeforeRenderObservable?.remove(this.updateObserver);
      this.updateObserver = null;
    }
  }
  
  /**
   * Stop the script manager and clean up all scripts
   */
  stop() {
    if (!this.isRunning) return;
    
    this.isRunning = false;
    // Stopping script execution
    
    // Dispose update observer
    if (this.updateObserver) {
      this.scene?.onBeforeRenderObservable?.remove(this.updateObserver);
      this.updateObserver = null;
    }
    
    // Clean up all scripts
    this.activeScripts.forEach((scripts, objectId) => {
      this.removeAllScriptsFromObject(objectId);
    });
    
    this.activeScripts.clear();
    this.scriptClasses.clear();
    this.pausedScripts.clear();
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
    // Registered script class
    return true;
  }
  
  /**
   * Add a script to an object
   * @param {string} objectId - ID of the object
   * @param {string} scriptPath - Path to the script file
   * @param {boolean} deferStart - If true, don't call onStart() immediately (for property restoration)
   */
  addScriptToObject(objectId, scriptPath, deferStart = false) {
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
    
    // Validate script type matches object type
    if (ScriptClass._scriptObjectType && ScriptClass._scriptObjectType !== 'script') {
      const objectClassName = babylonObject.getClassName ? babylonObject.getClassName().toLowerCase() : '';
      const scriptObjectType = ScriptClass._scriptObjectType.toLowerCase();
      
      // Check if types match
      let typeMatches = false;
      if (scriptObjectType === 'camera' && objectClassName.includes('camera')) {
        typeMatches = true;
      } else if (scriptObjectType === 'light' && objectClassName.includes('light')) {
        typeMatches = true;
      } else if (scriptObjectType === 'mesh' && (objectClassName.includes('mesh') || objectClassName === 'mesh')) {
        typeMatches = true;
      } else if (scriptObjectType === 'transform' && objectClassName === 'transformnode') {
        typeMatches = true;
      } else if (scriptObjectType === 'scene' && objectClassName === 'scene') {
        typeMatches = true;
      }
      
      if (!typeMatches) {
        const friendlyObjectType = objectClassName.replace('camera', 'Camera')
          .replace('light', 'Light')
          .replace('mesh', 'Mesh')
          .replace('transformnode', 'Transform Node');
        const friendlyScriptType = scriptObjectType.charAt(0).toUpperCase() + scriptObjectType.slice(1);
        
        console.error(`🔧 ScriptManager: Type mismatch! "${scriptPath}" is a ${friendlyScriptType} script but "${babylonObject.name}" is a ${friendlyObjectType}`);
        console.error(`💡 Tip: ${friendlyScriptType} scripts can only be attached to ${friendlyScriptType} objects`);
        return false;
      }
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
      // Initialize API modules
      const apiModules = this.initializeAPIModules(babylonObject);
      
      // Create API context object for RenScript
      const apiContext = {};
      
      // Bind all API methods to the context object
      Object.entries(apiModules).forEach(([moduleName, moduleInstance]) => {
        const methods = Object.getOwnPropertyNames(Object.getPrototypeOf(moduleInstance))
          .filter(name => name !== 'constructor' && typeof moduleInstance[name] === 'function');
          
        methods.forEach(methodName => {
          if (!apiContext[methodName]) { // Don't override existing methods
            apiContext[methodName] = moduleInstance[methodName].bind(moduleInstance);
          }
        });
      });
      
      // For RenScript compiled scripts, ScriptClass is the createRenScript function
      // We need to call it to get the actual constructor
      let ActualScriptClass;
      if (ScriptClass.name === 'createRenScript') {
        // RenScript detected, calling constructor
        ActualScriptClass = ScriptClass(this.scene, apiContext);
      } else {
        ActualScriptClass = ScriptClass;
      }
      
      // Create script instance 
      const scriptInstance = new ActualScriptClass();
      scriptInstance._scriptPath = scriptPath;
      
      // Bind API methods to script instance for direct access
      this.bindAPIMethodsToScript(scriptInstance, apiModules);
      scriptInstance._objectId = objectId;
      scriptInstance._babylonObject = babylonObject;
      
      // Create _scriptAPI object that ObjectProperties.jsx expects
      scriptInstance._scriptAPI = scriptInstance;
      
      // Initialize script properties if available
      if (scriptInstance._scriptProperties) {
        // Ensure properties have the correct format (transform if needed)
        const properties = scriptInstance._scriptProperties.map(prop => {
          // Handle both old format (propType) and new format (type)
          if (prop.propType && !prop.type) {
            return {
              ...prop,
              type: prop.propType
            };
          }
          return prop;
        });
        
        scriptInstance._scriptProperties = properties;
        
        // Initialize property values
        properties.forEach(property => {
          if (property.defaultValue !== undefined) {
            scriptInstance[property.name] = property.defaultValue;
          }
        });
      }
      
      // Initialize script set for this object if needed
      if (!this.activeScripts.has(objectId)) {
        this.activeScripts.set(objectId, new Set());
      }
      
      // Add to active scripts
      this.activeScripts.get(objectId).add(scriptInstance);
      
      // Call onStart if available and not deferred
      if (!deferStart && typeof scriptInstance.onStart === 'function') {
        scriptInstance.onStart();
      }
      
      // Call onOnce initially to apply any preset properties
      if (typeof scriptInstance.onOnce === 'function') {
        // Calling initial onOnce for script
        scriptInstance.onOnce();
      }
      
      // Script attached successfully
      return true;
      
    } catch (error) {
      console.error('❌ =================== SCRIPT INSTANTIATION ERROR ===================');
      console.error(`🔧 Script: ${scriptPath}`);
      console.error(`🎯 Object: ${objectId} (${babylonObject?.name || 'unknown'})`);
      console.error(`💥 Error: ${error.name}: ${error.message}`);
      
      // Check for missing method in instantiation
      if (error instanceof ReferenceError && error.message.includes('is not defined')) {
        const missingMethod = error.message.match(/(\w+) is not defined/)?.[1];
        if (missingMethod) {
          console.error(`🔍 MISSING METHOD DETECTED: ${missingMethod}`);
          console.error(`💡 This method is needed during script initialization`);
          
          // Create a temporary script instance for diagnosis  
          const tempAPIModules = this.initializeAPIModules(babylonObject);
          const tempAPIContext = {};
          
          // Bind API methods to temp context
          Object.entries(tempAPIModules).forEach(([moduleName, moduleInstance]) => {
            const methods = Object.getOwnPropertyNames(Object.getPrototypeOf(moduleInstance))
              .filter(name => name !== 'constructor' && typeof moduleInstance[name] === 'function');
              
            methods.forEach(methodName => {
              if (!tempAPIContext[methodName]) {
                tempAPIContext[methodName] = moduleInstance[methodName].bind(moduleInstance);
              }
            });
          });
          
          const tempScript = { ...tempAPIContext, _apiModules: tempAPIModules, _scriptPath: scriptPath };
          this.diagnosesMissingMethod(missingMethod, tempScript);
        }
      }
      
      if (error.stack) {
        console.error('📊 Stack Trace:');
        const stackLines = error.stack.split('\n');
        stackLines.slice(0, 3).forEach((line, index) => {
          console.error(`   ${index + 1}: ${line}`);
        });
      }
      
      console.error('❌ ================================================================');
      return false;
    }
  }
  
  /**
   * Remove a specific script from an object
   */
  removeScriptFromObject(objectId, scriptPath) {
    // Attempting to remove script from object
    // Checking if object has active scripts
    
    if (!this.activeScripts.has(objectId)) {
      // No active scripts found for object
      return false;
    }
    
    const scripts = this.activeScripts.get(objectId);
    // Found scripts for object
    let scriptToRemove = null;
    
    for (const script of scripts) {
      // Checking script path
      if (script._scriptPath === scriptPath) {
        // Found matching script to remove
        scriptToRemove = script;
        break;
      }
    }
    
    if (scriptToRemove) {
      // Call onDestroy if available
      if (typeof scriptToRemove.onDestroy === 'function') {
        // Calling onDestroy for script
        try {
          scriptToRemove.onDestroy();
          // onDestroy completed successfully
        } catch (error) {
          console.error('❌ ScriptManager: Error in onDestroy', scriptPath, error);
        }
      } else {
        // No onDestroy method found
      }
      
      // Dispose API module resources
      if (scriptToRemove._scriptAPI && typeof scriptToRemove._scriptAPI.dispose === 'function') {
        try {
          scriptToRemove._scriptAPI.dispose();
          // API modules disposed
        } catch (error) {
          console.error('🔧 ScriptManager: Error disposing API modules', scriptPath, error);
        }
      }
      
      // Clear script references
      scriptToRemove._scriptAPI = null;
      scriptToRemove._babylonObject = null;
      
      scripts.delete(scriptToRemove);
      
      // Clean up empty script sets
      if (scripts.size === 0) {
        this.activeScripts.delete(objectId);
      }
      
      // Script removed successfully
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
      
      // Dispose API module resources
      if (script._scriptAPI && typeof script._scriptAPI.dispose === 'function') {
        try {
          script._scriptAPI.dispose();
          // API modules disposed for script
        } catch (error) {
          console.error('🔧 ScriptManager: Error disposing API modules', script._scriptPath, error);
        }
      }
      
      // Clear script references
      script._scriptAPI = null;
      script._babylonObject = null;
    });
    
    this.activeScripts.delete(objectId);
    // All scripts removed from object
  }
  
  /**
   * Pause a specific script on an object
   */
  pauseScript(objectId, scriptPath) {
    if (!this.pausedScripts.has(objectId)) {
      this.pausedScripts.set(objectId, new Set());
    }
    this.pausedScripts.get(objectId).add(scriptPath);
    // Paused script on object
  }
  
  /**
   * Resume a specific script on an object
   */
  resumeScript(objectId, scriptPath) {
    if (this.pausedScripts.has(objectId)) {
      this.pausedScripts.get(objectId).delete(scriptPath);
      if (this.pausedScripts.get(objectId).size === 0) {
        this.pausedScripts.delete(objectId);
      }
    }
    // Resumed script on object
  }
  
  /**
   * Check if a specific script is paused
   */
  isScriptPaused(objectId, scriptPath) {
    return this.pausedScripts.has(objectId) && this.pausedScripts.get(objectId).has(scriptPath);
  }
  
  /**
   * Update all active scripts
   */
  update() {
    if (!this.isRunning) return;
    
    const rawDeltaTime = this.scene.getEngine().getDeltaTime();
    const deltaTime = rawDeltaTime / 1000; // Convert to seconds
    
    this.activeScripts.forEach((scripts, objectId) => {
      scripts.forEach(script => {
        // Skip paused scripts
        if (this.isScriptPaused(objectId, script._scriptPath)) {
          return;
        }
        
        // Update the API's delta time
        this._updateDeltaTime(script, deltaTime);
        
        if (typeof script.onUpdate === 'function') {
          try {
            script.onUpdate(deltaTime);
          } catch (error) {
            this.logDetailedScriptError(error, script, 'onUpdate');
            
            // Stop the script loop to prevent error spam
            console.error('🛑 ScriptManager: Stopping script execution due to error');
            this.pause();
            return;
          }
        }
      });
    });
  }
  
  /**
   * Find a Babylon.js object by ID
   */
  findBabylonObject(objectId) {
    // Handle scene object - check both legacy 'scene-root' and scene's unique ID
    const sceneId = this.scene.uniqueId || 'scene-root';
    if (objectId === 'scene-root' || objectId === sceneId) {
      return this.scene;
    }
    
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
    let totalPausedScripts = 0;
    
    this.activeScripts.forEach(scripts => {
      totalScripts += scripts.size;
    });
    
    this.pausedScripts.forEach(pausedSet => {
      totalPausedScripts += pausedSet.size;
    });
    
    return {
      objectsWithScripts: this.activeScripts.size,
      totalActiveScripts: totalScripts,
      totalPausedScripts: totalPausedScripts,
      registeredScriptClasses: this.scriptClasses.size,
      isRunning: this.isRunning
    };
  }
  
  /**
   * Get script instance for an object
   * @param {string} objectId - ID of the object
   * @param {string} scriptPath - Path to the script file
   * @returns {Object|null} Script instance or null if not found
   */
  getScriptInstance(objectId, scriptPath) {
    if (!this.activeScripts.has(objectId)) {
      return null;
    }
    
    const scripts = this.activeScripts.get(objectId);
    for (const script of scripts) {
      if (script._scriptPath === scriptPath) {
        return script;
      }
    }
    
    return null;
  }

  /**
   * Start a script instance (call onStart if available)
   * @param {Object} scriptInstance - Script instance to start
   */
  startScriptInstance(scriptInstance) {
    if (scriptInstance && typeof scriptInstance.onStart === 'function') {
      scriptInstance.onStart();
    }
  }

  /**
   * Update objectPropertiesStore when script property changes
   * @param {string} objectId - Object ID
   * @param {string} scriptPath - Script path
   * @param {string} propertyName - Property name
   * @param {*} value - New value
   */
  async updateObjectPropertiesStore(objectId, scriptPath, propertyName, value) {
    try {
      // Import objectPropertiesStore dynamically to avoid circular imports
      const { objectPropertiesStore, objectPropertiesActions } = await import('@/layout/stores/ViewportStore.jsx');
      
      // Find the script in objectPropertiesStore
      const objectProps = objectPropertiesStore.objects[objectId];
      if (objectProps && objectProps.scripts) {
        const scriptIndex = objectProps.scripts.findIndex(script => script.path === scriptPath);
        if (scriptIndex >= 0) {
          // Update the property in objectPropertiesStore
          const propertyPath = `scripts.${scriptIndex}.properties.${propertyName}`;
          objectPropertiesActions.updateObjectProperty(objectId, propertyPath, value);
        }
      }
    } catch (error) {
      console.warn('⚠️ ScriptManager: Could not update objectPropertiesStore:', error);
    }
  }
  
  /**
   * Update script properties for all instances of a script
   * @param {string} scriptPath - Path to the script file
   * @param {Array} newProperties - Array of new property definitions
   * @param {Object} propertyChanges - Changes object with added, removed, modified, renamed arrays
   */
  updateScriptProperties(scriptPath, newProperties, propertyChanges) {
    // Updating properties for script
    
    let updatedInstances = 0;
    
    this.activeScripts.forEach((scripts, objectId) => {
      scripts.forEach(script => {
        if (script._scriptPath === scriptPath) {
          this.updateScriptInstanceProperties(script, newProperties, propertyChanges);
          updatedInstances++;
        }
      });
    });
    
    // Updated script instances
  }
  
  /**
   * Update properties for a single script instance
   * @param {Object} scriptInstance - Script instance to update
   * @param {Array} newProperties - Array of new property definitions
   * @param {Object} propertyChanges - Changes object with added, removed, modified, renamed arrays
   */
  updateScriptInstanceProperties(scriptInstance, newProperties, propertyChanges) {
    if (!scriptInstance || !scriptInstance._scriptAPI) {
      console.warn('🔧 ScriptManager: Invalid script instance for property update');
      return;
    }
    
    const api = scriptInstance._scriptAPI;
    
    try {
      // Handle property additions - set default values
      propertyChanges.added.forEach(prop => {
        try {
          const defaultValue = this.evaluatePropertyDefault(prop.defaultValue);
          api.setScriptProperty(prop.name, defaultValue);
          scriptInstance[prop.name] = defaultValue;
          // Added property with default value
        } catch (error) {
          console.error(`🔧 ScriptManager: Failed to add property '${prop.name}':`, error);
        }
      });
      
      // Handle property modifications - update metadata and potentially update values
      propertyChanges.modified.forEach(change => {
        try {
          const currentValue = api.getScriptProperty(change.old.name);
          const oldDefault = this.evaluatePropertyDefault(change.old.defaultValue);
          const newDefault = this.evaluatePropertyDefault(change.new.defaultValue);
          
          
          // If type changed, reset to new default
          if (change.changes.includes('type')) {
            api.setScriptProperty(change.new.name, newDefault);
            scriptInstance[change.new.name] = newDefault;
            // Property type changed, reset to default
          }
          // If default value changed and current value matches old default, update to new default
          else if (change.changes.includes('defaultValue') && currentValue === oldDefault) {
            api.setScriptProperty(change.new.name, newDefault);
            scriptInstance[change.new.name] = newDefault;
            // Property default changed, updating value
          }
          // If default changed but current value was manually set, keep the manual value
          else if (change.changes.includes('defaultValue') && currentValue !== oldDefault) {
            // Property default changed, keeping manual value
          }
          // For other changes (min, max, description), just update metadata
          else {
            // Property updated, keeping current value
          }
        } catch (error) {
          console.error(`🔧 ScriptManager: Failed to modify property '${change.new.name}':`, error);
        }
      });
      
      // Handle property renames - migrate values
      propertyChanges.renamed.forEach(rename => {
        try {
          const oldValue = api.getScriptProperty(rename.from.name);
          
          // Remove old property
          api.setScriptProperty(rename.from.name, null);
          delete scriptInstance[rename.from.name];
          
          // Set new property with old value (or default if types don't match)
          let newValue = oldValue;
          if (rename.from.propType !== rename.to.propType) {
            newValue = this.evaluatePropertyDefault(rename.to.defaultValue);
            // Property type changed during rename
          }
          
          api.setScriptProperty(rename.to.name, newValue);
          scriptInstance[rename.to.name] = newValue;
          // Property renamed successfully
        } catch (error) {
          console.error(`🔧 ScriptManager: Failed to rename property '${rename.from.name}' to '${rename.to.name}':`, error);
        }
      });
      
      // Handle property removals
      propertyChanges.removed.forEach(prop => {
        try {
          api.setScriptProperty(prop.name, null);
          delete scriptInstance[prop.name];
          // Property removed successfully
        } catch (error) {
          console.error(`🔧 ScriptManager: Failed to remove property '${prop.name}':`, error);
        }
      });
      
      // Update the script properties metadata on the API
      // Transform properties to the format expected by Scene.jsx
      const transformedProperties = newProperties.map(prop => ({
        name: prop.name,
        type: prop.propType, // Scene.jsx expects 'type' not 'propType'
        section: prop.section,
        defaultValue: prop.defaultValue,
        min: prop.min,
        max: prop.max,
        description: prop.description,
        options: prop.options
      }));
      
      api._scriptProperties = transformedProperties;
      
    } catch (error) {
      console.error('🔧 ScriptManager: Error during property update:', error);
    }
  }
  
  /**
   * Log detailed script error with comprehensive debugging information
   */
  logDetailedScriptError(error, script, method) {
    console.error('❌ =================== SCRIPT ERROR ===================');
    console.error(`🔧 Script: ${script._scriptPath}`);
    console.error(`📍 Method: ${method}`);
    console.error(`🎯 Object: ${script._objectId} (${script._babylonObject?.name || 'unknown'})`);
    console.error(`💥 Error: ${error.name}: ${error.message}`);
    
    // Check for missing method errors
    if (error instanceof ReferenceError && error.message.includes('is not defined')) {
      const missingMethod = error.message.match(/(\w+) is not defined/)?.[1];
      if (missingMethod) {
        console.error(`🔍 MISSING METHOD DETECTED: ${missingMethod}`);
        this.diagnosesMissingMethod(missingMethod, script);
      }
    }
    
    // Show stack trace with line numbers
    if (error.stack) {
      console.error('📊 Stack Trace:');
      const stackLines = error.stack.split('\n');
      stackLines.forEach((line, index) => {
        if (index < 5) { // Show first 5 stack lines
          console.error(`   ${index + 1}: ${line}`);
        }
      });
    }
    
    // Show available API methods for debugging
    console.error('🛠️ Available API methods in script context:');
    this.showAvailableAPIMethods(script);
    
    console.error('❌ ================================================');
  }
  
  /**
   * Diagnose missing method and provide helpful suggestions
   */
  diagnosesMissingMethod(missingMethod, script) {
    const api = script._scriptAPI;
    if (!api) {
      console.error('🚨 No API modules available for script!');
      return;
    }
    
    // Check if method exists in API modules
    const modules = ['core', 'material', 'mesh', 'animation', 'sceneQuery', 'physics', 'input', 'texture', 'particle', 'audio', 'gui', 'postProcess', 'xr', 'debug', 'asset', 'utility', 'camera'];
    
    console.error(`🔍 Searching for '${missingMethod}' in API modules:`);
    
    let foundInModule = false;
    modules.forEach(moduleName => {
      const module = api[moduleName];
      if (module && typeof module[missingMethod] === 'function') {
        console.error(`✅ Found '${missingMethod}' in ${moduleName} module`);
        console.error(`💡 Use: ${moduleName}.${missingMethod}() or ensure it's exposed in API modules.js`);
        foundInModule = true;
      }
    });
    
    // Check if method exists directly on API modules
    if (typeof api[missingMethod] === 'function') {
      console.error(`✅ Method '${missingMethod}' exists in API modules but may not be in RenScript context`);
      console.error(`💡 Check Rust RenScript compiler for missing mapping`);
    } else if (!foundInModule) {
      // Try to find similar methods
      const allMethods = [];
      modules.forEach(moduleName => {
        const module = api[moduleName];
        if (module) {
          Object.getOwnPropertyNames(module).forEach(name => {
            if (typeof module[name] === 'function') {
              allMethods.push(`${moduleName}.${name}`);
            }
          });
        }
      });
      
      // Add direct API modules methods
      Object.getOwnPropertyNames(api).forEach(name => {
        if (typeof api[name] === 'function') {
          allMethods.push(name);
        }
      });
      
      // Find similar methods
      const similar = allMethods.filter(method => 
        method.toLowerCase().includes(missingMethod.toLowerCase()) || 
        missingMethod.toLowerCase().includes(method.toLowerCase())
      );
      
      if (similar.length > 0) {
        console.error(`💡 Similar methods found:`);
        similar.slice(0, 5).forEach(method => {
          console.error(`   - ${method}`);
        });
      } else {
        console.error(`❌ Method '${missingMethod}' not found in any module`);
        console.error(`💡 May need to be implemented in one of the API modules`);
      }
    }
  }
  
  /**
   * Show available API methods for debugging
   */
  showAvailableAPIMethods(script) {
    const api = script._scriptAPI;
    if (!api) return;
    
    const modules = ['input', 'animation', 'mesh', 'material', 'physics'];
    modules.forEach(moduleName => {
      const module = api[moduleName];
      if (module) {
        const methods = Object.getOwnPropertyNames(module)
          .filter(name => typeof module[name] === 'function')
          .slice(0, 5); // Show first 5 methods
        
        if (methods.length > 0) {
          console.error(`   ${moduleName}: ${methods.join(', ')}...`);
        }
      }
    });
  }

  /**
   * Evaluate property default value expression
   * @param {*} expression - Default value expression
   * @returns {*} Evaluated value
   */
  evaluatePropertyDefault(expression) {
    if (expression === null || expression === undefined) return null;
    
    try {
      // Handle boolean literals FIRST (before numeric check)
      if (expression === true || expression === 'true') return true;
      if (expression === false || expression === 'false') return false;
      
      // Handle string literals
      if (typeof expression === 'string' && expression.startsWith('"') && expression.endsWith('"')) {
        return expression.slice(1, -1);
      }
      
      // Handle numeric literals (but not booleans)
      if (typeof expression !== 'boolean' && !isNaN(expression)) {
        return parseFloat(expression);
      }
      
      // For more complex expressions, return as-is
      return expression;
    } catch (error) {
      console.warn('Failed to evaluate property default:', expression, error);
      return null;
    }
  }
  
  // === CONVENIENCE METHODS FOR EXTERNAL USE ===
  
  /**
   * Update a script property by object ID and script path
   * @param {string} objectId - Object ID
   * @param {string} scriptPath - Script path
   * @param {string} propertyName - Property name
   * @param {*} value - New value
   * @returns {boolean} True if property was updated
   */
  updateScriptPropertyByPath(objectId, scriptPath, propertyName, value) {
    const scriptInstance = this.getScriptInstance(objectId, scriptPath);
    if (scriptInstance) {
      return this.updateScriptProperty(scriptInstance, propertyName, value);
    }
    return false;
  }
  
  /**
   * Get a script property value by object ID and script path
   * @param {string} objectId - Object ID
   * @param {string} scriptPath - Script path
   * @param {string} propertyName - Property name
   * @returns {*} Property value or undefined
   */
  getScriptPropertyByPath(objectId, scriptPath, propertyName) {
    const scriptInstance = this.getScriptInstance(objectId, scriptPath);
    if (scriptInstance) {
      return this.getPropertyValue(scriptInstance, propertyName);
    }
    return undefined;
  }
  
  /**
   * Set a script property value by object ID and script path
   * @param {string} objectId - Object ID
   * @param {string} scriptPath - Script path
   * @param {string} propertyName - Property name
   * @param {*} value - New value
   * @returns {boolean} True if property was set
   */
  setScriptPropertyByPath(objectId, scriptPath, propertyName, value) {
    const scriptInstance = this.getScriptInstance(objectId, scriptPath);
    if (scriptInstance) {
      return this.setPropertyValue(scriptInstance, propertyName, value);
    }
    return false;
  }
}

export { ScriptManager };