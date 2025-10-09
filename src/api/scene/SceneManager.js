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

      // Get camera settings from camera store
      let cameraSettings = null;
      try {
        const { cameraSettings: getCameraSettings } = await import('@/plugins/core/camera/cameraStore.jsx');
        cameraSettings = getCameraSettings();
      } catch (error) {
        console.warn('Failed to get camera settings:', error);
      }

      // Get editor settings from editor store
      let editorSettings = null;
      try {
        const { editorStore } = await import('@/layout/stores/EditorStore.jsx');
        editorSettings = {
          settings: editorStore.settings,
          theme: editorStore.theme,
          ui: {
            currentMode: editorStore.ui.currentMode
          },
          scripts: editorStore.scripts
        };
      } catch (error) {
        console.warn('Failed to get editor settings:', error);
      }

      // Get UI state including color codes from Scene component
      let uiState = null;
      try {
        // Try to get color codes from the Scene component using a synchronous approach
        let colorCodes = {};
        
        // Create a promise-based approach for better synchronization
        const getColorCodes = () => {
          return new Promise((resolve) => {
            const colorCodesHandler = (e) => {
              document.removeEventListener('sceneColorCodesResponse', colorCodesHandler);
              resolve(e.detail.colorCodes || {});
            };
            
            document.addEventListener('sceneColorCodesResponse', colorCodesHandler);
            
            // Request color codes
            const sceneColorCodesEvent = new CustomEvent('getSceneColorCodes');
            document.dispatchEvent(sceneColorCodesEvent);
            
            // Fallback timeout
            setTimeout(() => {
              document.removeEventListener('sceneColorCodesResponse', colorCodesHandler);
              resolve({});
            }, 100);
          });
        };
        
        colorCodes = await getColorCodes();
        console.log('💾 Retrieved color codes for saving:', colorCodes);
        
        uiState = {
          colorCodes: colorCodes
        };
      } catch (error) {
        console.warn('Failed to get UI state for scene saving:', error);
        uiState = { colorCodes: {} };
      }

      // Create serializable scene data
      const sceneData = {
        hierarchy: this.serializeHierarchy(renderStore.hierarchy),
        lighting: renderStore.lighting,
        settings: renderStore.settings,
        cameraSettings: cameraSettings,
        editorSettings: editorSettings,
        uiState: uiState,
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
   * Clear the current scene without resetting hierarchy (for restoration)
   */
  clearSceneWithoutHierarchyReset() {
    const scene = renderStore.scene;
    if (!scene) return;

    // Clearing current scene without hierarchy reset

    // Remove all user objects (keep system objects)
    const objectsToRemove = [
      ...scene.meshes.filter(m => !this.isSystemObject(m)),
      ...scene.transformNodes.filter(n => !this.isSystemObject(n)),
      ...scene.lights.filter(l => !this.isSystemObject(l))
    ];

    objectsToRemove.forEach(obj => {
      renderActions.removeObject(obj);
    });

    // Don't call initializeHierarchy() - hierarchy will be restored separately
    this.markAsModified();
  }

  /**
   * Restore complete hierarchy structure including virtual folders and their relationships
   * @param {Array} savedHierarchy - The saved hierarchy from scene data
   * @param {Object} assets - Bundled assets
   * @param {Function} dispatchProgress - Progress callback
   */
  async restoreCompleteHierarchy(savedHierarchy, assets, dispatchProgress = null) {
    // First pass: Create all Babylon objects without hierarchy relationships
    const objectMap = new Map(); // Map saved ID to restored Babylon object
    
    // Create all objects first
    await this.createAllBabylonObjects(savedHierarchy, assets, objectMap, dispatchProgress);
    
    // Second pass: Restore complete hierarchy structure with proper relationships
    const restoredHierarchy = await this.buildRestoredHierarchy(savedHierarchy, objectMap);
    
    // Set the complete hierarchy in one operation  
    const { setRenderStore } = await import('@/render/store.jsx');
    setRenderStore('hierarchy', restoredHierarchy);
    
    console.log('✅ Complete hierarchy structure restored with virtual folders');
  }

  /**
   * First pass: Create all Babylon objects from saved data
   * @param {Array} hierarchy - Hierarchy items to process
   * @param {Object} assets - Bundled assets
   * @param {Map} objectMap - Map to store created objects
   * @param {Function} dispatchProgress - Progress callback
   */
  async createAllBabylonObjects(hierarchy, assets, objectMap, dispatchProgress = null) {
    for (const item of hierarchy) {
      // Skip scene root and virtual folders - they don't create Babylon objects
      if (item.type !== 'scene' && !item.isVirtual && item.babylonData) {
        const babylonObject = await this.createBabylonObjectFromData(item, assets, dispatchProgress);
        if (babylonObject) {
          objectMap.set(item.id, babylonObject);
        }
      }
      
      // Process children recursively
      if (item.children) {
        await this.createAllBabylonObjects(item.children, assets, objectMap, dispatchProgress);
      }
    }
  }

  /**
   * Create a single Babylon object from saved data (without adding to hierarchy)
   * @param {Object} item - Hierarchy item
   * @param {Object} assets - Bundled assets
   * @param {Function} dispatchProgress - Progress callback
   * @returns {Object|null} Created Babylon object
   */
  async createBabylonObjectFromData(item, assets) {
    const scene = renderStore.scene;
    if (!scene) return null;

    try {
      // Check if this object has asset data (3D model)
      const assetPath = item.babylonData?.metadata?.originalAssetData?.path;
      
      if (assetPath && assets[assetPath]) {
        // Create 3D model from bundled asset
        return await this.restore3DModelFromAsset(item, assets[assetPath], scene, false); // false = don't add to render store yet
      } else {
        // Create basic object without asset data
        return await this.restoreBasicObject(item, scene, false); // false = don't add to render store yet
      }
    } catch (error) {
      console.error(`❌ SceneManager: Failed to create object '${item.name}':`, error);
      return null;
    }
  }

  /**
   * Second pass: Build restored hierarchy with proper structure and relationships
   * @param {Array} savedHierarchy - Original saved hierarchy
   * @param {Map} objectMap - Map of created Babylon objects
   * @returns {Array} Complete restored hierarchy
   */
  async buildRestoredHierarchy(savedHierarchy, objectMap) {
    const scene = renderStore.scene;
    
    const restoreHierarchyItems = (items) => {
      return items.map(item => {
        if (item.type === 'scene') {
          // Scene root
          return {
            id: scene.uniqueId || 'scene-root',
            name: item.name || 'Scene',
            type: 'scene',
            expanded: item.expanded !== undefined ? item.expanded : true,
            babylonObject: scene,
            children: item.children ? restoreHierarchyItems(item.children) : []
          };
        } else if (item.isVirtual && item.type === 'folder') {
          // Virtual folder
          return {
            id: item.id,
            name: item.name,
            type: item.type,
            visible: item.visible !== undefined ? item.visible : true,
            expanded: item.expanded !== undefined ? item.expanded : false,
            children: item.children ? restoreHierarchyItems(item.children) : [],
            isVirtual: true
          };
        } else {
          // Regular Babylon object
          const babylonObject = objectMap.get(item.id);
          if (babylonObject) {
            // Add to render store now that we have the complete hierarchy
            renderActions.addObject(babylonObject);
            
            // Get type and light type from the Babylon object
            const { type, lightType } = this.getBabylonObjectTypeAndLightType(babylonObject);
            
            return {
              id: babylonObject.uniqueId || babylonObject.name,
              name: babylonObject.name,
              type: type,
              lightType: lightType,
              visible: babylonObject.isVisible !== false,
              expanded: item.expanded !== undefined ? item.expanded : false,
              babylonObject: babylonObject,
              children: item.children ? restoreHierarchyItems(item.children) : []
            };
          } else {
            // Object creation failed, skip it
            console.warn(`⚠️ SceneManager: Skipping failed object restoration: ${item.name}`);
            return null;
          }
        }
      }).filter(item => item !== null);
    };

    return restoreHierarchyItems(savedHierarchy);
  }

  /**
   * Get Babylon object type and light type for hierarchy display
   * @param {Object} babylonObject - Babylon.js object
   * @returns {Object} {type: string, lightType: string|null}
   */
  getBabylonObjectTypeAndLightType(babylonObject) {
    if (!babylonObject) return { type: 'unknown', lightType: null };
    
    const className = babylonObject.getClassName?.() || 'Unknown';
    let type = 'mesh';
    let lightType = null;
    
    // Check for special object types first (same logic as render store)
    if (babylonObject._terrainData) {
      type = 'terrain';
    } else if (babylonObject.metadata?.isEnvironmentObject || 
               babylonObject.name?.toLowerCase().includes('skybox') ||
               babylonObject.name?.toLowerCase() === 'skybox' ||
               (babylonObject.infiniteDistance === true && babylonObject.renderingGroupId === 0)) {
      type = 'skybox';
    } else if (babylonObject.name?.toLowerCase().includes('terrain') || 
               (babylonObject.material && babylonObject.getVerticesData && 
                babylonObject.getIndices && babylonObject.name?.toLowerCase() === 'terrain')) {
      type = 'terrain';
    } else if (className.includes('Light')) {
      type = 'light';
      lightType = className.toLowerCase().replace('light', '');
    } else if (className.includes('Camera')) {
      type = 'camera';
    } else if (className === 'TransformNode') {
      // Check if this is a light container first
      if (babylonObject.metadata?.isLightContainer) {
        type = 'light';
        lightType = babylonObject.metadata.lightType || 'directional';
      } else {
        // Check if this has light children (light containers created by restoration)
        const hasLightChildren = babylonObject.getChildren && 
                                babylonObject.getChildren().some(child => 
                                  child.getClassName && child.getClassName().includes('Light')
                                );
        
        if (hasLightChildren) {
          type = 'light';
          // Try to determine light type from child
          const lightChild = babylonObject.getChildren().find(child => 
            child.getClassName && child.getClassName().includes('Light')
          );
          if (lightChild) {
            lightType = lightChild.getClassName().toLowerCase().replace('light', '');
          }
        } else {
          // Check if this is an imported asset container (has mesh children)
          const hasMeshChildren = babylonObject.getChildren && 
                                babylonObject.getChildren().some(child => 
                                  child.getClassName && child.getClassName().includes('Mesh')
                                );
          type = hasMeshChildren ? 'mesh' : 'transformNode';
        }
      }
    } else if (className.includes('Mesh')) {
      type = 'mesh';
    } else if (className === 'Scene') {
      type = 'scene';
    }
    
    return { type, lightType };
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

    // Save virtual folder flag if present
    if (item.isVirtual) {
      serialized.isVirtual = item.isVirtual;
    }

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
        
        // Special handling for light containers - capture child light properties
        if (babylonObj.metadata?.isLightContainer) {
          const lightChild = babylonObj.getChildren().find(child => 
            child.getClassName && child.getClassName().includes('Light')
          );
          
          if (lightChild) {
            // Add light-specific properties to serialized data
            serializedData.lightType = babylonObj.metadata.lightType;
            
            // Serialize light color properties
            if (lightChild.diffuse) {
              serializedData.diffuse = [lightChild.diffuse.r, lightChild.diffuse.g, lightChild.diffuse.b];
            }
            if (lightChild.specular) {
              serializedData.specular = [lightChild.specular.r, lightChild.specular.g, lightChild.specular.b];
            }
            if (lightChild.intensity !== undefined) {
              serializedData.intensity = lightChild.intensity;
            }
            
            // Type-specific properties
            if (lightChild.direction && (babylonObj.metadata.lightType === 'directional' || babylonObj.metadata.lightType === 'spot')) {
              serializedData.direction = [lightChild.direction.x, lightChild.direction.y, lightChild.direction.z];
            }
            if (lightChild.groundColor && babylonObj.metadata.lightType === 'hemisphere') {
              serializedData.groundColor = [lightChild.groundColor.r, lightChild.groundColor.g, lightChild.groundColor.b];
            }
            if (lightChild.angle !== undefined && babylonObj.metadata.lightType === 'spot') {
              serializedData.angle = lightChild.angle;
            }
            if (lightChild.exponent !== undefined && babylonObj.metadata.lightType === 'spot') {
              serializedData.exponent = lightChild.exponent;
            }
            
            console.log(`💾 Saved light properties for ${babylonObj.name}:`, {
              diffuse: serializedData.diffuse,
              specular: serializedData.specular,
              intensity: serializedData.intensity
            });
          }
        }
      } else if (babylonObj.getClassName() === 'UniversalCamera') {
        // For cameras, use camera serializer
        serializedData = babylonObj.serialize();
      } else if (babylonObj.getClassName() === 'Scene') {
        // For Scene objects, we only need basic metadata (scripts are handled separately)
        serializedData = {
          name: babylonObj.name || 'Scene',
          className: babylonObj.getClassName(),
          id: babylonObj.uniqueId,
          metadata: babylonObj.metadata
        };
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
        
        // Preserve terrain data for terrain objects
        if (babylonObj._terrainData) {
          serializedData.__terrainData = {
            size: babylonObj._terrainData.size,
            subdivisions: babylonObj._terrainData.subdivisions,
            heightmapData: babylonObj._terrainData.heightmapData,
            brushSize: babylonObj._terrainData.brushSize,
            brushStrength: babylonObj._terrainData.brushStrength,
            brushFalloff: babylonObj._terrainData.brushFalloff,
            isInfiniteTerrain: babylonObj._terrainData.isInfiniteTerrain,
            chunkX: babylonObj._terrainData.chunkX,
            chunkZ: babylonObj._terrainData.chunkZ,
            isTerrainChunk: babylonObj._terrainData.isTerrainChunk
          };
          console.log('💾 Preserved terrain data for object:', babylonObj.name);
        }
        
        // Preserve terrain system data for terrain system objects
        if (babylonObj._isTerrainSystem) {
          serializedData.__isTerrainSystem = true;
          console.log('💾 Preserved terrain system marker for object:', babylonObj.name);
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

    // Restore editor settings if available
    if (sceneData.editorSettings) {
      try {
        const { editorActions } = await import('@/layout/stores/EditorStore.jsx');
        editorActions.loadFromProject(sceneData.editorSettings);
        console.log('✅ Restored editor settings from scene data');
      } catch (error) {
        console.warn('Failed to restore editor settings:', error);
      }
    }

    // TODO: Restore scene objects from hierarchy
    // This will require recreating Babylon objects from the serialized data
    // Scene object restoration not yet implemented
    
    // For now, just restore the hierarchy structure without Babylon objects
    if (sceneData.hierarchy) {
      this.cleanHierarchyForDisplay(sceneData.hierarchy);
      renderActions.initializeHierarchy();
    }
  }

  /**
   * Restore scene from bundled data (scene + assets)
   * @param {Object} bundleData - Bundle containing scene data and assets
   */
  async restoreSceneFromBundledData(bundleData, dispatchProgress = null) {
    // Starting scene restoration from bundled data

    // Clear current scene first (but don't call initializeHierarchy yet)
    if (dispatchProgress) dispatchProgress('Clearing current scene...');
    // Clearing current scene
    this.clearSceneWithoutHierarchyReset();

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

    // Restore the complete hierarchy structure first (including virtual folders)
    if (sceneData.hierarchy) {
      if (dispatchProgress) dispatchProgress('Restoring hierarchy structure...');
      await this.restoreCompleteHierarchy(sceneData.hierarchy, assets, dispatchProgress);
    } else {
      console.warn('⚠️ SceneManager: No hierarchy found in scene data');
      // Initialize empty hierarchy if no saved hierarchy exists
      renderActions.initializeHierarchy();
    }

    // Restore camera settings if available
    if (sceneData.cameraSettings) {
      try {
        const { cameraActions } = await import('@/plugins/core/camera/cameraStore.jsx');
        // Restore FOV setting
        if (sceneData.cameraSettings.fov !== undefined) {
          cameraActions.setFOV(sceneData.cameraSettings.fov);
        }
        // Restore vignette settings
        if (sceneData.cameraSettings.vignette) {
          cameraActions.setVignetteEnabled(sceneData.cameraSettings.vignette.enabled || false);
          cameraActions.setVignetteAmount(sceneData.cameraSettings.vignette.amount || 0.5);
          cameraActions.setVignetteColor(sceneData.cameraSettings.vignette.color || [0, 0, 0]);
        }
        console.log('✅ Restored camera settings from scene data');
      } catch (error) {
        console.warn('Failed to restore camera settings:', error);
      }
    }

    // Restore editor settings if available
    if (sceneData.editorSettings) {
      try {
        const { editorActions } = await import('@/layout/stores/EditorStore.jsx');
        editorActions.loadFromProject(sceneData.editorSettings);
        console.log('✅ Restored editor settings from scene data');
      } catch (error) {
        console.warn('Failed to restore editor settings:', error);
      }
    }

    // Restore UI state (color codes) if available - with delay to ensure Scene component is ready
    if (sceneData.uiState) {
      try {
        console.log('🔄 SceneManager: Restoring UI state:', sceneData.uiState);
        
        // Try multiple times with increasing delays to ensure Scene component is ready
        const colorCodes = sceneData.uiState.colorCodes || {};
        const attemptRestore = (attempt = 1) => {
          const restoreColorCodesEvent = new CustomEvent('restoreSceneColorCodes', {
            detail: { colorCodes }
          });
          document.dispatchEvent(restoreColorCodesEvent);
          console.log(`✅ Attempt ${attempt}: Dispatched color codes restoration event with data:`, colorCodes);
          
          // Try again with longer delay if this is the first few attempts
          if (attempt < 3) {
            setTimeout(() => attemptRestore(attempt + 1), attempt * 500);
          }
        };
        
        // Start first attempt immediately
        attemptRestore(1);
        
        console.log('✅ Scheduled UI state restoration');
      } catch (error) {
        console.warn('Failed to restore UI state:', error);
      }
    } else {
      console.log('🔄 SceneManager: No UI state found in scene data');
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

    // Handle scene root - process children but don't create a new Babylon object
    if (item.type === 'scene') {
      // For scene objects, restore scripts through normal babylon data processing
      if (item.babylonData) {
        await this.restoreObjectFromBabylonData(item, assets);
      }
      
      // Process children of scene root
      if (item.children) {
        for (let i = 0; i < item.children.length; i++) {
          const child = item.children[i];
          await this.restoreHierarchyItem(child, assets, progressTracker);
        }
      }
      return;
    }

    // Restore object based on babylon data (cameras, lights, meshes - all get restored)
    if (item.babylonData) {
      // Restoring object with babylon data
      await this.restoreObjectFromBabylonData(item, assets);
    } else if (item.isVirtual && item.type === 'folder') {
      // Restore virtual folder to render store hierarchy
      console.log(`🔄 SceneManager: Restoring virtual folder '${item.name}'`);
      const virtualFolder = {
        id: item.id,
        name: item.name,
        type: item.type,
        visible: item.visible !== undefined ? item.visible : true,
        expanded: item.expanded !== undefined ? item.expanded : false,
        children: [],
        isVirtual: true
      };
      
      // Add virtual folder to render store
      renderActions.addVirtualFolder(virtualFolder);
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

    // Log detailed restoration attempt
    console.log(`🔄 SceneManager: Restoring object '${item.name}' of type '${item.type}' with babylonType '${item.babylonData?.type || 'unknown'}'`);

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
   * @param {boolean} addToRenderStore - Whether to add to render store immediately (default: true)
   */
  async restore3DModelFromAsset(item, assetData, scene, addToRenderStore = true) {
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
        result.meshes.forEach((mesh) => {
          if (mesh.name !== "__root__") {
            mesh.setParent(container);
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

        // Add to render store if requested
        if (addToRenderStore) {
          // Adding container to render store
          renderActions.addObject(container);
        }
        
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
   * @param {boolean} addToRenderStore - Whether to add to render store immediately (default: true)
   * @returns {Object|null} Restored Babylon object or null
   */
  async restoreBasicObject(item, scene, addToRenderStore = true) {
    // Restoring basic object
    
    try {
      const babylonData = item.babylonData;
      if (!babylonData) {
        console.warn(`⚠️ SceneManager: No babylon data for ${item.name}`);
        return null;
      }

      let restoredObject = null;

      // Handle terrain objects specially
      if (babylonData.__terrainData || babylonData.__isTerrainSystem) {
        // Restoring terrain object
        console.log('🏔️ Restoring terrain object:', item.name);
        
        // Import terrain creation function
        const { createTerrainMesh } = await import('@/plugins/core/terrain/index.jsx');
        const { StandardMaterial } = await import('@babylonjs/core/Materials/standardMaterial.js');
        const { Color3 } = await import('@babylonjs/core/Maths/math.color.js');
        
        if (babylonData.__isTerrainSystem) {
          // Restore terrain system manager
          const { MeshBuilder } = await import('@babylonjs/core/Meshes/meshBuilder.js');
          const terrainManager = MeshBuilder.CreateBox(babylonData.name, { size: 0.1 }, scene);
          terrainManager.visibility = 0;
          terrainManager.isPickable = false;
          terrainManager._isTerrainSystem = true;
          terrainManager._terrainData = babylonData.__terrainData;
          
          // Restore position
          if (babylonData.position) {
            terrainManager.position.fromArray(babylonData.position);
          }
          
          // Set unique ID to match saved data for color code restoration
          terrainManager.uniqueId = babylonData.uniqueId || babylonData.__engineObjectId;
          
          // Restore metadata
          if (babylonData.metadata) {
            terrainManager.metadata = babylonData.metadata;
          }
          
          restoredObject = terrainManager;
        } else if (babylonData.__terrainData) {
          // Restore terrain mesh
          const terrainData = babylonData.__terrainData;
          const terrainMesh = createTerrainMesh(
            babylonData.name,
            terrainData.size,
            terrainData.subdivisions,
            terrainData.heightmapData,
            scene
          );
          
          // Restore material
          const material = new StandardMaterial(`${babylonData.name}_material`, scene);
          material.diffuseColor = new Color3(0.2, 0.8, 0.2);
          material.backFaceCulling = false;
          terrainMesh.material = material;
          
          // Restore position, rotation, scaling
          if (babylonData.position) {
            terrainMesh.position.fromArray(babylonData.position);
          }
          if (babylonData.rotation) {
            terrainMesh.rotation.fromArray(babylonData.rotation);
          }
          if (babylonData.scaling) {
            terrainMesh.scaling.fromArray(babylonData.scaling);
          }
          
          // Restore terrain data
          terrainMesh._terrainData = terrainData;
          
          // Set unique ID to match saved data for color code restoration
          terrainMesh.uniqueId = babylonData.uniqueId || babylonData.__engineObjectId;
          
          // Restore metadata
          if (babylonData.metadata) {
            terrainMesh.metadata = babylonData.metadata;
          }
          
          // Make visible if it was modified
          const hasModifiedTerrain = terrainData.heightmapData && 
            terrainData.heightmapData.some(height => Math.abs(height) > 0.001);
          if (hasModifiedTerrain) {
            terrainMesh.visibility = 1;
            material.alpha = 1;
          } else {
            terrainMesh.visibility = 0.001;
            material.alpha = 0;
          }
          
          restoredObject = terrainMesh;
        }
        
        console.log('🏔️ Terrain object restored successfully:', restoredObject?.name, 'with uniqueId:', restoredObject?.uniqueId);
        
        // Add terrain object to render store if successfully restored and requested
        if (restoredObject && addToRenderStore) {
          renderActions.addObject(restoredObject);
        }
      } else if (item.type === 'camera') {
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
        if (addToRenderStore) {
          renderActions.addObject(camera);
        }
        
        restoredObject = camera;
        // Camera restored successfully
        // Camera has scripts to attach
        
      } else if (item.type === 'mesh' && babylonData.type === 'Mesh') {
        // Restoring primitive mesh object
        console.log('🔲 Restoring mesh object:', item.name);
        
        // Import required Babylon classes
        const { MeshBuilder } = await import('@babylonjs/core/Meshes/meshBuilder.js');
        const { StandardMaterial } = await import('@babylonjs/core/Materials/standardMaterial.js');
        const { Color3 } = await import('@babylonjs/core/Maths/math.color.js');
        
        // Import specific builders based on mesh type (required for side effects)
        await import('@babylonjs/core/Meshes/Builders/boxBuilder.js');
        await import('@babylonjs/core/Meshes/Builders/sphereBuilder.js');
        await import('@babylonjs/core/Meshes/Builders/cylinderBuilder.js');
        await import('@babylonjs/core/Meshes/Builders/planeBuilder.js');
        
        let mesh = null;
        
        // Determine mesh type based on geometry or name
        const meshName = babylonData.name.toLowerCase();
        if (meshName.includes('cube') || meshName.includes('box')) {
          mesh = MeshBuilder.CreateBox(babylonData.name, { size: 1 }, scene);
        } else if (meshName.includes('sphere')) {
          mesh = MeshBuilder.CreateSphere(babylonData.name, { diameter: 1 }, scene);
        } else if (meshName.includes('cylinder')) {
          mesh = MeshBuilder.CreateCylinder(babylonData.name, { height: 1, diameter: 1 }, scene);
        } else if (meshName.includes('plane') || meshName.includes('ground')) {
          mesh = MeshBuilder.CreateGround(babylonData.name, { width: 1, height: 1 }, scene);
        } else {
          // Default to box for unknown mesh types
          mesh = MeshBuilder.CreateBox(babylonData.name, { size: 1 }, scene);
        }
        
        if (mesh) {
          // Restore material if material data exists
          if (babylonData.materialId) {
            // Create and restore material
            const material = new StandardMaterial(babylonData.materialId, scene);
            
            // Apply default colors based on mesh type
            const defaultColors = {
              cube: new Color3(0.2, 0.6, 1.0),     // Light blue
              sphere: new Color3(1.0, 0.4, 0.2),   // Orange-red
              cylinder: new Color3(0.2, 0.8, 0.4), // Green
              plane: new Color3(0.6, 0.6, 0.6)     // Light gray
            };
            
            let colorKey = 'cube'; // default
            if (meshName.includes('sphere')) colorKey = 'sphere';
            else if (meshName.includes('cylinder')) colorKey = 'cylinder';
            else if (meshName.includes('plane') || meshName.includes('ground')) colorKey = 'plane';
            
            material.diffuseColor = defaultColors[colorKey];
            material.emissiveColor = defaultColors[colorKey].scale(0.1);
            mesh.material = material;
            
            // Set material unique ID to match saved data
            material.uniqueId = babylonData.materialUniqueId;
          }
          
          // Restore transform properties
          if (babylonData.position) {
            mesh.position.fromArray(babylonData.position);
          }
          if (babylonData.rotation) {
            mesh.rotation.fromArray(babylonData.rotation);
          }
          if (babylonData.scaling) {
            mesh.scaling.fromArray(babylonData.scaling);
          }
          
          // Restore other mesh properties
          if (babylonData.isVisible !== undefined) {
            mesh.isVisible = babylonData.isVisible;
          }
          if (babylonData.visibility !== undefined) {
            mesh.visibility = babylonData.visibility;
          }
          if (babylonData.receiveShadows !== undefined) {
            mesh.receiveShadows = babylonData.receiveShadows;
          }
          if (babylonData.checkCollisions !== undefined) {
            mesh.checkCollisions = babylonData.checkCollisions;
          }
          
          // Restore metadata
          if (babylonData.metadata) {
            mesh.metadata = babylonData.metadata;
          }
          
          // Set unique ID to match saved data
          mesh.uniqueId = babylonData.uniqueId || babylonData.__engineObjectId;
          
          // Add shadow casting if there's a shadow generator
          if (scene.shadowGenerator && mesh.receiveShadows) {
            scene.shadowGenerator.addShadowCaster(mesh);
          }
          
          // Add to render store if requested
          if (addToRenderStore) {
            renderActions.addObject(mesh);
          }
          
          restoredObject = mesh;
          console.log('🔲 Mesh object restored successfully:', mesh.name);
        }
        
      } else if (item.type === 'light' || (babylonData.lightType || babylonData.type?.includes('Light'))) {
        // Restoring light object
        console.log('💡 Restoring light object:', item.name);
        
        // Import required Babylon light classes
        const { PointLight } = await import('@babylonjs/core/Lights/pointLight.js');
        const { DirectionalLight } = await import('@babylonjs/core/Lights/directionalLight.js');
        const { HemisphericLight } = await import('@babylonjs/core/Lights/hemisphericLight.js');
        const { SpotLight } = await import('@babylonjs/core/Lights/spotLight.js');
        const { TransformNode } = await import('@babylonjs/core/Meshes/transformNode.js');
        const { MeshBuilder } = await import('@babylonjs/core/Meshes/meshBuilder.js');
        const { StandardMaterial } = await import('@babylonjs/core/Materials/standardMaterial.js');
        const { Color3 } = await import('@babylonjs/core/Maths/math.color.js');
        const { Vector3 } = await import('@babylonjs/core/Maths/math.vector.js');
        
        // Create main container for the light (lights are wrapped in containers)
        const containerName = babylonData.name || item.name;
        const mainContainer = new TransformNode(containerName, scene);
        
        // Restore container transform
        if (babylonData.position) {
          mainContainer.position.fromArray(babylonData.position);
        }
        if (babylonData.rotation) {
          mainContainer.rotation.fromArray(babylonData.rotation);
        }
        if (babylonData.scaling) {
          mainContainer.scaling.fromArray(babylonData.scaling);
        }
        
        // Determine light type from the light type property or name
        let lightType = babylonData.lightType || 'directional';
        if (!lightType) {
          // Try to infer from name or other properties
          const lightName = babylonData.name.toLowerCase();
          if (lightName.includes('point')) lightType = 'point';
          else if (lightName.includes('spot')) lightType = 'spot';
          else if (lightName.includes('hemisphere') || lightName.includes('ambient')) lightType = 'hemisphere';
          else lightType = 'directional';
        }
        
        let light = null;
        const lightName = `${containerName}_light`;
        
        // Create the appropriate light type
        switch (lightType) {
          case 'point':
            light = new PointLight(lightName, Vector3.Zero(), scene);
            light.diffuse = new Color3(1, 0.95, 0.8);
            light.specular = new Color3(1, 1, 1);
            light.intensity = 10;
            break;
            
          case 'spot':
            light = new SpotLight(lightName, Vector3.Zero(), new Vector3(0, -1, 0), Math.PI / 3, 2, scene);
            light.diffuse = new Color3(1, 0.95, 0.8);
            light.specular = new Color3(1, 1, 1);
            light.intensity = 15;
            break;
            
          case 'hemisphere':
            light = new HemisphericLight(lightName, new Vector3(0, 1, 0), scene);
            light.diffuse = new Color3(1, 0.95, 0.8);
            light.groundColor = new Color3(0.3, 0.3, 0.3);
            light.intensity = 0.7;
            break;
            
          default: // directional
            light = new DirectionalLight(lightName, new Vector3(-1, -1, -1), scene);
            light.diffuse = new Color3(1, 0.95, 0.8);
            light.specular = new Color3(1, 1, 1);
            light.intensity = 1;
            break;
        }
        
        if (light) {
          // Restore light-specific properties from saved data
          if (babylonData.diffuse) {
            light.diffuse = new Color3(babylonData.diffuse[0], babylonData.diffuse[1], babylonData.diffuse[2]);
          }
          if (babylonData.specular) {
            light.specular = new Color3(babylonData.specular[0], babylonData.specular[1], babylonData.specular[2]);
          }
          if (babylonData.intensity !== undefined) {
            light.intensity = babylonData.intensity;
          }
          
          console.log(`🔄 Restored light properties for ${containerName}:`, {
            diffuse: babylonData.diffuse,
            specular: babylonData.specular,
            intensity: babylonData.intensity,
            lightType: lightType
          });
          if (babylonData.direction && (lightType === 'directional' || lightType === 'spot')) {
            light.direction = new Vector3(babylonData.direction[0], babylonData.direction[1], babylonData.direction[2]);
          }
          if (babylonData.groundColor && lightType === 'hemisphere') {
            light.groundColor = new Color3(babylonData.groundColor[0], babylonData.groundColor[1], babylonData.groundColor[2]);
          }
          if (babylonData.angle !== undefined && lightType === 'spot') {
            light.angle = babylonData.angle;
          }
          if (babylonData.exponent !== undefined && lightType === 'spot') {
            light.exponent = babylonData.exponent;
          }
          
          // Parent light to container
          light.position = Vector3.Zero();
          light.parent = mainContainer;
          
          // Create helper sphere for visualization
          const lightHelper = MeshBuilder.CreateSphere(`${containerName}_helper`, { diameter: 0.5 }, scene);
          const helperMaterial = new StandardMaterial(`${containerName}_helper_material`, scene);
          helperMaterial.emissiveColor = new Color3(1, 1, 0);
          helperMaterial.disableLighting = true;
          lightHelper.material = helperMaterial;
          lightHelper.parent = mainContainer;
          
          // Restore metadata and ensure light container metadata is set
          if (babylonData.metadata) {
            mainContainer.metadata = babylonData.metadata;
          } else {
            mainContainer.metadata = {};
          }
          
          // Ensure light container metadata is properly set
          mainContainer.metadata.isLightContainer = true;
          mainContainer.metadata.lightType = lightType;
          
          // Set unique ID to match saved data
          mainContainer.uniqueId = babylonData.uniqueId || babylonData.__engineObjectId;
          
          // Add to render store if requested
          if (addToRenderStore) {
            renderActions.addObject(mainContainer);
          }
          
          restoredObject = mainContainer;
          console.log('💡 Light object restored successfully:', mainContainer.name, 'type:', lightType);
        }
        
      } else if (item.type === 'transformNode' || item.type === 'folder' || (babylonData.type === 'TransformNode')) {
        // Restoring TransformNode container
        console.log('📦 Restoring TransformNode container:', item.name);
        
        const { TransformNode } = await import('@babylonjs/core/Meshes/transformNode.js');
        
        // Create TransformNode
        const transformNode = new TransformNode(babylonData.name || item.name, scene);
        
        // Restore transform properties
        if (babylonData.position) {
          transformNode.position.fromArray(babylonData.position);
        }
        if (babylonData.rotation) {
          transformNode.rotation.fromArray(babylonData.rotation);
        }
        if (babylonData.scaling) {
          transformNode.scaling.fromArray(babylonData.scaling);
        }
        
        // Restore other properties
        if (babylonData.isEnabled !== undefined) {
          transformNode.isEnabled = babylonData.isEnabled;
        }
        if (babylonData.isVisible !== undefined) {
          transformNode.isVisible = babylonData.isVisible;
        }
        
        // Restore metadata
        if (babylonData.metadata) {
          transformNode.metadata = babylonData.metadata;
        }
        
        // Set unique ID to match saved data
        transformNode.uniqueId = babylonData.uniqueId || babylonData.__engineObjectId;
        
        // Add to render store if requested
        if (addToRenderStore) {
          renderActions.addObject(transformNode);
        }
        
        restoredObject = transformNode;
        console.log('📦 TransformNode restored successfully:', transformNode.name);
        
      } else if (item.type === 'instancedMesh' || (babylonData.type === 'InstancedMesh')) {
        // Restoring InstancedMesh
        console.log('🔢 Restoring InstancedMesh:', item.name);
        
        // InstancedMesh restoration requires the source mesh to exist
        // For now, we'll log and skip, but this could be implemented if needed
        console.warn('⚠️ InstancedMesh restoration not yet implemented:', item.name);
        return null;
        
      } else if (item.type === 'bone' || (babylonData.type === 'Bone')) {
        // Restoring Bone (part of skeleton system)
        console.log('🦴 Restoring Bone:', item.name);
        
        // Bone restoration is complex and typically handled by model loading
        // For now, we'll log and skip
        console.warn('⚠️ Bone restoration handled by skeleton system during model loading:', item.name);
        return null;
        
      } else if (item.type === 'scene') {
        // Scene objects reference the existing scene - no new object creation needed
        // Just return the scene object so scripts can be reattached
        restoredObject = scene;
        
      } else if (item.type === 'skybox') {
        // Restoring skybox object
        console.log('🌐 Restoring skybox object:', item.name);
        
        const { CreateSphere } = await import('@babylonjs/core/Meshes/Builders/sphereBuilder.js');
        const { StandardMaterial } = await import('@babylonjs/core/Materials/standardMaterial.js');
        const { DynamicTexture } = await import('@babylonjs/core/Materials/Textures/dynamicTexture.js');
        const { Texture } = await import('@babylonjs/core/Materials/Textures/texture.js');
        const { Color3 } = await import('@babylonjs/core/Maths/math.color.js');
        
        // Recreate skybox sphere
        const skybox = CreateSphere(babylonData.name || item.name, { diameter: 1000 }, scene);
        skybox.infiniteDistance = true;
        skybox.renderingGroupId = 0;
        skybox.receiveShadows = false;
        skybox.isVisible = true;
        skybox.setEnabled(true);
        
        // Restore position and other transform properties
        if (babylonData.position) {
          skybox.position.fromArray(babylonData.position);
        }
        if (babylonData.rotation) {
          skybox.rotation.fromArray(babylonData.rotation);
        }
        if (babylonData.scaling) {
          skybox.scaling.fromArray(babylonData.scaling);
        }
        
        // Restore metadata
        skybox.metadata = {
          isEnvironmentObject: true,
          skyboxSettings: babylonData.metadata?.skyboxSettings || {
            turbidity: 10,
            luminance: 1.0,
            inclination: 0.5,
            azimuth: 0.25,
            cloudsEnabled: true,
            cloudSize: 25,
            cloudDensity: 0.6,
            cloudOpacity: 0.8,
            color: '#87CEEB'
          }
        };
        
        // Recreate skybox material
        const skyboxMaterial = new StandardMaterial(item.name + 'Material', scene);
        skyboxMaterial.backFaceCulling = false;
        skyboxMaterial.disableLighting = true;
        
        // Create texture
        const skyTexture = new DynamicTexture('skyboxTexture', { width: 512, height: 512 }, scene);
        const textureContext = skyTexture.getContext();
        const skyColor = skybox.metadata.skyboxSettings.color || '#7fccff';
        textureContext.fillStyle = skyColor;
        textureContext.fillRect(0, 0, 512, 512);
        skyTexture.update();
        
        skyboxMaterial.reflectionTexture = skyTexture;
        skyboxMaterial.reflectionTexture.coordinatesMode = Texture.SKYBOX_MODE;
        skyboxMaterial.diffuseColor = new Color3(0, 0, 0);
        skyboxMaterial.specularColor = new Color3(0, 0, 0);
        skybox.material = skyboxMaterial;
        
        // Set scene environment
        scene.environmentTexture = skyTexture;
        scene.environmentIntensity = 1.0;
        
        restoredObject = skybox;
        console.log('🌐 Skybox object restored successfully:', skybox.name);
        
      } else {
        // Handle any other object types that might exist
        console.warn(`⚠️ SceneManager: Unknown object type '${item.type}' for item '${item.name}'`);
        
        // Try to handle as generic Node if it has babylon data
        if (babylonData && babylonData.type) {
          console.log(`🔧 Attempting generic restoration for type '${babylonData.type}':`, item.name);
          
          try {
            // For unknown types, try to create a basic TransformNode as fallback
            const { TransformNode } = await import('@babylonjs/core/Meshes/transformNode.js');
            
            const genericNode = new TransformNode(babylonData.name || item.name, scene);
            
            // Restore basic transform properties
            if (babylonData.position) {
              genericNode.position.fromArray(babylonData.position);
            }
            if (babylonData.rotation) {
              genericNode.rotation.fromArray(babylonData.rotation);
            }
            if (babylonData.scaling) {
              genericNode.scaling.fromArray(babylonData.scaling);
            }
            
            // Restore metadata
            if (babylonData.metadata) {
              genericNode.metadata = babylonData.metadata;
            }
            
            // Set unique ID
            genericNode.uniqueId = babylonData.uniqueId || babylonData.__engineObjectId;
            
            // Add to render store if requested
            if (addToRenderStore) {
              renderActions.addObject(genericNode);
            }
            
            restoredObject = genericNode;
            console.log(`🔧 Generic restoration completed for:`, genericNode.name);
            
          } catch (error) {
            console.error(`❌ Failed generic restoration for '${item.name}':`, error);
            return null;
          }
        } else {
          return null;
        }
      }

      // Log successful restoration
      if (restoredObject) {
        console.log(`✅ SceneManager: Successfully restored object '${item.name}' (type: ${item.type}, babylonType: ${item.babylonData?.type || 'unknown'})`);
      }
      
      return restoredObject;
      
    } catch (error) {
      console.error(`❌ SceneManager: Failed to restore basic object '${item.name}' (type: ${item.type}):`, error);
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

      // Set up properties in objectPropertiesStore BEFORE attaching scripts
      // This ensures properties are available when scripts start
      for (const scriptInfo of attachedScripts) {
        objectPropertiesActions.addPropertySection(objectId, 'scripts', [{
          path: scriptInfo.path,
          name: scriptInfo.name,
          enabled: true,
          properties: scriptInfo.properties || {}
        }]);
        
        // Update Babylon object metadata with saved properties
        if (scriptInfo.properties && Object.keys(scriptInfo.properties).length > 0) {
          if (!babylonObject.metadata) babylonObject.metadata = {};
          if (!babylonObject.metadata.scriptProperties) babylonObject.metadata.scriptProperties = {};
          
          Object.entries(scriptInfo.properties).forEach(([propName, propValue]) => {
            babylonObject.metadata.scriptProperties[propName] = propValue;
          });
        }
      }

      // Attach each script using existing attachScript method (it will auto-detect bulk mode)
      for (const scriptInfo of attachedScripts) {
        // Reattaching individual script
        
        if (dispatchProgress) {
          dispatchProgress('Attaching scripts...', scriptInfo.name);
        }
        
        // Use deferred start to prevent onStart() from running before properties are restored
        const success = await runtime.attachScript(objectId, scriptInfo.path, true);
        
        if (success) {
          const scriptInstance = runtime.getScriptInstance(objectId, scriptInfo.path);
          
          // Apply saved properties to the script instance BEFORE starting
          if (scriptInfo.properties && Object.keys(scriptInfo.properties).length > 0 && scriptInstance) {
            Object.entries(scriptInfo.properties).forEach(([propName, propValue]) => {
              // Update script instance if available
              if (scriptInstance?._scriptAPI?.setScriptProperty) {
                scriptInstance._scriptAPI.setScriptProperty(propName, propValue);
              }
            });
          }

          // Now start the script with the correct properties
          if (scriptInstance) {
            runtime.startScriptInstance(scriptInstance);
          }

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
        if (item.type === 'scene') {
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