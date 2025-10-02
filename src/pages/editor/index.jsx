import { onMount, onCleanup, createSignal, createEffect, For, Show } from 'solid-js';
import { usePluginAPI } from '@/api/plugin';
import { 
  IconCamera, IconGrid3x3, IconSettings as SettingsIcon, IconMaximize, IconVideo, IconFolder, IconGridDots, IconClock, IconSun
} from '@tabler/icons-solidjs';
import ThemeSwitcher from '@/ui/ThemeSwitcher';

import Scene from './Scene.jsx';
import SettingsDropdownContent from '@/ui/display/SettingsDropdownContent.jsx';
import AssetLibrary from './AssetLibrary';

import { scriptEditorStore, scriptEditorActions } from '../../layout/stores/ScriptEditorStore.js';
import { getCurrentProject } from '@/api/bridge/projects';
import { readFile, writeFile } from '@/api/bridge/files';


export default function EditorPage() {
  onMount(() => {
    // Initializing editor components
    const api = usePluginAPI();
    

    // Settings moved to toolbar helper instead of tab


    api.panel('assets', {
      title: 'Assets',
      component: AssetLibrary,
      icon: IconFolder,
      order: 10,
      defaultHeight: 300
    });

    // Settings helper with dropdown
    api.helper('settings-button', {
      title: 'Settings',
      icon: SettingsIcon,
      order: 30,
      hasDropdown: true,
      dropdownComponent: SettingsDropdownContent,
      dropdownWidth: 320
    });

    api.helper('fullscreen-button', {
      title: 'Toggle Fullscreen',
      icon: IconMaximize,
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

    // Editor components registered
    
    onCleanup(() => {
      // Cleaning up editor components
    });
  });

  return null;
}