import { exportCompiler } from './ExportCompiler.js';
import { bridgeService } from '@/plugins/core/bridge';

/**
 * ProjectBundler - Creates complete project bundles for export
 */
export class ProjectBundler {
  constructor() {
    this.bundleVersion = '1.0.0';
  }

  /**
   * Create a complete bundle for a project
   * @param {string} projectName - Name of the project to bundle
   * @param {Object} options - Bundling options
   * @returns {Promise<{success: boolean, bundle?: Object, errors?: Array}>}
   */
  async createBundle(projectName, options = {}) {
    try {
      console.log('📦 ProjectBundler: Creating bundle for project:', projectName);
      
      const projectPath = `projects/${projectName}`;
      const errors = [];
      
      // 1. Read project configuration
      const projectConfig = await this.loadProjectConfig(projectPath);
      if (!projectConfig) {
        return { success: false, error: 'Could not load project configuration' };
      }

      // 2. Compile all RenScript files
      console.log('📦 ProjectBundler: Compiling scripts...');
      const scriptResults = await exportCompiler.compileProjectScripts(projectPath);
      if (scriptResults.errors) {
        errors.push(...scriptResults.errors);
      }

      // 3. Collect assets
      console.log('📦 ProjectBundler: Collecting assets...');
      const assets = await this.collectAssets(projectPath);

      // 4. Collect scenes
      console.log('📦 ProjectBundler: Collecting scenes...');
      const scenes = await this.collectScenes(projectPath);

      // 5. Generate runtime manifest
      const manifest = this.generateRuntimeManifest(projectConfig, scriptResults.scripts, assets, scenes);

      // 6. Create final bundle
      const bundle = {
        manifest,
        project: projectConfig,
        scripts: this.packageScripts(scriptResults.scripts),
        assets: this.packageAssets(assets),
        scenes: this.packageScenes(scenes),
        runtime: this.generateRuntimeCode(),
        metadata: {
          bundled: new Date().toISOString(),
          bundler_version: this.bundleVersion,
          engine_version: projectConfig.engine_version || '1.0.0',
          has_scripts: scriptResults.scripts.size > 0,
          script_count: scriptResults.scripts.size,
          asset_count: Object.keys(assets).length
        }
      };

      console.log('✅ ProjectBundler: Bundle created successfully');
      console.log(`📊 ProjectBundler: ${bundle.metadata.script_count} scripts, ${bundle.metadata.asset_count} assets`);
      
      return { 
        success: true, 
        bundle, 
        errors: errors.length > 0 ? errors : undefined 
      };

    } catch (error) {
      console.error('❌ ProjectBundler: Bundle creation failed:', error);
      return { success: false, error: error.message };
    }
  }

  /**
   * Load project configuration
   * @param {string} projectPath - Path to project
   * @returns {Promise<Object|null>}
   */
  async loadProjectConfig(projectPath) {
    try {
      const content = await bridgeService.readFile(`${projectPath}/project.json`);
      if (content) {
        return JSON.parse(content);
      }
      return null;
    } catch (error) {
      console.error('❌ ProjectBundler: Failed to load project config:', error);
      return null;
    }
  }

  /**
   * Collect all project assets with metadata
   * @param {string} projectPath - Path to project
   * @returns {Promise<Object>}
   */
  async collectAssets(projectPath) {
    const assets = {};
    
    try {
      const assetsData = await bridgeService.listDirectory(`${projectPath}/assets`);
      if (!assetsData || !assetsData.files) {
        console.log('ℹ️ ProjectBundler: No assets found');
        return assets;
      }

      // Recursively collect all asset files
      await this.collectAssetsRecursive(`${projectPath}/assets`, assets);
      
    } catch (error) {
      console.error('❌ ProjectBundler: Asset collection failed:', error);
    }
    
    return assets;
  }

  /**
   * Recursively collect assets from a directory
   * @param {string} dirPath - Directory path
   * @param {Object} assets - Assets collection object
   */
  async collectAssetsRecursive(dirPath, assets) {
    try {
      const dirData = await bridgeService.listDirectory(dirPath);
      if (!dirData || !dirData.files) return;

      for (const item of dirData.files) {
        if (item.type === 'directory') {
          await this.collectAssetsRecursive(item.path, assets);
        } else {
          // Calculate relative path from project root
          const relativePath = item.path.replace(/.*\/projects\/[^/]+\//, '');
          
          assets[relativePath] = {
            name: item.name,
            path: item.path,
            relativePath,
            size: item.size,
            type: this.getAssetType(item.name),
            modified: item.modified
          };
        }
      }
    } catch (error) {
      console.error('❌ ProjectBundler: Recursive asset collection failed:', error);
    }
  }

  /**
   * Collect scene files
   * @param {string} projectPath - Path to project
   * @returns {Promise<Object>}
   */
  async collectScenes(projectPath) {
    const scenes = {};
    
    try {
      const scenesData = await bridgeService.listDirectory(`${projectPath}/scenes`);
      if (!scenesData || !scenesData.files) {
        console.log('ℹ️ ProjectBundler: No scenes found');
        return scenes;
      }

      for (const file of scenesData.files) {
        if (file.type === 'file' && file.name.endsWith('.json')) {
          const sceneContent = await bridgeService.readFile(file.path);
          if (sceneContent) {
            scenes[file.name] = {
              data: JSON.parse(sceneContent),
              modified: file.modified,
              size: file.size
            };
          }
        }
      }
    } catch (error) {
      console.error('❌ ProjectBundler: Scene collection failed:', error);
    }
    
    return scenes;
  }

  /**
   * Package scripts for runtime
   * @param {Map<string, string>} scripts - Compiled scripts map
   * @returns {Object}
   */
  packageScripts(scripts) {
    const packagedScripts = {};
    
    scripts.forEach((code, path) => {
      const scriptName = path.replace(/.*\/([^/]+)\.ren$/, '$1');
      packagedScripts[scriptName] = {
        code,
        originalPath: path,
        compiled: true
      };
    });
    
    return packagedScripts;
  }

  /**
   * Package assets for runtime
   * @param {Object} assets - Assets collection
   * @returns {Object}
   */
  packageAssets(assets) {
    const packagedAssets = {};
    
    Object.entries(assets).forEach(([path, asset]) => {
      packagedAssets[path] = {
        name: asset.name,
        type: asset.type,
        size: asset.size,
        path: asset.relativePath,
        modified: asset.modified
      };
    });
    
    return packagedAssets;
  }

  /**
   * Package scenes for runtime
   * @param {Object} scenes - Scenes collection
   * @returns {Object}
   */
  packageScenes(scenes) {
    return scenes; // Scenes are already in the right format
  }

  /**
   * Generate runtime manifest
   * @param {Object} projectConfig - Project configuration
   * @param {Map} scripts - Compiled scripts
   * @param {Object} assets - Project assets
   * @param {Object} scenes - Project scenes
   * @returns {Object}
   */
  generateRuntimeManifest(projectConfig, scripts, assets, scenes) {
    return {
      name: projectConfig.name,
      version: projectConfig.version,
      description: projectConfig.description,
      author: projectConfig.author,
      
      runtime: {
        requires_physics: projectConfig.settings?.physics?.enabled || false,
        requires_audio: Object.values(assets).some(a => a.type === 'audio'),
        requires_input: scripts.size > 0, // Assume scripts need input
        entry_scene: this.findEntryScene(scenes),
        resolution: projectConfig.settings?.render?.resolution || { width: 1920, height: 1080 }
      },
      
      files: {
        scripts: Array.from(scripts.keys()).map(path => path.replace(/.*\/([^/]+)\.ren$/, '$1')),
        scenes: Object.keys(scenes),
        assets: Object.keys(assets)
      }
    };
  }

  /**
   * Find the main/entry scene
   * @param {Object} scenes - Scenes collection
   * @returns {string|null}
   */
  findEntryScene(scenes) {
    const sceneNames = Object.keys(scenes);
    
    // Look for common entry scene names
    const candidates = ['main.json', 'index.json', 'scene.json'];
    for (const candidate of candidates) {
      if (sceneNames.includes(candidate)) {
        return candidate;
      }
    }
    
    // Return first scene if available
    return sceneNames[0] || null;
  }

  /**
   * Generate runtime code for the exported app
   * @returns {string}
   */
  generateRuntimeCode() {
    return `
// Runtime bootstrapper for exported Renzora project
class ExportedRuntimeBootstrapper {
  constructor() {
    this.scene = null;
    this.engine = null;
    this.canvas = null;
    this.scriptInstances = new Map();
  }
  
  async initialize(canvas) {
    console.log('🚀 ExportedRuntime: Initializing...');
    
    this.canvas = canvas;
    
    // Initialize Babylon.js engine
    const { Engine } = await import('@babylonjs/core/Engines/engine.js');
    const { Scene } = await import('@babylonjs/core/scene.js');
    
    this.engine = new Engine(canvas, true);
    this.scene = new Scene(this.engine);
    
    // Load project data
    await this.loadProject();
    
    // Start render loop
    this.engine.runRenderLoop(() => {
      this.scene.render();
    });
    
    // Handle window resize
    window.addEventListener('resize', () => {
      this.engine.resize();
    });
    
    console.log('✅ ExportedRuntime: Initialization complete');
  }
  
  async loadProject() {
    // Project data will be embedded in the bundle
    const projectData = window.__RENZORA_PROJECT_DATA__;
    
    if (!projectData) {
      throw new Error('Project data not found');
    }
    
    console.log('📦 ExportedRuntime: Loading project:', projectData.project.name);
    
    // Load scenes
    if (projectData.manifest.runtime.entry_scene) {
      await this.loadScene(projectData.manifest.runtime.entry_scene, projectData.scenes);
    }
    
    // Initialize scripts
    await this.initializeScripts(projectData.scripts);
  }
  
  async loadScene(sceneName, scenesData) {
    console.log('🎭 ExportedRuntime: Loading scene:', sceneName);
    
    const sceneData = scenesData[sceneName];
    if (!sceneData) {
      console.error('❌ Scene not found:', sceneName);
      return;
    }
    
    // TODO: Implement scene loading from JSON data
    // This would involve recreating meshes, lights, cameras etc.
  }
  
  async initializeScripts(scriptsData) {
    console.log('📜 ExportedRuntime: Initializing scripts...');
    
    for (const [scriptName, scriptData] of Object.entries(scriptsData)) {
      try {
        // Evaluate the compiled script code
        const scriptFunction = new Function('scene', scriptData.code + '; return createRenScript;');
        const ScriptClass = scriptFunction(this.scene);
        
        // TODO: Attach scripts to appropriate objects based on scene data
        console.log('✅ Script loaded:', scriptName);
      } catch (error) {
        console.error('❌ Script loading failed:', scriptName, error);
      }
    }
  }
  
  dispose() {
    if (this.engine) {
      this.engine.dispose();
    }
  }
}

// Auto-initialize when DOM is ready
document.addEventListener('DOMContentLoaded', async () => {
  const canvas = document.getElementById('renderCanvas');
  if (canvas) {
    const runtime = new ExportedRuntimeBootstrapper();
    await runtime.initialize(canvas);
    
    // Make runtime globally available
    window.__RENZORA_RUNTIME__ = runtime;
  }
});
`;
  }

  /**
   * Get asset type from filename
   * @param {string} filename - File name
   * @returns {string} Asset type
   */
  getAssetType(filename) {
    return exportCompiler.getAssetType(filename);
  }

  /**
   * Write bundle to file system
   * @param {Object} bundle - The complete bundle
   * @param {string} outputPath - Path to write the bundle
   * @returns {Promise<{success: boolean}>}
   */
  async writeBundle(bundle, outputPath) {
    try {
      const bundleJSON = JSON.stringify(bundle, null, 2);
      
      const response = await bridgeService.writeFile(outputPath, bundleJSON);
      
      if (response.success) {
        console.log('✅ ProjectBundler: Bundle written to:', outputPath);
        return { success: true };
      } else {
        return { success: false, error: 'Failed to write bundle file' };
      }
    } catch (error) {
      console.error('❌ ProjectBundler: Bundle write failed:', error);
      return { success: false, error: error.message };
    }
  }
}

export const projectBundler = new ProjectBundler();