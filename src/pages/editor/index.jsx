import { onMount, onCleanup } from 'solid-js';
import { usePluginAPI } from '@/api/plugin';
import { Camera, Grid3x3, Settings as SettingsIcon, Maximize, Video, Folder } from '@/ui/icons';
import CameraDropdownContent from '@/ui/display/CameraDropdownContent.jsx';
import GridDropdownContent from '@/ui/display/GridDropdownContent.jsx';
import ThemeSwitcher from '@/ui/ThemeSwitcher';

import Scene from './Scene.jsx';
import SettingsComponent from './Settings.jsx';
import AssetLibrary from './AssetLibrary.jsx';

export default function EditorPage() {
  onMount(() => {
    console.log('[EditorPage] Initializing editor components...');
    const api = usePluginAPI();
    
    api.tab('scene', {
      title: 'Scene',
      component: Scene,
      icon: Video,
      order: 10
    });

    api.tab('settings', {
      title: 'Settings',
      component: SettingsComponent,
      icon: SettingsIcon,
      order: 20
    });

    api.panel('assets', {
      title: 'Assets',
      component: AssetLibrary,
      icon: Folder,
      order: 10,
      defaultHeight: 300
    });
    
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
    
    api.button('theme-switcher', {
      title: 'Theme',
      section: 'right',
      order: 30,
      isCustomComponent: true,
      customComponent: ThemeSwitcher
    });
    
    api.button('fullscreen-button', {
      title: 'Toggle Fullscreen',
      icon: Maximize,
      section: 'right',
      order: 40,
      onClick: () => {
        if (!document.fullscreenElement) {
          document.documentElement.requestFullscreen().catch(err => {
            console.error('[EditorPage] Error attempting to enable fullscreen:', err);
          });
        } else {
          document.exitFullscreen();
        }
      }
    });

    console.log('[EditorPage] Editor components registered');
    
    onCleanup(() => {
      console.log('[EditorPage] Cleaning up editor components...');
    });
  });

  return null;
}