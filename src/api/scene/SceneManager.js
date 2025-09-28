import { bridgeService } from '@/plugins/core/bridge';
import { renderStore, setRenderStore, renderActions } from '@/render/store.jsx';
import { getCurrentProject, updateProjectCurrentScene } from '@/api/bridge/projects.js';
import { objectPropertiesStore } from '@/layout/stores/ViewportStore.jsx';

/**
 * SceneManager - Handles scene persistence and loading
 */
export class SceneManager {
  constructor() {
    this.currentSceneName = 'main';
    this.hasUnsavedChanges = false;
    this.isLoading = false; // Track loading state to prevent false "modified" flags
  }

  /**
   * Save the current scene
   * @param {string} sceneName - Name of the scene to save (optional, uses current)
   * @returns {Promise<{success: boolean, error?: string}>}
   */
  async saveScene(sceneName = null) {
    try {
      const project = getCurrentProject();
      if (!project) {
        return { success: false, error: 'No project selected' };
      }

      const sceneNameToSave = sceneName || this.currentSceneName;
      // Saving scene to file

      // Create serializable scene data
      const sceneData = {
        hierarchy: this.serializeHierarchy(renderStore.hierarchy),
        lighting: renderStore.lighting,
        settings: renderStore.settings,
        metadata: {
          name: sceneNameToSave,
          saved: new Date().toISOString(),
          engineVersion: '1.0.0'
        }
      };

      // Write to scenes directory
      const scenePath = `projects/${project.name}/scenes/${sceneNameToSave}.json`;
      await bridgeService.writeFile(scenePath, JSON.stringify(sceneData, null, 2));

      this.currentSceneName = sceneNameToSave;
      this.hasUnsavedChanges = false;

      // Clear unsaved changes in global store
      try {
        import('@/stores/UnsavedChangesStore.jsx').then(({ unsavedChangesActions }) => {
          unsavedChangesActions.clearSceneChanges();
        });
      } catch (error) {
        console.warn('Failed to clear unsaved changes store:', error);
      }

      // Update the scene tree name to reflect the saved scene name
      this.updateSceneTreeName(sceneNameToSave);
      
      // Update project.json with current scene
      await updateProjectCurrentScene(sceneNameToSave);

      // Clear unsaved changes indicator in viewport tab
      try {
        const { viewportStore, viewportActions } = await import('@/layout/stores/ViewportStore.jsx');
        const sceneTab = viewportStore.tabs.find(tab => tab.type === '3d-viewport');
        if (sceneTab && sceneTab.hasUnsavedChanges) {
          viewportActions.setTabUnsavedChanges(sceneTab.id, false);
          // Cleared unsaved changes indicator
        }
      } catch (err) {
        console.error('❌ SceneManager: Failed to clear unsaved changes indicator:', err);
      }

      // Scene saved successfully
      return { success: true };

    } catch (error) {
      console.error('❌ SceneManager: Save failed:', error);
      return { success: false, error: error.message };
    }
  }

  /**
   * Load a scene using bundled loading (scene + assets in one request)
   * @param {string} sceneName - Name of the scene to load
   * @returns {Promise<{success: boolean, error?: string}>}
   */
  async loadScene(sceneName) {
    const startTime = Date.now();
    // Starting scene load process
    
    // Set loading state to prevent false "modified" flags during loading
    this.isLoading = true;
    
    // Dispatch progress events with detailed information
    const dispatchProgress = (stage, currentFile = '', processedCount = 0, totalCount = 0) => {
      document.dispatchEvent(new CustomEvent('scene-loading-progress', {
        detail: { stage, currentFile, processedCount, totalCount, sceneName }
      }));
    };
    
    try {
      const project = getCurrentProject();
      // Got current project for scene loading
      
      if (!project) {
        console.error('❌ SceneManager: No project selected');
        return { success: false, error: 'No project selected' };
      }

      dispatchProgress('Checking for unsaved changes...');
      // Check for unsaved changes before switching scenes
      // Checking for unsaved changes
      const canContinue = await this.promptUnsavedChanges();
      if (!canContinue) {
        // User cancelled scene loading - clear loading state
        this.isLoading = false;
        return { success: false, error: 'User cancelled scene loading' };
      }

      dispatchProgress('Fetching scene data...');
      // Loading scene with bundled assets

      // Use the new bundled scene endpoint
      const bundleUrl = `http://localhost:3001/scene-bundle/${encodeURIComponent(project.name)}/${encodeURIComponent(sceneName)}`;
      // Fetching scene bundle from server
      
      const response = await fetch(bundleUrl);
      // Got response from server
      
      if (!response.ok) {
        console.error('❌ SceneManager: Failed to load scene bundle:', response.status, response.statusText);
        throw new Error(`Failed to load scene bundle: ${response.status} ${response.statusText}`);
      }

      // Parsing scene bundle response
      const bundleData = await response.json();
      // Scene bundle received with assets and scripts

      // Restore scene state from bundled data
      // Starting scene restoration
      await this.restoreSceneFromBundledData(bundleData, dispatchProgress);
      // Scene restoration completed

      this.currentSceneName = sceneName;
      this.hasUnsavedChanges = false;

      // Update project.json with current scene
      dispatchProgress('Updating project settings...');
      // Updating project current scene
      await updateProjectCurrentScene(sceneName);

      dispatchProgress('Scene loading complete!');
      const totalTime = Date.now() - startTime;
      // Scene load completed
      
      // Clear loading state
      this.isLoading = false;
      
      return { success: true };

    } catch (error) {
      console.error('❌ SceneManager: Bundled load failed:', error);
      
      // Clear loading state even on error
      this.isLoading = false;
      
      return { success: false, error: error.message };
    }
  }

  /**
   * Get list of available scenes in current project
   * @returns {Promise<Array<string>>} Scene names
   */
  async getAvailableScenes() {
    try {
      const project = getCurrentProject();
      if (!project) return [];

      const scenesData = await bridgeService.listDirectory(`projects/${project.name}/scenes`);
      
      if (!scenesData) return [];

      // Handle both array response (direct) and object response (with files property)
      const files = Array.isArray(scenesData) ? scenesData : scenesData.files;
      if (!files) return [];

      return files
        .filter(file => !file.is_directory && file.name.endsWith('.json'))
        .map(file => file.name.replace('.json', ''));

    } catch (error) {
      console.error('❌ SceneManager: Failed to get scenes:', error);
      return [];
    }
  }

  /**
   * Create a new scene
   * @param {string} sceneName - Name for the new scene
   * @returns {Promise<{success: boolean, error?: string}>}
   */
  async createNewScene(sceneName) {
    try {
      // Clear current scene
      this.clearScene();
      
      // Save as new scene
      const result = await this.saveScene(sceneName);
      
      if (result.success) {
        // New scene created successfully
      }
      
      return result;
      
    } catch (error) {
      console.error('❌ SceneManager: New scene creation failed:', error);
      return { success: false, error: error.message };
    }
  }

  /**
   * Clear the current scene (remove all objects)
   */
  clearScene() {
    const scene = renderStore.scene;
    if (!scene) return;

    // Clearing current scene

    // Remove all user objects (keep system objects)
    const objectsToRemove = [
      ...scene.meshes.filter(m => !this.isSystemObject(m)),
      ...scene.transformNodes.filter(n => !this.isSystemObject(n)),
      ...scene.lights.filter(l => !this.isSystemObject(l))
    ];

    objectsToRemove.forEach(obj => {
      renderActions.removeObject(obj);
    });

    // Reset hierarchy
    renderActions.initializeHierarchy();
    
    this.markAsModified();
  }

  /**
   * Check if an object is a system object (shouldn't be saved/removed)
   * @param {Object} obj - Babylon.js object
   * @returns {boolean}
   */
  isSystemObject(obj) {
    if (!obj.name) return true;
    
    // Check if it's a camera (cameras should be preserved across scene loads)
    if (obj.getClassName && obj.getClassName().includes('Camera')) {
      return true;
    }
    
    return obj.name.startsWith('__') ||
           obj.name.includes('gizmo') ||
           obj.name.includes('helper') ||
           obj.name.includes('_internal_') ||
           obj.name === 'FreeCamera' ||
           obj.name === 'light';
  }

  /**
   * Get attached scripts for an object from objectPropertiesStore
   * @param {string} objectId - Object ID
   * @returns {Array} Array of script info {path, name, properties}
   */
  getAttachedScriptsForObject(objectId) {
    try {
      const objectProps = objectPropertiesStore.objects[objectId];
      if (!objectProps || !objectProps.scripts || !Array.isArray(objectProps.scripts)) {
        return [];
      }

      // Return script info needed for reattachment
      return objectProps.scripts
        .filter(script => script.enabled) // Only save enabled scripts
        .map(script => ({
          path: script.path,
          name: script.name,
          properties: script.properties || {}
        }));
    } catch (error) {
      console.warn('⚠️ SceneManager: Could not access objectPropertiesStore:', error);
      return [];
    }
  }

  /**
   * Serialize hierarchy for JSON storage
   * @param {Array} hierarchy - Hierarchy array from renderStore
   * @returns {Array} Serializable hierarchy
   */
  serializeHierarchy(hierarchy) {
    return hierarchy.map(item => this.serializeHierarchyItem(item));
  }

  /**
   * Serialize a single hierarchy item
   * @param {Object} item - Hierarchy item
   * @returns {Object} Serializable item
   */
  serializeHierarchyItem(item) {
    const serialized = {
      id: item.id,
      name: item.name,
      type: item.type,
      lightType: item.lightType,
      visible: item.visible,
      expanded: item.expanded
    };

    // Serialize Babylon.js object data if present
    if (item.babylonObject) {
      serialized.babylonData = this.serializeBabylonObject(item.babylonObject);
    }

    // Recursively serialize children
    if (item.children) {
      serialized.children = item.children.map(child => this.serializeHierarchyItem(child));
    }

    return serialized;
  }

  /**
   * Serialize entire Babylon.js object data
   * @param {Object} babylonObj - Babylon.js object
   * @returns {Object} Complete serializable object data
   */
  serializeBabylonObject(babylonObj) {
    try {
      // Use Babylon.js built-in serialization to capture everything
      let serializedData = null;
      
      if (babylonObj.getClassName() === 'Mesh') {
        // For meshes, use the mesh serializer
        serializedData = babylonObj.serialize();
      } else if (babylonObj.getClassName() === 'TransformNode') {
        // For transform nodes (containers), serialize as transform node
        serializedData = babylonObj.serialize();
      } else if (babylonObj.getClassName() === 'UniversalCamera') {
        // For cameras, use camera serializer
        serializedData = babylonObj.serialize();
      } else {
        // For other objects, try generic serialization
        if (typeof babylonObj.serialize === 'function') {
          serializedData = babylonObj.serialize();
        } else {
          console.warn(`⚠️ SceneManager: Object ${babylonObj.name} of type ${babylonObj.getClassName()} has no serialize method, using fallback`);
          // Fallback to basic data
          serializedData = {
            name: babylonObj.name,
            className: babylonObj.getClassName(),
            id: babylonObj.uniqueId,
            position: babylonObj.position ? [babylonObj.position.x, babylonObj.position.y, babylonObj.position.z] : null,
            rotation: babylonObj.rotation ? [babylonObj.rotation.x, babylonObj.rotation.y, babylonObj.rotation.z] : null,
            scaling: babylonObj.scaling ? [babylonObj.scaling.x, babylonObj.scaling.y, babylonObj.scaling.z] : null,
            metadata: babylonObj.metadata
          };
        }
      }

      // Ensure we have essential identification data
      if (serializedData) {
        serializedData.__engineObjectId = babylonObj.uniqueId || babylonObj.name;
        serializedData.__engineClassName = babylonObj.getClassName();
        serializedData.__engineName = babylonObj.name;
        
        // Add attached RenScripts info from objectPropertiesStore
        const attachedScripts = this.getAttachedScriptsForObject(babylonObj.uniqueId || babylonObj.name);
        if (attachedScripts.length > 0) {
          serializedData.__attachedScripts = attachedScripts;
          // Object has attached scripts for serialization
        }
        
        // Serialized object data
        
        // Log metadata for debugging
        // Object metadata included in serialization
      }

      return serializedData;
      
    } catch (error) {
      console.error(`❌ SceneManager: Failed to serialize object ${babylonObj.name}:`, error);
      
      // Fallback to basic data if serialization fails
      return {
        name: babylonObj.name,
        className: babylonObj.getClassName(),
        id: babylonObj.uniqueId,
        __engineObjectId: babylonObj.uniqueId || babylonObj.name,
        __engineClassName: babylonObj.getClassName(),
        __engineName: babylonObj.name,
        position: babylonObj.position ? [babylonObj.position.x, babylonObj.position.y, babylonObj.position.z] : null,
        rotation: babylonObj.rotation ? [babylonObj.rotation.x, babylonObj.rotation.y, babylonObj.rotation.z] : null,
        scaling: babylonObj.scaling ? [babylonObj.scaling.x, babylonObj.scaling.y, babylonObj.scaling.z] : null,
        metadata: babylonObj.metadata
      };
    }
  }

  /**
   * Restore scene from saved data
   * @param {Object} sceneData - Saved scene data
   */
  async restoreSceneFromData(sceneData) {
    // Restoring scene from saved data

    // Clear current scene first
    this.clearScene();

    // Wait for scene to be ready
    await this.waitForScene();

    // Restore settings and lighting
    if (sceneData.lighting) {
      renderActions.updateSettings({ lighting: sceneData.lighting });
    }
    
    if (sceneData.settings) {
      renderActions.updateSettings(sceneData.settings);
    }

    // TODO: Restore scene objects from hierarchy
    // This will require recreating Babylon objects from the serialized data
    // Scene object restoration not yet implemented
    
    // For now, just restore the hierarchy structure without Babylon objects
    if (sceneData.hierarchy) {
      const cleanHierarchy = this.cleanHierarchyForDisplay(sceneData.hierarchy);
      renderActions.initializeHierarchy();
    }
  }

  /**
   * Restore scene from bundled data (scene + assets)
   * @param {Object} bundleData - Bundle containing scene data and assets
   */
  async restoreSceneFromBundledData(bundleData, dispatchProgress = null) {
    // Starting scene restoration from bundled data

    // Clear current scene first
    if (dispatchProgress) dispatchProgress('Clearing current scene...');
    // Clearing current scene
    this.clearScene();

    // Wait for scene to be ready
    if (dispatchProgress) dispatchProgress('Preparing scene...');
    // Waiting for scene to be ready
    await this.waitForScene();

    const sceneData = bundleData.scene;
    const assets = bundleData.assets;
    const scripts = bundleData.scripts || {};

    // Analyzing scene bundle data

    // Store bundled assets and compiled scripts in memory for later use
    // Create global caches that can be accessed by asset loaders and script runtime
    window._sceneBundledAssets = assets;
    window._sceneBundledScripts = scripts;
    // Cached bundled assets and compiled scripts in memory

    // Restore settings and lighting
    if (sceneData.lighting) {
      // Restoring lighting settings
      renderActions.updateSettings({ lighting: sceneData.lighting });
    }
    
    if (sceneData.settings) {
      // Restoring scene settings
      renderActions.updateSettings(sceneData.settings);
    }

    // Restore scene objects from hierarchy with bundled assets
    // Starting object restoration
    
    if (sceneData.hierarchy) {
      await this.restoreSceneObjects(sceneData.hierarchy, assets, dispatchProgress);
    } else {
      console.warn('⚠️ SceneManager: No hierarchy found in scene data');
    }

    // Update scene tree name to reflect loaded scene
    // Updating scene tree name
    this.updateSceneTreeName(bundleData.sceneName);
    
    // Scene restoration completed
  }

  /**
   * Restore scene objects from hierarchy with bundled assets
   * @param {Array} hierarchy - Scene hierarchy data
   * @param {Object} assets - Bundled assets (base64 encoded)
   */
  async restoreSceneObjects(hierarchy, assets, dispatchProgress = null) {
    // Starting scene object restoration
    
    // Count total objects for progress tracking
    const totalObjects = this.countTotalObjects(hierarchy);
    let processedObjects = 0;
    
    // Create progress tracker that can be called from anywhere in the hierarchy
    const progressTracker = (currentItem) => {
      processedObjects++;
      if (dispatchProgress) {
        const currentFile = currentItem?.babylonData?.metadata?.originalAssetData?.path || currentItem?.name || 'Unknown';
        // Progress tracking for object restoration
        dispatchProgress('Restoring objects...', currentFile, processedObjects, totalObjects);
      }
    };
    
    // Process hierarchy items recursively
    for (let i = 0; i < hierarchy.length; i++) {
      const item = hierarchy[i];
      // Processing hierarchy item
      await this.restoreHierarchyItem(item, assets, progressTracker);
    }
    
    // Completed scene object restoration
  }

  /**
   * Count total objects in hierarchy for progress tracking
   * @param {Array} hierarchy - Scene hierarchy
   * @returns {number} Total object count
   */
  countTotalObjects(hierarchy) {
    let count = 0;
    const countRecursive = (items) => {
      for (const item of items) {
        count++;
        if (item.children) {
          countRecursive(item.children);
        }
      }
    };
    countRecursive(hierarchy);
    return count;
  }

  /**
   * Restore a single hierarchy item and its children
   * @param {Object} item - Hierarchy item
   * @param {Object} assets - Bundled assets
   * @param {Function} progressTracker - Progress tracking function
   */
  async restoreHierarchyItem(item, assets, progressTracker = null) {
    // Processing hierarchy item details

    // Track progress for this item (every item counts towards progress)
    if (progressTracker) {
      progressTracker(item);
    }

    // Skip system objects (scene root) but still process children
    if (item.type === 'scene') {
      // Processing scene root children
      // Process children of scene root
      if (item.children) {
        for (let i = 0; i < item.children.length; i++) {
          const child = item.children[i];
          // Processing scene child
          await this.restoreHierarchyItem(child, assets, progressTracker);
        }
      }
      return;
    }

    // Restore object based on babylon data (cameras, lights, meshes - all get restored)
    if (item.babylonData) {
      // Restoring object with babylon data
      await this.restoreObjectFromBabylonData(item, assets);
    } else {
      // Skipping item without babylon data
    }

    // Process children recursively
    if (item.children) {
      // Processing item children
      for (let i = 0; i < item.children.length; i++) {
        const child = item.children[i];
        // Processing child item
        await this.restoreHierarchyItem(child, assets, progressTracker);
      }
    }
  }

  /**
   * Restore an object from its Babylon serialization data
   * @param {Object} item - Hierarchy item with babylonData
   * @param {Object} assets - Bundled assets
   */
  async restoreObjectFromBabylonData(item, assets, dispatchProgress = null) {
    const scene = renderStore.scene;
    if (!scene) {
      console.error('❌ SceneManager: No scene available for object restoration');
      return;
    }

    // Restoring object from babylon data

    try {
      // Check if this object has asset data (3D model)
      const assetPath = item.babylonData?.metadata?.originalAssetData?.path;
      
      if (assetPath && assets[assetPath]) {
        // Restoring 3D model from bundled asset
        const restoredObject = await this.restore3DModelFromAsset(item, assets[assetPath], scene);
        
        // Reattach scripts after object restoration
        if (restoredObject && item.babylonData?.__attachedScripts) {
          await this.reattachScriptsToObject(restoredObject, item.babylonData.__attachedScripts, dispatchProgress);
        }
      } else {
        // Restoring basic object without asset data
        // Handle objects without asset data (cameras, lights, etc.)
        const restoredObject = await this.restoreBasicObject(item, scene);
        
        // Reattach scripts after object restoration
        if (restoredObject && item.babylonData?.__attachedScripts) {
          await this.reattachScriptsToObject(restoredObject, item.babylonData.__attachedScripts, dispatchProgress);
        }
      }
    } catch (error) {
      console.error(`❌ SceneManager: Failed to restore object '${item.name}':`, error);
    }
  }

  /**
   * Restore a 3D model object from bundled asset data
   * @param {Object} item - Hierarchy item
   * @param {string} assetData - Base64 encoded asset data  
   * @param {Scene} scene - Babylon scene
   */
  async restore3DModelFromAsset(item, assetData, scene) {
    // Starting 3D model restoration
    
    try {
      const { SceneLoader } = await import('@babylonjs/core/Loading/sceneLoader.js');
      const { TransformNode } = await import('@babylonjs/core/Meshes/transformNode.js');
      
      // Babylon modules imported
      // Processing asset data

      // Convert base64 to blob URL for SceneLoader
      // Converting asset data to binary
      const binaryData = atob(assetData);
      // Binary conversion completed
      
      const bytes = new Uint8Array(binaryData.length);
      for (let i = 0; i < binaryData.length; i++) {
        bytes[i] = binaryData.charCodeAt(i);
      }
      
      const blob = new Blob([bytes], { type: 'application/octet-stream' });
      const blobUrl = URL.createObjectURL(blob);
      // Created blob URL for loading

      // Load the 3D model with file extension hint
      const assetPath = item.babylonData?.metadata?.originalAssetData?.path;
      const fileExtension = assetPath ? assetPath.split('.').pop().toLowerCase() : 'glb';
      // Loading 3D model with SceneLoader
      const result = await SceneLoader.ImportMeshAsync("", "", blobUrl, scene, undefined, `.${fileExtension}`);
      // SceneLoader completed successfully
      
      // Clean up blob URL
      URL.revokeObjectURL(blobUrl);

      if (result.meshes && result.meshes.length > 0) {
        // Creating container for loaded meshes
        
        // Create container (use the existing name from the saved data)
        const containerName = item.babylonData.__engineName || item.name;
        const container = new TransformNode(containerName, scene);
        // Container created for meshes
        
        // Parent all loaded meshes to the container
        let parentedMeshes = 0;
        result.meshes.forEach((mesh, index) => {
          if (mesh.name !== "__root__") {
            mesh.setParent(container);
            parentedMeshes++;
            // Parented mesh to container
          }
        });
        // All meshes parented to container

        // Restore transform and metadata from saved data
        if (item.babylonData.position) {
          container.position.x = item.babylonData.position[0];
          container.position.y = item.babylonData.position[1]; 
          container.position.z = item.babylonData.position[2];
          // Restored object position
        }
        if (item.babylonData.rotation) {
          container.rotation.x = item.babylonData.rotation[0];
          container.rotation.y = item.babylonData.rotation[1];
          container.rotation.z = item.babylonData.rotation[2];
          // Restored object rotation
        }
        if (item.babylonData.scaling) {
          container.scaling.x = item.babylonData.scaling[0];
          container.scaling.y = item.babylonData.scaling[1];
          container.scaling.z = item.babylonData.scaling[2];
          // Restored object scaling
        }

        // Restore metadata
        if (item.babylonData.metadata) {
          container.metadata = item.babylonData.metadata;
          // Restored object metadata
        }

        // Add to render store
        // Adding container to render store
        renderActions.addObject(container);
        
        // 3D model restoration completed successfully
        return container;
      } else {
        console.error('❌ SceneManager: No meshes found in loaded result for', item.name);
        return null;
      }
    } catch (error) {
      console.error('❌ SceneManager: Failed to load 3D model for', item.name, error);
      console.error('❌ SceneManager: Error stack:', error.stack);
      return null;
    }
  }

  /**
   * Restore a basic object (camera, light, etc.) without asset data
   * @param {Object} item - Hierarchy item
   * @param {Scene} scene - Babylon scene
   * @returns {Object|null} Restored Babylon object or null
   */
  async restoreBasicObject(item, scene) {
    // Restoring basic object
    
    try {
      const babylonData = item.babylonData;
      if (!babylonData) {
        console.warn(`⚠️ SceneManager: No babylon data for ${item.name}`);
        return null;
      }

      let restoredObject = null;

      if (item.type === 'camera') {
        // Restoring camera object
        
        // Import Babylon camera classes
        const { UniversalCamera } = await import('@babylonjs/core/Cameras/universalCamera.js');
        const { Vector3 } = await import('@babylonjs/core/Maths/math.vector.js');
        
        // Create camera from babylon data
        const camera = new UniversalCamera(babylonData.name, new Vector3(0, 0, 0), scene);
        
        // Restore camera properties from saved data
        if (babylonData.position) {
          // Restoring camera position
          camera.position.fromArray(babylonData.position);
          // Camera position restored
        }
        if (babylonData.rotation) {
          // Restoring camera rotation
          camera.rotation.fromArray(babylonData.rotation);
          // Camera rotation restored
        }
        if (babylonData.fov !== undefined) {
          camera.fov = babylonData.fov;
        }
        if (babylonData.minZ !== undefined) {
          camera.minZ = babylonData.minZ;
        }
        if (babylonData.maxZ !== undefined) {
          camera.maxZ = babylonData.maxZ;
        }
        
        // Set camera target - check if there's a saved target, otherwise use origin
        if (babylonData.target) {
          camera.setTarget(new Vector3(babylonData.target[0], babylonData.target[1], babylonData.target[2]));
        } else {
          // For UniversalCamera, we might not need to set target if position and rotation are restored
          // Using camera position and rotation only
        }
        
        // Restore metadata
        if (babylonData.metadata) {
          camera.metadata = babylonData.metadata;
        }
        
        // Set unique ID to match saved data
        camera.uniqueId = babylonData.uniqueId || babylonData.__engineObjectId;
        
        // Set as active camera in both scene and render store
        scene.activeCamera = camera;
        renderActions.setCamera(camera);
        renderActions.addObject(camera);
        
        restoredObject = camera;
        // Camera restored successfully
        // Camera has scripts to attach
        
      } else if (item.type === 'light') {
        // Light restoration not yet implemented
        // TODO: Implement light restoration when needed
        return null;
        
      } else {
        // Unknown basic object type
        return null;
      }

      return restoredObject;
      
    } catch (error) {
      console.error(`❌ SceneManager: Failed to restore basic object '${item.name}':`, error);
      return null;
    }
  }

  /**
   * Reattach scripts to a restored object using pre-compiled scripts from bundle
   * @param {Object} babylonObject - The restored Babylon object
   * @param {Array} attachedScripts - Array of script info from scene data
   */
  async reattachScriptsToObject(babylonObject, attachedScripts, dispatchProgress = null) {
    if (!babylonObject || !attachedScripts || attachedScripts.length === 0) {
      return;
    }

    // Reattaching scripts to object

    try {
      // Get the script runtime
      const { getScriptRuntime } = await import('@/api/script');
      const runtime = getScriptRuntime();
      
      if (!runtime) {
        console.error('❌ SceneManager: No script runtime available for reattaching scripts');
        return;
      }

      // Get objectPropertiesActions for updating UI state
      const { objectPropertiesActions } = await import('@/layout/stores/ViewportStore.jsx');
      const objectId = babylonObject.uniqueId || babylonObject.name;

      // Ensure object properties exist
      objectPropertiesActions.ensureDefaultComponents(objectId);

      // Attach each script using existing attachScript method (it will auto-detect bulk mode)
      for (const scriptInfo of attachedScripts) {
        // Reattaching individual script
        
        if (dispatchProgress) {
          dispatchProgress('Attaching scripts...', scriptInfo.name);
        }
        
        // Use the existing attachScript method (it will check bundle cache automatically)
        const success = await runtime.attachScript(objectId, scriptInfo.path);
        
        if (success) {
          const scriptInstance = runtime.getScriptInstance(objectId, scriptInfo.path);
          
          // Restore script properties if they exist
          if (scriptInfo.properties && Object.keys(scriptInfo.properties).length > 0) {
            // Restoring script properties
            
            // Update Babylon object metadata
            if (!babylonObject.metadata) babylonObject.metadata = {};
            if (!babylonObject.metadata.scriptProperties) babylonObject.metadata.scriptProperties = {};
            
            Object.entries(scriptInfo.properties).forEach(([propName, propValue]) => {
              babylonObject.metadata.scriptProperties[propName] = propValue;
              
              // Also update script instance if available
              if (scriptInstance?._scriptAPI?.setScriptProperty) {
                scriptInstance._scriptAPI.setScriptProperty(propName, propValue);
              }
            });
          }

          // Update objectPropertiesStore UI state
          objectPropertiesActions.addPropertySection(objectId, 'scripts', [{
            path: scriptInfo.path,
            name: scriptInfo.name,
            enabled: true,
            properties: scriptInfo.properties || {}
          }]);

          // Successfully reattached script
        } else {
          console.error(`❌ SceneManager: Failed to reattach script '${scriptInfo.name}' to object '${babylonObject.name}'`);
        }
      }

      // Completed script reattachment
      
    } catch (error) {
      console.error(`❌ SceneManager: Error reattaching scripts to object '${babylonObject.name}':`, error);
    }
  }

  /**
   * Clean hierarchy for display (remove babylonObject references)
   * @param {Array} hierarchy - Original hierarchy
   * @returns {Array} Clean hierarchy
   */
  cleanHierarchyForDisplay(hierarchy) {
    return hierarchy.map(item => ({
      ...item,
      babylonObject: null, // Will be reconnected when objects are recreated
      children: item.children ? this.cleanHierarchyForDisplay(item.children) : undefined
    }));
  }

  /**
   * Wait for scene to be ready
   */
  async waitForScene() {
    while (!renderStore.scene) {
      await new Promise(resolve => setTimeout(resolve, 50));
    }
  }

  /**
   * Mark scene as modified and update UI indicators
   */
  markAsModified() {
    // Don't mark as modified if we're currently loading a scene
    if (this.isLoading) {
      console.log('🔄 Skipping markAsModified during scene loading');
      return;
    }
    
    this.hasUnsavedChanges = true;
    
    // Update global unsaved changes store
    try {
      import('@/stores/UnsavedChangesStore.jsx').then(({ unsavedChangesActions }) => {
        unsavedChangesActions.markSceneModified('Scene modifications');
      });
    } catch (error) {
      console.warn('Failed to update unsaved changes store:', error);
    }
    
    // Update viewport tab to show unsaved changes indicator
    import('@/layout/stores/ViewportStore.jsx').then(({ viewportStore, viewportActions }) => {
      const sceneTab = viewportStore.tabs.find(tab => tab.type === '3d-viewport');
      if (sceneTab && !sceneTab.hasUnsavedChanges) {
        viewportActions.setTabUnsavedChanges(sceneTab.id, true);
        // Marked scene as having unsaved changes
      }
    }).catch(err => {
      console.error('❌ SceneManager: Failed to update tab unsaved changes indicator:', err);
    });
  }

  /**
   * Check if scene has unsaved changes
   * @returns {boolean}
   */
  hasChanges() {
    return this.hasUnsavedChanges;
  }

  /**
   * Prompt user to save unsaved changes before switching scenes
   * @returns {Promise<boolean>} true if user wants to continue, false to cancel
   */
  async promptUnsavedChanges() {
    if (!this.hasUnsavedChanges) {
      return true;
    }

    const result = confirm(
      `Scene "${this.currentSceneName}" has unsaved changes.\n\n` +
      'Do you want to save before switching?\n\n' +
      'Click "OK" to save and continue, or "Cancel" to discard changes.'
    );

    if (result) {
      const saveResult = await this.saveScene();
      if (!saveResult.success) {
        alert(`Failed to save scene: ${saveResult.error}`);
        return false;
      }
    } else {
      // User chose to discard changes
      this.hasUnsavedChanges = false;
      
      // Clear unsaved changes indicator
      try {
        const { viewportStore, viewportActions } = await import('@/layout/stores/ViewportStore.jsx');
        const sceneTab = viewportStore.tabs.find(tab => tab.type === '3d-viewport');
        if (sceneTab && sceneTab.hasUnsavedChanges) {
          viewportActions.setTabUnsavedChanges(sceneTab.id, false);
        }
      } catch (err) {
        console.error('❌ SceneManager: Failed to clear unsaved changes indicator:', err);
      }
    }

    return true;
  }

  /**
   * Get current scene name
   * @returns {string}
   */
  getCurrentSceneName() {
    return this.currentSceneName;
  }

  /**
   * Set current scene name
   * @param {string} name - Scene name
   */
  setCurrentSceneName(name) {
    this.currentSceneName = name;
    this.updateSceneTreeName(name);
  }

  /**
   * Update the scene tree root name to reflect current scene
   * @param {string} sceneName - Name of the scene
   */
  updateSceneTreeName(sceneName) {
    setRenderStore('hierarchy', prev => {
      return prev.map(item => {
        if (item.id === 'scene-root') {
          return {
            ...item,
            name: sceneName
          };
        }
        return item;
      });
    });
    
    // Updated scene tree name
  }
}

export const sceneManager = new SceneManager();