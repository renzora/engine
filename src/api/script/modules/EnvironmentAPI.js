import { CreateSphere } from '@babylonjs/core/Meshes/Builders/sphereBuilder.js';
import { SkyMaterial } from '@babylonjs/materials/sky/skyMaterial.js';
import { PBRMaterial } from '@babylonjs/core/Materials/PBR/pbrMaterial.js';
import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';
import { Color3, Color4 } from '@babylonjs/core/Maths/math.color.js';
import { ImageProcessingConfiguration } from '@babylonjs/core/Materials/imageProcessingConfiguration.js';

/**
 * EnvironmentAPI - Sky, fog, and scene environment management for RenScript
 * Priority: HIGH - Essential for scene atmosphere
 */
export class EnvironmentAPI {
  constructor(scene, babylonObject) {
    this.scene = scene;
    this.babylonObject = babylonObject;
  }

  // === SKYBOX CREATION ===
  
  createSkybox(name, options = {}) {
    if (!this.scene) return null;
    
    const diameter = options.diameter || 1000.0;
    const skybox = CreateSphere(name || 'skyBox', { diameter }, this.scene);
    skybox.infiniteDistance = true;
    skybox._isInternalMesh = true;
    
    return skybox;
  }

  createSkyMaterial(name, options = {}) {
    if (!this.scene) return null;
    
    const skyMaterial = new SkyMaterial(name || 'skyMaterial', this.scene);
    skyMaterial.backFaceCulling = false;
    
    // Apply options
    if (options.turbidity !== undefined) skyMaterial.turbidity = options.turbidity;
    if (options.luminance !== undefined) skyMaterial.luminance = options.luminance;
    if (options.inclination !== undefined) skyMaterial.inclination = options.inclination;
    if (options.azimuth !== undefined) skyMaterial.azimuth = options.azimuth;
    if (options.cloudsEnabled !== undefined) skyMaterial.cloudsEnabled = options.cloudsEnabled;
    if (options.cloudSize !== undefined) skyMaterial.cumulusCloudSize = options.cloudSize;
    if (options.cloudDensity !== undefined) skyMaterial.cumulusCloudDensity = options.cloudDensity;
    if (options.sunPosition) skyMaterial.sunPosition = new Vector3(...options.sunPosition);
    
    return skyMaterial;
  }

  applySkyboxMaterial(skybox, skyMaterial) {
    if (!skybox || !skyMaterial) return false;
    skybox.material = skyMaterial;
    return true;
  }

  // === MOON CREATION ===
  
  createMoon(name, options = {}) {
    if (!this.scene) return null;
    
    const diameter = options.diameter || 20;
    const moon = CreateSphere(name || 'moon', { diameter }, this.scene);
    
    // Set initial position
    if (options.position) {
      moon.position = new Vector3(...options.position);
    } else {
      moon.position = new Vector3(100, 300, 200);
    }
    
    // Create moon material
    const moonMaterial = new PBRMaterial(`${name || 'moon'}Material`, this.scene);
    moonMaterial.baseColor = new Color3(...(options.baseColor || [0.9, 0.9, 0.8]));
    moonMaterial.emissiveColor = new Color3(...(options.emissiveColor || [0.3, 0.3, 0.25]));
    moonMaterial.metallicFactor = options.metallicFactor || 0.0;
    moonMaterial.roughnessFactor = options.roughnessFactor || 0.8;
    moonMaterial.disableLighting = options.disableLighting !== undefined ? options.disableLighting : true;
    
    moon.material = moonMaterial;
    moon._isInternalMesh = true;
    
    return moon;
  }

  // === FOG MANAGEMENT ===
  
  enableFog(enabled = true) {
    if (!this.scene) return false;
    this.scene.fogEnabled = enabled;
    return true;
  }

  setFogMode(mode) {
    if (!this.scene) return false;
    // FOGMODE_NONE = 0, FOGMODE_EXP = 1, FOGMODE_EXP2 = 2, FOGMODE_LINEAR = 3
    this.scene.fogMode = mode;
    return true;
  }

  setFogDensity(density) {
    if (!this.scene) return false;
    this.scene.fogDensity = density;
    return true;
  }

  setFogColor(r, g, b) {
    if (!this.scene) return false;
    this.scene.fogColor = new Color3(r, g, b);
    return true;
  }

  setFogStart(start) {
    if (!this.scene) return false;
    this.scene.fogStart = start;
    return true;
  }

  setFogEnd(end) {
    if (!this.scene) return false;
    this.scene.fogEnd = end;
    return true;
  }

  getFogSettings() {
    if (!this.scene) return null;
    
    return {
      enabled: this.scene.fogEnabled,
      mode: this.scene.fogMode,
      density: this.scene.fogDensity,
      color: this.scene.fogColor ? [this.scene.fogColor.r, this.scene.fogColor.g, this.scene.fogColor.b] : [1, 1, 1],
      start: this.scene.fogStart,
      end: this.scene.fogEnd
    };
  }

  // === SCENE PROPERTIES ===
  
  setClearColor(r, g, b, a = 1) {
    if (!this.scene) return false;
    this.scene.clearColor = new Color4(r, g, b, a);
    return true;
  }

  getClearColor() {
    if (!this.scene || !this.scene.clearColor) return [0, 0, 0, 1];
    return [this.scene.clearColor.r, this.scene.clearColor.g, this.scene.clearColor.b, this.scene.clearColor.a];
  }

  setEnvironmentIntensity(intensity) {
    if (!this.scene) return false;
    this.scene.environmentIntensity = intensity;
    return true;
  }

  getEnvironmentIntensity() {
    if (!this.scene) return 1.0;
    return this.scene.environmentIntensity;
  }

  setAutoClear(enabled) {
    if (!this.scene) return false;
    this.scene.autoClear = enabled;
    return true;
  }

  getAutoClear() {
    if (!this.scene) return true;
    return this.scene.autoClear;
  }

  // === IMAGE PROCESSING ===
  
  enableToneMapping(enabled = true) {
    if (!this.scene || !this.scene.imageProcessingConfiguration) return false;
    this.scene.imageProcessingConfiguration.toneMappingEnabled = enabled;
    return true;
  }

  setToneMappingType(type) {
    if (!this.scene || !this.scene.imageProcessingConfiguration) return false;
    // TONEMAPPING_STANDARD = 0, TONEMAPPING_ACES = 1
    this.scene.imageProcessingConfiguration.toneMappingType = type;
    return true;
  }

  setExposure(exposure) {
    if (!this.scene || !this.scene.imageProcessingConfiguration) return false;
    this.scene.imageProcessingConfiguration.exposure = exposure;
    return true;
  }

  getExposure() {
    if (!this.scene || !this.scene.imageProcessingConfiguration) return 1.0;
    return this.scene.imageProcessingConfiguration.exposure;
  }

  setContrast(contrast) {
    if (!this.scene || !this.scene.imageProcessingConfiguration) return false;
    this.scene.imageProcessingConfiguration.contrast = contrast;
    return true;
  }

  getContrast() {
    if (!this.scene || !this.scene.imageProcessingConfiguration) return 1.0;
    return this.scene.imageProcessingConfiguration.contrast;
  }

  // === VIGNETTE EFFECTS ===
  
  enableVignette(enabled = true) {
    if (!this.scene || !this.scene.imageProcessingConfiguration) return false;
    this.scene.imageProcessingConfiguration.vignetteEnabled = enabled;
    return true;
  }

  setVignetteWeight(weight) {
    if (!this.scene || !this.scene.imageProcessingConfiguration) return false;
    this.scene.imageProcessingConfiguration.vignetteWeight = weight;
    return true;
  }

  setVignetteStretch(stretch) {
    if (!this.scene || !this.scene.imageProcessingConfiguration) return false;
    this.scene.imageProcessingConfiguration.vignetteStretch = stretch;
    return true;
  }

  setVignetteCameraFov(fov) {
    if (!this.scene || !this.scene.imageProcessingConfiguration) return false;
    this.scene.imageProcessingConfiguration.vignetteCameraFov = fov;
    return true;
  }

  getVignetteSettings() {
    if (!this.scene || !this.scene.imageProcessingConfiguration) return null;
    
    const config = this.scene.imageProcessingConfiguration;
    return {
      enabled: config.vignetteEnabled,
      weight: config.vignetteWeight,
      stretch: config.vignetteStretch,
      cameraFov: config.vignetteCameraFov
    };
  }

  // === ANTI-ALIASING ===
  
  enableFXAA(enabled = true) {
    if (!this.scene || !this.scene.imageProcessingConfiguration) return false;
    this.scene.imageProcessingConfiguration.fxaaEnabled = enabled;
    return true;
  }

  isFXAAEnabled() {
    if (!this.scene || !this.scene.imageProcessingConfiguration) return false;
    return this.scene.imageProcessingConfiguration.fxaaEnabled;
  }

  // === SKY MATERIAL CONTROL ===
  
  setSkyTurbidity(skyMaterial, turbidity) {
    if (!skyMaterial) return false;
    skyMaterial.turbidity = turbidity;
    return true;
  }

  setSkyLuminance(skyMaterial, luminance) {
    if (!skyMaterial) return false;
    skyMaterial.luminance = luminance;
    return true;
  }

  setSkyInclination(skyMaterial, inclination) {
    if (!skyMaterial) return false;
    skyMaterial.inclination = inclination;
    return true;
  }

  setSkyAzimuth(skyMaterial, azimuth) {
    if (!skyMaterial) return false;
    skyMaterial.azimuth = azimuth;
    return true;
  }

  enableSkyClouds(skyMaterial, enabled = true) {
    if (!skyMaterial) return false;
    skyMaterial.cloudsEnabled = enabled;
    return true;
  }

  setSkyCloudSize(skyMaterial, size) {
    if (!skyMaterial) return false;
    skyMaterial.cumulusCloudSize = size;
    return true;
  }

  setSkyCloudDensity(skyMaterial, density) {
    if (!skyMaterial) return false;
    skyMaterial.cumulusCloudDensity = density;
    return true;
  }

  setSkyCloudOpacity(skyMaterial, opacity) {
    if (!skyMaterial) return false;
    skyMaterial.cumulusCloudOpacity = opacity;
    return true;
  }

  setSunPosition(skyMaterial, x, y, z) {
    if (!skyMaterial) return false;
    skyMaterial.sunPosition = new Vector3(x, y, z);
    return true;
  }

  getSkyMaterialInfo(skyMaterial) {
    if (!skyMaterial) return null;
    
    return {
      turbidity: skyMaterial.turbidity,
      luminance: skyMaterial.luminance,
      inclination: skyMaterial.inclination,
      azimuth: skyMaterial.azimuth,
      cloudsEnabled: skyMaterial.cloudsEnabled,
      cloudSize: skyMaterial.cumulusCloudSize,
      cloudDensity: skyMaterial.cumulusCloudDensity,
      cloudOpacity: skyMaterial.cumulusCloudOpacity,
      sunPosition: skyMaterial.sunPosition ? [skyMaterial.sunPosition.x, skyMaterial.sunPosition.y, skyMaterial.sunPosition.z] : [0, 1, 0]
    };
  }

  // === PRESET ENVIRONMENTS ===
  
  createStandardEnvironment(options = {}) {
    const environment = {};
    
    // Create skybox
    environment.skybox = this.createSkybox('skyBox', { diameter: options.skyboxSize || 1000 });
    
    // Create sky material
    environment.skyMaterial = this.createSkyMaterial('skyMaterial', {
      turbidity: options.turbidity || 10,
      luminance: options.luminance || 1.0,
      inclination: options.inclination || 0.5,
      azimuth: options.azimuth || 0.25,
      cloudsEnabled: options.cloudsEnabled !== undefined ? options.cloudsEnabled : true,
      cloudSize: options.cloudSize || 25,
      cloudDensity: options.cloudDensity || 0.6
    });
    
    // Apply material to skybox
    this.applySkyboxMaterial(environment.skybox, environment.skyMaterial);
    
    // Set up fog if enabled
    if (options.fogEnabled) {
      this.enableFog(true);
      this.setFogMode(options.fogMode || 2); // EXP2
      this.setFogDensity(options.fogDensity || 0.01);
      this.setFogColor(...(options.fogColor || [0.7, 0.8, 1.0]));
    }
    
    // Set clear color
    if (options.clearColor) {
      this.setClearColor(...options.clearColor);
    }
    
    // Set environment intensity
    if (options.environmentIntensity !== undefined) {
      this.setEnvironmentIntensity(options.environmentIntensity);
    }
    
    // Configure image processing
    if (options.toneMappingEnabled !== undefined) {
      this.enableToneMapping(options.toneMappingEnabled);
    }
    if (options.toneMappingType !== undefined) {
      this.setToneMappingType(options.toneMappingType);
    }
    if (options.exposure !== undefined) {
      this.setExposure(options.exposure);
    }
    if (options.contrast !== undefined) {
      this.setContrast(options.contrast);
    }
    
    // Configure vignette
    if (options.vignetteEnabled !== undefined) {
      this.enableVignette(options.vignetteEnabled);
    }
    if (options.vignetteWeight !== undefined) {
      this.setVignetteWeight(options.vignetteWeight);
    }
    if (options.vignetteStretch !== undefined) {
      this.setVignetteStretch(options.vignetteStretch);
    }
    if (options.vignetteCameraFov !== undefined) {
      this.setVignetteCameraFov(options.vignetteCameraFov);
    }
    
    // Configure FXAA
    if (options.fxaaEnabled !== undefined) {
      this.enableFXAA(options.fxaaEnabled);
    }
    
    return environment;
  }

  createMoon(name, options = {}) {
    if (!this.scene) return null;
    
    const diameter = options.diameter || 20;
    const moon = CreateSphere(name || 'moon', { diameter }, this.scene);
    
    // Set position
    if (options.position) {
      moon.position = new Vector3(...options.position);
    } else {
      moon.position = new Vector3(100, 300, 200);
    }
    
    // Create moon material
    const moonMaterial = new PBRMaterial(`${name || 'moon'}Material`, this.scene);
    moonMaterial.baseColor = new Color3(...(options.baseColor || [0.9, 0.9, 0.8]));
    moonMaterial.emissiveColor = new Color3(...(options.emissiveColor || [0.3, 0.3, 0.25]));
    moonMaterial.metallicFactor = options.metallicFactor || 0.0;
    moonMaterial.roughnessFactor = options.roughnessFactor || 0.8;
    moonMaterial.disableLighting = options.disableLighting !== undefined ? options.disableLighting : true;
    
    moon.material = moonMaterial;
    moon._isInternalMesh = true;
    
    return moon;
  }

  // === ENVIRONMENTAL PRESETS ===
  
  createDayEnvironment(options = {}) {
    return this.createStandardEnvironment({
      turbidity: options.turbidity || 2,
      luminance: options.luminance || 1.0,
      inclination: options.inclination || 0.0,
      azimuth: options.azimuth || 0.25,
      cloudsEnabled: options.cloudsEnabled !== undefined ? options.cloudsEnabled : true,
      cloudSize: options.cloudSize || 25,
      cloudDensity: options.cloudDensity || 0.6,
      clearColor: options.clearColor || [0.7, 0.8, 1.0, 1],
      fogEnabled: options.fogEnabled || false,
      environmentIntensity: options.environmentIntensity || 1.0,
      ...options
    });
  }

  createNightEnvironment(options = {}) {
    return this.createStandardEnvironment({
      turbidity: options.turbidity || 10,
      luminance: options.luminance || 0.1,
      inclination: options.inclination || -1.0, // Hide sun
      azimuth: options.azimuth || 0.0,
      cloudsEnabled: options.cloudsEnabled !== undefined ? options.cloudsEnabled : false,
      clearColor: options.clearColor || [0.05, 0.05, 0.15, 1],
      fogEnabled: options.fogEnabled || false,
      environmentIntensity: options.environmentIntensity || 0.3,
      ...options
    });
  }

  createSunsetEnvironment(options = {}) {
    return this.createStandardEnvironment({
      turbidity: options.turbidity || 8,
      luminance: options.luminance || 0.8,
      inclination: options.inclination || -0.4,
      azimuth: options.azimuth || 0.75,
      cloudsEnabled: options.cloudsEnabled !== undefined ? options.cloudsEnabled : true,
      cloudSize: options.cloudSize || 30,
      cloudDensity: options.cloudDensity || 0.8,
      clearColor: options.clearColor || [1.0, 0.6, 0.3, 1],
      environmentIntensity: options.environmentIntensity || 0.7,
      ...options
    });
  }

  // === SHORT NAME ALIASES ===
  
  skybox(name, options = {}) {
    return this.createSkybox(name, options);
  }
  
  skyMaterial(name, options = {}) {
    return this.createSkyMaterial(name, options);
  }
  
  moon(name, options = {}) {
    return this.createMoon(name, options);
  }
  
  fog(enabled = true) {
    return this.enableFog(enabled);
  }
  
  fogSettings() {
    return this.getFogSettings();
  }
  
  clearColor() {
    return this.getClearColor();
  }
  
  environmentIntensity() {
    return this.getEnvironmentIntensity();
  }
  
  vignetteSettings() {
    return this.getVignetteSettings();
  }
  
  skyInfo(skyMaterial) {
    return this.getSkyMaterialInfo(skyMaterial);
  }
  
  dayEnvironment(options = {}) {
    return this.createDayEnvironment(options);
  }
  
  nightEnvironment(options = {}) {
    return this.createNightEnvironment(options);
  }
  
  sunsetEnvironment(options = {}) {
    return this.createSunsetEnvironment(options);
  }
}