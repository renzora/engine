import { createPlugin } from '@/api/plugin';
import { createSignal } from 'solid-js';
import { IconRefresh, IconVideo, IconEdit, IconArrowLeft, IconArrowRight, IconPlus, IconFolder, IconFile, IconArrowDown, IconArrowUp, IconScissors, IconCopy, IconClipboard, IconTrash, IconCube, IconDownload, IconUpload, IconPhoto, IconDeviceGamepad2, IconWorld, IconDeviceDesktop, IconBox, IconCircle, IconCylinder, IconSquare, IconChartDonutFilled, IconChairDirector, IconNetwork, IconLink, IconHelp, IconHeadphones, IconBrandYoutube, IconBrandDiscord, IconBook, IconInfoCircle, IconPackageExport, IconDeviceFloppy
} from '@tabler/icons-solidjs';
import AboutOverlay from '@/ui/AboutOverlay.jsx';
import ExportDialog from '@/ui/ExportDialog.jsx';
import { sceneManager } from '@/api/scene/SceneManager.js';

// About overlay state
const [showAbout, setShowAbout] = createSignal(false);
// Export dialog state
const [showExport, setShowExport] = createSignal(false);

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
          icon: IconPlus
        },
        { id: 'open', label: 'Open Project', icon: IconFolder, shortcut: 'Ctrl+O' },
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
  }
});