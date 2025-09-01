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
      // Try server-side processing first
      try {
        onProgress?.({ stage: 'server_processing', message: 'Processing on server...', progress: 10 });
        const serverResult = await modelProcessingAPI.processModel(file, settings, projectName, onProgress);
        
        // Perform detailed BabylonJS scene analysis after server processing
        onProgress?.({ stage: 'analyzing', message: 'Analyzing scene hierarchy...', progress: 60 });
        const sceneAnalysis = await this.performDeepSceneAnalysis(file, serverResult, settings);
        
        // Update the server-side summary with analysis data
        if (sceneAnalysis && serverResult.import_summary) {
          onProgress?.({ stage: 'updating_summary', message: 'Updating import summary...', progress: 85 });
          // The summary path is relative to the projects directory
          const basePath = serverResult.folder_structure.base_path;
          const sanitizedBaseName = serverResult.import_summary.sanitized_file.replace(/\.[^/.]+$/, "");
          const summaryPath = `${basePath}/${sanitizedBaseName}_import_summary.json`;
          await modelProcessingAPI.updateModelSummary(summaryPath, sceneAnalysis);
        }
        
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

  sanitizeNodeName(name) {
    return name
      .replace(/[^a-zA-Z0-9\-_.]/g, '_')
      .replace(/_{2,}/g, '_')
      .replace(/^_+|_+$/g, '')
      .toLowerCase();
  }

  async performDeepSceneAnalysis(file, serverResult, settings) {
    try {
      this.initTempScene();
      
      // Create blob URL for BabylonJS loading
      const blob = new Blob([file]);
      const blobUrl = URL.createObjectURL(blob);
      
      try {
        // Load the model into BabylonJS for analysis
        const importResult = await SceneLoader.ImportMeshAsync("", "", blobUrl, this.tempScene);
        
        // Build comprehensive scene analysis
        const sceneAnalysis = {
          mesh_hierarchy: this.analyzeMeshHierarchy(importResult),
          animation_catalog: this.analyzeAnimations(importResult),
          material_library: this.analyzeMaterials(importResult),
          texture_dependencies: this.analyzeTextures(importResult),
          bone_structure: this.analyzeSkeletons(importResult),
          scene_bounds: this.calculateSceneBounds(importResult),
          performance_metrics: this.calculatePerformanceMetrics(importResult),
          lod_levels: this.detectLodLevels(importResult),
          physics_assets: this.analyzePhysicsAssets(importResult),
        };
        
        URL.revokeObjectURL(blobUrl);
        return sceneAnalysis;
        
      } catch (babylonError) {
        console.warn('BabylonJS analysis failed:', babylonError);
        URL.revokeObjectURL(blobUrl);
        return null;
      }
      
    } catch (error) {
      console.error('Scene analysis failed:', error);
      return null;
    }
  }

  analyzeMeshHierarchy(importResult) {
    const meshNodes = [];
    const meshMap = new Map();
    
    // First pass: create mesh nodes and build map
    importResult.meshes.forEach(mesh => {
      const geometryInfo = this.analyzeGeometry(mesh);
      const transform = this.extractTransform(mesh);
      
      const meshNode = {
        name: mesh.name,
        sanitized_name: this.sanitizeNodeName(mesh.name),
        id: mesh.id,
        parent_id: mesh.parent ? mesh.parent.id : null,
        children_ids: [],
        transform,
        geometry_info: geometryInfo,
        material_assignments: mesh.material ? [mesh.material.id] : [],
        animation_targets: this.getAnimationTargets(mesh, importResult),
        physics_properties: this.analyzePhysicsProperties(mesh),
        lod_group: this.detectLodGroup(mesh),
        mesh_type: this.classifyMeshType(mesh),
      };
      
      meshNodes.push(meshNode);
      meshMap.set(mesh.id, meshNode);
    });
    
    // Second pass: populate children_ids
    meshNodes.forEach(node => {
      if (node.parent_id) {
        const parent = meshMap.get(node.parent_id);
        if (parent) {
          parent.children_ids.push(node.id);
        }
      }
    });
    
    return meshNodes;
  }

  analyzeGeometry(mesh) {
    const positions = mesh.getVerticesData('position') || [];
    const indices = mesh.getIndices() || [];
    const normals = mesh.getVerticesData('normal');
    const uvs = mesh.getVerticesData('uv');
    const tangents = mesh.getVerticesData('tangent');
    const colors = mesh.getVerticesData('color');
    
    const vertexCount = positions.length / 3;
    const faceCount = indices.length / 3;
    
    // Calculate bounding box
    const boundingInfo = mesh.getBoundingInfo();
    const boundingBox = {
      min: [boundingInfo.boundingBox.minimumWorld.x, boundingInfo.boundingBox.minimumWorld.y, boundingInfo.boundingBox.minimumWorld.z],
      max: [boundingInfo.boundingBox.maximumWorld.x, boundingInfo.boundingBox.maximumWorld.y, boundingInfo.boundingBox.maximumWorld.z],
      center: [boundingInfo.boundingBox.centerWorld.x, boundingInfo.boundingBox.centerWorld.y, boundingInfo.boundingBox.centerWorld.z],
      size: [
        boundingInfo.boundingBox.maximumWorld.x - boundingInfo.boundingBox.minimumWorld.x,
        boundingInfo.boundingBox.maximumWorld.y - boundingInfo.boundingBox.minimumWorld.y,
        boundingInfo.boundingBox.maximumWorld.z - boundingInfo.boundingBox.minimumWorld.z
      ]
    };
    
    // Calculate surface area and volume approximation
    const size = boundingBox.size;
    const surfaceArea = 2 * (size[0] * size[1] + size[1] * size[2] + size[0] * size[2]);
    const volume = size[0] * size[1] * size[2];
    
    return {
      vertex_count: vertexCount,
      face_count: faceCount,
      triangle_count: faceCount,
      has_uvs: !!uvs,
      has_normals: !!normals,
      has_tangents: !!tangents,
      has_vertex_colors: !!colors,
      vertex_attributes: this.getVertexAttributes(mesh),
      bounding_box: boundingBox,
      surface_area: surfaceArea,
      volume: volume,
    };
  }

  extractTransform(mesh) {
    const position = mesh.position;
    const rotation = mesh.rotationQuaternion || mesh.rotation.toQuaternion();
    const scaling = mesh.scaling;
    
    return {
      position: [position.x, position.y, position.z],
      rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
      scale: [scaling.x, scaling.y, scaling.z],
      local_matrix: mesh.getWorldMatrix().asArray(),
      world_matrix: mesh.computeWorldMatrix(true).asArray(),
    };
  }

  getVertexAttributes(mesh) {
    const attributes = [];
    if (mesh.getVerticesData('position')) attributes.push('position');
    if (mesh.getVerticesData('normal')) attributes.push('normal');
    if (mesh.getVerticesData('uv')) attributes.push('uv');
    if (mesh.getVerticesData('uv2')) attributes.push('uv2');
    if (mesh.getVerticesData('tangent')) attributes.push('tangent');
    if (mesh.getVerticesData('color')) attributes.push('color');
    if (mesh.getVerticesData('matricesIndices')) attributes.push('bone_indices');
    if (mesh.getVerticesData('matricesWeights')) attributes.push('bone_weights');
    return attributes;
  }

  getAnimationTargets(mesh, importResult) {
    const targets = [];
    if (importResult.animationGroups) {
      importResult.animationGroups.forEach(animGroup => {
        animGroup.targetedAnimations?.forEach(targetedAnim => {
          if (targetedAnim.target === mesh || targetedAnim.target?.name === mesh.name) {
            targets.push(animGroup.name);
          }
        });
      });
    }
    return [...new Set(targets)]; // Remove duplicates
  }

  analyzePhysicsProperties(mesh) {
    // Check for physics impostor or collision properties
    const hasCollision = !!mesh.physicsImpostor || mesh.name.toLowerCase().includes('collision') || mesh.name.toLowerCase().includes('physics');
    
    return hasCollision ? {
      has_collision: true,
      collision_type: this.detectCollisionType(mesh),
      is_static: !mesh.skeleton,
      mass: 1.0 // Default mass
    } : null;
  }

  detectCollisionType(mesh) {
    const name = mesh.name.toLowerCase();
    if (name.includes('box') || name.includes('cube')) return 'box';
    if (name.includes('sphere') || name.includes('ball')) return 'sphere';
    if (name.includes('capsule') || name.includes('cylinder')) return 'capsule';
    if (name.includes('convex')) return 'convex';
    return 'triangle_mesh';
  }

  detectLodGroup(mesh) {
    const name = mesh.name.toLowerCase();
    const lodMatch = name.match(/lod[_\-]?(\d+)/);
    return lodMatch ? `lod_group_${lodMatch[1]}` : null;
  }

  classifyMeshType(mesh) {
    if (mesh.skeleton) return 'Skeletal';
    if (mesh.instances && mesh.instances.length > 0) return 'Instanced';
    if (mesh.name.toLowerCase().includes('terrain')) return 'Terrain';
    if (mesh.name.toLowerCase().includes('particle')) return 'Particle';
    if (mesh.name.toLowerCase().includes('ui') || mesh.name.toLowerCase().includes('hud')) return 'UI';
    return 'Static';
  }

  analyzeAnimations(importResult) {
    const animations = [];
    
    if (importResult.animationGroups) {
      importResult.animationGroups.forEach(animGroup => {
        const duration = animGroup.to - animGroup.from;
        const frameRate = 30; // Default frame rate
        
        const animationTracks = [];
        if (animGroup.targetedAnimations) {
          animGroup.targetedAnimations.forEach(targetedAnim => {
            const track = {
              target_name: targetedAnim.target?.name || 'unknown',
              property: targetedAnim.animation?.property || 'unknown',
              keyframe_count: targetedAnim.animation?.getKeys()?.length || 0,
              interpolation: this.getInterpolationType(targetedAnim.animation),
              has_curves: this.hasCurves(targetedAnim.animation),
            };
            animationTracks.push(track);
          });
        }
        
        animations.push({
          name: animGroup.name,
          sanitized_name: this.sanitizeNodeName(animGroup.name),
          duration: duration / frameRate,
          frame_rate: frameRate,
          frame_count: Math.ceil(duration),
          start_frame: animGroup.from,
          end_frame: animGroup.to,
          target_meshes: this.getAnimationTargetMeshes(animGroup, importResult),
          target_bones: this.getAnimationTargetBones(animGroup, importResult),
          animation_tracks: animationTracks,
          is_looping: animGroup.loopAnimation || false,
          blend_mode: 'linear',
        });
      });
    }
    
    return animations;
  }

  getInterpolationType(animation) {
    if (!animation) return 'linear';
    // BabylonJS animation interpolation detection
    return 'linear'; // Default, could be enhanced to detect cubic, etc.
  }

  hasCurves(animation) {
    return animation && animation.getKeys && animation.getKeys().length > 2;
  }

  getAnimationTargetMeshes(animGroup, importResult) {
    const meshNames = [];
    if (animGroup.targetedAnimations) {
      animGroup.targetedAnimations.forEach(targetedAnim => {
        const target = targetedAnim.target;
        if (target && importResult.meshes.includes(target)) {
          meshNames.push(target.name);
        }
      });
    }
    return [...new Set(meshNames)];
  }

  getAnimationTargetBones(animGroup, importResult) {
    const boneNames = [];
    if (animGroup.targetedAnimations && importResult.skeletons) {
      animGroup.targetedAnimations.forEach(targetedAnim => {
        const target = targetedAnim.target;
        importResult.skeletons.forEach(skeleton => {
          if (skeleton.bones.find(bone => bone === target || bone.name === target?.name)) {
            boneNames.push(target.name);
          }
        });
      });
    }
    return [...new Set(boneNames)];
  }

  analyzeMaterials(importResult) {
    const materials = [];
    const materialMap = new Map();
    
    // Collect unique materials
    importResult.meshes.forEach(mesh => {
      if (mesh.material && !materialMap.has(mesh.material.id)) {
        materialMap.set(mesh.material.id, mesh.material);
      }
    });
    
    materialMap.forEach(material => {
      const textureSlots = this.analyzeTextureSlots(material);
      const properties = this.extractMaterialProperties(material);
      const transparency = this.analyzeTransparency(material);
      const assignedMeshes = this.getMeshesUsingMaterial(material, importResult);
      
      materials.push({
        name: material.name,
        sanitized_name: this.sanitizeNodeName(material.name),
        id: material.id,
        material_type: this.getMaterialType(material),
        shader_type: this.getShaderType(material),
        texture_slots: textureSlots,
        properties,
        transparency,
        assigned_to_meshes: assignedMeshes,
      });
    });
    
    return materials;
  }

  analyzeTextureSlots(material) {
    const slots = [];
    
    const textureTypes = [
      { prop: 'diffuseTexture', slot: 'diffuse' },
      { prop: 'normalTexture', slot: 'normal' },
      { prop: 'specularTexture', slot: 'specular' },
      { prop: 'emissiveTexture', slot: 'emissive' },
      { prop: 'metallicTexture', slot: 'metallic' },
      { prop: 'roughnessTexture', slot: 'roughness' },
      { prop: 'ambientTexture', slot: 'ambient' },
      { prop: 'opacityTexture', slot: 'opacity' },
      { prop: 'bumpTexture', slot: 'bump' },
    ];
    
    textureTypes.forEach(({ prop, slot }) => {
      const texture = material[prop];
      if (texture) {
        slots.push({
          slot_name: slot,
          texture_name: texture.name,
          texture_path: texture.url || '',
          uv_channel: texture.coordinatesIndex || 0,
          wrap_mode: this.getWrapMode(texture),
          filter_mode: this.getFilterMode(texture),
        });
      }
    });
    
    return slots;
  }

  extractMaterialProperties(material) {
    return {
      diffuse_color: material.diffuseColor ? [material.diffuseColor.r, material.diffuseColor.g, material.diffuseColor.b, material.alpha || 1.0] : [1, 1, 1, 1],
      specular_color: material.specularColor ? [material.specularColor.r, material.specularColor.g, material.specularColor.b] : [1, 1, 1],
      emissive_color: material.emissiveColor ? [material.emissiveColor.r, material.emissiveColor.g, material.emissiveColor.b] : [0, 0, 0],
      metallic: material.metallic || 0.0,
      roughness: material.roughness || 0.5,
      normal_scale: material.bumpTexture?.level || 1.0,
      opacity: material.alpha || 1.0,
    };
  }

  analyzeTransparency(material) {
    const isTransparent = material.alpha < 1.0 || material.hasAlpha || !!material.opacityTexture;
    return {
      is_transparent: isTransparent,
      blend_mode: this.getBlendMode(material),
      alpha_cutoff: material.alphaCutOff || 0.5,
      two_sided: material.backFaceCulling === false,
    };
  }

  getBlendMode(material) {
    if (material.transparencyMode === 1) return 'alpha_test';
    if (material.transparencyMode === 2) return 'alpha_blend';
    if (material.transparencyMode === 3) return 'alpha_to_coverage';
    return 'opaque';
  }

  getMeshesUsingMaterial(material, importResult) {
    return importResult.meshes
      .filter(mesh => mesh.material && mesh.material.id === material.id)
      .map(mesh => mesh.name);
  }

  getMaterialType(material) {
    if (material.getClassName) {
      const className = material.getClassName();
      if (className.includes('PBR')) return 'PBR';
      if (className.includes('Standard')) return 'Standard';
      if (className.includes('Unlit')) return 'Unlit';
    }
    return 'Standard';
  }

  getShaderType(material) {
    // Analyze shader complexity
    const textureCount = this.analyzeTextureSlots(material).length;
    if (textureCount > 5) return 'complex';
    if (textureCount > 2) return 'standard';
    if (textureCount > 0) return 'textured';
    return 'simple';
  }

  getWrapMode(texture) {
    // BabylonJS wrap mode analysis
    if (texture.wrapU === 1 && texture.wrapV === 1) return 'repeat';
    if (texture.wrapU === 0 && texture.wrapV === 0) return 'clamp';
    return 'mirror';
  }

  getFilterMode(texture) {
    // BabylonJS filter mode analysis
    if (texture.samplingMode === 1) return 'nearest';
    if (texture.samplingMode === 2) return 'linear';
    if (texture.samplingMode === 3) return 'trilinear';
    return 'linear';
  }

  analyzeTextures(importResult) {
    const textures = [];
    const textureMap = new Map();
    
    // Collect all unique textures from materials
    importResult.meshes.forEach(mesh => {
      if (mesh.material) {
        this.collectTexturesFromMaterial(mesh.material, textureMap);
      }
    });
    
    textureMap.forEach(texture => {
      const usedByMaterials = this.getMaterialsUsingTexture(texture, importResult);
      
      textures.push({
        texture_name: texture.name,
        sanitized_name: this.sanitizeNodeName(texture.name),
        file_path: texture.url || '',
        format: this.getTextureFormat(texture),
        dimensions: this.getTextureDimensions(texture),
        file_size: this.estimateTextureSize(texture),
        compression: this.getTextureCompression(texture),
        mip_levels: texture.generateMipMaps ? 8 : 1,
        used_by_materials: usedByMaterials,
      });
    });
    
    return textures;
  }

  collectTexturesFromMaterial(material, textureMap) {
    const textureProps = ['diffuseTexture', 'normalTexture', 'specularTexture', 'emissiveTexture', 'metallicTexture', 'roughnessTexture', 'ambientTexture', 'opacityTexture', 'bumpTexture'];
    
    textureProps.forEach(prop => {
      const texture = material[prop];
      if (texture && !textureMap.has(texture.name)) {
        textureMap.set(texture.name, texture);
      }
    });
  }

  getMaterialsUsingTexture(texture, importResult) {
    const materialNames = [];
    importResult.meshes.forEach(mesh => {
      if (mesh.material && this.materialUsesTexture(mesh.material, texture)) {
        materialNames.push(mesh.material.name);
      }
    });
    return [...new Set(materialNames)];
  }

  materialUsesTexture(material, texture) {
    const textureProps = ['diffuseTexture', 'normalTexture', 'specularTexture', 'emissiveTexture', 'metallicTexture', 'roughnessTexture', 'ambientTexture', 'opacityTexture', 'bumpTexture'];
    return textureProps.some(prop => material[prop] === texture);
  }

  getTextureFormat(texture) {
    if (texture.url) {
      const ext = texture.url.toLowerCase().split('.').pop();
      return ext || 'unknown';
    }
    return 'embedded';
  }

  getTextureDimensions(texture) {
    return [texture.getSize()?.width || 512, texture.getSize()?.height || 512];
  }

  estimateTextureSize(texture) {
    const [width, height] = this.getTextureDimensions(texture);
    const bitsPerPixel = this.getBitsPerPixel(texture);
    return Math.floor((width * height * bitsPerPixel) / 8);
  }

  getBitsPerPixel(texture) {
    // Estimate based on format
    if (texture.hasAlpha) return 32;
    return 24;
  }

  getTextureCompression(texture) {
    // Detect compression type
    if (texture.format && texture.format.includes('DXT')) return 'DXT';
    if (texture.format && texture.format.includes('ETC')) return 'ETC';
    if (texture.format && texture.format.includes('ASTC')) return 'ASTC';
    return 'uncompressed';
  }

  analyzeSkeletons(importResult) {
    if (!importResult.skeletons || importResult.skeletons.length === 0) return null;
    
    const skeleton = importResult.skeletons[0]; // Take first skeleton
    const boneHierarchy = this.buildBoneHierarchy(skeleton);
    const rootBones = boneHierarchy.filter(bone => !bone.parent_id);
    
    return {
      name: skeleton.name,
      bone_count: skeleton.bones.length,
      root_bones: rootBones,
      bone_hierarchy: boneHierarchy,
      bind_pose_transforms: this.extractBindPoseTransforms(skeleton),
      inverse_bind_matrices: this.extractInverseBindMatrices(skeleton),
    };
  }

  buildBoneHierarchy(skeleton) {
    const bones = [];
    const boneMap = new Map();
    
    // First pass: create bone nodes
    skeleton.bones.forEach(bone => {
      const transform = this.extractBoneTransform(bone);
      const boneInfo = {
        name: bone.name,
        id: bone.id,
        parent_id: bone.getParent() ? bone.getParent().id : null,
        children_ids: [],
        transform,
        influenced_vertices: this.countInfluencedVertices(bone, skeleton),
        weight_influence: this.calculateWeightInfluence(bone, skeleton),
      };
      
      bones.push(boneInfo);
      boneMap.set(bone.id, boneInfo);
    });
    
    // Second pass: populate children
    bones.forEach(boneInfo => {
      if (boneInfo.parent_id) {
        const parent = boneMap.get(boneInfo.parent_id);
        if (parent) {
          parent.children_ids.push(boneInfo.id);
        }
      }
    });
    
    return bones;
  }

  extractBoneTransform(bone) {
    const localMatrix = bone.getLocalMatrix();
    const worldMatrix = bone.getWorldMatrix();
    
    return {
      position: [bone.position.x, bone.position.y, bone.position.z],
      rotation: [bone.rotationQuaternion.x, bone.rotationQuaternion.y, bone.rotationQuaternion.z, bone.rotationQuaternion.w],
      scale: [bone.scaling.x, bone.scaling.y, bone.scaling.z],
      local_matrix: localMatrix.asArray(),
      world_matrix: worldMatrix.asArray(),
    };
  }

  countInfluencedVertices(bone, skeleton) {
    // Estimate influenced vertices (would need mesh data for exact count)
    return Math.floor(skeleton.bones.length > 0 ? 100 / skeleton.bones.length : 0);
  }

  calculateWeightInfluence(bone, skeleton) {
    // Calculate relative influence weight
    return skeleton.bones.length > 0 ? 1.0 / skeleton.bones.length : 1.0;
  }

  extractBindPoseTransforms(skeleton) {
    return skeleton.bones.map(bone => this.extractBoneTransform(bone));
  }

  extractInverseBindMatrices(skeleton) {
    return skeleton.bones.map(bone => {
      const invMatrix = bone.getInvertedAbsoluteTransform();
      return invMatrix ? invMatrix.asArray() : Array(16).fill(0);
    });
  }

  calculateSceneBounds(importResult) {
    let minX = Infinity, minY = Infinity, minZ = Infinity;
    let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity;
    
    importResult.meshes.forEach(mesh => {
      const boundingInfo = mesh.getBoundingInfo();
      const min = boundingInfo.boundingBox.minimumWorld;
      const max = boundingInfo.boundingBox.maximumWorld;
      
      minX = Math.min(minX, min.x);
      minY = Math.min(minY, min.y);
      minZ = Math.min(minZ, min.z);
      maxX = Math.max(maxX, max.x);
      maxY = Math.max(maxY, max.y);
      maxZ = Math.max(maxZ, max.z);
    });
    
    return {
      min: [minX, minY, minZ],
      max: [maxX, maxY, maxZ],
      center: [(minX + maxX) / 2, (minY + maxY) / 2, (minZ + maxZ) / 2],
      size: [maxX - minX, maxY - minY, maxZ - minZ],
    };
  }

  calculatePerformanceMetrics(importResult) {
    let totalVertices = 0;
    let totalTriangles = 0;
    const materials = new Set();
    const textures = new Set();
    
    importResult.meshes.forEach(mesh => {
      totalVertices += mesh.getTotalVertices();
      totalTriangles += mesh.getTotalIndices() / 3;
      
      if (mesh.material) {
        materials.add(mesh.material.id);
        this.collectTexturesFromMaterial(mesh.material, textures);
      }
    });
    
    const memoryEstimate = this.estimateMemoryUsage(totalVertices, totalTriangles, textures.size);
    const drawCalls = this.estimateDrawCalls(importResult);
    const complexity = this.calculateComplexityScore(totalVertices, totalTriangles, materials.size, textures.size);
    const suggestions = this.generateOptimizationSuggestions(totalVertices, totalTriangles, materials.size, textures.size);
    
    return {
      total_vertices: totalVertices,
      total_triangles: Math.floor(totalTriangles),
      total_materials: materials.size,
      total_textures: textures.size,
      memory_estimate_mb: memoryEstimate,
      draw_calls_estimate: drawCalls,
      complexity_score: complexity,
      optimization_suggestions: suggestions,
    };
  }

  estimateMemoryUsage(vertices, triangles, textureCount) {
    // Rough estimation: vertices (32 bytes each) + indices (6 bytes per triangle) + textures (1MB average each)
    const vertexMemory = vertices * 32;
    const indexMemory = triangles * 6;
    const textureMemory = textureCount * 1024 * 1024;
    return (vertexMemory + indexMemory + textureMemory) / (1024 * 1024);
  }

  estimateDrawCalls(importResult) {
    // Each unique material typically requires a separate draw call
    const materials = new Set();
    importResult.meshes.forEach(mesh => {
      if (mesh.material) materials.add(mesh.material.id);
    });
    return Math.max(materials.size, importResult.meshes.length);
  }

  calculateComplexityScore(vertices, triangles, materials, textures) {
    // Score from 0-100 based on asset complexity
    let score = 0;
    
    // Vertex complexity (0-30 points)
    if (vertices > 100000) score += 30;
    else if (vertices > 50000) score += 20;
    else if (vertices > 10000) score += 10;
    
    // Triangle complexity (0-30 points)
    if (triangles > 100000) score += 30;
    else if (triangles > 50000) score += 20;
    else if (triangles > 10000) score += 10;
    
    // Material complexity (0-20 points)
    if (materials > 10) score += 20;
    else if (materials > 5) score += 10;
    else if (materials > 2) score += 5;
    
    // Texture complexity (0-20 points)
    if (textures > 20) score += 20;
    else if (textures > 10) score += 15;
    else if (textures > 5) score += 10;
    else if (textures > 0) score += 5;
    
    return Math.min(score, 100);
  }

  generateOptimizationSuggestions(vertices, triangles, materials, textures) {
    const suggestions = [];
    
    if (vertices > 50000) {
      suggestions.push("High vertex count - consider LOD models or mesh decimation");
    }
    
    if (triangles > 50000) {
      suggestions.push("High triangle count - consider mesh optimization");
    }
    
    if (materials > 10) {
      suggestions.push("Many materials - consider texture atlasing to reduce draw calls");
    }
    
    if (textures > 15) {
      suggestions.push("Many textures - consider texture atlasing or compression");
    }
    
    if (materials > 0 && textures === 0) {
      suggestions.push("Materials without textures - consider baking lighting or adding detail maps");
    }
    
    return suggestions;
  }

  detectLodLevels(importResult) {
    const lodLevels = [];
    const lodGroups = new Map();
    
    // Group meshes by LOD patterns
    importResult.meshes.forEach(mesh => {
      const lodMatch = mesh.name.toLowerCase().match(/lod[_\-]?(\d+)/);
      if (lodMatch) {
        const level = parseInt(lodMatch[1]);
        const baseName = mesh.name.replace(/lod[_\-]?\d+/i, '').trim();
        
        if (!lodGroups.has(baseName)) {
          lodGroups.set(baseName, []);
        }
        lodGroups.get(baseName).push({ mesh, level });
      }
    });
    
    // Create LOD level data
    lodGroups.forEach((lods, baseName) => {
      lods.sort((a, b) => a.level - b.level);
      
      lods.forEach((lod, index) => {
        const baseVertices = lods[0].mesh.getTotalVertices();
        const currentVertices = lod.mesh.getTotalVertices();
        const vertexReduction = 1.0 - (currentVertices / baseVertices);
        
        lodLevels.push({
          level: lod.level,
          distance: Math.pow(2, lod.level) * 10, // Estimated distance
          vertex_reduction: vertexReduction,
          triangle_reduction: vertexReduction, // Approximate
          meshes: [lod.mesh.name],
        });
      });
    });
    
    return lodLevels;
  }

  analyzePhysicsAssets(importResult) {
    const physicsAssets = [];
    
    importResult.meshes.forEach(mesh => {
      if (mesh.physicsImpostor || mesh.name.toLowerCase().includes('collision') || mesh.name.toLowerCase().includes('physics')) {
        const boundingInfo = mesh.getBoundingInfo();
        const bounds = {
          min: [boundingInfo.boundingBox.minimumWorld.x, boundingInfo.boundingBox.minimumWorld.y, boundingInfo.boundingBox.minimumWorld.z],
          max: [boundingInfo.boundingBox.maximumWorld.x, boundingInfo.boundingBox.maximumWorld.y, boundingInfo.boundingBox.maximumWorld.z],
          center: [boundingInfo.boundingBox.centerWorld.x, boundingInfo.boundingBox.centerWorld.y, boundingInfo.boundingBox.centerWorld.z],
          size: [
            boundingInfo.boundingBox.maximumWorld.x - boundingInfo.boundingBox.minimumWorld.x,
            boundingInfo.boundingBox.maximumWorld.y - boundingInfo.boundingBox.minimumWorld.y,
            boundingInfo.boundingBox.maximumWorld.z - boundingInfo.boundingBox.minimumWorld.z
          ]
        };
        
        physicsAssets.push({
          name: mesh.name,
          collision_type: this.detectCollisionType(mesh),
          physics_material: 'default',
          mass: mesh.physicsImpostor?.mass || 1.0,
          friction: 0.5,
          restitution: 0.3,
          bounds,
        });
      }
    });
    
    return physicsAssets;
  }
}

export const modelProcessor = new ModelProcessor();