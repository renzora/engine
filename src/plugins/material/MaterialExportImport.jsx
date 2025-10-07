// Material Export/Import System
import { bridgeService } from '@/plugins/core/bridge';

export class MaterialExportImport {
  constructor() {
    this.version = '1.0.0';
  }

  // Export material data to JSON format
  exportMaterial(material, nodes = [], connections = [], metadata = {}) {
    const exportData = {
      version: this.version,
      timestamp: new Date().toISOString(),
      metadata: {
        name: material.name || 'Untitled Material',
        description: metadata.description || '',
        author: metadata.author || '',
        tags: metadata.tags || [],
        ...metadata
      },
      material: this.serializeMaterial(material),
      nodes: this.serializeNodes(nodes),
      connections: this.serializeConnections(connections),
      assets: this.extractAssetReferences(nodes)
    };

    return JSON.stringify(exportData, null, 2);
  }

  // Import material from JSON data
  async importMaterial(jsonData, scene) {
    try {
      const data = typeof jsonData === 'string' ? JSON.parse(jsonData) : jsonData;
      
      // Validate format
      if (!this.validateImportData(data)) {
        throw new Error('Invalid material file format');
      }

      // Check version compatibility
      if (!this.isVersionCompatible(data.version)) {
        console.warn(`Material version ${data.version} may not be fully compatible with current version ${this.version}`);
      }

      // Create material with nodes and connections
      const result = {
        metadata: data.metadata,
        material: await this.deserializeMaterial(data.material, scene),
        nodes: this.deserializeNodes(data.nodes),
        connections: this.deserializeConnections(data.connections),
        assets: data.assets || [],
        missingAssets: []
      };

      // Check for missing assets
      result.missingAssets = await this.checkMissingAssets(result.assets);

      return result;
    } catch (error) {
      throw new Error(`Failed to import material: ${error.message}`);
    }
  }

  // Export material to file
  async exportToFile(material, nodes, connections, filename, metadata = {}) {
    try {
      const exportData = this.exportMaterial(material, nodes, connections, metadata);
      const fileName = filename.endsWith('.rmat') ? filename : `${filename}.rmat`;
      
      // Use bridge service to save file
      await bridgeService.writeFile(`materials/${fileName}`, exportData);
      
      return {
        success: true,
        filename: fileName,
        path: `materials/${fileName}`
      };
    } catch (error) {
      return {
        success: false,
        error: error.message
      };
    }
  }

  // Import material from file
  async importFromFile(filePath, scene) {
    try {
      const fileContent = await bridgeService.readFile(filePath);
      const result = await this.importMaterial(fileContent, scene);
      
      return {
        success: true,
        ...result
      };
    } catch (error) {
      return {
        success: false,
        error: error.message
      };
    }
  }

  // List available material files
  async listMaterialFiles() {
    try {
      const files = await bridgeService.listFiles('materials', '.rmat');
      return files.map(file => ({
        name: file.name,
        path: file.path,
        size: file.size,
        modified: file.modified
      }));
    } catch (error) {
      console.warn('Could not list material files:', error);
      return [];
    }
  }

  // Serialize material properties
  serializeMaterial(material) {
    const serialized = {
      name: material.name,
      id: material.id,
      type: material.getClassName ? material.getClassName() : 'StandardMaterial',
      properties: {}
    };

    // Common properties
    if (material.alpha !== undefined) serialized.properties.alpha = material.alpha;
    if (material.backFaceCulling !== undefined) serialized.properties.backFaceCulling = material.backFaceCulling;
    if (material.wireframe !== undefined) serialized.properties.wireframe = material.wireframe;

    // Standard Material properties
    if (material.diffuseColor) {
      serialized.properties.diffuseColor = [material.diffuseColor.r, material.diffuseColor.g, material.diffuseColor.b];
    }
    if (material.specularColor) {
      serialized.properties.specularColor = [material.specularColor.r, material.specularColor.g, material.specularColor.b];
    }
    if (material.emissiveColor) {
      serialized.properties.emissiveColor = [material.emissiveColor.r, material.emissiveColor.g, material.emissiveColor.b];
    }
    if (material.specularPower !== undefined) serialized.properties.specularPower = material.specularPower;

    // PBR Material properties
    if (material.baseColor) {
      serialized.properties.baseColor = [material.baseColor.r, material.baseColor.g, material.baseColor.b];
    }
    if (material.metallic !== undefined) serialized.properties.metallic = material.metallic;
    if (material.roughness !== undefined) serialized.properties.roughness = material.roughness;
    if (material.metallicFactor !== undefined) serialized.properties.metallicFactor = material.metallicFactor;
    if (material.roughnessFactor !== undefined) serialized.properties.roughnessFactor = material.roughnessFactor;

    // Node Material properties
    if (material.mode !== undefined) serialized.properties.mode = material.mode;

    return serialized;
  }

  // Serialize nodes data
  serializeNodes(nodes) {
    return nodes.map(node => ({
      id: node.id,
      type: node.type,
      position: node.position,
      title: node.title,
      inputs: node.inputs?.map(input => ({
        id: input.id,
        name: input.name,
        type: input.type,
        value: this.serializeValue(input.value),
        asset: input.asset ? {
          name: input.asset.name,
          path: input.asset.path,
          type: input.asset.type
        } : null
      })) || [],
      outputs: node.outputs?.map(output => ({
        id: output.id,
        name: output.name,
        type: output.type
      })) || [],
      asset: node.asset ? {
        name: node.asset.name,
        path: node.asset.path,
        type: node.asset.type
      } : null
    }));
  }

  // Serialize connections data
  serializeConnections(connections) {
    return connections.map(conn => ({
      id: conn.id,
      from: {
        nodeId: conn.from.nodeId,
        socketId: conn.from.socketId
      },
      to: {
        nodeId: conn.to.nodeId,
        socketId: conn.to.socketId
      }
    }));
  }

  // Extract asset references from nodes
  extractAssetReferences(nodes) {
    const assets = [];
    
    nodes.forEach(node => {
      if (node.asset) {
        assets.push({
          name: node.asset.name,
          path: node.asset.path,
          type: node.asset.type,
          nodeId: node.id
        });
      }
      
      node.inputs?.forEach(input => {
        if (input.asset) {
          assets.push({
            name: input.asset.name,
            path: input.asset.path,
            type: input.asset.type,
            nodeId: node.id,
            inputId: input.id
          });
        }
      });
    });

    // Remove duplicates
    return assets.filter((asset, index, self) => 
      index === self.findIndex(a => a.path === asset.path)
    );
  }

  // Serialize different value types
  serializeValue(value) {
    if (value === null || value === undefined) return null;
    
    if (typeof value === 'object' && value.constructor) {
      // Handle Babylon.js objects
      if (value.r !== undefined && value.g !== undefined && value.b !== undefined) {
        // Color3 or Color4
        return {
          type: 'Color',
          value: value.a !== undefined ? [value.r, value.g, value.b, value.a] : [value.r, value.g, value.b]
        };
      } else if (value.x !== undefined && value.y !== undefined) {
        // Vector2 or Vector3
        if (value.z !== undefined) {
          return {
            type: 'Vector3',
            value: [value.x, value.y, value.z]
          };
        } else {
          return {
            type: 'Vector2',
            value: [value.x, value.y]
          };
        }
      } else if (Array.isArray(value)) {
        return {
          type: 'Array',
          value: value
        };
      }
    }
    
    return {
      type: typeof value,
      value: value
    };
  }

  // Deserialize material from data
  async deserializeMaterial(materialData, _scene) {
    // This would need to be implemented based on the specific material system
    // For now, return the serialized data
    return materialData;
  }

  // Deserialize nodes data
  deserializeNodes(nodesData) {
    return nodesData.map(nodeData => ({
      ...nodeData,
      inputs: nodeData.inputs?.map(input => ({
        ...input,
        value: this.deserializeValue(input.value)
      })) || [],
      outputs: nodeData.outputs || []
    }));
  }

  // Deserialize connections data
  deserializeConnections(connectionsData) {
    return connectionsData;
  }

  // Deserialize values back to their original types
  deserializeValue(serializedValue) {
    if (!serializedValue || typeof serializedValue !== 'object') return serializedValue;
    
    const { type, value } = serializedValue;
    
    switch (type) {
      case 'Color':
        // Would need to import Color3/Color4 from Babylon.js
        return value;
      case 'Vector2':
      case 'Vector3':
        // Would need to import Vector2/Vector3 from Babylon.js
        return value;
      case 'Array':
        return value;
      default:
        return value;
    }
  }

  // Check for missing assets
  async checkMissingAssets(assets) {
    const missingAssets = [];
    
    for (const asset of assets) {
      try {
        await bridgeService.readFile(asset.path);
      } catch {
        missingAssets.push(asset);
      }
    }
    
    return missingAssets;
  }

  // Validate import data structure
  validateImportData(data) {
    return (
      data &&
      typeof data === 'object' &&
      data.version &&
      data.material &&
      Array.isArray(data.nodes) &&
      Array.isArray(data.connections)
    );
  }

  // Check version compatibility
  isVersionCompatible(version) {
    const [major, minor] = version.split('.').map(Number);
    const [currentMajor, currentMinor] = this.version.split('.').map(Number);
    
    // Compatible if major version matches and minor version is not more than 1 higher
    return major === currentMajor && minor <= currentMinor + 1;
  }

  // Export material as template for the preset system
  exportAsTemplate(material, nodes, connections, templateInfo) {
    const template = {
      name: templateInfo.name,
      description: templateInfo.description,
      category: templateInfo.category,
      author: templateInfo.author,
      tags: templateInfo.tags || [],
      thumbnail: templateInfo.thumbnail,
      nodes: this.serializeNodes(nodes),
      connections: this.serializeConnections(connections),
      defaultValues: this.extractDefaultValues(nodes)
    };

    return template;
  }

  // Extract default values from nodes for template usage
  extractDefaultValues(nodes) {
    const defaults = {};
    
    nodes.forEach(node => {
      node.inputs?.forEach(input => {
        if (input.value !== null && input.value !== undefined) {
          defaults[`${node.id}.${input.id}`] = this.serializeValue(input.value);
        }
      });
    });

    return defaults;
  }
}

export default MaterialExportImport;