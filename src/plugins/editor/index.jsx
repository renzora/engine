import { createPlugin } from '@/api/plugin';
import { Camera, Grid3x3, Settings as SettingsIcon, Maximize, Folder } from '@/ui/icons';

// Import standalone content components
import Scene from '@/pages/editor/Scene.jsx';
import SettingsComponent from '@/pages/editor/Settings.jsx';
import AssetLibrary from '@/pages/editor/AssetLibrary.jsx';
import CameraDropdownContent from '@/ui/display/CameraDropdownContent.jsx';
import GridDropdownContent from '@/ui/display/GridDropdownContent.jsx';

const SceneIcon = () => <div>🎬</div>;
const AssetsIcon = () => <div>📁</div>;

export default createPlugin({
  id: 'editor-plugin',
  name: 'Editor Plugin',
  version: '1.0.0',
  description: 'Core editor UI components and tools',
  author: 'Renzora Engine Team',

  async onInit(api) {
    console.log('[EditorPlugin] Initializing editor plugin...');
  },

  async onStart(api) {
    console.log('[EditorPlugin] Starting editor plugin...');
    
    // Register property tabs (right panel)
    api.tab('scene', {
      title: 'Scene',
      component: Scene,
      icon: SceneIcon,
      order: 10
    });

    api.tab('settings', {
      title: 'Settings',
      component: SettingsComponent,
      icon: SettingsIcon,
      order: 20
    });

    // Register bottom panel tabs
    api.panel('assets', {
      title: 'Assets',
      component: AssetLibrary,
      icon: AssetsIcon,
      order: 10,
      defaultHeight: 300
    });
    
    // Register toolbar buttons (right side of horizontal toolbar)
    api.button('camera-helper', {
      title: 'Camera Options',
      icon: Camera,
      section: 'right',
      order: 10,
      hasDropdown: true,
      dropdownComponent: CameraDropdownContent,
      dropdownWidth: 256
    });
    
    api.button('grid-helper', {
      title: 'Grid Options',
      icon: Grid3x3,
      section: 'right',
      order: 20,
      hasDropdown: true,
      dropdownComponent: GridDropdownContent,
      dropdownWidth: 256
    });
    
    api.button('settings-button', {
      title: 'Settings',
      icon: SettingsIcon,
      section: 'right',
      order: 30,
      onClick: () => {
        // Switch to settings tab in properties panel
        console.log('[EditorPlugin] Settings button clicked');
        // TODO: Add logic to switch to settings tab
      }
    });
    
    api.button('fullscreen-button', {
      title: 'Toggle Fullscreen',
      icon: Maximize,
      section: 'right',
      order: 40,
      onClick: () => {
        if (!document.fullscreenElement) {
          document.documentElement.requestFullscreen().catch(err => {
            console.error('[EditorPlugin] Error attempting to enable fullscreen:', err);
          });
        } else {
          document.exitFullscreen();
        }
      }
    });

    console.log('[EditorPlugin] Editor plugin started');
  },

  async onStop() {
    console.log('[EditorPlugin] Stopping editor plugin...');
  },

  async onDispose() {
    console.log('[EditorPlugin] Disposing editor plugin...');
  }
});