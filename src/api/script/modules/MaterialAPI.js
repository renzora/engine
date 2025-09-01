// === MATERIAL API MODULE ===

import { 
  StandardMaterial, 
  MultiMaterial, 
  Color3, 
  Color4,
  Vector2 
} from '@babylonjs/core';

// PBR Materials
import { 
  PBRMaterial,
  PBRMetallicRoughnessMaterial,
  PBRSpecularGlossinessMaterial
} from '@babylonjs/core/Materials/PBR/index.js';

// Special Materials
import { BackgroundMaterial } from '@babylonjs/core/Materials/Background/backgroundMaterial.js';
import { NodeMaterial } from '@babylonjs/core/Materials/Node/nodeMaterial.js';
import { ShaderMaterial } from '@babylonjs/core/Materials/shaderMaterial.js';

// Advanced Materials from materials package
import { CellMaterial } from '@babylonjs/materials/cell/cellMaterial.js';
import { CustomMaterial } from '@babylonjs/materials/custom/customMaterial.js';
import { PBRCustomMaterial } from '@babylonjs/materials/custom/pbrCustomMaterial.js';
import { SimpleMaterial } from '@babylonjs/materials/simple/simpleMaterial.js';
import { ShadowOnlyMaterial } from '@babylonjs/materials/shadowOnly/shadowOnlyMaterial.js';
import { SkyMaterial } from '@babylonjs/materials/sky/skyMaterial.js';
import { WaterMaterial } from '@babylonjs/materials/water/waterMaterial.js';
import { TerrainMaterial } from '@babylonjs/materials/terrain/terrainMaterial.js';
import { GridMaterial } from '@babylonjs/materials/grid/gridMaterial.js';
import { TriPlanarMaterial } from '@babylonjs/materials/triPlanar/triPlanarMaterial.js';
import { MixMaterial } from '@babylonjs/materials/mix/mixMaterial.js';
import { LavaMaterial } from '@babylonjs/materials/lava/lavaMaterial.js';
import { FireMaterial } from '@babylonjs/materials/fire/fireMaterial.js';
import { FurMaterial } from '@babylonjs/materials/fur/furMaterial.js';
import { GradientMaterial } from '@babylonjs/materials/gradient/gradientMaterial.js';

export class MaterialAPI {
  constructor(scene) {
    this.scene = scene;
  }

  // === ALL MATERIAL CREATION FUNCTIONS ===

  createStandardMaterial(name, options = {}) {
    const material = new StandardMaterial(name, this.scene);
    if (options.diffuseColor) material.diffuseColor = new Color3(...options.diffuseColor);
    if (options.specularColor) material.specularColor = new Color3(...options.specularColor);
    if (options.emissiveColor) material.emissiveColor = new Color3(...options.emissiveColor);
    if (options.ambientColor) material.ambientColor = new Color3(...options.ambientColor);
    if (options.specularPower !== undefined) material.specularPower = options.specularPower;
    if (options.alpha !== undefined) material.alpha = options.alpha;
    return material;
  }

  createPBRMaterial(name, options = {}) {
    const material = new PBRMaterial(name, this.scene);
    if (options.baseColor) material.baseColor = new Color3(...options.baseColor);
    if (options.metallic !== undefined) material.metallic = options.metallic;
    if (options.roughness !== undefined) material.roughness = options.roughness;
    if (options.emissiveColor) material.emissiveColor = new Color3(...options.emissiveColor);
    if (options.alpha !== undefined) material.alpha = options.alpha;
    return material;
  }

  createPBRMetallicRoughnessMaterial(name, options = {}) {
    const material = new PBRMetallicRoughnessMaterial(name, this.scene);
    if (options.baseColor) material.baseColor = new Color3(...options.baseColor);
    if (options.metallic !== undefined) material.metallicFactor = options.metallic;
    if (options.roughness !== undefined) material.roughnessFactor = options.roughness;
    if (options.alpha !== undefined) material.alpha = options.alpha;
    return material;
  }

  createPBRSpecularGlossinessMaterial(name, options = {}) {
    const material = new PBRSpecularGlossinessMaterial(name, this.scene);
    if (options.diffuseColor) material.diffuseColor = new Color3(...options.diffuseColor);
    if (options.specularColor) material.specularColor = new Color3(...options.specularColor);
    if (options.glossiness !== undefined) material.glossiness = options.glossiness;
    return material;
  }

  createUnlitMaterial(name, options = {}) {
    const material = new SimpleMaterial(name, this.scene);
    if (options.diffuseColor) material.diffuseColor = new Color3(...options.diffuseColor);
    if (options.alpha !== undefined) material.alpha = options.alpha;
    return material;
  }

  createBackgroundMaterial(name, options = {}) {
    const material = new BackgroundMaterial(name, this.scene);
    if (options.primaryColor) material.primaryColor = new Color3(...options.primaryColor);
    if (options.shadowLevel !== undefined) material.shadowLevel = options.shadowLevel;
    if (options.alpha !== undefined) material.alpha = options.alpha;
    return material;
  }

  createNodeMaterial(name, options = {}) {
    const material = new NodeMaterial(name, this.scene, options);
    return material;
  }

  createShaderMaterial(name, vertexShader, fragmentShader, options = {}) {
    const material = new ShaderMaterial(name, this.scene, { 
      vertex: vertexShader, 
      fragment: fragmentShader 
    }, options);
    return material;
  }

  createMultiMaterial(name, subMaterials = []) {
    const material = new MultiMaterial(name, this.scene);
    material.subMaterials = subMaterials;
    return material;
  }

  // === ADVANCED MATERIALS ===

  createCellMaterial(name, options = {}) {
    const material = new CellMaterial(name, this.scene);
    if (options.diffuseColor) material.diffuseColor = new Color3(...options.diffuseColor);
    if (options.computeHighLevel !== undefined) material.computeHighLevel = options.computeHighLevel;
    return material;
  }

  createCustomMaterial(name, options = {}) {
    return new CustomMaterial(name, this.scene, options);
  }

  createPBRCustomMaterial(name, options = {}) {
    return new PBRCustomMaterial(name, this.scene, options);
  }

  createSimpleMaterial(name, options = {}) {
    const material = new SimpleMaterial(name, this.scene);
    if (options.diffuseColor) material.diffuseColor = new Color3(...options.diffuseColor);
    return material;
  }

  createShadowOnlyMaterial(name, options = {}) {
    return new ShadowOnlyMaterial(name, this.scene, options);
  }

  createSkyMaterial(name, options = {}) {
    const material = new SkyMaterial(name, this.scene);
    if (options.turbidity !== undefined) material.turbidity = options.turbidity;
    if (options.luminance !== undefined) material.luminance = options.luminance;
    if (options.inclination !== undefined) material.inclination = options.inclination;
    if (options.azimuth !== undefined) material.azimuth = options.azimuth;
    return material;
  }

  createWaterMaterial(name, options = {}) {
    const material = new WaterMaterial(name, this.scene, options.size || new Vector2(512, 512));
    if (options.bumpTexture) material.bumpTexture = options.bumpTexture;
    if (options.windDirection) material.windDirection = new Vector2(...options.windDirection);
    if (options.waveHeight !== undefined) material.waveHeight = options.waveHeight;
    if (options.waveSpeed !== undefined) material.waveSpeed = options.waveSpeed;
    if (options.waveLength !== undefined) material.waveLength = options.waveLength;
    return material;
  }

  createTerrainMaterial(name, options = {}) {
    const material = new TerrainMaterial(name, this.scene);
    if (options.diffuseTexture1) material.diffuseTexture1 = options.diffuseTexture1;
    if (options.diffuseTexture2) material.diffuseTexture2 = options.diffuseTexture2;
    if (options.diffuseTexture3) material.diffuseTexture3 = options.diffuseTexture3;
    if (options.bumpTexture1) material.bumpTexture1 = options.bumpTexture1;
    if (options.bumpTexture2) material.bumpTexture2 = options.bumpTexture2;
    if (options.bumpTexture3) material.bumpTexture3 = options.bumpTexture3;
    if (options.mixTexture) material.mixTexture = options.mixTexture;
    return material;
  }

  createGridMaterial(name, options = {}) {
    const material = new GridMaterial(name, this.scene);
    if (options.mainColor) material.mainColor = new Color3(...options.mainColor);
    if (options.lineColor) material.lineColor = new Color3(...options.lineColor);
    if (options.gridRatio !== undefined) material.gridRatio = options.gridRatio;
    if (options.majorUnitFrequency !== undefined) material.majorUnitFrequency = options.majorUnitFrequency;
    if (options.minorUnitVisibility !== undefined) material.minorUnitVisibility = options.minorUnitVisibility;
    return material;
  }

  createTriPlanarMaterial(name, options = {}) {
    const material = new TriPlanarMaterial(name, this.scene);
    if (options.diffuseTextureX) material.diffuseTextureX = options.diffuseTextureX;
    if (options.diffuseTextureY) material.diffuseTextureY = options.diffuseTextureY;
    if (options.diffuseTextureZ) material.diffuseTextureZ = options.diffuseTextureZ;
    if (options.normalTextureX) material.normalTextureX = options.normalTextureX;
    if (options.normalTextureY) material.normalTextureY = options.normalTextureY;
    if (options.normalTextureZ) material.normalTextureZ = options.normalTextureZ;
    if (options.tileSize !== undefined) material.tileSize = options.tileSize;
    return material;
  }

  createMixMaterial(name, options = {}) {
    const material = new MixMaterial(name, this.scene);
    if (options.texture1) material.texture1 = options.texture1;
    if (options.texture2) material.texture2 = options.texture2;
    if (options.texture3) material.texture3 = options.texture3;
    if (options.texture4) material.texture4 = options.texture4;
    if (options.mixTexture1) material.mixTexture1 = options.mixTexture1;
    if (options.mixTexture2) material.mixTexture2 = options.mixTexture2;
    return material;
  }

  createLavaMaterial(name, options = {}) {
    const material = new LavaMaterial(name, this.scene);
    if (options.speed !== undefined) material.speed = options.speed;
    if (options.fogColor) material.fogColor = new Color3(...options.fogColor);
    if (options.diffuseColor) material.diffuseColor = new Color3(...options.diffuseColor);
    return material;
  }

  createFireMaterial(name, options = {}) {
    const material = new FireMaterial(name, this.scene);
    if (options.speed !== undefined) material.speed = options.speed;
    if (options.diffuse) material.diffuse = new Color3(...options.diffuse);
    if (options.opacityFresnel !== undefined) material.opacityFresnel = options.opacityFresnel;
    return material;
  }

  createFurMaterial(name, options = {}) {
    const material = new FurMaterial(name, this.scene);
    if (options.furLength !== undefined) material.furLength = options.furLength;
    if (options.furAngle !== undefined) material.furAngle = options.furAngle;
    if (options.furColor) material.furColor = new Color3(...options.furColor);
    if (options.furOffset !== undefined) material.furOffset = options.furOffset;
    if (options.furSpacing !== undefined) material.furSpacing = options.furSpacing;
    if (options.furSpeed !== undefined) material.furSpeed = options.furSpeed;
    if (options.furDensity !== undefined) material.furDensity = options.furDensity;
    if (options.furTexture) material.furTexture = options.furTexture;
    return material;
  }

  createGradientMaterial(name, options = {}) {
    const material = new GradientMaterial(name, this.scene);
    if (options.topColor) material.topColor = new Color3(...options.topColor);
    if (options.bottomColor) material.bottomColor = new Color3(...options.bottomColor);
    if (options.offset !== undefined) material.offset = options.offset;
    if (options.scale !== undefined) material.scale = options.scale;
    if (options.smoothness !== undefined) material.smoothness = options.smoothness;
    return material;
  }

  // === MATERIAL PROPERTY SETTERS ===

  setMaterialProperty(material, property, value) {
    if (!material) return false;
    
    try {
      if (property.includes('Color') && Array.isArray(value)) {
        material[property] = new Color3(...value);
      } else if (property.includes('Color4') && Array.isArray(value)) {
        material[property] = new Color4(...value);
      } else if (property.includes('Vector') && Array.isArray(value)) {
        material[property] = new Vector2(...value);
      } else {
        material[property] = value;
      }
      return true;
    } catch (error) {
      console.warn('Failed to set material property:', property, error);
      return false;
    }
  }

  getMaterialProperty(material, property) {
    if (!material) return null;
    
    const value = material[property];
    if (value && typeof value === 'object') {
      // Convert Babylon objects back to arrays for script compatibility
      if (value.x !== undefined && value.y !== undefined && value.z !== undefined) {
        return [value.x, value.y, value.z];
      } else if (value.r !== undefined && value.g !== undefined && value.b !== undefined) {
        return value.a !== undefined ? [value.r, value.g, value.b, value.a] : [value.r, value.g, value.b];
      } else if (value.x !== undefined && value.y !== undefined) {
        return [value.x, value.y];
      }
    }
    return value;
  }

  // === MATERIAL UTILITY FUNCTIONS ===

  enableMaterialTransparency(material) {
    if (!material) return false;
    material.hasAlpha = true;
    material.useAlphaFromDiffuseTexture = true;
    return true;
  }

  setMaterialAlphaMode(material, mode) {
    if (!material) return false;
    // ALPHA_MODE: 0=OPAQUE, 1=CUTOFF, 2=BLEND, 3=PREMULTIPLIED
    material.transparencyMode = mode;
    return true;
  }

  setMaterialAlphaCutoff(material, cutoff) {
    if (!material) return false;
    material.alphaCutOff = cutoff;
    return true;
  }

  setMaterialDoubleSided(material, doubleSided) {
    if (!material) return false;
    material.backFaceCulling = !doubleSided;
    return true;
  }

  createFresnelParameters(leftColor, rightColor, bias = 0, power = 1, isEnabled = true) {
    return {
      leftColor: new Color3(...leftColor),
      rightColor: new Color3(...rightColor),
      bias,
      power,
      isEnabled
    };
  }

  setMaterialLevelOfDetail(material, distances = [10, 50, 100]) {
    if (!material || !material.addLODLevel) return false;
    
    distances.forEach((distance, index) => {
      if (index < distances.length - 1) {
        material.addLODLevel(distance, null);
      }
    });
    return true;
  }

  // === MATERIAL RENDERING PROPERTIES ===

  setBackFaceCulling(material, enabled) {
    if (!material) return false;
    material.backFaceCulling = enabled;
    return true;
  }

  setDisableLighting(material, disabled) {
    if (!material) return false;
    material.disableLighting = disabled;
    return true;
  }

  setWireframe(material, enabled) {
    if (!material) return false;
    material.wireframe = enabled;
    return true;
  }

  setPointsCloud(material, enabled) {
    if (!material) return false;
    material.pointsCloud = enabled;
    return true;
  }

  setFillMode(material, mode) {
    if (!material) return false;
    // MATERIAL.MATERIAL_POINTFILLMODE = 4
    // MATERIAL.MATERIAL_WIREFRAMEFILLMODE = 5  
    // MATERIAL.MATERIAL_SOLIDFILLMODE = 6
    material.fillMode = mode;
    return true;
  }

  setInvertNormalMapX(material, invert) {
    if (!material) return false;
    material.invertNormalMapX = invert;
    return true;
  }

  setInvertNormalMapY(material, invert) {
    if (!material) return false;
    material.invertNormalMapY = invert;
    return true;
  }

  setBumpLevel(material, level) {
    if (!material) return false;
    material.bumpTexture && (material.bumpTexture.level = level);
    return true;
  }

  setParallaxScaleBias(material, scale, bias) {
    if (!material) return false;
    material.parallaxScaleBias = scale;
    material.parallaxBias = bias;
    return true;
  }

  setIndexOfRefraction(material, ior) {
    if (!material) return false;
    material.indexOfRefraction = ior;
    return true;
  }

  setFresnelParameters(material, property, params) {
    if (!material) return false;
    material[property + 'FresnelParameters'] = params;
    return true;
  }

  // === MATERIAL ASSIGNMENT HELPERS ===

  applyMaterialToMesh(mesh, material) {
    if (!mesh || !material) return false;
    mesh.material = material;
    return true;
  }

  cloneMaterial(material, name) {
    if (!material || !material.clone) return null;
    return material.clone(name);
  }

  disposeMaterial(material) {
    if (!material || !material.dispose) return false;
    material.dispose();
    return true;
  }

  getMaterialInfo(material) {
    if (!material) return null;
    
    return {
      name: material.name,
      id: material.id,
      alpha: material.alpha,
      hasAlpha: material.hasAlpha,
      backFaceCulling: material.backFaceCulling,
      wireframe: material.wireframe,
      pointsCloud: material.pointsCloud,
      fillMode: material.fillMode,
      diffuseColor: material.diffuseColor ? [material.diffuseColor.r, material.diffuseColor.g, material.diffuseColor.b] : null,
      emissiveColor: material.emissiveColor ? [material.emissiveColor.r, material.emissiveColor.g, material.emissiveColor.b] : null,
      specularColor: material.specularColor ? [material.specularColor.r, material.specularColor.g, material.specularColor.b] : null
    };
  }
  
  // === SHORT NAME ALIASES ===
  
  materialProperty(material, property) {
    return this.getMaterialProperty(material, property);
  }
  
  materialInfo(material) {
    return this.getMaterialInfo(material);
  }
}