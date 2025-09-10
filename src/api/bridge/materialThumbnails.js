import { bridgeService } from '@/plugins/core/bridge';

export class MaterialThumbnailAPI {
  constructor() {
    this.apiPrefix = 'http://localhost:3001';
  }

  async generateMaterialThumbnail(projectName, materialPath, size = 256) {
    try {
      const response = await fetch(`${this.apiPrefix}/material-thumbnail`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          project_name: projectName,
          material_path: materialPath,
          size: size
        })
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`HTTP error! status: ${response.status} - ${errorText}`);
      }

      return await response.json();
    } catch (error) {
      console.error('Material thumbnail generation failed:', error);
      throw error;
    }
  }
}

export const materialThumbnailAPI = new MaterialThumbnailAPI();

// Helper function to check if a file is a material file
export const isMaterialFile = (extension) => {
  const materialExtensions = ['.mat', '.material'];
  return materialExtensions.includes(extension?.toLowerCase() || '');
};

// Helper function to check if a file path indicates it's a material
export const isMaterialPath = (path) => {
  if (!path) return false;
  
  const pathLower = path.toLowerCase();
  
  // Check if it's a .material file anywhere
  if (pathLower.endsWith('.material')) {
    return true;
  }
  
  // Check if it's a .json file in a materials directory (but not project.json or other system files)
  if (pathLower.endsWith('.json') && 
      (pathLower.includes('/materials/') || pathLower.includes('\\materials\\'))) {
    
    // Exclude system files
    const filename = path.split(/[/\\]/).pop().toLowerCase();
    const systemFiles = ['project.json', 'package.json', 'scene.json', 'config.json'];
    
    return !systemFiles.includes(filename);
  }
  
  return false;
};

// Helper function to identify material files based on content structure
export const identifyMaterialFile = async (filePath) => {
  try {
    const content = await bridgeService.readFile(filePath);
    const data = JSON.parse(content);
    
    // Check if it has material-like properties
    return !!(
      data.name ||
      data.diffuseColor ||
      data.metallic !== undefined ||
      data.roughness !== undefined ||
      data.textures
    );
  } catch {
    return false;
  }
};