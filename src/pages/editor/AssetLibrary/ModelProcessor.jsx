import { SceneLoader } from '@babylonjs/core/Loading/sceneLoader';
import { Scene } from '@babylonjs/core/scene';
import { NullEngine } from '@babylonjs/core/Engines/nullEngine';
import { bridgeService } from '@/plugins/core/bridge';
import { modelProcessingAPI } from '@/api/bridge/modelProcessing';
import '@babylonjs/loaders';

export class ModelProcessor {
  constructor() {
    this.tempEngine = null;
    this.tempScene = null;
  }

  initTempScene() {
    if (!this.tempEngine) {
      this.tempEngine = new NullEngine();
      this.tempScene = new Scene(this.tempEngine);
    }
  }

  dispose() {
    if (this.tempScene) {
      this.tempScene.dispose();
      this.tempScene = null;
    }
    if (this.tempEngine) {
      this.tempEngine.dispose();
      this.tempEngine = null;
    }
  }

  async processModelFile(file, settings, projectName, onProgress) {
    const fileName = file.name;
    const fileNameWithoutExt = fileName.replace(/\.[^/.]+$/, "");
    const ext = fileName.toLowerCase().match(/\.[^.]+$/)?.[0] || '';
    
    try {
      // Try server-side processing first (if bridge supports it)
      try {
        onProgress?.({ stage: 'server_processing', message: 'Processing on server...', progress: 10 });
        const serverResult = await modelProcessingAPI.processModel(file, settings, projectName, onProgress);
        return serverResult;
      } catch (serverError) {
        console.error('Server-side processing failed:', serverError);
        onProgress?.({ stage: 'fallback', message: 'Using client-side processing...', progress: 15 });
      }
      
      // Fallback to client-side processing - just upload and organize the files
      onProgress?.({ stage: 'uploading', message: `Uploading ${fileName}...`, progress: 20 });
      
      // Create folder structure based on settings
      const assetStructure = this.createAssetStructure(fileNameWithoutExt, {}, settings);
      
      onProgress?.({ stage: 'organizing', message: 'Creating folder structure...', progress: 40 });
      
      // Upload the original file
      const savedAssets = await this.saveOriginalFile(file, assetStructure, projectName, settings);
      
      onProgress?.({ stage: 'thumbnails', message: 'Generating thumbnails...', progress: 80 });
      
      // Generate thumbnail if it's a 3D model
      if (['.fbx', '.obj', '.gltf', '.glb', '.dae'].includes(ext)) {
        try {
          const assetPath = `${assetStructure.basePath}/${fileName}`.replace('assets/', '');
          await bridgeService.generateThumbnail(assetPath);
        } catch (error) {
          console.warn('Failed to generate thumbnail:', error);
        }
      }
      
      onProgress?.({ stage: 'complete', message: 'Import complete!', progress: 100 });
      
      return {
        success: true,
        analysis: {},
        assetStructure,
        savedAssets
      };
      
    } catch (error) {
      console.error('Model processing failed:', error);
      throw error;
    }
  }

  analyzeModel(importResult) {
    const analysis = {
      meshCount: importResult.meshes.length,
      animationCount: importResult.animationGroups?.length || 0,
      skeletonCount: importResult.skeletons?.length || 0,
      materialCount: 0,
      textureCount: 0,
      hasAnimations: (importResult.animationGroups?.length || 0) > 0,
      hasSkeleton: (importResult.skeletons?.length || 0) > 0,
      meshTypes: [],
      animations: [],
      materials: [],
      textures: []
    };

    // Analyze meshes
    importResult.meshes.forEach((mesh, index) => {
      const meshInfo = {
        name: mesh.name,
        id: mesh.id,
        vertexCount: mesh.getTotalVertices(),
        faceCount: mesh.getTotalIndices() / 3,
        hasSkeleton: !!mesh.skeleton,
        hasAnimations: mesh.animations?.length > 0,
        boundingInfo: mesh.getBoundingInfo()
      };
      
      analysis.meshTypes.push(meshInfo);
    });

    // Analyze animations
    if (importResult.animationGroups) {
      importResult.animationGroups.forEach(animGroup => {
        analysis.animations.push({
          name: animGroup.name,
          from: animGroup.from,
          to: animGroup.to,
          targetedAnimations: animGroup.targetedAnimations?.length || 0
        });
      });
    }

    // Analyze materials
    if (importResult.meshes) {
      const materialSet = new Set();
      const textureSet = new Set();
      
      importResult.meshes.forEach(mesh => {
        if (mesh.material) {
          materialSet.add(mesh.material);
          
          // Check for textures in material
          const material = mesh.material;
          if (material.diffuseTexture) textureSet.add(material.diffuseTexture);
          if (material.normalTexture) textureSet.add(material.normalTexture);
          if (material.specularTexture) textureSet.add(material.specularTexture);
          if (material.emissiveTexture) textureSet.add(material.emissiveTexture);
        }
      });
      
      analysis.materialCount = materialSet.size;
      analysis.textureCount = textureSet.size;
      
      materialSet.forEach(material => {
        analysis.materials.push({
          name: material.name,
          id: material.id,
          hasTextures: !!(material.diffuseTexture || material.normalTexture)
        });
      });
    }

    return analysis;
  }

  createAssetStructure(baseName, analysis, settings) {
    const structure = {
      basePath: '',
      folders: [],
      assets: []
    };

    // Determine base path
    let basePath = 'assets';
    
    if (settings.general.assetTypeSubFolders) {
      basePath += '/models';
    }
    
    if (settings.general.sceneNameSubFolder) {
      basePath += `/${baseName}`;
    }
    
    structure.basePath = basePath;

    // Create folders based on content
    if (analysis.hasAnimations && settings.animations.importAnimations) {
      structure.folders.push(`${basePath}/animations`);
    }
    
    if (analysis.materialCount > 0 && settings.materials.importTextures) {
      structure.folders.push(`${basePath}/materials`);
      structure.folders.push(`${basePath}/textures`);
    }
    
    if (settings.general.keepSectionsSeparate && analysis.meshCount > 1) {
      structure.folders.push(`${basePath}/meshes`);
    }

    return structure;
  }

  sanitizeFileName(name) {
    return name
      .replace(/[^a-zA-Z0-9\-_.]/g, '_')  // Replace invalid chars with underscore
      .replace(/_{2,}/g, '_')             // Replace multiple underscores with single
      .replace(/^_+|_+$/g, '')           // Remove leading/trailing underscores
      .toLowerCase();                     // Convert to lowercase
  }

  async saveOriginalFile(file, assetStructure, projectName, settings) {
    const originalFileName = file.name;
    const fileExtension = originalFileName.toLowerCase().match(/\.[^.]+$/)?.[0] || '';
    const baseNameWithoutExt = originalFileName.replace(/\.[^/.]+$/, "");
    const sanitizedBaseName = this.sanitizeFileName(baseNameWithoutExt);
    const sanitizedFileName = sanitizedBaseName + fileExtension;
    
    const savedAssets = [];
    
    // Update asset structure to use sanitized name for folder
    let sanitizedBasePath = assetStructure.basePath;
    if (settings.general.sceneNameSubFolder) {
      // Replace the original base name with sanitized version in the path
      sanitizedBasePath = sanitizedBasePath.replace(baseNameWithoutExt, sanitizedBaseName);
    }
    
    // Determine target path with sanitized names
    const targetPath = `projects/${projectName}/${sanitizedBasePath}/${sanitizedFileName}`;
    
    // Convert file to base64
    const reader = new FileReader();
    const base64 = await new Promise((resolve, reject) => {
      reader.onload = () => {
        const base64String = reader.result.split(',')[1];
        resolve(base64String);
      };
      reader.onerror = reject;
      reader.readAsDataURL(file);
    });
    
    // Upload to bridge server
    await bridgeService.writeBinaryFile(targetPath, base64);
    savedAssets.push({ type: 'original', path: targetPath });
    
    // Create import summary
    const summary = {
      originalFile: originalFileName,
      sanitizedFile: sanitizedFileName,
      importedAt: new Date().toISOString(),
      settings: settings,
      assets: savedAssets
    };

    const summaryPath = `projects/${projectName}/${sanitizedBasePath}/${sanitizedBaseName}_import_summary.json`;
    await bridgeService.writeFile(summaryPath, JSON.stringify(summary, null, 2));
    savedAssets.push({ type: 'summary', path: summaryPath });

    return savedAssets;
  }

  async saveProcessedAssets(originalFile, assetStructure, importResult, projectName, settings, onProgress) {
    const savedAssets = [];
    let progressCount = 0;
    const totalOperations = this.countTotalOperations(importResult, settings);

    // Create necessary folders
    for (const folderPath of assetStructure.folders) {
      try {
        await bridgeService.listDirectory(`projects/${projectName}/${folderPath}`);
      } catch {
        // Folder doesn't exist, but bridge will create it when we write files
      }
    }

    // Save original file
    onProgress?.(++progressCount / totalOperations);
    const originalPath = `projects/${projectName}/${assetStructure.basePath}/${originalFile.name}`;
    const reader = new FileReader();
    const base64 = await new Promise((resolve, reject) => {
      reader.onload = () => resolve(reader.result.split(',')[1]);
      reader.onerror = reject;
      reader.readAsDataURL(originalFile);
    });
    
    await bridgeService.writeBinaryFile(originalPath, base64);
    savedAssets.push({ type: 'original', path: originalPath });

    // Save individual meshes if keeping sections separate
    if (settings.general.keepSectionsSeparate && importResult.meshes.length > 1) {
      for (let i = 0; i < importResult.meshes.length; i++) {
        const mesh = importResult.meshes[i];
        onProgress?.(++progressCount / totalOperations);
        
        // Create mesh metadata
        const meshData = {
          name: mesh.name,
          id: mesh.id,
          vertexCount: mesh.getTotalVertices(),
          faceCount: mesh.getTotalIndices() / 3,
          hasSkeleton: !!mesh.skeleton,
          boundingBox: {
            min: mesh.getBoundingInfo().boundingBox.minimumWorld,
            max: mesh.getBoundingInfo().boundingBox.maximumWorld
          }
        };
        
        const meshMetaPath = `projects/${projectName}/${assetStructure.basePath}/meshes/${mesh.name}_info.json`;
        await bridgeService.writeFile(meshMetaPath, JSON.stringify(meshData, null, 2));
        savedAssets.push({ type: 'mesh_metadata', path: meshMetaPath, meshName: mesh.name });
      }
    }

    // Save animations
    if (settings.animations.importAnimations && importResult.animationGroups) {
      for (const animGroup of importResult.animationGroups) {
        onProgress?.(++progressCount / totalOperations);
        
        const animData = {
          name: animGroup.name,
          from: animGroup.from,
          to: animGroup.to,
          length: animGroup.to - animGroup.from,
          targetedAnimations: animGroup.targetedAnimations?.map(ta => ({
            target: ta.target?.name || 'unknown',
            property: ta.animation?.property || 'unknown',
            keys: ta.animation?.getKeys()?.length || 0
          })) || []
        };
        
        const animPath = `projects/${projectName}/${assetStructure.basePath}/animations/${animGroup.name}.json`;
        await bridgeService.writeFile(animPath, JSON.stringify(animData, null, 2));
        savedAssets.push({ type: 'animation', path: animPath, animationName: animGroup.name });
      }
    }

    // Save material information
    if (settings.materials.importTextures && importResult.meshes) {
      const materials = new Map();
      const textures = new Map();
      
      importResult.meshes.forEach(mesh => {
        if (mesh.material && !materials.has(mesh.material.id)) {
          materials.set(mesh.material.id, mesh.material);
          
          // Collect textures
          const material = mesh.material;
          if (material.diffuseTexture && !textures.has(material.diffuseTexture.name)) {
            textures.set(material.diffuseTexture.name, material.diffuseTexture);
          }
          if (material.normalTexture && !textures.has(material.normalTexture.name)) {
            textures.set(material.normalTexture.name, material.normalTexture);
          }
        }
      });

      // Save material definitions
      for (const [materialId, material] of materials) {
        onProgress?.(++progressCount / totalOperations);
        
        const materialData = {
          name: material.name,
          id: material.id,
          diffuseColor: material.diffuseColor,
          specularColor: material.specularColor,
          emissiveColor: material.emissiveColor,
          textures: {
            diffuse: material.diffuseTexture?.name || null,
            normal: material.normalTexture?.name || null,
            specular: material.specularTexture?.name || null,
            emissive: material.emissiveTexture?.name || null
          }
        };
        
        const materialPath = `projects/${projectName}/${assetStructure.basePath}/materials/${material.name}.json`;
        await bridgeService.writeFile(materialPath, JSON.stringify(materialData, null, 2));
        savedAssets.push({ type: 'material', path: materialPath, materialName: material.name });
      }
    }

    // Create import summary
    const summary = {
      originalFile: originalFile.name,
      importedAt: new Date().toISOString(),
      settings: settings,
      analysis: {
        meshCount: importResult.meshes.length,
        animationCount: importResult.animationGroups?.length || 0,
        materialCount: importResult.meshes ? new Set(importResult.meshes.filter(m => m.material).map(m => m.material.id)).size : 0
      },
      assets: savedAssets
    };

    const fileNameWithoutExt = originalFile.name.replace(/\.[^/.]+$/, "");
    const summaryPath = `projects/${projectName}/${assetStructure.basePath}/${fileNameWithoutExt}_import_summary.json`;
    await bridgeService.writeFile(summaryPath, JSON.stringify(summary, null, 2));
    savedAssets.push({ type: 'summary', path: summaryPath });

    return savedAssets;
  }

  countTotalOperations(importResult, settings) {
    let count = 1; // Original file
    
    if (settings.general.keepSectionsSeparate) {
      count += importResult.meshes.length; // Mesh metadata
    }
    
    if (settings.animations.importAnimations && importResult.animationGroups) {
      count += importResult.animationGroups.length; // Animation files
    }
    
    if (settings.materials.importTextures && importResult.meshes) {
      const materials = new Set(importResult.meshes.filter(m => m.material).map(m => m.material.id));
      count += materials.size; // Material files
    }
    
    count += 1; // Summary file
    
    return count;
  }
}

export const modelProcessor = new ModelProcessor();