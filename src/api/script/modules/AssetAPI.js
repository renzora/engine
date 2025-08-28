// === ASSET API MODULE ===

import {
  SceneLoader,
  AssetsManager,
  MeshAssetTask,
  TextureAssetTask,
  CubeTextureAssetTask,
  HDRCubeTextureAssetTask,
  BinaryFileAssetTask,
  ImageAssetTask,
  TextFileAssetTask,
  AbstractMesh,
  Texture,
  AnimationGroup,
  Skeleton,
  Vector3,
  ImportMeshMode
} from '@babylonjs/core';

import '@babylonjs/loaders'; // Enable all loaders

export class AssetAPI {
  constructor(scene) {
    this.scene = scene;
    this.assetsManager = null;
    this.loadedAssets = new Map();
  }

  // === BASIC ASSET LOADING ===

  async loadMesh(name, rootUrl, fileName, options = {}) {
    try {
      const result = await SceneLoader.ImportMeshAsync(
        options.meshNames || '', 
        rootUrl, 
        fileName, 
        this.scene,
        options.onProgress,
        options.pluginExtension
      );
      
      this.loadedAssets.set(name, {
        type: 'mesh',
        meshes: result.meshes,
        particleSystems: result.particleSystems,
        skeletons: result.skeletons,
        animationGroups: result.animationGroups,
        transformNodes: result.transformNodes,
        geometries: result.geometries,
        lights: result.lights
      });
      
      return result;
    } catch (error) {
      console.error(`Failed to load mesh ${name}:`, error);
      return null;
    }
  }

  async loadGLTF(name, rootUrl, fileName, options = {}) {
    return this.loadMesh(name, rootUrl, fileName, {
      ...options,
      pluginExtension: '.gltf'
    });
  }

  async loadFBX(name, rootUrl, fileName, options = {}) {
    return this.loadMesh(name, rootUrl, fileName, {
      ...options,
      pluginExtension: '.fbx'
    });
  }

  async loadOBJ(name, rootUrl, fileName, options = {}) {
    return this.loadMesh(name, rootUrl, fileName, {
      ...options,
      pluginExtension: '.obj'
    });
  }

  async loadSTL(name, rootUrl, fileName, options = {}) {
    return this.loadMesh(name, rootUrl, fileName, {
      ...options,
      pluginExtension: '.stl'
    });
  }

  // === ASSET MANAGER ===

  createAssetsManager() {
    if (!this.assetsManager) {
      this.assetsManager = new AssetsManager(this.scene);
    }
    return this.assetsManager;
  }

  addMeshTask(name, meshNames, rootUrl, fileName) {
    const manager = this.createAssetsManager();
    const task = manager.addMeshTask(name, meshNames, rootUrl, fileName);
    
    task.onSuccess = (task) => {
      this.loadedAssets.set(name, {
        type: 'mesh',
        meshes: task.loadedMeshes,
        particleSystems: task.loadedParticleSystems,
        skeletons: task.loadedSkeletons,
        animationGroups: task.loadedAnimationGroups
      });
    };
    
    return task;
  }

  addTextureTask(name, url, noMipmap = false, invertY = true) {
    const manager = this.createAssetsManager();
    const task = manager.addTextureTask(name, url, noMipmap, invertY);
    
    task.onSuccess = (task) => {
      this.loadedAssets.set(name, {
        type: 'texture',
        texture: task.texture
      });
    };
    
    return task;
  }

  addCubeTextureTask(name, url, extensions = null) {
    const manager = this.createAssetsManager();
    const task = manager.addCubeTextureTask(name, url, extensions);
    
    task.onSuccess = (task) => {
      this.loadedAssets.set(name, {
        type: 'cubeTexture',
        texture: task.texture
      });
    };
    
    return task;
  }

  addBinaryFileTask(name, url) {
    const manager = this.createAssetsManager();
    const task = manager.addBinaryFileTask(name, url);
    
    task.onSuccess = (task) => {
      this.loadedAssets.set(name, {
        type: 'binary',
        data: task.data
      });
    };
    
    return task;
  }

  loadAllAssets() {
    if (!this.assetsManager) return Promise.resolve();
    
    return new Promise((resolve, reject) => {
      this.assetsManager.onFinish = (tasks) => {
        console.log(`Loaded ${tasks.length} assets`);
        resolve(tasks);
      };
      
      this.assetsManager.onTaskError = (task) => {
        console.error(`Failed to load asset: ${task.name}`, task.errorObject);
      };
      
      this.assetsManager.load();
    });
  }

  // === ASSET MANAGEMENT ===

  getLoadedAsset(name) {
    return this.loadedAssets.get(name) || null;
  }

  getLoadedMesh(assetName, meshName = null) {
    const asset = this.loadedAssets.get(assetName);
    if (!asset || asset.type !== 'mesh') return null;
    
    if (meshName) {
      return asset.meshes.find(mesh => mesh.name === meshName) || null;
    }
    return asset.meshes[0] || null;
  }

  getLoadedTexture(name) {
    const asset = this.loadedAssets.get(name);
    if (!asset || (asset.type !== 'texture' && asset.type !== 'cubeTexture')) return null;
    return asset.texture;
  }

  getLoadedAnimations(assetName) {
    const asset = this.loadedAssets.get(assetName);
    if (!asset || asset.type !== 'mesh') return [];
    return asset.animationGroups || [];
  }

  getLoadedSkeleton(assetName, skeletonIndex = 0) {
    const asset = this.loadedAssets.get(assetName);
    if (!asset || asset.type !== 'mesh' || !asset.skeletons) return null;
    return asset.skeletons[skeletonIndex] || null;
  }

  // === ASSET INSTANCES ===

  instantiateAsset(assetName, instanceName = null, position = [0, 0, 0], rotation = [0, 0, 0], scale = [1, 1, 1]) {
    const asset = this.loadedAssets.get(assetName);
    if (!asset || asset.type !== 'mesh') return null;
    
    const rootMesh = asset.meshes[0];
    if (!rootMesh) return null;
    
    const instance = rootMesh.createInstance(instanceName || `${assetName}_instance_${Date.now()}`);
    instance.position = new Vector3(...position);
    instance.rotation = new Vector3(...rotation);
    instance.scaling = new Vector3(...scale);
    
    return instance;
  }

  cloneAsset(assetName, cloneName = null, position = [0, 0, 0]) {
    const asset = this.loadedAssets.get(assetName);
    if (!asset || asset.type !== 'mesh') return null;
    
    const clonedMeshes = [];
    const namePrefix = cloneName || `${assetName}_clone_${Date.now()}`;
    
    asset.meshes.forEach((mesh, index) => {
      const cloned = mesh.clone(`${namePrefix}_${index}`);
      if (index === 0) {
        cloned.position = new Vector3(...position);
      }
      clonedMeshes.push(cloned);
    });
    
    return {
      meshes: clonedMeshes,
      root: clonedMeshes[0],
      animationGroups: asset.animationGroups ? asset.animationGroups.map(ag => ag.clone(namePrefix, ag.targetedAnimations.map(ta => ta.target))) : []
    };
  }

  // === BATCH LOADING ===

  async loadAssetBatch(assets) {
    const manager = this.createAssetsManager();
    
    assets.forEach(asset => {
      switch (asset.type) {
        case 'mesh':
          this.addMeshTask(asset.name, asset.meshNames || '', asset.rootUrl, asset.fileName);
          break;
        case 'texture':
          this.addTextureTask(asset.name, asset.url, asset.noMipmap, asset.invertY);
          break;
        case 'cubeTexture':
          this.addCubeTextureTask(asset.name, asset.url, asset.extensions);
          break;
        case 'binary':
          this.addBinaryFileTask(asset.name, asset.url);
          break;
      }
    });
    
    return this.loadAllAssets();
  }

  // === ASSET STREAMING ===

  async loadAssetProgressive(assetName, rootUrl, fileName, onProgress = null) {
    let loadedBytes = 0;
    let totalBytes = 0;
    
    const progressCallback = (event) => {
      if (event.lengthComputable) {
        loadedBytes = event.loaded;
        totalBytes = event.total;
        const progress = (loadedBytes / totalBytes) * 100;
        
        if (onProgress) {
          onProgress(progress, loadedBytes, totalBytes);
        }
      }
    };
    
    return this.loadMesh(assetName, rootUrl, fileName, {
      onProgress: progressCallback
    });
  }

  // === ASSET OPTIMIZATION ===

  optimizeAsset(assetName, options = {}) {
    const asset = this.loadedAssets.get(assetName);
    if (!asset || asset.type !== 'mesh') return false;
    
    asset.meshes.forEach(mesh => {
      // Merge vertices if requested
      if (options.mergeVertices !== false) {
        mesh.mergeVertices();
      }
      
      // Freeze world matrix for static meshes
      if (options.freezeWorldMatrix !== false) {
        mesh.freezeWorldMatrix();
      }
      
      // Convert to unindexed if small enough
      if (options.convertToUnindexed && mesh.getTotalVertices() < 65536) {
        mesh.convertToUnIndexedMesh();
      }
      
      // Optimize materials
      if (mesh.material && options.optimizeMaterials !== false) {
        mesh.material.freeze();
      }
    });
    
    return true;
  }

  // === ASSET PRELOADING ===

  preloadAssets(assetList, onProgress = null) {
    const manager = this.createAssetsManager();
    let completed = 0;
    
    assetList.forEach((asset, index) => {
      let task;
      
      switch (asset.type) {
        case 'mesh':
          task = manager.addMeshTask(asset.name, '', asset.rootUrl, asset.fileName);
          break;
        case 'texture':
          task = manager.addTextureTask(asset.name, asset.url);
          break;
        default:
          return;
      }
      
      task.onSuccess = () => {
        completed++;
        if (onProgress) {
          onProgress(completed / assetList.length, asset.name);
        }
      };
    });
    
    return this.loadAllAssets();
  }

  // === ASSET DISPOSAL ===

  disposeAsset(name) {
    const asset = this.loadedAssets.get(name);
    if (!asset) return false;
    
    switch (asset.type) {
      case 'mesh':
        asset.meshes?.forEach(mesh => mesh.dispose());
        asset.skeletons?.forEach(skeleton => skeleton.dispose());
        asset.animationGroups?.forEach(ag => ag.dispose());
        break;
      case 'texture':
      case 'cubeTexture':
        asset.texture?.dispose();
        break;
    }
    
    this.loadedAssets.delete(name);
    return true;
  }

  disposeAllAssets() {
    const assetNames = Array.from(this.loadedAssets.keys());
    assetNames.forEach(name => this.disposeAsset(name));
    return assetNames.length;
  }

  // === ASSET INFO ===

  getAssetInfo(name) {
    const asset = this.loadedAssets.get(name);
    if (!asset) return null;
    
    const info = {
      name,
      type: asset.type
    };
    
    switch (asset.type) {
      case 'mesh':
        info.meshCount = asset.meshes?.length || 0;
        info.skeletonCount = asset.skeletons?.length || 0;
        info.animationCount = asset.animationGroups?.length || 0;
        info.totalVertices = asset.meshes?.reduce((sum, mesh) => 
          sum + (mesh.getTotalVertices ? mesh.getTotalVertices() : 0), 0) || 0;
        break;
      case 'texture':
      case 'cubeTexture':
        if (asset.texture) {
          const size = asset.texture.getBaseSize();
          info.width = size.width;
          info.height = size.height;
          info.ready = asset.texture.isReady();
        }
        break;
      case 'binary':
        info.size = asset.data?.byteLength || 0;
        break;
    }
    
    return info;
  }

  getAllLoadedAssets() {
    const assets = [];
    this.loadedAssets.forEach((asset, name) => {
      assets.push(this.getAssetInfo(name));
    });
    return assets;
  }

  // === ASSET CACHING ===

  enableAssetCaching(enabled = true) {
    SceneLoader.ShowLoadingScreen = !enabled;
    return true;
  }

  clearAssetCache() {
    // Clear internal Babylon.js caches
    this.scene.getEngine().wipeCaches();
    return true;
  }

  // === ASSET VALIDATION ===

  validateAsset(assetName) {
    const asset = this.loadedAssets.get(assetName);
    if (!asset) return { valid: false, errors: ['Asset not found'] };
    
    const errors = [];
    
    switch (asset.type) {
      case 'mesh':
        if (!asset.meshes || asset.meshes.length === 0) {
          errors.push('No meshes loaded');
        }
        
        asset.meshes?.forEach((mesh, index) => {
          if (mesh.getTotalVertices && mesh.getTotalVertices() === 0) {
            errors.push(`Mesh ${index} has no vertices`);
          }
          if (!mesh.material) {
            errors.push(`Mesh ${index} has no material`);
          }
        });
        break;
      case 'texture':
        if (!asset.texture?.isReady()) {
          errors.push('Texture not ready');
        }
        break;
    }
    
    return {
      valid: errors.length === 0,
      errors
    };
  }

  // === ASSET TRANSFORMS ===

  transformLoadedAsset(assetName, transform = {}) {
    const asset = this.loadedAssets.get(assetName);
    if (!asset || asset.type !== 'mesh') return false;
    
    const rootMesh = asset.meshes[0];
    if (!rootMesh) return false;
    
    if (transform.position) {
      rootMesh.position = new Vector3(...transform.position);
    }
    if (transform.rotation) {
      rootMesh.rotation = new Vector3(...transform.rotation);
    }
    if (transform.scaling) {
      rootMesh.scaling = new Vector3(...transform.scaling);
    }
    
    return true;
  }

  centerAsset(assetName) {
    const asset = this.loadedAssets.get(assetName);
    if (!asset || asset.type !== 'mesh') return false;
    
    asset.meshes.forEach(mesh => {
      const boundingInfo = mesh.getBoundingInfo();
      const center = boundingInfo.boundingBox.center;
      mesh.position = mesh.position.subtract(center);
    });
    
    return true;
  }

  scaleAssetToSize(assetName, targetSize = 1.0) {
    const asset = this.loadedAssets.get(assetName);
    if (!asset || asset.type !== 'mesh') return false;
    
    const rootMesh = asset.meshes[0];
    if (!rootMesh) return false;
    
    const boundingInfo = rootMesh.getBoundingInfo();
    const size = boundingInfo.boundingBox.size;
    const maxDimension = Math.max(size.x, size.y, size.z);
    const scaleFactor = targetSize / maxDimension;
    
    asset.meshes.forEach(mesh => {
      mesh.scaling = mesh.scaling.scale(scaleFactor);
    });
    
    return true;
  }

  // === LOD MANAGEMENT ===

  createLODVersions(assetName, lodLevels = [10, 50, 100]) {
    const asset = this.loadedAssets.get(assetName);
    if (!asset || asset.type !== 'mesh') return false;
    
    const rootMesh = asset.meshes[0];
    if (!rootMesh) return false;
    
    lodLevels.forEach((distance, index) => {
      let lodMesh;
      
      if (index === lodLevels.length - 1) {
        // Furthest LOD can be null (invisible)
        lodMesh = null;
      } else {
        // Create simplified versions
        lodMesh = rootMesh.clone(`${rootMesh.name}_LOD_${index}`);
        
        // Simplify mesh for LOD
        const quality = Math.max(0.1, 1.0 - (index * 0.3));
        if (lodMesh.simplify) {
          lodMesh.simplify([{ quality, distance: 0.01 }], false, 0);
        }
      }
      
      rootMesh.addLODLevel(distance, lodMesh);
    });
    
    return true;
  }

  // === ASSET EXPORT ===

  async exportMesh(mesh, fileName, format = 'babylon') {
    if (!mesh) return null;
    
    try {
      let exported;
      switch (format.toLowerCase()) {
        case 'gltf':
          exported = await GLTF2Export.GLTFAsync(this.scene, fileName, {
            shouldExportMesh: (mesh) => mesh === mesh
          });
          break;
        case 'obj':
          exported = OBJExport.OBJ([mesh], false, fileName);
          break;
        case 'stl':
          exported = STLExport.CreateSTL([mesh], false, fileName);
          break;
        default:
          // Babylon format
          exported = SceneSerializer.SerializeMesh(mesh);
          break;
      }
      
      return exported;
    } catch (error) {
      console.error(`Failed to export mesh in ${format} format:`, error);
      return null;
    }
  }

  // === ASSET STREAMING ===

  enableAssetStreaming(distance = 50) {
    // Simple distance-based streaming
    const streamingCheck = () => {
      if (!this.scene.activeCamera) return;
      
      const cameraPosition = this.scene.activeCamera.position;
      
      this.loadedAssets.forEach((asset, name) => {
        if (asset.type === 'mesh' && asset.meshes) {
          asset.meshes.forEach(mesh => {
            const distance = Vector3.Distance(cameraPosition, mesh.position);
            mesh.setEnabled(distance <= distance);
          });
        }
      });
    };
    
    this.scene.registerBeforeRender(streamingCheck);
    return true;
  }

  // === ASSET UTILITIES ===

  getAllAssetNames() {
    return Array.from(this.loadedAssets.keys());
  }

  getAssetCount() {
    return this.loadedAssets.size;
  }

  getTotalMemoryUsage() {
    let totalBytes = 0;
    
    this.loadedAssets.forEach(asset => {
      switch (asset.type) {
        case 'mesh':
          asset.meshes?.forEach(mesh => {
            const vertexCount = mesh.getTotalVertices ? mesh.getTotalVertices() : 0;
            totalBytes += vertexCount * 32; // Rough estimate: 32 bytes per vertex
          });
          break;
        case 'texture':
          if (asset.texture) {
            const size = asset.texture.getBaseSize();
            totalBytes += size.width * size.height * 4; // 4 bytes per pixel (RGBA)
          }
          break;
        case 'binary':
          totalBytes += asset.data?.byteLength || 0;
          break;
      }
    });
    
    return totalBytes;
  }

  isAssetLoaded(name) {
    return this.loadedAssets.has(name);
  }

  waitForAsset(name, timeout = 10000) {
    return new Promise((resolve, reject) => {
      const startTime = Date.now();
      
      const check = () => {
        if (this.isAssetLoaded(name)) {
          resolve(this.getLoadedAsset(name));
        } else if (Date.now() - startTime > timeout) {
          reject(new Error(`Asset ${name} failed to load within timeout`));
        } else {
          setTimeout(check, 100);
        }
      };
      
      check();
    });
  }
}