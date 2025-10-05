// Material Presets and Templates System
import { Color3 } from '@babylonjs/core/Maths/math.color';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial';
import { PBRMaterial } from '@babylonjs/core/Materials/PBR/pbrMaterial';

export class MaterialPresets {
  constructor(scene) {
    this.scene = scene;
  }

  // Standard Material Presets
  getStandardPresets() {
    return {
      // Basic Materials
      plastic: {
        name: 'Plastic',
        category: 'Basic',
        description: 'Smooth plastic surface with moderate shininess',
        properties: {
          diffuseColor: [0.8, 0.8, 0.8],
          specularColor: [0.9, 0.9, 0.9],
          emissiveColor: [0, 0, 0],
          specularPower: 64,
          alpha: 1.0
        }
      },
      
      metal: {
        name: 'Metal',
        category: 'Basic',
        description: 'Reflective metal surface',
        properties: {
          diffuseColor: [0.6, 0.6, 0.7],
          specularColor: [1, 1, 1],
          emissiveColor: [0, 0, 0],
          specularPower: 128,
          alpha: 1.0
        }
      },
      
      rubber: {
        name: 'Rubber',
        category: 'Basic',
        description: 'Matte rubber surface with no shine',
        properties: {
          diffuseColor: [0.3, 0.3, 0.3],
          specularColor: [0.1, 0.1, 0.1],
          emissiveColor: [0, 0, 0],
          specularPower: 4,
          alpha: 1.0
        }
      },
      
      // Colored Materials
      redPlastic: {
        name: 'Red Plastic',
        category: 'Colored',
        description: 'Bright red plastic material',
        properties: {
          diffuseColor: [0.9, 0.2, 0.2],
          specularColor: [0.8, 0.8, 0.8],
          emissiveColor: [0, 0, 0],
          specularPower: 64,
          alpha: 1.0
        }
      },
      
      blueMetal: {
        name: 'Blue Metal',
        category: 'Colored',
        description: 'Metallic blue surface',
        properties: {
          diffuseColor: [0.2, 0.4, 0.8],
          specularColor: [1, 1, 1],
          emissiveColor: [0, 0, 0],
          specularPower: 128,
          alpha: 1.0
        }
      },
      
      greenGlass: {
        name: 'Green Glass',
        category: 'Colored',
        description: 'Translucent green glass',
        properties: {
          diffuseColor: [0.2, 0.8, 0.4],
          specularColor: [1, 1, 1],
          emissiveColor: [0, 0, 0],
          specularPower: 128,
          alpha: 0.6
        }
      },
      
      // Special Effects
      neon: {
        name: 'Neon',
        category: 'Effects',
        description: 'Glowing neon material',
        properties: {
          diffuseColor: [0.1, 1, 0.8],
          specularColor: [0.5, 0.5, 0.5],
          emissiveColor: [0.05, 0.5, 0.4],
          specularPower: 32,
          alpha: 1.0
        }
      },
      
      hologram: {
        name: 'Hologram',
        category: 'Effects',
        description: 'Translucent holographic material',
        properties: {
          diffuseColor: [0.4, 0.8, 1],
          specularColor: [1, 1, 1],
          emissiveColor: [0.1, 0.2, 0.3],
          specularPower: 64,
          alpha: 0.3
        }
      },
      
      lava: {
        name: 'Lava',
        category: 'Effects',
        description: 'Hot glowing lava material',
        properties: {
          diffuseColor: [0.8, 0.2, 0.1],
          specularColor: [1, 0.5, 0.2],
          emissiveColor: [0.4, 0.1, 0.05],
          specularPower: 16,
          alpha: 1.0
        }
      }
    };
  }

  // PBR Material Presets
  getPBRPresets() {
    return {
      // Metals
      gold: {
        name: 'Gold',
        category: 'Metals',
        description: 'Realistic gold material',
        type: 'pbr',
        properties: {
          baseColor: [1, 0.766, 0.336],
          metallic: 1.0,
          roughness: 0.1,
          emissiveColor: [0, 0, 0],
          alpha: 1.0
        }
      },
      
      silver: {
        name: 'Silver',
        category: 'Metals',
        description: 'Polished silver material',
        type: 'pbr',
        properties: {
          baseColor: [0.972, 0.960, 0.915],
          metallic: 1.0,
          roughness: 0.05,
          emissiveColor: [0, 0, 0],
          alpha: 1.0
        }
      },
      
      copper: {
        name: 'Copper',
        category: 'Metals',
        description: 'Weathered copper material',
        type: 'pbr',
        properties: {
          baseColor: [0.955, 0.637, 0.538],
          metallic: 1.0,
          roughness: 0.3,
          emissiveColor: [0, 0, 0],
          alpha: 1.0
        }
      },
      
      // Non-metals
      concrete: {
        name: 'Concrete',
        category: 'Building',
        description: 'Rough concrete surface',
        type: 'pbr',
        properties: {
          baseColor: [0.6, 0.6, 0.6],
          metallic: 0.0,
          roughness: 0.9,
          emissiveColor: [0, 0, 0],
          alpha: 1.0
        }
      },
      
      wood: {
        name: 'Wood',
        category: 'Natural',
        description: 'Natural wood material',
        type: 'pbr',
        properties: {
          baseColor: [0.6, 0.4, 0.2],
          metallic: 0.0,
          roughness: 0.7,
          emissiveColor: [0, 0, 0],
          alpha: 1.0
        }
      },
      
      fabric: {
        name: 'Fabric',
        category: 'Soft',
        description: 'Soft fabric material',
        type: 'pbr',
        properties: {
          baseColor: [0.5, 0.5, 0.8],
          metallic: 0.0,
          roughness: 0.8,
          emissiveColor: [0, 0, 0],
          alpha: 1.0
        }
      }
    };
  }

  // Create material from preset
  createMaterialFromPreset(presetKey, materialName) {
    const standardPresets = this.getStandardPresets();
    const pbrPresets = this.getPBRPresets();
    
    let preset = standardPresets[presetKey] || pbrPresets[presetKey];
    if (!preset) return null;
    
    let material;
    
    if (preset.type === 'pbr') {
      material = new PBRMaterial(materialName || preset.name, this.scene);
      
      // Set PBR properties
      if (preset.properties.baseColor) {
        material.baseColor = new Color3(...preset.properties.baseColor);
      }
      if (preset.properties.metallic !== undefined) {
        material.metallic = preset.properties.metallic;
      }
      if (preset.properties.roughness !== undefined) {
        material.roughness = preset.properties.roughness;
      }
      if (preset.properties.emissiveColor) {
        material.emissiveColor = new Color3(...preset.properties.emissiveColor);
      }
      if (preset.properties.alpha !== undefined) {
        material.alpha = preset.properties.alpha;
      }
    } else {
      // Standard Material
      material = new StandardMaterial(materialName || preset.name, this.scene);
      
      // Set Standard properties
      if (preset.properties.diffuseColor) {
        material.diffuseColor = new Color3(...preset.properties.diffuseColor);
      }
      if (preset.properties.specularColor) {
        material.specularColor = new Color3(...preset.properties.specularColor);
      }
      if (preset.properties.emissiveColor) {
        material.emissiveColor = new Color3(...preset.properties.emissiveColor);
      }
      if (preset.properties.specularPower !== undefined) {
        material.specularPower = preset.properties.specularPower;
      }
      if (preset.properties.alpha !== undefined) {
        material.alpha = preset.properties.alpha;
      }
    }
    
    return material;
  }

  // Get all presets organized by category
  getAllPresetsByCategory() {
    const standardPresets = this.getStandardPresets();
    const pbrPresets = this.getPBRPresets();
    const allPresets = { ...standardPresets, ...pbrPresets };
    
    const categories = {};
    
    Object.entries(allPresets).forEach(([key, preset]) => {
      if (!categories[preset.category]) {
        categories[preset.category] = [];
      }
      categories[preset.category].push({ key, ...preset });
    });
    
    return categories;
  }

  // Material Templates for node materials
  getNodeMaterialTemplates() {
    return {
      // Basic Templates
      basicLit: {
        name: 'Basic Lit',
        description: 'Simple lit material with diffuse and normal',
        category: 'Basic',
        nodes: [
          {
            type: 'input',
            blockType: 'texture2d',
            name: 'Diffuse Texture',
            position: { x: 100, y: 100 }
          },
          {
            type: 'input',
            blockType: 'texture2d',
            name: 'Normal Texture',
            position: { x: 100, y: 200 }
          },
          {
            type: 'output',
            blockType: 'fragment',
            name: 'Fragment Output',
            position: { x: 400, y: 150 }
          }
        ],
        connections: [
          { from: 'Diffuse Texture.rgb', to: 'Fragment Output.baseColor' },
          { from: 'Normal Texture.rgb', to: 'Fragment Output.normal' }
        ]
      },
      
      metallicRoughness: {
        name: 'Metallic Roughness',
        description: 'PBR material with metallic and roughness maps',
        category: 'PBR',
        nodes: [
          {
            type: 'input',
            blockType: 'texture2d',
            name: 'Albedo',
            position: { x: 100, y: 100 }
          },
          {
            type: 'input',
            blockType: 'texture2d',
            name: 'MetallicRoughness',
            position: { x: 100, y: 200 }
          },
          {
            type: 'input',
            blockType: 'texture2d',
            name: 'Normal',
            position: { x: 100, y: 300 }
          },
          {
            type: 'output',
            blockType: 'fragment',
            name: 'Fragment Output',
            position: { x: 500, y: 200 }
          }
        ],
        connections: [
          { from: 'Albedo.rgb', to: 'Fragment Output.baseColor' },
          { from: 'MetallicRoughness.b', to: 'Fragment Output.metallic' },
          { from: 'MetallicRoughness.g', to: 'Fragment Output.roughness' },
          { from: 'Normal.rgb', to: 'Fragment Output.normal' }
        ]
      },
      
      animated: {
        name: 'Animated Material',
        description: 'Material with time-based animation',
        category: 'Effects',
        nodes: [
          {
            type: 'input',
            blockType: 'time',
            name: 'Time',
            position: { x: 50, y: 100 }
          },
          {
            type: 'math',
            blockType: 'multiply',
            name: 'Speed',
            position: { x: 200, y: 100 }
          },
          {
            type: 'math',
            blockType: 'sin',
            name: 'Wave',
            position: { x: 350, y: 100 }
          },
          {
            type: 'output',
            blockType: 'fragment',
            name: 'Fragment Output',
            position: { x: 500, y: 150 }
          }
        ],
        connections: [
          { from: 'Time.output', to: 'Speed.a' },
          { from: 'Speed.output', to: 'Wave.input' },
          { from: 'Wave.output', to: 'Fragment Output.emissiveColor' }
        ]
      }
    };
  }

  // Quick color palette for material editor
  getColorPalette() {
    return {
      // Primary Colors
      primary: [
        '#FF0000', '#00FF00', '#0000FF', '#FFFF00', '#FF00FF', '#00FFFF'
      ],
      
      // Neutral Colors
      neutral: [
        '#FFFFFF', '#C0C0C0', '#808080', '#404040', '#000000'
      ],
      
      // Material Colors
      metals: [
        '#FFD700', // Gold
        '#C0C0C0', // Silver
        '#B87333', // Copper
        '#4682B4', // Steel Blue
        '#2F4F4F'  // Dark Slate Gray
      ],
      
      // Natural Colors
      natural: [
        '#8B4513', // Saddle Brown (Wood)
        '#A0522D', // Sienna (Clay)
        '#556B2F', // Dark Olive Green
        '#708090', // Slate Gray (Stone)
        '#F5DEB3'  // Wheat (Sand)
      ],
      
      // Effect Colors
      effects: [
        '#00FFFF', // Cyan (Neon)
        '#FF1493', // Deep Pink (Energy)
        '#7FFF00', // Chartreuse (Toxic)
        '#FF4500', // Orange Red (Fire)
        '#9370DB'  // Medium Purple (Magic)
      ]
    };
  }

  // Material property validation
  validateMaterialProperties(properties, materialType = 'standard') {
    const errors = [];
    
    if (materialType === 'pbr') {
      // PBR specific validation
      if (properties.metallic !== undefined && (properties.metallic < 0 || properties.metallic > 1)) {
        errors.push('Metallic value must be between 0 and 1');
      }
      if (properties.roughness !== undefined && (properties.roughness < 0 || properties.roughness > 1)) {
        errors.push('Roughness value must be between 0 and 1');
      }
    } else {
      // Standard material validation
      if (properties.specularPower !== undefined && properties.specularPower < 0) {
        errors.push('Specular power must be positive');
      }
    }
    
    // Common validation
    if (properties.alpha !== undefined && (properties.alpha < 0 || properties.alpha > 1)) {
      errors.push('Alpha value must be between 0 and 1');
    }
    
    return errors;
  }
}

export default MaterialPresets;