import { createPlugin } from '@/api/plugin';
import { createSignal } from 'solid-js';
import { IconRefresh, IconVideo, IconEdit, IconArrowLeft, IconArrowRight, IconPlus, IconFolder, IconFile, IconArrowDown, IconArrowUp, IconScissors, IconCopy, IconClipboard, IconTrash, IconCube, IconDownload, IconUpload, IconPhoto, IconDeviceGamepad2, IconWorld, IconDeviceDesktop, IconBox, IconCircle, IconCylinder, IconSquare, IconChartDonutFilled, IconChairDirector, IconNetwork, IconLink, IconHelp, IconHeadphones, IconBrandYoutube, IconBrandDiscord, IconBook, IconInfoCircle, IconPackageExport, IconDeviceFloppy
} from '@tabler/icons-solidjs';
import AboutOverlay from '@/ui/AboutOverlay.jsx';
import ExportDialog from '@/ui/ExportDialog.jsx';
import NewProjectOverlay from '@/ui/NewProjectOverlay.jsx';
import OpenProjectOverlay from '@/ui/OpenProjectOverlay.jsx';
import { sceneManager } from '@/api/scene/SceneManager.js';
import { getCurrentProject, getProjectCurrentScene } from '@/api/bridge/projects.js';
import { projectDataAuditor } from '@/api/debug/ProjectDataAuditor.js';

// About overlay state
const [showAbout, setShowAbout] = createSignal(false);
// Export dialog state
const [showExport, setShowExport] = createSignal(false);
// New project overlay state
const [showNewProject, setShowNewProject] = createSignal(false);
// Open project overlay state
const [showOpenProject, setShowOpenProject] = createSignal(false);

// Handle new project creation with save prompt
const handleNewProject = async () => {
  const currentProject = getCurrentProject();
  
  if (currentProject) {
    // Ask user if they want to save current project
    const shouldSave = confirm(
      `You have project "${currentProject.name}" open. Do you want to save your current scene before creating a new project?`
    );
    
    if (shouldSave) {
      try {
        const result = await sceneManager.saveScene();
        if (!result.success) {
          alert(`Failed to save scene: ${result.error}`);
          return; // Don't proceed if save failed
        }
      } catch (error) {
        console.error('Failed to save scene:', error);
        alert('Failed to save scene. Please try again.');
        return;
      }
    }
  }
  
  // Show the new project overlay
  setShowNewProject(true);
};

// Handle open project with save prompt
const handleOpenProject = async () => {
  const currentProject = getCurrentProject();
  
  if (currentProject) {
    // Ask user if they want to save current project
    const shouldSave = confirm(
      `You have project "${currentProject.name}" open. Do you want to save your current scene before opening another project?`
    );
    
    if (shouldSave) {
      try {
        const result = await sceneManager.saveScene();
        if (!result.success) {
          alert(`Failed to save scene: ${result.error}`);
          return; // Don't proceed if save failed
        }
      } catch (error) {
        console.error('Failed to save scene:', error);
        alert('Failed to save scene. Please try again.');
        return;
      }
    }
  }
  
  // Show the open project overlay
  setShowOpenProject(true);
};

// Handle project selection from open project overlay
const handleProjectSelect = async (project) => {
  // Replicate the EXACT splash screen project switching logic with complete data refresh
  try {
    console.log('🔄 Starting complete project switch to:', project.name);
    
    // Take pre-switch snapshot for debugging
    const currentProject = getCurrentProject();
    if (currentProject) {
      projectDataAuditor.takePreSwitchSnapshot(currentProject.name);
    }
    
    // Step 1: Clear all data stores to ensure clean state
    console.log('🧹 Clearing all data stores...');
    try {
      // Clear asset store
      const { assetsActions } = await import('@/layout/stores/AssetStore.jsx');
      assetsActions.clearAllAssetCache();
      
      // Clear editor store project data
      const { editorActions } = await import('@/layout/stores/EditorStore.jsx');
      editorActions.clearProjectData();
      
      // Clear render store project data
      const { renderActions } = await import('@/render/store.jsx');
      renderActions.clearProjectData();
      
      // Clear viewport stores project data
      const { viewportActions, objectPropertiesActions } = await import('@/layout/stores/ViewportStore.jsx');
      viewportActions.clearProjectData();
      objectPropertiesActions.clearProjectData();
      
    } catch (storeError) {
      console.warn('⚠️ Failed to clear some data stores:', storeError);
    }
    
    // Step 2: Set project in ALL relevant contexts (exactly like splash screen)
    console.log('📝 Setting project in all contexts...');
    const { setCurrentProject: setApiProject } = await import('@/api/bridge/projects.js');
    
    // Set in API (this is critical for all bridge operations)
    setApiProject(project);
    
    // Set in project store context (used by asset library and other components)
    try {
      // Try to access project store through splash plugin context
      const projectStoreSetters = document.querySelectorAll('[data-project-context]');
      projectStoreSetters.forEach(element => {
        if (element.__setCurrentProject) {
          element.__setCurrentProject(project);
        }
      });
      
      // Also try direct context access
      const { Project } = await import('@/plugins/splash/ProjectStore.jsx');
      // This won't work outside context, but it's here for completeness
    } catch (storeError) {
      console.warn('Project store context not available, but API setting should be sufficient');
    }

    // Step 3: Create Babylon scene (this clears the old scene completely)
    if (window._createBabylonScene) {
      try {
        console.log('🎬 Creating fresh Babylon scene (disposing old one)...');
        const scene = await window._createBabylonScene();
        
        if (scene) {
          console.log('✅ Babylon scene created successfully');
          
          // Wait a moment for scene initialization to complete
          await new Promise(resolve => setTimeout(resolve, 200));
          
          // Step 4: Load the project's specific scene (not default content)
          try {
            console.log('📂 Loading project-specific scene...');
            const currentSceneName = await getProjectCurrentScene(project.name);
            console.log('🎯 Loading scene:', currentSceneName, 'for project:', project.name);
            
            const result = await sceneManager.loadScene(currentSceneName);
            
            if (result.success) {
              console.log('✅ Project scene loaded successfully');
              
              // Wait for scene loading to complete before proceeding
              await new Promise(resolve => setTimeout(resolve, 300));
              
              // Force hierarchy refresh if needed
              if (window._cleanBabylonScene) {
                console.log('🔄 Refreshing scene hierarchy...');
                const { renderActions } = await import('@/render/store.jsx');
                renderActions.initializeHierarchy();
              }
            } else {
              console.warn('⚠️ Failed to load project scene:', result.error);
            }
          } catch (error) {
            console.warn('⚠️ Failed to get/load project scene:', error);
          }
        } else {
          console.warn('⚠️ Scene creation returned null/undefined');
        }
      } catch (sceneError) {
        console.error('❌ Failed to create Babylon scene:', sceneError);
        // Continue - the event dispatch will still trigger asset refreshes
      }
    } else {
      console.warn('⚠️ window._createBabylonScene not available');
    }

    // Step 5: Set up UI exactly like splash screen
    const api = document.querySelector('[data-plugin-api]')?.__pluginAPI;
    if (api) {
      console.log('🎨 Setting up interface elements...');
      api.showProps();
      api.showPanel();
      api.showMenu();
      api.showFooter();
      api.showToolbar();
      api.showHelper();
      
      // Create scene viewport
      api.createSceneViewport({
        name: 'Scene 1',
        setActive: true
      });
    }
    
    // Step 6: Dispatch the SAME event that triggers all component refreshes
    console.log('📡 Dispatching project-selected event...');
    document.dispatchEvent(new CustomEvent('engine:project-selected', { 
      detail: { project } 
    }));
    
    // Step 7: Trigger asset refresh explicitly (this ensures bottom panel updates)
    setTimeout(() => {
      console.log('🔄 Triggering asset refresh...');
      document.dispatchEvent(new CustomEvent('engine:assets-refresh', {
        detail: { project, forceRefresh: true }
      }));
    }, 500); // Small delay to ensure project context is fully set
    
    // Take post-switch snapshot for debugging
    setTimeout(() => {
      if (currentProject) {
        projectDataAuditor.takePostSwitchSnapshot(project.name, currentProject.name);
      }
    }, 1000); // Allow time for all async operations to complete
    
    console.log('✅ Complete project switch successful:', project.name);
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
          action: async () => {
            const scenes = await sceneManager.getAvailableScenes();
            if (scenes.length === 0) {
              alert('No scenes found in current project');
              return;
            }
            
            const sceneList = scenes.join('\n');
            const sceneName = prompt(`Available scenes:\n${sceneList}\n\nEnter scene name to load:`);
            if (sceneName && sceneName.trim()) {
              const result = await sceneManager.loadScene(sceneName.trim());
              if (result.success) {
                // Scene loaded successfully
                
                // Switch to existing scene tab instead of creating new one
                const { viewportStore, viewportActions } = await import('@/layout/stores/ViewportStore.jsx');
                
                // Find existing scene tab
                const sceneTab = viewportStore.tabs.find(tab => tab.type === '3d-viewport');
                
                if (sceneTab) {
                  // Switch to existing scene tab
                  viewportActions.setActiveViewportTab(sceneTab.id);
                  // Switched to existing scene tab
                } else {
                  // Only create new tab if none exists
                  // Creating scene viewport
                  api.createSceneViewport({
                    name: sceneName.trim(),
                    setActive: true
                  });
                }
              } else {
                alert(`Failed to load scene: ${result.error}`);
              }
            }
          }
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
          action: async () => {
            const sceneName = prompt('Enter scene name:');
            if (sceneName && sceneName.trim()) {
              const result = await sceneManager.saveScene(sceneName.trim());
              if (result.success) {
                // Scene saved successfully
              } else {
                alert(`Failed to save scene: ${result.error}`);
              }
            }
          }
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
        { 
          id: 'export', 
          label: 'Export Game', 
          icon: IconPackageExport,
          action: () => setShowExport(true)
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
          action: async () => {
            const sceneName = prompt('Enter scene name:');
            if (sceneName && sceneName.trim()) {
              const result = await sceneManager.createNewScene(sceneName.trim());
              if (result.success) {
                // New scene created
                
                // Switch to existing scene tab instead of creating new one
                const { viewportStore, viewportActions } = await import('@/layout/stores/ViewportStore.jsx');
                
                // Find existing scene tab
                const sceneTab = viewportStore.tabs.find(tab => tab.type === '3d-viewport');
                
                if (sceneTab) {
                  // Switch to existing scene tab
                  viewportActions.setActiveViewportTab(sceneTab.id);
                  // Switched to existing scene tab
                } else {
                  // Only create new tab if none exists
                  // Creating scene viewport
                  api.createSceneViewport({
                    name: sceneName.trim(),
                    setActive: true
                  });
                }
              } else {
                alert(`Failed to create scene: ${result.error}`);
              }
            }
          }
        },
        { 
          id: 'mesh', 
          label: 'Mesh', 
          icon: IconCube,
          submenu: [
            { id: 'add-cube', label: 'Cube', icon: IconBox },
            { id: 'add-plane', label: 'Plane', icon: IconSquare },
            { id: 'add-cylinder', label: 'Cylinder', icon: IconCylinder },
            { id: 'add-sphere', label: 'Sphere', icon: IconCircle },
            { id: 'add-torus', label: 'Torus', icon: IconChartDonutFilled }
          ]
        }
      ]
    });

    api.menu('viewports', {
      label: 'Viewports',
      icon: IconChairDirector,
      order: 4,
      submenu: [
        { id: 'viewport-bridge', label: 'Bridge', icon: IconLink },
        { id: 'viewport-web-browser', label: 'Web Browser', icon: IconWorld, 
          action: () => {
            const api = document.querySelector('[data-plugin-api]')?.__pluginAPI;
            if (api) {
              api.open('web-browser', { label: 'Web Browser' });
            }
          }
        }
      ]
    });

    api.menu('help', {
      label: 'Help',
      icon: IconHelp,
      order: 5,
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
    api.registerLayoutComponent('about-overlay', () => (
      <AboutOverlay 
        isOpen={showAbout} 
        onClose={() => setShowAbout(false)} 
      />
    ));
    
    // Register Export dialog component
    api.registerLayoutComponent('export-dialog', () => (
      <ExportDialog 
        isOpen={showExport} 
        onClose={() => setShowExport(false)} 
      />
    ));
    
    // Register New Project overlay component
    api.registerLayoutComponent('new-project-overlay', () => (
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
    ));
    
    // Register Open Project overlay component
    api.registerLayoutComponent('open-project-overlay', () => (
      <OpenProjectOverlay 
        isOpen={showOpenProject} 
        onClose={() => setShowOpenProject(false)}
        onProjectSelect={handleProjectSelect}
      />
    ));
  }
});