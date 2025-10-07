import { bridgeService } from '@/plugins/core/bridge';

/**
 * ExportCompiler - Compiles RenScript files for standalone export
 * Works independently of the bridge server to create self-contained JavaScript
 */
export class ExportCompiler {
  constructor() {
    this.apiMethods = this.getAPIMethodMappings();
    this.mathFunctions = ['sin', 'cos', 'tan', 'asin', 'acos', 'atan', 'atan2', 'sqrt', 'abs', 'floor', 'ceil', 'round', 'pow', 'exp', 'min', 'max', 'random'];
  }

  /**
   * Compile a single RenScript file to standalone JavaScript
   * @param {string} scriptPath - Path to the .ren file
   * @param {string} projectName - Name of the project
   * @returns {Promise<{success: boolean, code?: string, error?: string}>}
   */
  async compileScript(scriptPath, _projectName) {
    try {
      console.log('🔥 ExportCompiler: Compiling script for export:', scriptPath);
      
      // Use bridge service to compile the script
      const response = await fetch(`http://localhost:3001/script/${scriptPath.replace('.ren', '')}`);
      
      if (!response.ok) {
        throw new Error(`Failed to compile script: HTTP ${response.status}`);
      }
      
      const compiledJS = await response.text();
      
      // Transform the compiled JavaScript to be standalone
      const standaloneJS = this.makeScriptStandalone(compiledJS, scriptPath);
      
      console.log('✅ ExportCompiler: Script compiled successfully');
      return {
        success: true,
        code: standaloneJS
      };
      
    } catch (error) {
      console.error('❌ ExportCompiler: Compilation failed:', error);
      return {
        success: false,
        error: error.message
      };
    }
  }

  /**
   * Compile all RenScript files in a project
   * @param {string} projectPath - Path to the project directory
   * @returns {Promise<{success: boolean, scripts?: Map<string, string>, errors?: Array}>}
   */
  async compileProjectScripts(projectPath) {
    try {
      console.log('🔥 ExportCompiler: Compiling all project scripts');
      
      // Get list of .ren files in the project
      const scriptsData = await bridgeService.listDirectory(`${projectPath}/assets/scripts`);
      
      if (!scriptsData || !scriptsData.files) {
        console.log('ℹ️ ExportCompiler: No scripts directory found, skipping script compilation');
        return { success: true, scripts: new Map() };
      }
      
      const renFiles = scriptsData.files
        .filter(file => file.name.endsWith('.ren') && file.type === 'file')
        .map(file => file.path);
      
      console.log(`🔥 ExportCompiler: Found ${renFiles.length} RenScript files`);
      
      const compiledScripts = new Map();
      const errors = [];
      
      for (const scriptPath of renFiles) {
        const result = await this.compileScript(scriptPath, projectPath);
        
        if (result.success) {
          compiledScripts.set(scriptPath, result.code);
        } else {
          errors.push({ path: scriptPath, error: result.error });
        }
      }
      
      if (errors.length > 0) {
        console.warn('⚠️ ExportCompiler: Some scripts failed to compile:', errors);
      }
      
      return {
        success: true,
        scripts: compiledScripts,
        errors: errors.length > 0 ? errors : undefined
      };
      
    } catch (error) {
      console.error('❌ ExportCompiler: Project compilation failed:', error);
      return {
        success: false,
        error: error.message
      };
    }
  }

  /**
   * Transform compiled JavaScript to be standalone (no bridge dependency)
   * @param {string} compiledJS - Original compiled JavaScript
   * @param {string} scriptPath - Path to the original script
   * @returns {string} Standalone JavaScript code
   */
  makeScriptStandalone(compiledJS, _scriptPath) {
    // Inject embedded API runtime instead of requiring bridge connection
    const apiRuntime = this.generateEmbeddedAPIRuntime();
    
    // Replace the function signature to accept embedded API
    let standaloneJS = compiledJS.replace(
      /function createRenScript\(scene, api\) \{/,
      `function createRenScript(scene, embeddedAPI) {
  // Embedded API Runtime
${apiRuntime}`
    );
    
    // Replace API binding checks to use embedded API
    standaloneJS = standaloneJS.replace(
      /if \(!api\.(\w+)\) throw new Error\(/g,
      'if (!embeddedAPI.$1) throw new Error('
    );
    
    standaloneJS = standaloneJS.replace(
      /const (\w+) = api\.(\w+)\.bind\(api\);/g,
      'const $1 = embeddedAPI.$2.bind(embeddedAPI);'
    );
    
    return standaloneJS;
  }

  /**
   * Generate embedded API runtime for standalone scripts
   * @returns {string} JavaScript code for embedded API
   */
  generateEmbeddedAPIRuntime() {
    return `  // Embedded API Runtime for Export
  const embeddedAPI = {
    // Core API
    log: (...args) => console.log('[RenScript]', ...args),
    time: () => performance.now() / 1000,
    
    // Transform API
    getPosition: function() {
      return this._object ? { 
        x: this._object.position.x, 
        y: this._object.position.y, 
        z: this._object.position.z 
      } : { x: 0, y: 0, z: 0 };
    },
    setPosition: function(x, y, z) {
      if (this._object) {
        if (typeof x === 'object') {
          this._object.position.x = x.x || 0;
          this._object.position.y = x.y || 0;
          this._object.position.z = x.z || 0;
        } else {
          this._object.position.x = x || 0;
          this._object.position.y = y || 0;
          this._object.position.z = z || 0;
        }
      }
    },
    setRotation: function(x, y, z) {
      if (this._object) {
        if (typeof x === 'object') {
          this._object.rotation.x = x.x || 0;
          this._object.rotation.y = x.y || 0;
          this._object.rotation.z = x.z || 0;
        } else {
          this._object.rotation.x = x || 0;
          this._object.rotation.y = y || 0;
          this._object.rotation.z = z || 0;
        }
      }
    },
    
    // Input API (minimal for runtime)
    isKeyPressed: (key) => false, // TODO: Implement basic input
    getMousePosition: () => ({ x: 0, y: 0 }),
    
    // Physics API (minimal)
    enablePhysics: () => console.warn('Physics not available in export'),
    
    // Scene Query API
    findObjectByName: function(name) {
      return scene.getMeshByName(name) || scene.getNodeByName(name);
    },
    
    // Math utilities
    random: () => Math.random(),
    randomRange: (min, max) => Math.random() * (max - min) + min,
    clamp: (value, min, max) => Math.max(min, Math.min(max, value)),
    lerp: (a, b, t) => a + (b - a) * t,
    distance: (a, b) => {
      const dx = a.x - b.x;
      const dy = a.y - b.y;
      const dz = a.z - b.z;
      return Math.sqrt(dx * dx + dy * dy + dz * dz);
    }
  };
  
  // Bind object reference
  embeddedAPI._object = scene.meshes.find(m => m.metadata?.entityId === this._objectId) || null;
  embeddedAPI._scene = scene;
  
  // Use embedded API instead of external api parameter
  const api = embeddedAPI;`;
  }

  /**
   * Get API method mappings (copy of the Rust mappings)
   */
  getAPIMethodMappings() {
    return new Map([
      // Core functions
      ['log', 'log'],
      ['time', 'time'],
      
      // Transform functions  
      ['position', 'getPosition'],
      ['setPosition', 'setPosition'],
      ['rotation', 'getRotation'],
      ['setRotation', 'setRotation'],
      ['setScale', 'setScale'],
      ['move', 'moveBy'],
      ['rotate', 'rotateBy'],
      
      // Input functions
      ['isKeyPressed', 'isKeyPressed'],
      ['mousePosition', 'getMousePosition'],
      ['mouseX', 'getMouseX'],
      ['mouseY', 'getMouseY'],
      
      // Scene query functions
      ['findByName', 'findObjectByName'],
      ['findByTag', 'findObjectsByTag'],
      ['getAllMeshes', 'getAllMeshes'],
      
      // Animation functions
      ['animate', 'animate'],
      ['animatePosition', 'animatePosition'],
      ['animateRotation', 'animateRotation'],
      
      // Utility functions
      ['random', 'random'],
      ['randomRange', 'randomRange'],
      ['clamp', 'clamp'],
      ['lerp', 'lerp'],
      ['distance', 'distance']
    ]);
  }

  /**
   * Generate a complete runtime package for a project
   * @param {string} projectPath - Path to the project
   * @returns {Promise<{success: boolean, package?: Object}>}
   */
  async generateRuntimePackage(projectPath) {
    try {
      console.log('📦 ExportCompiler: Generating runtime package for:', projectPath);
      
      // 1. Compile all scripts
      const scriptResults = await this.compileProjectScripts(projectPath);
      if (!scriptResults.success) {
        return { success: false, error: 'Script compilation failed' };
      }

      // 2. Read project configuration
      const projectJson = await bridgeService.readFile(`${projectPath}/project.json`);
      if (!projectJson.success) {
        return { success: false, error: 'Could not read project.json' };
      }

      const projectConfig = JSON.parse(projectJson.content);

      // 3. Create runtime package
      const runtimePackage = {
        project: projectConfig,
        scripts: Object.fromEntries(scriptResults.scripts),
        assets: await this.collectProjectAssets(projectPath),
        scenes: await this.collectProjectScenes(projectPath),
        version: '1.0.0',
        exported: new Date().toISOString()
      };

      console.log('✅ ExportCompiler: Runtime package generated');
      return { success: true, package: runtimePackage };

    } catch (error) {
      console.error('❌ ExportCompiler: Runtime package generation failed:', error);
      return { success: false, error: error.message };
    }
  }

  /**
   * Collect project assets metadata
   * @param {string} projectPath - Path to the project
   * @returns {Promise<Object>} Assets metadata
   */
  async collectProjectAssets(projectPath) {
    try {
      const assetsResponse = await bridgeService.listFiles(`${projectPath}/assets`);
      if (!assetsResponse.success) {
        return {};
      }

      // Collect metadata for all asset files
      const assets = {};
      
      for (const file of assetsResponse.files) {
        if (file.type === 'file') {
          assets[file.path] = {
            name: file.name,
            size: file.size,
            modified: file.modified,
            type: this.getAssetType(file.name)
          };
        }
      }

      return assets;
    } catch (error) {
      console.error('❌ ExportCompiler: Asset collection failed:', error);
      return {};
    }
  }

  /**
   * Collect project scenes
   * @param {string} projectPath - Path to the project
   * @returns {Promise<Object>} Scenes data
   */
  async collectProjectScenes(projectPath) {
    try {
      const scenesResponse = await bridgeService.listFiles(`${projectPath}/scenes`);
      if (!scenesResponse.success) {
        return {};
      }

      const scenes = {};
      
      for (const file of scenesResponse.files) {
        if (file.name.endsWith('.json')) {
          const sceneData = await bridgeService.readFile(file.path);
          if (sceneData.success) {
            scenes[file.name] = JSON.parse(sceneData.content);
          }
        }
      }

      return scenes;
    } catch (error) {
      console.error('❌ ExportCompiler: Scene collection failed:', error);
      return {};
    }
  }

  /**
   * Determine asset type from file extension
   * @param {string} filename - Name of the file
   * @returns {string} Asset type
   */
  getAssetType(filename) {
    const ext = filename.toLowerCase().split('.').pop();
    
    const typeMap = {
      'glb': 'model',
      'gltf': 'model',
      'png': 'texture',
      'jpg': 'texture',
      'jpeg': 'texture',
      'wav': 'audio',
      'mp3': 'audio',
      'ogg': 'audio',
      'json': 'data',
      'material': 'material',
      'ren': 'script'
    };
    
    return typeMap[ext] || 'unknown';
  }
}

export const exportCompiler = new ExportCompiler();