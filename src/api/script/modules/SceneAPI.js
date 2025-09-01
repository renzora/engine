import { Ray } from '@babylonjs/core/Culling/ray.js';
import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';
import { PickingInfo } from '@babylonjs/core/Collisions/pickingInfo.js';
import { Tags } from '@babylonjs/core/Misc/tags.js';

/**
 * SceneAPI - Scene queries, raycasting, and object management
 * Priority: HIGH - Essential for object interaction and queries
 */
export class SceneAPI {
  constructor(scene, babylonObject) {
    this.scene = scene;
    this.babylonObject = babylonObject;
  }

  // === OBJECT QUERIES ===
  
  findObjectByName(name) {
    if (!this.scene) return null;
    
    // Search meshes
    let found = this.scene.meshes.find(mesh => mesh.name === name);
    if (found) return found;
    
    // Search lights
    found = this.scene.lights.find(light => light.name === name);
    if (found) return found;
    
    // Search cameras
    found = this.scene.cameras.find(camera => camera.name === name);
    if (found) return found;
    
    return null;
  }

  findObjectsByName(name) {
    if (!this.scene) return [];
    
    const objects = [];
    
    // Search meshes
    objects.push(...this.scene.meshes.filter(mesh => mesh.name === name));
    
    // Search lights
    objects.push(...this.scene.lights.filter(light => light.name === name));
    
    // Search cameras
    objects.push(...this.scene.cameras.filter(camera => camera.name === name));
    
    return objects;
  }

  findObjectsByTag(tag) {
    if (!this.scene) return [];
    
    const objects = [];
    
    // Search meshes
    this.scene.meshes.forEach(mesh => {
      if (mesh.metadata?.tags?.has(tag)) {
        objects.push(mesh);
      }
    });
    
    // Search lights
    this.scene.lights.forEach(light => {
      if (light.metadata?.tags?.has(tag)) {
        objects.push(light);
      }
    });
    
    // Search cameras
    this.scene.cameras.forEach(camera => {
      if (camera.metadata?.tags?.has(tag)) {
        objects.push(camera);
      }
    });
    
    return objects;
  }

  findObjectsWithTag(tag) {
    return this.findObjectsByTag(tag);
  }

  getAllMeshes() {
    return this.scene?.meshes || [];
  }

  getAllLights() {
    return this.scene?.lights || [];
  }

  getAllCameras() {
    return this.scene?.cameras || [];
  }

  // === RAYCASTING ===
  
  raycast(originX, originY, originZ, directionX, directionY, directionZ, maxDistance = Infinity) {
    if (!this.scene) return null;
    
    const origin = new Vector3(originX, originY, originZ);
    const direction = new Vector3(directionX, directionY, directionZ).normalize();
    const ray = new Ray(origin, direction, maxDistance);
    
    const hit = this.scene.pickWithRay(ray);
    
    if (hit && hit.hit) {
      return {
        hit: true,
        object: hit.pickedMesh,
        point: [hit.pickedPoint.x, hit.pickedPoint.y, hit.pickedPoint.z],
        normal: hit.getNormal ? [hit.getNormal().x, hit.getNormal().y, hit.getNormal().z] : [0, 1, 0],
        distance: hit.distance,
        uv: hit.getTextureCoordinates ? [hit.getTextureCoordinates().x, hit.getTextureCoordinates().y] : [0, 0]
      };
    }
    
    return { hit: false };
  }

  raycastFromCamera(screenX, screenY, camera = null) {
    if (!this.scene) return null;
    
    const cam = camera || this.scene.activeCamera;
    if (!cam) return null;
    
    const hit = this.scene.pick(screenX, screenY);
    
    if (hit && hit.hit) {
      return {
        hit: true,
        object: hit.pickedMesh,
        point: [hit.pickedPoint.x, hit.pickedPoint.y, hit.pickedPoint.z],
        normal: hit.getNormal ? [hit.getNormal().x, hit.getNormal().y, hit.getNormal().z] : [0, 1, 0],
        distance: hit.distance,
        uv: hit.getTextureCoordinates ? [hit.getTextureCoordinates().x, hit.getTextureCoordinates().y] : [0, 0]
      };
    }
    
    return { hit: false };
  }

  multiRaycast(originX, originY, originZ, directionX, directionY, directionZ, maxDistance = Infinity) {
    if (!this.scene) return [];
    
    const origin = new Vector3(originX, originY, originZ);
    const direction = new Vector3(directionX, directionY, directionZ).normalize();
    const ray = new Ray(origin, direction, maxDistance);
    
    const hits = this.scene.multiPickWithRay(ray);
    
    return hits.map(hit => ({
      hit: hit.hit,
      object: hit.pickedMesh,
      point: hit.pickedPoint ? [hit.pickedPoint.x, hit.pickedPoint.y, hit.pickedPoint.z] : [0, 0, 0],
      normal: hit.getNormal ? [hit.getNormal().x, hit.getNormal().y, hit.getNormal().z] : [0, 1, 0],
      distance: hit.distance || 0,
      uv: hit.getTextureCoordinates ? [hit.getTextureCoordinates().x, hit.getTextureCoordinates().y] : [0, 0]
    }));
  }

  // === OBJECT PICKING ===
  
  pickObject(screenX, screenY) {
    if (!this.scene) return null;
    
    const pickInfo = this.scene.pick(screenX, screenY);
    return pickInfo?.pickedMesh || null;
  }

  pickObjects(screenX, screenY) {
    if (!this.scene) return [];
    
    const pickInfos = this.scene.multiPick(screenX, screenY);
    return pickInfos?.map(info => info.pickedMesh).filter(mesh => mesh) || [];
  }

  // === SPATIAL QUERIES ===
  
  getObjectsInRadius(centerX, centerY, centerZ, radius) {
    if (!this.scene) return [];
    
    const center = new Vector3(centerX, centerY, centerZ);
    const objects = [];
    
    this.scene.meshes.forEach(mesh => {
      if (!mesh.position) return;
      
      const distance = Vector3.Distance(center, mesh.position);
      if (distance <= radius) {
        objects.push({
          object: mesh,
          distance: distance,
          position: [mesh.position.x, mesh.position.y, mesh.position.z]
        });
      }
    });
    
    // Sort by distance
    objects.sort((a, b) => a.distance - b.distance);
    
    return objects;
  }

  getObjectsInBox(minX, minY, minZ, maxX, maxY, maxZ) {
    if (!this.scene) return [];
    
    const objects = [];
    
    this.scene.meshes.forEach(mesh => {
      if (!mesh.position) return;
      
      const pos = mesh.position;
      if (pos.x >= minX && pos.x <= maxX &&
          pos.y >= minY && pos.y <= maxY &&
          pos.z >= minZ && pos.z <= maxZ) {
        objects.push({
          object: mesh,
          position: [pos.x, pos.y, pos.z]
        });
      }
    });
    
    return objects;
  }

  getClosestObject(targetX, targetY, targetZ, tag = null) {
    if (!this.scene) return null;
    
    const target = new Vector3(targetX, targetY, targetZ);
    let closest = null;
    let closestDistance = Infinity;
    
    this.scene.meshes.forEach(mesh => {
      if (!mesh.position) return;
      
      // Check tag filter
      if (tag && !mesh.metadata?.tags?.has(tag)) return;
      
      const distance = Vector3.Distance(target, mesh.position);
      if (distance < closestDistance) {
        closest = mesh;
        closestDistance = distance;
      }
    });
    
    return closest ? {
      object: closest,
      distance: closestDistance,
      position: [closest.position.x, closest.position.y, closest.position.z]
    } : null;
  }

  // === MESH INTERSECTION ===
  
  intersectsMesh(otherMesh) {
    if (!this.babylonObject || !otherMesh) return false;
    
    if (this.babylonObject.intersectsMesh && otherMesh.intersectsMesh) {
      return this.babylonObject.intersectsMesh(otherMesh);
    }
    
    return false;
  }

  intersectsPoint(x, y, z) {
    if (!this.babylonObject) return false;
    
    const point = new Vector3(x, y, z);
    
    if (this.babylonObject.intersectsPoint) {
      return this.babylonObject.intersectsPoint(point);
    }
    
    return false;
  }

  getBoundingInfo() {
    if (!this.babylonObject?.getBoundingInfo) return null;
    
    const boundingInfo = this.babylonObject.getBoundingInfo();
    const min = boundingInfo.minimum;
    const max = boundingInfo.maximum;
    
    return {
      min: [min.x, min.y, min.z],
      max: [max.x, max.y, max.z],
      center: [(min.x + max.x) / 2, (min.y + max.y) / 2, (min.z + max.z) / 2],
      size: [max.x - min.x, max.y - min.y, max.z - min.z]
    };
  }

  // === SCENE MANAGEMENT ===
  
  disposeObject(object = null) {
    const target = object || this.babylonObject;
    if (target?.dispose) {
      target.dispose();
    }
  }

  cloneObject(name = null, parent = null) {
    if (!this.babylonObject?.clone) return null;
    
    const cloneName = name || `${this.babylonObject.name}_clone`;
    const cloned = this.babylonObject.clone(cloneName, parent);
    
    return cloned;
  }

  // === VISIBILITY CULLING ===
  
  isInCameraView(camera = null) {
    if (!this.babylonObject) return false;
    
    const cam = camera || this.scene.activeCamera;
    if (!cam) return false;
    
    // This is a simplified check - full implementation would be more complex
    if (this.babylonObject.isInFrustum) {
      return this.babylonObject.isInFrustum(cam);
    }
    
    return true; // Fallback
  }

  setOcclusionQuery(enabled) {
    if (!this.babylonObject) return;
    
    if (this.babylonObject.occlusionQueryAlgorithmType !== undefined) {
      this.babylonObject.occlusionQueryAlgorithmType = enabled ? 
        this.babylonObject.OCCLUSION_ALGORITHM_TYPE_ACCURATE : 
        this.babylonObject.OCCLUSION_ALGORITHM_TYPE_CONSERVATIVE;
    }
  }

  // === LEVEL OF DETAIL (LOD) ===
  
  addLODLevel(distance, lodMesh) {
    if (!this.babylonObject?.addLODLevel || !lodMesh) return;
    
    return this.babylonObject.addLODLevel(distance, lodMesh);
  }

  removeLODLevel(lodMesh) {
    if (!this.babylonObject?.removeLODLevel || !lodMesh) return;
    
    return this.babylonObject.removeLODLevel(lodMesh);
  }

  // === METADATA MANAGEMENT ===
  
  setMetadata(key, value) {
    if (!this.babylonObject) return;
    
    if (!this.babylonObject.metadata) {
      this.babylonObject.metadata = {};
    }
    
    this.babylonObject.metadata[key] = value;
  }

  getMetadata(key) {
    if (!this.babylonObject?.metadata) return null;
    return this.babylonObject.metadata[key];
  }

  hasMetadata(key) {
    if (!this.babylonObject?.metadata) return false;
    return key in this.babylonObject.metadata;
  }

  removeMetadata(key) {
    if (!this.babylonObject?.metadata) return;
    delete this.babylonObject.metadata[key];
  }

  // === SCENE STATISTICS ===
  
  getSceneInfo() {
    if (!this.scene) return {};
    
    return {
      meshCount: this.scene.meshes.length,
      lightCount: this.scene.lights.length,
      cameraCount: this.scene.cameras.length,
      materialCount: this.scene.materials.length,
      textureCount: this.scene.textures.length,
      animationGroupCount: this.scene.animationGroups.length,
      totalVertices: this.scene.getTotalVertices(),
      totalIndices: this.scene.getTotalIndices(),
      drawCalls: this.scene.getActiveIndices(),
      frameRate: this.scene.getEngine().getFps()
    };
  }

  // === PERFORMANCE MONITORING ===
  
  enablePerformanceMonitor() {
    if (this.scene?.performanceMonitor) {
      this.scene.performanceMonitor.enable();
    }
  }

  disablePerformanceMonitor() {
    if (this.scene?.performanceMonitor) {
      this.scene.performanceMonitor.disable();
    }
  }

  getPerformanceData() {
    if (!this.scene?.performanceMonitor) return {};
    
    const monitor = this.scene.performanceMonitor;
    return {
      averageFrameTime: monitor.averageFrameTime,
      instantaneousFrameTime: monitor.instantaneousFrameTime,
      averageFrameTimeVariance: monitor.averageFrameTimeVariance,
      instantaneousFrameTimeVariance: monitor.instantaneousFrameTimeVariance,
      isEnabled: monitor.isEnabled
    };
  }
  
  // === SHORT NAME ALIASES ===
  
  allMeshes() {
    return this.getAllMeshes();
  }
  
  allLights() {
    return this.getAllLights();
  }
  
  allCameras() {
    return this.getAllCameras();
  }
  
  objectsInRadius(centerX, centerY, centerZ, radius) {
    return this.getObjectsInRadius(centerX, centerY, centerZ, radius);
  }
  
  objectsInBox(minX, minY, minZ, maxX, maxY, maxZ) {
    return this.getObjectsInBox(minX, minY, minZ, maxX, maxY, maxZ);
  }
  
  closestObject(targetX, targetY, targetZ, tag = null) {
    return this.getClosestObject(targetX, targetY, targetZ, tag);
  }
  
  boundingInfo() {
    return this.getBoundingInfo();
  }
  
  metadata(key) {
    return this.getMetadata(key);
  }
  
  sceneInfo() {
    return this.getSceneInfo();
  }
  
  performanceData() {
    return this.getPerformanceData();
  }
}