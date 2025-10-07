/**
 * RuntimeAssetLoader - Loads assets for exported projects
 * Handles models, textures, materials without bridge dependency
 */
export class RuntimeAssetLoader {
  constructor(scene) {
    this.scene = scene;
    this.loadedAssets = new Map();
    this.loadingProgress = 0;
  }

  /**
   * Load project assets from bundle data
   * @param {Object} assetsData - Assets from project bundle
   */
  async loadProjectAssets(assetsData) {
    try {
      // Initialize asset loading process
      
      const assetEntries = Object.entries(assetsData);
      // Assets discovered and prioritized
      
      // Load assets in order of priority (models first, then textures, etc.)
      const prioritizedAssets = this.prioritizeAssets(assetEntries);
      
      for (let i = 0; i < prioritizedAssets.length; i++) {
        const [path, assetData] = prioritizedAssets[i];
        
        this.loadingProgress = (i / prioritizedAssets.length) * 100;
        
        await this.loadAsset(path, assetData);
      }
      
      this.loadingProgress = 100;
      // Asset loading complete
      
    } catch (error) {
      console.error('❌ RuntimeAssetLoader: Asset loading failed:', error);
      throw error;
    }
  }

  /**
   * Prioritize assets for loading order
   * @param {Array} assetEntries - Array of [path, assetData] tuples
   * @returns {Array} Prioritized asset entries
   */
  prioritizeAssets(assetEntries) {
    const priority = { 'model': 1, 'texture': 2, 'material': 3, 'audio': 4, 'data': 5, 'unknown': 6 };
    
    return assetEntries.sort(([, a], [, b]) => {
      return (priority[a.type] || 6) - (priority[b.type] || 6);
    });
  }

  /**
   * Load a single asset
   * @param {string} path - Asset path
   * @param {Object} assetData - Asset metadata
   */
  async loadAsset(path, assetData) {
    try {
      // Load individual asset
      
      switch (assetData.type) {
        case 'model':
          await this.loadModel(path, assetData);
          break;
        case 'texture':
          await this.loadTexture(path, assetData);
          break;
        case 'material':
          await this.loadMaterial(path, assetData);
          break;
        case 'audio':
          await this.loadAudio(path, assetData);
          break;
        default:
          // Skip unsupported asset type
      }
      
      this.loadedAssets.set(path, assetData);
      
    } catch (error) {
      console.error('❌ RuntimeAssetLoader: Failed to load asset:', path, error);
      // Continue loading other assets
    }
  }

  /**
   * Load a 3D model
   * @param {string} path - Model path
   * @param {Object} assetData - Asset metadata
   */
  async loadModel(path) {
    try {
      const { SceneLoader } = await import('@babylonjs/core/Loading/sceneLoader.js');
      
      // For runtime, we need to resolve the actual file path
      // In the bundle, assets should be embedded or accessible via relative paths
      const modelUrl = this.resolveAssetUrl(path);
      
      const result = await SceneLoader.ImportMeshAsync('', '', modelUrl, this.scene);
      
      // 3D model loaded successfully
      return result;
      
    } catch (error) {
      console.error('❌ RuntimeAssetLoader: Model loading failed:', path, error);
      throw error;
    }
  }

  /**
   * Load a texture
   * @param {string} path - Texture path
   * @param {Object} assetData - Asset metadata
   */
  async loadTexture(path, assetData) {
    try {
      const { Texture } = await import('@babylonjs/core/Materials/Textures/texture.js');
      
      const textureUrl = this.resolveAssetUrl(path);
      const texture = new Texture(textureUrl, this.scene);
      texture.name = assetData.name;
      
      // Texture loaded successfully
      return texture;
      
    } catch (error) {
      console.error('❌ RuntimeAssetLoader: Texture loading failed:', path, error);
      throw error;
    }
  }

  /**
   * Load a material
   * @param {string} path - Material path
   * @param {Object} assetData - Asset metadata
   */
  async loadMaterial(path) {
    try {
      // Material files are JSON describing material properties
      // For runtime, we need to create Babylon materials from this data
      // Loading material configuration
      
      // TODO: Implement material loading from JSON data
      // This would involve creating StandardMaterial or PBRMaterial instances
      
      return null;
      
    } catch (error) {
      console.error('❌ RuntimeAssetLoader: Material loading failed:', path, error);
      throw error;
    }
  }

  /**
   * Load an audio file
   * @param {string} path - Audio path
   * @param {Object} assetData - Asset metadata
   */
  async loadAudio(path, assetData) {
    try {
      const { Sound } = await import('@babylonjs/core/Audio/sound.js');
      
      const audioUrl = this.resolveAssetUrl(path);
      const sound = new Sound(assetData.name, audioUrl, this.scene);
      
      // Audio asset loaded successfully
      return sound;
      
    } catch (error) {
      console.error('❌ RuntimeAssetLoader: Audio loading failed:', path, error);
      throw error;
    }
  }

  /**
   * Load scene from scene data
   * @param {Object} sceneData - Scene configuration
   */
  async loadScene() {
    try {
      // Load scene data and hierarchy
      
      // TODO: Implement scene loading from JSON
      // This involves recreating the scene hierarchy, object positions, etc.
      
      // Scene loading complete
      
    } catch (error) {
      console.error('❌ RuntimeAssetLoader: Scene loading failed:', error);
      throw error;
    }
  }

  /**
   * Resolve asset URL for runtime access
   * @param {string} path - Asset path
   * @returns {string} Resolved URL
   */
  resolveAssetUrl(path) {
    // In a Tauri app, assets would be bundled and accessible via tauri://
    // For web runtime, they'd be served from a relative path
    
    if (window.__TAURI__) {
      // Tauri runtime
      return `tauri://localhost/assets/${path}`;
    } else {
      // Web runtime
      return `./assets/${path}`;
    }
  }

  /**
   * Get loading progress
   * @returns {number} Progress percentage (0-100)
   */
  getLoadingProgress() {
    return this.loadingProgress;
  }

  /**
   * Dispose of asset loader
   */
  dispose() {
    // Clean up asset loader resources
    this.loadedAssets.clear();
  }
}