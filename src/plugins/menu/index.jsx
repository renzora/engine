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