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
      console.log('💾 SceneManager: Saving scene:', sceneNameToSave);

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
          console.log('✅ SceneManager: Cleared unsaved changes indicator');
        }
      } catch (err) {
        console.error('❌ SceneManager: Failed to clear unsaved changes indicator:', err);
      }

      console.log('✅ SceneManager: Scene saved successfully:', sceneNameToSave);
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
    console.log(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() STARTED - Scene: "${sceneName}"`);
    
    // Dispatch progress events with detailed information
    const dispatchProgress = (stage, currentFile = '', processedCount = 0, totalCount = 0) => {
      document.dispatchEvent(new CustomEvent('scene-loading-progress', {
        detail: { stage, currentFile, processedCount, totalCount, sceneName }
      }));
    };
    
    try {
      const project = getCurrentProject();
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() - Project:`, project);
      
      if (!project) {
        console.error(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() - ERROR: No project selected`);
        return { success: false, error: 'No project selected' };
      }

      dispatchProgress('Checking for unsaved changes...');
      // Check for unsaved changes before switching scenes
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() - Checking unsaved changes...`);
      const canContinue = await this.promptUnsavedChanges();
      if (!canContinue) {
        console.log(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() - User cancelled scene loading`);
        return { success: false, error: 'User cancelled scene loading' };
      }

      dispatchProgress('Fetching scene data...');
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() - Loading scene with bundled assets:`, sceneName);

      // Use the new bundled scene endpoint
      const bundleUrl = `http://localhost:3001/scene-bundle/${encodeURIComponent(project.name)}/${encodeURIComponent(sceneName)}`;
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() - Fetching bundle from:`, bundleUrl);
      
      const response = await fetch(bundleUrl);
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() - Response status:`, response.status, response.statusText);
      
      if (!response.ok) {
        console.error(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() - ERROR: Failed to load scene bundle:`, response.status, response.statusText);
        throw new Error(`Failed to load scene bundle: ${response.status} ${response.statusText}`);
      }

      console.log(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() - Parsing JSON response...`);
      const bundleData = await response.json();
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() - Bundle received:`, {
        assetCount: bundleData.assetCount,
        scriptCount: bundleData.scriptCount || 0,
        project: bundleData.project,
        sceneName: bundleData.sceneName,
        bundledAt: bundleData.bundledAt,
        sceneHierarchyLength: bundleData.scene?.hierarchy?.length,
        assetsKeys: Object.keys(bundleData.assets || {}),
        scriptsKeys: Object.keys(bundleData.scripts || {})
      });

      // Restore scene state from bundled data
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() - Starting scene restoration...`);
      await this.restoreSceneFromBundledData(bundleData, dispatchProgress);
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() - Scene restoration completed`);

      this.currentSceneName = sceneName;
      this.hasUnsavedChanges = false;

      // Update project.json with current scene
      dispatchProgress('Updating project settings...');
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() - Updating project currentScene...`);
      await updateProjectCurrentScene(sceneName);

      dispatchProgress('Scene loading complete!');
      const totalTime = Date.now() - startTime;
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.loadScene() - COMPLETED in ${totalTime}ms - Scene: "${sceneName}"`);
      return { success: true };

    } catch (error) {
      console.error('❌ SceneManager: Bundled load failed:', error);
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
      
      if (!scenesData || !scenesData.files) return [];

      return scenesData.files
        .filter(file => file.type === 'file' && file.name.endsWith('.json'))
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
        console.log('✅ SceneManager: New scene created:', sceneName);
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

    console.log('🗑️ SceneManager: Clearing scene...');

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
          console.log(`📜 SceneManager: Object ${babylonObj.name} has ${attachedScripts.length} attached scripts`);
        }
        
        console.log(`📄 SceneManager: Serialized ${babylonObj.getClassName()} '${babylonObj.name}' with ${Object.keys(serializedData).length} properties`);
        
        // Log metadata for debugging
        if (babylonObj.metadata && Object.keys(babylonObj.metadata).length > 0) {
          console.log(`📄 SceneManager: Object ${babylonObj.name} has metadata:`, Object.keys(babylonObj.metadata));
        }
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
    console.log('🔄 SceneManager: Restoring scene from data...');

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
    console.log('🏗️ SceneManager: Scene object restoration not yet implemented');
    
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
    console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreSceneFromBundledData() STARTED`);

    // Clear current scene first
    if (dispatchProgress) dispatchProgress('Clearing current scene...');
    console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreSceneFromBundledData() - Clearing current scene...`);
    this.clearScene();

    // Wait for scene to be ready
    if (dispatchProgress) dispatchProgress('Preparing scene...');
    console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreSceneFromBundledData() - Waiting for scene...`);
    await this.waitForScene();

    const sceneData = bundleData.scene;
    const assets = bundleData.assets;
    const scripts = bundleData.scripts || {};

    console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreSceneFromBundledData() - Scene data:`, {
      hasLighting: !!sceneData.lighting,
      hasSettings: !!sceneData.settings,
      hasHierarchy: !!sceneData.hierarchy,
      hierarchyLength: sceneData.hierarchy?.length,
      assetsCount: Object.keys(assets).length,
      scriptsCount: Object.keys(scripts).length
    });

    // Store bundled assets and compiled scripts in memory for later use
    // Create global caches that can be accessed by asset loaders and script runtime
    window._sceneBundledAssets = assets;
    window._sceneBundledScripts = scripts;
    console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreSceneFromBundledData() - Cached ${Object.keys(assets).length} bundled assets and ${Object.keys(scripts).length} compiled scripts in memory`);

    // Restore settings and lighting
    if (sceneData.lighting) {
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreSceneFromBundledData() - Restoring lighting settings...`);
      renderActions.updateSettings({ lighting: sceneData.lighting });
    }
    
    if (sceneData.settings) {
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreSceneFromBundledData() - Restoring scene settings...`);
      renderActions.updateSettings(sceneData.settings);
    }

    // Restore scene objects from hierarchy with bundled assets
    console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreSceneFromBundledData() - Starting object restoration...`);
    
    if (sceneData.hierarchy) {
      await this.restoreSceneObjects(sceneData.hierarchy, assets, dispatchProgress);
    } else {
      console.warn(`🔥 [${new Date().toISOString()}] SceneManager.restoreSceneFromBundledData() - WARNING: No hierarchy found in scene data!`);
    }

    // Update scene tree name to reflect loaded scene
    console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreSceneFromBundledData() - Updating scene tree name...`);
    this.updateSceneTreeName(bundleData.sceneName);
    
    console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreSceneFromBundledData() - COMPLETED`);
  }

  /**
   * Restore scene objects from hierarchy with bundled assets
   * @param {Array} hierarchy - Scene hierarchy data
   * @param {Object} assets - Bundled assets (base64 encoded)
   */
  async restoreSceneObjects(hierarchy, assets, dispatchProgress = null) {
    console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreSceneObjects() STARTED - Processing ${hierarchy.length} hierarchy items`);
    
    // Count total objects for progress tracking
    const totalObjects = this.countTotalObjects(hierarchy);
    let processedObjects = 0;
    
    // Create progress tracker that can be called from anywhere in the hierarchy
    const progressTracker = (currentItem) => {
      processedObjects++;
      if (dispatchProgress) {
        const currentFile = currentItem?.babylonData?.metadata?.originalAssetData?.path || currentItem?.name || 'Unknown';
        console.log(`📊 Progress: ${processedObjects}/${totalObjects} - Processing: ${currentFile}`);
        dispatchProgress('Restoring objects...', currentFile, processedObjects, totalObjects);
      }
    };
    
    // Process hierarchy items recursively
    for (let i = 0; i < hierarchy.length; i++) {
      const item = hierarchy[i];
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreSceneObjects() - Processing hierarchy item ${i + 1}/${hierarchy.length}: "${item.name}"`);
      await this.restoreHierarchyItem(item, assets, progressTracker);
    }
    
    console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreSceneObjects() - COMPLETED processing ${hierarchy.length} hierarchy items`);
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
    console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreHierarchyItem() - Item: "${item.name}" Type: "${item.type}" HasBabylonData: ${!!item.babylonData} HasChildren: ${!!item.children}`);

    // Track progress for this item (every item counts towards progress)
    if (progressTracker) {
      progressTracker(item);
    }

    // Skip system objects (scene root) but still process children
    if (item.type === 'scene') {
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreHierarchyItem() - Scene root found, processing ${item.children?.length || 0} children...`);
      // Process children of scene root
      if (item.children) {
        for (let i = 0; i < item.children.length; i++) {
          const child = item.children[i];
          console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreHierarchyItem() - Processing scene child ${i + 1}/${item.children.length}: "${child.name}"`);
          await this.restoreHierarchyItem(child, assets, progressTracker);
        }
      }
      return;
    }

    // Restore object based on babylon data (cameras, lights, meshes - all get restored)
    if (item.babylonData) {
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreHierarchyItem() - Restoring object with babylon data: "${item.name}"`);
      await this.restoreObjectFromBabylonData(item, assets);
    } else {
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreHierarchyItem() - Skipping item: "${item.name}" (type: ${item.type}, hasBabylonData: ${!!item.babylonData})`);
    }

    // Process children recursively
    if (item.children) {
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreHierarchyItem() - Processing ${item.children.length} children of "${item.name}"`);
      for (let i = 0; i < item.children.length; i++) {
        const child = item.children[i];
        console.log(`🔥 [${new Date().toISOString()}] SceneManager.restoreHierarchyItem() - Processing child ${i + 1}/${item.children.length} of "${item.name}": "${child.name}"`);
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

    console.log(`📦 SceneManager: Restoring object '${item.name}' with babylon data`);

    try {
      // Check if this object has asset data (3D model)
      const assetPath = item.babylonData?.metadata?.originalAssetData?.path;
      
      if (assetPath && assets[assetPath]) {
        console.log(`🎯 SceneManager: Restoring 3D model from bundled asset: ${assetPath}`);
        const restoredObject = await this.restore3DModelFromAsset(item, assets[assetPath], scene);
        
        // Reattach scripts after object restoration
        if (restoredObject && item.babylonData?.__attachedScripts) {
          await this.reattachScriptsToObject(restoredObject, item.babylonData.__attachedScripts, dispatchProgress);
        }
      } else {
        console.log(`📋 SceneManager: Restoring basic object without asset data`);
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
    console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() STARTED - Item: "${item.name}"`);
    
    try {
      const { SceneLoader } = await import('@babylonjs/core/Loading/sceneLoader.js');
      const { TransformNode } = await import('@babylonjs/core/Meshes/transformNode.js');
      
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Babylon modules imported successfully`);
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Asset data size: ${assetData.length} chars (base64)`);

      // Convert base64 to blob URL for SceneLoader
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Converting base64 to binary...`);
      const binaryData = atob(assetData);
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Binary data size: ${binaryData.length} bytes`);
      
      const bytes = new Uint8Array(binaryData.length);
      for (let i = 0; i < binaryData.length; i++) {
        bytes[i] = binaryData.charCodeAt(i);
      }
      
      const blob = new Blob([bytes], { type: 'application/octet-stream' });
      const blobUrl = URL.createObjectURL(blob);
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Created blob URL: ${blobUrl.substring(0, 50)}...`);

      // Load the 3D model with file extension hint
      const assetPath = item.babylonData?.metadata?.originalAssetData?.path;
      const fileExtension = assetPath ? assetPath.split('.').pop().toLowerCase() : 'glb';
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Loading 3D model via SceneLoader.ImportMeshAsync with extension hint: .${fileExtension}`);
      const result = await SceneLoader.ImportMeshAsync("", "", blobUrl, scene, undefined, `.${fileExtension}`);
      console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - SceneLoader completed:`, {
        meshesCount: result.meshes?.length,
        transformNodesCount: result.transformNodes?.length,
        animationGroupsCount: result.animationGroups?.length,
        skeletonsCount: result.skeletons?.length
      });
      
      // Clean up blob URL
      URL.revokeObjectURL(blobUrl);

      if (result.meshes && result.meshes.length > 0) {
        console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Creating container for ${result.meshes.length} meshes`);
        
        // Create container (use the existing name from the saved data)
        const containerName = item.babylonData.__engineName || item.name;
        const container = new TransformNode(containerName, scene);
        console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Container created: "${containerName}" (ID: ${container.uniqueId})`);
        
        // Parent all loaded meshes to the container
        let parentedMeshes = 0;
        result.meshes.forEach((mesh, index) => {
          if (mesh.name !== "__root__") {
            mesh.setParent(container);
            parentedMeshes++;
            console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Parented mesh ${index + 1}: "${mesh.name}"`);
          }
        });
        console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Parented ${parentedMeshes}/${result.meshes.length} meshes to container`);

        // Restore transform and metadata from saved data
        if (item.babylonData.position) {
          container.position.x = item.babylonData.position[0];
          container.position.y = item.babylonData.position[1]; 
          container.position.z = item.babylonData.position[2];
          console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Restored position: [${item.babylonData.position.join(', ')}]`);
        }
        if (item.babylonData.rotation) {
          container.rotation.x = item.babylonData.rotation[0];
          container.rotation.y = item.babylonData.rotation[1];
          container.rotation.z = item.babylonData.rotation[2];
          console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Restored rotation: [${item.babylonData.rotation.join(', ')}]`);
        }
        if (item.babylonData.scaling) {
          container.scaling.x = item.babylonData.scaling[0];
          container.scaling.y = item.babylonData.scaling[1];
          container.scaling.z = item.babylonData.scaling[2];
          console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Restored scaling: [${item.babylonData.scaling.join(', ')}]`);
        }

        // Restore metadata
        if (item.babylonData.metadata) {
          container.metadata = item.babylonData.metadata;
          console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Restored metadata with ${Object.keys(item.babylonData.metadata).length} properties`);
        }

        // Add to render store
        console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Adding container to render store...`);
        renderActions.addObject(container);
        
        console.log(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - COMPLETED SUCCESSFULLY for "${item.name}" - Container ID: ${container.uniqueId}`);
        return container;
      } else {
        console.error(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - ERROR: No meshes found in loaded result for "${item.name}"`);
        return null;
      }
    } catch (error) {
      console.error(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - ERROR: Failed to load 3D model for "${item.name}":`, error);
      console.error(`🔥 [${new Date().toISOString()}] SceneManager.restore3DModelFromAsset() - Error stack:`, error.stack);
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
    console.log(`📋 SceneManager: Restoring basic object '${item.name}' of type '${item.type}'`);
    
    try {
      const babylonData = item.babylonData;
      if (!babylonData) {
        console.warn(`⚠️ SceneManager: No babylon data for ${item.name}`);
        return null;
      }

      let restoredObject = null;

      if (item.type === 'camera') {
        console.log(`📷 SceneManager: Restoring camera: ${item.name}`);
        
        // Import Babylon camera classes
        const { UniversalCamera } = await import('@babylonjs/core/Cameras/universalCamera.js');
        const { Vector3 } = await import('@babylonjs/core/Maths/math.vector.js');
        
        // Create camera from babylon data
        const camera = new UniversalCamera(babylonData.name, new Vector3(0, 0, 0), scene);
        
        // Restore camera properties from saved data
        if (babylonData.position) {
          console.log('📷 SceneManager: Restoring camera position:', babylonData.position);
          camera.position.fromArray(babylonData.position);
          console.log('📷 SceneManager: Camera position set to:', camera.position.x, camera.position.y, camera.position.z);
        }
        if (babylonData.rotation) {
          console.log('📷 SceneManager: Restoring camera rotation:', babylonData.rotation);
          camera.rotation.fromArray(babylonData.rotation);
          console.log('📷 SceneManager: Camera rotation set to:', camera.rotation.x, camera.rotation.y, camera.rotation.z);
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
          console.log('📷 SceneManager: No saved target for camera, using position and rotation only');
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
        console.log(`✅ SceneManager: Camera '${item.name}' restored successfully with uniqueId:`, camera.uniqueId);
        console.log(`📋 SceneManager: Camera scripts to attach:`, item.babylonData?.__attachedScripts?.length || 0);
        
      } else if (item.type === 'light') {
        console.log(`💡 SceneManager: Light restoration not yet implemented for '${item.name}'`);
        // TODO: Implement light restoration when needed
        return null;
        
      } else {
        console.log(`🤷 SceneManager: Unknown basic object type '${item.type}' for '${item.name}'`);
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

    console.log(`📜 SceneManager: Reattaching ${attachedScripts.length} scripts to object '${babylonObject.name}'`);

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
        console.log(`📜 SceneManager: Reattaching script '${scriptInfo.name}' to object '${babylonObject.name}'`);
        
        if (dispatchProgress) {
          dispatchProgress('Attaching scripts...', scriptInfo.name);
        }
        
        // Use the existing attachScript method (it will check bundle cache automatically)
        const success = await runtime.attachScript(objectId, scriptInfo.path);
        
        if (success) {
          const scriptInstance = runtime.getScriptInstance(objectId, scriptInfo.path);
          
          // Restore script properties if they exist
          if (scriptInfo.properties && Object.keys(scriptInfo.properties).length > 0) {
            console.log(`📜 SceneManager: Restoring ${Object.keys(scriptInfo.properties).length} script properties`);
            
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

          console.log(`✅ SceneManager: Successfully reattached script '${scriptInfo.name}' using pre-compiled code`);
        } else {
          console.error(`❌ SceneManager: Failed to reattach script '${scriptInfo.name}' to object '${babylonObject.name}'`);
        }
      }

      console.log(`✅ SceneManager: Completed reattaching scripts to object '${babylonObject.name}'`);
      
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
    this.hasUnsavedChanges = true;
    
    // Update viewport tab to show unsaved changes indicator
    import('@/layout/stores/ViewportStore.jsx').then(({ viewportStore, viewportActions }) => {
      const sceneTab = viewportStore.tabs.find(tab => tab.type === '3d-viewport');
      if (sceneTab && !sceneTab.hasUnsavedChanges) {
        viewportActions.setTabUnsavedChanges(sceneTab.id, true);
        console.log('🔸 SceneManager: Marked scene tab as having unsaved changes');
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
    
    console.log('🌳 SceneManager: Updated scene tree name to:', sceneName);
  }
}

export const sceneManager = new SceneManager();