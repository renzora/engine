import { renderStore, renderActions } from '@/render/store.jsx';
import { engineStore, engineActions, engineGetters } from '@/stores/EngineStore.jsx';
import { createEffect } from 'solid-js';

/**
 * Store Integration Bridge
 * Provides bidirectional synchronization between renderStore and engineStore
 * This allows the existing UI to work with the new engine architecture
 */

let integrationInitialized = false;

/**
 * Initialize bidirectional store synchronization
 * Call this once when the application starts
 */
export function initializeStoreIntegration() {
  if (integrationInitialized) {
    console.warn('⚠️ Store integration already initialized');
    return;
  }

  console.log('🔗 Initializing store integration bridge...');

  // Sync engine object selection to render store
  createEffect(() => {
    const selectedIds = engineStore.editor.viewport.selection;
    const currentScene = engineGetters.getCurrentSceneData();
    
    if (currentScene && selectedIds.length > 0) {
      // Convert engine IDs to babylon objects for renderStore
      const babylonObjects = selectedIds
        .map(id => engineGetters.getBabylonObject(id))
        .filter(Boolean);
      
      if (babylonObjects.length > 0) {
        // Update renderStore selection without triggering loops
        const currentSelected = renderStore.selectedObjects || [];
        if (JSON.stringify(currentSelected.map(obj => obj.uniqueId || obj.id)) 
            !== JSON.stringify(babylonObjects.map(obj => obj.uniqueId || obj.id))) {
          // Select first object, then add others with multi-select
          renderActions.selectObject(babylonObjects[0]);
          for (let i = 1; i < babylonObjects.length; i++) {
            renderActions.selectObject(babylonObjects[i], true);
          }
        }
      }
    } else {
      // Clear selection if no objects selected
      if (renderStore.selectedObjects.length > 0) {
        renderActions.selectObject(null);
      }
    }
  });

  // Sync render store selection back to engine store
  createEffect(() => {
    const selectedBabylonObjects = renderStore.selectedObjects || [];
    const babylonBridge = engineGetters.getSystemManager('babylonBridge');
    
    if (babylonBridge && selectedBabylonObjects.length > 0) {
      // Convert babylon objects back to engine IDs
      const engineIds = selectedBabylonObjects
        .map(obj => babylonBridge.engineMap.get(obj))
        .filter(Boolean);
      
      // Update engine store if different
      const currentSelection = engineStore.editor.viewport.selection;
      if (JSON.stringify(currentSelection) !== JSON.stringify(engineIds)) {
        engineActions.setSelection(engineIds);
      }
    } else if (selectedBabylonObjects.length === 0) {
      // Clear engine selection
      if (engineStore.editor.viewport.selection.length > 0) {
        engineActions.clearSelection();
      }
    }
  });

  // Sync scene loading state
  createEffect(() => {
    const currentSceneId = engineStore.scenes.currentScene;
    const babylonBridge = engineGetters.getSystemManager('babylonBridge');
    
    if (currentSceneId && babylonBridge) {
      // Sync scene objects when scene changes
      babylonBridge.syncSceneObjects(currentSceneId);
    }
  });

  // Sync project settings
  createEffect(() => {
    const renderingSettings = engineStore.project.settings.rendering;
    
    // Update render store settings from engine store
    if (renderingSettings) {
      renderActions.updateSettings({
        renderingEngine: 'webgl', // Keep current
        // Add other settings sync as needed
      });
    }
  });

  integrationInitialized = true;
  console.log('✅ Store integration bridge initialized');
}

/**
 * Helper to ensure object is in both stores when added
 */
export function addObjectToScene(objectData, parentId = null) {
  const currentSceneId = engineGetters.getCurrentSceneId();
  if (!currentSceneId) {
    console.error('❌ No current scene to add object to');
    return null;
  }

  // Add to engine store first
  const objectId = engineActions.addObjectToScene(currentSceneId, {
    ...objectData,
    parent: parentId || 'root_node'
  });

  // The BabylonBridge should automatically create the babylon object
  // and renderStore will be updated via the integration effects

  return objectId;
}

/**
 * Helper to ensure object is removed from both stores
 */
export function removeObjectFromScene(objectId) {
  const currentSceneId = engineGetters.getCurrentSceneId();
  if (!currentSceneId) {
    console.error('❌ No current scene to remove object from');
    return false;
  }

  // Remove from engine store first
  engineActions.removeObjectFromScene(currentSceneId, objectId);

  // The BabylonBridge should automatically dispose the babylon object
  // and renderStore will be updated via the integration effects

  return true;
}

/**
 * Helper to update object in both stores
 */
export function updateObjectInScene(objectId, updates) {
  const currentSceneId = engineGetters.getCurrentSceneId();
  if (!currentSceneId) {
    console.error('❌ No current scene to update object in');
    return false;
  }

  // Update in engine store
  engineActions.updateObjectInScene(currentSceneId, objectId, updates);

  // The BabylonBridge should automatically update the babylon object
  // and renderStore will be updated via the integration effects

  return true;
}

/**
 * Get current scene hierarchy for UI components
 */
export function getSceneHierarchy() {
  const currentScene = engineGetters.getCurrentSceneData();
  if (!currentScene) return [];

  // Convert engine scene graph to hierarchy format expected by UI
  const nodes = currentScene.sceneGraph.nodes;
  const rootNode = nodes['root_node'];
  
  if (!rootNode) return [];

  const buildHierarchy = (nodeId) => {
    const node = nodes[nodeId];
    if (!node) return null;

    return {
      id: node.id,
      name: node.name,
      type: 'object', // Could be enhanced based on components
      visible: node.metadata?.visible !== false,
      selected: engineStore.editor.viewport.selection.includes(nodeId),
      children: (node.children || []).map(buildHierarchy).filter(Boolean)
    };
  };

  return rootNode.children.map(buildHierarchy).filter(Boolean);
}

/**
 * Helper functions for backward compatibility with existing code
 */
export const storeIntegration = {
  addObject: addObjectToScene,
  removeObject: removeObjectFromScene,
  updateObject: updateObjectInScene,
  getHierarchy: getSceneHierarchy,
  
  // Getters for common data
  getCurrentScene: () => engineGetters.getCurrentSceneData(),
  getSelection: () => engineGetters.getSelection(),
  getSelectedObjects: () => engineGetters.getSelectedObjectsData(),
  
  // State helpers
  isPlaying: () => engineGetters.isPlaying(),
  isPaused: () => engineGetters.isPaused(),
  hasUnsavedChanges: () => engineGetters.hasUnsavedChanges(),
};

export default { initializeStoreIntegration, storeIntegration };