import { ShadowGenerator } from '@babylonjs/core/Lights/Shadows/shadowGenerator.js';
import { CascadedShadowGenerator } from '@babylonjs/core/Lights/Shadows/cascadedShadowGenerator.js';

/**
 * ShadowAPI - Shadow generation and management for RenScript
 * Priority: MEDIUM - Advanced lighting feature
 */
export class ShadowAPI {
  constructor(scene, babylonObject) {
    this.scene = scene;
    this.babylonObject = babylonObject;
    this.shadowGenerators = new Map(); // Store shadow generators by name
  }

  // === SHADOW GENERATOR CREATION ===
  
  createShadowGenerator(name, light, options = {}) {
    if (!light || !this.scene) return null;
    
    const mapSize = options.mapSize || 1024;
    const shadowGenerator = new ShadowGenerator(mapSize, light);
    
    // Apply options
    if (options.usePercentageCloserFiltering !== undefined) {
      shadowGenerator.usePercentageCloserFiltering = options.usePercentageCloserFiltering;
    }
    if (options.filteringQuality !== undefined) {
      shadowGenerator.filteringQuality = options.filteringQuality;
    }
    if (options.darkness !== undefined) shadowGenerator.darkness = options.darkness;
    if (options.bias !== undefined) shadowGenerator.bias = options.bias;
    if (options.useContactHardeningShadow !== undefined) {
      shadowGenerator.useContactHardeningShadow = options.useContactHardeningShadow;
    }
    if (options.usePoissonSampling !== undefined) {
      shadowGenerator.usePoissonSampling = options.usePoissonSampling;
    }
    if (options.useExponentialShadowMap !== undefined) {
      shadowGenerator.useExponentialShadowMap = options.useExponentialShadowMap;
    }
    if (options.blurKernel !== undefined) shadowGenerator.blurKernel = options.blurKernel;
    
    // Cascade shadows
    if (options.useCascades !== undefined) shadowGenerator.useCascades = options.useCascades;
    if (options.numCascades !== undefined) shadowGenerator.numCascades = options.numCascades;
    if (options.cascadeBlendPercentage !== undefined) {
      shadowGenerator.cascadeBlendPercentage = options.cascadeBlendPercentage;
    }
    
    this.shadowGenerators.set(name, shadowGenerator);
    
    // Store in scene for global access
    if (!this.scene.shadowGenerator) {
      this.scene.shadowGenerator = shadowGenerator;
    }
    
    return shadowGenerator;
  }

  createCascadedShadowGenerator(name, light, options = {}) {
    if (!light || !this.scene) return null;
    
    const mapSize = options.mapSize || 1024;
    const shadowGenerator = new CascadedShadowGenerator(mapSize, light);
    
    // Apply options
    if (options.numCascades !== undefined) shadowGenerator.numCascades = options.numCascades;
    if (options.autoCalcDepthBounds !== undefined) shadowGenerator.autoCalcDepthBounds = options.autoCalcDepthBounds;
    if (options.shadowMaxZ !== undefined) shadowGenerator.shadowMaxZ = options.shadowMaxZ;
    if (options.darkness !== undefined) shadowGenerator.darkness = options.darkness;
    if (options.bias !== undefined) shadowGenerator.bias = options.bias;
    if (options.usePercentageCloserFiltering !== undefined) {
      shadowGenerator.usePercentageCloserFiltering = options.usePercentageCloserFiltering;
    }
    if (options.filteringQuality !== undefined) {
      shadowGenerator.filteringQuality = options.filteringQuality;
    }
    
    this.shadowGenerators.set(name, shadowGenerator);
    
    return shadowGenerator;
  }

  // === SHADOW CASTER MANAGEMENT ===
  
  addShadowCaster(generatorName, mesh) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator || !mesh) return false;
    
    generator.addShadowCaster(mesh);
    return true;
  }

  removeShadowCaster(generatorName, mesh) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator || !mesh) return false;
    
    generator.removeShadowCaster(mesh);
    return true;
  }

  addShadowCasters(generatorName, meshes) {
    if (!Array.isArray(meshes)) return false;
    
    let success = true;
    meshes.forEach(mesh => {
      if (!this.addShadowCaster(generatorName, mesh)) {
        success = false;
      }
    });
    
    return success;
  }

  // === SHADOW RECEIVER MANAGEMENT ===
  
  enableShadowReceiver(mesh, enabled = true) {
    if (!mesh) return false;
    mesh.receiveShadows = enabled;
    return true;
  }

  isShadowReceiver(mesh) {
    if (!mesh) return false;
    return mesh.receiveShadows;
  }

  // === SHADOW PROPERTIES ===
  
  setShadowMapSize(generatorName, size) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return false;
    
    generator.mapSize = size;
    return true;
  }

  getShadowMapSize(generatorName) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return 1024;
    return generator.mapSize;
  }

  setShadowDarkness(generatorName, darkness) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return false;
    generator.darkness = darkness;
    return true;
  }

  getShadowDarkness(generatorName) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return 0.0;
    return generator.darkness;
  }

  setShadowBias(generatorName, bias) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return false;
    generator.bias = bias;
    return true;
  }

  getShadowBias(generatorName) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return 0.0;
    return generator.bias;
  }

  setBlurKernel(generatorName, kernel) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return false;
    generator.blurKernel = kernel;
    return true;
  }

  getBlurKernel(generatorName) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return 1;
    return generator.blurKernel;
  }

  // === SHADOW TECHNIQUES ===
  
  enablePercentageCloserFiltering(generatorName, enabled = true) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return false;
    generator.usePercentageCloserFiltering = enabled;
    return true;
  }

  setFilteringQuality(generatorName, quality) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return false;
    // QUALITY_HIGH = 0, QUALITY_MEDIUM = 1, QUALITY_LOW = 2
    generator.filteringQuality = quality;
    return true;
  }

  enableContactHardeningShadow(generatorName, enabled = true) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return false;
    generator.useContactHardeningShadow = enabled;
    return true;
  }

  enablePoissonSampling(generatorName, enabled = true) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return false;
    generator.usePoissonSampling = enabled;
    return true;
  }

  enableExponentialShadowMap(generatorName, enabled = true) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return false;
    generator.useExponentialShadowMap = enabled;
    return true;
  }

  // === CASCADE SHADOWS ===
  
  enableCascades(generatorName, enabled = true) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return false;
    generator.useCascades = enabled;
    return true;
  }

  setNumCascades(generatorName, num) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return false;
    generator.numCascades = num;
    return true;
  }

  setCascadeBlendPercentage(generatorName, percentage) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return false;
    generator.cascadeBlendPercentage = percentage;
    return true;
  }

  // === SHADOW QUERIES ===
  
  getShadowGenerator(name) {
    return this.shadowGenerators.get(name);
  }

  getAllShadowGenerators() {
    return Array.from(this.shadowGenerators.values());
  }

  getShadowGeneratorInfo(generatorName) {
    const generator = this.shadowGenerators.get(generatorName);
    if (!generator) return null;
    
    return {
      name: generatorName,
      mapSize: generator.mapSize,
      darkness: generator.darkness,
      bias: generator.bias,
      blurKernel: generator.blurKernel,
      usePercentageCloserFiltering: generator.usePercentageCloserFiltering,
      filteringQuality: generator.filteringQuality,
      useContactHardeningShadow: generator.useContactHardeningShadow,
      usePoissonSampling: generator.usePoissonSampling,
      useExponentialShadowMap: generator.useExponentialShadowMap,
      useCascades: generator.useCascades,
      numCascades: generator.numCascades,
      cascadeBlendPercentage: generator.cascadeBlendPercentage
    };
  }

  // === SHADOW PRESETS ===
  
  createOptimizedShadows(name, light, options = {}) {
    const shadowMapSize = Math.min(2048, options.shadowMapSize || 1024);
    
    return this.createShadowGenerator(name, light, {
      mapSize: shadowMapSize,
      usePercentageCloserFiltering: true,
      filteringQuality: 1, // QUALITY_MEDIUM
      darkness: options.shadowDarkness || 0.5,
      bias: options.shadowBias || 0.00005,
      useContactHardeningShadow: false, // Disabled for performance
      useCascades: options.cascadeShadows !== undefined ? options.cascadeShadows : true,
      numCascades: Math.min(2, options.shadowCascades || 2),
      cascadeBlendPercentage: 0.1,
      useExponentialShadowMap: false,
      usePoissonSampling: true,
      blurKernel: Math.min(32, options.shadowBlur || 16)
    });
  }

  createHighQualityShadows(name, light, options = {}) {
    return this.createShadowGenerator(name, light, {
      mapSize: options.shadowMapSize || 4096,
      usePercentageCloserFiltering: true,
      filteringQuality: 0, // QUALITY_HIGH
      darkness: options.shadowDarkness || 0.5,
      bias: options.shadowBias || 0.00001,
      useContactHardeningShadow: true,
      useCascades: options.cascadeShadows !== undefined ? options.cascadeShadows : true,
      numCascades: options.shadowCascades || 4,
      cascadeBlendPercentage: 0.05,
      useExponentialShadowMap: false,
      usePoissonSampling: false,
      blurKernel: options.shadowBlur || 64
    });
  }

  createFastShadows(name, light, options = {}) {
    return this.createShadowGenerator(name, light, {
      mapSize: options.shadowMapSize || 512,
      usePercentageCloserFiltering: false,
      filteringQuality: 2, // QUALITY_LOW
      darkness: options.shadowDarkness || 0.3,
      bias: options.shadowBias || 0.0001,
      useContactHardeningShadow: false,
      useCascades: false,
      useExponentialShadowMap: false,
      usePoissonSampling: true,
      blurKernel: Math.min(16, options.shadowBlur || 8)
    });
  }

  // === SHADOW MANAGEMENT ===
  
  disposeShadowGenerator(name) {
    const generator = this.shadowGenerators.get(name);
    if (!generator) return false;
    
    generator.dispose();
    this.shadowGenerators.delete(name);
    
    // Clear from scene if it was the main generator
    if (this.scene.shadowGenerator === generator) {
      this.scene.shadowGenerator = null;
    }
    
    return true;
  }

  disposeAllShadowGenerators() {
    for (const name of this.shadowGenerators.keys()) {
      this.disposeShadowGenerator(name);
    }
    return true;
  }

  // === SHADOW UTILITIES ===
  
  enableShadowsForMesh(mesh, castShadows = true, receiveShadows = true) {
    if (!mesh) return false;
    
    // Add to shadow casters if requested
    if (castShadows) {
      // Add to all shadow generators
      for (const generator of this.shadowGenerators.values()) {
        generator.addShadowCaster(mesh);
      }
    }
    
    // Enable shadow receiving
    mesh.receiveShadows = receiveShadows;
    
    return true;
  }

  disableShadowsForMesh(mesh) {
    if (!mesh) return false;
    
    // Remove from all shadow casters
    for (const generator of this.shadowGenerators.values()) {
      generator.removeShadowCaster(mesh);
    }
    
    // Disable shadow receiving
    mesh.receiveShadows = false;
    
    return true;
  }

  enableShadowsForAllMeshes(filter = null) {
    if (!this.scene) return false;
    
    let count = 0;
    this.scene.meshes.forEach(mesh => {
      // Skip system meshes
      if (mesh.name.includes('gizmo') || mesh.name.includes('helper') || mesh.name.startsWith('__')) {
        return;
      }
      
      // Apply filter if provided
      if (filter && !filter(mesh)) {
        return;
      }
      
      this.enableShadowsForMesh(mesh);
      count++;
    });
    
    return count;
  }

  // === SHADOW CONSTANTS ===
  
  static get QUALITY_HIGH() { return 0; }
  static get QUALITY_MEDIUM() { return 1; }
  static get QUALITY_LOW() { return 2; }
  
  static get FILTER_NONE() { return 0; }
  static get FILTER_PCF() { return 1; }
  static get FILTER_PCSS() { return 2; }
  static get FILTER_POISSON() { return 3; }

  // === SHORT NAME ALIASES ===
  
  shadowGenerator(name, light, options = {}) {
    return this.createShadowGenerator(name, light, options);
  }
  
  cascadedShadows(name, light, options = {}) {
    return this.createCascadedShadowGenerator(name, light, options);
  }
  
  optimizedShadows(name, light, options = {}) {
    return this.createOptimizedShadows(name, light, options);
  }
  
  highQualityShadows(name, light, options = {}) {
    return this.createHighQualityShadows(name, light, options);
  }
  
  fastShadows(name, light, options = {}) {
    return this.createFastShadows(name, light, options);
  }
  
  shadowCaster(mesh, enabled = true) {
    return this.enableShadowsForMesh(mesh, enabled, mesh.receiveShadows);
  }
  
  shadowReceiver(mesh, enabled = true) {
    return this.enableShadowReceiver(mesh, enabled);
  }
  
  shadowInfo(generatorName) {
    return this.getShadowGeneratorInfo(generatorName);
  }
  
  allShadowGenerators() {
    return this.getAllShadowGenerators();
  }
}