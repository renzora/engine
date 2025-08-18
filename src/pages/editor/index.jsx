import { onMount, onCleanup } from 'solid-js';
import { usePluginAPI } from '@/api/plugin';
import { Camera, Grid3x3, Settings as SettingsIcon, Maximize } from '@/ui/icons';
import CameraDropdownContent from '@/ui/display/CameraDropdownContent.jsx';
import GridDropdownContent from '@/ui/display/GridDropdownContent.jsx';

// Import content components (these go INTO the layout)
import Scene from './Scene.jsx';
import SettingsComponent from './Settings.jsx';
import AssetLibrary from './AssetLibrary.jsx';


export default function EditorPage() {
  onMount(() => {
    const pluginAPI = usePluginAPI();
    
    // Register content components into layout regions
    pluginAPI.registerPropertyTab('scene', {
      title: 'Scene',
      component: Scene,
      icon: () => <div>🎬</div>,
      order: 10
    });

    pluginAPI.registerPropertyTab('settings', {
      title: 'Settings',
      component: SettingsComponent,
      icon: SettingsIcon,
      order: 20
    });

    pluginAPI.registerBottomPanelTab('assets', {
      title: 'Assets',
      component: AssetLibrary,
      icon: () => <div>📁</div>,
      order: 10,
      defaultHeight: 300
    });
    
    // Register toolbar buttons
    pluginAPI.registerToolbarButton('camera-helper', {
      title: 'Camera Options',
      icon: Camera,
      section: 'right',
      order: 10,
      hasDropdown: true,
      dropdownComponent: CameraDropdownContent,
      dropdownWidth: 256
    });
    
    pluginAPI.registerToolbarButton('grid-helper', {
      title: 'Grid Options',
      icon: Grid3x3,
      section: 'right',
      order: 20,
      hasDropdown: true,
      dropdownComponent: GridDropdownContent,
      dropdownWidth: 256
    });
    
    pluginAPI.registerToolbarButton('settings-button', {
      title: 'Settings',
      icon: SettingsIcon,
      section: 'right',
      order: 30,
      onClick: () => {
        // Switch to settings tab in properties panel
        console.log('Switch to settings tab');
      }
    });
    
    pluginAPI.registerToolbarButton('fullscreen-button', {
      title: 'Toggle Fullscreen',
      icon: Maximize,
      section: 'right',
      order: 40,
      onClick: () => {
        if (!document.fullscreenElement) {
          document.documentElement.requestFullscreen().catch(err => {
            console.error('Error attempting to enable fullscreen:', err);
          });
        } else {
          document.exitFullscreen();
        }
      }
    });
    
    onCleanup(() => {
      // Cleanup when page unmounts
      // TODO: Add unregister methods to plugin API
    });
  });

  return null; // This component just registers content with the plugin API
}