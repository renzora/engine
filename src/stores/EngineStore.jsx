import { createStore } from 'solid-js/store';
import { createSignal } from 'solid-js';

// Core engine store - centralized state management for the entire game engine
const [engineStore, setEngineStore] = createStore({
  // Project-level data
  project: {
    metadata: {
      name: "",
      version: "1.0.0",
      created: null,
      modified: null,
      description: "",
      author: "",
      engineVersion: "1.0.0"
    },
    settings: {
      rendering: {
        targetFPS: 60,
        enableVSync: true,
        shadowMapSize: 2048,
        enableFrustumCulling: true,
        enableOcclusionCulling: false,
        antialiasingMode: "FXAA",
        toneMappingType: "ACES"
      },
      physics: {
        gravity: [0, -9.81, 0],
        enablePhysics: true,
        physicsEngine: "havok",
        fixedTimeStep: 1/60,
        maxSubSteps: 3
      },
      audio: {
        masterVolume: 1.0,
        enableSpatialAudio: true,
        dopplerEffect: true,
        maxAudioSources: 32
      }
    },
    buildSettings: {
      platforms: ["web", "desktop"],
      optimization: "production",
      compressionLevel: 5,
      includeSourceMaps: false
    }
  },

  // Asset database - shared across all scenes
  assets: {
    // Indexed by asset ID for O(1) lookups
    geometries: {},
    materials: {},
    textures: {},
    scripts: {},
    blueprints: {},
    audio: {},
    scenes: {},
    models: {},
    animations: {},
    
    // Asset relationships and dependencies
    dependencies: {},
    instances: {}, // Track which objects use which assets
    
    // Asset metadata
    metadata: {
      totalAssets: 0,
      totalSize: 0,
      lastImported: null
    }
  },

  // Scene management - multiple scenes can be loaded
  scenes: {
    activeScenes: [], // Array of loaded scene IDs
    currentScene: null, // Primary active scene
    sceneData: {
      // Example scene structure - will be populated dynamically
      // "scene_001": { metadata, settings, sceneGraph, systems }
    }
  },

  // Editor state - UI and tools
  editor: {
    viewport: {
      activeScenes: [], // Which scenes are open in tabs
      selection: [], // Selected object IDs
      camera: {
        position: [0, 5, 10],
        target: [0, 0, 0],
        mode: "arcRotate" // "arcRotate", "universal", "free"
      },
      tools: {
        currentTool: "select", // "select", "move", "rotate", "scale"
        gizmoMode: "local", // "local", "world"
        snapToGrid: false,
        gridSize: 1.0
      },
      rendering: {
        showWireframe: false,
        showBoundingBoxes: false,
        showNormals: false,
        enablePostProcessing: true
      }
    },
    ui: {
      panels: {
        inspector: { visible: true, width: 300 },
        hierarchy: { visible: true, width: 250 },
        console: { visible: true, height: 200 },
        assetBrowser: { visible: true, height: 300 }
      },
      inspector: {
        expandedSections: [],
        lockedObject: null
      },
      console: {
        messages: [],
        filter: "all", // "all", "info", "warning", "error"
        maxMessages: 1000
      }
    },
    preferences: {
      theme: "dark",
      autoSave: true,
      autoSaveInterval: 300, // seconds
      undoLevels: 50
    }
  },

  // Runtime state - game execution
  runtime: {
    isPlaying: false,
    isPaused: false,
    playbackSpeed: 1.0,
    currentFrame: 0,
    deltaTime: 0,
    totalTime: 0,
    
    // Performance metrics
    performance: {
      fps: 0,
      frameTime: 0,
      drawCalls: 0,
      triangles: 0,
      memoryUsage: 0
    },
    
    // Runtime-only data that doesn't get serialized
    babylonObjects: {}, // engineId -> babylonObject mapping
    systemManagers: {}, // system manager instances
    scriptRuntime: null, // renscript execution context
    
    // Debug information
    debug: {
      enableDebugMode: false,
      showPerformanceStats: false,
      enableConsoleLogging: true,
      logLevel: "info" // "debug", "info", "warning", "error"
    }
  }
});

// Signals for reactive updates
const [isLoading, setIsLoading] = createSignal(false);
const [hasUnsavedChanges, setHasUnsavedChanges] = createSignal(false);
const [lastSaved, setLastSaved] = createSignal(null);

// Engine Store Actions
export const engineActions = {
  // Project management
  setProjectMetadata: (metadata) => {
    setEngineStore('project', 'metadata', metadata);
    setHasUnsavedChanges(true);
  },

  updateProjectSetting: (category, key, value) => {
    setEngineStore('project', 'settings', category, key, value);
    setHasUnsavedChanges(true);
  },

  // Asset management
  addAsset: (category, id, assetData) => {
    setEngineStore('assets', category, id, assetData);
    setEngineStore('assets', 'metadata', 'totalAssets', 
      prev => prev + 1);
    setHasUnsavedChanges(true);
  },

  removeAsset: (category, id) => {
    setEngineStore('assets', category, id, undefined);
    setEngineStore('assets', 'metadata', 'totalAssets', 
      prev => Math.max(0, prev - 1));
    setHasUnsavedChanges(true);
  },

  updateAsset: (category, id, updates) => {
    setEngineStore('assets', category, id, updates);
    setHasUnsavedChanges(true);
  },

  // Asset dependency tracking
  addAssetDependency: (assetId, dependsOnId) => {
    setEngineStore('assets', 'dependencies', assetId, prev => {
      const deps = prev || [];
      return deps.includes(dependsOnId) ? deps : [...deps, dependsOnId];
    });
  },

  removeAssetDependency: (assetId, dependsOnId) => {
    setEngineStore('assets', 'dependencies', assetId, prev => {
      const deps = prev || [];
      return deps.filter(id => id !== dependsOnId);
    });
  },

  // Scene management
  createScene: (sceneData) => {
    const sceneId = sceneData.metadata.id || `scene_${Date.now()}`;
    setEngineStore('scenes', 'sceneData', sceneId, {
      metadata: {
        id: sceneId,
        name: sceneData.metadata.name || "New Scene",
        path: `scenes/${sceneId}.json`,
        created: new Date().toISOString(),
        modified: new Date().toISOString(),
        ...sceneData.metadata
      },
      settings: {
        environment: {
          skyboxId: null,
          fogEnabled: false,
          timeOfDay: 12.0,
          weather: "clear"
        },
        physics: {
          enabled: true,
          gravity: [0, -9.81, 0]
        },
        audio: {
          enabled: true,
          reverbZone: null
        },
        ...sceneData.settings
      },
      sceneGraph: {
        root: "root_node",
        nodes: {
          "root_node": {
            id: "root_node",
            name: "Scene Root",
            children: [],
            transform: { position: [0,0,0], rotation: [0,0,0], scale: [1,1,1] },
            components: {},
            metadata: { tags: [], layer: "default", visible: true }
          }
        }
      },
      systems: {
        physics: { enabled: true, objects: [] },
        audio: { enabled: true, sources: [] },
        particles: { systems: [] },
        animation: { clips: [], playing: [] },
        lighting: { lights: [], shadows: true },
        rendering: { cameras: [] }
      }
    });
    setHasUnsavedChanges(true);
    return sceneId;
  },

  loadScene: (sceneId) => {
    if (!engineStore.scenes.activeScenes.includes(sceneId)) {
      setEngineStore('scenes', 'activeScenes', prev => [...prev, sceneId]);
    }
    setEngineStore('scenes', 'currentScene', sceneId);
  },

  unloadScene: (sceneId) => {
    setEngineStore('scenes', 'activeScenes', prev => 
      prev.filter(id => id !== sceneId));
    
    // If this was the current scene, switch to another or null
    if (engineStore.scenes.currentScene === sceneId) {
      const remaining = engineStore.scenes.activeScenes.filter(id => id !== sceneId);
      setEngineStore('scenes', 'currentScene', remaining[0] || null);
    }
  },

  setCurrentScene: (sceneId) => {
    setEngineStore('scenes', 'currentScene', sceneId);
  },

  // Scene object management
  addObjectToScene: (sceneId, objectData) => {
    const objectId = objectData.id || `obj_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    const object = {
      id: objectId,
      name: objectData.name || "New Object",
      parent: objectData.parent || "root_node",
      children: [],
      transform: {
        position: [0, 0, 0],
        rotation: [0, 0, 0],
        scale: [1, 1, 1],
        ...objectData.transform
      },
      components: objectData.components || {},
      metadata: {
        tags: [],
        layer: "default",
        visible: true,
        ...objectData.metadata
      }
    };

    // Add to scene graph
    setEngineStore('scenes', 'sceneData', sceneId, 'sceneGraph', 'nodes', objectId, object);
    
    // Add to parent's children
    if (object.parent) {
      setEngineStore('scenes', 'sceneData', sceneId, 'sceneGraph', 'nodes', object.parent, 'children', 
        prev => [...(prev || []), objectId]);
    }

    setHasUnsavedChanges(true);
    return objectId;
  },

  removeObjectFromScene: (sceneId, objectId) => {
    const object = engineStore.scenes.sceneData[sceneId]?.sceneGraph?.nodes?.[objectId];
    if (!object) return;

    // Remove from parent's children
    if (object.parent) {
      setEngineStore('scenes', 'sceneData', sceneId, 'sceneGraph', 'nodes', object.parent, 'children',
        prev => (prev || []).filter(id => id !== objectId));
    }

    // Recursively remove children
    if (object.children) {
      object.children.forEach(childId => 
        engineActions.removeObjectFromScene(sceneId, childId));
    }

    // Remove the object itself
    setEngineStore('scenes', 'sceneData', sceneId, 'sceneGraph', 'nodes', objectId, undefined);
    setHasUnsavedChanges(true);
  },

  updateObjectInScene: (sceneId, objectId, updates) => {
    setEngineStore('scenes', 'sceneData', sceneId, 'sceneGraph', 'nodes', objectId, updates);
    setHasUnsavedChanges(true);
  },

  // Editor state management
  setSelection: (objectIds) => {
    const ids = Array.isArray(objectIds) ? objectIds : [objectIds].filter(Boolean);
    setEngineStore('editor', 'viewport', 'selection', ids);
  },

  addToSelection: (objectId) => {
    setEngineStore('editor', 'viewport', 'selection', prev => {
      return prev.includes(objectId) ? prev : [...prev, objectId];
    });
  },

  removeFromSelection: (objectId) => {
    setEngineStore('editor', 'viewport', 'selection', prev => 
      prev.filter(id => id !== objectId));
  },

  clearSelection: () => {
    setEngineStore('editor', 'viewport', 'selection', []);
  },

  setCurrentTool: (tool) => {
    setEngineStore('editor', 'viewport', 'tools', 'currentTool', tool);
  },

  setGizmoMode: (mode) => {
    setEngineStore('editor', 'viewport', 'tools', 'gizmoMode', mode);
  },

  // Runtime state management
  setPlaying: (playing) => {
    setEngineStore('runtime', 'isPlaying', playing);
    if (playing) {
      setEngineStore('runtime', 'isPaused', false);
    }
  },

  setPaused: (paused) => {
    setEngineStore('runtime', 'isPaused', paused);
  },

  setPlaybackSpeed: (speed) => {
    setEngineStore('runtime', 'playbackSpeed', Math.max(0.1, Math.min(4.0, speed)));
  },

  updatePerformanceStats: (stats) => {
    setEngineStore('runtime', 'performance', stats);
  },

  updateFrameData: (deltaTime, totalTime, frame) => {
    setEngineStore('runtime', 'deltaTime', deltaTime);
    setEngineStore('runtime', 'totalTime', totalTime);
    setEngineStore('runtime', 'currentFrame', frame);
  },

  // Babylon.js runtime object management
  registerBabylonObject: (engineId, babylonObject) => {
    setEngineStore('runtime', 'babylonObjects', engineId, babylonObject);
  },

  unregisterBabylonObject: (engineId) => {
    setEngineStore('runtime', 'babylonObjects', engineId, undefined);
  },

  // System manager registration
  registerSystemManager: (systemName, manager) => {
    setEngineStore('runtime', 'systemManagers', systemName, manager);
  },

  unregisterSystemManager: (systemName) => {
    setEngineStore('runtime', 'systemManagers', systemName, undefined);
  },

  getSystemManager: (systemName) => {
    return engineStore.runtime.systemManagers[systemName];
  },

  // Utility functions
  getCurrentScene: () => {
    const currentSceneId = engineStore.scenes.currentScene;
    return currentSceneId ? engineStore.scenes.sceneData[currentSceneId] : null;
  },

  getSelectedObjects: () => {
    const currentScene = engineActions.getCurrentScene();
    if (!currentScene) return [];
    
    return engineStore.editor.viewport.selection
      .map(id => currentScene.sceneGraph.nodes[id])
      .filter(Boolean);
  },

  getObjectById: (sceneId, objectId) => {
    return engineStore.scenes.sceneData[sceneId]?.sceneGraph?.nodes?.[objectId];
  },

  getAllObjectsInScene: (sceneId) => {
    const sceneData = engineStore.scenes.sceneData[sceneId];
    return sceneData ? Object.values(sceneData.sceneGraph.nodes) : [];
  },

  // Save/Load state management
  markSaved: () => {
    setHasUnsavedChanges(false);
    setLastSaved(new Date());
  },

  markUnsaved: () => {
    setHasUnsavedChanges(true);
  },

  setLoading: (loading) => {
    setIsLoading(loading);
  }
};

// Computed getters for common data access patterns
export const engineGetters = {
  // Current scene helpers
  getCurrentSceneId: () => engineStore.scenes.currentScene,
  getCurrentSceneData: () => engineActions.getCurrentScene(),
  getCurrentSceneObjects: () => {
    const scene = engineActions.getCurrentScene();
    return scene ? Object.values(scene.sceneGraph.nodes) : [];
  },

  // Selection helpers
  getSelection: () => engineStore.editor.viewport.selection,
  getSelectedObjectsData: () => engineActions.getSelectedObjects(),
  hasSelection: () => engineStore.editor.viewport.selection.length > 0,

  // Asset helpers
  getAsset: (category, id) => engineStore.assets[category]?.[id],
  getAllAssets: (category) => engineStore.assets[category] || {},
  getAssetDependencies: (assetId) => engineStore.assets.dependencies[assetId] || [],

  // Runtime helpers
  isPlaying: () => engineStore.runtime.isPlaying,
  isPaused: () => engineStore.runtime.isPaused,
  getBabylonObject: (engineId) => engineStore.runtime.babylonObjects[engineId],
  getSystemManager: (systemName) => engineStore.runtime.systemManagers[systemName],

  // Project helpers
  getProjectMetadata: () => engineStore.project.metadata,
  getProjectSettings: () => engineStore.project.settings,

  // State helpers
  hasUnsavedChanges: () => hasUnsavedChanges(),
  isLoading: () => isLoading(),
  getLastSaved: () => lastSaved()
};

export { engineStore, isLoading, hasUnsavedChanges, lastSaved };