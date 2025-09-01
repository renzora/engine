import { bridgeService } from '@/plugins/core/bridge';

export class ModelProcessingAPI {
  constructor() {
    this.apiPrefix = 'http://localhost:3001';
  }

  // Convert camelCase settings to snake_case for Rust server
  transformSettingsToSnakeCase(settings) {
    // Map of specific problematic conversions
    const specialCases = {
      'importUDIMs': 'import_udims',
      'importSparsVolumeTextures': 'import_sparse_volume_textures', 
      'importAnimatedSparseVolumeTextures': 'import_animated_sparse_volume_textures',
      'use30HzToBakeBoneAnimation': 'use_30hz_to_bake_bone_animation',
    };
    
    const camelToSnake = (str) => {
      // Check for special cases first
      if (specialCases[str]) {
        return specialCases[str];
      }
      
      // Handle regular camelCase conversion
      return str.replace(/[A-Z]/g, letter => `_${letter.toLowerCase()}`);
    };
    
    const transformObject = (obj) => {
      const result = {};
      for (const [key, value] of Object.entries(obj)) {
        const snakeKey = camelToSnake(key);
        
        if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
          // Handle special cases for nested objects that should become arrays
          if (key === 'frameImportRange' && value.start !== undefined && value.end !== undefined) {
            result[snakeKey] = [value.start, value.end];
          } else if (key === 'offsetTranslation' && value.x !== undefined) {
            result[snakeKey] = [value.x, value.y, value.z];
          } else if (key === 'offsetRotation' && value.x !== undefined) {
            result[snakeKey] = [value.x, value.y, value.z];
          } else {
            result[snakeKey] = transformObject(value);
          }
        } else {
          result[snakeKey] = value;
        }
      }
      return result;
    };
    
    return transformObject(settings);
  }

  async processModel(file, settings, projectName, onProgress) {
    try {
      onProgress?.({ stage: 'uploading', message: 'Uploading model for processing...', progress: 5 });
      
      // Convert file to base64
      const base64Data = await new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => {
          const base64String = reader.result.split(',')[1];
          resolve(base64String);
        };
        reader.onerror = reject;
        reader.readAsDataURL(file);
      });
      
      onProgress?.({ stage: 'processing', message: 'Processing model on server...', progress: 25 });
      
      // Send only the file - let Rust extract everything
      const requestData = {
        file_data: base64Data,
        filename: file.name,
        project_name: projectName
      };
      
      const response = await fetch(`${this.apiPrefix}/process-model`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(requestData)
      });
      
      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`HTTP error! status: ${response.status} - ${errorText}`);
      }
      
      onProgress?.({ stage: 'complete', message: 'Model processing complete!', progress: 100 });
      
      const result = await response.json();
      return result;
      
    } catch (error) {
      console.error('Model processing failed:', error);
      throw error;
    }
  }

  async extractMeshes(filePath, projectName, options = {}) {
    try {
      const response = await fetch(`${this.apiPrefix}/extract-meshes`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          file_path: filePath,
          project_name: projectName,
          separate_meshes: options.separateMeshes || false,
          extract_animations: options.extractAnimations || false,
          extract_materials: options.extractMaterials || false,
          draco_compression: options.dracoCompression || false
        })
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      return await response.json();
    } catch (error) {
      console.error('Mesh extraction failed:', error);
      throw error;
    }
  }

  async generateModelThumbnails(filePath, projectName, options = {}) {
    try {
      const response = await fetch(`${this.apiPrefix}/generate-model-thumbnails`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          file_path: filePath,
          project_name: projectName,
          size: options.size || 512,
          angles: options.angles || ['front', 'side', 'top'],
          background_color: options.backgroundColor || '#f0f0f0'
        })
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      return await response.json();
    } catch (error) {
      console.error('Thumbnail generation failed:', error);
      throw error;
    }
  }

  async optimizeModel(filePath, projectName, options = {}) {
    try {
      const response = await fetch(`${this.apiPrefix}/optimize-model`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          file_path: filePath,
          project_name: projectName,
          draco_compression: options.dracoCompression || false,
          quantization_bits: options.quantizationBits || 14,
          compression_level: options.compressionLevel || 7,
          optimize_textures: options.optimizeTextures || false,
          max_texture_size: options.maxTextureSize || 2048
        })
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      return await response.json();
    } catch (error) {
      console.error('Model optimization failed:', error);
      throw error;
    }
  }

  async analyzeModel(filePath, projectName) {
    try {
      const response = await fetch(`${this.apiPrefix}/analyze-model`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          file_path: filePath,
          project_name: projectName
        })
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      return await response.json();
    } catch (error) {
      console.error('Model analysis failed:', error);
      throw error;
    }
  }

  async updateModelSummary(summaryPath, sceneAnalysis) {
    try {
      const response = await fetch(`${this.apiPrefix}/update-model-summary`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          summary_path: summaryPath,
          scene_analysis: sceneAnalysis
        })
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`HTTP error! status: ${response.status} - ${errorText}`);
      }

      return await response.json();
    } catch (error) {
      console.error('Failed to update model summary:', error);
      throw error;
    }
  }
}

export const modelProcessingAPI = new ModelProcessingAPI();