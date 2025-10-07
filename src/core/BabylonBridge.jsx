import { engineStore, engineActions } from '@/stores/EngineStore.jsx';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { TransformNode } from '@babylonjs/core/Meshes/transformNode';
import { Quaternion } from '@babylonjs/core/Maths/math.vector';

// System managers removed - using engine store for system management
import { ComponentRegistry } from './components/ComponentRegistry.jsx';

/**
 * Bridge between the engine and Babylon.js
 * Manages the lifecycle of Babylon.js objects and exposes the full API
 */
export class BabylonBridge {
  constructor(scene) {
    this.scene = scene;
    this.engine = scene.getEngine();
    
    // Object mapping between engine and babylon
    this.objectMap = new Map(); // engineId -> babylonObject
    this.engineMap = new Map(); // babylonObject -> engineId
    
    // Component registry for handling different object types
    this.componentRegistry = new ComponentRegistry();
    
    // Performance tracking
    this.performanceStats = {
      lastFrameTime: 0,
      drawCalls: 0,
      triangles: 0,
      activeObjects: 0
    };
    
    // Setup performance monitoring
    this.setupPerformanceMonitoring();
    
    console.log('🌉 BabylonBridge: Initialized with scene');
  }

  /**
   * Create a Babylon.js object from engine object data
   */
  createBabylonObject(engineObject, parentBabylonObject = null) {
    try {
      // Create the base transform node
      const babylonNode = new TransformNode(engineObject.name, this.scene);
      
      // Set parent if provided
      if (parentBabylonObject) {
        babylonNode.parent = parentBabylonObject;
      }
      
      // Apply transform data
      this.applyTransform(babylonNode, engineObject.transform);
      
      // Create and add components
      const componentInstances = {};
      Object.entries(engineObject.components || {}).forEach(([componentType, componentData]) => {
        const component = this.createComponent(babylonNode, componentType, componentData);
        if (component) {
          componentInstances[componentType] = component;
        }
      });
      
      // Store component references on the babylon object
      babylonNode.engineComponents = componentInstances;
      babylonNode.engineData = engineObject;
      
      // Register the mapping
      this.objectMap.set(engineObject.id, babylonNode);
      this.engineMap.set(babylonNode, engineObject.id);
      
      // Register with engine store for runtime access
      engineActions.registerBabylonObject(engineObject.id, babylonNode);
      
      // Apply metadata
      this.applyMetadata(babylonNode, engineObject.metadata);
      
      console.log(`🔨 Created Babylon object: ${engineObject.name} (${engineObject.id})`);
      return babylonNode;
      
    } catch (error) {
      console.error(`❌ Failed to create Babylon object for ${engineObject.id}:`, error);
      return null;
    }
  }

  /**
   * Update an existing Babylon.js object from engine data
   */
  updateBabylonObject(engineObjectId, updates) {
    const babylonObject = this.objectMap.get(engineObjectId);
    if (!babylonObject) {
      console.warn(`⚠️ Cannot update: Babylon object not found for ${engineObjectId}`);
      return;
    }

    try {
      // Update transform if provided
      if (updates.transform) {
        this.applyTransform(babylonObject, updates.transform);
      }

      // Update components if provided
      if (updates.components) {
        Object.entries(updates.components).forEach(([componentType, componentData]) => {
          this.updateComponent(babylonObject, componentType, componentData);
        });
      }

      // Update metadata if provided
      if (updates.metadata) {
        this.applyMetadata(babylonObject, updates.metadata);
      }

      // Update the stored engine data
      if (babylonObject.engineData) {
        babylonObject.engineData = { ...babylonObject.engineData, ...updates };
      }

      console.log(`🔄 Updated Babylon object: ${engineObjectId}`);

    } catch (error) {
      console.error(`❌ Failed to update Babylon object ${engineObjectId}:`, error);
    }
  }

  /**
   * Remove a Babylon.js object
   */
  removeBabylonObject(engineObjectId) {
    const babylonObject = this.objectMap.get(engineObjectId);
    if (!babylonObject) return;

    try {
      // Dispose components first
      if (babylonObject.engineComponents) {
        Object.values(babylonObject.engineComponents).forEach(component => {
          if (component.dispose && typeof component.dispose === 'function') {
            component.dispose();
          }
        });
      }

      // Remove from mappings
      this.objectMap.delete(engineObjectId);
      this.engineMap.delete(babylonObject);
      
      // Unregister from engine store
      engineActions.unregisterBabylonObject(engineObjectId);

      // Dispose the Babylon.js object
      babylonObject.dispose();
      
      console.log(`🗑️ Removed Babylon object: ${engineObjectId}`);

    } catch (error) {
      console.error(`❌ Failed to remove Babylon object ${engineObjectId}:`, error);
    }
  }

  /**
   * Apply transform data to a Babylon.js object
   */
  applyTransform(babylonObject, transform) {
    if (!transform) return;

    // Position
    if (transform.position) {
      babylonObject.position = new Vector3(
        transform.position[0] || 0,
        transform.position[1] || 0,
        transform.position[2] || 0
      );
    }

    // Rotation - convert from euler angles to quaternion if needed
    if (transform.rotation) {
      if (Array.isArray(transform.rotation)) {
        // Euler angles [x, y, z] in radians
        babylonObject.rotation = new Vector3(
          transform.rotation[0] || 0,
          transform.rotation[1] || 0,
          transform.rotation[2] || 0
        );
      } else if (transform.rotation.w !== undefined) {
        // Quaternion
        babylonObject.rotationQuaternion = new Quaternion(
          transform.rotation.x || 0,
          transform.rotation.y || 0,
          transform.rotation.z || 0,
          transform.rotation.w || 1
        );
      }
    }

    // Scale
    if (transform.scale) {
      babylonObject.scaling = new Vector3(
        transform.scale[0] || 1,
        transform.scale[1] || 1,
        transform.scale[2] || 1
      );
    }
  }

  /**
   * Apply metadata to a Babylon.js object
   */
  applyMetadata(babylonObject, metadata) {
    if (!metadata) return;

    // Visibility
    if (metadata.visible !== undefined) {
      babylonObject.setEnabled(metadata.visible);
    }

    // Tags
    if (metadata.tags && Array.isArray(metadata.tags)) {
      // Store tags in metadata for easy access
      babylonObject.metadata = babylonObject.metadata || {};
      babylonObject.metadata.tags = metadata.tags;
    }

    // Layer
    if (metadata.layer) {
      babylonObject.metadata = babylonObject.metadata || {};
      babylonObject.metadata.layer = metadata.layer;
    }
  }

  /**
   * Create a component for a Babylon.js object
   */
  createComponent(babylonObject, componentType, componentData) {
    const ComponentClass = this.componentRegistry.getComponent(componentType);
    if (!ComponentClass) {
      console.warn(`⚠️ Unknown component type: ${componentType}`);
      return null;
    }

    try {
      const component = new ComponentClass(babylonObject, componentData, this);
      component.onCreate();
      return component;
    } catch (error) {
      console.error(`❌ Failed to create component ${componentType}:`, error);
      return null;
    }
  }

  /**
   * Update an existing component
   */
  updateComponent(babylonObject, componentType, componentData) {
    const component = babylonObject.engineComponents?.[componentType];
    if (!component) {
      // Component doesn't exist, create it
      const newComponent = this.createComponent(babylonObject, componentType, componentData);
      if (newComponent) {
        babylonObject.engineComponents = babylonObject.engineComponents || {};
        babylonObject.engineComponents[componentType] = newComponent;
      }
      return;
    }

    try {
      if (component.updateData && typeof component.updateData === 'function') {
        component.updateData(componentData);
      }
    } catch (error) {
      console.error(`❌ Failed to update component ${componentType}:`, error);
    }
  }

  /**
   * Get the complete Babylon.js API for RenScript
   */
  getBabylonAPI() {
    return {
      // Core scene and engine
      scene: this.scene,
      engine: this.engine,
      
      // Math classes
      Vector3: BABYLON.Vector3,
      Vector2: BABYLON.Vector2,
      Vector4: BABYLON.Vector4,
      Matrix: BABYLON.Matrix,
      Quaternion: BABYLON.Quaternion,
      Color3: BABYLON.Color3,
      Color4: BABYLON.Color4,
      
      // Core object classes
      TransformNode: BABYLON.TransformNode,
      Mesh: BABYLON.Mesh,
      AbstractMesh: BABYLON.AbstractMesh,
      InstancedMesh: BABYLON.InstancedMesh,
      
      // Material classes
      Material: BABYLON.Material,
      StandardMaterial: BABYLON.StandardMaterial,
      PBRMaterial: BABYLON.PBRMaterial,
      NodeMaterial: BABYLON.NodeMaterial,
      
      // Texture classes
      Texture: BABYLON.Texture,
      DynamicTexture: BABYLON.DynamicTexture,
      RenderTargetTexture: BABYLON.RenderTargetTexture,
      CubeTexture: BABYLON.CubeTexture,
      
      // Light classes
      Light: BABYLON.Light,
      DirectionalLight: BABYLON.DirectionalLight,
      PointLight: BABYLON.PointLight,
      SpotLight: BABYLON.SpotLight,
      HemisphericLight: BABYLON.HemisphericLight,
      
      // Camera classes
      Camera: BABYLON.Camera,
      FreeCamera: BABYLON.FreeCamera,
      ArcRotateCamera: BABYLON.ArcRotateCamera,
      UniversalCamera: BABYLON.UniversalCamera,
      
      // Utility classes
      MeshBuilder: BABYLON.MeshBuilder,
      Tools: BABYLON.Tools,
      Animation: BABYLON.Animation,
      AnimationGroup: BABYLON.AnimationGroup,
      
      // System APIs (direct Babylon.js access - implement as needed)
      // physics, audio, particles, materials, animation, lighting available through BABYLON namespace
      
      // Bridge utilities
      bridge: {
        getObjectById: (engineId) => this.objectMap.get(engineId),
        getEngineId: (babylonObject) => this.engineMap.get(babylonObject),
        createObject: (engineData) => this.createBabylonObject(engineData),
        updateObject: (engineId, updates) => this.updateBabylonObject(engineId, updates),
        removeObject: (engineId) => this.removeBabylonObject(engineId)
      },
      
      // Full Babylon.js namespace for advanced users
      BABYLON: BABYLON
    };
  }

  /**
   * Setup performance monitoring
   */
  setupPerformanceMonitoring() {
    // Hook into the render loop to collect performance data
    this.scene.registerBeforeRender(() => {
      const now = performance.now();
      this.performanceStats.lastFrameTime = now - (this.performanceStats.lastUpdateTime || now);
      this.performanceStats.lastUpdateTime = now;
      
      // Update active object count
      this.performanceStats.activeObjects = this.objectMap.size;
      
      // Get render stats from engine
      const renderInfo = this.engine.getRenderingStats();
      this.performanceStats.drawCalls = renderInfo.drawCalls || 0;
      this.performanceStats.triangles = renderInfo.triangles || 0;
      
      // Update engine store with performance data
      engineActions.updatePerformanceStats({
        fps: Math.round(1000 / this.performanceStats.lastFrameTime),
        frameTime: this.performanceStats.lastFrameTime,
        drawCalls: this.performanceStats.drawCalls,
        triangles: this.performanceStats.triangles,
        activeObjects: this.performanceStats.activeObjects
      });
    });
  }

  /**
   * Sync all engine objects to Babylon.js (used when loading scenes)
   */
  syncSceneObjects(sceneId) {
    const sceneData = engineStore.scenes.sceneData[sceneId];
    if (!sceneData) {
      console.warn(`⚠️ Scene not found: ${sceneId}`);
      return;
    }

    console.log(`🔄 Syncing scene objects for: ${sceneId}`);

    // Clear existing mappings
    this.clearAllObjects();

    // Create objects in dependency order (parents before children)
    const objects = Object.values(sceneData.sceneGraph.nodes);
    const created = new Set();
    
    const createObjectWithParents = (obj) => {
      if (created.has(obj.id)) return;
      
      // Ensure parent is created first
      if (obj.parent && obj.parent !== "root_node") {
        const parentObj = sceneData.sceneGraph.nodes[obj.parent];
        if (parentObj) {
          createObjectWithParents(parentObj);
        }
      }
      
      // Create this object
      const parentBabylonObj = obj.parent && obj.parent !== "root_node" 
        ? this.objectMap.get(obj.parent) 
        : null;
        
      this.createBabylonObject(obj, parentBabylonObj);
      created.add(obj.id);
    };

    // Create all objects
    objects.forEach(obj => {
      if (obj.id !== "root_node") { // Skip the root node
        createObjectWithParents(obj);
      }
    });

    console.log(`✅ Synced ${created.size} objects to Babylon.js`);
  }

  /**
   * Clear all Babylon.js objects
   */
  clearAllObjects() {
    console.log('🧹 Clearing all Babylon objects...');
    
    // Dispose all objects
    Array.from(this.objectMap.keys()).forEach(engineId => {
      this.removeBabylonObject(engineId);
    });
    
    // Clear mappings
    this.objectMap.clear();
    this.engineMap.clear();
    
    console.log('✅ All Babylon objects cleared');
  }

  /**
   * Get performance statistics
   */
  getPerformanceStats() {
    return { ...this.performanceStats };
  }

  /**
   * Dispose the bridge and cleanup
   */
  dispose() {
    console.log('🗑️ BabylonBridge: Disposing...');
    
    // Clear all objects
    this.clearAllObjects();
    
    // System managers removed - cleanup handled by engine store

    console.log('✅ BabylonBridge: Disposed');
  }
}

export default BabylonBridge;