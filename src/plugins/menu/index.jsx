import { createPlugin } from '@/api/plugin';
import { createSignal } from 'solid-js';
import { IconRefresh, IconVideo, IconEdit, IconArrowLeft, IconArrowRight, IconPlus, IconFolder, IconFile, IconArrowDown, IconScissors, IconCopy, IconClipboard, IconTrash, IconCube, IconWorld, IconBox, IconCircle, IconCylinder, IconSquare, IconChairDirector, IconLink, IconHelp, IconHeadphones, IconBrandYoutube, IconBrandDiscord, IconBook, IconInfoCircle, IconDeviceFloppy, IconMountain, IconSun, IconBulb, IconSphere, IconPackage, IconSettings
} from '@tabler/icons-solidjs';
import AboutOverlay from '@/ui/AboutOverlay.jsx';
import NewProjectOverlay from '@/ui/NewProjectOverlay.jsx';
import OpenProjectOverlay from '@/ui/OpenProjectOverlay.jsx';
import { sceneManager } from '@/api/scene/SceneManager.js';
import { getCurrentProject } from '@/api/bridge/projects.js';
import UnsavedChangesOverlay from '@/ui/UnsavedChangesOverlay.jsx';
import SceneSelectionOverlay from '@/ui/SceneSelectionOverlay.jsx';
import SaveAsOverlay from '@/ui/SaveAsOverlay.jsx';
import PluginUploadOverlay from '@/components/PluginUploadOverlay.jsx';

// About overlay state
const [showAbout, setShowAbout] = createSignal(false);
// New project overlay state
const [showNewProject, setShowNewProject] = createSignal(false);
// Open project overlay state
const [showOpenProject, setShowOpenProject] = createSignal(false);
// Scene selection overlay state
const [showSceneSelection, setShowSceneSelection] = createSignal(false);
// Save As overlay state
const [showSaveAs, setShowSaveAs] = createSignal(false);
// Unsaved changes overlay state
const [showUnsavedChanges, setShowUnsavedChanges] = createSignal(false);
const [pendingAction, setPendingAction] = createSignal(null);
// Plugin upload overlay state
const [showPluginUpload, setShowPluginUpload] = createSignal(false);

// Helper function to check for unsaved changes and handle accordingly
const checkUnsavedChanges = async (action) => {
  try {
    // Import unsaved changes store
    const { unsavedChangesStore } = await import('@/stores/UnsavedChangesStore.jsx');
    
    if (unsavedChangesStore.hasChanges) {
      // Show unsaved changes overlay
      setPendingAction(() => action);
      setShowUnsavedChanges(true);
      return false; // Don't proceed with action yet
    } else {
      // No unsaved changes, proceed with action
      action();
      return true;
    }
  } catch (error) {
    console.warn('Failed to check unsaved changes:', error);
    // If there's an error checking, proceed with action
    action();
    return true;
  }
};

// Handle save changes from unsaved changes overlay
const handleSaveChanges = async () => {
  try {
    const result = await sceneManager.saveScene();
    if (result.success) {
      console.log('✅ Changes saved successfully');
      return true;
    } else {
      console.error('❌ Failed to save changes:', result.error);
      throw new Error(result.error);
    }
  } catch (error) {
    console.error('❌ Failed to save changes:', error);
    throw error;
  }
};

// Handle discard changes from unsaved changes overlay
const handleDiscardChanges = () => {
  // Clear the unsaved changes store
  import('@/stores/UnsavedChangesStore.jsx').then(({ unsavedChangesActions }) => {
    unsavedChangesActions.clearChanges();
  });
  
  // Execute the pending action
  const action = pendingAction();
  if (action) {
    action();
  }
};

// Handle new project creation with unsaved changes check
const handleNewProject = async () => {
  const proceedWithNewProject = () => {
    setShowNewProject(true);
  };
  
  // Check for unsaved changes before proceeding
  checkUnsavedChanges(proceedWithNewProject);
};

// Handle open project with unsaved changes check
const handleOpenProject = async () => {
  const proceedWithOpenProject = () => {
    setShowOpenProject(true);
  };
  
  // Check for unsaved changes before proceeding
  checkUnsavedChanges(proceedWithOpenProject);
};

// Handle scene selection
const handleSceneSelect = async (sceneName) => {
  try {
    const result = await sceneManager.loadScene(sceneName);
    if (result.success) {
      // Scene loaded successfully
      
      // Switch to existing scene tab instead of creating new one
      const { viewportStore, viewportActions } = await import('@/layout/stores/ViewportStore.jsx');
      
      // Find existing scene tab
      const sceneTab = viewportStore.tabs.find(tab => tab.type === '3d-viewport');
      
      if (sceneTab) {
        // Switch to existing scene tab
        viewportActions.setActiveViewportTab(sceneTab.id);
      } else {
        // Only create new tab if none exists
        const api = document.querySelector('[data-plugin-api]')?.__pluginAPI;
        if (api) {
          api.createSceneViewport({
            name: sceneName,
            setActive: true
          });
        }
      }
    } else {
      alert(`Failed to load scene: ${result.error}`);
    }
  } catch (error) {
    console.error('Failed to load scene:', error);
    alert('Failed to load the selected scene. Please try again.');
  }
};

// Handle scene creation
const handleSceneCreate = async (sceneName) => {
  try {
    const result = await sceneManager.createNewScene(sceneName);
    if (result.success) {
      // New scene created successfully
      
      // Switch to existing scene tab instead of creating new one
      const { viewportStore, viewportActions } = await import('@/layout/stores/ViewportStore.jsx');
      
      // Find existing scene tab
      const sceneTab = viewportStore.tabs.find(tab => tab.type === '3d-viewport');
      
      if (sceneTab) {
        // Switch to existing scene tab
        viewportActions.setActiveViewportTab(sceneTab.id);
      } else {
        // Only create new tab if none exists
        const api = document.querySelector('[data-plugin-api]')?.__pluginAPI;
        if (api) {
          api.createSceneViewport({
            name: sceneName,
            setActive: true
          });
        }
      }
    } else {
      throw new Error(result.error || 'Failed to create scene');
    }
  } catch (error) {
    console.error('Failed to create scene:', error);
    throw error; // Re-throw to let the overlay handle the error display
  }
};

// Handle terrain creation
const handleTerrainCreate = async () => {
  try {
    // Dispatch terrain creation event that terrain plugin will listen to
    document.dispatchEvent(new CustomEvent('engine:create-terrain'));
  } catch (error) {
    console.error('Failed to create terrain:', error);
  }
};

// Handle skybox creation
const handleSkyboxCreate = async () => {
  try {
    console.log('🌍 Skybox creation requested...');
    // Dispatch skybox creation event that environment plugin will listen to
    document.dispatchEvent(new CustomEvent('engine:create-skybox'));
    console.log('🌍 Event dispatched: engine:create-skybox');
  } catch (error) {
    console.error('Failed to create skybox:', error);
  }
};

// Handle object creation using unified system
const handleObjectCreate = async (type) => {
  try {
    const { renderStore } = await import('@/render/store');
    const { createAndAddObject } = await import('@/api/creation/ObjectCreationUtils.jsx');
    
    const scene = renderStore.scene;
    if (!scene) {
      console.error('No active scene');
      return;
    }

    // Use unified creation system for consistent sizes and colors
    createAndAddObject(type, scene);
  } catch (error) {
    console.error('Failed to create object:', error);
  }
};

// Handle light creation using unified system  
const handleLightCreate = async (type) => {
  try {
    const { renderStore } = await import('@/render/store');
    const { createAndAddObject } = await import('@/api/creation/ObjectCreationUtils.jsx');
    
    const scene = renderStore.scene;
    if (!scene) {
      console.error('No active scene');
      return;
    }

    // Use unified creation system for consistent behavior
    createAndAddObject(`${type}-light`, scene);
  } catch (error) {
    console.error('Failed to create light:', error);
  }
};

// Handle camera creation using unified system
const handleCameraCreate = async () => {
  try {
    const { renderStore } = await import('@/render/store');
    const { createAndAddObject } = await import('@/api/creation/ObjectCreationUtils.jsx');
    
    const scene = renderStore.scene;
    if (!scene) {
      console.error('No active scene');
      return;
    }

    // Use unified creation system for consistent behavior
    createAndAddObject('camera', scene);
  } catch (error) {
    console.error('Failed to create camera:', error);
  }
};

// Handle load scene with unsaved changes check
const handleLoadScene = async () => {
  const proceedWithLoadScene = () => {
    setShowSceneSelection(true);
  };
  
  // Check for unsaved changes before proceeding
  checkUnsavedChanges(proceedWithLoadScene);
};

// Handle save as
const handleSaveAs = async (sceneName) => {
  try {
    const result = await sceneManager.saveScene(sceneName);
    if (result.success) {
      console.log('✅ Scene saved as:', sceneName);
      return true;
    } else {
      throw new Error(result.error || 'Failed to save scene');
    }
  } catch (error) {
    console.error('❌ Failed to save scene as:', error);
    throw error;
  }
};

// Handle project selection from open project overlay
const handleProjectSelect = async (project) => {
  try {
    console.log('🔄 Starting project switch to:', project.name);
    
    // Set the project in the API first so it persists through the refresh
    const { setCurrentProject: setApiProject } = await import('@/api/bridge/projects.js');
    setApiProject(project);
    
    // Store the project selection in localStorage to persist through refresh
    localStorage.setItem('pendingProjectLoad', JSON.stringify({
      project,
      timestamp: Date.now()
    }));
    
    console.log('🔄 Refreshing app for clean project load...');
    
    // Refresh the entire page for a completely clean state
    window.location.reload();
    
  } catch (error) {
    console.error('❌ Failed to switch to project:', error);
    alert('Failed to switch to the selected project. Please try again.');
  }
};


export default createPlugin({
  id: 'menu-plugin',
  name: 'Menu Plugin',
  version: '1.0.0',
  description: 'Core application menu items',
  author: 'Renzora Engine Team',

  async onInit() {
    // Menu plugin initialized
  },

  async onStart(api) {
    // Registering menu items

    api.menu('file', {
      label: 'File',
      icon: IconFile,
      order: 1,
      submenu: [
        { 
          id: 'new', 
          label: 'New Project', 
          icon: IconPlus,
          action: handleNewProject
        },
        { 
          id: 'open', 
          label: 'Open Project', 
          icon: IconFolder, 
          shortcut: 'Ctrl+O',
          action: handleOpenProject
        },
        { 
          id: 'load-scene', 
          label: 'Load Scene', 
          icon: IconFolder,
          action: handleLoadScene
        },
        { 
          id: 'save', 
          label: 'Save Scene', 
          icon: IconDeviceFloppy, 
          shortcut: 'Ctrl+S',
          action: async () => {
            const result = await sceneManager.saveScene();
            if (result.success) {
              // Scene saved successfully
            } else {
              alert(`Failed to save scene: ${result.error}`);
            }
          }
        },
        { 
          id: 'save-as', 
          label: 'Save Scene As...', 
          icon: IconDeviceFloppy, 
          shortcut: 'Ctrl+Shift+S',
          action: () => setShowSaveAs(true)
        },
        { divider: true },
        { 
          id: 'import', 
          label: 'Import', 
          icon: IconArrowDown,
          action: () => {
            document.dispatchEvent(new CustomEvent('engine:open-model-importer'));
          }
        },
        { divider: true },
        { id: 'recent', label: 'Recent Projects', icon: IconRefresh },
      ],
      onClick: () => {
        // File menu clicked
      }
    });

    api.menu('edit', {
      label: 'Edit',
      icon: IconEdit,
      order: 2,
      submenu: [
        { id: 'undo', label: 'Undo', icon: IconArrowLeft, shortcut: 'Ctrl+Z' },
        { id: 'redo', label: 'Redo', icon: IconArrowRight, shortcut: 'Ctrl+Y' },
        { divider: true },
        { id: 'cut', label: 'Cut', icon: IconScissors, shortcut: 'Ctrl+X' },
        { id: 'copy', label: 'Copy', icon: IconCopy, shortcut: 'Ctrl+C' },
        { id: 'paste', label: 'Paste', icon: IconClipboard, shortcut: 'Ctrl+V' },
        { id: 'duplicate', label: 'Duplicate', icon: IconCopy, shortcut: 'Ctrl+D' },
        { id: 'delete', label: 'Delete', icon: IconTrash, shortcut: 'Delete' },
        { divider: true },
        { id: 'select-all', label: 'Select All', shortcut: 'Ctrl+A' },
      ],
      onClick: () => {
        // Edit menu clicked
      }
    });

    api.menu('create', {
      label: 'Create',
      icon: IconPlus,
      order: 3,
      submenu: [
        { 
          id: 'create-scene', 
          label: 'Scene', 
          icon: IconChairDirector,
          action: handleLoadScene
        },
        { 
          id: 'object', 
          label: 'Object', 
          icon: IconCube,
          submenu: [
            { id: 'add-cube', label: 'Cube', icon: IconBox, action: () => handleObjectCreate('cube') },
            { id: 'add-sphere', label: 'Sphere', icon: IconCircle, action: () => handleObjectCreate('sphere') },
            { id: 'add-cylinder', label: 'Cylinder', icon: IconCylinder, action: () => handleObjectCreate('cylinder') },
            { id: 'add-plane', label: 'Plane', icon: IconSquare, action: () => handleObjectCreate('plane') }
          ]
        },
        { 
          id: 'light', 
          label: 'Light', 
          icon: IconBulb,
          submenu: [
            { id: 'add-point-light', label: 'Point Light', icon: IconBulb, action: () => handleLightCreate('point') },
            { id: 'add-spot-light', label: 'Spot Light', icon: IconBulb, action: () => handleLightCreate('spot') },
            { id: 'add-hemispheric-light', label: 'Hemispheric Light', icon: IconSun, action: () => handleLightCreate('hemispheric') },
            { id: 'add-directional-light', label: 'Directional Light', icon: IconSun, action: () => handleLightCreate('directional') }
          ]
        },
        { 
          id: 'camera', 
          label: 'Camera', 
          icon: IconVideo,
          action: handleCameraCreate
        },
        { 
          id: 'environment', 
          label: 'Environment', 
          icon: IconSphere,
          submenu: [
            { 
              id: 'skybox', 
              label: 'Skybox', 
              icon: IconSun,
              action: handleSkyboxCreate
            },
            { 
              id: 'terrain', 
              label: 'Terrain', 
              icon: IconMountain,
              action: handleTerrainCreate
            }
          ]
        }
      ]
    });

    api.menu('tools', {
      label: 'Tools',
      icon: IconSettings,
      order: 4,
      submenu: [
        { 
          id: 'install-plugin', 
          label: 'Install Plugin...', 
          icon: IconPackage,
          action: () => setShowPluginUpload(true)
        }
      ]
    });


    api.menu('help', {
      label: 'Help',
      icon: IconHelp,
      order: 6,
      submenu: [
        { id: 'help-support', label: 'Support', icon: IconHeadphones },
        { id: 'help-youtube', label: 'YouTube', icon: IconBrandYoutube },
        { id: 'help-discord', label: 'Discord', icon: IconBrandDiscord },
        { id: 'help-documentation', label: 'Documentation', icon: IconBook },
        { id: 'help-about', label: 'About', icon: IconInfoCircle, 
          action: () => setShowAbout(true) }
      ]
    });

    // All menu items registered
    
    // Register About overlay component
    api.registerLayoutComponent('about-overlay', () => {
      return (
        <AboutOverlay 
          isOpen={showAbout} 
          onClose={() => setShowAbout(false)} 
        />
      );
    });
    
    
    // Register New Project overlay component
    api.registerLayoutComponent('new-project-overlay', () => {
      return (
        <NewProjectOverlay 
          isOpen={showNewProject} 
          onClose={() => setShowNewProject(false)}
          onProjectSelect={handleProjectSelect}
          reloadProjects={async () => {
            // For menu context, we don't need to reload a project list
            // but this prop is required by NewProjectOverlay for splash screen compatibility
            console.log('Menu context: project reload not needed');
          }}
        />
      );
    });
    
    // Register Open Project overlay component
    api.registerLayoutComponent('open-project-overlay', () => {
      return (
        <OpenProjectOverlay 
          isOpen={showOpenProject} 
          onClose={() => setShowOpenProject(false)}
          onProjectSelect={handleProjectSelect}
        />
      );
    });
    
    // Register Scene Selection overlay component
    api.registerLayoutComponent('scene-selection-overlay', () => {
      return (
        <SceneSelectionOverlay 
          isOpen={showSceneSelection} 
          onClose={() => setShowSceneSelection(false)}
          onSceneSelect={handleSceneSelect}
          onCreateScene={handleSceneCreate}
        />
      );
    });
    
    // Register Save As overlay component
    api.registerLayoutComponent('save-as-overlay', () => {
      return (
        <SaveAsOverlay 
          isOpen={showSaveAs} 
          onClose={() => setShowSaveAs(false)}
          onSave={handleSaveAs}
          currentSceneName={sceneManager.getCurrentSceneName()}
        />
      );
    });
    
    // Register Unsaved Changes overlay component
    api.registerLayoutComponent('unsaved-changes-overlay', () => {
      // Get changes from store (will be reactive)
      let changes = [];
      try {
        import('@/stores/UnsavedChangesStore.jsx').then(({ unsavedChangesStore }) => {
          changes = unsavedChangesStore.changes || [];
        });
      } catch {
        changes = [];
      }
      
      return (
        <UnsavedChangesOverlay 
          isOpen={showUnsavedChanges} 
          onClose={() => {
            setShowUnsavedChanges(false);
            setPendingAction(null);
          }}
          onSave={async () => {
            await handleSaveChanges();
            const action = pendingAction();
            if (action) {
              action();
            }
          }}
          onDiscard={handleDiscardChanges}
          projectName={getCurrentProject()?.name}
          changes={changes}
        />
      );
    });
    
    // Register Plugin Upload overlay component
    api.registerLayoutComponent('plugin-upload-overlay', () => {
      return (
        <PluginUploadOverlay 
          isOpen={showPluginUpload()} 
          onClose={() => setShowPluginUpload(false)}
        />
      );
    });
  }
});