import { DirectionalLight } from '@babylonjs/core/Lights/directionalLight.js';
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight.js';
import { PointLight } from '@babylonjs/core/Lights/pointLight.js';
import { SpotLight } from '@babylonjs/core/Lights/spotLight.js';
import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';
import { Color3 } from '@babylonjs/core/Maths/math.color.js';

/**
 * LightingAPI - Light creation and management for RenScript
 * Priority: HIGH - Essential for scene lighting
 */
export class LightingAPI {
  constructor(scene, babylonObject) {
    this.scene = scene;
    this.babylonObject = babylonObject;
  }

  // === DIRECTIONAL LIGHT CREATION ===
  
  createDirectionalLight(name, directionX, directionY, directionZ, options = {}) {
    if (!this.scene) return null;
    
    const direction = new Vector3(directionX, directionY, directionZ);
    const light = new DirectionalLight(name, direction, this.scene);
    
    // Apply options
    if (options.intensity !== undefined) light.intensity = options.intensity;
    if (options.diffuse) light.diffuse = new Color3(...options.diffuse);
    if (options.specular) light.specular = new Color3(...options.specular);
    if (options.enabled !== undefined) light.setEnabled(options.enabled);
    
    // Store base properties for day/night cycling
    if (options.baseIntensity !== undefined) light._baseIntensity = options.baseIntensity;
    if (options.baseColor) light._baseColor = options.baseColor;
    
    return light;
  }

  // === HEMISPHERIC LIGHT CREATION ===
  
  createHemisphericLight(name, directionX, directionY, directionZ, options = {}) {
    if (!this.scene) return null;
    
    const direction = new Vector3(directionX, directionY, directionZ);
    const light = new HemisphericLight(name, direction, this.scene);
    
    // Apply options
    if (options.intensity !== undefined) light.intensity = options.intensity;
    if (options.diffuse) light.diffuse = new Color3(...options.diffuse);
    if (options.specular) light.specular = new Color3(...options.specular);
    if (options.groundColor) light.groundColor = new Color3(...options.groundColor);
    if (options.enabled !== undefined) light.setEnabled(options.enabled);
    
    // Store base properties for day/night cycling
    if (options.baseIntensity !== undefined) light._baseIntensity = options.baseIntensity;
    if (options.baseColor) light._baseColor = options.baseColor;
    
    return light;
  }

  // === POINT LIGHT CREATION ===
  
  createPointLight(name, positionX, positionY, positionZ, options = {}) {
    if (!this.scene) return null;
    
    const position = new Vector3(positionX, positionY, positionZ);
    const light = new PointLight(name, position, this.scene);
    
    // Apply options
    if (options.intensity !== undefined) light.intensity = options.intensity;
    if (options.diffuse) light.diffuse = new Color3(...options.diffuse);
    if (options.specular) light.specular = new Color3(...options.specular);
    if (options.range !== undefined) light.range = options.range;
    if (options.enabled !== undefined) light.setEnabled(options.enabled);
    
    // Store base properties
    if (options.baseMoonIntensity !== undefined) light._baseMoonIntensity = options.baseMoonIntensity;
    
    return light;
  }

  // === SPOT LIGHT CREATION ===
  
  createSpotLight(name, positionX, positionY, positionZ, directionX, directionY, directionZ, angle, exponent, options = {}) {
    if (!this.scene) return null;
    
    const position = new Vector3(positionX, positionY, positionZ);
    const direction = new Vector3(directionX, directionY, directionZ);
    const light = new SpotLight(name, position, direction, angle, exponent, this.scene);
    
    // Apply options
    if (options.intensity !== undefined) light.intensity = options.intensity;
    if (options.diffuse) light.diffuse = new Color3(...options.diffuse);
    if (options.specular) light.specular = new Color3(...options.specular);
    if (options.range !== undefined) light.range = options.range;
    if (options.enabled !== undefined) light.setEnabled(options.enabled);
    
    return light;
  }

  // === LIGHT PROPERTY MANAGEMENT ===
  
  setLightIntensity(light, intensity) {
    if (!light) return false;
    light.intensity = intensity;
    return true;
  }

  getLightIntensity(light) {
    if (!light) return 0;
    return light.intensity;
  }

  setLightDiffuse(light, r, g, b) {
    if (!light) return false;
    light.diffuse = new Color3(r, g, b);
    return true;
  }

  getLightDiffuse(light) {
    if (!light || !light.diffuse) return [1, 1, 1];
    return [light.diffuse.r, light.diffuse.g, light.diffuse.b];
  }

  setLightSpecular(light, r, g, b) {
    if (!light) return false;
    light.specular = new Color3(r, g, b);
    return true;
  }

  getLightSpecular(light) {
    if (!light || !light.specular) return [1, 1, 1];
    return [light.specular.r, light.specular.g, light.specular.b];
  }

  setLightDirection(light, x, y, z) {
    if (!light || !light.direction) return false;
    light.direction = new Vector3(x, y, z);
    return true;
  }

  getLightDirection(light) {
    if (!light || !light.direction) return [0, -1, 0];
    return [light.direction.x, light.direction.y, light.direction.z];
  }

  setLightPosition(light, x, y, z) {
    if (!light || !light.position) return false;
    light.position = new Vector3(x, y, z);
    return true;
  }

  getLightPosition(light) {
    if (!light || !light.position) return [0, 0, 0];
    return [light.position.x, light.position.y, light.position.z];
  }

  enableLight(light) {
    if (!light) return false;
    light.setEnabled(true);
    return true;
  }

  disableLight(light) {
    if (!light) return false;
    light.setEnabled(false);
    return true;
  }

  isLightEnabled(light) {
    if (!light) return false;
    return light.isEnabled();
  }

  // === HEMISPHERIC LIGHT SPECIFIC ===
  
  setGroundColor(light, r, g, b) {
    if (!light || !light.groundColor) return false;
    light.groundColor = new Color3(r, g, b);
    return true;
  }

  getGroundColor(light) {
    if (!light || !light.groundColor) return [0.5, 0.5, 0.5];
    return [light.groundColor.r, light.groundColor.g, light.groundColor.b];
  }

  // === POINT/SPOT LIGHT SPECIFIC ===
  
  setLightRange(light, range) {
    if (!light || light.range === undefined) return false;
    light.range = range;
    return true;
  }

  getLightRange(light) {
    if (!light || light.range === undefined) return 100;
    return light.range;
  }

  // === LIGHT QUERIES ===
  
  findLightByName(name) {
    if (!this.scene) return null;
    return this.scene.lights.find(light => light.name === name);
  }

  getAllLights() {
    if (!this.scene) return [];
    return this.scene.lights;
  }

  getLightsByType(type) {
    if (!this.scene) return [];
    
    const typeMap = {
      'directional': DirectionalLight,
      'hemispheric': HemisphericLight,
      'point': PointLight,
      'spot': SpotLight
    };
    
    const LightClass = typeMap[type.toLowerCase()];
    if (!LightClass) return [];
    
    return this.scene.lights.filter(light => light instanceof LightClass);
  }

  // === LIGHT MANAGEMENT ===
  
  disposeLight(light) {
    if (!light || !light.dispose) return false;
    light.dispose();
    return true;
  }

  cloneLight(light, name) {
    if (!light || !light.clone) return null;
    return light.clone(name);
  }

  getLightInfo(light) {
    if (!light) return null;
    
    const info = {
      name: light.name,
      type: light.getTypeID(),
      intensity: light.intensity,
      enabled: light.isEnabled(),
      diffuse: light.diffuse ? [light.diffuse.r, light.diffuse.g, light.diffuse.b] : null,
      specular: light.specular ? [light.specular.r, light.specular.g, light.specular.b] : null
    };
    
    // Add type-specific properties
    if (light.direction) {
      info.direction = [light.direction.x, light.direction.y, light.direction.z];
    }
    if (light.position) {
      info.position = [light.position.x, light.position.y, light.position.z];
    }
    if (light.groundColor) {
      info.groundColor = [light.groundColor.r, light.groundColor.g, light.groundColor.b];
    }
    if (light.range !== undefined) {
      info.range = light.range;
    }
    
    return info;
  }

  // === PRESET LIGHTING SETUPS ===
  
  createStandardLighting(options = {}) {
    const lights = {};
    
    // Sky light (ambient)
    lights.skyLight = this.createHemisphericLight(
      'skyLight',
      0, 1, 0,
      {
        intensity: options.skyIntensity || 1.0,
        diffuse: options.skyColor || [0.7, 0.8, 1.0],
        groundColor: options.groundColor || [0.6, 0.55, 0.5],
        baseIntensity: options.skyIntensity || 1.0,
        baseColor: options.skyColor || [0.7, 0.8, 1.0]
      }
    );
    
    // Sun light (main directional)
    lights.sunLight = this.createDirectionalLight(
      'sunLight',
      options.sunDirection ? options.sunDirection[0] : -0.3,
      options.sunDirection ? options.sunDirection[1] : -0.8,
      options.sunDirection ? options.sunDirection[2] : -0.5,
      {
        intensity: options.sunIntensity || 1.0,
        diffuse: options.sunColor || [1.0, 0.98, 0.9],
        specular: [1.0, 0.95, 0.85],
        baseIntensity: options.sunIntensity || 1.0,
        baseColor: options.sunColor || [1.0, 0.98, 0.9]
      }
    );
    
    // Rim light (atmospheric scattering)
    lights.rimLight = this.createDirectionalLight(
      'rimLight',
      0.8, 0.2, -0.6,
      {
        intensity: options.rimIntensity || 0.4,
        diffuse: options.rimColor || [0.9, 0.7, 0.5],
        specular: [0.0, 0.0, 0.0],
        baseIntensity: options.rimIntensity || 0.4,
        baseColor: options.rimColor || [0.9, 0.7, 0.5]
      }
    );
    
    // Bounce light (indirect lighting)
    lights.bounceLight = this.createDirectionalLight(
      'bounceLight',
      -0.2, 0.3, 0.9,
      {
        intensity: options.bounceIntensity || 0.3,
        diffuse: options.bounceColor || [0.4, 0.5, 0.7],
        specular: [0.0, 0.0, 0.0],
        baseIntensity: options.bounceIntensity || 0.3,
        baseColor: options.bounceColor || [0.4, 0.5, 0.7]
      }
    );
    
    return lights;
  }

  createMoonLighting(options = {}) {
    const moonLight = this.createPointLight(
      'moonLight',
      options.position ? options.position[0] : 100,
      options.position ? options.position[1] : 300,
      options.position ? options.position[2] : 200,
      {
        intensity: options.intensity || 0,
        diffuse: options.diffuse || [0.3, 0.3, 0.4],
        specular: options.specular || [0.2, 0.2, 0.3],
        range: options.range || 1000,
        baseMoonIntensity: options.baseMoonIntensity || 15.0
      }
    );
    
    return moonLight;
  }

  // === SHORT NAME ALIASES ===
  
  directionalLight(name, directionX, directionY, directionZ, options = {}) {
    return this.createDirectionalLight(name, directionX, directionY, directionZ, options);
  }
  
  hemisphericLight(name, directionX, directionY, directionZ, options = {}) {
    return this.createHemisphericLight(name, directionX, directionY, directionZ, options);
  }
  
  pointLight(name, positionX, positionY, positionZ, options = {}) {
    return this.createPointLight(name, positionX, positionY, positionZ, options);
  }
  
  spotLight(name, positionX, positionY, positionZ, directionX, directionY, directionZ, angle, exponent, options = {}) {
    return this.createSpotLight(name, positionX, positionY, positionZ, directionX, directionY, directionZ, angle, exponent, options);
  }
  
  lightIntensity(light) {
    return this.getLightIntensity(light);
  }
  
  lightDiffuse(light) {
    return this.getLightDiffuse(light);
  }
  
  lightSpecular(light) {
    return this.getLightSpecular(light);
  }
  
  lightDirection(light) {
    return this.getLightDirection(light);
  }
  
  lightPosition(light) {
    return this.getLightPosition(light);
  }
  
  lightInfo(light) {
    return this.getLightInfo(light);
  }
  
  standardLighting(options = {}) {
    return this.createStandardLighting(options);
  }
  
  moonLighting(options = {}) {
    return this.createMoonLighting(options);
  }
}